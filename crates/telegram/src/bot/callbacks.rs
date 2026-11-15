//! Inline keyboard callbacks: search choices and per-user settings.

use crate::bot::commands::settings_keyboard;
use crate::bot::handlers::{deliver_album, deliver_track, edit_html, error_text, escape_html};
use crate::state::AppState;
use common::error::HandlerResult;
use common::i18n;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, ParseMode};
use tracing::{info, warn};

pub async fn handle_callback(state: AppState, bot: Bot, query: CallbackQuery) -> HandlerResult {
    let user_id = query.from.id.0 as i64;
    let user = state
        .db
        .ensure_user(user_id, state.config.default_lang)
        .await?;
    let is_admin = state.config.is_admin(user_id);
    if user.banned && !is_admin {
        let _ = bot
            .answer_callback_query(query.id.clone())
            .text(i18n::banned(user.lang))
            .await;
        return Ok(());
    }

    let Some(data) = query.data.as_deref() else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    };

    if let Some(id) = data.strip_prefix("q:") {
        handle_selection(&state, &bot, &query, id, user.lang, user.quality).await?;
    } else if let Some(id) = data.strip_prefix("alc:") {
        handle_album_confirm(&state, &bot, &query, id, user.lang, user.quality).await?;
    } else if let Some(id) = data.strip_prefix("alx:") {
        handle_album_cancel(&state, &bot, &query, id, user.lang).await?;
    } else if let Some(value) = data.strip_prefix("s:q:") {
        if let Ok(quality) = value.parse::<u32>() {
            state.db.set_quality(user_id, quality).await?;
            refresh_settings(&state, &bot, &query, user.lang, quality).await?;
            let _ = bot
                .answer_callback_query(query.id.clone())
                .text(i18n::settings_saved(user.lang))
                .await;
        }
    } else if let Some(code) = data.strip_prefix("s:lang:") {
        if let Some(lang) = common::domain::Lang::from_code(code) {
            state.db.set_language(user_id, lang).await?;
            let user = state
                .db
                .ensure_user(user_id, state.config.default_lang)
                .await?;
            refresh_settings(&state, &bot, &query, lang, user.quality).await?;
            let _ = bot
                .answer_callback_query(query.id.clone())
                .text(i18n::settings_saved(lang))
                .await;
        }
    } else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
    }
    Ok(())
}

async fn handle_selection(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    lang: common::domain::Lang,
    quality: u32,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let chat = message.chat.id;
    let message_id = message.id;
    let user_id = query.from.id.0 as i64;

    if !state.limiter.allow(user_id).await {
        let _ = edit_html(
            bot,
            chat,
            message_id,
            i18n::rate_limited(lang, state.config.rate_limit_per_min),
        )
        .await;
        return Ok(());
    }

    let Some(request) = state.selections.get(selection_id).await else {
        let _ = edit_html(bot, chat, message_id, i18n::selection_expired(lang)).await;
        return Ok(());
    };

    let _ = edit_html(bot, chat, message_id, i18n::downloading(lang)).await;
    info!(selection_id, "delivering selected search result");
    match deliver_track(state, bot, chat, request.clone(), quality, user_id).await {
        Ok(_) => {
            let title = request
                .meta
                .as_ref()
                .map(|meta| meta.display_title())
                .unwrap_or_else(|| "audio".to_owned());
            let _ = edit_html(
                bot,
                chat,
                message_id,
                i18n::sent_track(lang, &escape_html(&title)),
            )
            .await;
        }
        Err(error) => {
            warn!(%error, "selected result delivery failed");
            let text = error_text(lang, &error);
            let _ = edit_html(bot, chat, message_id, text).await;
        }
    }
    Ok(())
}

/// User confirmed a collection: start downloading it with limited parallelism.
async fn handle_album_confirm(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    lang: common::domain::Lang,
    quality: u32,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let chat = message.chat.id;
    let message_id = message.id;
    let user_id = query.from.id.0 as i64;

    let Some(album) = state.album_selections.get(selection_id).await else {
        let _ = edit_html(bot, chat, message_id, i18n::selection_expired(lang)).await;
        return Ok(());
    };
    state.album_selections.invalidate(selection_id).await;
    info!(
        selection_id,
        tracks = album.tracks.len(),
        "album confirmed; starting downloads"
    );
    deliver_album(state, bot, chat, message_id, lang, quality, user_id, album).await
}

/// User declined a collection: discard the pending expansion.
async fn handle_album_cancel(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    lang: common::domain::Lang,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    if let Some(message) = query.regular_message() {
        let _ = edit_html(
            bot,
            message.chat.id,
            message.id,
            i18n::album_cancelled(lang),
        )
        .await;
    }
    state.album_selections.invalidate(selection_id).await;
    info!(selection_id, "album download cancelled");
    Ok(())
}

async fn refresh_settings(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    lang: common::domain::Lang,
    quality: u32,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    bot.edit_message_text(
        message.chat.id,
        message.id,
        i18n::settings_title(lang, quality, lang),
    )
    .parse_mode(ParseMode::Html)
    .reply_markup(settings_keyboard(lang, quality))
    .await?;
    let _ = state;
    Ok(())
}
