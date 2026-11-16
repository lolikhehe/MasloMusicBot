//! Shared state injected into every teloxide handler.

use crate::rate_limit::RateLimiter;
use common::config::Config;
use common::domain::{Album, TrackRequest};
use downloader::DownloadManager;
use moka::future::Cache;
use std::sync::Arc;
use std::time::{Duration, Instant};
use storage::Db;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
    pub manager: Arc<DownloadManager>,
    pub selections: Cache<String, TrackRequest>,
    /// Album/playlist selections awaiting the user's confirmation
    /// (spec: collections require a confirmation step before downloading).
    pub album_selections: Cache<String, Album>,
    pub limiter: Arc<RateLimiter>,
    /// Separate, much looser limiter for inline queries: Telegram emits one
    /// query per keystroke, so the regular budget is exhausted almost
    /// instantly while the user is typing. Default 120/min.
    pub inline_limiter: Arc<RateLimiter>,
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
    /// Last.fm client (public "now playing" feed); `None` when the operator
    /// did not set LASTFM_API_KEY.
    pub lastfm: Option<providers::lastfm::LastFmClient>,
}

impl AppState {
    pub fn new(
        config: Arc<Config>,
        db: Db,
        manager: Arc<DownloadManager>,
        bot_username: String,
    ) -> Self {
        let selections = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(30 * 60))
            .build();
        let lastfm = config
            .lastfm_api_key
            .clone()
            .map(providers::lastfm::LastFmClient::new);
        let album_selections = Cache::builder()
            .max_capacity(1_000)
            .time_to_live(Duration::from_secs(30 * 60))
            .build();
        let limiter = Arc::new(RateLimiter::new(config.rate_limit_per_min));
        let inline_limiter = Arc::new(RateLimiter::new(config.rate_limit_inline_per_min));
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
            selections,
            album_selections,
            limiter,
            inline_limiter,
            bot_username,
            started_at: Instant::now(),
            inline_generations,
            inline_generation_tx,
            lastfm,
        }
    }

    pub fn new_selection_id(&self) -> String {
        uuid::Uuid::new_v4().simple().to_string()[..16].to_owned()
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
