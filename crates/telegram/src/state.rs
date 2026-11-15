//! Shared state injected into every teloxide handler.

use crate::rate_limit::RateLimiter;
use common::config::Config;
use common::domain::{Album, TrackRequest};
use downloader::DownloadManager;
use moka::future::Cache;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use storage::Db;
use tokio::sync::Mutex;

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
    pub inline_generations: Arc<Mutex<HashMap<i64, u64>>>,
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
        Self {
            config,
            db,
            manager,
            selections,
            album_selections,
            limiter,
            bot_username,
            started_at: Instant::now(),
            inline_generations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn new_selection_id(&self) -> String {
        uuid::Uuid::new_v4().simple().to_string()[..16].to_owned()
    }

    /// Bump the per-user inline generation counter; returns the new value.
    pub async fn bump_inline_generation(&self, user_id: i64) -> u64 {
        let mut map = self.inline_generations.lock().await;
        let entry = map.entry(user_id).or_insert(0);
        *entry += 1;
        *entry
    }

    /// Current generation for the user (0 if unknown).
    pub async fn inline_generation(&self, user_id: i64) -> u64 {
        self.inline_generations
            .lock()
            .await
            .get(&user_id)
            .copied()
            .unwrap_or(0)
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
