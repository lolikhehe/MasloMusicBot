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
    pub bot_username: String,
    pub started_at: Instant,
    /// Per-user generation counter for inline queries; a new query cancels the
    /// answer of a stale one (Telegram sends `inline_query` on every keystroke).
    /// Bounded and self-expiring so the map can't grow without limit.
    pub inline_generations: Cache<i64, u64>,
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
        let album_selections = Cache::builder()
            .max_capacity(1_000)
            .time_to_live(Duration::from_secs(30 * 60))
            .build();
        let limiter = Arc::new(RateLimiter::new(config.rate_limit_per_min));
        let inline_generations = Cache::builder()
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
            bot_username,
            started_at: Instant::now(),
            inline_generations,
        }
    }

    pub fn new_selection_id(&self) -> String {
        uuid::Uuid::new_v4().simple().to_string()[..16].to_owned()
    }

    /// Bump the per-user inline generation counter; returns the new value.
    /// Atomic per key (concurrent bumps are serialized) and bounded in size.
    pub async fn bump_inline_generation(&self, user_id: i64) -> u64 {
        let entry = self
            .inline_generations
            .entry(user_id)
            .and_upsert_with(|old| async move { old.map(|entry| *entry.value()).unwrap_or(0) + 1 })
            .await;
        *entry.value()
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
