//! Inline keyboard callbacks: search choices and per-user settings.

use crate::bot::commands::{
    AlbumPageSelection, additional_keyboard, albums_page, autodelete_view, language_keyboard,
    settings_keyboard,
};
use crate::bot::handlers::{
    ask_album_confirmation, deliver_album, deliver_track, edit_html, error_text, escape_html,
    hq_cover_url, spawn_download_countdown,
};
use crate::state::AppState;
use common::domain::{AlbumRef, Provider, UserRecord};
use common::error::{DownloadError, HandlerResult};
use common::i18n;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{
    CallbackQuery, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, MessageId,
    ParseMode,
};
use tracing::{info, warn};

pub async fn handle_callback(state: AppState, bot: Bot, query: CallbackQuery) -> HandlerResult {
    let user_id = query.from.id.0 as i64;
    let preferred = common::domain::Lang::from_telegram_hint(
        query.from.language_code.as_deref(),
        state.config.default_lang,
    );
    let user = state.db.ensure_user(user_id, preferred).await?;
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
        handle_selection(
            &state,
            &bot,
            &query,
            id,
            user.lang,
            user.quality,
            user.tags_enabled,
            user.hq_cover,
            user.genre_analysis,
        )
        .await?;
    } else if let Some(id) = data.strip_prefix("alq:") {
        handle_album_pick(&state, &bot, &query, id, user.lang).await?;
    } else if let Some(payload) = data.strip_prefix("alv:")
        && let Some((id, index)) = payload.rsplit_once(':')
        && let Ok(index) = index.parse::<usize>()
    {
        handle_album_view(&state, &bot, &query, id, index, &user, user_id).await?;
    } else if let Some(payload) = data.strip_prefix("alr:")
        && let Some((id, index)) = payload.rsplit_once(':')
        && let Ok(index) = index.parse::<usize>()
    {
        handle_album_remove(&state, &bot, &query, id, index, user.lang, user_id).await?;
    } else if let Some(offset) = data.strip_prefix("alp:")
        && let Ok(offset) = offset.parse::<u32>()
    {
        handle_album_page(&state, &bot, &query, offset, user.lang, user_id).await?;
    } else if let Some(payload) = data.strip_prefix("alb:") {
        let offset = payload
            .rsplit_once(':')
            .and_then(|(_, value)| value.parse::<u32>().ok())
            .unwrap_or(0);
        handle_album_back(&state, &bot, &query, offset, user.lang, user_id).await?;
    } else if let Some(payload) = data.strip_prefix("azp:")
        && let Some((source, id)) = payload.split_once(':')
    {
        handle_album_zip(&state, &bot, &query, source, id, &user, user_id).await?;
    } else if let Some(payload) = data.strip_prefix("als:")
        && let Some((source, id)) = payload.split_once(':')
    {
        handle_album_save(&state, &bot, &query, source, id, user.lang, user_id).await?;
    } else if let Some(id) = data.strip_prefix("alc:") {
        handle_album_confirm(
            &state,
            &bot,
            &query,
            id,
            user.lang,
            user.quality,
            user.tags_enabled,
            user.hq_cover,
            user.genre_analysis,
            user.album_info,
        )
        .await?;
    } else if let Some(id) = data.strip_prefix("alx:") {
        handle_album_cancel(&state, &bot, &query, id, user.lang).await?;
    } else if let Some(id) = data.strip_prefix("more:") {
        // Legacy pre-2.6.18 forward button: its target offset is in the selection.
        crate::bot::handlers::handle_search_page(
            &state, &bot, &query, id, None, user.lang, user_id,
        )
        .await?;
    } else if let Some(payload) = data.strip_prefix("pg:")
        && let Some((id, offset)) = payload.rsplit_once(':')
        && let Ok(offset) = offset.parse::<u32>().map(|offset| offset.min(10_000))
    {
        crate::bot::handlers::handle_search_page(
            &state,
            &bot,
            &query,
            id,
            Some(offset),
            user.lang,
            user_id,
        )
        .await?;
    } else if let Some(key) = data.strip_prefix("fav:") {
        handle_fav_toggle(&state, &bot, &query, key, user.lang, user_id, user.quality).await?;
    } else if let Some(payload) = data.strip_prefix("s:") {
        let Some(message) = query.regular_message() else {
            let _ = bot.answer_callback_query(query.id.clone()).await;
            return Ok(());
        };
        if payload == "admenu" || payload == "adback" {
            let _ = bot.answer_callback_query(query.id.clone()).await;
            let (text, markup) = if payload == "admenu" {
                autodelete_view(user.lang, user.auto_delete_hours)
            } else {
                (
                    i18n::additional_title(user.lang).to_owned(),
                    additional_keyboard(
                        user.lang,
                        user.tags_enabled,
                        user.hq_cover,
                        user.lastfm_stats,
                        user.genre_analysis,
                        user.album_info,
                    ),
                )
            };
            bot.edit_message_text(message.chat.id, message.id, text)
                .parse_mode(ParseMode::Html)
                .reply_markup(markup)
                .await?;
            return Ok(());
        }
        if let Some(value) = payload.strip_prefix("ad:") {
            let parsed: Option<Option<i32>> = match value {
                "off" => Some(None),
                "1" => Some(Some(1)),
                "6" => Some(Some(6)),
                "12" => Some(Some(12)),
                "24" => Some(Some(24)),
                _ => None,
            };
            if let Some(hours) = parsed {
                state.db.set_auto_delete_hours(user_id, hours).await?;
                let toast = match hours {
                    None => i18n::autodelete_off(user.lang).to_owned(),
                    Some(hours) => i18n::autodelete_set(user.lang, hours as u32),
                };
                let _ = bot
                    .answer_callback_query(query.id.clone())
                    .text(toast)
                    .await;
                let (text, markup) = autodelete_view(user.lang, hours);
                bot.edit_message_text(message.chat.id, message.id, text)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(markup)
                    .await?;
            } else {
                let _ = bot.answer_callback_query(query.id.clone()).await;
            }
            return Ok(());
        }
        if payload == "langmenu"
            || payload == "langback"
            || payload == "advmenu"
            || payload == "advback"
        {
            let _ = bot.answer_callback_query(query.id.clone()).await;
            let (text, markup) = if payload == "langmenu" {
                (
                    i18n::language_menu_title(user.lang).to_owned(),
                    language_keyboard(user.lang),
                )
            } else if payload == "advmenu" {
                (
                    i18n::additional_title(user.lang).to_owned(),
                    additional_keyboard(
                        user.lang,
                        user.tags_enabled,
                        user.hq_cover,
                        user.lastfm_stats,
                        user.genre_analysis,
                        user.album_info,
                    ),
                )
            } else {
                // Both "langback" and "advback" return to the main view.
                (
                    i18n::settings_title(
                        user.lang,
                        user.quality,
                        user.lang,
                        user.search_source,
                        user.tags_enabled,
                        user.lastfm_user.as_deref(),
                        user.hq_cover,
                        user.lastfm_stats,
                        user.genre_analysis,
                    ),
                    settings_keyboard(user.lang, user.quality, user.search_source),
                )
            };
            bot.edit_message_text(message.chat.id, message.id, text)
                .parse_mode(ParseMode::Html)
                .reply_markup(markup)
                .await?;
            return Ok(());
        }
        match payload.split_once(':') {
            Some((setting, value)) => {
                if let Some(updated) = apply_setting(&state, user_id, &user, setting, value).await?
                {
                    // The toggle settings live in the advanced submenu; refresh
                    // the view the user is actually looking at.
                    let advanced = matches!(setting, "tags" | "hq" | "lstats" | "gan" | "ain");
                    refresh_settings(&state, &bot, &query, &updated, advanced).await?;
                    let _ = bot
                        .answer_callback_query(query.id.clone())
                        .text(i18n::settings_saved(updated.lang))
                        .await;
                } else {
                    let _ = bot.answer_callback_query(query.id.clone()).await;
                }
            }
            None => {
                let _ = bot.answer_callback_query(query.id.clone()).await;
            }
        }
    } else if let Some(value) = data.strip_prefix("gn:c:") {
        let (bars, period) = value
            .split_once(':')
            .map(|(bars, period)| {
                (
                    bars.parse::<u8>().ok(),
                    providers::lastfm::StatsPeriod::parse(period),
                )
            })
            .unwrap_or((
                value.parse::<u8>().ok(),
                Some(providers::lastfm::StatsPeriod::Overall),
            ));
        let _ = bot.answer_callback_query(query.id.clone()).await;
        if let (Some(bars), Some(period)) = (bars, period) {
            crate::bot::stats::handle_genre_chart_callback(&state, &bot, &query, bars, period)
                .await?;
        }
    } else if let Some(value) = data.strip_prefix("gn:p:") {
        let parts: Vec<&str> = value.split(':').collect();
        let parsed = parts
            .first()
            .and_then(|period| providers::lastfm::StatsPeriod::parse(period))
            .zip(parts.get(1).and_then(|bars| bars.parse::<u8>().ok()));
        let _ = bot.answer_callback_query(query.id.clone()).await;
        if let Some((period, bars)) = parsed {
            crate::bot::stats::handle_genre_chart_callback(&state, &bot, &query, bars, period)
                .await?;
        }
    } else if let Some(value) = data.strip_prefix("dyn:p:") {
        let _ = bot.answer_callback_query(query.id.clone()).await;
        if let Some(period) = providers::lastfm::StatsPeriod::parse(value) {
            crate::bot::history::handle_dynamics_callback(&state, &bot, &query, period).await?;
        }
    } else if let Some(id) = data.strip_prefix("ly:lrc:") {
        crate::bot::lyrics::handle_lrc_callback(&state, &bot, &query, id).await?;
    } else if let Some(id) = data.strip_prefix("ly:alt:") {
        crate::bot::lyrics::handle_alt_source_callback(&state, &bot, &query, id).await?;
    } else if let Some(payload) = data.strip_prefix("lf:m:") {
        // First level of the /profile menu: pick a kind, show grid sizes.
        let parts: Vec<&str> = payload.split(':').collect();
        if parts.len() == 2
            && let (Some(kind), Some(period)) = (
                providers::lastfm::StatsKind::parse(parts[0]),
                providers::lastfm::StatsPeriod::parse(parts[1]),
            )
        {
            let _ = bot.answer_callback_query(query.id.clone()).await;
            crate::bot::stats::handle_collage_menu_callback(
                &state,
                &bot,
                &query,
                Some(kind),
                period,
            )
            .await?;
        } else {
            let _ = bot.answer_callback_query(query.id.clone()).await;
        }
    } else if let Some(payload) = data.strip_prefix("lf:b:") {
        // Back to the first level of the /profile menu.
        if let Some(period) = providers::lastfm::StatsPeriod::parse(payload) {
            let _ = bot.answer_callback_query(query.id.clone()).await;
            crate::bot::stats::handle_collage_menu_callback(&state, &bot, &query, None, period)
                .await?;
        } else {
            let _ = bot.answer_callback_query(query.id.clone()).await;
        }
    } else if let Some(payload) = data.strip_prefix("lf:c:") {
        let parts: Vec<&str> = payload.split(':').collect();
        if parts.len() == 3
            && let (Some(kind), Some(period), Ok(grid)) = (
                providers::lastfm::StatsKind::parse(parts[0]),
                providers::lastfm::StatsPeriod::parse(parts[2]),
                parts[1].parse::<u8>(),
            )
            && (3..=10).contains(&grid)
        {
            let _ = bot.answer_callback_query(query.id.clone()).await;
            crate::bot::stats::handle_collage_callback(&state, &bot, &query, kind, grid, period)
                .await?;
        } else {
            let _ = bot.answer_callback_query(query.id.clone()).await;
        }
    } else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_selection(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    lang: common::domain::Lang,
    quality: u32,
    tags_enabled: bool,
    hq_cover: bool,
    genre_analysis: bool,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let chat = message.chat.id;
    let message_id = message.id;
    let user_id = query.from.id.0 as i64;

    if !state.limiter.allow(user_id).await? {
        let _ = edit_html(
            bot,
            chat,
            message_id,
            i18n::rate_limited(lang, state.config.rate_limit_per_min),
        )
        .await;
        return Ok(());
    }

    let Some(request) = state
        .get_selection::<common::domain::TrackRequest>(selection_id, crate::state::selection::TRACK)
        .await?
    else {
        let _ = edit_html(bot, chat, message_id, i18n::selection_expired(lang)).await;
        return Ok(());
    };

    let duration = request
        .meta
        .as_ref()
        .and_then(|meta| meta.duration_secs)
        .map(f64::from);
    let rate = state.db.download_rate().await?;
    let avg = state.db.avg_recent_download_secs().await?;
    let secs = match (rate, duration) {
        (Some(rate), Some(duration)) => Some(rate * duration),
        _ => avg,
    };
    let base = i18n::downloading(lang).to_owned();
    let countdown = if let Some(secs) = secs {
        let eta = i18n::download_eta(lang, secs);
        let _ = edit_html(bot, chat, message_id, format!("{base} {eta}")).await;
        Some(spawn_download_countdown(
            bot,
            chat,
            message_id,
            base,
            (secs.ceil() as u64).max(1),
            lang,
            true,
        ))
    } else {
        let _ = edit_html(bot, chat, message_id, base).await;
        None
    };
    info!(selection_id, "delivering selected search result");
    let result = deliver_track(
        state,
        bot,
        chat,
        request.clone(),
        quality,
        user_id,
        tags_enabled,
        hq_cover,
        genre_analysis,
    )
    .await;
    if let Some(handle) = &countdown {
        handle.abort();
        // Let any in-flight ticker edit resolve so the final edit lands last.
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    match result {
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

/// User picked one of the `/alb` album candidates; resolve it through the
/// regular collection pipeline (the candidate list message is edited in
/// place into the confirmation keyboard).
async fn handle_album_pick(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    lang: common::domain::Lang,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let chat = message.chat.id;
    let message_id = message.id;

    let Some(album_ref) = state
        .get_selection::<common::domain::AlbumRef>(selection_id, crate::state::selection::ALBUM_REF)
        .await?
    else {
        let _ = edit_html(bot, chat, message_id, i18n::selection_expired(lang)).await;
        return Ok(());
    };
    state.delete_selection(selection_id).await?;
    info!(
        selection_id,
        provider = %album_ref.provider,
        "album candidate picked; resolving"
    );
    let _ = edit_html(bot, chat, message_id, i18n::album_loading(lang)).await;
    ask_album_confirmation(
        state,
        bot,
        chat,
        message_id,
        lang,
        &album_ref,
        query.from.id.0 as i64,
    )
    .await
}

/// User confirmed a collection: start downloading it with limited parallelism.
#[allow(clippy::too_many_arguments)]
async fn handle_album_confirm(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    lang: common::domain::Lang,
    quality: u32,
    tags_enabled: bool,
    hq_cover: bool,
    genre_analysis: bool,
    album_info: bool,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let chat = message.chat.id;
    let message_id = message.id;
    let user_id = query.from.id.0 as i64;

    let Some(album) = state
        .get_selection::<common::domain::Album>(selection_id, crate::state::selection::ALBUM)
        .await?
    else {
        let _ = edit_html(bot, chat, message_id, i18n::selection_expired(lang)).await;
        return Ok(());
    };
    state.delete_selection(selection_id).await?;
    info!(
        selection_id,
        tracks = album.tracks.len(),
        "album confirmed; starting downloads"
    );
    let job = match crate::bot::jobs::JobGuard::start(
        state,
        user_id,
        "album",
        serde_json::json!({"title": album.title, "tracks": album.tracks.len()}),
    )
    .await?
    {
        Some(job) => job,
        None => {
            let _ = edit_html(bot, chat, message_id, i18n::job_already_running(lang)).await;
            return Ok(());
        }
    };
    let result = deliver_album(
        state,
        bot,
        chat,
        message_id,
        lang,
        quality,
        user_id,
        album,
        tags_enabled,
        hq_cover,
        genre_analysis,
        album_info,
        Some(&job),
    )
    .await;
    if job.cancelled().await {
        job.finish("cancelled", None).await;
    } else if matches!(&result, Ok(delivered) if *delivered > 0) {
        job.finish("done", None).await;
    } else {
        job.finish("failed", None).await;
    }
    result.map(|_| ())
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
    state.delete_selection(selection_id).await?;
    info!(selection_id, "album download cancelled");
    Ok(())
}

/// Re-render an `/albums` page into the message a library callback came from.
async fn show_albums_page(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    message_id: MessageId,
    lang: common::domain::Lang,
    user_id: i64,
    offset: u32,
) -> HandlerResult {
    match albums_page(state, user_id, lang, offset).await? {
        Some((text, markup)) => {
            let _ = bot
                .edit_message_text(chat, message_id, text)
                .parse_mode(ParseMode::Html)
                .reply_markup(markup)
                .await;
        }
        None => {
            let _ = edit_html(bot, chat, message_id, i18n::albums_empty(lang)).await;
        }
    }
    Ok(())
}

async fn handle_album_page(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    offset: u32,
    lang: common::domain::Lang,
    user_id: i64,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    show_albums_page(
        state,
        bot,
        message.chat.id,
        message.id,
        lang,
        user_id,
        offset,
    )
    .await
}

/// Return to the `/albums` page the detail card was opened from. The offset is
/// embedded in the callback; if it is missing, fall back to the first page.
async fn handle_album_back(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    offset: u32,
    lang: common::domain::Lang,
    user_id: i64,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    show_albums_page(
        state,
        bot,
        message.chat.id,
        message.id,
        lang,
        user_id,
        offset,
    )
    .await
}

/// Open a saved album: rebuild its `AlbumRef` from the payload saved at
/// delivery time and resolve it through the same manager call `alq:` uses.
#[allow(clippy::too_many_arguments)]
async fn handle_album_view(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    index: usize,
    user: &UserRecord,
    user_id: i64,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    };
    let chat = message.chat.id;
    let message_id = message.id;
    let Some(page) = state
        .get_selection::<AlbumPageSelection>(selection_id, crate::state::selection::ALBUM_LIBRARY)
        .await?
    else {
        let _ = bot
            .answer_callback_query(query.id.clone())
            .text(i18n::selection_expired(user.lang))
            .await;
        return show_albums_page(state, bot, chat, message_id, user.lang, user_id, 0).await;
    };
    let Some(item) = page.items.get(index) else {
        let _ = bot
            .answer_callback_query(query.id.clone())
            .text(i18n::selection_expired(user.lang))
            .await;
        return show_albums_page(
            state,
            bot,
            chat,
            message_id,
            user.lang,
            user_id,
            page.offset,
        )
        .await;
    };
    let _ = bot.answer_callback_query(query.id.clone()).await;

    let album_ref = item.album_ref();
    let resolved = match &album_ref {
        Some(album_ref) => state.manager.resolve_album(album_ref).await.ok(),
        None => None,
    };
    let title = escape_html(&item.title);
    let artist = escape_html(&item.artist);
    let mut text = match &resolved {
        Some(album) => i18n::albums_item(user.lang, &title, album.tracks.len() as u32),
        None => format!("<b>{title}</b>"),
    };
    text.push('\n');
    text.push_str(&artist);

    let mut first_row = Vec::new();
    if let Some(album_ref) = &album_ref {
        let zip = format!(
            "azp:{}:{}",
            album_ref.provider.as_str(),
            album_ref.url.as_deref().unwrap_or("")
        );
        if zip.len() <= 64 {
            first_row.push(InlineKeyboardButton::callback(
                i18n::download_archive(user.lang).to_owned(),
                zip,
            ));
        }
    }
    let remove = format!("alr:{selection_id}:{index}");
    if remove.len() <= 64 {
        first_row.push(InlineKeyboardButton::callback(
            i18n::albums_remove_button(user.lang).to_owned(),
            remove,
        ));
    }
    let mut rows = Vec::new();
    if !first_row.is_empty() {
        rows.push(first_row);
    }
    let back = format!("alb:{selection_id}:{}", page.offset);
    if back.len() <= 64 {
        rows.push(vec![InlineKeyboardButton::callback(
            i18n::back_arrow(user.lang).to_owned(),
            back,
        )]);
    }

    // Saved cover first, then the resolved album's first track artwork.
    let cover = item
        .cover_url
        .as_deref()
        .or_else(|| {
            resolved
                .as_ref()
                .and_then(|album| album.tracks.first())
                .and_then(|track| track.meta.as_ref())
                .and_then(|meta| meta.thumbnail_url.as_deref())
        })
        .filter(|url| !url.trim().is_empty())
        .map(hq_cover_url);
    if let Some(url) = cover
        && let Some(bytes) = providers::http::fetch_bytes(&state.http, &url, 8 * 1024 * 1024).await
    {
        let _ = bot.send_photo(chat, InputFile::memory(bytes)).await;
    }
    let _ = bot
        .edit_message_text(chat, message_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new(rows))
        .await;
    Ok(())
}

async fn handle_album_remove(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    selection_id: &str,
    index: usize,
    lang: common::domain::Lang,
    user_id: i64,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    };
    let Some(page) = state
        .get_selection::<AlbumPageSelection>(selection_id, crate::state::selection::ALBUM_LIBRARY)
        .await?
    else {
        let _ = bot
            .answer_callback_query(query.id.clone())
            .text(i18n::selection_expired(lang))
            .await;
        return show_albums_page(state, bot, message.chat.id, message.id, lang, user_id, 0).await;
    };
    let Some(item) = page.items.get(index) else {
        let _ = bot
            .answer_callback_query(query.id.clone())
            .text(i18n::selection_expired(lang))
            .await;
        return show_albums_page(
            state,
            bot,
            message.chat.id,
            message.id,
            lang,
            user_id,
            page.offset,
        )
        .await;
    };
    state
        .db
        .delete_saved_album(user_id, &item.album_key)
        .await?;
    let _ = bot
        .answer_callback_query(query.id.clone())
        .text(i18n::albums_removed(lang))
        .await;
    show_albums_page(
        state,
        bot,
        message.chat.id,
        message.id,
        lang,
        user_id,
        page.offset,
    )
    .await
}

/// Build a ZIP from the album's cached tracks. `prepare_zip_track` rebuilds
/// every request from the DB cache, so uncached keys are skipped there; verify
/// at least one key is reconstructable before packing.
#[allow(clippy::too_many_arguments)]
async fn handle_album_zip(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    source: &str,
    id: &str,
    user: &UserRecord,
    user_id: i64,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    };
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let chat = message.chat.id;
    let status_id = message.id;
    let state = state.clone();
    let bot = bot.clone();
    let lang = user.lang;
    let quality = user.quality;
    let hq_cover = user.hq_cover;
    let source = source.to_owned();
    let id = id.to_owned();
    tokio::spawn(async move {
        // Resolve first (bounded): the ETA below must count the album's real
        // track count, not the 25-track worst case.
        let resolved = tokio::time::timeout(Duration::from_secs(20), async {
            let provider: Provider = source
                .parse()
                .map_err(|_| DownloadError::Unsupported("unknown album source".to_owned()))?;
            let album = state
                .manager
                .resolve_album(&AlbumRef {
                    provider,
                    url: Some(id.clone()),
                    title: None,
                })
                .await?;
            let all_keys: Vec<_> = album.tracks.iter().map(|track| track.key.clone()).collect();
            // zip_album rejects >MAX_ZIP_TRACKS keys outright; pack the first
            // ones instead of failing the whole request.
            let selected = all_keys.len().min(downloader::MAX_ZIP_TRACKS);
            let keys: Vec<_> = all_keys[..selected].to_vec();
            let durations: f64 = album.tracks[..selected]
                .iter()
                .filter_map(|track| {
                    track
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.duration_secs)
                        .map(f64::from)
                })
                .sum();
            let mut cached = 0usize;
            for key in &keys {
                if state.manager.get_cached(key, quality).await?.is_some() {
                    cached += 1;
                }
            }
            Ok::<_, common::error::AppError>((album.title, keys, cached, durations))
        })
        .await;
        let (title, keys, cached, durations) = match resolved {
            Ok(Ok(resolved)) => resolved,
            Ok(Err(error)) => {
                let _ = edit_html(
                    &bot,
                    chat,
                    status_id,
                    i18n::error(lang, &DownloadError::Internal(error.to_string())),
                )
                .await;
                return;
            }
            Err(_) => {
                let _ = edit_html(
                    &bot,
                    chat,
                    status_id,
                    i18n::error(
                        lang,
                        &DownloadError::Internal("album resolve timed out".to_owned()),
                    ),
                )
                .await;
                return;
            }
        };
        if cached == 0 {
            let _ = edit_html(&bot, chat, status_id, i18n::album_pack_no_tracks(lang)).await;
            return;
        }
        let base = i18n::album_packing(lang).to_owned();
        let rate = state.db.download_rate().await.ok().flatten();
        let avg = state.db.avg_recent_download_secs().await.ok().flatten();
        // ZIP downloads run two tracks at a time; the per-job mean is already
        // a single-track wall time, so only the size-weighted path halves it.
        let estimate = match rate {
            Some(rate) if durations > 0.0 => Some((rate * durations / 2.0).max(1.0)),
            _ => avg.map(|avg| avg * keys.len() as f64),
        };
        let countdown = match estimate {
            Some(estimate) => {
                let eta = i18n::download_eta(lang, estimate);
                let _ = edit_html(&bot, chat, status_id, format!("{base} {eta}")).await;
                Some(spawn_download_countdown(
                    &bot,
                    chat,
                    status_id,
                    base,
                    (estimate.ceil() as u64).max(1),
                    lang,
                    true,
                ))
            }
            None => {
                let _ = edit_html(&bot, chat, status_id, base).await;
                None
            }
        };
        let result = tokio::time::timeout(
            Duration::from_secs(10 * 60),
            state.manager.zip_album(&keys, quality, hq_cover),
        )
        .await;
        if let Some(handle) = &countdown {
            handle.abort();
            // Let any in-flight ticker edit resolve so the final edit lands last.
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        match result {
            Ok(Ok(outcome)) => {
                let safe: String = title
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || "-_. ".contains(c) {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let filename = format!("{}.zip", crate::bot::send::truncate(&safe, 80));
                let caption = i18n::albums_item(lang, &escape_html(&title), outcome.tracks as u32);
                let input = InputFile::memory(outcome.bytes).file_name(filename);
                match bot
                    .send_document(chat, input)
                    .caption(caption)
                    .parse_mode(ParseMode::Html)
                    .await
                {
                    Ok(message) => {
                        state.maybe_track_deletion(user_id, chat, message.id);
                        let _ = bot.delete_message(chat, status_id).await;
                    }
                    // Keep the packing message as the error report: deleting
                    // it would leave the user with no feedback at all.
                    Err(error) => {
                        warn!(%error, "album ZIP upload failed");
                        let _ = edit_html(
                            &bot,
                            chat,
                            status_id,
                            i18n::error(lang, &DownloadError::Internal(error.to_string())),
                        )
                        .await;
                    }
                }
            }
            Ok(Err(error)) => {
                let lower = error.to_string().to_ascii_lowercase();
                let text = if lower.contains("too large") {
                    i18n::album_pack_too_large(lang, 900)
                } else if lower.contains("no track could be downloaded") {
                    i18n::album_pack_no_tracks(lang).to_owned()
                } else {
                    i18n::error(lang, &DownloadError::Internal(error.to_string()))
                };
                let _ = edit_html(&bot, chat, status_id, text).await;
            }
            Err(_) => {
                let _ = edit_html(
                    &bot,
                    chat,
                    status_id,
                    i18n::error(lang, &DownloadError::Internal("ZIP timed out".to_owned())),
                )
                .await;
            }
        }
    });
    Ok(())
}

/// 💾 under a delivered album: resolve the same way `alq:` does, then save
/// the album's identity and metadata in the user's library.
async fn handle_album_save(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    source: &str,
    id: &str,
    lang: common::domain::Lang,
    user_id: i64,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    };
    let provider: Provider = match source.parse() {
        Ok(provider) => provider,
        Err(_) => {
            let _ = bot.answer_callback_query(query.id.clone()).await;
            return Ok(());
        }
    };
    let album = match state
        .manager
        .resolve_album(&AlbumRef {
            provider,
            url: Some(id.to_owned()),
            title: None,
        })
        .await
    {
        Ok(album) => album,
        Err(error) => {
            let _ = edit_html(bot, message.chat.id, message.id, i18n::error(lang, &error)).await;
            let _ = bot.answer_callback_query(query.id.clone()).await;
            return Ok(());
        }
    };
    let first = album.tracks.first().and_then(|track| track.meta.as_ref());
    let artist = first
        .and_then(|meta| meta.performer.as_deref())
        .filter(|artist| !artist.trim().is_empty())
        .unwrap_or("");
    let cover = first
        .and_then(|meta| meta.thumbnail_url.as_deref())
        .filter(|url| !url.trim().is_empty())
        .map(str::to_owned);
    let key = format!("{source}:{id}");
    let payload = serde_json::json!({"source": source, "id": id}).to_string();
    let inserted = state
        .db
        .save_album(
            user_id,
            &key,
            &album.title,
            artist,
            cover.as_deref(),
            &payload,
        )
        .await?;
    let _ = bot
        .answer_callback_query(query.id.clone())
        .text(if inserted {
            i18n::albums_saved(lang)
        } else {
            i18n::albums_already_saved(lang)
        })
        .await;
    Ok(())
}

/// ⭐/☆ toggle under a delivered track. Inline messages are edited through
/// their inline id; regular ones through chat + message id.
async fn handle_fav_toggle(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    key: &str,
    lang: common::domain::Lang,
    user_id: i64,
    quality: u32,
) -> HandlerResult {
    // Title/performer from the message audio keep /fav readable without a
    // catalog lookup; the key stays authoritative.
    let (title, performer) = match query.regular_message().and_then(|message| message.audio()) {
        Some(audio) => (
            audio.title.clone().unwrap_or_else(|| key.to_owned()),
            audio.performer.clone(),
        ),
        None => {
            // Inline messages have no audio to read; use cached metadata so
            // /fav shows a name instead of the raw track key.
            let cached = state
                .manager
                .get_cached(&common::domain::TrackKey(key.to_owned()), quality)
                .await
                .ok()
                .flatten();
            cached.map_or((key.to_owned(), None), |cached| {
                let title = cached.meta.stripped_title();
                (
                    if title.is_empty() {
                        key.to_owned()
                    } else {
                        title.into_owned()
                    },
                    cached.meta.performer.clone(),
                )
            })
        }
    };
    let added = state
        .db
        .toggle_favorite(user_id, key, &title, performer.as_deref())
        .await?;
    let _ = bot
        .answer_callback_query(query.id.clone())
        .text(i18n::fav_toggled(lang, added))
        .await;
    // None means the button cannot be built (overlong key or DB error); keep
    // the existing markup rather than showing a wrong/toggle-destroying star.
    if let Some(markup) = crate::bot::handlers::fav_markup_for(state, user_id, key).await {
        if let Some(inline_message_id) = query.inline_message_id.clone() {
            let _ = bot
                .edit_message_reply_markup_inline(inline_message_id)
                .reply_markup(markup)
                .await;
        } else if let Some(message) = query.regular_message() {
            let _ = bot
                .edit_message_reply_markup(message.chat.id, message.id)
                .reply_markup(markup)
                .await;
        }
    }
    Ok(())
}

/// Apply one `/settings` toggle and return the updated record. The caller
/// already read the user row for the callback, so the new state is patched in
/// memory instead of re-reading the row after every write.
async fn apply_setting(
    state: &AppState,
    user_id: i64,
    user: &common::domain::UserRecord,
    setting: &str,
    value: &str,
) -> Result<Option<common::domain::UserRecord>, Box<dyn std::error::Error + Send + Sync>> {
    let mut updated = user.clone();
    match setting {
        "tags" => {
            let Some(enabled) = parse_bool(value) else {
                return Ok(None);
            };
            state.db.set_tags_enabled(user_id, enabled).await?;
            updated.tags_enabled = enabled;
        }
        "hq" => {
            let Some(enabled) = parse_bool(value) else {
                return Ok(None);
            };
            state.db.set_hq_cover(user_id, enabled).await?;
            updated.hq_cover = enabled;
        }
        "lstats" => {
            let Some(enabled) = parse_bool(value) else {
                return Ok(None);
            };
            state.db.set_lastfm_stats(user_id, enabled).await?;
            updated.lastfm_stats = enabled;
        }
        "gan" => {
            let Some(enabled) = parse_bool(value) else {
                return Ok(None);
            };
            state.db.set_genre_analysis(user_id, enabled).await?;
            updated.genre_analysis = enabled;
        }
        "ain" => {
            let Some(enabled) = parse_bool(value) else {
                return Ok(None);
            };
            state.db.set_album_info(user_id, enabled).await?;
            updated.album_info = enabled;
        }
        "q" => {
            let Ok(quality) = value.parse::<u32>() else {
                return Ok(None);
            };
            let quality = if quality == 0 {
                0
            } else {
                quality.clamp(64, 320)
            };
            state.db.set_quality(user_id, quality).await?;
            updated.quality = quality;
        }
        "src" => {
            let Some(source) = parse_search_source(value) else {
                return Ok(None);
            };
            state.db.set_search_source(user_id, source).await?;
            updated.search_source = source;
        }
        "lang" => {
            let Some(lang) = common::domain::Lang::from_code(value) else {
                return Ok(None);
            };
            state.db.set_language(user_id, lang).await?;
            updated.lang = lang;
        }
        _ => return Ok(None),
    }
    Ok(Some(updated))
}

async fn refresh_settings(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    user: &common::domain::UserRecord,
    advanced: bool,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let (body, markup) = if advanced {
        (
            i18n::additional_title(user.lang).to_owned(),
            additional_keyboard(
                user.lang,
                user.tags_enabled,
                user.hq_cover,
                user.lastfm_stats,
                user.genre_analysis,
                user.album_info,
            ),
        )
    } else {
        let mut body = i18n::settings_title(
            user.lang,
            user.quality,
            user.lang,
            user.search_source,
            user.tags_enabled,
            user.lastfm_user.as_deref(),
            user.hq_cover,
            user.lastfm_stats,
            user.genre_analysis,
        );
        if !state.config.search_providers.contains(&user.search_source) {
            body.push_str("\n\n");
            body.push_str(i18n::search_source_blocked(user.lang));
        }
        (
            body,
            settings_keyboard(user.lang, user.quality, user.search_source),
        )
    };
    bot.edit_message_text(message.chat.id, message.id, body)
        .parse_mode(ParseMode::Html)
        .reply_markup(markup)
        .await?;
    Ok(())
}

/// Accept only the exact flag values produced by the keyboard.
fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

/// Parse a `/settings` search-source callback, accepting only the exposed
/// choices (YouTube / SoundCloud / Deezer).
fn parse_search_source(value: &str) -> Option<common::domain::Provider> {
    use common::domain::Provider;
    use std::str::FromStr;
    Provider::from_str(value.trim())
        .ok()
        .filter(|source| Provider::SEARCH_CHOICES.contains(source))
}
