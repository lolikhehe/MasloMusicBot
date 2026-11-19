//! Minimal HTTP listener exposing `/healthz` (for the Docker healthcheck) and
//! `/metrics` (Prometheus). Implemented directly on `tokio::net::TcpListener`
//! to avoid pulling a web framework in for two static routes.

use metrics_exporter_prometheus::PrometheusHandle;
use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use storage::Db;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Whole-request read budget; Alertmanager can post several alerts at once.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// Refuse absurd Content-Length values instead of allocating without bound.
const MAX_BODY: usize = 1024 * 1024;

pub async fn serve(
    addr: SocketAddr,
    db: Db,
    handle: PrometheusHandle,
    ready: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "health/metrics listener up (/healthz, /metrics)");

    // The task runs for the lifetime of the process; the dispatcher owns shutdown.
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                continue;
            };
            let db = db.clone();
            let handle = handle.clone();
            let ready = ready.clone();
            tokio::spawn(async move {
                if let Err(error) = respond(&mut socket, db, handle, ready).await {
                    tracing::debug!(%error, "health listener connection failed");
                }
            });
        }
    });

    Ok(())
}

async fn respond(
    socket: &mut tokio::net::TcpStream,
    db: Db,
    handle: PrometheusHandle,
    ready: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let buf = tokio::time::timeout(READ_TIMEOUT, read_request(socket))
        .await
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "request read timed out")
        })??;
    let request = String::from_utf8_lossy(&buf);
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");

    let (status, content_type, body): (&str, &str, Cow<'static, str>) = match path {
        "/healthz" => {
            if ready.load(Ordering::Relaxed) && db.ping().await {
                (
                    "200 OK",
                    "application/json",
                    Cow::Borrowed("{\"status\":\"ok\"}"),
                )
            } else {
                (
                    "503 Service Unavailable",
                    "application/json",
                    Cow::Borrowed("{\"status\":\"degraded\"}"),
                )
            }
        }
        "/metrics" => (
            "200 OK",
            "text/plain; version=0.0.4; charset=utf-8",
            Cow::Owned(handle.render()),
        ),
        // Alertmanager webhook sink. Keeps the monitoring stack self-contained:
        // no extra service/image, alerts land in the bot's structured logs.
        "/alert" => {
            let payload = request
                .split_once("\r\n\r\n")
                .map(|(_, body)| body)
                .unwrap_or("");
            tracing::warn!(
                alert = %payload.chars().take(512).collect::<String>(),
                "alertmanager notification"
            );
            (
                "200 OK",
                "application/json",
                Cow::Borrowed("{\"status\":\"ok\"}"),
            )
        }
        _ => ("404 Not Found", "text/plain", Cow::Borrowed("not found")),
    };

    // Header and body are written separately so the body is not copied into
    // another allocation just to be sent.
    let header = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(header.as_bytes()).await?;
    socket.write_all(body.as_bytes()).await
}

/// Read headers, then exactly Content-Length body bytes. A fixed 4096-byte
/// read silently truncates batch alerts; this follows the advertised length.
async fn read_request(socket: &mut tokio::net::TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let read = socket.read(&mut chunk).await?;
        if read == 0 {
            return Ok(buf);
        }
        buf.extend_from_slice(&chunk[..read]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let headers = String::from_utf8_lossy(&buf[..header_end]);
    let total = header_end + content_length(&headers).min(MAX_BODY);
    while buf.len() < total {
        let read = socket.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
    }
    Ok(buf)
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_length_case_insensitively() {
        let headers = "POST /alert HTTP/1.1\r\nHost: bot\r\nContent-Length: 2048\r\n\r\n";
        assert_eq!(content_length(headers), 2048);
        assert_eq!(content_length("GET /healthz HTTP/1.1\r\n\r\n"), 0);
        assert_eq!(content_length("X: y\r\ncontent-length: nope\r\n\r\n"), 0);
    }
}
