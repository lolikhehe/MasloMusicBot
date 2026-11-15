//! Prometheus recorder setup. The recorder is always installed so `metrics::counter!`
//! never panics; when a metrics address is configured the app crate serves
//! `/metrics` and `/healthz` over a minimal HTTP listener (see `app::health`).

use anyhow::Result;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub fn install() -> Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new().install_recorder()?;

    metrics::describe_counter!("music_bot_requests_total", "Incoming user requests");
    metrics::describe_counter!("music_bot_cache_hits_total", "Served directly from cache");
    metrics::describe_counter!("music_bot_downloads_total", "Cold-path download attempts");
    metrics::describe_counter!(
        "music_bot_download_errors_total",
        "Failed cold-path downloads"
    );
    metrics::describe_counter!("music_bot_uploads_total", "Audio uploads attempted");
    metrics::describe_histogram!(
        "music_bot_request_seconds",
        "End-to-end handler latency in seconds"
    );
    metrics::describe_histogram!(
        "music_bot_download_seconds",
        "Cold-path download latency in seconds"
    );
    metrics::describe_histogram!(
        "music_bot_upload_seconds",
        "Telegram upload latency in seconds"
    );
    Ok(handle)
}
