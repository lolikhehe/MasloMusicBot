//! Prometheus recorder setup. Metrics are cheap and always installed; the HTTP
//! listener is optional so local development does not need a port.

use anyhow::Result;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;

pub fn install(addr: Option<SocketAddr>) -> Result<()> {
    if let Some(addr) = addr {
        PrometheusBuilder::new()
            .with_http_listener(addr)
            .install()?;
    } else {
        // Install a recorder anyway so `metrics::counter!` never panics, but
        // serve nothing; handy for local runs and tests.
        PrometheusBuilder::new().install_recorder()?;
    }

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
    Ok(())
}
