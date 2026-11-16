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

use crate::bot::handlers::{deliver_track, enrich_tags, tags_caption, truncate};
use crate::bot::send;
use crate::state::AppState;
use common::domain::{CachedTrack, DownloadTarget, Lang, Provider, TrackKey, TrackRequest};
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
    if !state.inline_limiter.allow(user_id).await {
        bot.answer_inline_query(query.id.clone(), Vec::new())
            .await?;
        return Ok(());
    }

    let text = query.query.trim();
    let short = text.chars().count() < 2;

    let generation = state.bump_inline_generation(user_id).await;
    let mut generation_rx = state.subscribe_inline_generation(user_id).await;

    // With fewer than 2 chars: empty answer — unless the user connected
    // Last.fm, in which case we propose the (likely) currently playing track
    // as the single result.
    let requests: Vec<TrackRequest> = if short {
        match lastfm_now_playing(&state, &user).await {
            Some(request) => vec![request],
            None => Vec::new(),
        }
    } else {
        tokio::select! {
            result = async {
                search_requests(&state, &user, text).await
            } => result,
            _ = generation_rx.changed() => {
                info!("inline query superseded; dropping in-flight search");
                return Ok(());
            }
        }
    };

    // If the user keeps typing, the next keystroke wakes this branch and the
    // in-flight search (a yt-dlp subprocess or an HTTP call) is dropped here
    // instead of running to completion and being discarded.
    // (Short-circuit: fewer than 2 chars is handled above, without dropping.)

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

        // Cached tracks answer instantly with the stored file_id — unless the
        // user opted into the album/genre tags caption, because CachedAudio
        // results can't carry a caption; those answers go out as an article so
        // the file_id edit shows the caption.
        if let Some(cached) = state.manager.get_cached(&request.key, user.quality).await?
            && !user.tags_enabled
        {
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

        let placeholder = if request.key.as_str().starts_with("lastfm:") {
            i18n::now_playing_inline(lang, &title)
        } else {
            i18n::downloading_track(lang, &title)
        };
        let content = InputMessageContent::Text(InputMessageContentText::new(placeholder));
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
    // Results are personalized by the user's /settings search source, so
    // server-side caching must be off: a 30 s shared cache meant the old
    // source kept being served after the user switched it.
    bot.answer_inline_query(query.id.clone(), results)
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
        let mut caption_meta = cached.meta.clone();
        enrich_tags(&state, &mut caption_meta, user.tags_enabled).await;
        let caption = tags_caption(&caption_meta, user.tags_enabled);
        match bot
            .edit_message_media_inline(inline_message_id, cached_media(&cached, caption))
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

    let mut prepared = prepared;
    enrich_tags(&state, &mut prepared.meta, user.tags_enabled).await;
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
    let caption = tags_caption(&prepared.meta, user.tags_enabled);
    let mut minted: Option<(String, String)> = None;
    for chat in upload_targets {
        match send::send_prepared(&bot, chat, &prepared, caption.clone()).await {
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
        fallback_private(&state, &bot, &result, user.quality, lang, user.tags_enabled).await;
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
        let media = file_id_media(&prepared, &file_id, caption.clone());
        if let Err(error) = bot
            .edit_message_media_inline(inline_message_id, media)
            .await
        {
            warn!(%error, "could not edit inline message with minted file_id");
        }
    }
    Ok(())
}

fn cached_media(cached: &CachedTrack, caption: Option<String>) -> InputMedia {
    let mut media = InputMediaAudio::new(InputFile::file_id(FileId(cached.file_id.clone())));
    media.title = Some(truncate(&cached.meta.title, 128));
    media.caption = caption;
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

fn file_id_media(
    prepared: &downloader::PreparedAudio,
    file_id: &str,
    caption: Option<String>,
) -> InputMedia {
    let mut media = InputMediaAudio::new(InputFile::file_id(FileId(file_id.to_owned())));
    media.title = Some(truncate(&prepared.meta.title, 128));
    media.caption = caption;
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
    tags_enabled: bool,
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
    let _ = deliver_track(state, bot, chat, request, quality, user_id, tags_enabled).await;
}

/// The search side of an inline query: parse, describe or search the user's
/// chosen source. Split out so the caller can race it against the next
/// keystroke (generation watch).
async fn search_requests(
    state: &AppState,
    user: &common::domain::UserRecord,
    text: &str,
) -> Vec<TrackRequest> {
    match parse_input(text) {
        ParsedInput::Track(request) => match state.manager.describe(request).await {
            Ok(request) => vec![request],
            Err(error) => {
                warn!(%error, "inline metadata resolution failed");
                Vec::new()
            }
        },
        ParsedInput::Search(query) => {
            match state.manager.search(&query, 10, user.search_source).await {
                Ok(results) => results
                    .into_iter()
                    .map(|result| result.into_request())
                    .collect(),
                Err(error) => {
                    warn!(%error, "inline search failed");
                    Vec::new()
                }
            }
        }
        ParsedInput::Album(_) => Vec::new(),
        ParsedInput::Unsupported(reason) => {
            info!(%reason, "unsupported inline input");
            Vec::new()
        }
    }
}

/// Track the user is (most likely) playing on Last.fm. `None` when Last.fm is
/// not connected/configured, nothing is playing, or the fetch failed (never
/// breaks the inline answer itself).
async fn lastfm_now_playing(
    state: &AppState,
    user: &common::domain::UserRecord,
) -> Option<TrackRequest> {
    let nick = user.lastfm_user.as_deref()?;
    let client = state.lastfm.as_ref()?;
    match client.now_playing(nick).await {
        Ok(Some(now)) => {
            let artist = now.meta.performer.clone().unwrap_or_default();
            let query = if artist.is_empty() {
                now.meta.title.clone()
            } else {
                format!("{artist} {}", now.meta.title)
            };
            Some(TrackRequest {
                key: TrackKey::new(format!("lastfm:{}", slugify(&artist, &now.meta.title))),
                provider: Provider::YouTube,
                source_url: now.url,
                meta: Some(now.meta),
                target: DownloadTarget::Search(query),
            })
        }
        Ok(None) => None,
        Err(error) => {
            warn!(%error, nick, "lastfm now playing fetch failed");
            None
        }
    }
}

/// Deterministic, filename-safe-ish slug used for the Last.fm cache key so
/// the same track maps to one cached entry across users and restarts.
fn slugify(artist: &str, title: &str) -> String {
    let mut out = String::with_capacity(artist.len() + title.len() + 1);
    let mut last_dash = true; // no leading dash
    for c in format!("{artist} {title}").chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn format_duration(seconds: u32) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes}:{seconds:02}")
}
