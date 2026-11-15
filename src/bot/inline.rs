//! Inline mode (`@bot query`) and the `chosen_inline_result` follow-up.
//!
//! Telegram only lets a bot edit an inline message when `inline_message_id` is
//! present, and it supplies that id when the chosen result carries a keyboard.
//! We therefore attach a URL keyboard to every article.

use crate::bot::handlers::{deliver_track, truncate};
use crate::bot::send;
use crate::domain::{Lang, TrackRequest};
use crate::error::HandlerResult;
use crate::i18n;
use crate::providers::{ParsedInput, parse_input};
use crate::state::AppState;
use teloxide::prelude::*;
use teloxide::types::{
    ChosenInlineResult, FileId, InlineKeyboardButton, InlineKeyboardMarkup, InlineQuery,
    InlineQueryResult, InlineQueryResultArticle, InputFile, InputMedia, InputMediaAudio,
    InputMessageContent, InputMessageContentText,
};
use tracing::{info, warn};

pub async fn handle_inline(state: AppState, bot: Bot, query: InlineQuery) -> HandlerResult {
    let user_id = query.from.id.0 as i64;
    let user = state
        .db
        .ensure_user(user_id, state.config.default_lang)
        .await?;
    let lang = user.lang;

    if user.banned && !state.config.is_admin(user_id) {
        bot.answer_inline_query(query.id.clone(), Vec::new())
            .await?;
        return Ok(());
    }
    if !state.limiter.allow(user_id).await {
        bot.answer_inline_query(query.id.clone(), Vec::new())
            .await?;
        return Ok(());
    }

    let text = query.query.trim();
    if text.is_empty() {
        bot.answer_inline_query(query.id.clone(), Vec::new())
            .await?;
        return Ok(());
    }

    let requests: Vec<TrackRequest> = match parse_input(text) {
        ParsedInput::Track(request) => match state.manager.describe(request).await {
            Ok(request) => vec![request],
            Err(error) => {
                warn!(%error, "inline metadata resolution failed");
                Vec::new()
            }
        },
        ParsedInput::Search(query) => match state.manager.search(&query, 10).await {
            Ok(results) => results
                .into_iter()
                .map(|result| result.into_request())
                .collect(),
            Err(error) => {
                warn!(%error, "inline search failed");
                Vec::new()
            }
        },
        ParsedInput::Album(_) => Vec::new(),
        ParsedInput::Unsupported(reason) => {
            info!(%reason, "unsupported inline input");
            Vec::new()
        }
    };

    let mut articles = Vec::with_capacity(requests.len());
    for request in requests {
        let selection_id = state.new_selection_id();
        let title = request
            .meta
            .as_ref()
            .map(|meta| meta.display_title())
            .unwrap_or_else(|| "Audio".to_owned());
        let description = request
            .meta
            .as_ref()
            .map(|meta| {
                let mut parts = Vec::new();
                if let Some(performer) = meta.performer.as_deref() {
                    parts.push(performer.to_owned());
                }
                if let Some(album) = meta.album.as_deref() {
                    parts.push(album.to_owned());
                }
                if let Some(duration) = meta.duration_secs {
                    parts.push(format_duration(duration));
                }
                parts.join(" • ")
            })
            .filter(|value| !value.is_empty());

        let content =
            InputMessageContent::Text(InputMessageContentText::new(i18n::preparing_inline(lang)));
        let mut article = InlineQueryResultArticle::new(selection_id.clone(), title, content);
        if let Some(description) = description {
            article = article.description(description);
        }
        if let Some(thumbnail) = request
            .meta
            .as_ref()
            .and_then(|meta| meta.thumbnail_url.as_deref())
            .and_then(|url| reqwest::Url::parse(url).ok())
        {
            article = article.thumbnail_url(thumbnail);
        }
        if let Some(source) = request
            .source_url
            .as_deref()
            .and_then(|url| reqwest::Url::parse(url).ok())
        {
            article = article.url(source);
        }
        article = article.reply_markup(inline_keyboard(&state, lang));
        state.selections.insert(selection_id, request).await;
        articles.push(InlineQueryResult::Article(article));
    }

    bot.answer_inline_query(query.id.clone(), articles)
        .cache_time(0)
        .is_personal(true)
        .await?;
    Ok(())
}

fn inline_keyboard(state: &AppState, lang: Lang) -> InlineKeyboardMarkup {
    let url = if state.bot_username.is_empty() {
        reqwest::Url::parse("https://t.me").expect("static Telegram URL is valid")
    } else {
        reqwest::Url::parse(&format!("https://t.me/{}", state.bot_username))
            .unwrap_or_else(|_| reqwest::Url::parse("https://t.me").expect("valid URL"))
    };
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::url(
        i18n::open_bot(lang),
        url,
    )]])
}

pub async fn handle_chosen(state: AppState, bot: Bot, result: ChosenInlineResult) -> HandlerResult {
    let user_id = result.from.id.0 as i64;
    let user = state
        .db
        .ensure_user(user_id, state.config.default_lang)
        .await?;
    let lang = user.lang;
    let Some(inline_message_id) = result.inline_message_id.clone() else {
        fallback_private(&state, &bot, &result, user.quality).await;
        return Ok(());
    };

    let Some(request) = state.selections.get(&result.result_id).await else {
        let _ = bot
            .edit_message_text_inline(inline_message_id, i18n::selection_expired(lang))
            .await;
        return Ok(());
    };

    if user.banned && !state.config.is_admin(user_id) {
        let _ = bot
            .edit_message_text_inline(inline_message_id, i18n::banned(lang))
            .await;
        return Ok(());
    }

    if let Some(cached) = state.manager.get_cached(&request.key).await? {
        let media = cached_media(&cached);
        match bot
            .edit_message_media_inline(inline_message_id.clone(), media)
            .await
        {
            Ok(_) => {
                state.manager.touch_cached(&request.key).await;
                metrics::counter!("music_bot_cache_hits_total").increment(1);
                return Ok(());
            }
            Err(error) if send::is_invalid_file_id(&error) => {
                warn!(%error, "inline cached file id rejected; purging");
                state.manager.purge(&request.key).await?;
            }
            Err(error) => {
                warn!(%error, "could not edit inline message with cached audio");
                fallback_private(&state, &bot, &result, user.quality).await;
                return Ok(());
            }
        }
    }

    metrics::counter!("music_bot_downloads_total").increment(1);
    let prepared = match state.manager.prepare(request.clone(), user.quality).await {
        Ok(prepared) => prepared,
        Err(error) => {
            metrics::counter!("music_bot_download_errors_total").increment(1);
            let _ = bot
                .edit_message_text_inline(inline_message_id, i18n::error(lang, &error))
                .await;
            return Ok(());
        }
    };

    let media = prepared_media(&prepared);
    match bot
        .edit_message_media_inline(inline_message_id, media)
        .await
    {
        Ok(_) => {
            // Telegram does not return a Message (and therefore no file_id)
            // from editMessageMedia, so cold inline results are not cached.
            metrics::counter!("music_bot_uploads_total").increment(1);
        }
        Err(error) => {
            warn!(%error, "could not edit inline message with uploaded audio");
            fallback_private(&state, &bot, &result, user.quality).await;
        }
    }
    Ok(())
}

fn cached_media(cached: &crate::domain::CachedTrack) -> InputMedia {
    let mut media = InputMediaAudio::new(InputFile::file_id(FileId(cached.file_id.clone())));
    media.title = Some(truncate(&cached.meta.title, 128));
    media.performer = cached
        .meta
        .performer
        .as_deref()
        .map(|value| truncate(value, 128));
    media.duration = cached
        .meta
        .duration_secs
        .map(|value| value.min(u16::MAX as u32) as u16);
    InputMedia::Audio(media)
}

fn prepared_media(prepared: &crate::download::PreparedAudio) -> InputMedia {
    let input = InputFile::file(prepared.audio_path.clone()).file_name(prepared.file_name());
    let mut media = InputMediaAudio::new(input);
    media.title = Some(truncate(&prepared.meta.title, 128));
    media.performer = prepared
        .meta
        .performer
        .as_deref()
        .map(|value| truncate(value, 128));
    media.duration = prepared
        .meta
        .duration_secs
        .map(|value| value.min(u16::MAX as u32) as u16);
    media.thumbnail = prepared
        .cover_path
        .as_ref()
        .map(|path| InputFile::file(path.clone()));
    InputMedia::Audio(media)
}

async fn fallback_private(state: &AppState, bot: &Bot, result: &ChosenInlineResult, quality: u32) {
    let Some(request) = state.selections.get(&result.result_id).await else {
        return;
    };
    let chat = ChatId(result.from.id.0 as i64);
    info!("falling back to private delivery for inline result");
    let _ = deliver_track(state, bot, chat, request, quality).await;
}

fn format_duration(seconds: u32) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes}:{seconds:02}")
}
