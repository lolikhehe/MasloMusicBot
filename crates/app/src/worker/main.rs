//! Worker replica: claims persisted download jobs and drives the same shared
//! delivery pipeline the bot uses. It only sends/edits Telegram messages — it
//! never polls for updates.

mod guard;
mod pipeline;

// Reuse the bot's health/metrics listener verbatim; /healthz stays 503 until
// the worker is about to enter its claim loop.
#[path = "../health.rs"]
mod health;

use anyhow::Context;
use common::config::Config;
use common::shutdown;
use downloader::DownloadManager;
use providers::acoustid::AcoustId;
use providers::{ExternalResolver, YtDlp};
use sqlx::postgres::PgListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use storage::Db;
use telegram::state::AppState;
use teloxide::prelude::Requester;
use tokio::task::JoinSet;
use tracing::{info, warn};

/// Max jobs claimed per drain pass; a burst of status sends past this trips
/// Telegram 429s. The 30s fallback tick picks up whatever we leave behind.
const DRAIN_CAP: usize = 12;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Clone)]
pub(crate) struct WorkerState {
    pub(crate) app: AppState,
    pub(crate) bot: teloxide::Bot,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Arc::new(Config::from_env()?);
    init_tracing(&config);

    let metrics_handle = common::metrics::install()?;
    tokio::fs::create_dir_all(&config.tmp_dir).await?;

    // Runs migrations and the stale-lease requeue — a worker crash must hand
    // its claimed rows back to the queue.
    let db = Db::connect(&config.database_url).await?;
    let ready = Arc::new(AtomicBool::new(false));
    if let Some(addr) = config.metrics_addr {
        health::serve(addr, db.clone(), metrics_handle, ready.clone()).await?;
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
    // Delivery never uses the bot username, so a transient getMe failure must
    // not keep the worker from draining the queue.
    let bot_username = bot
        .get_me()
        .await
        .ok()
        .and_then(|me| me.user.username)
        .unwrap_or_else(|| "music_bot_worker".to_owned());
    let acoustid = config.acoustid_api_key.clone().map(AcoustId::new);
    let app = AppState::new(config.clone(), db.clone(), manager, bot_username, acoustid);
    let state = WorkerState { app, bot };
    let worker_id = state.app.worker_id.clone();

    // NOTIFY wakes the worker immediately; the 30s tick is the safety net if a
    // notification is lost or the listener is down.
    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::channel::<()>(64);
    tokio::spawn(listener_task(config.clone(), notify_tx));
    let mut poll = tokio::time::interval(Duration::from_secs(30));
    poll.tick().await; // consume the immediate first tick
    let mut tasks: JoinSet<()> = JoinSet::new();

    info!(%worker_id, "worker ready; waiting for jobs");
    ready.store(true, Ordering::Relaxed);

    let shutdown_future = shutdown::wait_for_shutdown();
    tokio::pin!(shutdown_future);
    loop {
        tokio::select! {
            _ = &mut shutdown_future => {
                info!("shutdown requested; stopping job claims");
                break;
            }
            _ = notify_rx.recv() => {
                drain(&db, &worker_id, &state, &mut tasks).await;
            }
            _ = poll.tick() => {
                drain(&db, &worker_id, &state, &mut tasks).await;
            }
        }
    }

    // Stop claiming, then let in-flight jobs finish within the compose stop
    // grace (60s). The 55s cap leaves room for the final telemetry flush;
    // abandoned rows are covered by the lease requeue on the next boot.
    ready.store(false, Ordering::Relaxed);
    let finish = tokio::time::timeout(Duration::from_secs(55), async {
        while tasks.join_next().await.is_some() {}
    });
    if finish.await.is_err() {
        let remaining = tasks.len();
        warn!(
            remaining,
            "shutdown drain timed out; aborting in-flight jobs"
        );
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
    info!("worker stopped");
    Ok(())
}

/// Owns the PgListener so reconnection never borrows across the main select.
/// On any failure it drops the listener and retries with 1s→30s backoff; the
/// poll tick keeps the worker functional throughout.
async fn listener_task(config: Arc<Config>, tx: tokio::sync::mpsc::Sender<()>) {
    let mut backoff = Duration::from_secs(1);
    let mut attempts = 0usize;
    loop {
        match connect_listener(&config).await {
            Ok(mut listener) => {
                backoff = Duration::from_secs(1);
                attempts = 0;
                loop {
                    match listener.recv().await {
                        Ok(_) => {
                            if tx.send(()).await.is_err() {
                                return;
                            }
                        }
                        Err(error) => {
                            warn!(%error, "job listener recv failed; reconnecting");
                            break;
                        }
                    }
                }
            }
            Err(error) => {
                attempts += 1;
                warn!(%error, attempt = attempts, "could not connect job listener");
            }
        }
        // A burst of 10 failed attempts still yields; the outer loop keeps
        // trying instead of giving up permanently.
        if attempts >= 10 {
            attempts = 0;
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

async fn connect_listener(config: &Config) -> anyhow::Result<PgListener> {
    // sqlx 0.9's `connect` takes the URL directly and owns its reconnect pool.
    let mut listener = PgListener::connect(&config.database_url).await?;
    listener.listen("new_job").await?;
    Ok(listener)
}

/// Reap finished tasks, then claim up to `DRAIN_CAP` queued rows per pass.
async fn drain(db: &Db, worker_id: &str, state: &WorkerState, tasks: &mut JoinSet<()>) {
    while let Some(result) = tasks.try_join_next() {
        if let Err(error) = result {
            warn!(%error, "in-flight job task panicked");
        }
    }
    loop {
        // Stop claiming at the cap; the 30s fallback tick drains the remainder.
        if tasks.len() >= DRAIN_CAP {
            break;
        }
        match db.claim_next_job(worker_id).await {
            Ok(Some(job)) => {
                let id = job.id.clone();
                let ws = state.clone();
                tasks.spawn(async move {
                    if let Err(error) = crate::pipeline::execute(ws, job).await {
                        warn!(%error, job_id = %id, "job execution failed");
                    }
                });
            }
            Ok(None) => break,
            Err(error) => {
                warn!(%error, "could not claim next job");
                break;
            }
        }
    }
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
