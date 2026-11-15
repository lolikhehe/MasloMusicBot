//! Entry point. Everything here is deliberately small: parse config, build the
//! shared state, then hand control to the Telegram dispatcher.

mod health;

use anyhow::Context;
use common::config::Config;
use downloader::DownloadManager;
use providers::{ExternalResolver, YtDlp};
use std::sync::Arc;
use std::time::Duration;
use storage::Db;
use telegram::state::AppState;
use teloxide::prelude::Requester;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Arc::new(Config::from_env()?);
    init_tracing(&config);

    let metrics_handle = common::metrics::install()?;

    tokio::fs::create_dir_all(&config.tmp_dir).await?;

    let db = Db::connect(&config.database_url).await?;
    if let Some(addr) = config.metrics_addr {
        health::serve(addr, db.clone(), metrics_handle).await?;
    } else {
        drop(metrics_handle);
    }
    let ytdlp = YtDlp::new(&config);
    let external = ExternalResolver::new(config.metadata_timeout)?;
    let manager = Arc::new(DownloadManager::new(
        config.clone(),
        db.clone(),
        ytdlp,
        external,
    ));

    let bot = make_bot(&config)?;
    wait_for_bot_api(&config).await;

    let me = bot.get_me().await?;
    let bot_username = me
        .user
        .username
        .clone()
        .unwrap_or_else(|| "unknown_bot".to_owned());
    tracing::info!(bot_username = %bot_username, "connected to Telegram");

    let state = AppState::new(config.clone(), db, manager, bot_username);
    telegram::bot::run(bot, state).await
}

fn init_tracing(config: &Config) {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let builder = fmt().with_env_filter(filter);
    if config.log_json {
        builder
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .init();
    } else {
        builder.init();
    }
}

fn make_bot(config: &Config) -> anyhow::Result<teloxide::Bot> {
    let bot = teloxide::Bot::new(config.token.clone());
    match &config.telegram_api_url {
        Some(url) => {
            let parsed = reqwest::Url::parse(url)
                .with_context(|| format!("TELEGRAM_API_URL is not a valid URL: {url}"))?;
            tracing::info!(%parsed, "using custom Bot API endpoint");
            Ok(bot.set_api_url(parsed))
        }
        None => Ok(bot),
    }
}

/// When a local Bot API server is used (docker compose), give it a chance to
/// become healthy before the first getMe instead of crashing on a race.
async fn wait_for_bot_api(config: &Config) {
    let Some(url) = &config.telegram_api_url else {
        return;
    };
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return;
    };
    let Some(host) = parsed.host_str().map(str::to_owned) else {
        return;
    };
    let port = parsed.port().unwrap_or(80);
    let deadline = std::time::Instant::now() + config.bot_api_wait;

    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect((host.as_str(), port)),
        )
        .await
        {
            Ok(Ok(_)) => return,
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
    tracing::warn!(
        host = %host,
        %port,
        "Bot API server did not respond within {:?}; continuing anyway",
        config.bot_api_wait
    );
}
