//! Message routing: commands, links, albums and text search.

use crate::bot::commands::{self, Command};
use crate::bot::send;
use crate::state::AppState;
use common::domain::{Lang, TrackKey, TrackRequest};
use common::error::{AppError, DownloadError, HandlerResult};
use common::i18n;
use providers::{ParsedInput, parse_input};
use std::time::Instant;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, Message, MessageEntityKind, MessageId, ParseMode,
};
use tracing::{info, warn};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy)]
pub enum Delivery {
    Cached,
    Uploaded,
}

/// Extract the first hidden link attached to a message via a text_link entity.
/// Visible URLs are found by the `url` crate inside `parse_input`.
fn entity_url(msg: &Message) -> Option<String> {
    let entities = msg.entities()?;
    entities.iter().find_map(|entity| match &entity.kind {
        MessageEntityKind::TextLink { url } => Some(url.to_string()),
        _ => None,
    })
}

pub async fn handle_message(state: AppState, bot: Bot, msg: Message) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };

    if text.trim_start().starts_with('/') {
        return match commands::parse_command(text, &state.bot_username) {
            Some(Command::Unknown(_)) => {
                commands::handle_command(&state, &bot, &msg, Command::Help).await
            }
            Some(command) => commands::handle_command(&state, &bot, &msg, command).await,
            None => Ok(()),
        };
    }

    if !msg.chat.is_private() {
        let lang = common::domain::Lang::from_telegram_hint(
            msg.from.as_ref().and_then(|u| u.language_code.as_deref()),
            state.config.default_lang,
        );
        send::send_html(&bot, msg.chat.id, i18n::private_only(lang)).await?;
        return Ok(());
    }

    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };
    let user_id = from.id.0 as i64;
    let preferred = common::domain::Lang::from_telegram_hint(
        from.language_code.as_deref(),
        state.config.default_lang,
    );
    let user = state.db.ensure_user(user_id, preferred).await?;
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
    // Spec §10: user texts must not be logged beyond debugging, so we log the
    // routing decision rather than the text itself.
    let input = entity_url(&msg).unwrap_or_else(|| text.to_owned());
    info!(user_id, "handling input");

    let result = match parse_input(&input) {
        ParsedInput::Track(request) => {
            handle_track(
                &state,
                &bot,
                msg.chat.id,
                lang,
                user.quality,
                user_id,
                request,
            )
            .await
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
                handle_search(&state, &bot, msg.chat.id, lang, query, user.search_source).await
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
    user_id: i64,
    request: TrackRequest,
) -> HandlerResult {
    if try_cache_send(state, bot, chat, &request.key, quality).await? {
        return Ok(());
    }

    let placeholder = send::send_html(bot, chat, i18n::downloading(lang)).await?;
    match deliver_track(state, bot, chat, request, quality, user_id).await {
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
    search_source: common::domain::Provider,
) -> HandlerResult {
    let query: String = query.chars().take(200).collect();
    let progress = send::send_html(bot, chat, i18n::searching(lang)).await?;
    let results = match state.manager.search(&query, 8, search_source).await {
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

/// Collections are never downloaded silently: the user confirms the size and
/// the tracklist first (spec §9 stage 6).
async fn handle_album(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    lang: Lang,
    _quality: u32,
    album_ref: common::domain::AlbumRef,
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
        tracks = album.tracks.len(),
        "resolved album; awaiting user confirmation"
    );

    let selection_id = state.new_selection_id();
    let count = album.tracks.len();
    state
        .album_selections
        .insert(selection_id.clone(), album.clone())
        .await;
    let title = escape_html(&album.title);
    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            i18n::album_download_all(lang).to_owned(),
            format!("alc:{selection_id}"),
        ),
        InlineKeyboardButton::callback(
            i18n::album_cancel(lang).to_owned(),
            format!("alx:{selection_id}"),
        ),
    ]]);
    bot.edit_message_text(chat, progress.id, i18n::album_confirm(lang, &title, count))
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;
    Ok(())
}

/// Confirmed collection download. Tracks are delivered with limited parallelism
/// and the progress message is edited at most once every 2 seconds.
#[allow(clippy::too_many_arguments)]
pub async fn deliver_album(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    progress: MessageId,
    lang: Lang,
    quality: u32,
    user_id: i64,
    album: common::domain::Album,
) -> HandlerResult {
    let total = album.tracks.len();
    let title = escape_html(&album.title);
    let _ = edit_html(
        bot,
        chat,
        progress,
        i18n::album_started(lang, &title, total),
    )
    .await;

    let mut set = tokio::task::JoinSet::new();
    for track in album.tracks {
        let state = state.clone();
        let bot_clone = bot.clone();
        set.spawn(
            async move { deliver_track(&state, &bot_clone, chat, track, quality, user_id).await },
        );
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut done = 0usize;
    let mut last_edit = Instant::now();
    while let Some(result) = set.join_next().await {
        match result {
            Ok(Ok(_)) => ok += 1,
            _ => failed += 1,
        }
        done += 1;
        if last_edit.elapsed() >= std::time::Duration::from_secs(2) || done == total {
            let _ = edit_html(
                bot,
                chat,
                progress,
                i18n::album_progress(lang, done, total, &title),
            )
            .await;
            last_edit = Instant::now();
        }
    }

    let _ = edit_html(bot, chat, progress, i18n::album_done(lang, ok, failed)).await;
    Ok(())
}

/// Serve a cache entry if possible. A stale `file_id` is purged so the caller
/// can fall back to a fresh download.
async fn try_cache_send(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    key: &TrackKey,
    quality: u32,
) -> Result<bool, BoxError> {
    let Some(cached) = state.manager.get_cached(key, quality).await? else {
        return Ok(false);
    };
    match send::send_cached(bot, chat, &cached).await {
        Ok(_) => {
            state.manager.touch_cached(key, quality).await;
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
    user_id: i64,
) -> Result<Delivery, BoxError> {
    if try_cache_send(state, bot, chat, &request.key, quality).await? {
        return Ok(Delivery::Cached);
    }

    metrics::counter!("music_bot_downloads_total").increment(1);
    let download_started = Instant::now();
    let prepared = match state
        .manager
        .prepare(request.clone(), quality, user_id)
        .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            metrics::counter!("music_bot_download_errors_total").increment(1);
            return Err(error.into());
        }
    };
    metrics::histogram!("music_bot_download_seconds")
        .record(download_started.elapsed().as_secs_f64());

    // Another request may have finished while we downloaded.
    if try_cache_send(state, bot, chat, &request.key, quality).await? {
        return Ok(Delivery::Cached);
    }

    metrics::counter!("music_bot_uploads_total").increment(1);
    let upload_started = Instant::now();
    let message = send::send_prepared(bot, chat, &prepared).await?;
    metrics::histogram!("music_bot_upload_seconds").record(upload_started.elapsed().as_secs_f64());

    let Some((file_id, file_unique_id)) = send::extract_file_id(&message) else {
        return Err(AppError::Message("Telegram response has no audio file id".to_owned()).into());
    };
    let size_bytes = tokio::fs::metadata(&prepared.audio_path)
        .await
        .ok()
        .map(|meta| meta.len() as i64);
    if let Err(error) = state
        .manager
        .record(
            &request,
            &prepared.meta,
            file_id,
            file_unique_id,
            quality,
            size_bytes,
        )
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
    // Fast path: skip the escaping machinery entirely for clean titles.
    if !value
        .bytes()
        .any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\''))
    {
        return value.to_owned();
    }
    html_escape::encode_text(value).to_string()
}

/// Single pass: the old implementation iterated `chars()` twice.
pub fn truncate(value: &str, max: usize) -> String {
    let mut result = String::with_capacity(value.len().min((max + 1) * 4));
    for (index, ch) in value.chars().enumerate() {
        if index == max {
            result.push('…');
            return result;
        }
        result.push(ch);
    }
    result
}
