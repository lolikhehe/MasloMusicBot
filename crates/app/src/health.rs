//! Minimal HTTP listener exposing `/healthz` (for the Docker healthcheck) and
//! `/metrics` (Prometheus). Implemented directly on `tokio::net::TcpListener`
//! to avoid pulling a web framework in for two static routes.

use metrics_exporter_prometheus::PrometheusHandle;
use std::borrow::Cow;
use std::net::SocketAddr;
use storage::Db;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn serve(addr: SocketAddr, db: Db, handle: PrometheusHandle) -> anyhow::Result<()> {
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
            tokio::spawn(async move {
                if let Err(error) = respond(&mut socket, db, handle).await {
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
) -> std::io::Result<()> {
    // Requests are tiny (`GET /healthz HTTP/1.1`); a stack buffer avoids an
    // allocation per probe.
    let mut buf = [0u8; 4096];
    let read = socket.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..read]);
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");

    let (status, content_type, body): (&str, &str, Cow<'static, str>) = match path {
        "/healthz" => {
            if db.ping().await {
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
            let raw = String::from_utf8_lossy(&buf[..read]);
            let payload = raw
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
