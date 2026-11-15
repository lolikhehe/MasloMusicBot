//! Shared state injected into every teloxide handler.

use crate::rate_limit::RateLimiter;
use common::config::Config;
use common::domain::TrackRequest;
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
    pub limiter: Arc<RateLimiter>,
    pub bot_username: String,
    pub started_at: Instant,
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
        let limiter = Arc::new(RateLimiter::new(config.rate_limit_per_min));
        Self {
            config,
            db,
            manager,
            selections,
            limiter,
            bot_username,
            started_at: Instant::now(),
        }
    }

    pub fn new_selection_id(&self) -> String {
        uuid::Uuid::new_v4().simple().to_string()[..16].to_owned()
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
