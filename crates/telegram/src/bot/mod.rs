//! Telegram layer.

pub mod announce;
pub mod callbacks;
pub mod commands;
pub mod handlers;
pub mod import;
pub mod inline;
pub mod lyrics;
pub mod send;
pub mod stats;

use crate::state::AppState;
use common::error::HandlerResult;
use common::shutdown;
use teloxide::dispatching::Dispatcher;
use teloxide::prelude::*;
use teloxide::types::{Update, UpdateKind};
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
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(teloxide::dptree::deps![state])
        .build();
    let shutdown_token = dispatcher.shutdown_token();
    let mut dispatcher_task = tokio::spawn(async move {
        dispatcher.dispatch().await;
    });

    shutdown::wait_for_shutdown().await;
    info!("shutdown requested");
    match shutdown_token.shutdown() {
        Ok(future) => future.await,
        Err(error) => warn!(%error, "dispatcher was already idle"),
    }

    match tokio::time::timeout(std::time::Duration::from_secs(20), &mut dispatcher_task).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) if !error.is_cancelled() => error!(%error, "dispatcher task failed"),
        Ok(Err(_)) => {}
        Err(_) => {
            warn!("dispatcher did not stop in time; aborting");
            dispatcher_task.abort();
            let _ = dispatcher_task.await;
        }
    }
    Ok(())
}
