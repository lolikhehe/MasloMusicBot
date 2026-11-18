//! Rust-only webhook gateway.
//!
//! Terminates TLS with an automatically renewed Let's Encrypt certificate
//! (ACME TLS-ALPN-01), verifies the Telegram secret header, and proxies the
//! update to the bot's own axum listener inside the Docker network.
//!
//! No nginx, Caddy, Python or Node is involved — just this one small binary.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Router, serve};
use rustls_acme::AcmeConfig;
use rustls_acme::caches::DirCache;
use std::net::SocketAddr;
use subtle::ConstantTimeEq;
use tokio_stream::StreamExt;
use tracing::{error, info, warn};

#[derive(Clone)]
struct GatewayState {
    client: reqwest::Client,
    upstream: String,
    path: String,
    secret: Option<String>,
    webhook_url: String,
    secret_configured: bool,
}

#[derive(Clone)]
struct Config {
    domains: Vec<String>,
    email: String,
    cache_dir: String,
    production: bool,
    https_port: u16,
    http_port: u16,
    upstream: String,
    path: String,
    secret: Option<String>,
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn load_config() -> Config {
    let domains = env("DOMAINS", "localhost")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    let secret = env("WEBHOOK_SECRET", "");
    let secret = (!secret.is_empty()).then_some(secret);
    let https_port = env("HTTPS_PORT", "443").parse().unwrap_or(443);
    let http_port = env("HTTP_PORT", "80").parse().unwrap_or(80);
    Config {
        domains,
        email: env("ACME_EMAIL", "admin@localhost"),
        cache_dir: env("CACHE_DIR", "/cache"),
        production: env("ACME_PROD", "false").eq_ignore_ascii_case("true"),
        https_port,
        http_port,
        upstream: env("BOT_UPSTREAM", "http://bot:8080")
            .trim_end_matches('/')
            .to_owned(),
        path: {
            let path = env("WEBHOOK_PATH", "/telegram/webhook");
            if path.starts_with('/') {
                path
            } else {
                format!("/{path}")
            }
        },
        secret,
    }
}

/// Constant-time secret comparison. The bot performs the same check on its
/// own listener; doing it here too keeps unauthorized traffic off the Docker
/// network entirely.
fn secret_matches(expected: Option<&str>, provided: Option<&str>) -> bool {
    match (expected, provided) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(expected), Some(provided)) => expected.as_bytes().ct_eq(provided.as_bytes()).into(),
    }
}

async fn webhook(State(state): State<GatewayState>, headers: HeaderMap, body: Bytes) -> Response {
    let provided = headers
        .get("x-telegram-bot-api-secret-token")
        .and_then(|value| value.to_str().ok());
    if !secret_matches(state.secret.as_deref(), provided) {
        warn!("rejected webhook request with missing or wrong secret");
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if body.is_empty() || body.len() > 2 * 1024 * 1024 {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let mut request = state
        .client
        .post(format!("{}{}", state.upstream, state.path));
    if let Some(content_type) = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
    {
        request = request.header("content-type", content_type);
    }
    if let Some(secret) = state.secret.as_deref() {
        request = request.header("x-telegram-bot-api-secret-token", secret);
    }
    match request.body(body.to_vec()).send().await {
        Ok(response) => {
            let status =
                StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("text/plain; charset=utf-8")
                .to_owned();
            match response.bytes().await {
                Ok(bytes) => (
                    status,
                    [(axum::http::header::CONTENT_TYPE, content_type)],
                    bytes.to_vec(),
                )
                    .into_response(),
                Err(error) => {
                    error!(%error, "could not read bot response");
                    StatusCode::BAD_GATEWAY.into_response()
                }
            }
        }
        Err(error) => {
            error!(%error, "bot upstream unreachable");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

async fn health() -> Response {
    (StatusCode::OK, "{\"status\":\"ok\"}").into_response()
}

async fn index(State(state): State<GatewayState>) -> Html<String> {
    Html(format!(
        "<!doctype html><html lang=\"uk\"><meta charset=\"utf-8\">\
         <title>Webhook gateway</title><body>\
         <h1>Webhook gateway</h1>\
         <p>Endpoint: <code>{}</code></p>\
         <p>Secret configured: <b>{}</b></p>\
         <p>Upstream: <code>{}</code></p>\
         <p><a href=\"/healthz\">/healthz</a></p>\
         </body></html>",
        state.webhook_url, state.secret_configured, state.upstream
    ))
}

/// Plain HTTP: redirect to the canonical HTTPS URL. The browser/Telegram only
/// ever uses the HTTPS side; this exists so a mistyped `http://` still works.
async fn http_redirect(domain: String, uri: axum::http::Uri) -> Response {
    let path = uri.path().to_owned();
    let query = uri
        .query()
        .map(|value| format!("?{value}"))
        .unwrap_or_default();
    let location = format!("https://{domain}{path}{query}");
    (
        StatusCode::PERMANENT_REDIRECT,
        [(axum::http::header::LOCATION, location)],
    )
        .into_response()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = load_config();
    let domain = cfg
        .domains
        .first()
        .cloned()
        .unwrap_or_else(|| "localhost".into());
    let webhook_url = format!("https://{domain}{}", cfg.path);
    info!(%webhook_url, upstream = %cfg.upstream, "webhook gateway starting");
    if !cfg.production {
        warn!("ACME_PROD is false: using Let's Encrypt STAGING certificates");
    }
    if cfg.secret.is_none() {
        warn!("WEBHOOK_SECRET is not set: anyone who knows the URL can post updates");
    }

    let state = GatewayState {
        client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?,
        upstream: cfg.upstream.clone(),
        path: cfg.path.clone(),
        secret: cfg.secret.clone(),
        secret_configured: cfg.secret.is_some(),
        webhook_url,
    };

    // ---- Plain HTTP redirect server (port 80) -----------------------------
    let http_router = Router::new().fallback({
        let domain = domain.clone();
        move |uri| http_redirect(domain.clone(), uri)
    });
    let http_addr = SocketAddr::from(([0, 0, 0, 0], cfg.http_port));
    let http_task = tokio::spawn(async move {
        match tokio::net::TcpListener::bind(http_addr).await {
            Ok(listener) => {
                info!(%http_addr, "HTTP redirect listener up");
                if let Err(error) = serve(listener, http_router).await {
                    error!(%error, "HTTP server failed");
                }
            }
            Err(error) => error!(%error, %http_addr, "could not bind HTTP port"),
        }
    });

    // ---- HTTPS + automatic certificate (port 443, TLS-ALPN-01) ------------
    let mut acme = AcmeConfig::new(cfg.domains.clone())
        .contact([format!("mailto:{}", cfg.email)])
        .cache(DirCache::new(cfg.cache_dir.clone()))
        .directory_lets_encrypt(cfg.production)
        .state();
    let acceptor = acme.axum_acceptor(acme.default_rustls_config());
    let acme_task = tokio::spawn(async move {
        loop {
            match acme.next().await {
                Some(Ok(event)) => info!(?event, "ACME event"),
                Some(Err(error)) => error!(%error, "ACME error"),
                None => {
                    error!("ACME stream ended");
                    break;
                }
            }
        }
    });

    let https_router = Router::new()
        .route("/", get(index))
        .route("/healthz", get(health))
        .route(&cfg.path, post(webhook))
        .with_state(state);

    let https_addr = SocketAddr::from(([0, 0, 0, 0], cfg.https_port));
    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("shutdown requested");
                shutdown_handle.graceful_shutdown(None);
            }
            Err(error) => error!(%error, "ctrl_c failed"),
        }
    });

    info!(%https_addr, "HTTPS listener up");
    axum_server::bind(https_addr)
        .acceptor(acceptor)
        .handle(handle)
        .serve(https_router.into_make_service())
        .await?;

    http_task.abort();
    acme_task.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::secret_matches;

    #[test]
    fn secret_check_is_strict() {
        assert!(secret_matches(None, None));
        assert!(secret_matches(Some("secret"), Some("secret")));
        assert!(!secret_matches(Some("secret"), None));
        assert!(!secret_matches(Some("secret"), Some("wrong")));
        assert!(!secret_matches(Some("secret"), Some("secre")));
    }
}
