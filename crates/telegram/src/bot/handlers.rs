//! Message routing: commands, links, albums and text search.

use crate::bot::commands::{self, Command};
use crate::bot::send;
use crate::state::AppState;
use common::domain::{
    Lang, SearchPage, SearchResult, TrackKey, TrackMeta, TrackRequest, clean_genres,
};
use common::error::{AppError, DownloadError, HandlerResult};
use common::i18n;
use common::version::DEV_CHANNEL_LINE;
use downloader::PreparedAudio;
use providers::{ParsedInput, parse_input};
use std::borrow::Cow;
use std::time::{Duration, Instant};
use teloxide::prelude::*;
use teloxide::types::{
    CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, Message,
    MessageEntityKind, MessageId, ParseMode,
};
use tracing::{debug, info, warn};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Request counter + latency histogram for every message the bot handles.
///
/// Commands in private chats used to return from `handle_message` before the
/// old counter/histogram lines, so a load scenario made of `/help`, `/me` and
/// `/lyrics` produced an empty `/metrics` scrape. The `Drop` guard records the
/// latency on *all* exit paths, including early command returns and documents.
struct RequestMetrics {
    started: Instant,
    counted: bool,
}

impl RequestMetrics {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            counted: false,
        }
    }

    fn count(&mut self) {
        if !self.counted {
            metrics::counter!("music_bot_requests_total").increment(1);
            self.counted = true;
        }
    }
}

impl Drop for RequestMetrics {
    fn drop(&mut self) {
        if self.counted {
            metrics::histogram!("music_bot_request_seconds")
                .record(self.started.elapsed().as_secs_f64());
        }
    }
}

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
    let mut request_metrics = RequestMetrics::new();

    // Playlist files: CSV/TXT/JSON exports from Spotify transfer tools.
    let document = msg.document().cloned();
    if let Some(document) = document {
        request_metrics.count();
        // A bare `/import` was answered with a file. Consume the one-shot marker
        // so a later unrelated message is not mistaken for playlist content.
        if let Some(user) = msg.from.as_ref() {
            let _ = state.take_pending_import(user.id.0 as i64).await?;
        }
        return crate::bot::import::handle_document(state, bot, msg, &document).await;
    }
    let Some(text) = msg.text() else {
        return Ok(());
    };
    request_metrics.count();

    if text.trim_start().starts_with('/') {
        // Any command cancels a waiting `/import`; `/import` re-arms it below.
        if let Some(user) = msg.from.as_ref() {
            let _ = state.take_pending_import(user.id.0 as i64).await?;
        }
        // An admin must be able to enable the group before it is allowed.
        let groupmode = text
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("/groupmode");
        if !msg.chat.is_private() && !groupmode && !state.chat_allowed(msg.chat.id.0).await? {
            let lang = common::domain::Lang::from_telegram_hint(
                msg.from
                    .as_ref()
                    .and_then(|user| user.language_code.as_deref()),
                state.config.default_lang,
            );
            send::send_html(&bot, msg.chat.id, i18n::private_only(lang)).await?;
            return Ok(());
        }
        return match commands::parse_command(text, &state.bot_username) {
            Some(Command::Unknown(_)) => {
                commands::handle_command(&state, &bot, &msg, Command::Help).await
            }
            Some(command) => commands::handle_command(&state, &bot, &msg, command).await,
            None => Ok(()),
        };
    }

    if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
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

    if !state.limiter.allow(user_id).await? {
        send::send_html(
            &bot,
            msg.chat.id,
            i18n::rate_limited(lang, state.config.rate_limit_per_min),
        )
        .await?;
        return Ok(());
    }

    // A bare `/import` asked for the playlist in the next message. Plain text
    // here is `Artist – Title` lines, not a search query or a track link.
    if state.take_pending_import(user_id).await? {
        return crate::bot::import::handle_import_text(&state, &bot, &msg, text, lang).await;
    }

    // Spec §10: user texts must not be logged beyond debugging, so we log the
    // routing decision rather than the text itself. Borrowed when there is no
    // hidden URL entity, which is the common case.
    let input = entity_url(&msg).map_or(Cow::Borrowed(text), Cow::Owned);
    info!(user_id, "handling input");

    match parse_input(&input) {
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
            handle_album(
                &state,
                &bot,
                msg.chat.id,
                lang,
                user.quality,
                album,
                user_id,
            )
            .await
        }
        ParsedInput::Search(query) => {
            let query = query.trim();
            if query.is_empty() {
                send::send_html(&bot, msg.chat.id, i18n::no_results(lang)).await?;
                Ok(())
            } else {
                handle_search(
                    &state,
                    &bot,
                    msg.chat.id,
                    lang,
                    query,
                    user.search_source,
                    user_id,
                )
                .await
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
    }
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

    let job = match crate::bot::jobs::JobGuard::start(
        state,
        user_id,
        "track",
        serde_json::json!({
            "key": request.key.as_str(),
            "title": request.meta.as_ref().map(|meta| meta.display_title()),
        }),
    )
    .await?
    {
        Some(job) => job,
        None => {
            send::send_html(bot, chat, i18n::job_already_running(lang)).await?;
            return Ok(());
        }
    };

    let placeholder = send::send_html(bot, chat, i18n::downloading(lang)).await?;
    let outcome = deliver_track_inner(
        state,
        bot,
        chat,
        request,
        quality,
        user_id,
        tags_enabled,
        hq_cover,
        genre_analysis,
        Some(&job),
    )
    .await;
    match &outcome {
        Ok(_) => {
            job.finish("done", None).await;
            if let Err(error) = bot.delete_message(chat, placeholder.id).await {
                warn!(%error, "could not delete progress message");
            }
        }
        Err(error) => {
            if job.cancelled().await {
                job.finish("cancelled", None).await;
                let _ = edit_html(bot, chat, placeholder.id, i18n::job_cancelled(lang)).await;
            } else {
                job.finish("failed", Some(&error.to_string())).await;
                let text = error_text(lang, error);
                let _ = edit_html(bot, chat, placeholder.id, text).await;
                warn!(%error, "track delivery failed");
            }
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
    user_id: i64,
) -> HandlerResult {
    let query: String = query.chars().take(200).collect();
    let progress = send::send_html(bot, chat, i18n::searching(lang)).await?;
    // The ninth result is only a sentinel: it decides whether the
    // "more results" button appears.
    let results = match state.manager.search(&query, 9, search_source).await {
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

    let has_more = results.len() > 8;
    let page: Vec<SearchResult> = results.into_iter().take(8).collect();
    let result_count = page.len();
    let keys: Vec<String> = page.iter().map(|result| result.key.to_string()).collect();
    let heard = state
        .db
        .heard_before(user_id, &keys)
        .await
        .unwrap_or_default();
    let mut rows = search_result_rows(state, user_id, &page, &heard, 0).await?;
    if has_more {
        let selection_id = state.new_selection_id();
        state
            .put_selection(
                &selection_id,
                crate::state::selection::SEARCH_PAGE,
                user_id,
                &SearchPage {
                    query: query.clone(),
                    source: search_source,
                    // pg: buttons carry the offset themselves; stays 0.
                    offset: 0,
                },
            )
            .await?;
        // One selection backs every page; symbols skip any language.
        rows.push(vec![InlineKeyboardButton::callback(
            "▶",
            format!("pg:{selection_id}:8"),
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

/// One row of search-result buttons; the `q:` selection re-delivers the track
/// through the normal pipeline. `↩` marks tracks the user already downloaded.
pub async fn search_result_rows(
    state: &AppState,
    user_id: i64,
    results: &[SearchResult],
    heard: &std::collections::HashSet<String>,
    start_index: usize,
) -> Result<Vec<Vec<InlineKeyboardButton>>, common::error::AppError> {
    let requests: Vec<TrackRequest> = results
        .iter()
        .map(|result| result.clone().into_request())
        .collect();
    let mut rows = Vec::with_capacity(results.len());
    let mut selections: Vec<(String, &TrackRequest)> = Vec::with_capacity(results.len());
    for (offset, (result, request)) in results.iter().zip(&requests).enumerate() {
        let selection_id = state.new_selection_id();
        let title = result.meta.display_title();
        let mut button_text = format!("{}. {}", start_index + offset + 1, truncate(&title, 56));
        if heard.contains(result.key.as_str()) {
            button_text.push_str(" · ↩");
        }
        selections.push((selection_id.clone(), request));
        rows.push(vec![InlineKeyboardButton::callback(
            button_text,
            format!("q:{selection_id}"),
        )]);
    }
    state
        .put_selections(crate::state::selection::TRACK, user_id, &selections)
        .await?;
    Ok(rows)
}

/// Chat search page turning (`pg:<selection>:<offset>`). Re-runs the same
/// provider search and slices the requested window; the ninth result stays
/// the sentinel that decides whether ▶ appears. `target: None` is the legacy
/// pre-2.6.18 `more:` button, whose offset lives in the selection.
pub async fn handle_search_page(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    target: Option<u32>,
    lang: Lang,
    user_id: i64,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let Some(page) = state
        .get_selection::<SearchPage>(selection_id, crate::state::selection::SEARCH_PAGE)
        .await?
    else {
        let _ = edit_html(
            bot,
            message.chat.id,
            message.id,
            i18n::selection_expired(lang),
        )
        .await;
        return Ok(());
    };
    let offset = target.unwrap_or(page.offset) as usize;
    let results = match state
        .manager
        .search(&page.query, offset + 9, page.source)
        .await
    {
        Ok(results) => results,
        Err(error) => {
            let _ = edit_html(bot, message.chat.id, message.id, i18n::error(lang, &error)).await;
            return Ok(());
        }
    };
    let total = results.len();
    let slice: Vec<SearchResult> = results.into_iter().skip(offset).take(8).collect();
    if slice.is_empty() {
        if offset > 0 {
            // Stale ▶ tap: keep the current menu instead of wiping it.
            return Ok(());
        }
        let _ = edit_html(bot, message.chat.id, message.id, i18n::no_results(lang)).await;
        return Ok(());
    }
    let keys: Vec<String> = slice.iter().map(|result| result.key.to_string()).collect();
    let heard = state
        .db
        .heard_before(user_id, &keys)
        .await
        .unwrap_or_default();
    let mut rows = search_result_rows(state, user_id, &slice, &heard, offset).await?;
    let mut arrows: Vec<InlineKeyboardButton> = Vec::new();
    if offset > 0 {
        arrows.push(InlineKeyboardButton::callback(
            "◀",
            format!("pg:{selection_id}:{}", offset.saturating_sub(8)),
        ));
    }
    if total > offset + 8 {
        arrows.push(InlineKeyboardButton::callback(
            "▶",
            format!("pg:{selection_id}:{}", offset + 8),
        ));
    }
    if !arrows.is_empty() {
        rows.push(arrows);
    }
    let header = i18n::search_results(lang, &escape_html(&page.query), slice.len());
    let _ = bot
        .edit_message_text(message.chat.id, message.id, header)
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await;
    Ok(())
}

/// The ⭐/☆ toggle row under a delivered track. Symbol-only so the hot
/// delivery path needs no language lookup; the toggle toast is localized.
pub async fn fav_markup_for(
    state: &AppState,
    user_id: i64,
    key: &str,
) -> Option<InlineKeyboardMarkup> {
    // Telegram caps callback_data at 64 bytes; a longer `fav:` key makes the
    // ENTIRE audio send fail with BUTTON_DATA_INVALID.
    if format!("fav:{key}").len() > 64 {
        return None;
    }
    // A transient DB error must not render ☆ for a favorited track: one tap
    // would then silently DELETE the favorite. Skip the button instead.
    let Ok(added) = state.db.is_favorite(user_id, key).await else {
        return None;
    };
    Some(InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(if added { "⭐" } else { "☆" }, format!("fav:{key}")),
    ]]))
}

/// Largest cover URL the CDN can serve for this artwork. Apple stores
/// 600x600 and serves the same asset at any size; Deezer is already
/// 1000px (`cover_xl`).
pub fn hq_cover_url(url: &str) -> String {
    url.replace("600x600bb", "1000x1000bb")
        .replace("100x100bb", "1000x1000bb")
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
    user_id: i64,
) -> HandlerResult {
    let progress = send::send_html(bot, chat, i18n::album_loading(lang)).await?;
    ask_album_confirmation(state, bot, chat, progress.id, lang, &album_ref, user_id).await
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
    user_id: i64,
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

    confirm_album(state, bot, chat, progress, lang, &album, user_id).await
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
    user_id: i64,
) -> HandlerResult {
    tracing::info!(
        provider = %album.provider,
        source_url = album.source_url.as_deref().unwrap_or("<none>"),
        tracks = album.tracks.len(),
        "resolved collection; awaiting user confirmation"
    );

    let selection_id = state.new_selection_id();
    let count = album.tracks.len();
    if let Err(error) = state
        .put_selection(
            &selection_id,
            crate::state::selection::ALBUM,
            user_id,
            album,
        )
        .await
    {
        warn!(%error, "could not store album confirmation");
        let _ = edit_html(
            bot,
            chat,
            progress,
            i18n::error(
                lang,
                &DownloadError::Internal("could not prepare the album".to_owned()),
            ),
        )
        .await;
        return Ok(());
    }
    let title = escape_html(&album.title);
    let mut first_row = vec![InlineKeyboardButton::callback(
        i18n::album_download_all(lang).to_owned(),
        format!("alc:{selection_id}"),
    )];
    // Offer the ZIP path next to "download all", reusing the same identity and
    // 64-byte guard as the delivered-album summary row.
    if let Some((source, id)) = album_identity(album) {
        let zip = format!("azp:{source}:{id}");
        if zip.len() <= 64 {
            first_row.push(InlineKeyboardButton::callback(
                i18n::download_archive(lang).to_owned(),
                zip,
            ));
        }
    }
    let keyboard = InlineKeyboardMarkup::new(vec![
        first_row,
        vec![InlineKeyboardButton::callback(
            i18n::album_cancel(lang).to_owned(),
            format!("alx:{selection_id}"),
        )],
    ]);
    if let Err(error) = bot
        .edit_message_text(chat, progress, i18n::album_confirm(lang, &title, count))
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await
    {
        warn!(%error, "could not render album confirmation");
        let _ = edit_html(
            bot,
            chat,
            progress,
            i18n::error(
                lang,
                &DownloadError::Internal("could not render the album".to_owned()),
            ),
        )
        .await;
    }
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
    album_info: bool,
    job: Option<&crate::bot::jobs::JobGuard>,
) -> Result<usize, BoxError> {
    let total = album.tracks.len();
    let title = escape_html(&album.title);
    // Keep the album's identity before `album.tracks` is consumed below; the
    // save/ZIP callbacks carry it, and over-long ids are dropped per button.
    let album_identity = album_identity(&album);
    let base = i18n::album_started(lang, &title, total);
    // Download-only ETA from the live measured speed, divided by the number of
    // tracks the send semaphore actually runs at once. Unknown durations borrow
    // the mean of the known ones; with none known we stay silent (no guess).
    let known: Vec<f64> = album
        .tracks
        .iter()
        .filter_map(|track| {
            track
                .meta
                .as_ref()
                .and_then(|meta| meta.duration_secs)
                .and_then(|duration| state.manager.estimate_download_secs(duration, quality))
        })
        .collect();
    let parallelism = state.config.max_active_downloads_per_user.min(total).max(1);
    let estimate = if known.is_empty() {
        None
    } else {
        let sum: f64 = known.iter().sum();
        let mean = sum / known.len() as f64;
        let total_work = sum + mean * (total - known.len()) as f64;
        Some(total_work / parallelism as f64)
    };
    if let Some(secs) = estimate {
        let eta = i18n::download_eta(lang, secs);
        let _ = edit_html(bot, chat, progress, format!("{base} {eta}")).await;
    } else {
        let _ = edit_html(bot, chat, progress, base).await;
    }
    let started = Instant::now();

    // The album-info toggle sends the cover and facts before the first track.
    if album_info && let Some(meta) = album.tracks.first().and_then(|track| track.meta.as_ref()) {
        send_album_info(state, bot, chat, &album, meta).await;
    }

    // Hold a slot for the whole download+upload, not just the download part:
    // releasing the user semaphore before the Telegram upload let ~10 large
    // files land in one chat at once and triggered 429s.
    let send_slots = std::sync::Arc::new(tokio::sync::Semaphore::new(
        state.config.max_active_downloads_per_user,
    ));
    // Owned clone so each spawned task can carry the guard into its delivery
    // path; cancellation must reach in-flight tracks, not just the spawn loop.
    let job = job.cloned();
    let mut set = tokio::task::JoinSet::new();
    for track in album.tracks {
        if let Some(job) = &job
            && job.cancelled().await
        {
            break;
        }
        let state = state.clone();
        let bot_clone = bot.clone();
        let track_title = track
            .meta
            .as_ref()
            .map(|meta| meta.display_title())
            .unwrap_or_else(|| track.key.to_string());
        let send_slots = send_slots.clone();
        let job_clone = job.clone();
        set.spawn(async move {
            let Ok(_permit) = send_slots.acquire().await else {
                return (
                    track_title,
                    Err(AppError::Message("album send semaphore closed".to_owned()).into()),
                );
            };
            // Cancellation may have landed while this track waited for a slot.
            if let Some(job) = &job_clone
                && job.cancelled().await
            {
                return (
                    track_title,
                    Err(crate::bot::jobs::JobGuard::cancelled_error().into()),
                );
            }
            let result = deliver_track_inner(
                &state,
                &bot_clone,
                chat,
                track,
                quality,
                user_id,
                tags_enabled,
                hq_cover,
                genre_analysis,
                job_clone.as_ref(),
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
                    failed_titles.push(truncate_escaped_utf16(&escape_html(&track_title), 80));
                }
            }
            Err(error) => {
                failed += 1;
                warn!(%error, "album track task failed to join");
            }
        }
        done += 1;
        if last_edit.elapsed() >= std::time::Duration::from_secs(2) || done == total {
            let mut progress_text = i18n::album_progress(lang, done, total, &title);
            if let Some(remaining) =
                empirical_remaining_secs(total, done, started.elapsed().as_secs_f64())
            {
                progress_text.push_str(" · ");
                progress_text.push_str(&i18n::download_eta(lang, remaining));
            }
            let _ = edit_html(bot, chat, progress, progress_text).await;
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
    // 💾 / 🗜 under the album summary. Each callback is checked against
    // Telegram's 64-byte limit before the button is built at all.
    let mut extra_rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    if let Some((source, id)) = album_identity {
        let mut buttons = Vec::new();
        let save = format!("als:{source}:{id}");
        if save.len() <= 64 {
            buttons.push(InlineKeyboardButton::callback(
                i18n::albums_save_button(lang).to_owned(),
                save,
            ));
        }
        let zip = format!("azp:{source}:{id}");
        if zip.len() <= 64 {
            buttons.push(InlineKeyboardButton::callback("🗜 ZIP", zip));
        }
        if !buttons.is_empty() {
            extra_rows.push(buttons);
        }
    }
    if extra_rows.is_empty() {
        let _ = edit_html(bot, chat, progress, final_text).await;
    } else {
        let _ = bot
            .edit_message_text(chat, progress, final_text)
            .parse_mode(ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(extra_rows))
            .await;
    }
    // The caller marks the job "done" only if at least one track landed: an
    // album where every track failed must be recorded as "failed".
    Ok(ok)
}

/// Identity used by the save/ZIP buttons. `id` is the album's full source URL:
/// Deezer/Apple/YouTube ids fit the 64-byte callback cap, SoundCloud/Bandcamp
/// URLs do not and are skipped when building the button.
fn album_identity(album: &common::domain::Album) -> Option<(String, String)> {
    let id = album.source_url.as_deref()?.trim();
    (!id.is_empty()).then(|| (album.provider.as_str().to_owned(), id.to_owned()))
}

/// Remaining seconds from the pace of already-finished tracks: `(left) ×
/// (elapsed / done)`. `None` before the first track lands (or once all did),
/// so an album never shows an invented countdown.
fn empirical_remaining_secs(total: usize, done: usize, elapsed_secs: f64) -> Option<f64> {
    if done > 0 && done < total {
        Some((total - done) as f64 * elapsed_secs / done as f64)
    } else {
        None
    }
}

/// Cover photo + caption sent before an album's tracks (the
/// "ℹ️ Album & artist info" toggle in /settings). Best-effort: every source
/// is optional, a failed fetch only drops its line or the photo.
async fn send_album_info(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    album: &common::domain::Album,
    meta: &TrackMeta,
) {
    let performer = meta
        .performer
        .as_deref()
        .map(str::trim)
        .filter(|artist| !artist.is_empty());
    let facts = state.manager.album_facts(album).await;
    // The album-level name from the catalog is fresher than a track's performer.
    let artist = facts
        .as_ref()
        .and_then(|facts| facts.artist.as_deref())
        .map(str::trim)
        .filter(|artist| !artist.is_empty())
        .or(performer);
    let Some(artist) = artist else {
        return;
    };
    // Long-form text goes in separate messages: photo captions stop at 1024
    // characters, plain messages at 4096.
    let (bio, wiki) = match &state.lastfm {
        Some(client) => {
            let bio = client.artist_bio(artist).await.ok().flatten();
            let wiki = client.album_wiki(artist, &album.title).await.ok().flatten();
            (bio, wiki)
        }
        None => (None, None),
    };
    let caption = album_info_caption(&album.title, artist, facts.as_ref());
    match meta
        .thumbnail_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        Some(url) => {
            // Apple artwork is stored at 600x600; the CDN serves the same asset
            // at any size. Deezer thumbnails are already 1000px (`cover_xl`).
            let url = hq_cover_url(url);
            match providers::http::fetch_bytes(&state.http, &url, 8 * 1024 * 1024).await {
                Some(bytes) => {
                    let _ = bot
                        .send_photo(chat, InputFile::memory(bytes))
                        .caption(caption)
                        .parse_mode(ParseMode::Html)
                        .await;
                }
                None => {
                    let _ = send::send_html(bot, chat, caption).await;
                }
            }
        }
        None => {
            let _ = send::send_html(bot, chat, caption).await;
        }
    }
    send_info_sections(
        bot,
        chat,
        artist,
        &album.title,
        bio.as_deref(),
        wiki.as_deref(),
    )
    .await;
}

/// Artist bio and album wiki as their own messages, so neither is cut short by
/// the 1024-character caption limit on the cover photo.
async fn send_info_sections(
    bot: &Bot,
    chat: ChatId,
    artist: &str,
    title: &str,
    bio: Option<&str>,
    wiki: Option<&str>,
) {
    if let Some(bio) = bio {
        let _ = send::send_html(bot, chat, info_section("👤", artist, bio)).await;
    }
    if let Some(wiki) = wiki {
        let _ = send::send_html(bot, chat, info_section("💿", title, wiki)).await;
    }
}

/// One long-form info section. Telegram's text limit applies per message, so
/// each section is sent separately; an over-long source is cut at 2040 UTF-16
/// units (astral emoji count double) to stay safely inside the 4096 cap.
fn info_section(icon: &str, name: &str, body: &str) -> String {
    let short = truncate_utf16(body.trim(), 2040);
    format!(
        "{icon} <b>{}</b>\n\n{}",
        escape_html(name.trim()),
        escape_html(short.trim_end())
    )
}

/// Caption for the album-info photo. Deliberately language-neutral (symbols,
/// names, numbers) so it needs no per-locale strings; the long-form bio and
/// album wiki are sent as separate messages instead of being squeezed into
/// the 1024-character caption limit.
fn album_info_caption(title: &str, artist: &str, facts: Option<&providers::AlbumFacts>) -> String {
    let title = truncate_utf16(title.trim(), 400);
    let artist = truncate_utf16(artist.trim(), 400);
    let mut header = format!(
        "💿 <b>{}</b>\n🎤 {}",
        escape_html(&title),
        escape_html(&artist)
    );
    if let Some(facts) = facts {
        let mut parts: Vec<String> = Vec::new();
        if let Some(year) = facts
            .release_year
            .as_deref()
            .map(str::trim)
            .filter(|year| !year.is_empty())
        {
            parts.push(escape_html(year));
        }
        if let Some(label) = facts
            .label
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
        {
            parts.push(escape_html(&truncate_utf16(label, 60)));
        }
        let genres = facts
            .genres
            .iter()
            .take(3)
            .map(|genre| escape_html(genre.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        if !genres.is_empty() {
            parts.push(genres);
        }
        if !parts.is_empty() {
            let line = parts.join(" · ");
            header.push_str("\n📅 ");
            header.push_str(&line);
        }
    }
    // Hard cap on the whole caption: Telegram's 1024 limit counts UTF-16 units.
    truncate_escaped_utf16(&header, 1000)
}

#[cfg(test)]
mod album_info_tests {
    use super::*;

    #[test]
    fn album_info_caption_carries_facts() {
        let facts = providers::AlbumFacts {
            release_year: Some("2018".to_owned()),
            label: Some("Star Records".to_owned()),
            genres: vec!["Rock".to_owned(), "Pop".to_owned()],
            ..Default::default()
        };
        let caption = album_info_caption("Night <Drive>", "Testo & Band", Some(&facts));
        assert!(caption.contains("💿 <b>Night &lt;Drive&gt;</b>"));
        assert!(caption.contains("🎤 Testo &amp; Band"));
        assert!(caption.contains("📅 2018 · Star Records · Rock, Pop"));
    }

    #[test]
    fn album_info_caption_without_extras() {
        let bare = album_info_caption("Title", "Artist", None);
        assert_eq!(bare, "💿 <b>Title</b>\n🎤 Artist");
    }

    #[test]
    fn info_section_stays_within_message_limit() {
        let long = "word ".repeat(3000);
        let text = info_section("👤", "Artist <&>", &long);
        assert!(
            text.encode_utf16().count() <= 4096,
            "section message under Telegram's UTF-16 text cap"
        );
        assert!(text.starts_with("👤 <b>Artist &lt;&amp;&gt;</b>\n\n"));
        let text = info_section("💿", "Album", "Short story.");
        assert!(text.ends_with("Short story."));
    }
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
/// above it. Telegram caps audio captions at 1024 UTF-16 units, so long tags are
/// trimmed by that measure and room is left for the FLAC-fallback note.
pub fn tags_caption(meta: &TrackMeta, tags_enabled: bool) -> Option<String> {
    const MAX_CAPTION_UNITS: usize = 1000;
    const RESERVED_FOR_NOTES: usize = 80;
    let body = if tags_enabled {
        meta.tags_caption().unwrap_or_default()
    } else {
        String::new()
    };
    if body.is_empty() {
        return Some(DEV_CHANNEL_LINE.to_owned());
    }
    let dev_units = DEV_CHANNEL_LINE.encode_utf16().count();
    let max_body = MAX_CAPTION_UNITS.saturating_sub(dev_units + 1 + RESERVED_FOR_NOTES);
    let mut caption = truncate_utf16(&body, max_body);
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
    // Genre enrichment (Last.fm up to ~6 s + iTunes ~5 s) must not sit on the
    // cache-hit fast path. Build the caption from cached metadata as-is, send,
    // then enrich in the detached task below and edit the caption only if the
    // tags actually changed it. Analyzer-only enrichment stays in that task.
    let caption_meta = cached.meta.clone();
    let caption = tags_caption(&caption_meta, tags_enabled);
    let fav_markup = fav_markup_for(state, user_id, key.as_str()).await;
    match send::send_cached(bot, chat, &cached, caption.clone(), fav_markup.clone()).await {
        Ok(message) => {
            state.maybe_track_deletion(user_id, chat, message.id);
            state.manager.touch_cached(key, quality).await;
            state
                .remember_track(user_id, &caption_meta, key, cached.provider)
                .await;
            // The audio is already with Telegram. All remaining bookkeeping is
            // best-effort metadata: do NOT add two Postgres round trips to the
            // cache-hit (S1) path. One detached task handles enrichment,
            // listen + genres; losing it on a crash only costs a history row,
            // never a download (or an enriched caption).
            metrics::counter!("music_bot_cache_hits_total").increment(1);
            let record_state = state.clone();
            let record_bot = bot.clone();
            let record_key = key.to_string();
            let mut record_meta = caption_meta.clone();
            let record_markup = fav_markup;
            let record_message_id = message.id;
            tokio::spawn(async move {
                // Enrichment that used to be awaited before the send now runs
                // here. With tags on it feeds the caption edit; for the genre
                // analyzer it supplies the genres to record.
                if (tags_enabled || (genre_analysis && record_meta.genres.is_empty()))
                    && record_meta.genres.len() < 8
                {
                    enrich_tags(&record_state, &mut record_meta, true).await;
                }
                if tags_enabled {
                    let enriched_caption = tags_caption(&record_meta, tags_enabled);
                    if enriched_caption != caption
                        && let Some(enriched_text) = enriched_caption
                    {
                        // Rebuild the ⭐ row fresh: the user may have tapped it
                        // since the send, and re-passing the send-time clone
                        // would revert their choice.
                        let fresh_markup =
                            fav_markup_for(&record_state, user_id, &record_key).await;
                        // Editing without reply_markup wipes existing buttons,
                        // so skip the edit only when the fresh fetch failed but
                        // the send actually had buttons to lose.
                        if !matches!((&fresh_markup, &record_markup), (None, Some(_))) {
                            let mut edit = record_bot
                                .edit_message_caption(chat, record_message_id)
                                .caption(enriched_text);
                            if let Some(markup) = fresh_markup {
                                edit = edit.reply_markup(markup);
                            }
                            if let Err(error) = edit.await {
                                warn!(%error, key = %record_key, "could not enrich cached track caption");
                            }
                        }
                    }
                }
                let genres = record_meta.genres.clone();
                if let Err(error) = record_state
                    .db
                    .record_listen(
                        user_id,
                        &record_key,
                        &record_meta.title,
                        record_meta.performer.as_deref(),
                        &genres,
                    )
                    .await
                {
                    warn!(%error, key = %record_key, "could not record listen for cache hit");
                }
                if genre_analysis
                    && let Err(error) = record_state
                        .db
                        .record_genres(user_id, &record_key, &genres)
                        .await
                {
                    warn!(%error, key = %record_key, "could not record genres for cache hit");
                }
            });
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
async fn deliver_track_inner(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    request: TrackRequest,
    quality: u32,
    user_id: i64,
    tags_enabled: bool,
    hq_cover: bool,
    genre_analysis: bool,
    job: Option<&crate::bot::jobs::JobGuard>,
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

    if let Some(job) = job
        && job.cancelled().await
    {
        return Err(crate::bot::jobs::JobGuard::cancelled_error().into());
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

    if let Some(job) = job
        && job.cancelled().await
    {
        return Err(crate::bot::jobs::JobGuard::cancelled_error().into());
    }

    if let Ok((format, normalize)) = state.db.audio_prefs(user_id).await {
        downloader::postprocess_audio(&mut prepared, &format, normalize, &state.config.ffmpeg_path)
            .await;
    }

    metrics::counter!("music_bot_uploads_total").increment(1);
    let caption =
        caption_with_flac_fallback(tags_caption(&prepared.meta, tags_enabled), flac_fell_back);
    let upload_started = Instant::now();
    let fav_markup = fav_markup_for(state, user_id, request.key.as_str()).await;
    let message = match send::send_prepared(bot, chat, &prepared, caption, fav_markup).await {
        Ok(message) => message,
        Err(error) => {
            warn!(key = %request.key, %error, "track upload to Telegram failed");
            return Err(error.into());
        }
    };
    state.maybe_track_deletion(user_id, chat, message.id);
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
    state
        .remember_track(user_id, &prepared.meta, &request.key, request.provider)
        .await;
    // Same reasoning as the cache-hit path: the upload already succeeded, so
    // history bookkeeping must not delay the handler's response.
    let record_state = state.clone();
    let record_key = request.key.to_string();
    let record_meta = prepared.meta.clone();
    tokio::spawn(async move {
        if let Err(error) = record_state
            .db
            .record_listen(
                user_id,
                &record_key,
                &record_meta.title,
                record_meta.performer.as_deref(),
                &record_meta.genres,
            )
            .await
        {
            warn!(%error, key = %record_key, "could not record listen for upload");
        }
    });
    Ok(Delivery::Uploaded)
}

/// Delivery without a persistent job (callbacks and inline results).
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
    deliver_track_inner(
        state,
        bot,
        chat,
        request,
        quality,
        user_id,
        tags_enabled,
        hq_cover,
        genre_analysis,
        None,
    )
    .await
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

/// Adaptive tick gap: 15s while the estimate is far out, 5s near the end.
/// Sub-5s ticks would hit Telegram's edit flood limit.
fn countdown_interval(remaining_secs: u64) -> u64 {
    if remaining_secs > 120 { 15 } else { 5 }
}

/// Re-edit a download placeholder with a shrinking ETA. The caller aborts the
/// returned handle before rendering the final result.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_download_countdown(
    bot: &Bot,
    chat: ChatId,
    msg_id: MessageId,
    base_text: String,
    total_secs: u64,
    lang: Lang,
    html: bool,
) -> tokio::task::JoinHandle<()> {
    let bot = bot.clone();
    tokio::spawn(async move {
        let started = tokio::time::Instant::now();
        for _ in 0..24 {
            let remaining = total_secs
                .saturating_sub(started.elapsed().as_secs())
                .max(1);
            tokio::time::sleep(Duration::from_secs(countdown_interval(remaining))).await;
            let remaining = total_secs
                .saturating_sub(started.elapsed().as_secs())
                .max(1);
            let text = format!(
                "{} {}",
                base_text,
                i18n::download_eta(lang, remaining as f64)
            );
            let mut request = bot.edit_message_text(chat, msg_id, text);
            if html {
                request = request.parse_mode(ParseMode::Html);
            }
            if request.await.is_err() {
                break;
            }
        }
    })
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

/// Cut back a trailing partial HTML entity (`&am` without its `;`) left
/// behind by truncation, so an already-escaped string can never split an
/// escape sequence. Raw ampersands not followed by entity chars are kept.
fn trim_partial_entity(s: &str) -> String {
    if let Some(pos) = s.rfind('&') {
        let tail = &s[pos..];
        let partial = !tail.contains(';')
            && tail.len() <= 10
            && tail[1..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '#');
        if partial {
            return s[..pos].to_owned();
        }
    }
    s.to_owned()
}

/// Truncate an already-escaped string to `max_units` UTF-16 units without
/// ever splitting an HTML entity.
fn truncate_escaped_utf16(s: &str, max_units: usize) -> String {
    trim_partial_entity(&truncate_utf16(s, max_units))
}

/// Truncate to at most `max_units` UTF-16 units (Telegram's text/caption limits
/// count units, so astral emoji cost two). The trailing ellipsis is itself one
/// unit and is only appended when there is room for it.
fn truncate_utf16(s: &str, max_units: usize) -> String {
    let mut units = 0usize;
    let mut result = String::with_capacity(s.len().min(max_units.saturating_mul(2) + 3));
    for ch in s.chars() {
        let len = ch.len_utf16();
        if units + len > max_units {
            if units < max_units {
                result.push('…');
            }
            break;
        }
        units += len;
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
        assert!(caption.encode_utf16().count() <= 1024);
        assert!(caption.ends_with(DEV_CHANNEL_LINE));

        // Astral chars count as two units; a char-based cap would overshoot.
        let astral = TrackMeta {
            album: Some("🌀".repeat(2000)),
            ..Default::default()
        };
        let caption = tags_caption(&astral, true).expect("caption");
        assert!(caption.encode_utf16().count() <= 1024);
    }
}

#[cfg(test)]
mod utf16_tests {
    use super::truncate_utf16;

    #[test]
    fn truncation_counts_astral_chars_as_two_units() {
        let text = "a🌀b🌀c"; // units: a=1, 🌀=2, b=1, 🌀=2, c=1
        assert_eq!(truncate_utf16(text, 7), text); // exactly 7 units, no cut
        assert_eq!(truncate_utf16(text, 6), "a🌀b🌀"); // no room for ellipsis
        assert_eq!(truncate_utf16(text, 5), "a🌀b…"); // 1+2+1+1 = 5 units
        assert_eq!(truncate_utf16(text, 2), "a…"); // 1+1 units
        assert_eq!(truncate_utf16(text, 0), "");
        assert_eq!(truncate_utf16("short", 16), "short");
    }

    #[test]
    fn astral_emoji_never_exceed_unit_budget() {
        let text = "🌀".repeat(3000); // 6000 UTF-16 units
        let clipped = truncate_utf16(&text, 2041);
        assert_eq!(clipped.encode_utf16().count(), 2041); // 1020 emoji + ellipsis
        assert!(clipped.ends_with('…'));
    }
}

#[cfg(test)]
mod countdown_tests {
    use super::{countdown_interval, empirical_remaining_secs};
    use common::domain::Lang;
    use common::i18n;

    #[test]
    fn countdown_interval_and_eta_unit_switch() {
        assert_eq!(countdown_interval(121), 15);
        assert_eq!(countdown_interval(120), 5);
        assert_eq!(i18n::download_eta(Lang::En, 89.0), "≈ 89 sec");
        assert_eq!(i18n::download_eta(Lang::En, 90.0), "≈ 2 min");
    }

    #[test]
    fn empirical_remaining_scales_with_completed_pace() {
        // 10 of 40 tracks in 100s -> 30 left at 10s/track.
        assert_eq!(empirical_remaining_secs(40, 10, 100.0), Some(300.0));
        assert_eq!(empirical_remaining_secs(40, 5, 20.0), Some(140.0));
        assert_eq!(empirical_remaining_secs(40, 0, 100.0), None);
        assert_eq!(empirical_remaining_secs(40, 40, 100.0), None);
    }
}
