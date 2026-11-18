//! Shared state injected into every teloxide handler.

use common::config::Config;
use common::domain::{Provider, TrackKey, TrackMeta};
use common::error::AppResult;
use downloader::DownloadManager;
use moka::future::Cache;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::{Duration, Instant};
use storage::Db;

/// Minimal identity of the last successfully delivered track. Used by
/// `/lyrics` with no arguments, so users don't have to retype Artist — Title.
#[derive(Clone)]
pub struct LastTrack {
    pub key: TrackKey,
    pub title: String,
    pub performer: Option<String>,
    pub provider: Provider,
}

/// Postgres-backed fixed-window limiter. Every replica decrements the same
/// counter, so a webhook farm cannot be bypassed by hopping between instances.
#[derive(Clone)]
pub struct SharedLimiter {
    db: Db,
    limit: i64,
}

impl SharedLimiter {
    pub async fn allow(&self, user_id: i64) -> AppResult<bool> {
        self.db.allow(user_id, self.limit).await
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
    pub manager: Arc<DownloadManager>,
    pub limiter: SharedLimiter,
    /// Separate, much looser limiter for inline queries: Telegram emits one
    /// query per keystroke, so the regular budget is exhausted almost
    /// instantly while the user is typing. Default 120/min.
    pub inline_limiter: SharedLimiter,
    pub bot_username: String,
    pub started_at: Instant,
    /// Per-user generation counter for inline queries; a new query cancels the
    /// answer of a stale one (Telegram sends `inline_query` on every keystroke).
    /// Bounded and self-expiring so the map can't grow without limit.
    pub inline_generations: Cache<i64, u64>,
    /// Watch channel per user, fired on every inline generation bump.
    /// Lets in-flight searches abort instantly when the user keeps typing
    /// instead of running a yt-dlp subprocess to the end and discarding its
    /// output.
    pub inline_generation_tx: Cache<i64, tokio::sync::watch::Sender<u64>>,
    /// Last.fm client (public "now playing" feed + genre tags); `None` when the
    /// operator did not set LASTFM_API_KEY.
    pub lastfm: Option<providers::lastfm::LastFmClient>,
    /// Genre lookups are repeated on cache hits and album replays; keep them in
    /// memory for a day so we don't hammer Last.fm/iTunes per send.
    pub genre_cache: Cache<String, Vec<String>>,
    /// Resolved collage artwork URLs (Last.fm or Deezer catalog). In-memory
    /// only; each collage re-downloads the image bytes so nothing accumulates
    /// on the operator's disk.
    pub artwork_cache: Cache<String, Option<String>>,
    /// Shared pooled client for artwork/catalog HTTP calls (Last.fm, Deezer,
    /// iTunes, cover images). Reusing it keeps TLS handshakes and idle
    /// connections out of every collage request.
    pub http: reqwest::Client,
    /// Public lyrics lookup (LRCLIB with lyrics.ovh fallback). Metadata only —
    /// no lyrics are persisted to disk.
    pub lyrics: providers::lyrics::LyricsClient,
    /// 24-hour in-memory lyrics cache (positive and negative results). No disk
    /// caching: text lives only as long as this process.
    pub lyrics_cache: Cache<String, Option<providers::lyrics::Lyrics>>,
    /// Last track sent to each user, kept in memory for the duration of a
    /// session. Bounded and expiring; lets `/lyrics` work without an argument.
    pub last_tracks: Cache<i64, LastTrack>,
    /// Callback selections live in Postgres (not in this process) so a webhook
    /// callback delivered to another replica still resolves.
    pub selection_ttl: Duration,
    /// Identifies this replica in the shared job ledger.
    pub worker_id: String,
}

/// Selection kinds stored in the `selections` table.
pub mod selection {
    pub const TRACK: &str = "track";
    pub const ALBUM: &str = "album";
    pub const ALBUM_REF: &str = "album_ref";
    pub const LYRICS: &str = "lyrics";
    /// One-shot `/import` prompt: the next message from this user is a playlist.
    pub const PENDING_IMPORT: &str = "pending_import";
}

impl AppState {
    pub fn new(
        config: Arc<Config>,
        db: Db,
        manager: Arc<DownloadManager>,
        bot_username: String,
    ) -> Self {
        let lastfm = config
            .lastfm_api_key
            .clone()
            .map(providers::lastfm::LastFmClient::new);
        let genre_cache = Cache::builder()
            .max_capacity(5_000)
            .time_to_live(Duration::from_secs(24 * 60 * 60))
            .build();
        let artwork_cache = Cache::builder()
            .max_capacity(5_000)
            .time_to_live(Duration::from_secs(24 * 60 * 60))
            .build();
        // 6 s is enough for every public catalog and cover host used here; the
        // client is shared so keep-alive connections are reused across charts.
        let http = providers::http::client(Duration::from_secs(6)).unwrap_or_default();
        let lyrics = providers::lyrics::LyricsClient::new(
            http.clone(),
            config.lrclib_base_url.clone(),
            config.lyrics_ovh_base_url.clone(),
        );
        let lyrics_cache = Cache::builder()
            .max_capacity(2_000)
            .time_to_live(Duration::from_secs(24 * 60 * 60))
            .build();
        let last_tracks = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(30 * 60))
            .build();
        let limiter = SharedLimiter {
            db: db.clone(),
            limit: i64::from(config.rate_limit_per_min),
        };
        let inline_limiter = SharedLimiter {
            db: db.clone(),
            limit: i64::from(config.rate_limit_inline_per_min),
        };
        let inline_generations = Cache::builder()
            .max_capacity(10_000)
            .time_to_idle(Duration::from_secs(600))
            .build();
        let inline_generation_tx = Cache::builder()
            .max_capacity(10_000)
            .time_to_idle(Duration::from_secs(600))
            .build();
        Self {
            config,
            db,
            manager,
            limiter,
            inline_limiter,
            bot_username,
            started_at: Instant::now(),
            inline_generations,
            inline_generation_tx,
            lastfm,
            genre_cache,
            artwork_cache,
            http,
            lyrics,
            lyrics_cache,
            last_tracks,
            selection_ttl: Duration::from_secs(30 * 60),
            worker_id: uuid::Uuid::new_v4().simple().to_string(),
        }
    }

    /// Persist a callback payload so any replica can resolve the button press.
    pub async fn put_selection<T: Serialize>(
        &self,
        id: &str,
        kind: &str,
        user_id: i64,
        payload: &T,
    ) -> AppResult<()> {
        let value = serde_json::to_value(payload)
            .map_err(|error| common::error::AppError::Other(error.into()))?;
        self.db
            .put_selection(id, kind, user_id, value, self.selection_ttl)
            .await
    }

    pub async fn get_selection<T: DeserializeOwned>(
        &self,
        id: &str,
        kind: &str,
    ) -> AppResult<Option<T>> {
        self.db.get_selection(id, kind).await
    }

    pub async fn delete_selection(&self, id: &str) -> AppResult<()> {
        self.db.delete_selection(id).await
    }

    /// Remember that a user answered `/import` with no arguments: the next
    /// message (a file or pasted track lines) is the playlist itself. The
    /// marker lives in Postgres, so a follow-up delivered to another replica
    /// still resolves. Keyed by user: a new `/import` replaces the old prompt.
    pub async fn begin_import(&self, user_id: i64) -> AppResult<()> {
        let key = format!("import:{user_id}");
        self.put_selection(
            &key,
            selection::PENDING_IMPORT,
            user_id,
            &serde_json::json!({}),
        )
        .await
    }

    /// Atomically consume the pending prompt. Returns false once it has been
    /// used or expired, so a stray message can never be imported twice.
    pub async fn take_pending_import(&self, user_id: i64) -> AppResult<bool> {
        let key = format!("import:{user_id}");
        self.db
            .take_selection(&key, selection::PENDING_IMPORT)
            .await
    }

    /// Remember a successfully sent track for argument-less `/lyrics`.
    pub async fn remember_track(
        &self,
        user_id: i64,
        meta: &TrackMeta,
        key: &TrackKey,
        provider: Provider,
    ) {
        let title = meta.stripped_title().into_owned();
        if title.is_empty() {
            return;
        }
        self.last_tracks
            .insert(
                user_id,
                LastTrack {
                    key: key.clone(),
                    title,
                    performer: meta.performer.clone(),
                    provider,
                },
            )
            .await;
    }

    /// Private chats are always allowed; groups need an admin's opt-in.
    pub async fn chat_allowed(&self, chat_id: i64) -> AppResult<bool> {
        self.db.chat_enabled(chat_id).await
    }

    pub fn new_selection_id(&self) -> String {
        let mut id = uuid::Uuid::new_v4().simple().to_string();
        id.truncate(16);
        id
    }

    /// Bump the per-user inline generation counter; returns the new value.
    /// Atomic per key (concurrent bumps are serialized) and bounded in size.
    /// Wakes any in-flight search for this user so it can abort early.
    pub async fn bump_inline_generation(&self, user_id: i64) -> u64 {
        let entry = self
            .inline_generations
            .entry(user_id)
            .and_upsert_with(|old| async move { old.map(|entry| *entry.value()).unwrap_or(0) + 1 })
            .await;
        let generation = *entry.value();
        let tx = self
            .inline_generation_tx
            .get_with(user_id, async { tokio::sync::watch::channel(generation).0 })
            .await;
        let _ = tx.send_replace(generation);
        generation
    }

    /// Subscribe to inline_generation bumps for a user (created if missing).
    pub async fn subscribe_inline_generation(
        &self,
        user_id: i64,
    ) -> tokio::sync::watch::Receiver<u64> {
        let current = self.inline_generation(user_id).await;
        self.inline_generation_tx
            .get_with(
                user_id,
                async move { tokio::sync::watch::channel(current).0 },
            )
            .await
            .subscribe()
    }

    /// Current generation for the user (0 if unknown).
    pub async fn inline_generation(&self, user_id: i64) -> u64 {
        self.inline_generations.get(&user_id).await.unwrap_or(0)
    }

    pub fn uptime(&self) -> String {
        let seconds = self.started_at.elapsed().as_secs();
        let days = seconds / 86_400;
        let hours = (seconds % 86_400) / 3_600;
        let minutes = (seconds % 3_600) / 60;
        if days > 0 {
            format!("{days}d {hours}h {minutes}m")
        } else if hours > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{minutes}m")
        }
    }
}
