//! Per-user sliding-window limiter. A zero limit disables limiting.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct RateLimiter {
    per_minute: u32,
    inner: Mutex<HashMap<i64, VecDeque<Instant>>>,
}

impl RateLimiter {
    pub fn new(per_minute: u32) -> Self {
        Self {
            per_minute,
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if the request is allowed.
    pub async fn allow(&self, user_id: i64) -> bool {
        if self.per_minute == 0 {
            return true;
        }
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(60);
        let mut users = self.inner.lock().await;
        users.retain(|_, entries| {
            while entries.front().is_some_and(|instant| *instant < cutoff) {
                entries.pop_front();
            }
            !entries.is_empty()
        });
        let entries = users.entry(user_id).or_default();
        if entries.len() >= self.per_minute as usize {
            return false;
        }
        entries.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn limits_after_quota() {
        let limiter = RateLimiter::new(2);
        assert!(limiter.allow(1).await);
        assert!(limiter.allow(1).await);
        assert!(!limiter.allow(1).await);
        assert!(limiter.allow(2).await);
    }

    #[tokio::test]
    async fn zero_means_unlimited() {
        let limiter = RateLimiter::new(0);
        for _ in 0..10 {
            assert!(limiter.allow(1).await);
        }
    }
}
