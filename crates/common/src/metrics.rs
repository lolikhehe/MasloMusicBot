//! Prometheus recorder setup. The recorder is always installed so `metrics::counter!`
//! never panics; when a metrics address is configured the app crate serves
//! `/metrics` and `/healthz` over a minimal HTTP listener (see `app::health`).

use anyhow::Result;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub fn install() -> Result<PrometheusHandle> {
    // Buckets make the exporter emit true Prometheus histograms (`_bucket`
    // series). Without them every histogram becomes a summary with client-side
    // quantiles, so `histogram_quantile` over several replicas returns NO
    // SAMPLES. Values are seconds: S1 targets < 0.3 s, cold paths up to 30 s.
    let handle = PrometheusBuilder::new()
        .set_buckets(&[
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
        ])?
        .install_recorder()?;

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
    metrics::describe_histogram!(
        "music_bot_inline_seconds",
        "Inline query answer latency in seconds"
    );
    metrics::describe_gauge!(
        "music_bot_downloads_inflight",
        "Downloads running concurrently right now"
    );
    metrics::describe_gauge!(
        "music_bot_downloads_queued",
        "Keys currently tracked by the in-flight deduplication map (queue length)"
    );
    Ok(handle)
}
