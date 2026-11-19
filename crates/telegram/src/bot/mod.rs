//! Telegram layer.

pub mod announce;
pub mod callbacks;
pub mod commands;
pub mod digest;
pub mod handlers;
pub mod history;
pub mod import;
pub mod inline;
pub mod jobs;
pub mod lyrics;
pub mod recognize;
pub mod send;
pub mod stats;

use crate::state::AppState;
use common::error::HandlerResult;
use common::shutdown;
use teloxide::dispatching::Dispatcher;
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::prelude::*;
use teloxide::types::{Update, UpdateKind};
use teloxide::update_listeners::UpdateListener;
use teloxide::update_listeners::webhooks::{Options, axum_to_router};
use tracing::{error, info, warn};

async fn handle_update(bot: Bot, update: Update, state: AppState) -> HandlerResult {
    match update.kind {
        UpdateKind::Message(message) => handlers::handle_message(state, bot, message).await,
        UpdateKind::CallbackQuery(query) => callbacks::handle_callback(state, bot, query).await,
        UpdateKind::InlineQuery(query) => inline::handle_inline(state, bot, query).await,
        UpdateKind::ChosenInlineResult(result) => inline::handle_chosen(state, bot, result).await,
        _ => Ok(()),
    }
}

pub async fn run(bot: Bot, state: AppState) -> anyhow::Result<()> {
    let handler = teloxide::dptree::entry().endpoint(handle_update);
    let dependencies = teloxide::dptree::deps![state.clone()];

    if let Some(url) = state.config.webhook_url.clone() {
        info!(%url, "starting webhook listener");
        let mut options = Options::new(state.config.webhook_addr, url)
            .max_connections(state.config.webhook_max_connections);
        if let Some(secret) = state.config.webhook_secret.clone() {
            options = options.secret_token(secret);
        }
        let address = state.config.webhook_addr;
        let path = options.path.clone();
        let (mut listener, stop_future, router) = axum_to_router(bot.clone(), options).await?;
        let stop_token = listener.stop_token();
        let server = tokio::spawn(async move {
            let tcp = tokio::net::TcpListener::bind(address)
                .await
                .expect("webhook bind failed");
            axum::serve(tcp, router)
                .await
                .expect("webhook server failed");
        });
        info!(%address, %path, "webhook HTTP server listening");

        let mut dispatcher = Dispatcher::builder(bot, handler)
            .dependencies(dependencies)
            // Default keying by chat id makes teloxide handle every update from
            // one chat sequentially: /help waits for a whole album download.
            // Use the concurrent worker for all updates instead.
            .distribution_function(|_| None::<()>)
            .build();
        let shutdown_token = dispatcher.shutdown_token();
        let mut dispatcher_task = tokio::spawn(async move {
            dispatcher
                .dispatch_with_listener(
                    listener,
                    LoggingErrorHandler::with_custom_text("webhook listener error"),
                )
                .await;
        });
        await_shutdown(shutdown_token, &mut dispatcher_task).await;
        // The stop future deletes the webhook from Telegram; without awaiting
        // it a restarted replica would leave a stale webhook registered.
        stop_token.stop();
        if tokio::time::timeout(std::time::Duration::from_secs(10), stop_future)
            .await
            .is_err()
        {
            warn!("webhook deletion timed out");
        }
        server.abort();
    } else {
        info!("starting polling listener");
        let mut dispatcher = Dispatcher::builder(bot, handler)
            .dependencies(dependencies)
            // Default keying by chat id makes teloxide handle every update from
            // one chat sequentially: /help waits for a whole album download.
            // Use the concurrent worker for all updates instead.
            .distribution_function(|_| None::<()>)
            .build();
        let shutdown_token = dispatcher.shutdown_token();
        let mut dispatcher_task = tokio::spawn(async move {
            dispatcher.dispatch().await;
        });
        await_shutdown(shutdown_token, &mut dispatcher_task).await;
    }
    Ok(())
}

async fn await_shutdown(
    shutdown_token: teloxide::dispatching::ShutdownToken,
    dispatcher_task: &mut tokio::task::JoinHandle<()>,
) {
    shutdown::wait_for_shutdown().await;
    info!("shutdown requested");
    match shutdown_token.shutdown() {
        Ok(future) => {
            // A 300s download can outlast the orchestrator's stop grace period;
            // bound the graceful drain so the process exits before SIGKILL.
            if tokio::time::timeout(std::time::Duration::from_secs(30), future)
                .await
                .is_err()
            {
                warn!("dispatcher shutdown timed out");
            }
        }
        Err(error) => warn!(%error, "dispatcher was already idle"),
    }
    match tokio::time::timeout(std::time::Duration::from_secs(20), &mut *dispatcher_task).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) if !error.is_cancelled() => error!(%error, "dispatcher task failed"),
        Ok(Err(_)) => {}
        Err(_) => {
            warn!("dispatcher did not stop in time; aborting");
            dispatcher_task.abort();
            let _ = dispatcher_task.await;
        }
    }
}
