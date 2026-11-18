//! Postgres persistence. Only metadata, Telegram file identifiers and
//! short-lived coordination state are stored; audio bytes are never kept by
//! the bot.
//!
//! Static SQL goes through `sqlx::query!`/`query_scalar!` so column names and
//! bind types are checked at compile time against the schema in
//! `migrations/`. The only runtime-built statement is the batched genre upsert,
//! whose number of rows is data-dependent.
//!
//! All replicas share this database: selections, search cache, rate limits and
//! the once-per-version announcement marker work with webhooks and polling.

use common::domain::{
    CachedTrack, DigestDue, DigestSchedule, DigestSummary, DynamicsCell, Lang, ListenEntry,
    Provider, Stats, TrackKey, TrackMeta, UserRecord, UserStats,
};
use common::error::{AppError, AppResult};
use serde::de::DeserializeOwned;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, QueryBuilder};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::warn;

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
    /// Last time each user's `last_seen_at` was written. The message hot path
    /// only reads the user row; this throttle keeps it from turning every
    /// message into a write transaction. With several replicas this means at
    /// most one write per user per interval per replica.
    seen: Arc<Mutex<HashMap<i64, i64>>>,
}

/// `last_seen_at` is bookkeeping for the release broadcast and admin stats;
/// one write per active user per five minutes is plenty.
const LAST_SEEN_INTERVAL_SECS: i64 = 5 * 60;

impl Db {
    pub async fn connect(database_url: &str) -> AppResult<Self> {
        let options = PgConnectOptions::from_str(database_url)
            .map_err(|error| AppError::Config(format!("DATABASE_URL: {error}")))?
            // This database holds a cache of Telegram file ids and counters,
            // not billing data. Skipping the WAL flush per write removes a
            // multi-millisecond fsync from the cache-hit bookkeeping path;
            // losing the last few writes after a crash only costs a re-download.
            .options([("synchronous_commit", "off")]);
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await
            .map_err(db_err)?;
        Self::migrate(&pool).await?;
        Ok(Self::from_pool(pool))
    }

    pub async fn migrate(pool: &PgPool) -> AppResult<()> {
        // `migrate!` embeds the SQL at compile time and records checksums, so
        // an accidental edit to an applied migration is detected on startup.
        sqlx::migrate!("../../migrations")
            .run(pool)
            .await
            .map_err(|error| AppError::Other(error.into()))?;
        // A replica that crashed mid-download leaves a claimed job behind.
        let _ = sqlx::query!(
            "UPDATE download_jobs
             SET status = 'queued', claimed_by = NULL, updated_at = $1
             WHERE status IN ('downloading', 'uploading') AND cancel_requested = false",
            unix_now()
        )
        .execute(pool)
        .await;
        let now = unix_now();
        let _ = sqlx::query!("DELETE FROM search_cache WHERE expires_at < $1", now)
            .execute(pool)
            .await;
        let _ = sqlx::query!("DELETE FROM selections WHERE expires_at < $1", now)
            .execute(pool)
            .await;
        Ok(())
    }

    /// Wrap an existing pool (used by `#[sqlx::test]`, which creates one
    /// isolated database per test and applies migrations itself).
    pub fn from_pool(pool: PgPool) -> Self {
        Self {
            pool,
            seen: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Lightweight liveness check used by /healthz.
    pub async fn ping(&self) -> bool {
        sqlx::query_scalar!("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|value| value == Some(1))
            .unwrap_or(false)
    }

    pub async fn get_track(&self, key: &TrackKey, quality: u32) -> AppResult<Option<CachedTrack>> {
        let row = sqlx::query!(
            "SELECT track_key, quality, file_id, file_unique_id, source_url, provider, title,
                    performer, album, duration_secs, size_bytes, thumbnail_url, genres
             FROM tracks WHERE track_key = $1 AND quality = $2",
            key.as_str(),
            i64::from(quality)
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(|row| {
            let provider = row
                .provider
                .parse::<Provider>()
                .unwrap_or(Provider::YouTube);
            CachedTrack {
                key: TrackKey(row.track_key),
                file_id: row.file_id,
                file_unique_id: row.file_unique_id,
                source_url: row.source_url,
                provider,
                meta: TrackMeta {
                    title: row.title,
                    performer: row.performer,
                    album: row.album,
                    duration_secs: row
                        .duration_secs
                        .and_then(|value| u32::try_from(value).ok()),
                    thumbnail_url: row.thumbnail_url,
                    genres: serde_json::from_str(&row.genres).unwrap_or_default(),
                },
            }
        }))
    }

    /// Find an already-downloaded track by the Telegram file id of the audio
    /// the bot sent. Used by `/convert` when replying to a track message.
    pub async fn track_by_file_id(&self, file_id: &str) -> AppResult<Option<CachedTrack>> {
        let row = sqlx::query!(
            "SELECT track_key, quality, file_id, file_unique_id, source_url, provider, title,
                    performer, album, duration_secs, thumbnail_url, genres
             FROM tracks WHERE file_id = $1
             ORDER BY last_used_at DESC
             LIMIT 1",
            file_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(|row| CachedTrack {
            key: TrackKey(row.track_key),
            file_id: row.file_id,
            file_unique_id: row.file_unique_id,
            source_url: row.source_url,
            provider: row
                .provider
                .parse::<Provider>()
                .unwrap_or(Provider::YouTube),
            meta: TrackMeta {
                title: row.title,
                performer: row.performer,
                album: row.album,
                duration_secs: row
                    .duration_secs
                    .and_then(|value| u32::try_from(value).ok()),
                thumbnail_url: row.thumbnail_url,
                genres: serde_json::from_str(&row.genres).unwrap_or_default(),
            },
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
        sqlx::query!(
            "INSERT INTO tracks
                (track_key, quality, file_id, file_unique_id, source_url, provider, title, performer,
                 album, duration_secs, size_bytes, thumbnail_url, genres, created_at, last_used_at, uses)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 1)
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
            track.key.as_str(),
            i64::from(quality),
            track.file_id,
            track.file_unique_id,
            track.source_url,
            track.provider.as_str(),
            track.meta.title,
            track.meta.performer,
            track.meta.album,
            track.meta.duration_secs.map(i64::from),
            size_bytes,
            track.meta.thumbnail_url,
            genres,
            now,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn touch_track(&self, key: &TrackKey, quality: u32) -> AppResult<()> {
        sqlx::query!(
            "UPDATE tracks SET last_used_at = $1, uses = uses + 1
             WHERE track_key = $2 AND quality = $3",
            unix_now(),
            key.as_str(),
            i64::from(quality)
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn purge_track(&self, key: &TrackKey) -> AppResult<bool> {
        let result = sqlx::query!("DELETE FROM tracks WHERE track_key = $1", key.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    /// One statement instead of three round trips; `/stats` runs on demand but
    /// the three scans used to hit the same rows separately.
    pub async fn stats(&self) -> AppResult<Stats> {
        let row = sqlx::query!(
            "SELECT
                 (SELECT COUNT(*) FROM tracks) AS tracks,
                 (SELECT COUNT(*) FROM users) AS users,
                 (SELECT COALESCE(SUM(uses), 0)::BIGINT FROM tracks) AS sends"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(Stats {
            tracks: row.tracks.unwrap_or_default(),
            users: row.users.unwrap_or_default(),
            sends: row.sends.unwrap_or_default(),
        })
    }

    /// L2 search cache: fetch a cached payload if it has not expired.
    pub async fn get_search_cache(&self, provider: &str, query: &str) -> AppResult<Option<String>> {
        let row = sqlx::query!(
            "SELECT result FROM search_cache
             WHERE provider = $1 AND query = $2 AND expires_at >= $3",
            provider,
            query,
            unix_now()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|row| row.result))
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
        sqlx::query!(
            "INSERT INTO search_cache (provider, query, result, expires_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT(provider, query) DO UPDATE SET
                result = excluded.result,
                expires_at = excluded.expires_at",
            provider,
            query,
            result_json,
            expires_at
        )
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
        // unique-constraint error; both replicas end up reading the same row.
        sqlx::query!(
            "INSERT INTO users (user_id, language, quality, banned, created_at, last_seen_at)
             VALUES ($1, $2, 192, 0, $3, $4)
             ON CONFLICT(user_id) DO NOTHING",
            id,
            fallback.code(),
            now,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        self.mark_seen(id, now);

        self.user_by_id(id)
            .await?
            .ok_or_else(|| AppError::Message("user row disappeared after insert".to_owned()))
    }

    async fn user_by_id(&self, id: i64) -> AppResult<Option<UserRecord>> {
        let row = sqlx::query!(
            "SELECT language, quality, banned, search_source, tags_enabled, lastfm_user,
                    hq_cover, lastfm_stats, genre_analysis, album_info, audio_format,
                    normalize_audio
             FROM users WHERE user_id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|row| UserRecord {
            lang: Lang::from_code(&row.language).unwrap_or(Lang::En),
            quality: u32::try_from(row.quality).unwrap_or(192),
            banned: row.banned != 0,
            search_source: row
                .search_source
                .parse::<Provider>()
                .unwrap_or(Provider::YouTube),
            tags_enabled: row.tags_enabled != 0,
            lastfm_user: row.lastfm_user,
            hq_cover: row.hq_cover != 0,
            lastfm_stats: row.lastfm_stats != 0,
            genre_analysis: row.genre_analysis != 0,
            album_info: row.album_info != 0,
            audio_format: row.audio_format,
            normalize_audio: row.normalize_audio != 0,
        }))
    }

    /// Write `last_seen_at` at most once per interval per user per replica.
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
        if let Err(error) = sqlx::query!(
            "UPDATE users SET last_seen_at = $1 WHERE user_id = $2",
            now,
            id
        )
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
        sqlx::query!(
            "UPDATE users SET language = $1, last_seen_at = $2 WHERE user_id = $3",
            lang.code(),
            unix_now(),
            id
        )
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
        sqlx::query!(
            "UPDATE users SET quality = $1, last_seen_at = $2 WHERE user_id = $3",
            quality,
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_search_source(&self, id: i64, source: Provider) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET search_source = $1, last_seen_at = $2 WHERE user_id = $3",
            source.as_str(),
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_lastfm_user(&self, id: i64, lastfm_user: Option<&str>) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET lastfm_user = $1, last_seen_at = $2 WHERE user_id = $3",
            lastfm_user,
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_hq_cover(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET hq_cover = $1, last_seen_at = $2 WHERE user_id = $3",
            i64::from(enabled),
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_lastfm_stats(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET lastfm_stats = $1, last_seen_at = $2 WHERE user_id = $3",
            i64::from(enabled),
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_genre_analysis(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET genre_analysis = $1, last_seen_at = $2 WHERE user_id = $3",
            i64::from(enabled),
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_album_info(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET album_info = $1, last_seen_at = $2 WHERE user_id = $3",
            i64::from(enabled),
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_audio_format(&self, id: i64, format: &str) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET audio_format = $1, last_seen_at = $2 WHERE user_id = $3",
            format,
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_normalize_audio(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET normalize_audio = $1, last_seen_at = $2 WHERE user_id = $3",
            i64::from(enabled),
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn audio_prefs(&self, id: i64) -> AppResult<(String, bool)> {
        let row = sqlx::query!(
            "SELECT audio_format, normalize_audio FROM users WHERE user_id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row
            .map(|row| (row.audio_format, row.normalize_audio != 0))
            .unwrap_or_else(|| ("auto".to_owned(), false)))
    }

    pub async fn set_chat_enabled(
        &self,
        chat_id: i64,
        enabled: bool,
        added_by: i64,
    ) -> AppResult<()> {
        sqlx::query!(
            "INSERT INTO chats (chat_id, enabled, added_by, created_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT(chat_id) DO UPDATE SET enabled = excluded.enabled,
                added_by = excluded.added_by",
            chat_id,
            enabled,
            added_by,
            unix_now()
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn chat_enabled(&self, chat_id: i64) -> AppResult<bool> {
        Ok(
            sqlx::query_scalar!("SELECT enabled FROM chats WHERE chat_id = $1", chat_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?
                .unwrap_or(false),
        )
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
        let inserted = sqlx::query!(
            "INSERT INTO user_genre_tracks (user_id, track_key, created_at)
             SELECT $1, $2, $3
             WHERE EXISTS (SELECT 1 FROM users WHERE user_id = $4 AND genre_analysis = 1)
             ON CONFLICT DO NOTHING",
            id,
            track_key,
            now,
            id
        )
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
            // Timestamped events for period-filtered charts. Same transaction
            // as the dedupe insert, so a replay never inflates either table.
            let mut events =
                QueryBuilder::new("INSERT INTO user_genre_events (user_id, genre, created_at) ");
            events.push_values(&unique, |mut row, genre| {
                row.push_bind(id).push_bind(*genre).push_bind(now);
            });
            events
                .build()
                .execute(&mut *transaction)
                .await
                .map_err(db_err)?;

            // The only runtime-built statement: the row count depends on the
            // number of tags the source returned.
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
                      listens = user_genres.listens + 1,
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
        let rows = sqlx::query!(
            "SELECT genre, listens FROM user_genres
             WHERE user_id = $1
             ORDER BY listens DESC, genre ASC
             LIMIT $2",
            id,
            limit.clamp(1, 50) as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.genre, row.listens))
            .collect())
    }

    /// Genre counts within a time window, for `/genres [bars] [period]`.
    pub async fn top_genres_since(
        &self,
        id: i64,
        limit: usize,
        since: i64,
    ) -> AppResult<Vec<(String, i64)>> {
        let rows = sqlx::query!(
            "SELECT genre, COUNT(*) AS listens FROM user_genre_events
             WHERE user_id = $1 AND created_at >= $2
             GROUP BY genre
             ORDER BY listens DESC, genre ASC
             LIMIT $3",
            id,
            since,
            limit.clamp(1, 50) as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.genre, row.listens.unwrap_or_default()))
            .collect())
    }

    /// Counters for `/me`. Durations are taken from the cached metadata of
    /// distinct counted tracks (best known quality), never from disk.
    pub async fn user_stats(&self, id: i64) -> AppResult<UserStats> {
        let row = sqlx::query!(
            "SELECT
                 (SELECT COUNT(*) FROM user_genre_tracks WHERE user_id = $1) AS tracks,
                 (SELECT COALESCE(SUM(listens), 0)::BIGINT FROM user_genres WHERE user_id = $1) AS listens,
                 (SELECT COUNT(*) FROM user_genres WHERE user_id = $1) AS genres,
                 (SELECT COALESCE(SUM(duration), 0)::BIGINT FROM (
                      SELECT MAX(t.duration_secs) AS duration
                      FROM user_genre_tracks g
                      JOIN tracks t ON t.track_key = g.track_key
                      WHERE g.user_id = $1
                      GROUP BY g.track_key
                  ) d) AS seconds",
            id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(UserStats {
            tracks: row.tracks.unwrap_or_default(),
            listens: row.listens.unwrap_or_default(),
            genres: row.genres.unwrap_or_default(),
            seconds: row.seconds.unwrap_or_default(),
        })
    }

    /// Number of distinct tracks counted by the analyzer.
    pub async fn genre_track_count(&self, id: i64) -> AppResult<i64> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM user_genre_tracks WHERE user_id = $1",
            id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(count.unwrap_or_default())
    }

    pub async fn set_tags_enabled(&self, id: i64, enabled: bool) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET tags_enabled = $1, last_seen_at = $2 WHERE user_id = $3",
            i64::from(enabled),
            unix_now(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn set_banned(&self, id: i64, banned: bool) -> AppResult<bool> {
        let result = sqlx::query!(
            "UPDATE users SET banned = $1 WHERE user_id = $2",
            i64::from(banned),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    /// Recipients of the release notice: everyone who has used the bot and is
    /// not banned, most recently seen first.
    pub async fn active_user_ids(&self) -> AppResult<Vec<(i64, Lang)>> {
        let rows = sqlx::query!(
            "SELECT user_id, language FROM users WHERE banned = 0 ORDER BY last_seen_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let lang = Lang::from_code(&row.language).unwrap_or(Lang::En);
                (row.user_id, lang)
            })
            .collect())
    }

    pub async fn get_meta(&self, key: &str) -> AppResult<Option<String>> {
        sqlx::query_scalar!("SELECT meta_value FROM meta WHERE meta_key = $1", key)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)
    }

    pub async fn set_meta(&self, key: &str, value: &str) -> AppResult<()> {
        sqlx::query!(
            "INSERT INTO meta (meta_key, meta_value) VALUES ($1, $2)
             ON CONFLICT(meta_key) DO UPDATE SET meta_value = excluded.meta_value",
            key,
            value
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    // ---- Listening history, genre dynamics and digests (v2.5) ------------

    /// Append one delivered track to the history. Only metadata is persisted;
    /// audio, covers and lyrics are never written to disk by the bot.
    pub async fn record_listen(
        &self,
        user_id: i64,
        track_key: &str,
        title: &str,
        performer: Option<&str>,
        genres: &[String],
    ) -> AppResult<()> {
        if title.trim().is_empty() {
            return Ok(());
        }
        let genres = serde_json::to_value(genres).unwrap_or_else(|_| serde_json::json!([]));
        sqlx::query!(
            "INSERT INTO listens (user_id, track_key, title, performer, genres, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
            user_id,
            track_key,
            title,
            performer,
            genres,
            unix_now()
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Most recent listens in a time window, newest first.
    pub async fn list_listens(
        &self,
        user_id: i64,
        since: i64,
        limit: i64,
    ) -> AppResult<Vec<ListenEntry>> {
        let rows = sqlx::query!(
            "SELECT title, performer, created_at FROM listens
             WHERE user_id = $1 AND created_at >= $2
             ORDER BY created_at DESC, id DESC
             LIMIT $3",
            user_id,
            since,
            limit.clamp(1, 50)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| ListenEntry {
                title: row.title,
                performer: row.performer,
                created_at: row.created_at,
            })
            .collect())
    }

    pub async fn count_listens(&self, user_id: i64, since: i64) -> AppResult<i64> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM listens WHERE user_id = $1 AND created_at >= $2",
            user_id,
            since
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(count.unwrap_or_default())
    }

    /// Genre listens grouped into fixed unix-second buckets. The chart layer
    /// turns this sparse list into a dense matrix for the selected window.
    pub async fn genre_dynamics(
        &self,
        user_id: i64,
        since: i64,
        bucket_secs: i64,
    ) -> AppResult<Vec<DynamicsCell>> {
        let rows = sqlx::query!(
            "SELECT (created_at / $2) * $2 AS bucket, genre, COUNT(*) AS listens
             FROM user_genre_events
             WHERE user_id = $1 AND created_at >= $3
             GROUP BY bucket, genre
             ORDER BY bucket ASC, listens DESC, genre ASC",
            user_id,
            bucket_secs.max(1),
            since
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| DynamicsCell {
                bucket: row.bucket.unwrap_or_default(),
                genre: row.genre,
                listens: row.listens.unwrap_or_default(),
            })
            .collect())
    }

    /// Create or update a user's digest plan. `last_sent_at` is deliberately
    /// not cleared, so changing the hour does not re-send an old digest.
    #[allow(clippy::too_many_arguments)]
    pub async fn set_digest_schedule(
        &self,
        user_id: i64,
        period: &str,
        enabled: bool,
        next_send_at: i64,
        quiet_start: i16,
        quiet_end: i16,
    ) -> AppResult<()> {
        let now = unix_now();
        sqlx::query!(
            "INSERT INTO digest_schedule
                (user_id, period, enabled, next_send_at, quiet_start_utc, quiet_end_utc,
                 created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
             ON CONFLICT(user_id) DO UPDATE SET
                period = excluded.period,
                enabled = excluded.enabled,
                next_send_at = excluded.next_send_at,
                quiet_start_utc = excluded.quiet_start_utc,
                quiet_end_utc = excluded.quiet_end_utc,
                updated_at = excluded.updated_at",
            user_id,
            period,
            enabled,
            next_send_at,
            quiet_start,
            quiet_end,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn get_digest_schedule(&self, user_id: i64) -> AppResult<Option<DigestSchedule>> {
        let row = sqlx::query!(
            "SELECT period, enabled, next_send_at, last_sent_at, quiet_start_utc, quiet_end_utc
             FROM digest_schedule WHERE user_id = $1",
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|row| DigestSchedule {
            period: row.period,
            enabled: row.enabled,
            next_send_at: row.next_send_at,
            last_sent_at: row.last_sent_at,
            quiet_start_utc: row.quiet_start_utc,
            quiet_end_utc: row.quiet_end_utc,
        }))
    }

    /// Due plans joined with the user's language. The CAS in
    /// `claim_digest`/`defer_digest` makes the claim safe with N replicas.
    pub async fn due_digests(&self, now: i64, limit: i64) -> AppResult<Vec<DigestDue>> {
        let rows = sqlx::query!(
            "SELECT s.user_id, s.period, s.next_send_at, s.quiet_start_utc, s.quiet_end_utc,
                    u.language
             FROM digest_schedule s
             JOIN users u ON u.user_id = s.user_id
             WHERE s.enabled AND s.next_send_at <= $1
             ORDER BY s.next_send_at ASC
             LIMIT $2",
            now,
            limit.clamp(1, 200)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| DigestDue {
                user_id: row.user_id,
                lang: Lang::from_code(&row.language).unwrap_or(Lang::En),
                period: row.period,
                next_send_at: row.next_send_at,
                quiet_start_utc: row.quiet_start_utc,
                quiet_end_utc: row.quiet_end_utc,
            })
            .collect())
    }

    /// Atomically move a claimed plan to its next slot. Returns false when the
    /// row was already claimed/deleted by another replica.
    pub async fn claim_digest(
        &self,
        user_id: i64,
        expected_next: i64,
        next_send_at: i64,
    ) -> AppResult<bool> {
        let now = unix_now();
        let result = sqlx::query!(
            "UPDATE digest_schedule
             SET next_send_at = $3, last_sent_at = $4, updated_at = $4
             WHERE user_id = $1 AND enabled = true AND next_send_at = $2",
            user_id,
            expected_next,
            next_send_at,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    /// Move a due plan forward without marking it sent (quiet hours, retry).
    pub async fn defer_digest(
        &self,
        user_id: i64,
        expected_next: i64,
        next_send_at: i64,
    ) -> AppResult<bool> {
        let result = sqlx::query!(
            "UPDATE digest_schedule SET next_send_at = $3, updated_at = $3
             WHERE user_id = $1 AND enabled = true AND next_send_at = $2",
            user_id,
            expected_next,
            next_send_at
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn disable_digest(&self, user_id: i64) -> AppResult<()> {
        sqlx::query!(
            "UPDATE digest_schedule SET enabled = false, updated_at = $2 WHERE user_id = $1",
            user_id,
            unix_now()
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn digest_summary(&self, user_id: i64, since: i64) -> AppResult<DigestSummary> {
        let counts = sqlx::query!(
            "SELECT COUNT(*) AS listens, COUNT(DISTINCT track_key) AS tracks
             FROM listens WHERE user_id = $1 AND created_at >= $2",
            user_id,
            since
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        let genre_rows = sqlx::query!(
            "SELECT g.genre, COUNT(*) AS listens
             FROM listens l CROSS JOIN LATERAL jsonb_array_elements_text(l.genres) AS g(genre)
             WHERE l.user_id = $1 AND l.created_at >= $2
             GROUP BY g.genre
             ORDER BY listens DESC, g.genre ASC
             LIMIT 5",
            user_id,
            since
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let track_rows = sqlx::query!(
            "SELECT title, performer, COUNT(*) AS listens
             FROM listens WHERE user_id = $1 AND created_at >= $2
             GROUP BY title, performer
             ORDER BY listens DESC, title ASC
             LIMIT 5",
            user_id,
            since
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(DigestSummary {
            listens: counts.listens.unwrap_or_default(),
            tracks: counts.tracks.unwrap_or_default(),
            genres: genre_rows
                .into_iter()
                .map(|row| {
                    (
                        row.genre.unwrap_or_default(),
                        row.listens.unwrap_or_default(),
                    )
                })
                .collect(),
            top_tracks: track_rows
                .into_iter()
                .map(|row| (row.title, row.performer, row.listens.unwrap_or_default()))
                .collect(),
        })
    }

    // ---- Shared state for webhooks / multiple replicas -------------------

    /// Store a callback selection with a TTL. Every replica reads the same row,
    /// so a callback delivered to a different instance still resolves.
    pub async fn put_selection(
        &self,
        id: &str,
        kind: &str,
        user_id: i64,
        payload: serde_json::Value,
        ttl: Duration,
    ) -> AppResult<()> {
        sqlx::query!(
            "INSERT INTO selections (id, kind, user_id, payload, expires_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind,
                user_id = excluded.user_id,
                payload = excluded.payload,
                expires_at = excluded.expires_at",
            id,
            kind,
            user_id,
            payload,
            unix_now() + ttl.as_secs() as i64
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Fetch and deserialize a selection, checking its kind and expiry.
    pub async fn get_selection<T: DeserializeOwned>(
        &self,
        id: &str,
        kind: &str,
    ) -> AppResult<Option<T>> {
        let row = sqlx::query!(
            "SELECT payload, expires_at FROM selections WHERE id = $1 AND kind = $2",
            id,
            kind
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        let Some(row) = row else {
            return Ok(None);
        };
        if row.expires_at < unix_now() {
            let _ = sqlx::query!("DELETE FROM selections WHERE id = $1", id)
                .execute(&self.pool)
                .await;
            return Ok(None);
        }
        serde_json::from_value(row.payload)
            .map(Some)
            .map_err(|error| AppError::Other(error.into()))
    }

    pub async fn delete_selection(&self, id: &str) -> AppResult<()> {
        sqlx::query!("DELETE FROM selections WHERE id = $1", id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    /// Atomically consume a non-expired selection. Two webhook replicas cannot
    /// both act on the same one-shot prompt (e.g. a pending `/import`).
    pub async fn take_selection(&self, id: &str, kind: &str) -> AppResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM selections WHERE id = $1 AND kind = $2 AND expires_at >= $3",
            id,
            kind,
            unix_now()
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    // ---- Persistent download jobs ----------------------------------------

    /// Insert a job and atomically claim it for this replica. Returns `false`
    /// when another worker already owns the id (or enqueued the same request).
    pub async fn enqueue_and_claim_job(
        &self,
        id: &str,
        user_id: i64,
        kind: &str,
        payload: serde_json::Value,
        worker: &str,
    ) -> AppResult<bool> {
        let now = unix_now();
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let inserted = sqlx::query!(
            "INSERT INTO download_jobs
                (id, user_id, kind, status, payload, created_at, updated_at)
             VALUES ($1, $2, $3, 'queued', $4, $5, $5)
             ON CONFLICT(id) DO NOTHING",
            id,
            user_id,
            kind,
            payload,
            now
        )
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        if inserted.rows_affected() == 0 {
            tx.commit().await.map_err(db_err)?;
            return Ok(false);
        }
        let claimed = sqlx::query!(
            "UPDATE download_jobs
             SET status = 'downloading', claimed_by = $2, updated_at = $3
             WHERE id = $1 AND status = 'queued'
             RETURNING id",
            id,
            worker,
            now
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(claimed.is_some())
    }

    pub async fn finish_job(&self, id: &str, status: &str, error: Option<&str>) -> AppResult<()> {
        sqlx::query!(
            "UPDATE download_jobs
             SET status = $2, error = $3, updated_at = $4
             WHERE id = $1",
            id,
            status,
            error,
            unix_now()
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn request_cancel(&self, id: &str, user_id: i64) -> AppResult<bool> {
        let result = sqlx::query!(
            "UPDATE download_jobs SET cancel_requested = true, updated_at = $3
             WHERE id = $1 AND user_id = $2
               AND status IN ('queued', 'downloading', 'uploading')",
            id,
            user_id,
            unix_now()
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    /// `/stop`: flag every active job of a user as cancelled in one statement.
    pub async fn cancel_all_jobs_for_user(&self, user_id: i64) -> AppResult<u64> {
        let result = sqlx::query!(
            "UPDATE download_jobs SET cancel_requested = true, updated_at = $2
             WHERE user_id = $1
               AND status IN ('queued', 'downloading', 'uploading')
               AND NOT cancel_requested",
            user_id,
            unix_now()
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected())
    }

    /// `/stop`: drop every pending one-shot prompt (playlist import, album
    /// picks, lyrics choices) so no stray follow-up is acted on afterwards.
    pub async fn clear_user_selections(&self, user_id: i64) -> AppResult<u64> {
        let result = sqlx::query!("DELETE FROM selections WHERE user_id = $1", user_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected())
    }

    pub async fn job_cancel_requested(&self, id: &str) -> AppResult<bool> {
        Ok(sqlx::query_scalar!(
            "SELECT cancel_requested FROM download_jobs WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .unwrap_or(false))
    }

    /// Requeue jobs abandoned by a crashed replica on startup.
    pub async fn requeue_stale_jobs(&self) -> AppResult<()> {
        sqlx::query!(
            "UPDATE download_jobs
             SET status = 'queued', claimed_by = NULL, updated_at = $1
             WHERE status IN ('downloading', 'uploading') AND cancel_requested = false",
            unix_now()
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn list_jobs(
        &self,
        user_id: i64,
        limit: i64,
    ) -> AppResult<Vec<(String, String, String, bool)>> {
        let rows = sqlx::query!(
            "SELECT id, kind, status, cancel_requested FROM download_jobs
             WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2",
            user_id,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.id, row.kind, row.status, row.cancel_requested))
            .collect())
    }

    /// Atomic fixed-window rate limit shared by all replicas. Returns `false`
    /// when the user has spent the budget for the current minute.
    /// Current fixed-window hit count (used by the two-replica integration test).
    pub async fn rate_limit_hits(&self, user_id: i64) -> AppResult<i64> {
        Ok(
            sqlx::query_scalar!("SELECT hits FROM rate_limits WHERE user_id = $1", user_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?
                .unwrap_or_default(),
        )
    }

    pub async fn allow(&self, user_id: i64, limit: i64) -> AppResult<bool> {
        let now = unix_now();
        let window = now - now.rem_euclid(60);
        let row = sqlx::query!(
            "INSERT INTO rate_limits (user_id, window_start, hits) VALUES ($1, $2, 1)
             ON CONFLICT (user_id) DO UPDATE SET
                window_start = CASE
                    WHEN rate_limits.window_start < $2 THEN $2
                    ELSE rate_limits.window_start END,
                hits = CASE
                    WHEN rate_limits.window_start < $2 THEN 1
                    ELSE rate_limits.hits + 1 END
             RETURNING hits",
            user_id,
            window
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.hits <= limit)
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

    async fn db(pool: PgPool) -> Db {
        Db::migrate(&pool).await.expect("migrate");
        Db::from_pool(pool)
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn stores_tracks_and_users(pool: PgPool) {
        let db = db(pool).await;
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
        assert!(user.album_info);
        db.set_album_info(42, false).await.expect("album info");
        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert!(!user.album_info);
        db.set_album_info(42, true).await.expect("album info on");
        db.set_lastfm_user(42, None).await.expect("lastfm off");
        let user = db.ensure_user(42, Lang::Uk).await.expect("user");
        assert!(user.lastfm_user.is_none());

        assert!(db.purge_track(&key).await.expect("purge"));
        assert!(db.get_track(&key, 192).await.expect("get").is_none());
        assert!(db.get_track(&key, 320).await.expect("get").is_none());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn genre_analyzer_deduplicates_and_respects_toggle(pool: PgPool) {
        let db = db(pool).await;
        let user = db.ensure_user(99, Lang::En).await.expect("user");
        assert!(!user.genre_analysis);

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

        db.set_genre_analysis(99, false).await.expect("toggle off");
        assert!(
            !db.record_genres(99, "youtube:three", &["Jazz".to_owned()])
                .await
                .expect("record off again")
        );
        assert_eq!(db.genre_track_count(99).await.expect("count"), 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn genre_events_support_periods_and_user_stats(pool: PgPool) {
        let db = db(pool).await;
        db.ensure_user(77, Lang::En).await.expect("user");
        db.set_genre_analysis(77, true).await.expect("toggle");
        db.record_genres(77, "youtube:a", &["Rock".to_owned(), "Jazz".to_owned()])
            .await
            .expect("record a");

        let all = db.top_genres_since(77, 10, 0).await.expect("all time");
        assert_eq!(all.len(), 2);
        let now = unix_now();
        let old = db.top_genres_since(77, 10, now + 60).await.expect("future");
        assert!(old.is_empty(), "future window must contain no events");

        let stats = db.user_stats(77).await.expect("stats");
        assert_eq!(stats.tracks, 1);
        assert_eq!(stats.listens, 2);
        assert_eq!(stats.genres, 2);
        assert_eq!(stats.seconds, 0);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn ensure_user_throttles_last_seen_writes(pool: PgPool) {
        let db = db(pool).await;
        db.ensure_user(5, Lang::En).await.expect("create");

        sqlx::query!("UPDATE users SET last_seen_at = 0 WHERE user_id = 5")
            .execute(&db.pool)
            .await
            .expect("reset");
        db.seen.lock().expect("seen").remove(&5);
        db.ensure_user(5, Lang::En).await.expect("touch");
        assert!(last_seen(&db, 5).await > 0);

        sqlx::query!("UPDATE users SET last_seen_at = 0 WHERE user_id = 5")
            .execute(&db.pool)
            .await
            .expect("reset");
        db.ensure_user(5, Lang::En).await.expect("throttled touch");
        assert_eq!(last_seen(&db, 5).await, 0);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn stats_counts_tracks_and_users(pool: PgPool) {
        let db = db(pool).await;
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn search_cache_roundtrip(pool: PgPool) {
        let db = db(pool).await;
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn shared_selection_and_rate_limit(pool: PgPool) {
        let db = db(pool).await;
        db.put_selection(
            "abc",
            "track",
            7,
            serde_json::json!({"value": 1}),
            Duration::from_secs(60),
        )
        .await
        .expect("put selection");
        let value: serde_json::Value = db
            .get_selection("abc", "track")
            .await
            .expect("get")
            .expect("some");
        assert_eq!(value["value"], 1);
        let wrong: Option<serde_json::Value> =
            db.get_selection("abc", "album").await.expect("kind");
        assert!(wrong.is_none());
        // One-shot prompts are consumed atomically: the second caller loses.
        db.put_selection(
            "take-me",
            "track",
            7,
            serde_json::json!({"value": 1}),
            Duration::from_secs(60),
        )
        .await
        .expect("put take");
        assert!(db.take_selection("take-me", "track").await.expect("take"));
        assert!(
            !db.take_selection("take-me", "track")
                .await
                .expect("second take must fail")
        );

        db.delete_selection("abc").await.expect("delete");
        assert!(
            db.get_selection::<serde_json::Value>("abc", "track")
                .await
                .expect("get")
                .is_none()
        );

        assert!(db.allow(7, 2).await.expect("first"));
        assert!(db.allow(7, 2).await.expect("second"));
        assert!(!db.allow(7, 2).await.expect("third"));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn audio_prefs_and_group_mode_roundtrip(pool: PgPool) {
        let db = db(pool).await;
        db.ensure_user(88, Lang::En).await.expect("user");
        assert_eq!(
            db.audio_prefs(88).await.expect("prefs"),
            ("auto".to_owned(), false)
        );
        db.set_audio_format(88, "mp3").await.expect("format");
        db.set_normalize_audio(88, true).await.expect("normalize");
        assert_eq!(
            db.audio_prefs(88).await.expect("prefs"),
            ("mp3".to_owned(), true)
        );
        let user = db.ensure_user(88, Lang::En).await.expect("user");
        assert_eq!(user.audio_format, "mp3");
        assert!(user.normalize_audio);

        assert!(!db.chat_enabled(-1001).await.expect("chat"));
        db.set_chat_enabled(-1001, true, 88).await.expect("enable");
        assert!(db.chat_enabled(-1001).await.expect("chat"));
        db.set_chat_enabled(-1001, false, 88)
            .await
            .expect("disable");
        assert!(!db.chat_enabled(-1001).await.expect("chat"));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn jobs_are_claimed_cancelled_and_listed(pool: PgPool) {
        let db = db(pool).await;
        let payload = serde_json::json!({"key": "youtube:job"});
        assert!(
            db.enqueue_and_claim_job("job-1", 42, "track", payload.clone(), "replica-a")
                .await
                .expect("claim")
        );
        assert!(
            !db.enqueue_and_claim_job("job-1", 42, "track", payload, "replica-b")
                .await
                .expect("second claim must fail")
        );
        assert!(!db.job_cancel_requested("job-1").await.expect("flag"));
        assert!(db.request_cancel("job-1", 42).await.expect("cancel"));
        assert!(db.job_cancel_requested("job-1").await.expect("flag"));
        assert!(!db.request_cancel("job-1", 43).await.expect("wrong user"));

        let jobs = db.list_jobs(42, 10).await.expect("list");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].0, "job-1");
        assert_eq!(jobs[0].1, "track");
        assert!(jobs[0].3);

        db.finish_job("job-1", "cancelled", None)
            .await
            .expect("finish");
        let jobs = db.list_jobs(42, 10).await.expect("list");
        assert_eq!(jobs[0].2, "cancelled");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn listens_feed_history_dynamics_and_digest(pool: PgPool) {
        let db = db(pool).await;
        db.ensure_user(11, Lang::En).await.expect("user");
        db.set_genre_analysis(11, true).await.expect("toggle");
        let now = unix_now();
        db.record_listen(11, "yt:a", "Song A", Some("Artist"), &["Rock".into()])
            .await
            .expect("listen a");
        db.record_listen(11, "yt:b", "Song B", None, &["Jazz".into()])
            .await
            .expect("listen b");
        db.record_genres(11, "yt:a", &["Rock".into()])
            .await
            .expect("genres a");
        db.record_genres(11, "yt:b", &["Jazz".into()])
            .await
            .expect("genres b");

        let history = db.list_listens(11, now - 60, 10).await.expect("history");
        assert_eq!(history.len(), 2);
        assert_eq!(db.count_listens(11, now - 60).await.expect("count"), 2);
        assert_eq!(history[0].title, "Song B");
        let dynamics = db
            .genre_dynamics(11, now - 60, 3_600)
            .await
            .expect("dynamics");
        assert_eq!(dynamics.len(), 2);
        let summary = db.digest_summary(11, now - 60).await.expect("summary");
        assert_eq!(summary.listens, 2);
        assert_eq!(summary.tracks, 2);
        assert_eq!(summary.genres.len(), 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn digest_schedule_claim_is_atomic(pool: PgPool) {
        let db = db(pool).await;
        db.ensure_user(12, Lang::Uk).await.expect("user");
        let now = unix_now();
        db.set_digest_schedule(12, "daily", true, now - 10, 23, 7)
            .await
            .expect("set");
        let due = db.due_digests(now, 10).await.expect("due");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].lang, Lang::Uk);
        assert!(
            db.claim_digest(12, now - 10, now + 86_400)
                .await
                .expect("claim")
        );
        assert!(
            !db.claim_digest(12, now - 10, now + 86_400)
                .await
                .expect("second claim must fail")
        );
        assert!(db.due_digests(now, 10).await.expect("due after").is_empty());
        let schedule = db
            .get_digest_schedule(12)
            .await
            .expect("get")
            .expect("some");
        assert_eq!(schedule.period, "daily");
        assert!(schedule.last_sent_at.is_some());
        assert!(
            db.defer_digest(12, now + 86_400, now + 100_000)
                .await
                .expect("defer")
        );
        db.disable_digest(12).await.expect("disable");
        assert!(
            !db.get_digest_schedule(12)
                .await
                .expect("get")
                .expect("some")
                .enabled
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn ping_works(pool: PgPool) {
        let db = db(pool).await;
        assert!(db.ping().await);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn meta_and_announcement_recipients(pool: PgPool) {
        let db = db(pool).await;
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

    async fn last_seen(db: &Db, id: i64) -> i64 {
        sqlx::query_scalar!("SELECT last_seen_at FROM users WHERE user_id = $1", id)
            .fetch_one(&db.pool)
            .await
            .expect("last_seen")
    }
}
