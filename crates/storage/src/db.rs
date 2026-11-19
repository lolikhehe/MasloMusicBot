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

/// Default lifetime of a callback selection. Matches `AppState::selection_ttl`,
/// used by the batch writer which has no per-row TTL parameter.
const SELECTION_TTL_SECS: i64 = 30 * 60;

/// A claimed download job older than this (unix seconds) is treated as
/// abandoned and may be requeued by a booting replica. Newer leases belong to
/// a live worker and must be left alone.
const JOB_LEASE_SECS: i64 = 15 * 60;

/// One saved-album bookmark. `added_at` is a unix timestamp, like every other
/// row-struct time field in this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAlbum {
    pub album_key: String,
    pub title: String,
    pub artist: String,
    pub cover_url: Option<String>,
    pub payload_json: String,
    pub added_at: i64,
}

/// A sent message queued for auto-delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMessageRef {
    pub chat_id: i64,
    pub message_id: i32,
}

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
            .max_connections(32)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(5))
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
        // A replica that crashed mid-download leaves a claimed job behind. Only
        // reclaim stale leases so a booting replica cannot reset a job that
        // another replica is still actively working on.
        let now = unix_now();
        let _ = sqlx::query!(
            "UPDATE download_jobs
             SET status = 'queued', claimed_by = NULL, updated_at = $1
             WHERE status IN ('downloading', 'uploading') AND cancel_requested = false
               AND updated_at < $2",
            now,
            now - JOB_LEASE_SECS
        )
        .execute(pool)
        .await;
        let _ = sqlx::query!("DELETE FROM search_cache WHERE expires_at < $1", now)
            .execute(pool)
            .await;
        let _ = sqlx::query!("DELETE FROM selections WHERE expires_at < $1", now)
            .execute(pool)
            .await;
        Ok(())
    }

    /// Periodic counterparts of the one-shot cleanup in `migrate()`.
    pub async fn delete_expired_selections(&self) -> AppResult<()> {
        sqlx::query!("DELETE FROM selections WHERE expires_at < $1", unix_now())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn delete_expired_search_cache(&self) -> AppResult<()> {
        sqlx::query!("DELETE FROM search_cache WHERE expires_at < $1", unix_now())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    /// Record a sent message for auto-delete. A repeated message id refreshes
    /// both its owner and its deadline instead of failing on the primary key.
    pub async fn track_sent_message(
        &self,
        chat_id: i64,
        message_id: i32,
        user_id: i64,
        delete_after_secs: i64,
    ) -> AppResult<()> {
        sqlx::query!(
            "INSERT INTO sent_messages (chat_id, message_id, user_id, delete_after)
             VALUES ($1, $2, $3, to_timestamp(EXTRACT(EPOCH FROM now())::bigint + $4))
             ON CONFLICT (chat_id, message_id) DO UPDATE SET
                 user_id = excluded.user_id,
                 delete_after = excluded.delete_after",
            chat_id,
            message_id,
            user_id,
            delete_after_secs
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Messages whose auto-delete deadline has passed, soonest first.
    pub async fn due_sent_messages(&self, limit: i64) -> AppResult<Vec<SentMessageRef>> {
        let rows = sqlx::query!(
            "SELECT chat_id, message_id FROM sent_messages
             WHERE delete_after <= now()
             ORDER BY delete_after
             LIMIT $1",
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| SentMessageRef {
                chat_id: row.chat_id,
                message_id: row.message_id,
            })
            .collect())
    }

    pub async fn delete_sent_message(&self, chat_id: i64, message_id: i32) -> AppResult<()> {
        sqlx::query!(
            "DELETE FROM sent_messages WHERE chat_id = $1 AND message_id = $2",
            chat_id,
            message_id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Janitor safety net: drop rows that were already due more than
    /// `older_than_secs` ago (e.g. the bot was down when they came due).
    pub async fn delete_stale_sent_messages(&self, older_than_secs: i64) -> AppResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM sent_messages
             WHERE delete_after < to_timestamp(EXTRACT(EPOCH FROM now())::bigint - $1)",
            older_than_secs
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected())
    }

    /// Drop only finished jobs older than `retention_secs`; active and recently
    /// finished jobs stay so a slow replica can still report their outcome.
    pub async fn delete_old_finished_jobs(&self, retention_secs: i64) -> AppResult<()> {
        sqlx::query!(
            "DELETE FROM download_jobs
             WHERE status IN ('done', 'failed', 'cancelled') AND updated_at < $1",
            unix_now() - retention_secs
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
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

    /// Batch variant of `get_track`: rows for a set of keys at one quality.
    /// Callers dedup per key; `(track_key, quality)` is already the primary key.
    pub async fn tracks_for_keys(
        &self,
        keys: &[TrackKey],
        quality: u32,
    ) -> AppResult<Vec<CachedTrack>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let keys: Vec<String> = keys.iter().map(|key| key.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            "SELECT track_key, quality, file_id, file_unique_id, source_url, provider, title,
                    performer, album, duration_secs, size_bytes, thumbnail_url, genres
             FROM tracks WHERE track_key = ANY($1) AND quality = $2",
            &keys,
            i64::from(quality)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(rows
            .into_iter()
            .map(|row| CachedTrack {
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
            })
            .collect())
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
                    normalize_audio, auto_delete_hours
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
            auto_delete_hours: row.auto_delete_hours,
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

    /// Per-user auto-delete lifetime in hours; `None` disables auto-delete.
    /// Mirrors `set_lastfm_stats`: one UPDATE, same `last_seen_at` write.
    pub async fn set_auto_delete_hours(&self, user_id: i64, hours: Option<i32>) -> AppResult<()> {
        sqlx::query!(
            "UPDATE users SET auto_delete_hours = $1, last_seen_at = $2 WHERE user_id = $3",
            hours,
            unix_now(),
            user_id
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Read side for the auto-delete setting. Kept as a dedicated accessor so
    /// callers don't pay for the column on the message hot path (`user_by_id`).
    pub async fn auto_delete_hours(&self, user_id: i64) -> AppResult<Option<i32>> {
        Ok(sqlx::query_scalar!(
            "SELECT auto_delete_hours FROM users WHERE user_id = $1",
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .flatten())
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

    /// Add the track to the caller's favorites; returns `true` when it was
    /// added and `false` when an existing entry was removed (toggle).
    pub async fn toggle_favorite(
        &self,
        id: i64,
        key: &str,
        title: &str,
        performer: Option<&str>,
    ) -> AppResult<bool> {
        // Both statements share one transaction: a concurrent toggle cannot
        // observe the row between the insert and the delete.
        let mut transaction = self.pool.begin().await.map_err(db_err)?;
        let inserted = sqlx::query!(
            "INSERT INTO favorites (user_id, track_key, title, performer, added_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id, track_key) DO NOTHING",
            id,
            key,
            title,
            performer,
            unix_now()
        )
        .execute(&mut *transaction)
        .await
        .map_err(db_err)?
        .rows_affected()
            > 0;
        if inserted {
            transaction.commit().await.map_err(db_err)?;
            return Ok(true);
        }
        sqlx::query!(
            "DELETE FROM favorites WHERE user_id = $1 AND track_key = $2",
            id,
            key
        )
        .execute(&mut *transaction)
        .await
        .map_err(db_err)?;
        transaction.commit().await.map_err(db_err)?;
        Ok(false)
    }

    pub async fn is_favorite(&self, id: i64, key: &str) -> AppResult<bool> {
        let row = sqlx::query!(
            "SELECT 1 AS present FROM favorites WHERE user_id = $1 AND track_key = $2 LIMIT 1",
            id,
            key
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.is_some())
    }

    /// `(key, title, performer)` saved via the ⭐ button, newest first.
    pub async fn favorites(
        &self,
        id: i64,
        limit: u64,
    ) -> AppResult<Vec<(String, String, Option<String>)>> {
        let rows = sqlx::query!(
            "SELECT track_key, title, performer FROM favorites
             WHERE user_id = $1 ORDER BY added_at DESC LIMIT $2",
            id,
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.track_key, row.title, row.performer))
            .collect())
    }

    /// Save an album bookmark. `album_key` is its dedup identity, so saving a
    /// second time is a no-op. Returns `true` only when a new row was inserted.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_album(
        &self,
        user_id: i64,
        album_key: &str,
        title: &str,
        artist: &str,
        cover_url: Option<&str>,
        payload_json: &str,
    ) -> AppResult<bool> {
        let result = sqlx::query!(
            "INSERT INTO saved_albums
                (user_id, album_key, title, artist, cover_url, payload_json)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_id, album_key) DO NOTHING",
            user_id,
            album_key,
            title,
            artist,
            cover_url,
            payload_json
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    /// Remove one saved album. Returns `false` when it was not saved.
    pub async fn delete_saved_album(&self, user_id: i64, album_key: &str) -> AppResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM saved_albums WHERE user_id = $1 AND album_key = $2",
            user_id,
            album_key
        )
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    /// Saved albums, newest first.
    pub async fn saved_albums(
        &self,
        user_id: i64,
        limit: u32,
        offset: u32,
    ) -> AppResult<Vec<SavedAlbum>> {
        let rows = sqlx::query!(
            "SELECT album_key, title, artist, cover_url, payload_json,
                    EXTRACT(EPOCH FROM added_at)::BIGINT AS added_at
             FROM saved_albums WHERE user_id = $1
             ORDER BY added_at DESC, album_key ASC
             LIMIT $2 OFFSET $3",
            user_id,
            i64::from(limit),
            i64::from(offset)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|row| SavedAlbum {
                album_key: row.album_key,
                title: row.title,
                artist: row.artist,
                cover_url: row.cover_url,
                payload_json: row.payload_json,
                added_at: row.added_at.unwrap_or_default(),
            })
            .collect())
    }

    pub async fn count_saved_albums(&self, user_id: i64) -> AppResult<i64> {
        Ok(sqlx::query_scalar!(
            "SELECT COUNT(*) FROM saved_albums WHERE user_id = $1",
            user_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?
        .unwrap_or_default())
    }

    /// Keys from the list the caller has already listened to (the ↩ marker).
    pub async fn heard_before(
        &self,
        id: i64,
        keys: &[String],
    ) -> AppResult<std::collections::HashSet<String>> {
        if keys.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        let rows = sqlx::query!(
            "SELECT DISTINCT track_key FROM listens WHERE user_id = $1 AND track_key = ANY($2)",
            id,
            keys
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(|row| row.track_key).collect())
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

    /// Batch writer for callback selections. Rows are `(id, kind, user_id,
    /// payload_json)`; every row gets the same TTL and conflict handling as
    /// `put_selection`, in one round trip.
    pub async fn put_selections(&self, rows: &[(String, String, i64, String)]) -> AppResult<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = rows.iter().map(|row| row.0.clone()).collect();
        let kinds: Vec<String> = rows.iter().map(|row| row.1.clone()).collect();
        let user_ids: Vec<i64> = rows.iter().map(|row| row.2).collect();
        let payloads: Vec<String> = rows.iter().map(|row| row.3.clone()).collect();
        let expires_at = unix_now() + SELECTION_TTL_SECS;
        sqlx::query!(
            "INSERT INTO selections (id, kind, user_id, payload, expires_at)
             SELECT row.id, row.kind, row.user_id, row.payload::jsonb, $5
             FROM unnest($1::text[], $2::text[], $3::bigint[], $4::text[])
                  AS row(id, kind, user_id, payload)
             ON CONFLICT(id) DO UPDATE SET
                 kind = excluded.kind,
                 user_id = excluded.user_id,
                 payload = excluded.payload,
                 expires_at = excluded.expires_at",
            &ids,
            &kinds,
            &user_ids,
            &payloads,
            expires_at
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
        match serde_json::from_value(row.payload) {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                // A legacy payload the current type cannot decode is useless to
                // the caller; treat it like an expired row so callbacks never
                // error out on old data.
                warn!(%error, id, kind, "dropping selection with undecodable payload");
                let _ = sqlx::query!("DELETE FROM selections WHERE id = $1", id)
                    .execute(&self.pool)
                    .await;
                Ok(None)
            }
        }
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

    /// Atomically consume a non-expired selection and return its raw payload.
    /// One statement replaces the `get_selection` + `delete_selection` pair,
    /// closing the window where two replicas could both act on a one-shot
    /// callback (or a duplicate delivery could be replayed).
    pub async fn consume_selection(&self, id: &str, kind: &str) -> AppResult<Option<String>> {
        Ok(sqlx::query_scalar!(
            "DELETE FROM selections
             WHERE id = $1 AND kind = $2 AND expires_at >= $3
             RETURNING payload",
            id,
            kind,
            unix_now()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .map(|payload| payload.to_string()))
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

    /// Move an active job to its terminal status. The guard makes late writers
    /// harmless (a terminal job is never overwritten), and a pending cancel
    /// always wins over the worker's own result. The cancel flag is cleared so
    /// a reused id cannot inherit a stale cancellation.
    pub async fn finish_job(&self, id: &str, status: &str, error: Option<&str>) -> AppResult<()> {
        sqlx::query!(
            "UPDATE download_jobs
             SET status = CASE WHEN cancel_requested THEN 'cancelled' ELSE $2 END,
                 error = $3, updated_at = $4, cancel_requested = false
             WHERE id = $1 AND status IN ('queued', 'downloading', 'uploading')",
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

    /// Mean wall-time of terminal `done` jobs finished in the last 24h, in
    /// seconds. Each sample is clamped to 1..=1800s so one stalled download
    /// cannot skew the estimate; `None` until at least one sample exists.
    pub async fn avg_recent_download_secs(&self) -> AppResult<Option<f64>> {
        Ok(sqlx::query_scalar!(
            "SELECT AVG(duration) FROM (
                 SELECT LEAST(1800, GREATEST(1, updated_at - created_at))::double precision AS duration
                 FROM download_jobs
                 WHERE status = 'done' AND updated_at >= $1
             ) d
             HAVING COUNT(*) >= 1",
            unix_now() - 86_400
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .flatten())
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

        db.record_listen(
            42,
            "deezer:track:2246288",
            "Alison",
            Some("Slowdive"),
            &["Dream Pop".to_owned()],
        )
        .await
        .expect("listen");
        let heard = db
            .heard_before(
                42,
                &["deezer:track:2246288".to_owned(), "youtube:x".to_owned()],
            )
            .await
            .expect("heard");
        assert!(heard.contains("deezer:track:2246288"));
        assert!(!heard.contains("youtube:x"));
        assert!(
            !db.is_favorite(42, "deezer:track:2246288")
                .await
                .expect("fav off")
        );
        assert!(
            db.toggle_favorite(42, "deezer:track:2246288", "Alison", Some("Slowdive"))
                .await
                .expect("fav add")
        );
        assert!(
            db.is_favorite(42, "deezer:track:2246288")
                .await
                .expect("fav on")
        );
        assert!(
            !db.toggle_favorite(42, "deezer:track:2246288", "Alison", Some("Slowdive"))
                .await
                .expect("fav remove")
        );
        let favorites = db.favorites(42, 30).await.expect("favorites");
        assert!(favorites.is_empty(), "toggled back off");
        db.toggle_favorite(42, "deezer:track:2246288", "Alison", Some("Slowdive"))
            .await
            .expect("fav add again");
        let favorites = db.favorites(42, 30).await.expect("favorites");
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].0, "deezer:track:2246288");
        assert_eq!(favorites[0].1, "Alison");
        assert_eq!(favorites[0].2.as_deref(), Some("Slowdive"));
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
    async fn consume_selection_returns_payload_once(pool: PgPool) {
        let db = db(pool).await;
        db.put_selection(
            "once",
            "track",
            7,
            serde_json::json!({"v": 42}),
            Duration::from_secs(60),
        )
        .await
        .expect("put");
        let first = db
            .consume_selection("once", "track")
            .await
            .expect("consume")
            .expect("some");
        let value: serde_json::Value = serde_json::from_str(&first).expect("valid payload");
        assert_eq!(value["v"], 42);
        assert!(
            db.consume_selection("once", "track")
                .await
                .expect("consume again")
                .is_none(),
            "a consumed selection must not be returned twice"
        );
        // Wrong kind or expired rows must not be consumed either.
        db.put_selection(
            "other-kind",
            "track",
            7,
            serde_json::json!({}),
            Duration::from_secs(60),
        )
        .await
        .expect("put other");
        assert!(
            db.consume_selection("other-kind", "album")
                .await
                .expect("wrong kind")
                .is_none()
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_selection_drops_legacy_payload(pool: PgPool) {
        let db = db(pool).await;
        db.put_selection(
            "legacy",
            "track",
            7,
            serde_json::json!({"old": true}),
            Duration::from_secs(60),
        )
        .await
        .expect("put");
        let parsed: Option<Vec<u32>> = db.get_selection("legacy", "track").await.expect("get");
        assert!(parsed.is_none(), "undecodable payload treated as expired");
        let remaining: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM selections WHERE id = 'legacy'")
                .fetch_one(&db.pool)
                .await
                .expect("count")
                .unwrap_or_default();
        assert_eq!(remaining, 0, "legacy row deleted after decode failure");
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
    async fn finish_job_honours_cancel_and_ignores_terminal(pool: PgPool) {
        let db = db(pool).await;
        db.enqueue_and_claim_job("j-cancel", 1, "track", serde_json::json!({}), "w")
            .await
            .expect("claim");
        assert!(db.request_cancel("j-cancel", 1).await.expect("cancel"));
        // Worker finishes after the cancel check: the pending cancel wins.
        db.finish_job("j-cancel", "done", None)
            .await
            .expect("finish");
        let jobs = db.list_jobs(1, 10).await.expect("jobs");
        assert_eq!(jobs[0].2, "cancelled");
        assert!(!jobs[0].3, "cancel_requested cleared on finish");

        // A late second writer must not overwrite an already-terminal job.
        db.finish_job("j-cancel", "done", None)
            .await
            .expect("late finish");
        let jobs = db.list_jobs(1, 10).await.expect("jobs after late finish");
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn put_selections_and_tracks_for_keys(pool: PgPool) {
        let db = db(pool).await;
        let rows = vec![
            (
                "sel-a".to_owned(),
                "track".to_owned(),
                7,
                serde_json::json!({"v": 1}).to_string(),
            ),
            (
                "sel-b".to_owned(),
                "album".to_owned(),
                8,
                serde_json::json!({"v": 2}).to_string(),
            ),
        ];
        db.put_selections(&rows).await.expect("put batch");
        let a: serde_json::Value = db
            .get_selection("sel-a", "track")
            .await
            .expect("get a")
            .expect("some");
        assert_eq!(a["v"], 1);
        let b: serde_json::Value = db
            .get_selection("sel-b", "album")
            .await
            .expect("get b")
            .expect("some");
        assert_eq!(b["v"], 2);
        // Same conflict path as put_selection: id overwrites all other columns.
        db.put_selections(&[(
            "sel-a".to_owned(),
            "album".to_owned(),
            9,
            serde_json::json!({"v": 3}).to_string(),
        )])
        .await
        .expect("upsert");
        assert!(
            db.get_selection::<serde_json::Value>("sel-a", "track")
                .await
                .expect("old kind")
                .is_none()
        );
        let updated: serde_json::Value = db
            .get_selection("sel-a", "album")
            .await
            .expect("get updated")
            .expect("some");
        assert_eq!(updated["v"], 3);

        let key = TrackKey::new("youtube:batch");
        let track = CachedTrack {
            key: key.clone(),
            file_id: "FILE".into(),
            file_unique_id: "UNIQUE".into(),
            source_url: None,
            provider: Provider::YouTube,
            meta: TrackMeta {
                title: "Batch".into(),
                ..Default::default()
            },
        };
        db.put_track(&track, 192, Some(1000)).await.expect("put");
        let other = CachedTrack {
            key: TrackKey::new("youtube:other"),
            ..track.clone()
        };
        db.put_track(&other, 192, Some(1000))
            .await
            .expect("put other");
        let found = db
            .tracks_for_keys(&[key.clone(), TrackKey::new("youtube:missing")], 192)
            .await
            .expect("keys");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].key, key);
        assert_eq!(found[0].meta.title, "Batch");
        assert!(
            db.tracks_for_keys(&[], 192)
                .await
                .expect("empty keys")
                .is_empty()
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn periodic_cleanup_deletes_expired_rows_and_old_jobs(pool: PgPool) {
        let db = db(pool).await;
        db.put_selection(
            "old-sel",
            "track",
            1,
            serde_json::json!({}),
            Duration::from_secs(60),
        )
        .await
        .expect("old selection");
        db.put_selection(
            "fresh-sel",
            "track",
            1,
            serde_json::json!({}),
            Duration::from_secs(60),
        )
        .await
        .expect("fresh selection");
        db.put_search_cache("yt", "old", "[]", Duration::from_secs(60))
            .await
            .expect("old cache");
        db.put_search_cache("yt", "fresh", "[]", Duration::from_secs(60))
            .await
            .expect("fresh cache");
        sqlx::query!("UPDATE selections SET expires_at = 0 WHERE id = 'old-sel'")
            .execute(&db.pool)
            .await
            .expect("backdate selection");
        sqlx::query!("UPDATE search_cache SET expires_at = 0 WHERE query = 'old'")
            .execute(&db.pool)
            .await
            .expect("backdate cache");

        db.enqueue_and_claim_job("old-done", 1, "track", serde_json::json!({}), "w")
            .await
            .expect("old job");
        db.finish_job("old-done", "done", None)
            .await
            .expect("finish old");
        db.enqueue_and_claim_job("fresh-done", 1, "track", serde_json::json!({}), "w")
            .await
            .expect("fresh job");
        db.finish_job("fresh-done", "done", None)
            .await
            .expect("finish fresh");
        sqlx::query!("UPDATE download_jobs SET updated_at = 0 WHERE id = 'old-done'")
            .execute(&db.pool)
            .await
            .expect("backdate job");

        db.delete_expired_selections()
            .await
            .expect("selection cleanup");
        db.delete_expired_search_cache()
            .await
            .expect("cache cleanup");
        db.delete_old_finished_jobs(60).await.expect("job cleanup");

        let old_sel: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM selections WHERE id = 'old-sel'")
                .fetch_one(&db.pool)
                .await
                .expect("count old selection")
                .unwrap_or_default();
        assert_eq!(old_sel, 0);
        assert!(
            db.get_selection::<serde_json::Value>("fresh-sel", "track")
                .await
                .expect("fresh selection")
                .is_some()
        );
        assert!(
            db.get_search_cache("yt", "old")
                .await
                .expect("old cache")
                .is_none()
        );
        assert!(
            db.get_search_cache("yt", "fresh")
                .await
                .expect("fresh cache")
                .is_some()
        );
        let jobs = db.list_jobs(1, 10).await.expect("jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].0, "fresh-done");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn saved_albums_save_list_delete(pool: PgPool) {
        let db = db(pool).await;
        assert!(
            db.save_album(
                42,
                "deezer:album:1",
                "Album A",
                "Artist A",
                Some("https://cover/a"),
                r#"{"source":"deezer","id":"1"}"#,
            )
            .await
            .expect("save")
        );
        assert!(
            !db.save_album(42, "deezer:album:1", "Album A", "Artist A", None, "{}")
                .await
                .expect("duplicate")
        );
        assert_eq!(db.count_saved_albums(42).await.expect("count"), 1);
        assert!(
            db.save_album(
                42,
                "youtube:album:2",
                "Album B",
                "Artist B",
                None,
                r#"{"source":"youtube","id":"2"}"#,
            )
            .await
            .expect("save 2")
        );
        assert_eq!(db.count_saved_albums(42).await.expect("count"), 2);

        // Both rows share the same `now()`; backdate one for deterministic order.
        sqlx::query!(
            "UPDATE saved_albums SET added_at = now() - interval '1 hour'
             WHERE user_id = 42 AND album_key = 'deezer:album:1'"
        )
        .execute(&db.pool)
        .await
        .expect("backdate album");

        let list = db.saved_albums(42, 10, 0).await.expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].album_key, "youtube:album:2");
        assert_eq!(list[0].title, "Album B");
        assert_eq!(list[1].album_key, "deezer:album:1");
        assert_eq!(list[1].artist, "Artist A");
        assert_eq!(list[1].cover_url.as_deref(), Some("https://cover/a"));
        assert_eq!(list[1].payload_json, r#"{"source":"deezer","id":"1"}"#);
        assert!(list[0].added_at > list[1].added_at);

        let page = db.saved_albums(42, 1, 1).await.expect("page");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].album_key, "deezer:album:1");
        assert!(
            db.saved_albums(43, 10, 0)
                .await
                .expect("other user")
                .is_empty()
        );

        assert!(
            db.delete_saved_album(42, "deezer:album:1")
                .await
                .expect("delete")
        );
        assert!(
            !db.delete_saved_album(42, "deezer:album:1")
                .await
                .expect("delete again")
        );
        assert_eq!(db.count_saved_albums(42).await.expect("count after"), 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn saved_albums_order_is_stable_with_tied_timestamps(pool: PgPool) {
        let db = db(pool).await;
        for (key, title) in [
            ("z:album", "Zed"),
            ("a:album", "Alpha"),
            ("m:album", "Middle"),
        ] {
            db.save_album(42, key, title, "Artist", None, "{}")
                .await
                .expect("save");
        }
        // Give every row an identical added_at so only the tiebreaker decides.
        sqlx::query!(
            "UPDATE saved_albums SET added_at = now() - interval '1 hour' WHERE user_id = 42"
        )
        .execute(&db.pool)
        .await
        .expect("tie timestamps");

        let list = db.saved_albums(42, 10, 0).await.expect("list");
        let keys: Vec<&str> = list.iter().map(|album| album.album_key.as_str()).collect();
        assert_eq!(keys, vec!["a:album", "m:album", "z:album"]);

        // The same order holds across pages, so pagination cannot skip/duplicate.
        let first = db.saved_albums(42, 1, 0).await.expect("page 0");
        let second = db.saved_albums(42, 1, 1).await.expect("page 1");
        let third = db.saved_albums(42, 1, 2).await.expect("page 2");
        assert_eq!(first[0].album_key, "a:album");
        assert_eq!(second[0].album_key, "m:album");
        assert_eq!(third[0].album_key, "z:album");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sent_messages_track_due_delete(pool: PgPool) {
        let db = db(pool).await;
        db.track_sent_message(-1001, 10, 42, 60)
            .await
            .expect("track");
        assert!(db.due_sent_messages(10).await.expect("due").is_empty());

        // Upsert refreshes both the owner and the deadline.
        db.track_sent_message(-1001, 10, 43, 3600)
            .await
            .expect("refresh");
        let owner: i64 = sqlx::query_scalar!(
            "SELECT user_id FROM sent_messages WHERE chat_id = -1001 AND message_id = 10"
        )
        .fetch_one(&db.pool)
        .await
        .expect("owner");
        assert_eq!(owner, 43);
        assert!(
            db.due_sent_messages(10)
                .await
                .expect("due after refresh")
                .is_empty()
        );

        sqlx::query!(
            "UPDATE sent_messages SET delete_after = now() - interval '1 second'
             WHERE chat_id = -1001 AND message_id = 10"
        )
        .execute(&db.pool)
        .await
        .expect("backdate");
        let due = db.due_sent_messages(10).await.expect("due");
        assert_eq!(
            due,
            vec![SentMessageRef {
                chat_id: -1001,
                message_id: 10
            }]
        );

        db.delete_sent_message(-1001, 10).await.expect("delete");
        assert!(
            db.due_sent_messages(10)
                .await
                .expect("due after delete")
                .is_empty()
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn delete_stale_sent_messages_only_removes_old(pool: PgPool) {
        let db = db(pool).await;
        db.track_sent_message(-2001, 1, 42, 60)
            .await
            .expect("old track");
        db.track_sent_message(-2001, 2, 42, 60)
            .await
            .expect("fresh track");
        sqlx::query!(
            "UPDATE sent_messages SET delete_after = now() - interval '2 hours'
             WHERE chat_id = -2001 AND message_id = 1"
        )
        .execute(&db.pool)
        .await
        .expect("backdate old");
        sqlx::query!(
            "UPDATE sent_messages SET delete_after = now() - interval '10 seconds'
             WHERE chat_id = -2001 AND message_id = 2"
        )
        .execute(&db.pool)
        .await
        .expect("backdate fresh");

        assert_eq!(db.delete_stale_sent_messages(60).await.expect("janitor"), 1);
        let remaining = sqlx::query!(
            "SELECT message_id FROM sent_messages WHERE chat_id = -2001 ORDER BY message_id"
        )
        .fetch_all(&db.pool)
        .await
        .expect("remaining");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].message_id, 2);
        assert_eq!(
            db.delete_stale_sent_messages(60)
                .await
                .expect("janitor again"),
            0
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn auto_delete_hours_roundtrip(pool: PgPool) {
        let db = db(pool).await;
        db.ensure_user(42, Lang::En).await.expect("user");
        assert_eq!(db.auto_delete_hours(42).await.expect("default"), None);
        db.set_auto_delete_hours(42, Some(24)).await.expect("set");
        assert_eq!(db.auto_delete_hours(42).await.expect("get"), Some(24));
        db.set_auto_delete_hours(42, None).await.expect("clear");
        assert_eq!(db.auto_delete_hours(42).await.expect("get cleared"), None);
    }

    async fn seed_job(db: &Db, id: &str, status: &str, created_at: i64, updated_at: i64) {
        sqlx::query!(
            "INSERT INTO download_jobs (id, user_id, kind, status, payload, created_at, updated_at)
             VALUES ($1, 1, 'track', $2, '{}'::jsonb, $3, $4)",
            id,
            status,
            created_at,
            updated_at
        )
        .execute(&db.pool)
        .await
        .expect("seed job");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn avg_recent_download_secs_uses_recent_done_samples(pool: PgPool) {
        let db = db(pool).await;
        let now = unix_now();
        // Backdated but inside 24h; zero duration clamps to 1, the 48h-old and
        // non-done jobs must not count.
        seed_job(&db, "j1", "done", now - 7200, now - 7190).await;
        seed_job(&db, "j2", "done", now - 3600, now - 3580).await;
        seed_job(&db, "j3", "done", now - 60, now - 30).await;
        seed_job(&db, "j-zero", "done", now - 5, now - 5).await;
        seed_job(&db, "j-old", "done", now - 172_800, now - 172_700).await;
        seed_job(&db, "j-queued", "queued", now - 100, now).await;
        assert_eq!(
            db.avg_recent_download_secs().await.expect("avg"),
            Some(15.25)
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn avg_recent_download_secs_needs_one_sample(pool: PgPool) {
        let db = db(pool).await;
        assert_eq!(db.avg_recent_download_secs().await.expect("avg"), None);
        let now = unix_now();
        seed_job(&db, "j1", "done", now - 100, now - 90).await;
        assert_eq!(
            db.avg_recent_download_secs().await.expect("avg"),
            Some(10.0)
        );
        seed_job(&db, "j2", "done", now - 200, now - 180).await;
        assert_eq!(
            db.avg_recent_download_secs().await.expect("avg"),
            Some(15.0)
        );
    }

    async fn last_seen(db: &Db, id: i64) -> i64 {
        sqlx::query_scalar!("SELECT last_seen_at FROM users WHERE user_id = $1", id)
            .fetch_one(&db.pool)
            .await
            .expect("last_seen")
    }
}
