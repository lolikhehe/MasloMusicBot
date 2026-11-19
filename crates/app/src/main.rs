//! Entry point. Everything here is deliberately small: parse config, build the
//! shared state, then hand control to the Telegram dispatcher.

mod health;
mod janitor;

use anyhow::Context;
use common::config::Config;
use downloader::DownloadManager;
use providers::acoustid::AcoustId;
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
    // The janitor reuses the app's Db: a second Db::connect would re-run
    // migrate() and its stale-job requeue, which can reset a just-claimed job.
    janitor::spawn(db.clone(), config.tmp_dir.clone());
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
    let me = wait_for_bot_api(&config, &bot).await?;
    let bot_username = me
        .user
        .username
        .clone()
        .unwrap_or_else(|| "unknown_bot".to_owned());
    tracing::info!(bot_username = %bot_username, "connected to Telegram");

    let acoustid = config.acoustid_api_key.clone().map(AcoustId::new);
    let state = AppState::new(config.clone(), db, manager, bot_username, acoustid);
    // Opt-in digests. The planner is a detached task; all replicas sweep the
    // same Postgres rows and a CAS on `next_send_at` elects a single sender.
    telegram::bot::digest::spawn(bot.clone(), state.clone());
    // Best-effort, once per version: never block startup if Telegram refuses a PM.
    if let Err(error) = telegram::bot::announce::announce_update(&bot, &state).await {
        tracing::warn!(%error, "could not broadcast update notice");
    }
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

/// When a local Bot API server is used (docker compose), retry getMe with
/// backoff until it becomes healthy instead of crashing on a startup race.
async fn wait_for_bot_api(
    config: &Config,
    bot: &teloxide::Bot,
) -> anyhow::Result<teloxide::types::Me> {
    let Some(url) = &config.telegram_api_url else {
        return bot.get_me().await.context("getMe failed");
    };
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return bot.get_me().await.context("getMe failed");
    };
    let Some(host) = parsed.host_str().map(str::to_owned) else {
        return bot.get_me().await.context("getMe failed");
    };
    let port = parsed.port().unwrap_or(80);
    let deadline = std::time::Instant::now() + config.bot_api_wait;
    let mut backoff = Duration::from_millis(250);

    loop {
        let reachable = matches!(
            tokio::time::timeout(
                Duration::from_secs(2),
                tokio::net::TcpStream::connect((host.as_str(), port)),
            )
            .await,
            Ok(Ok(_))
        );
        if reachable
            && let Ok(Ok(me)) = tokio::time::timeout(Duration::from_secs(2), bot.get_me()).await
        {
            return Ok(me);
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(2));
    }

    bot.get_me().await.with_context(|| {
        format!(
            "Bot API server at {host}:{port} did not become ready within {:?}",
            config.bot_api_wait
        )
    })
}
