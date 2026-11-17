//! SQLite persistence. Only metadata and Telegram file identifiers are stored;
//! audio bytes are never kept by the bot.
//!
//! Cache key: (`track_key`, `quality`) per ТЗ §8 — the same track is cached per
//! bitrate. Search cache is two-tiered: moka (in-memory, L1) in the manager and
//! this `search_cache` table (L2) which survives restarts.

use common::domain::{CachedTrack, Lang, Provider, Stats, TrackKey, TrackMeta, UserRecord};
use common::error::{AppError, AppResult};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{QueryBuilder, Row, SqlitePool};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::warn;

const SCHEMA: &str = include_str!("../../../migrations/0001_init.sql");
const MIGRATION_0002: &str = include_str!("../../../migrations/0002_user_search_source.sql");
const MIGRATION_0004: &str = include_str!("../../../migrations/0004_lastfm.sql");
const MIGRATION_0005: &str = include_str!("../../../migrations/0005_hq_cover.sql");
const MIGRATION_0006: &str = include_str!("../../../migrations/0006_meta.sql");
const MIGRATION_0007: &str = include_str!("../../../migrations/0007_lastfm_stats.sql");
const MIGRATION_0008: &str = include_str!("../../../migrations/0008_genre_analysis.sql");

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
    /// Last time each user's `last_seen_at` was written. The message hot path
    /// only reads the user row; this throttle keeps it from turning every
    /// message into a write transaction.
    seen: Arc<Mutex<HashMap<i64, i64>>>,
}

/// `last_seen_at` is bookkeeping for the release broadcast and admin stats;
/// one write per active user per five minutes is plenty.
const LAST_SEEN_INTERVAL_SECS: i64 = 5 * 60;

impl Db {
    pub async fn connect(database_url: &str) -> AppResult<Self> {
        if let Some(path) = database_url.strip_prefix("sqlite://")
            && path != ":memory:"
            && let Some(parent) = Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::from_str(database_url)
            .map_err(|error| AppError::Config(format!("DATABASE_URL: {error}")))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(10))
            .foreign_keys(true)
            // Read-heavy cache workload: 64 MiB of page cache and in-memory
            // temp tables. Measured: mmap_size made reads *slower* on this
            // class of tiny hot DB (see docs/decisions.md), so it is off.
            .pragma("cache_size", "-65536")
            .pragma("temp_store", "MEMORY")
            .statement_cache_capacity(256);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .min_connections(1)
            .connect_with(options)
            .await
            .map_err(db_err)?;

        sqlx::raw_sql(SCHEMA).execute(&pool).await.map_err(db_err)?;
        // 0001 is IF NOT EXISTS and never touches an existing users table, so
        // the new column is added here when missing.
        let has_search_source: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'search_source'",
        )
        .fetch_one(&pool)
        .await
        .map_err(db_err)?;
        if has_search_source == 0 {
            sqlx::raw_sql(MIGRATION_0002)
                .execute(&pool)
                .await
                .map_err(db_err)?;
        }
        // 0003: per-user tags toggle + genres column on tracks. The users and
        // tracks columns are checked independently so partial states upgrade
        // cleanly (kept in migrations/0003_tags.sql for reference).
        let has_tags_enabled: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'tags_enabled'",
        )
        .fetch_one(&pool)
        .await
        .map_err(db_err)?;
        let has_genres: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('tracks') WHERE name = 'genres'",
        )
        .fetch_one(&pool)
        .await
        .map_err(db_err)?;
        if has_tags_enabled == 0 {
            sqlx::query("ALTER TABLE users ADD COLUMN tags_enabled INTEGER NOT NULL DEFAULT 0")
                .execute(&pool)
                .await
                .map_err(db_err)?;
        }
        if has_genres == 0 {
            sqlx::query("ALTER TABLE tracks ADD COLUMN genres TEXT NOT NULL DEFAULT '[]'")
                .execute(&pool)
                .await
                .map_err(db_err)?;
        }
        let has_lastfm_user: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'lastfm_user'",
        )
        .fetch_one(&pool)
        .await
        .map_err(db_err)?;
        if has_lastfm_user == 0 {
            sqlx::raw_sql(MIGRATION_0004)
                .execute(&pool)
                .await
                .map_err(db_err)?;
        }
        let has_hq_cover: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'hq_cover'",
        )
        .fetch_one(&pool)
        .await
        .map_err(db_err)?;
        if has_hq_cover == 0 {
            sqlx::raw_sql(MIGRATION_0005)
                .execute(&pool)
                .await
                .map_err(db_err)?;
        }
        // 0006: key/value store for the once-per-version update notice.
        sqlx::raw_sql(MIGRATION_0006)
            .execute(&pool)
            .await
            .map_err(db_err)?;
        let has_lastfm_stats: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'lastfm_stats'",
        )
        .fetch_one(&pool)
        .await
        .map_err(db_err)?;
        if has_lastfm_stats == 0 {
            sqlx::raw_sql(MIGRATION_0007)
                .execute(&pool)
                .await
                .map_err(db_err)?;
        }
        let has_genre_analysis: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'genre_analysis'",
        )
        .fetch_one(&pool)
        .await
        .map_err(db_err)?;
        if has_genre_analysis == 0 {
            sqlx::raw_sql(MIGRATION_0008)
                .execute(&pool)
                .await
                .map_err(db_err)?;
        }

        // Drop expired L2 search cache rows; they are cheap to recompute.
        let now = unix_now();
        let _ = sqlx::query("DELETE FROM search_cache WHERE expires_at < ?")
            .bind(now)
            .execute(&pool)
            .await;

        Ok(Self {
            pool,
            seen: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Lightweight liveness check used by /healthz.
    pub async fn ping(&self) -> bool {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|value| value == 1)
            .unwrap_or(false)
    }

    pub async fn get_track(&self, key: &TrackKey, quality: u32) -> AppResult<Option<CachedTrack>> {
        let row = sqlx::query(
            "SELECT track_key, quality, file_id, file_unique_id, source_url, provider, title,
                    performer, album, duration_secs, size_bytes, thumbnail_url, genres
             FROM tracks WHERE track_key = ? AND quality = ?",
        )
        .bind(key.as_str())
        .bind(i64::from(quality))
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(|row| {
            let provider = row
                .get::<String, _>("provider")
                .parse::<Provider>()
                .unwrap_or(Provider::YouTube);
            let duration = row
                .get::<Option<i64>, _>("duration_secs")
                .and_then(|value| u32::try_from(value).ok());
            CachedTrack {
                key: TrackKey(row.get::<String, _>("track_key")),
                file_id: row.get("file_id"),
                file_unique_id: row.get("file_unique_id"),
                source_url: row.get("source_url"),
                provider,
                meta: TrackMeta {
                    title: row.get("title"),
                    performer: row.get("performer"),
                    album: row.get("album"),
                    duration_secs: duration,
                    thumbnail_url: row.get("thumbnail_url"),
                    genres: serde_json::from_str(row.get::<String, _>("genres").as_str())
                        .unwrap_or_default(),
                },
            }
        }))
    }

    pub async fn put_track(
        &self,
        track: &CachedTrack,
        quality: u32,
        size_bytes: Option<i64>,
    ) -> AppResult<()> {
        let now = unix_now();
        let genres = serde_json::to_string(&track.meta.genres).unwrap_or_else(|_| "[]".to_owned());
        sqlx::query(
            "INSERT INTO tracks
                (track_key, quality, file_id, file_unique_id, source_url, provider, title, performer,
                 album, duration_secs, size_bytes, thumbnail_url, genres, created_at, last_used_at, uses)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
             ON CONFLICT(track_key, quality) DO UPDATE SET
                file_id = excluded.file_id,
                file_unique_id = excluded.file_unique_id,
                source_url = excluded.source_url,
                provider = excluded.provider,
                title = excluded.title,
                performer = excluded.performer,
                album = excluded.album,
                duration_secs = excluded.duration_secs,
                size_bytes = excluded.size_bytes,
                thumbnail_url = excluded.thumbnail_url,
                genres = excluded.genres,
                last_used_at = excluded.last_used_at,
                uses = tracks.uses + 1",
        )
        .bind(track.key.as_str())
        .bind(i64::from(quality))
        .bind(&track.file_id)
        .bind(&track.file_unique_id)
        .bind(&track.source_url)
        .bind(track.provider.as_str())
        .bind(&track.meta.title)
        .bind(&track.meta.performer)
        .bind(&track.meta.album)
        .bind(track.meta.duration_secs.map(i64::from))
        .bind(size_bytes)
        .bind(&track.meta.thumbnail_url)
        .bind(&genres)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn touch_track(&self, key: &TrackKey, quality: u32) -> AppResult<()> {
        sqlx::query(
            "UPDATE tracks SET last_used_at = ?, uses = uses + 1 WHERE track_key = ? AND quality = ?",
        )
        .bind(unix_now())
        .bind(key.as_str())
        .bind(i64::from(quality))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn purge_track(&self, key: &TrackKey) -> AppResult<bool> {
        let result = sqlx::query("DELETE FROM tracks WHERE track_key = ?")
            .bind(key.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    /// One statement instead of three round trips; `/stats` runs on demand but
    /// the three scans used to hit the same rows separately.
    pub async fn stats(&self) -> AppResult<Stats> {
        let row = sqlx::query(
            "SELECT
                 (SELECT COUNT(*) FROM tracks) AS tracks,
                 (SELECT COUNT(*) FROM users) AS users,
                 (SELECT COALESCE(SUM(uses), 0) FROM tracks) AS sends",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(Stats {
            tracks: row.get::<i64, _>("tracks"),
            users: row.get::<i64, _>("users"),
            sends: row.get::<i64, _>("sends"),
        })
    }

    /// L2 search cache: fetch a cached payload if it has not expired.
    pub async fn get_search_cache(&self, provider: &str, query: &str) -> AppResult<Option<String>> {
        let row = sqlx::query(
            "SELECT result FROM search_cache WHERE provider = ? AND query = ? AND expires_at >= ?",
        )
        .bind(provider)
        .bind(query)
        .bind(unix_now())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|row| row.get::<String, _>("result")))
    }

    /// L2 search cache: store a payload with a TTL.
    pub async fn put_search_cache(
        &self,
        provider: &str,
        query: &str,
        result_json: &str,
        ttl: Duration,
    ) -> AppResult<()> {
        let expires_at = unix_now() + ttl.as_secs() as i64;
        sqlx::query(
            "INSERT INTO search_cache (provider, query, result, expires_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(provider, query) DO UPDATE SET
                result = excluded.result,
                expires_at = excluded.expires_at",
        )
        .bind(provider)
        .bind(query)
        .bind(result_json)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Message hot path. The common case (returning user) is a single `SELECT`;
    /// `last_seen_at` is only written once per `LAST_SEEN_INTERVAL_SECS`.
    /// First contact inserts the row and returns its defaults.
    pub async fn ensure_user(&self, id: i64, fallback: Lang) -> AppResult<UserRecord> {
        if let Some(user) = self.user_by_id(id).await? {
            self.touch_last_seen(id).await;
            return Ok(user);
        }

        let now = unix_now();
        // `DO NOTHING` keeps a concurrent first message from failing with a
        // unique-constraint error; both workers end up reading the same row.
        sqlx::query(
            "INSERT INTO users (user_id, language, quality, banned, created_at, last_seen_at)
             VALUES (?, ?, 192, 0, ?, ?)
             ON CONFLICT(user_id) DO NOTHING",
        )
        .bind(id)
        .bind(fallback.code())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        self.mark_seen(id, now);

        self.user_by_id(id)
            .await?
            .ok_or_else(|| AppError::Message("user row disappeared after insert".to_owned()))
    }

    async fn user_by_id(&self, id: i64) -> AppResult<Option<UserRecord>> {
        let row = sqlx::query(
            "SELECT language, quality, banned, search_source, tags_enabled, lastfm_user,
                    hq_cover, lastfm_stats, genre_analysis
             FROM users WHERE user_id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|row| UserRecord {
            lang: Lang::from_code(row.get::<String, _>("language").as_str()).unwrap_or(Lang::En),
            quality: u32::try_from(row.get::<i64, _>("quality")).unwrap_or(192),
            banned: row.get::<i64, _>("banned") != 0,
            search_source: row
                .get::<String, _>("search_source")
                .parse::<Provider>()
                .unwrap_or(Provider::YouTube),
            tags_enabled: row.get::<i64, _>("tags_enabled") != 0,
            lastfm_user: row.get::<Option<String>, _>("lastfm_user"),
            hq_cover: row.get::<i64, _>("hq_cover") != 0,
            lastfm_stats: row.get::<i64, _>("lastfm_stats") != 0,
            genre_analysis: row.get::<i64, _>("genre_analysis") != 0,
        }))
    }

    /// Write `last_seen_at` at most once per interval per user. The map is a
    /// std mutex (never held across an await) and is swept in place whenever it
    /// grows past the watermark, so memory stays bounded.
    async fn touch_last_seen(&self, id: i64) {
        const WATERMARK: usize = 8_192;
        let now = unix_now();
        {
            let mut seen = self
                .seen
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if seen.len() > WATERMARK {
                seen.retain(|_, stamp| now.saturating_sub(*stamp) < LAST_SEEN_INTERVAL_SECS);
            }
            if let Some(stamp) = seen.get(&id)
                && now.saturating_sub(*stamp) < LAST_SEEN_INTERVAL_SECS
            {
                return;
            }
            seen.insert(id, now);
        }
        if let Err(error) = sqlx::query("UPDATE users SET last_seen_at = ? WHERE user_id = ?")
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
        {
            warn!(%error, user_id = id, "failed to update last_seen_at");
        }
    }

    fn mark_seen(&self, id: i64, now: i64) {
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        seen.insert(id, now);
    }

    pub async fn set_language(&self, id: i64, lang: Lang) -> AppResult<()> {
        sqlx::query("UPDATE users SET language = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(lang.code())
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_quality(&self, id: i64, quality: u32) -> AppResult<()> {
        // 0 is the lossless sentinel (FLAC); keep it, clamp the rest.
        let quality = if quality == 0 {
            0
        } else {
            quality.clamp(64, 320)
        } as i64;
        sqlx::query("UPDATE users SET quality = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(quality)
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_search_source(&self, id: i64, source: Provider) -> AppResult<()> {
        sqlx::query("UPDATE users SET search_source = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(source.as_str())
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_lastfm_user(&self, id: i64, lastfm_user: Option<&str>) -> AppResult<()> {
        sqlx::query("UPDATE users SET lastfm_user = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(lastfm_user)
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_hq_cover(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query("UPDATE users SET hq_cover = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(i64::from(enabled))
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_lastfm_stats(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query("UPDATE users SET lastfm_stats = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(i64::from(enabled))
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_genre_analysis(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query("UPDATE users SET genre_analysis = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(i64::from(enabled))
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    /// Record one downloaded track for the genre analyzer. The user's toggle is
    /// checked in the same statement that deduplicates the track, so there is
    /// no separate read; genres are upserted in one multi-row statement.
    /// Returns `true` when the track was new (its genres were counted).
    pub async fn record_genres(
        &self,
        id: i64,
        track_key: &str,
        genres: &[String],
    ) -> AppResult<bool> {
        if genres.is_empty() {
            return Ok(false);
        }
        let now = unix_now();
        let mut transaction = self.pool.begin().await.map_err(db_err)?;
        // `WHERE EXISTS` folds the toggle check into the insert; zero rows
        // affected means "off" or "already counted".
        let inserted = sqlx::query(
            "INSERT OR IGNORE INTO user_genre_tracks (user_id, track_key, created_at)
             SELECT ?, ?, ?
             WHERE EXISTS (
                 SELECT 1 FROM users WHERE user_id = ? AND genre_analysis = 1
             )",
        )
        .bind(id)
        .bind(track_key)
        .bind(now)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(db_err)?;
        if inserted.rows_affected() == 0 {
            transaction.commit().await.map_err(db_err)?;
            return Ok(false);
        }

        // Genres may repeat within one track's tag list ("Rock", "rock");
        // count each distinct label once, keeping the first spelling.
        let mut unique: Vec<&str> = Vec::with_capacity(genres.len());
        for genre in genres {
            let genre = genre.trim();
            if genre.is_empty() || unique.iter().any(|seen| seen.eq_ignore_ascii_case(genre)) {
                continue;
            }
            unique.push(genre);
        }
        if !unique.is_empty() {
            let mut builder =
                QueryBuilder::new("INSERT INTO user_genres (user_id, genre, listens, updated_at) ");
            builder.push_values(unique, |mut row, genre| {
                row.push_bind(id)
                    .push_bind(genre)
                    .push_bind(1_i64)
                    .push_bind(now);
            });
            builder.push(
                " ON CONFLICT(user_id, genre) DO UPDATE SET
                      listens = listens + 1,
                      updated_at = excluded.updated_at",
            );
            builder
                .build()
                .execute(&mut *transaction)
                .await
                .map_err(db_err)?;
        }
        transaction.commit().await.map_err(db_err)?;
        Ok(true)
    }

    /// Most-listened genres for one user, highest first.
    pub async fn top_genres(&self, id: i64, limit: usize) -> AppResult<Vec<(String, i64)>> {
        let rows = sqlx::query(
            "SELECT genre, listens FROM user_genres
             WHERE user_id = ?
             ORDER BY listens DESC, genre ASC
             LIMIT ?",
        )
        .bind(id)
        .bind(limit.clamp(1, 50) as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get::<String, _>("genre"), row.get::<i64, _>("listens")))
            .collect())
    }

    /// Number of distinct tracks counted by the analyzer.
    pub async fn genre_track_count(&self, id: i64) -> AppResult<i64> {
        sqlx::query_scalar("SELECT COUNT(*) FROM user_genre_tracks WHERE user_id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }

    pub async fn set_tags_enabled(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query("UPDATE users SET tags_enabled = ?, last_seen_at = ? WHERE user_id = ?")
            .bind(i64::from(enabled))
            .bind(unix_now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_banned(&self, id: i64, banned: bool) -> AppResult<bool> {
        let result = sqlx::query("UPDATE users SET banned = ? WHERE user_id = ?")
            .bind(i64::from(banned))
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    /// Recipients of the release notice: everyone who has used the bot and is
    /// not banned, most recently seen first.
    pub async fn active_user_ids(&self) -> AppResult<Vec<(i64, Lang)>> {
        let rows = sqlx::query(
            "SELECT user_id, language FROM users WHERE banned = 0 ORDER BY last_seen_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let lang =
                    Lang::from_code(row.get::<String, _>("language").as_str()).unwrap_or(Lang::En);
                (row.get::<i64, _>("user_id"), lang)
            })
            .collect())
    }

    pub async fn get_meta(&self, key: &str) -> AppResult<Option<String>> {
        let value = sqlx::query_scalar::<_, String>("SELECT value FROM meta WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(value)
    }

    pub async fn set_meta(&self, key: &str, value: &str) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO meta (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn db_err(error: sqlx::Error) -> AppError {
    AppError::Other(error.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Db {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.db");
        let db = Db::connect(&format!("sqlite://{}", path.display()))
            .await
            .expect("db");
        std::mem::forget(dir);
        db
    }

    #[tokio::test]
    async fn stores_tracks_and_users() {
        let db = test_db().await;
        let key = TrackKey::new("youtube:abc");
        let track = CachedTrack {
            key: key.clone(),
            file_id: "FILE".into(),
            file_unique_id: "UNIQUE".into(),
            source_url: Some("https://youtube.com/watch?v=abc".into()),
            provider: Provider::YouTube,
            meta: TrackMeta {
                title: "Song".into(),
                performer: Some("Artist".into()),
                duration_secs: Some(123),
                ..Default::default()
            },
        };
        assert!(db.get_track(&key, 192).await.expect("miss").is_none());
        db.put_track(&track, 192, Some(1_234_567))
            .await
            .expect("put");
        db.put_track(&track, 320, Some(3_456_789))
            .await
            .expect("put 320");
        let cached = db.get_track(&key, 192).await.expect("get").expect("some");
        assert_eq!(cached.file_id, "FILE");
        assert_eq!(cached.meta.performer.as_deref(), Some("Artist"));

        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert_eq!(user.lang, Lang::Uk);
        db.set_quality(42, 320).await.expect("quality");
        db.set_language(42, Lang::En).await.expect("lang");
        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert_eq!(user.quality, 320);
        assert_eq!(user.lang, Lang::En);
        assert_eq!(user.search_source, Provider::YouTube);
        assert!(!user.tags_enabled);
        db.set_search_source(42, Provider::Deezer)
            .await
            .expect("source");
        db.set_tags_enabled(42, true).await.expect("tags");
        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert_eq!(user.search_source, Provider::Deezer);
        assert!(user.tags_enabled);
        assert!(user.lastfm_user.is_none());
        db.set_lastfm_user(42, Some("TestUser"))
            .await
            .expect("lastfm set");
        db.set_hq_cover(42, true).await.expect("hq cover");
        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert_eq!(user.lastfm_user.as_deref(), Some("TestUser"));
        assert!(user.hq_cover);
        db.set_lastfm_user(42, None).await.expect("lastfm off");
        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert!(user.lastfm_user.is_none());

        assert!(db.purge_track(&key).await.expect("purge"));
        assert!(db.get_track(&key, 192).await.expect("get").is_none());
        assert!(db.get_track(&key, 320).await.expect("get").is_none());
    }

    #[tokio::test]
    async fn genre_analyzer_deduplicates_and_respects_toggle() {
        let db = test_db().await;
        let user = db.ensure_user(99, Lang::En).await.expect("user");
        assert!(!user.genre_analysis);

        // Toggle off: nothing is recorded even when genres are available.
        assert!(
            !db.record_genres(99, "youtube:one", &["Rock".to_owned()])
                .await
                .expect("record off")
        );
        assert_eq!(db.genre_track_count(99).await.expect("count"), 0);

        db.set_genre_analysis(99, true).await.expect("toggle");
        assert!(
            db.record_genres(
                99,
                "youtube:one",
                &["Rock".to_owned(), "Metal".to_owned(), "rock".to_owned()]
            )
            .await
            .expect("record one")
        );
        // The same track again must not inflate the chart.
        assert!(
            !db.record_genres(99, "youtube:one", &["Rock".to_owned()])
                .await
                .expect("record replay")
        );
        assert!(
            db.record_genres(99, "youtube:two", &["Rock".to_owned()])
                .await
                .expect("record two")
        );
        let top = db.top_genres(99, 10).await.expect("top");
        assert_eq!(top, vec![("Rock".to_owned(), 2), ("Metal".to_owned(), 1)]);
        assert_eq!(db.genre_track_count(99).await.expect("count"), 2);

        // Turning the analyzer off stops new recordings but keeps history.
        db.set_genre_analysis(99, false).await.expect("toggle off");
        assert!(
            !db.record_genres(99, "youtube:three", &["Jazz".to_owned()])
                .await
                .expect("record off again")
        );
        assert_eq!(db.genre_track_count(99).await.expect("count"), 2);
    }

    #[tokio::test]
    async fn ensure_user_throttles_last_seen_writes() {
        let db = test_db().await;
        db.ensure_user(5, Lang::En).await.expect("create");

        // Simulate an old stamp and force the next call to write.
        sqlx::query("UPDATE users SET last_seen_at = 0 WHERE user_id = 5")
            .execute(&db.pool)
            .await
            .expect("reset");
        db.seen.lock().expect("seen").remove(&5);
        db.ensure_user(5, Lang::En).await.expect("touch");
        assert!(last_seen(&db, 5).await > 0);

        // Within the interval the row is read but not written again.
        sqlx::query("UPDATE users SET last_seen_at = 0 WHERE user_id = 5")
            .execute(&db.pool)
            .await
            .expect("reset");
        db.ensure_user(5, Lang::En).await.expect("throttled touch");
        assert_eq!(last_seen(&db, 5).await, 0);
    }

    #[tokio::test]
    async fn stats_counts_tracks_and_users() {
        let db = test_db().await;
        let key = TrackKey::new("youtube:stats");
        let track = CachedTrack {
            key: key.clone(),
            file_id: "FILE".into(),
            file_unique_id: "UNIQUE".into(),
            source_url: None,
            provider: Provider::YouTube,
            meta: TrackMeta::default(),
        };
        db.put_track(&track, 192, Some(1000)).await.expect("put");
        db.put_track(&track, 320, Some(2000)).await.expect("put");
        db.ensure_user(1, Lang::En).await.expect("user");
        let stats = db.stats().await.expect("stats");
        assert_eq!(stats.tracks, 2);
        assert_eq!(stats.users, 1);
        assert!(stats.sends >= 0);
    }

    async fn last_seen(db: &Db, id: i64) -> i64 {
        sqlx::query_scalar("SELECT last_seen_at FROM users WHERE user_id = ?")
            .bind(id)
            .fetch_one(&db.pool)
            .await
            .expect("last_seen")
    }

    #[tokio::test]
    async fn search_cache_roundtrip() {
        let db = test_db().await;
        db.put_search_cache("youtube", "soft", "[\"x\"]", Duration::from_secs(60))
            .await
            .expect("put");
        assert_eq!(
            db.get_search_cache("youtube", "soft")
                .await
                .expect("get")
                .as_deref(),
            Some("[\"x\"]")
        );
        assert!(
            db.get_search_cache("youtube", "absent")
                .await
                .expect("miss")
                .is_none()
        );
    }

    #[tokio::test]
    async fn ping_works() {
        let db = test_db().await;
        assert!(db.ping().await);
    }

    #[tokio::test]
    async fn meta_and_announcement_recipients() {
        let db = test_db().await;
        assert!(
            db.get_meta("announced_version")
                .await
                .expect("get")
                .is_none()
        );
        db.ensure_user(7, Lang::Uk).await.expect("user 7");
        db.ensure_user(8, Lang::En).await.expect("user 8");
        db.set_banned(8, true).await.expect("ban");
        let recipients = db.active_user_ids().await.expect("recipients");
        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0].0, 7);
        assert!(matches!(recipients[0].1, Lang::Uk));

        db.set_meta("announced_version", "1.1.0")
            .await
            .expect("set");
        assert_eq!(
            db.get_meta("announced_version")
                .await
                .expect("get")
                .as_deref(),
            Some("1.1.0")
        );
    }
}
