//! SQLite persistence. Only metadata and Telegram file identifiers are stored;
//! audio bytes are never kept by the bot.
//!
//! Cache key: (`track_key`, `quality`) per ТЗ §8 — the same track is cached per
//! bitrate. Search cache is two-tiered: moka (in-memory, L1) in the manager and
//! this `search_cache` table (L2) which survives restarts.

use common::domain::{CachedTrack, Lang, Provider, Stats, TrackKey, TrackMeta, UserRecord};
use common::error::{AppError, AppResult};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA: &str = include_str!("../../../migrations/0001_init.sql");
const MIGRATION_0002: &str = include_str!("../../../migrations/0002_user_search_source.sql");
const MIGRATION_0004: &str = include_str!("../../../migrations/0004_lastfm.sql");
const MIGRATION_0005: &str = include_str!("../../../migrations/0005_hq_cover.sql");
const MIGRATION_0006: &str = include_str!("../../../migrations/0006_meta.sql");

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

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

        // Drop expired L2 search cache rows; they are cheap to recompute.
        let now = unix_now();
        let _ = sqlx::query("DELETE FROM search_cache WHERE expires_at < ?")
            .bind(now)
            .execute(&pool)
            .await;

        Ok(Self { pool })
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

    pub async fn stats(&self) -> AppResult<Stats> {
        let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;
        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;
        let sends: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(uses), 0) FROM tracks")
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(Stats {
            tracks,
            users,
            sends,
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

    /// One round trip: `RETURNING` hands back the post-upsert row. Called on
    /// every incoming message, so the extra round trip used to matter.
    pub async fn ensure_user(&self, id: i64, fallback: Lang) -> AppResult<UserRecord> {
        let now = unix_now();
        let row = sqlx::query(
            "INSERT INTO users (user_id, language, quality, banned, created_at, last_seen_at)
             VALUES (?, ?, 192, 0, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET last_seen_at = excluded.last_seen_at
             RETURNING language, quality, banned, search_source, tags_enabled, lastfm_user, hq_cover",
        )
        .bind(id)
        .bind(fallback.code())
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(UserRecord {
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
        })
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
