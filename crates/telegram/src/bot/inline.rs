//! Inline mode (`@bot query`) and the `chosen_inline_result` follow-up.
//!
//! Cached tracks answer immediately with `CachedAudio` (Telegram streams the
//! existing file_id, no bot involvement). Uncached results are articles whose
//! inline keyboard forces `inline_message_id` to be present, so the bot can
//! later swap the placeholder for the audio via `editMessageMedia`.
//! Bot API forbids uploading a fresh file into an inline message: the new
//! media must reference a previously uploaded `file_id`, so a cold track is
//! first uploaded to the `STORAGE_CHAT_ID` channel — or, when no storage
//! channel is configured, to the user's own private chat — and the resulting
//! `file_id` is cached and used to edit the placeholder in place. No group
//! membership is required for `editMessageMedia` on an inline message (§9,
//! етап 7).
//!
//! `inline_query` fires on every keystroke, so results are per-user and the
//! answer of a stale query is silently dropped (generation counter).

use crate::bot::handlers::{deliver_track, truncate};
use crate::bot::send;
use crate::state::AppState;
use common::domain::{CachedTrack, Lang, TrackRequest};
use common::error::HandlerResult;
use common::i18n;
use providers::{ParsedInput, parse_input};
use teloxide::prelude::*;
use teloxide::types::{
    ChosenInlineResult, FileId, InlineKeyboardButton, InlineKeyboardMarkup, InlineQuery,
    InlineQueryResult, InlineQueryResultArticle, InlineQueryResultCachedAudio, InputFile,
    InputMedia, InputMediaAudio, InputMessageContent, InputMessageContentText,
};
use tracing::{info, warn};

pub async fn handle_inline(state: AppState, bot: Bot, query: InlineQuery) -> HandlerResult {
    let started = std::time::Instant::now();
    let user_id = query.from.id.0 as i64;
    let preferred = common::domain::Lang::from_telegram_hint(
        query.from.language_code.as_deref(),
        state.config.default_lang,
    );
    let user = state.db.ensure_user(user_id, preferred).await?;
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
    if text.chars().count() < 2 {
        bot.answer_inline_query(query.id.clone(), Vec::new())
            .await?;
        return Ok(());
    }

    let generation = state.bump_inline_generation(user_id).await;

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

    // The user typed more while we were searching; Telegram already knows.
    if generation != state.inline_generation(user_id).await {
        info!("stale inline query dropped");
        return Ok(());
    }

    let mut results = Vec::with_capacity(requests.len());
    for request in requests {
        let selection_id = state.new_selection_id();
        let title = request
            .meta
            .as_ref()
            .map(|meta| meta.display_title())
            .unwrap_or_else(|| "Audio".to_owned());

        // Cached tracks answer instantly with the stored file_id.
        if let Some(cached) = state.manager.get_cached(&request.key, user.quality).await? {
            let audio = InlineQueryResultCachedAudio::new(
                selection_id.clone(),
                FileId(cached.file_id.clone()),
            );
            state.selections.insert(selection_id, request).await;
            results.push(InlineQueryResult::CachedAudio(audio));
            continue;
        }

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

        let content = InputMessageContent::Text(InputMessageContentText::new(
            i18n::downloading_track(lang, &title),
        ));
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
        results.push(InlineQueryResult::Article(article));
    }

    metrics::histogram!("music_bot_inline_seconds").record(started.elapsed().as_secs_f64());
    bot.answer_inline_query(query.id.clone(), results)
        .cache_time(30)
        .is_personal(false)
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
    let preferred = common::domain::Lang::from_telegram_hint(
        result.from.language_code.as_deref(),
        state.config.default_lang,
    );
    let user = state.db.ensure_user(user_id, preferred).await?;
    let lang = user.lang;

    if user.banned && !state.config.is_admin(user_id) {
        if let Some(inline_message_id) = result.inline_message_id.clone() {
            let _ = bot
                .edit_message_text_inline(inline_message_id, i18n::banned(lang))
                .await;
        }
        return Ok(());
    }

    let Some(request) = state.selections.get(&result.result_id).await else {
        // Cached results never need a message edit (Telegram streams the file
        // itself), and expired selections simply cannot be resolved anymore.
        if let Some(inline_message_id) = result.inline_message_id.clone() {
            let _ = bot
                .edit_message_text_inline(inline_message_id, i18n::selection_expired(lang))
                .await;
        }
        return Ok(());
    };

    // Cached path: the file_id already exists, so the audio is either already
    // posted by Telegram (CachedAudio, no inline_message_id) or the placeholder
    // message can be replaced with the audio right away.
    if let Some(cached) = state.manager.get_cached(&request.key, user.quality).await? {
        state.manager.touch_cached(&request.key, user.quality).await;
        metrics::counter!("music_bot_cache_hits_total").increment(1);
        let Some(inline_message_id) = result.inline_message_id.clone() else {
            return Ok(());
        };
        match bot
            .edit_message_media_inline(inline_message_id, cached_media(&cached))
            .await
        {
            Ok(_) => return Ok(()),
            Err(error) if send::is_invalid_file_id(&error) => {
                warn!(%error, "inline cached file id rejected; purging");
                state.manager.purge(&request.key).await?;
            }
            Err(error) => {
                warn!(%error, "could not edit inline message with cached audio");
                return Ok(());
            }
        }
    }

    metrics::counter!("music_bot_downloads_total").increment(1);
    let prepared = match state
        .manager
        .prepare(request.clone(), user.quality, user_id)
        .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            metrics::counter!("music_bot_download_errors_total").increment(1);
            if let Some(inline_message_id) = result.inline_message_id.clone() {
                let _ = bot
                    .edit_message_text_inline(inline_message_id, i18n::error(lang, &error))
                    .await;
            }
            return Ok(());
        }
    };

    let size_bytes = tokio::fs::metadata(&prepared.audio_path)
        .await
        .ok()
        .map(|meta| meta.len() as i64);

    // An inline message may only carry a previously uploaded `file_id` — fresh
    // uploads are forbidden ("a new file can't be uploaded; use a previously
    // uploaded file via its file_id or specify a URL"). Mint a `file_id` in the
    // first chat we know we can reach: the storage channel if configured,
    // otherwise the user's own private chat with the bot (they started the bot
    // to use inline mode, so the send always lands there). The placeholder in
    // the group is then edited to the audio — no group membership needed — and
    // the `file_id` is cached so the next pick answers instantly via
    // `CachedAudio`. The PM copy is the only durable holder of that `file_id`;
    // keep it (deleting the storage message may invalidate the cached id).
    let upload_targets = [
        state.config.storage_chat_id.map(ChatId),
        Some(ChatId(user_id)),
    ]
    .into_iter()
    .flatten();
    let mut minted: Option<(String, String)> = None;
    for chat in upload_targets {
        match send::send_prepared(&bot, chat, &prepared).await {
            Ok(message) => match send::extract_file_id(&message) {
                Some(pair) => {
                    minted = Some(pair);
                    break;
                }
                None => warn!(
                    chat = chat.0,
                    "inline file_id upload yielded no audio file_id"
                ),
            },
            Err(error) => warn!(%error, chat = chat.0, "inline file_id upload failed"),
        }
    }

    // The storage/PM message must NOT be deleted (the cached `file_id` may die
    // with it — spec §9, етап 7).
    let Some((file_id, file_unique_id)) = minted else {
        warn!("could not mint file_id for inline result; falling back to private delivery");
        fallback_private(&state, &bot, &result, user.quality, lang).await;
        return Ok(());
    };

    if let Err(error) = state
        .manager
        .record(
            &request,
            &prepared.meta,
            file_id.clone(),
            file_unique_id.clone(),
            user.quality,
            size_bytes,
        )
        .await
    {
        warn!(%error, "could not record inline file_id in cache");
    }
    metrics::counter!("music_bot_uploads_total").increment(1);
    if let Some(inline_message_id) = result.inline_message_id.clone() {
        let media = file_id_media(&prepared, &file_id);
        if let Err(error) = bot
            .edit_message_media_inline(inline_message_id, media)
            .await
        {
            warn!(%error, "could not edit inline message with minted file_id");
        }
    }
    Ok(())
}

fn cached_media(cached: &CachedTrack) -> InputMedia {
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

fn file_id_media(prepared: &downloader::PreparedAudio, file_id: &str) -> InputMedia {
    let mut media = InputMediaAudio::new(InputFile::file_id(FileId(file_id.to_owned())));
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
    InputMedia::Audio(media)
}

async fn fallback_private(
    state: &AppState,
    bot: &Bot,
    result: &ChosenInlineResult,
    quality: u32,
    lang: Lang,
) {
    // Fix the inline placeholder so the chat doesn't keep a stale
    // "Downloading…" message; the audio arrives in private chat instead.
    if let Some(inline_message_id) = result.inline_message_id.clone() {
        if let Err(error) = bot
            .edit_message_text_inline(inline_message_id, i18n::sent_to_private(lang))
            .await
        {
            warn!(%error, "could not edit inline placeholder to point at private chat");
        }
    }
    let Some(request) = state.selections.get(&result.result_id).await else {
        return;
    };
    let chat = ChatId(result.from.id.0 as i64);
    let user_id = result.from.id.0 as i64;
    info!("falling back to private delivery for inline result");
    let _ = deliver_track(state, bot, chat, request, quality, user_id).await;
}

fn format_duration(seconds: u32) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes}:{seconds:02}")
}
