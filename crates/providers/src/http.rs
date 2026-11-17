//! Shared HTTP client tuning and bounded downloads.
//!
//! Every metadata/image client in the workspace is built here so connection
//! pooling, keep-alive and timeouts are consistent. Response compression is
//! enabled workspace-wide in `Cargo.toml` (`reqwest/gzip`); JSON APIs benefit
//! from it, already-compressed images simply pass through.

use std::time::Duration;

/// Product identification sent to public APIs.
pub const USER_AGENT: &str = "music-bot/0.1 (Telegram music bot)";

/// Build a pooled client with a hard per-request timeout. Idle connections are
/// kept for reuse across calls instead of paying a new TLS handshake each time.
pub fn client(timeout: Duration) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        .user_agent(USER_AGENT)
        .build()
}

/// Download a response body with a hard byte ceiling. `reqwest::Response`
/// buffers the whole body by default, which lets a misbehaving server allocate
/// far past our limits; streaming in chunks stops at the ceiling instead.
pub async fn fetch_bytes(client: &reqwest::Client, url: &str, max_bytes: u64) -> Option<Vec<u8>> {
    let mut response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes)
    {
        return None;
    }
    let capacity = response
        .content_length()
        .unwrap_or(64 * 1024)
        .min(256 * 1024) as usize;
    let mut bytes = Vec::with_capacity(capacity);
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() as u64 + chunk.len() as u64 > max_bytes {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    (!bytes.is_empty()).then_some(bytes)
}
