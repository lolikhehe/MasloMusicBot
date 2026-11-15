//! Minimal HTTP listener exposing `/healthz` (for the Docker healthcheck) and
//! `/metrics` (Prometheus). Implemented directly on `tokio::net::TcpListener`
//! to avoid pulling a web framework in for two static routes.

use metrics_exporter_prometheus::PrometheusHandle;
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
    let mut buf = vec![0u8; 8192];
    let read = socket.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..read]);
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");

    let (status, content_type, body) = match path {
        "/healthz" => {
            if db.ping().await {
                ("200 OK", "application/json", "{\"status\":\"ok\"}".to_owned())
            } else {
                ("503 Service Unavailable", "application/json", "{\"status\":\"degraded\"}".to_owned())
            }
        }
        "/metrics" => (
            "200 OK",
            "text/plain; version=0.0.4; charset=utf-8",
            handle.render(),
        ),
        _ => ("404 Not Found", "text/plain", "not found".to_owned()),
    };

    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await
}
