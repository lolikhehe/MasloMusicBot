//! Inline keyboard callbacks: search choices and per-user settings.

use crate::bot::commands::settings_keyboard;
use crate::bot::handlers::{
    ask_album_confirmation, deliver_album, deliver_track, edit_html, error_text, escape_html,
};
use crate::state::AppState;
use common::error::HandlerResult;
use common::i18n;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, ParseMode};
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
        )
        .await?;
    } else if let Some(id) = data.strip_prefix("alx:") {
        handle_album_cancel(&state, &bot, &query, id, user.lang).await?;
    } else if let Some(payload) = data.strip_prefix("s:") {
        match payload.split_once(':') {
            Some((setting, value)) => {
                if let Some(updated) = apply_setting(&state, user_id, &user, setting, value).await?
                {
                    refresh_settings(&state, &bot, &query, &updated).await?;
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
        if let Ok(bars) = value.parse::<u8>() {
            let _ = bot.answer_callback_query(query.id.clone()).await;
            crate::bot::stats::handle_genre_chart_callback(&state, &bot, &query, bars).await?;
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

    let _ = edit_html(bot, chat, message_id, i18n::downloading(lang)).await;
    info!(selection_id, "delivering selected search result");
    match deliver_track(
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
    .await
    {
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
    deliver_album(
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
    )
    .await
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
) -> HandlerResult {
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
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
    bot.edit_message_text(message.chat.id, message.id, body)
        .parse_mode(ParseMode::Html)
        .reply_markup(settings_keyboard(
            user.lang,
            user.quality,
            user.search_source,
            user.tags_enabled,
            user.hq_cover,
            user.lastfm_stats,
            user.genre_analysis,
        ))
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
