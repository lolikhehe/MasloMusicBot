//! Message routing: commands, links, albums and text search.

use crate::bot::commands::{self, Command};
use crate::bot::send;
use crate::state::AppState;
use common::domain::{Lang, TrackKey, TrackMeta, TrackRequest, clean_genres};
use common::error::{AppError, DownloadError, HandlerResult};
use common::i18n;
use common::version::DEV_CHANNEL_LINE;
use downloader::PreparedAudio;
use providers::{ParsedInput, parse_input};
use std::borrow::Cow;
use std::time::Instant;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, Message, MessageEntityKind, MessageId, ParseMode,
};
use tracing::{debug, info, warn};

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
    // Playlist files: CSV/TXT/JSON exports from Spotify transfer tools.
    let document = msg.document().cloned();
    if let Some(document) = document {
        return crate::bot::import::handle_document(state, bot, msg, &document).await;
    }
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

    if !state.limiter.allow(user_id) {
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
    // routing decision rather than the text itself. Borrowed when there is no
    // hidden URL entity, which is the common case.
    let input = entity_url(&msg).map_or(Cow::Borrowed(text), Cow::Owned);
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
                user.tags_enabled,
                user.hq_cover,
                user.genre_analysis,
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

#[allow(clippy::too_many_arguments)]
async fn handle_track(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    lang: Lang,
    quality: u32,
    user_id: i64,
    request: TrackRequest,
    tags_enabled: bool,
    hq_cover: bool,
    genre_analysis: bool,
) -> HandlerResult {
    if try_cache_send(
        state,
        bot,
        chat,
        &request.key,
        quality,
        user_id,
        tags_enabled,
        genre_analysis,
    )
    .await?
    {
        return Ok(());
    }

    let placeholder = send::send_html(bot, chat, i18n::downloading(lang)).await?;
    match deliver_track(
        state,
        bot,
        chat,
        request,
        quality,
        user_id,
        tags_enabled,
        hq_cover,
        genre_analysis,
    )
    .await
    {
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
    ask_album_confirmation(state, bot, chat, progress.id, lang, &album_ref).await
}

/// Fetch a collection and ask the user to confirm the download. `progress` is
/// edited in place: either the "loading" placeholder (PM links) or the
/// `/alb` candidate list message.
pub async fn ask_album_confirmation(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    progress: MessageId,
    lang: Lang,
    album_ref: &common::domain::AlbumRef,
) -> HandlerResult {
    let album = match state.manager.resolve_album(album_ref).await {
        Ok(album) => album,
        Err(error) => {
            let _ = edit_html(bot, chat, progress, i18n::error(lang, &error)).await;
            return Ok(());
        }
    };
    if album.tracks.is_empty() {
        let _ = edit_html(bot, chat, progress, i18n::album_empty(lang)).await;
        return Ok(());
    }

    confirm_album(state, bot, chat, progress, lang, &album).await
}

/// Store a resolved collection and ask for confirmation. Shared by album links,
/// `/alb` picks and imported playlists.
pub async fn confirm_album(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    progress: MessageId,
    lang: Lang,
    album: &common::domain::Album,
) -> HandlerResult {
    tracing::info!(
        provider = %album.provider,
        source_url = album.source_url.as_deref().unwrap_or("<none>"),
        tracks = album.tracks.len(),
        "resolved collection; awaiting user confirmation"
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
    bot.edit_message_text(chat, progress, i18n::album_confirm(lang, &title, count))
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
    tags_enabled: bool,
    hq_cover: bool,
    genre_analysis: bool,
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

    // Hold a slot for the whole download+upload, not just the download part:
    // releasing the user semaphore before the Telegram upload let ~10 large
    // files land in one chat at once and triggered 429s.
    let send_slots = std::sync::Arc::new(tokio::sync::Semaphore::new(
        state.config.max_active_downloads_per_user,
    ));
    let mut set = tokio::task::JoinSet::new();
    for track in album.tracks {
        let state = state.clone();
        let bot_clone = bot.clone();
        let track_title = track
            .meta
            .as_ref()
            .map(|meta| meta.display_title())
            .unwrap_or_else(|| track.key.to_string());
        let send_slots = send_slots.clone();
        set.spawn(async move {
            let Ok(_permit) = send_slots.acquire().await else {
                return (
                    track_title,
                    Err(AppError::Message("album send semaphore closed".to_owned()).into()),
                );
            };
            let result = deliver_track(
                &state,
                &bot_clone,
                chat,
                track,
                quality,
                user_id,
                tags_enabled,
                hq_cover,
                genre_analysis,
            )
            .await;
            (track_title, result)
        });
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut done = 0usize;
    let mut failed_titles: Vec<String> = Vec::new();
    let mut last_edit = Instant::now();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((track_title, Ok(_))) => {
                let _ = track_title;
                ok += 1;
            }
            Ok((track_title, Err(error))) => {
                failed += 1;
                warn!(%track_title, %error, "album track delivery failed");
                if failed_titles.len() < 8 {
                    failed_titles.push(escape_html(&track_title));
                }
            }
            Err(error) => {
                failed += 1;
                warn!(%error, "album track task failed to join");
            }
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

    let mut final_text = i18n::album_done(lang, ok, failed);
    if !failed_titles.is_empty() {
        final_text.push_str("\n\n");
        final_text.push_str(&i18n::album_failed_list(
            lang,
            &failed_titles,
            failed.saturating_sub(failed_titles.len()),
        ));
    }
    let _ = edit_html(bot, chat, progress, final_text).await;
    Ok(())
}

/// Note appended to the caption when a FLAC render exceeded Telegram's size
/// limit and the track was re-sent as m4a 320 kbps. Language-neutral on
/// purpose (same style as the tags caption).
const FLAC_FALLBACK_NOTE: &str = "⚠ FLAC exceeded Telegram's 50 MB limit — sent as 320 kbps";

/// Download with one automatic downgrade: if quality 0 (FLAC) produces a file
/// above Telegram's 50 MB ceiling, retry once as 320 kbps and report it, so
/// the user always gets audio (and knows what they got). Returns the prepared
/// audio and whether the fallback happened.
pub async fn prepare_with_flac_fallback(
    state: &AppState,
    request: &TrackRequest,
    quality: u32,
    user_id: i64,
    hq_cover: bool,
) -> Result<(PreparedAudio, bool), DownloadError> {
    match state
        .manager
        .prepare(request.clone(), quality, user_id, hq_cover)
        .await
    {
        Ok(prepared) => Ok((prepared, false)),
        Err(error)
            if quality == 0 && matches!(error, common::error::DownloadError::TooLarge { .. }) =>
        {
            warn!(
                key = %request.key,
                "FLAC render too large for Telegram; resending as 320 kbps"
            );
            metrics::counter!("music_bot_flac_fallback_total").increment(1);
            let prepared = state
                .manager
                .prepare(request.clone(), 320, user_id, hq_cover)
                .await?;
            Ok((prepared, true))
        }
        Err(error) => Err(error),
    }
}

/// Append the FLAC-fallback note to an optional tags caption.
pub fn caption_with_flac_fallback(caption: Option<String>, fell_back: bool) -> Option<String> {
    match (fell_back, caption) {
        (false, caption) => caption,
        (true, Some(caption)) => Some(format!("{caption}\n{FLAC_FALLBACK_NOTE}")),
        (true, None) => Some(FLAC_FALLBACK_NOTE.to_owned()),
    }
}

/// Best-effort genre enrichment when the user opted into tags or the genre
/// analyzer (the latter needs genres to count them).
///
/// Last.fm is the primary source: a track's community tags are specific and
/// numerous (`thrash metal`, `groove metal`, ...) unlike the single coarse
/// `primaryGenreName` iTunes returns. iTunes stays as a fallback when there is
/// no Last.fm key, no match, or the lookup fails. The result is cached in memory
/// because this runs again on cache hits and album replays.
pub async fn enrich_tags(state: &AppState, meta: &mut TrackMeta, enabled: bool) {
    if !enabled || meta.genres.len() >= 8 {
        return;
    }
    let tags = fetch_genres(state, meta).await;
    merge_genres(meta, tags);
}

/// Fetch genres only (no mutation) so callers can overlap this HTTP work with
/// the audio download. Results are cached in memory for 24 hours.
pub async fn fetch_genres(state: &AppState, meta: &TrackMeta) -> Vec<String> {
    let clean = meta.without_artist_prefix();
    let cache_key = clean.search_query().to_lowercase();
    if let Some(cached) = state.genre_cache.get(&cache_key).await {
        return cached;
    }
    let mut tags: Vec<String> = Vec::new();
    let artist = clean.performer.as_deref().unwrap_or("");
    if let Some(lastfm) = &state.lastfm {
        match lastfm.track_tags(artist, &clean.title).await {
            Ok(found) => tags = found,
            Err(error) => debug!(%error, "last.fm genre lookup failed; trying iTunes"),
        }
    }
    if tags.is_empty()
        && let Some((genre, _album)) = state
            .manager
            .itunes_tags(&clean.search_query(), clean.duration_secs)
            .await
    {
        tags.push(genre);
    }
    let tags = clean_genres(tags, 8);
    state.genre_cache.insert(cache_key, tags.clone()).await;
    tags
}

/// Merge fetched genres into a track's metadata, keeping at most 8 unique tags.
pub fn merge_genres(meta: &mut TrackMeta, tags: Vec<String>) {
    for tag in tags {
        if !meta
            .genres
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&tag))
        {
            meta.genres.push(tag);
        }
    }
    meta.genres.truncate(8);
}

/// Caption for a sent track. The developer-channel line always goes out (that
/// is the point — every track carries it); the optional album/genre tags sit
/// above it. Telegram caps audio captions at 1024 characters, so long tags are
/// trimmed and a little room is left for the FLAC-fallback note.
pub fn tags_caption(meta: &TrackMeta, tags_enabled: bool) -> Option<String> {
    const MAX_CAPTION_CHARS: usize = 1024;
    const RESERVED_FOR_NOTES: usize = 80;
    let body = if tags_enabled {
        meta.tags_caption().unwrap_or_default()
    } else {
        String::new()
    };
    if body.is_empty() {
        return Some(DEV_CHANNEL_LINE.to_owned());
    }
    let max_body =
        MAX_CAPTION_CHARS.saturating_sub(DEV_CHANNEL_LINE.chars().count() + 1 + RESERVED_FOR_NOTES);
    let mut caption: String = body.chars().take(max_body).collect();
    caption.push('\n');
    caption.push_str(DEV_CHANNEL_LINE);
    Some(caption)
}

/// Serve a cache entry if possible. A stale `file_id` is purged so the caller
/// can fall back to a fresh download.
#[allow(clippy::too_many_arguments)]
async fn try_cache_send(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    key: &TrackKey,
    quality: u32,
    user_id: i64,
    tags_enabled: bool,
    genre_analysis: bool,
) -> Result<bool, BoxError> {
    let Some(cached) = state.manager.get_cached(key, quality).await? else {
        return Ok(false);
    };
    // Legacy cache entries (and sources with no genre info) may lack genres;
    // fill them on demand for the tags caption. The genre analyzer must not
    // slow the cache fast path, so analyzer-only enrichment happens after the
    // send in a background task.
    let mut caption_meta = cached.meta.clone();
    enrich_tags(state, &mut caption_meta, tags_enabled).await;
    let caption = tags_caption(&caption_meta, tags_enabled);
    match send::send_cached(bot, chat, &cached, caption).await {
        Ok(_) => {
            state.manager.touch_cached(key, quality).await;
            metrics::counter!("music_bot_cache_hits_total").increment(1);
            if genre_analysis {
                let track_key = key.to_string();
                if caption_meta.genres.is_empty() && !tags_enabled {
                    let state = state.clone();
                    let mut meta = cached.meta.clone();
                    tokio::spawn(async move {
                        enrich_tags(&state, &mut meta, true).await;
                        let _ = state
                            .db
                            .record_genres(user_id, &track_key, &meta.genres)
                            .await;
                    });
                } else if let Err(error) = state
                    .db
                    .record_genres(user_id, &track_key, &caption_meta.genres)
                    .await
                {
                    warn!(%error, key = %key, "could not record genres for cache hit");
                }
            }
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
#[allow(clippy::too_many_arguments)]
pub async fn deliver_track(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    request: TrackRequest,
    quality: u32,
    user_id: i64,
    tags_enabled: bool,
    hq_cover: bool,
    genre_analysis: bool,
) -> Result<Delivery, BoxError> {
    let enrich = tags_enabled || genre_analysis;
    if try_cache_send(
        state,
        bot,
        chat,
        &request.key,
        quality,
        user_id,
        tags_enabled,
        genre_analysis,
    )
    .await?
    {
        return Ok(Delivery::Cached);
    }

    metrics::counter!("music_bot_downloads_total").increment(1);
    let download_started = Instant::now();
    // Genre lookups are HTTP requests too; start them against the metadata we
    // already have from search so they overlap the (usually slower) audio
    // download instead of adding latency after it.
    let genre_task = if enrich
        && request
            .meta
            .as_ref()
            .is_some_and(|meta| meta.genres.len() < 8)
    {
        let state = state.clone();
        let seed = request.meta.clone().unwrap_or_default();
        Some(tokio::spawn(
            async move { fetch_genres(&state, &seed).await },
        ))
    } else {
        None
    };
    let (prepared, flac_fell_back) =
        match prepare_with_flac_fallback(state, &request, quality, user_id, hq_cover).await {
            Ok(value) => value,
            Err(error) => {
                metrics::counter!("music_bot_download_errors_total").increment(1);
                warn!(key = %request.key, %error, "track preparation failed");
                return Err(error.into());
            }
        };
    let mut prepared = prepared;
    match genre_task {
        Some(task) => match task.await {
            Ok(tags) => merge_genres(&mut prepared.meta, tags),
            Err(error) => warn!(%error, "parallel genre lookup failed"),
        },
        None => enrich_tags(state, &mut prepared.meta, enrich).await,
    }
    metrics::histogram!("music_bot_download_seconds")
        .record(download_started.elapsed().as_secs_f64());

    // Another request may have finished while we downloaded.
    if try_cache_send(
        state,
        bot,
        chat,
        &request.key,
        quality,
        user_id,
        tags_enabled,
        genre_analysis,
    )
    .await?
    {
        return Ok(Delivery::Cached);
    }

    metrics::counter!("music_bot_uploads_total").increment(1);
    let caption =
        caption_with_flac_fallback(tags_caption(&prepared.meta, tags_enabled), flac_fell_back);
    let upload_started = Instant::now();
    let message = match send::send_prepared(bot, chat, &prepared, caption).await {
        Ok(message) => message,
        Err(error) => {
            warn!(key = %request.key, %error, "track upload to Telegram failed");
            return Err(error.into());
        }
    };
    metrics::histogram!("music_bot_upload_seconds").record(upload_started.elapsed().as_secs_f64());

    let Some((file_id, file_unique_id)) = send::extract_file_id(&message) else {
        return Err(AppError::Message("Telegram response has no audio file id".to_owned()).into());
    };
    let size_bytes = tokio::fs::metadata(&prepared.audio_path)
        .await
        .ok()
        .map(|meta| meta.len() as i64);
    // Cache under the quality that was actually sent (FLAC fallback sends 320).
    let sent_quality = if flac_fell_back { 320 } else { quality };
    if let Err(error) = state
        .manager
        .record(
            &request,
            &prepared.meta,
            file_id,
            file_unique_id,
            sent_quality,
            size_bytes,
        )
        .await
    {
        warn!(%error, key = %request.key, "could not cache uploaded track");
    }
    if genre_analysis {
        let track_key = request.key.to_string();
        if let Err(error) = state
            .db
            .record_genres(user_id, &track_key, &prepared.meta.genres)
            .await
        {
            warn!(%error, key = %request.key, "could not record genres for upload");
        }
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

#[cfg(test)]
mod caption_tests {
    use super::tags_caption;
    use common::domain::TrackMeta;
    use common::version::DEV_CHANNEL_LINE;

    #[test]
    fn every_track_caption_carries_the_developer_channel() {
        let bare = TrackMeta::default();
        assert_eq!(
            tags_caption(&bare, false).as_deref(),
            Some(DEV_CHANNEL_LINE)
        );

        let tagged = TrackMeta {
            album: Some("Album".to_owned()),
            genres: vec!["Rock".to_owned()],
            ..Default::default()
        };
        let caption = tags_caption(&tagged, true).expect("caption");
        assert!(caption.contains("Album"), "tags must be kept: {caption}");
        assert!(
            caption.contains(DEV_CHANNEL_LINE),
            "channel missing: {caption}"
        );
    }

    #[test]
    fn caption_stays_within_telegram_audio_limit() {
        let meta = TrackMeta {
            album: Some("x".repeat(2000)),
            ..Default::default()
        };
        let caption = tags_caption(&meta, true).expect("caption");
        assert!(caption.chars().count() <= 1024);
        assert!(caption.ends_with(DEV_CHANNEL_LINE));
    }
}
