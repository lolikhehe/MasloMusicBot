//! Message routing: commands, links, albums and text search.

use crate::bot::commands::{self, Command};
use crate::bot::send;
use crate::domain::{Lang, TrackKey, TrackRequest};
use crate::error::{AppError, DownloadError, HandlerResult};
use crate::i18n;
use crate::providers::{ParsedInput, parse_input};
use crate::state::AppState;
use std::time::Instant;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, Message, MessageId, ParseMode};
use tracing::{info, warn};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy)]
pub enum Delivery {
    Cached,
    Uploaded,
}

pub async fn handle_message(state: AppState, bot: Bot, msg: Message) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };

    if text.trim_start().starts_with('/') {
        if let Some(command) = commands::parse_command(text, &state.bot_username)
            && !matches!(command, Command::Unknown(_))
        {
            return commands::handle_command(&state, &bot, &msg, command).await;
        }
        if let Some(Command::Unknown(_)) = commands::parse_command(text, &state.bot_username) {
            return commands::handle_command(&state, &bot, &msg, Command::Help).await;
        }
        return Ok(());
    }

    if !msg.chat.is_private() {
        let lang = state.config.default_lang;
        send::send_html(&bot, msg.chat.id, i18n::private_only(lang)).await?;
        return Ok(());
    }

    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };
    let user_id = from.id.0 as i64;
    let user = state
        .db
        .ensure_user(user_id, state.config.default_lang)
        .await?;
    let lang = user.lang;

    if user.banned && !state.config.is_admin(user_id) {
        send::send_html(&bot, msg.chat.id, i18n::banned(lang)).await?;
        return Ok(());
    }

    if !state.limiter.allow(user_id).await {
        send::send_html(
            &bot,
            msg.chat.id,
            i18n::rate_limited(lang, state.config.rate_limit_per_min),
        )
        .await?;
        return Ok(());
    }

    metrics::counter!("music_bot_requests_total").increment(1);
    let started = Instant::now();
    info!(user_id, text = %text, "handling input");

    let result = match parse_input(text) {
        ParsedInput::Track(request) => {
            handle_track(&state, &bot, msg.chat.id, lang, user.quality, request).await
        }
        ParsedInput::Album(album) => {
            handle_album(&state, &bot, msg.chat.id, lang, user.quality, album).await
        }
        ParsedInput::Search(query) => {
            let query = query.trim();
            if query.is_empty() {
                send::send_html(&bot, msg.chat.id, i18n::no_results(lang)).await?;
                Ok(())
            } else {
                handle_search(&state, &bot, msg.chat.id, lang, query).await
            }
        }
        ParsedInput::Unsupported(reason) => {
            warn!(%reason, "unsupported input");
            send::send_html(
                &bot,
                msg.chat.id,
                i18n::error(lang, &DownloadError::Unsupported(reason)),
            )
            .await?;
            Ok(())
        }
    };
    metrics::histogram!("music_bot_request_seconds").record(started.elapsed().as_secs_f64());
    result
}

async fn handle_track(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    lang: Lang,
    quality: u32,
    request: TrackRequest,
) -> HandlerResult {
    if try_cache_send(state, bot, chat, &request.key).await? {
        return Ok(());
    }

    let placeholder = send::send_html(bot, chat, i18n::downloading(lang)).await?;
    match deliver_track(state, bot, chat, request, quality).await {
        Ok(_) => {
            if let Err(error) = bot.delete_message(chat, placeholder.id).await {
                warn!(%error, "could not delete progress message");
            }
        }
        Err(error) => {
            let text = error_text(lang, &error);
            let _ = edit_html(bot, chat, placeholder.id, text).await;
            warn!(%error, "track delivery failed");
        }
    }
    Ok(())
}

async fn handle_search(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    lang: Lang,
    query: &str,
) -> HandlerResult {
    let query: String = query.chars().take(200).collect();
    let progress = send::send_html(bot, chat, i18n::searching(lang)).await?;
    let results = match state.manager.search(&query, 8).await {
        Ok(results) => results,
        Err(error) => {
            let _ = edit_html(bot, chat, progress.id, i18n::error(lang, &error)).await;
            return Ok(());
        }
    };
    if results.is_empty() {
        let _ = edit_html(bot, chat, progress.id, i18n::no_results(lang)).await;
        return Ok(());
    }

    let result_count = results.len();
    let mut rows = Vec::with_capacity(result_count);
    for (index, result) in results.into_iter().enumerate() {
        let selection_id = state.new_selection_id();
        let title = result.meta.display_title();
        let button_text = format!("{}. {}", index + 1, truncate(&title, 60));
        state
            .selections
            .insert(selection_id.clone(), result.into_request())
            .await;
        rows.push(vec![InlineKeyboardButton::callback(
            button_text,
            format!("q:{selection_id}"),
        )]);
    }
    let keyboard = InlineKeyboardMarkup::new(rows);
    let header = i18n::search_results(lang, &escape_html(&query), result_count);

    let edited = bot
        .edit_message_text(chat, progress.id, header.clone())
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard.clone())
        .await;
    if edited.is_err() {
        let _ = bot.delete_message(chat, progress.id).await;
        bot.send_message(chat, header)
            .parse_mode(ParseMode::Html)
            .reply_markup(keyboard)
            .await?;
    }
    Ok(())
}

async fn handle_album(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    lang: Lang,
    quality: u32,
    album_ref: crate::domain::AlbumRef,
) -> HandlerResult {
    let progress = send::send_html(bot, chat, i18n::album_loading(lang)).await?;
    let album = match state.manager.resolve_album(&album_ref).await {
        Ok(album) => album,
        Err(error) => {
            let _ = edit_html(bot, chat, progress.id, i18n::error(lang, &error)).await;
            return Ok(());
        }
    };
    if album.tracks.is_empty() {
        let _ = edit_html(bot, chat, progress.id, i18n::album_empty(lang)).await;
        return Ok(());
    }

    tracing::info!(
        provider = %album.provider,
        source_url = album.source_url.as_deref().unwrap_or("<none>"),
        "resolved album"
    );

    let total = album.tracks.len();
    let _ = edit_html(
        bot,
        chat,
        progress.id,
        i18n::album_started(lang, &escape_html(&album.title), total),
    )
    .await;

    let mut ok = 0usize;
    let mut failed = 0usize;
    for (index, track) in album.tracks.into_iter().enumerate() {
        let track_title = track
            .meta
            .as_ref()
            .map(|meta| meta.display_title())
            .unwrap_or_else(|| format!("Track {}", index + 1));
        let _ = edit_html(
            bot,
            chat,
            progress.id,
            i18n::album_progress(lang, index + 1, total, &escape_html(&track_title)),
        )
        .await;

        match deliver_track(state, bot, chat, track, quality).await {
            Ok(_) => ok += 1,
            Err(error) => {
                failed += 1;
                warn!(%error, index, "album track failed");
            }
        }
    }

    let _ = edit_html(bot, chat, progress.id, i18n::album_done(lang, ok, failed)).await;
    Ok(())
}

/// Serve a cache entry if possible. A stale `file_id` is purged so the caller
/// can fall back to a fresh download.
async fn try_cache_send(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    key: &TrackKey,
) -> Result<bool, BoxError> {
    let Some(cached) = state.manager.get_cached(key).await? else {
        return Ok(false);
    };
    match send::send_cached(bot, chat, &cached).await {
        Ok(_) => {
            state.manager.touch_cached(key).await;
            metrics::counter!("music_bot_cache_hits_total").increment(1);
            Ok(true)
        }
        Err(error) if send::is_invalid_file_id(&error) => {
            warn!(%error, key = %key, "cached file id rejected; purging");
            state.manager.purge(key).await?;
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

/// Shared delivery path for direct requests, search callbacks and albums.
pub async fn deliver_track(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    request: TrackRequest,
    quality: u32,
) -> Result<Delivery, BoxError> {
    if try_cache_send(state, bot, chat, &request.key).await? {
        return Ok(Delivery::Cached);
    }

    metrics::counter!("music_bot_downloads_total").increment(1);
    let download_started = Instant::now();
    let prepared = match state.manager.prepare(request.clone(), quality).await {
        Ok(prepared) => prepared,
        Err(error) => {
            metrics::counter!("music_bot_download_errors_total").increment(1);
            return Err(error.into());
        }
    };
    metrics::histogram!("music_bot_download_seconds")
        .record(download_started.elapsed().as_secs_f64());

    // Another request may have finished while we downloaded.
    if try_cache_send(state, bot, chat, &request.key).await? {
        return Ok(Delivery::Cached);
    }

    metrics::counter!("music_bot_uploads_total").increment(1);
    let upload_started = Instant::now();
    let message = send::send_prepared(bot, chat, &prepared).await?;
    metrics::histogram!("music_bot_upload_seconds").record(upload_started.elapsed().as_secs_f64());

    let Some((file_id, file_unique_id)) = send::extract_file_id(&message) else {
        return Err(AppError::Other("Telegram response has no audio file id".to_owned()).into());
    };
    if let Err(error) = state
        .manager
        .record(&request, &prepared.meta, file_id, file_unique_id)
        .await
    {
        warn!(%error, key = %request.key, "could not cache uploaded track");
    }
    Ok(Delivery::Uploaded)
}

pub async fn edit_html(
    bot: &Bot,
    chat: ChatId,
    message_id: MessageId,
    text: impl Into<String>,
) -> Result<(), teloxide::RequestError> {
    bot.edit_message_text(chat, message_id, text)
        .parse_mode(ParseMode::Html)
        .await?;
    Ok(())
}

pub fn error_text(lang: Lang, error: &BoxError) -> String {
    if let Some(download) = error.downcast_ref::<DownloadError>() {
        i18n::error(lang, download)
    } else {
        i18n::error(
            lang,
            &DownloadError::Internal("unexpected delivery error".to_owned()),
        )
    }
}

pub fn escape_html(value: &str) -> String {
    html_escape::encode_text(value).to_string()
}

pub fn truncate(value: &str, max: usize) -> String {
    let mut result: String = value.chars().take(max).collect();
    if value.chars().count() > max {
        result.push('…');
    }
    result
}
