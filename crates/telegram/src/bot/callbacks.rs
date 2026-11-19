//! Inline keyboard callbacks: search choices and per-user settings.

use crate::bot::commands::{
    AlbumPageSelection, additional_keyboard, albums_page, autodelete_view, language_keyboard,
    settings_keyboard,
};
use crate::bot::handlers::{
    ask_album_confirmation, edit_html, escape_html, hq_cover_url, try_cache_send,
};
use crate::state::AppState;
use common::domain::{AlbumRef, Provider, UserRecord};
use common::error::HandlerResult;
use common::i18n;
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
    _hq_cover: bool,
    genre_analysis: bool,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
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
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    }

    let Some(request) = state
        .get_selection::<common::domain::TrackRequest>(selection_id, crate::state::selection::TRACK)
        .await?
    else {
        let _ = edit_html(bot, chat, message_id, i18n::selection_expired(lang)).await;
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    };

    // A cache hit is still served inline: no download, no worker job.
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
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    }

    // Cache miss: hand the download to the worker, deduped by the request key.
    let key = request.key.as_str();
    if state.db.find_active_job_for_user_key(user_id, key).await? {
        let _ = bot
            .answer_callback_query(query.id.clone())
            .text(i18n::job_already_running(lang))
            .await;
        return Ok(());
    }
    match state
        .db
        .enqueue_job(
            &state.new_selection_id(),
            user_id,
            "track",
            serde_json::json!({
                "key": key,
                "chat_id": chat.0,
                "request": &request,
            }),
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            let _ = bot
                .answer_callback_query(query.id.clone())
                .text(i18n::job_already_running(lang))
                .await;
            return Ok(());
        }
        Err(error) => warn!(%error, selection_id, "could not enqueue track job"),
    }
    let _ = bot
        .answer_callback_query(query.id.clone())
        .text(i18n::downloading(lang))
        .await;
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
    _quality: u32,
    _tags_enabled: bool,
    _hq_cover: bool,
    _genre_analysis: bool,
    _album_info: bool,
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    };
    let chat = message.chat.id;
    let user_id = query.from.id.0 as i64;

    let Some(album) = state
        .get_selection::<common::domain::Album>(selection_id, crate::state::selection::ALBUM)
        .await?
    else {
        let _ = edit_html(bot, chat, message.id, i18n::selection_expired(lang)).await;
        let _ = bot.answer_callback_query(query.id.clone()).await;
        return Ok(());
    };
    state.delete_selection(selection_id).await?;

    // Stable per-album key: source id when known, otherwise the selection id.
    let dedup_key = match crate::bot::handlers::album_identity(&album) {
        Some((source, id)) => format!("album:{source}:{id}"),
        None => format!("album:{selection_id}"),
    };
    if state
        .db
        .find_active_job_for_user_key(user_id, &dedup_key)
        .await?
    {
        let _ = bot
            .answer_callback_query(query.id.clone())
            .text(i18n::job_already_running(lang))
            .await;
        return Ok(());
    }
    match state
        .db
        .enqueue_job(
            &state.new_selection_id(),
            user_id,
            "album",
            serde_json::json!({
                "key": dedup_key,
                "chat_id": chat.0,
                "card_id": message.id.0,
                "title": album.title,
                "album": &album,
            }),
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            let _ = bot
                .answer_callback_query(query.id.clone())
                .text(i18n::job_already_running(lang))
                .await;
            return Ok(());
        }
        Err(error) => warn!(%error, selection_id, "could not enqueue album job"),
    }
    let _ = bot
        .answer_callback_query(query.id.clone())
        .text(i18n::downloading(lang))
        .await;
    Ok(())
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
    // Answer before any DB/network work: teloxide does not auto-answer on `?`.
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let chat = message.chat.id;
    let message_id = message.id;
    let Some(page) = state
        .get_selection::<AlbumPageSelection>(selection_id, crate::state::selection::ALBUM_LIBRARY)
        .await?
    else {
        return show_albums_page(state, bot, chat, message_id, user.lang, user_id, 0).await;
    };
    let Some(item) = page.items.get(index) else {
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
    // Answer before the DB reads/writes below (no auto-answer on `?`).
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let Some(page) = state
        .get_selection::<AlbumPageSelection>(selection_id, crate::state::selection::ALBUM_LIBRARY)
        .await?
    else {
        return show_albums_page(state, bot, message.chat.id, message.id, lang, user_id, 0).await;
    };
    let Some(item) = page.items.get(index) else {
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

/// Hand an album ZIP off to the worker, deduped by source id. The worker owns
/// the tapped card and the status message; this handler only announces intent.
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
    let lang = user.lang;
    let dedup_key = format!("zip:{source}:{id}");
    if state
        .db
        .find_active_job_for_user_key(user_id, &dedup_key)
        .await?
    {
        let _ = bot
            .answer_callback_query(query.id.clone())
            .text(i18n::album_packing(lang))
            .await;
        return Ok(());
    }
    match state
        .db
        .enqueue_job(
            &state.new_selection_id(),
            user_id,
            "zip",
            serde_json::json!({
                "key": dedup_key,
                "chat_id": message.chat.id.0,
                "card_id": message.id.0,
                "source": source,
                "id": id,
            }),
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            let _ = bot
                .answer_callback_query(query.id.clone())
                .text(i18n::album_packing(lang))
                .await;
            return Ok(());
        }
        Err(error) => warn!(%error, source, id, "could not enqueue zip job"),
    }
    let _ = bot
        .answer_callback_query(query.id.clone())
        .text(i18n::downloading(lang))
        .await;
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
    // Answer before the resolve/save IO below (no auto-answer on `?`).
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let provider: Provider = match source.parse() {
        Ok(provider) => provider,
        Err(_) => return Ok(()),
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
    // The callback is already answered, so report the result as a short note
    // instead of a second (rejected) answer.
    let toast = if inserted {
        i18n::albums_saved(lang)
    } else {
        i18n::albums_already_saved(lang)
    };
    let _ = bot.send_message(message.chat.id, toast).await;
    Ok(())
}

/// ⭐/☆ toggle under a delivered track. Inline messages are edited through
/// their inline id; regular ones through chat + message id.
async fn handle_fav_toggle(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    key: &str,
    _lang: common::domain::Lang,
    user_id: i64,
    quality: u32,
) -> HandlerResult {
    // Answer before any IO; serialize concurrent taps on the same (user, key)
    // so toggle + markup render see one authoritative post-toggle state.
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let lock = state.inflight.fav_lock(user_id, key);
    let _guard = lock.lock().await;

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
    state
        .db
        .toggle_favorite(user_id, key, &title, performer.as_deref())
        .await?;
    // None means the button cannot be built (overlong key or DB error); keep
    // the existing markup rather than showing a wrong/toggle-destroying star.
    if let Some(markup) = crate::bot::handlers::fav_markup_for(&state.db, user_id, key).await {
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
