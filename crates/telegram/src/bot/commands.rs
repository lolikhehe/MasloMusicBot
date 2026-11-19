//! Command parsing and handlers. Commands are parsed manually so `@botname`
//! suffixes and admin arguments behave predictably in groups.

use crate::bot::handlers::{escape_html, spawn_download_countdown};
use crate::bot::send::truncate;
use crate::state::AppState;
use common::domain::{Lang, Provider};
use common::error::HandlerResult;
use common::i18n;
use providers::key_from_input;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile, Message, ParseMode};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::warn;

/// Simultaneous artwork resolutions for `/similar` (mirrors the collage path).
const COVER_PARALLELISM: usize = 6;
/// Albums shown per `/albums` page.
const ALBUMS_PER_PAGE: u32 = 10;

/// One saved album on an `alib` selection page. The `alv:` callback indexes
/// into the page's Vec of these, so every detail stays server-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedAlbumRef {
    pub album_key: String,
    pub title: String,
    pub artist: String,
    pub cover_url: Option<String>,
    pub payload_json: String,
}

/// Payload of one `alib` selection: offset lets the detail/remove handlers
/// reopen exactly the page the user came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumPageSelection {
    pub offset: u32,
    pub items: Vec<SavedAlbumRef>,
}

impl SavedAlbumRef {
    /// Rebuild the album identity saved at delivery time. The same `AlbumRef`
    /// shape the `/alb` flow resolves, so the download path is shared.
    pub fn album_ref(&self) -> Option<common::domain::AlbumRef> {
        let payload: serde_json::Value = serde_json::from_str(&self.payload_json).ok()?;
        let source = payload.get("source")?.as_str()?;
        let id = payload.get("id")?.as_str()?;
        Some(common::domain::AlbumRef {
            provider: source.parse().ok()?,
            url: Some(id.to_owned()),
            title: Some(self.title.clone()),
        })
    }
}

/// Render one `/albums` page into a header + keyboard. `None` means the library
/// is empty; the offset is clamped so removed items never leave a blank page.
pub async fn albums_page(
    state: &AppState,
    user_id: i64,
    lang: Lang,
    offset: u32,
) -> common::error::AppResult<Option<(String, InlineKeyboardMarkup)>> {
    let count = state.db.count_saved_albums(user_id).await?;
    if count == 0 {
        return Ok(None);
    }
    let per_page = i64::from(ALBUMS_PER_PAGE);
    let offset = i64::from(offset).min(count - per_page).max(0) as u32;
    let rows = state
        .db
        .saved_albums(user_id, ALBUMS_PER_PAGE, offset)
        .await?;
    let items: Vec<SavedAlbumRef> = rows
        .into_iter()
        .map(|row| SavedAlbumRef {
            album_key: row.album_key,
            title: row.title,
            artist: row.artist,
            cover_url: row.cover_url,
            payload_json: row.payload_json,
        })
        .collect();
    let selection_id = state.new_selection_id();
    let selection = AlbumPageSelection { offset, items };
    state
        .put_selections(
            crate::state::selection::ALBUM_LIBRARY,
            user_id,
            &[(selection_id.clone(), &selection)],
        )
        .await?;
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = selection
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            vec![InlineKeyboardButton::callback(
                truncate(&format!("{} — {}", item.title, item.artist), 60),
                format!("alv:{selection_id}:{index}"),
            )]
        })
        .collect();
    let mut arrows = Vec::new();
    if offset > 0 {
        arrows.push(InlineKeyboardButton::callback(
            "◀",
            format!("alp:{}", offset.saturating_sub(ALBUMS_PER_PAGE)),
        ));
    }
    if i64::from(offset) + per_page < count {
        arrows.push(InlineKeyboardButton::callback(
            "▶",
            format!("alp:{}", offset + ALBUMS_PER_PAGE),
        ));
    }
    if !arrows.is_empty() {
        keyboard.push(arrows);
    }
    let header = format!("{} ({})", i18n::albums_title(lang), count);
    Ok(Some((header, InlineKeyboardMarkup::new(keyboard))))
}

/// Auto-delete submenu: current value + hint + Off/1/6/12/24 + back.
pub fn autodelete_view(lang: Lang, hours: Option<i32>) -> (String, InlineKeyboardMarkup) {
    let mut text = match hours {
        Some(hours) => i18n::autodelete_current(lang, hours as u32),
        None => i18n::autodelete_off(lang).to_owned(),
    };
    text.push_str("\n\n");
    text.push_str(i18n::autodelete_hint(lang));
    let rows = vec![
        vec![
            InlineKeyboardButton::callback(i18n::autodelete_off(lang), "s:ad:off"),
            InlineKeyboardButton::callback(i18n::autodelete_hour_1(lang), "s:ad:1"),
        ],
        vec![
            InlineKeyboardButton::callback(i18n::autodelete_hour_6(lang), "s:ad:6"),
            InlineKeyboardButton::callback(i18n::autodelete_hour_12(lang), "s:ad:12"),
        ],
        vec![InlineKeyboardButton::callback(
            i18n::autodelete_hour_24(lang),
            "s:ad:24",
        )],
        vec![InlineKeyboardButton::callback(
            i18n::back_button(lang),
            "s:adback",
        )],
    ];
    (text, InlineKeyboardMarkup::new(rows))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Start,
    Help,
    Settings,
    Stats,
    Ban(Option<i64>),
    Unban(Option<i64>),
    Purge(Option<String>),
    /// `/lastfm` — show status; `/lastfm <nick>` — connect; `/lastfm off` — disconnect.
    LastFm(Option<String>),
    /// `/alb <name>` — search public album catalogs (Deezer / Apple Music) and
    /// download the chosen album track by track.
    AlbumSearch(Option<String>),
    /// `/profile` / `/lastfmstats [period]` — Last.fm scrobble summary.
    Profile(Option<String>),
    /// `/collage <tracks|albums|artists> [NxN] [period]` — PNG chart.
    Collage(Option<String>),
    /// `/import <playlist text>` or an uploaded CSV/TXT/JSON document.
    Import(Option<String>),
    /// `/genres [bars]` — per-user genre analyzer chart.
    GenreStats(Option<String>),
    /// `/lyrics [Artist — Title]` — public lyrics lookup; falls back to the
    /// last delivered track or an audio reply.
    Lyrics(Option<String>),
    /// `/changelog` — show release notes without a broadcast.
    Changelog,
    /// `/me` — the caller's own counters.
    UserStats,
    /// `/history [period]` — recent listening history from Postgres.
    History(Option<String>),
    /// `/dynamics [period]` — genre dynamics over time chart.
    Dynamics(Option<String>),
    /// `/digest daily|weekly|off|quiet HH-HH|status` — personal digest planner.
    Digest(Option<String>),
    /// `/broadcast <text>` — admin-only one-off notice.
    Broadcast(Option<String>),
    Queue,
    Cancel(Option<String>),
    /// `/stop` — cancel every active action for the caller.
    Stop,
    Convert(Option<String>),
    Normalize(Option<String>),
    Playlist(Option<String>),
    /// `/fav` — saved tracks, re-served instantly from the Telegram cache.
    Favorites,
    /// `/albums` — paginated library of saved albums.
    Albums,
    /// `/recognize` in reply to audio — identify by acoustic fingerprint.
    Recognize,
    /// `/similar` in reply to a bot track — similar tracks as buttons.
    Similar,
    /// `/artist <name>` — top catalog tracks of an artist as buttons.
    Artist(Option<String>),
    /// `/cover` in reply to a bot track — the 1000px album art as a photo.
    Cover,
    /// `/groupmode on|off` — admin-only opt-in for this group.
    GroupMode(Option<String>),
    Unknown(String),
}

pub fn parse_command(text: &str, bot_username: &str) -> Option<Command> {
    let text = text.trim();
    let rest = text.strip_prefix('/')?;
    let mut parts = rest.split_whitespace();
    let head = parts.next()?;
    let (name, target) = head
        .split_once('@')
        .map(|(name, target)| (name, Some(target)))
        .unwrap_or((head, None));

    if let Some(target) = target
        && !target.eq_ignore_ascii_case(bot_username)
    {
        return None;
    }

    let args = parts.collect::<Vec<_>>().join(" ");
    let command = match name.to_ascii_lowercase().as_str() {
        "start" => Command::Start,
        "help" => Command::Help,
        "settings" => Command::Settings,
        "stats" => Command::Stats,
        "lastfm" => Command::LastFm((!args.is_empty()).then_some(args)),
        "alb" => Command::AlbumSearch((!args.is_empty()).then_some(args)),
        "profile" | "lastfmstats" => Command::Profile((!args.is_empty()).then_some(args)),
        "collage" => Command::Collage((!args.is_empty()).then_some(args)),
        "import" => Command::Import((!args.is_empty()).then_some(args)),
        "genres" | "genre" | "analysis" | "жанри" | "аналітика" => {
            Command::GenreStats((!args.is_empty()).then_some(args))
        }
        "lyrics" | "lyric" | "текст" | "лірика" => {
            Command::Lyrics((!args.is_empty()).then_some(args))
        }
        "changelog" | "зміни" => Command::Changelog,
        "me" | "mystats" | "моястатистика" => Command::UserStats,
        "history" | "історія" => Command::History((!args.is_empty()).then_some(args)),
        "dynamics" | "динаміка" => Command::Dynamics((!args.is_empty()).then_some(args)),
        "digest" | "дайджест" => Command::Digest((!args.is_empty()).then_some(args)),
        "broadcast" | "розсилка" => Command::Broadcast((!args.is_empty()).then_some(args)),
        "queue" | "черга" => Command::Queue,
        "cancel" | "скасувати" => Command::Cancel((!args.is_empty()).then_some(args)),
        "stop" | "стоп" => Command::Stop,
        "convert" | "формат" => Command::Convert((!args.is_empty()).then_some(args)),
        "normalize" | "нормалізація" => {
            Command::Normalize((!args.is_empty()).then_some(args))
        }
        "playlist" | "плейлист" => Command::Playlist((!args.is_empty()).then_some(args)),
        "fav" | "favorites" | "обране" | "улюблене" => Command::Favorites,
        "albums" | "альбоми" => Command::Albums,
        "recognize" | "recognise" | "розпізнати" => Command::Recognize,
        "similar" | "схожі" => Command::Similar,
        "artist" | "виконавець" => Command::Artist((!args.is_empty()).then_some(args)),
        "cover" | "обкладинка" => Command::Cover,
        "groupmode" | "група" => Command::GroupMode((!args.is_empty()).then_some(args)),
        "ban" => Command::Ban(args.parse::<i64>().ok()),
        "unban" => Command::Unban(args.parse::<i64>().ok()),
        "purge" => Command::Purge((!args.is_empty()).then_some(args)),
        other => Command::Unknown(other.to_owned()),
    };
    Some(command)
}

pub fn settings_keyboard(
    lang: Lang,
    quality: u32,
    search_source: Provider,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = [128u32, 192, 256, 320]
        .chunks(2)
        .map(|chunk| {
            chunk
                .iter()
                .map(|value| {
                    let check = if *value == quality { "✓ " } else { "" };
                    InlineKeyboardButton::callback(
                        format!("{check}{value} kbps"),
                        format!("s:q:{value}"),
                    )
                })
                .collect()
        })
        .collect();
    rows.push(vec![InlineKeyboardButton::callback(
        format!(
            "{}{}",
            if quality == 0 { "✓ " } else { "" },
            i18n::quality_lossless_label(lang)
        ),
        "s:q:0",
    )]);
    rows.push(
        Provider::SEARCH_CHOICES
            .into_iter()
            .map(|candidate| {
                let check = if candidate == search_source {
                    "✓ "
                } else {
                    ""
                };
                InlineKeyboardButton::callback(
                    format!("{check}{}", candidate.display_name()),
                    format!("s:src:{}", candidate.as_str()),
                )
            })
            .collect(),
    );
    rows.push(vec![InlineKeyboardButton::callback(
        i18n::additional_menu_button(lang),
        "s:advmenu",
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        format!(
            "{} {}",
            i18n::language_menu_button(lang),
            i18n::language_button(lang)
        ),
        "s:langmenu",
    )]);
    InlineKeyboardMarkup::new(rows)
}

/// Advanced settings submenu: the rarely-used toggles live here so the main
/// /settings view stays compact. `s:advback` returns to /settings.
pub fn additional_keyboard(
    lang: Lang,
    tags_enabled: bool,
    hq_cover: bool,
    lastfm_stats: bool,
    genre_analysis: bool,
    album_info: bool,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    rows.push(vec![InlineKeyboardButton::callback(
        format!(
            "{}{}",
            if tags_enabled { "✓ " } else { "" },
            i18n::tags_label(lang)
        ),
        format!("s:tags:{}", u8::from(!tags_enabled)),
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        format!(
            "{}{}",
            if hq_cover { "✓ " } else { "" },
            i18n::hq_cover_label(lang)
        ),
        format!("s:hq:{}", u8::from(!hq_cover)),
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        format!(
            "{}{}",
            if lastfm_stats { "✓ " } else { "" },
            i18n::lastfm_stats_label(lang)
        ),
        format!("s:lstats:{}", u8::from(!lastfm_stats)),
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        format!(
            "{}{}",
            if genre_analysis { "✓ " } else { "" },
            i18n::genre_analysis_label(lang)
        ),
        format!("s:gan:{}", u8::from(!genre_analysis)),
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        format!(
            "{}{}",
            if album_info { "✓ " } else { "" },
            i18n::album_info_label(lang)
        ),
        format!("s:ain:{}", u8::from(!album_info)),
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        i18n::genre_chart_label(lang),
        "gn:c:10",
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        i18n::autodelete_label(lang),
        "s:admenu",
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        i18n::back_button(lang),
        "s:advback",
    )]);
    InlineKeyboardMarkup::new(rows)
}

/// Separate language submenu: every offered language gets its own button and
/// the current choice is marked. `s:langback` returns to `/settings`.
pub fn language_keyboard(lang: Lang) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Lang::ALL
        .chunks(2)
        .map(|chunk| {
            chunk
                .iter()
                .map(|candidate| {
                    let mark = if *candidate == lang { "✓ " } else { "" };
                    InlineKeyboardButton::callback(
                        format!("{mark}{}", candidate.native_name()),
                        format!("s:lang:{}", candidate.code()),
                    )
                })
                .collect()
        })
        .collect();
    rows.push(vec![InlineKeyboardButton::callback(
        i18n::back_button(lang),
        "s:langback",
    )]);
    InlineKeyboardMarkup::new(rows)
}

pub async fn handle_command(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    command: Command,
) -> HandlerResult {
    let Some(user) = msg.from.as_ref() else {
        return Ok(());
    };
    let user_id = user.id.0 as i64;
    let preferred =
        Lang::from_telegram_hint(user.language_code.as_deref(), state.config.default_lang);
    let user_record = state.db.ensure_user(user_id, preferred).await?;
    let lang = user_record.lang;
    let is_admin = state.config.is_admin(user_id);

    if user_record.banned && !is_admin {
        send_plain(bot, msg.chat.id, i18n::banned(lang)).await?;
        return Ok(());
    }

    match command {
        Command::Start => {
            let text = personalize(i18n::start(lang), &state.bot_username);
            send_plain(bot, msg.chat.id, text).await?;
        }
        Command::Help | Command::Unknown(_) => {
            let text = personalize(i18n::help(lang, is_admin), &state.bot_username);
            send_plain(bot, msg.chat.id, text).await?;
        }
        Command::Settings => {
            let mut body = i18n::settings_title(
                lang,
                user_record.quality,
                lang,
                user_record.search_source,
                user_record.tags_enabled,
                user_record.lastfm_user.as_deref(),
                user_record.hq_cover,
                user_record.lastfm_stats,
                user_record.genre_analysis,
            );
            if !state
                .config
                .search_providers
                .contains(&user_record.search_source)
            {
                body.push_str("\n\n");
                body.push_str(i18n::search_source_blocked(lang));
            }
            bot.send_message(msg.chat.id, body)
                .parse_mode(ParseMode::Html)
                .reply_markup(settings_keyboard(
                    lang,
                    user_record.quality,
                    user_record.search_source,
                ))
                .await?;
        }
        Command::Stats => {
            if !is_admin {
                send_plain(bot, msg.chat.id, i18n::admin_only(lang)).await?;
                return Ok(());
            }
            let stats = state.db.stats().await?;
            let text = i18n::stats(lang, &stats, &state.uptime());
            send_plain(bot, msg.chat.id, text).await?;
        }
        Command::Ban(target) => {
            if !is_admin {
                send_plain(bot, msg.chat.id, i18n::admin_only(lang)).await?;
                return Ok(());
            }
            let Some(target) = target else {
                send_plain(bot, msg.chat.id, i18n::invalid_user_id(lang)).await?;
                return Ok(());
            };
            state.db.set_banned(target, true).await?;
            send_plain(bot, msg.chat.id, i18n::ban_ok(lang, target, true)).await?;
        }
        Command::Unban(target) => {
            if !is_admin {
                send_plain(bot, msg.chat.id, i18n::admin_only(lang)).await?;
                return Ok(());
            }
            let Some(target) = target else {
                send_plain(bot, msg.chat.id, i18n::invalid_user_id(lang)).await?;
                return Ok(());
            };
            state.db.set_banned(target, false).await?;
            send_plain(bot, msg.chat.id, i18n::ban_ok(lang, target, false)).await?;
        }
        Command::LastFm(arg) => {
            if state.config.lastfm_api_key.is_none() {
                send_plain(bot, msg.chat.id, i18n::lastfm_disabled(lang)).await?;
                return Ok(());
            }
            match arg.as_deref() {
                None => {
                    let text = i18n::lastfm_status(lang, user_record.lastfm_user.as_deref());
                    send_plain(bot, msg.chat.id, text).await?;
                }
                Some(value) => {
                    let value = value.trim();
                    if matches!(
                        value.to_ascii_lowercase().as_str(),
                        "off" | "reset" | "none"
                    ) {
                        state.db.set_lastfm_user(user_id, None).await?;
                        send_plain(bot, msg.chat.id, i18n::lastfm_disconnected(lang)).await?;
                        return Ok(());
                    }
                    if !is_valid_lastfm_username(value) {
                        send_plain(bot, msg.chat.id, i18n::lastfm_invalid(lang)).await?;
                        return Ok(());
                    }
                    // Live check through the operator's API key: don't save
                    // nicks that Last.fm can't resolve.
                    let Some(client) = state.lastfm.as_ref() else {
                        send_plain(bot, msg.chat.id, i18n::lastfm_disabled(lang)).await?;
                        return Ok(());
                    };
                    match client.now_playing(value).await {
                        Ok(Some(now)) => {
                            state.db.set_lastfm_user(user_id, Some(value)).await?;
                            let title = now.meta.display_title();
                            send_plain(
                                bot,
                                msg.chat.id,
                                i18n::lastfm_connected_track(lang, value, &title),
                            )
                            .await?;
                        }
                        Ok(None) => {
                            state.db.set_lastfm_user(user_id, Some(value)).await?;
                            send_plain(bot, msg.chat.id, i18n::lastfm_no_activity(lang, value))
                                .await?;
                        }
                        Err(error) => {
                            warn!(%error, nick = value, "lastfm rejected the nick at connect time");
                            send_plain(bot, msg.chat.id, i18n::lastfm_not_found(lang)).await?;
                        }
                    }
                }
            }
        }
        Command::Profile(period) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            crate::bot::stats::handle_profile(state, bot, msg, period.as_deref(), lang).await?;
        }
        Command::Collage(args) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            crate::bot::stats::handle_collage(state, bot, msg, args.as_deref(), lang).await?;
        }
        Command::Import(content) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            crate::bot::import::handle_import_args(state, bot, msg, content.as_deref(), lang)
                .await?;
        }
        Command::GenreStats(args) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            crate::bot::stats::handle_genres(state, bot, msg, args.as_deref(), lang).await?;
        }
        Command::Lyrics(query) => {
            crate::bot::lyrics::handle_lyrics(state, bot, msg, query, lang).await?;
        }
        Command::Changelog => {
            let mut text = i18n::changelog_title(lang).to_owned();
            text.push_str(&crate::bot::handlers::escape_html(
                common::version::PATCH_NOTES,
            ));
            send_plain(bot, msg.chat.id, text).await?;
        }
        Command::UserStats => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            let stats = state.db.user_stats(user_id).await?;
            send_plain(
                bot,
                msg.chat.id,
                i18n::user_stats(
                    lang,
                    stats.tracks,
                    stats.listens,
                    stats.genres,
                    stats.seconds,
                ),
            )
            .await?;
        }
        Command::History(arg) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            crate::bot::history::handle_history(state, bot, msg, arg.as_deref(), lang).await?;
        }
        Command::Dynamics(arg) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            crate::bot::history::handle_dynamics(state, bot, msg, arg.as_deref(), lang).await?;
        }
        Command::Digest(arg) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            crate::bot::digest::handle_digest(state, bot, msg, arg.as_deref(), lang).await?;
        }
        Command::Broadcast(text) => {
            if !is_admin {
                send_plain(bot, msg.chat.id, i18n::admin_only(lang)).await?;
                return Ok(());
            }
            let Some(text) = text.filter(|text| !text.trim().is_empty()) else {
                send_plain(bot, msg.chat.id, i18n::broadcast_usage(lang)).await?;
                return Ok(());
            };
            let body = crate::bot::handlers::escape_html(text.trim());
            let mut sent = 0usize;
            let mut failed = 0usize;
            for (target, _) in state.db.active_user_ids().await? {
                match send_plain(bot, ChatId(target), body.clone()).await {
                    Ok(()) => sent += 1,
                    Err(error) => {
                        failed += 1;
                        warn!(%error, user_id = target, "broadcast delivery failed");
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            }
            send_plain(bot, msg.chat.id, i18n::broadcast_result(lang, sent, failed)).await?;
        }
        Command::Queue => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            let jobs = state.db.list_jobs(user_id, 10).await?;
            let avg_secs = state.db.avg_recent_download_secs().await?;
            let active = jobs
                .iter()
                .filter(|(_, _, status, _)| matches!(status.as_str(), "downloading" | "uploading"))
                .count() as u32;
            let mut queued_position = 0u32;
            let mut text = i18n::queue_title(lang).to_owned();
            if jobs.is_empty() {
                text.push_str(i18n::queue_empty(lang));
            } else {
                for (id, kind, status, cancelling) in jobs {
                    let mut line = i18n::queue_item(lang, &id, &kind, &status, cancelling);
                    if status == "queued" {
                        queued_position += 1;
                        if let Some(avg) = avg_secs {
                            // Active jobs finish first, then the queued ones in order.
                            let minutes = eta_minutes(avg, active, queued_position);
                            line.push(' ');
                            line.push_str(&i18n::queue_eta(lang, minutes));
                        }
                    }
                    text.push_str(&line);
                }
            }
            text.push_str(&format!("\n— {}", common::version::DEV_CHANNEL_HANDLE));
            send_plain(bot, msg.chat.id, text).await?;
        }
        Command::Cancel(id) => {
            let Some(id) = id.filter(|value| !value.trim().is_empty()) else {
                send_plain(bot, msg.chat.id, i18n::cancel_usage(lang)).await?;
                return Ok(());
            };
            let ok = state.db.request_cancel(id.trim(), user_id).await?;
            send_plain(bot, msg.chat.id, i18n::cancel_result(lang, ok)).await?;
        }
        Command::Stop => {
            let jobs = state.db.cancel_all_jobs_for_user(user_id).await?;
            let prompts = state.db.clear_user_selections(user_id).await?;
            send_plain(bot, msg.chat.id, i18n::stop_result(lang, jobs, prompts)).await?;
        }
        Command::Convert(format) => {
            let Some(value) = format
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| matches!(value.as_str(), "auto" | "m4a" | "mp3" | "flac" | "opus"))
            else {
                send_plain(bot, msg.chat.id, i18n::convert_usage(lang)).await?;
                return Ok(());
            };

            // Replying to a bot-sent audio converts *that* track; a bare
            // `/convert <fmt>` still only changes the default for new ones.
            let reply_file_id = msg.reply_to_message().and_then(|m| {
                m.audio()
                    .map(|audio| audio.file.id.0.clone())
                    .or_else(|| m.document().map(|doc| doc.file.id.0.clone()))
            });
            if let Some(file_id) = reply_file_id {
                let Some(cached) = state.db.track_by_file_id(&file_id).await? else {
                    send_plain(bot, msg.chat.id, i18n::convert_reply_no_track(lang)).await?;
                    return Ok(());
                };
                let Some(url) = cached.source_url.clone() else {
                    send_plain(bot, msg.chat.id, i18n::convert_reply_no_track(lang)).await?;
                    return Ok(());
                };
                state.db.set_audio_format(user_id, &value).await?;
                let request = common::domain::TrackRequest {
                    key: cached.key.clone(),
                    provider: cached.provider,
                    source_url: Some(url.clone()),
                    meta: Some(cached.meta.clone()),
                    target: common::domain::DownloadTarget::Url(url),
                };
                let duration = cached.meta.duration_secs.map(f64::from);
                let rate = state.db.download_rate().await?;
                let avg = state.db.avg_recent_download_secs().await?;
                let secs = match (rate, duration) {
                    (Some(rate), Some(duration)) => Some(rate * duration),
                    _ => avg,
                };
                let base = i18n::downloading(lang).to_owned();
                let text = if let Some(secs) = secs {
                    format!("{} {}", base, i18n::download_eta(lang, secs))
                } else {
                    base.clone()
                };
                let progress = bot.send_message(msg.chat.id, text).await?;
                let countdown = secs.map(|secs| {
                    spawn_download_countdown(
                        bot,
                        msg.chat.id,
                        progress.id,
                        base,
                        (secs.ceil() as u64).max(1),
                        lang,
                        false,
                    )
                });
                let result = crate::bot::handlers::deliver_track(
                    state,
                    bot,
                    msg.chat.id,
                    request,
                    user_record.quality,
                    user_id,
                    user_record.tags_enabled,
                    user_record.hq_cover,
                    user_record.genre_analysis,
                )
                .await;
                if let Some(handle) = &countdown {
                    handle.abort();
                    // Let any in-flight ticker edit resolve so the final edit lands last.
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                let text = match result {
                    Ok(_) => i18n::convert_reply_done(lang, &value),
                    Err(error) => {
                        i18n::convert_reply_failed(lang, &escape_html(&error.to_string()))
                    }
                };
                let _ = crate::bot::handlers::edit_html(bot, msg.chat.id, progress.id, text).await;
                return Ok(());
            }

            state.db.set_audio_format(user_id, &value).await?;
            send_plain(
                bot,
                msg.chat.id,
                i18n::convert_saved(lang, &value, user_record.normalize_audio),
            )
            .await?;
        }
        Command::Favorites => {
            let rows = state.db.favorites(user_id, 30).await?;
            // One batch fetch instead of up to 30 sequential get_cached SELECTs.
            // Dedup per key; favorites() already returns newest-first order.
            let mut seen = std::collections::HashSet::new();
            let keys: Vec<common::domain::TrackKey> = rows
                .iter()
                .filter(|&(key, _, _)| seen.insert(key.as_str()))
                .map(|(key, _, _)| common::domain::TrackKey(key.clone()))
                .collect();
            let cached = state.db.tracks_for_keys(&keys, user_record.quality).await?;
            let by_key: std::collections::HashMap<&str, &common::domain::CachedTrack> = cached
                .iter()
                .map(|track| (track.key.as_str(), track))
                .collect();
            let mut buttons: Vec<Vec<InlineKeyboardButton>> = Vec::with_capacity(rows.len());
            let mut selections: Vec<(String, common::domain::TrackRequest)> =
                Vec::with_capacity(rows.len());
            for (key, _title, _performer) in rows {
                // Only favorites with a live Telegram file_id can be replayed;
                // the rest silently drop off the list until re-downloaded.
                let Some(cached) = by_key.get(key.as_str()) else {
                    continue;
                };
                let request = common::domain::TrackRequest {
                    key: cached.key.clone(),
                    provider: cached.provider,
                    source_url: cached.source_url.clone(),
                    meta: Some(cached.meta.clone()),
                    target: common::domain::DownloadTarget::Search(cached.meta.search_query()),
                };
                let selection_id = state.new_selection_id();
                let label = truncate(&cached.meta.display_title(), 60);
                buttons.push(vec![InlineKeyboardButton::callback(
                    format!("⭐ {label}"),
                    format!("q:{selection_id}"),
                )]);
                selections.push((selection_id, request));
            }
            if buttons.is_empty() {
                send_plain(bot, msg.chat.id, i18n::favorites_empty(lang)).await?;
                return Ok(());
            }
            let selection_refs: Vec<(String, &common::domain::TrackRequest)> = selections
                .iter()
                .map(|(id, request)| (id.clone(), request))
                .collect();
            state
                .put_selections(crate::state::selection::TRACK, user_id, &selection_refs)
                .await?;
            bot.send_message(msg.chat.id, i18n::favorites_title(lang))
                .reply_markup(InlineKeyboardMarkup::new(buttons))
                .await?;
        }
        Command::Albums => {
            let Some((text, markup)) = albums_page(state, user_id, lang, 0).await? else {
                send_plain(bot, msg.chat.id, i18n::albums_empty(lang)).await?;
                return Ok(());
            };
            bot.send_message(msg.chat.id, text)
                .parse_mode(ParseMode::Html)
                .reply_markup(markup)
                .await?;
        }
        Command::Recognize => {
            crate::bot::recognize::handle_recognize(state, bot, msg, &user_record, user_id).await?;
        }
        Command::Similar => {
            let reply_file_id = msg.reply_to_message().and_then(|m| {
                m.audio()
                    .map(|audio| audio.file.id.0.clone())
                    .or_else(|| m.document().map(|doc| doc.file.id.0.clone()))
            });
            let Some(file_id) = reply_file_id else {
                send_plain(bot, msg.chat.id, i18n::similar_usage(lang)).await?;
                return Ok(());
            };
            let Some(cached) = state.db.track_by_file_id(&file_id).await? else {
                send_plain(bot, msg.chat.id, i18n::similar_usage(lang)).await?;
                return Ok(());
            };
            let Some(artist) = cached
                .meta
                .performer
                .as_deref()
                .map(str::trim)
                .filter(|artist| !artist.is_empty())
            else {
                send_plain(bot, msg.chat.id, i18n::similar_usage(lang)).await?;
                return Ok(());
            };
            // Give immediate feedback: uncached Last.fm + artwork lookups can
            // take ~12 s, so show the standard searching placeholder now and
            // replace it with the result below.
            let progress = bot.send_message(msg.chat.id, i18n::searching(lang)).await?;
            let similar = match &state.lastfm {
                Some(client) => client
                    .similar_tracks(artist, &cached.meta.title)
                    .await
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            if similar.is_empty() {
                let _ = crate::bot::handlers::edit_html(
                    bot,
                    msg.chat.id,
                    progress.id,
                    i18n::similar_none(lang),
                )
                .await;
                return Ok(());
            }
            // Delivery only downloads a cover when the request carries
            // thumbnail_url (search picks always do); Last.fm gives us no
            // images, so resolve each similar track through the Deezer catalog.
            // Resolve them in parallel: sequential lookups cost 6 s each and
            // used to stall /similar for up to a minute. Collect by index so
            // the button order stays deterministic.
            let queries: Vec<String> = similar
                .iter()
                .map(|(artist, title)| format!("{artist} {title}"))
                .collect();
            let mut results: Vec<common::domain::SearchResult> = similar
                .iter()
                .zip(&queries)
                .map(|((artist, title), query)| common::domain::SearchResult {
                    key: common::domain::TrackKey::new(format!("similar:{artist} - {title}")),
                    provider: common::domain::Provider::YouTube,
                    source_url: String::new(),
                    meta: common::domain::TrackMeta {
                        title: title.clone(),
                        performer: Some(artist.clone()),
                        thumbnail_url: None,
                        ..Default::default()
                    },
                    target: common::domain::DownloadTarget::Search(query.clone()),
                })
                .collect();
            let mut resolves = JoinSet::new();
            let resolve_semaphore = Arc::new(Semaphore::new(COVER_PARALLELISM));
            for (index, query) in queries.iter().enumerate() {
                let cache_key = format!(
                    "{}:{}",
                    providers::external::ArtworkKind::Track.as_str(),
                    query.trim().to_lowercase()
                );
                if let Some(cached) = state.artwork_cache.get(&cache_key).await {
                    results[index].meta.thumbnail_url = cached;
                    continue;
                }
                let state = state.clone();
                let semaphore = resolve_semaphore.clone();
                let query = query.clone();
                resolves.spawn(async move {
                    let _permit = semaphore.acquire().await.ok()?;
                    let resolved = state
                        .manager
                        .artwork_url(providers::external::ArtworkKind::Track, &query)
                        .await;
                    state
                        .artwork_cache
                        .insert(cache_key, resolved.clone())
                        .await;
                    Some((index, resolved))
                });
            }
            while let Some(joined) = resolves.join_next().await {
                if let Ok(Some((index, resolved))) = joined {
                    results[index].meta.thumbnail_url = resolved;
                }
            }
            let keys: Vec<String> = results
                .iter()
                .map(|result| result.key.to_string())
                .collect();
            let heard = state
                .db
                .heard_before(user_id, &keys)
                .await
                .unwrap_or_default();
            let rows =
                match crate::bot::handlers::search_result_rows(state, user_id, &results, &heard, 0)
                    .await
                {
                    Ok(rows) => rows,
                    Err(error) => {
                        // Don't leave the searching placeholder behind on failure.
                        let _ = bot.delete_message(msg.chat.id, progress.id).await;
                        return Err(error.into());
                    }
                };
            let subject = escape_html(&cached.meta.display_title());
            let header = i18n::similar_title(lang, &subject);
            let markup = InlineKeyboardMarkup::new(rows);
            let edited = bot
                .edit_message_text(msg.chat.id, progress.id, header.clone())
                .parse_mode(ParseMode::Html)
                .reply_markup(markup.clone())
                .await;
            if edited.is_err() {
                let _ = bot.delete_message(msg.chat.id, progress.id).await;
                bot.send_message(msg.chat.id, header)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(markup)
                    .await?;
            }
        }
        Command::Artist(name) => {
            let Some(name) = name
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
            else {
                send_plain(bot, msg.chat.id, i18n::artist_usage(lang)).await?;
                return Ok(());
            };
            let progress = bot.send_message(msg.chat.id, i18n::searching(lang)).await?;
            match state.manager.artist_top(&name, 8).await {
                Ok(Some(results)) if !results.is_empty() => {
                    let keys: Vec<String> = results
                        .iter()
                        .map(|result| result.key.to_string())
                        .collect();
                    let heard = state
                        .db
                        .heard_before(user_id, &keys)
                        .await
                        .unwrap_or_default();
                    let rows = crate::bot::handlers::search_result_rows(
                        state, user_id, &results, &heard, 0,
                    )
                    .await?;
                    let header = i18n::artist_title(lang, &escape_html(&name));
                    let markup = InlineKeyboardMarkup::new(rows);
                    let edited = bot
                        .edit_message_text(msg.chat.id, progress.id, header.clone())
                        .parse_mode(ParseMode::Html)
                        .reply_markup(markup.clone())
                        .await;
                    if edited.is_err() {
                        let _ = bot.delete_message(msg.chat.id, progress.id).await;
                        bot.send_message(msg.chat.id, header)
                            .parse_mode(ParseMode::Html)
                            .reply_markup(markup)
                            .await?;
                    }
                }
                _ => {
                    let _ = crate::bot::handlers::edit_html(
                        bot,
                        msg.chat.id,
                        progress.id,
                        i18n::artist_none(lang),
                    )
                    .await;
                }
            }
        }
        Command::Cover => {
            let reply_file_id = msg.reply_to_message().and_then(|m| {
                m.audio()
                    .map(|audio| audio.file.id.0.clone())
                    .or_else(|| m.document().map(|doc| doc.file.id.0.clone()))
            });
            let Some(file_id) = reply_file_id else {
                send_plain(bot, msg.chat.id, i18n::cover_usage(lang)).await?;
                return Ok(());
            };
            let Some(cached) = state.db.track_by_file_id(&file_id).await? else {
                send_plain(bot, msg.chat.id, i18n::cover_usage(lang)).await?;
                return Ok(());
            };
            let Some(url) = cached
                .meta
                .thumbnail_url
                .as_deref()
                .map(str::trim)
                .filter(|url| !url.is_empty())
            else {
                send_plain(bot, msg.chat.id, i18n::cover_none(lang)).await?;
                return Ok(());
            };
            let url = crate::bot::handlers::hq_cover_url(url);
            match providers::http::fetch_bytes(&state.http, &url, 8 * 1024 * 1024).await {
                Some(bytes) => {
                    bot.send_photo(msg.chat.id, InputFile::memory(bytes))
                        .await?;
                }
                None => {
                    send_plain(bot, msg.chat.id, i18n::cover_none(lang)).await?;
                }
            }
        }
        Command::Normalize(value) => {
            let Some(enabled) = value.as_deref().and_then(|value| {
                match value.trim().to_ascii_lowercase().as_str() {
                    "on" | "true" | "1" | "увімк" => Some(true),
                    "off" | "false" | "0" | "вимк" => Some(false),
                    _ => None,
                }
            }) else {
                send_plain(bot, msg.chat.id, i18n::normalize_usage(lang)).await?;
                return Ok(());
            };
            state.db.set_normalize_audio(user_id, enabled).await?;
            send_plain(
                bot,
                msg.chat.id,
                i18n::convert_saved(lang, &user_record.audio_format, enabled),
            )
            .await?;
        }
        Command::GroupMode(value) => {
            // The static ADMIN_IDS list is not enough: a group owner/admin must
            // be able to opt the group in. Query Telegram for chat privileges.
            let chat_admin = matches!(
                bot.get_chat_member(msg.chat.id, user.id).await,
                Ok(member) if member.is_privileged()
            );
            if !is_admin && !chat_admin {
                send_plain(bot, msg.chat.id, i18n::admin_only(lang)).await?;
                return Ok(());
            }
            let Some(enabled) = value.as_deref().and_then(|value| {
                match value.trim().to_ascii_lowercase().as_str() {
                    "on" | "true" | "1" | "увімк" => Some(true),
                    "off" | "false" | "0" | "вимк" => Some(false),
                    _ => None,
                }
            }) else {
                send_plain(bot, msg.chat.id, i18n::groupmode_usage(lang)).await?;
                return Ok(());
            };
            state
                .db
                .set_chat_enabled(msg.chat.id.0, enabled, user_id)
                .await?;
            send_plain(bot, msg.chat.id, i18n::groupmode_result(lang, enabled)).await?;
        }
        Command::Playlist(query) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            let Some(query) = query
                .as_deref()
                .map(str::trim)
                .filter(|query| !query.is_empty())
            else {
                send_plain(bot, msg.chat.id, i18n::playlist_usage(lang)).await?;
                return Ok(());
            };
            let progress = bot
                .send_message(msg.chat.id, i18n::playlist_searching(lang))
                .await?;
            let candidates = match state.manager.search_playlists(query, 6).await {
                Ok(candidates) => candidates,
                Err(error) => {
                    let _ = bot.delete_message(msg.chat.id, progress.id).await;
                    warn!(%error, "/playlist catalog search failed");
                    send_plain(bot, msg.chat.id, i18n::error(lang, &error)).await?;
                    return Ok(());
                }
            };
            if candidates.is_empty() {
                bot.edit_message_text(msg.chat.id, progress.id, i18n::no_playlists_found(lang))
                    .await?;
                return Ok(());
            }
            let mut rows = Vec::with_capacity(candidates.len());
            for candidate in &candidates {
                let selection_id = state.new_selection_id();
                state
                    .put_selection(
                        &selection_id,
                        crate::state::selection::ALBUM_REF,
                        user_id,
                        &candidate.into_album_ref(),
                    )
                    .await?;
                rows.push(vec![InlineKeyboardButton::callback(
                    crate::bot::handlers::truncate(&album_button_label(lang, candidate), 60),
                    format!("alq:{selection_id}"),
                )]);
            }
            let header = i18n::playlist_results(
                lang,
                &crate::bot::handlers::escape_html(query),
                candidates.len(),
            );
            let keyboard = InlineKeyboardMarkup::new(rows);
            if let Err(error) = bot
                .edit_message_text(msg.chat.id, progress.id, header.clone())
                .parse_mode(ParseMode::Html)
                .reply_markup(keyboard.clone())
                .await
            {
                let _ = bot.delete_message(msg.chat.id, progress.id).await;
                warn!(%error, "could not edit playlist list; sending a new message");
                bot.send_message(msg.chat.id, header)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        Command::AlbumSearch(query) => {
            if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
                send_plain(bot, msg.chat.id, i18n::private_only(lang)).await?;
                return Ok(());
            }
            let Some(query) = query
                .as_deref()
                .map(str::trim)
                .filter(|query| !query.is_empty())
            else {
                send_plain(bot, msg.chat.id, i18n::album_search_usage(lang)).await?;
                return Ok(());
            };
            let progress = bot
                .send_message(msg.chat.id, i18n::album_searching(lang))
                .await?;
            let candidates = match state.manager.search_albums(query, 6).await {
                Ok(candidates) => candidates,
                Err(error) => {
                    let _ = bot.delete_message(msg.chat.id, progress.id).await;
                    warn!(%error, "/alb catalog search failed");
                    send_plain(bot, msg.chat.id, i18n::error(lang, &error)).await?;
                    return Ok(());
                }
            };
            if candidates.is_empty() {
                bot.edit_message_text(msg.chat.id, progress.id, i18n::no_albums_found(lang))
                    .await?;
                return Ok(());
            }
            let mut rows = Vec::with_capacity(candidates.len());
            for candidate in &candidates {
                let selection_id = state.new_selection_id();
                state
                    .put_selection(
                        &selection_id,
                        crate::state::selection::ALBUM_REF,
                        user_id,
                        &candidate.into_album_ref(),
                    )
                    .await?;
                rows.push(vec![InlineKeyboardButton::callback(
                    album_button_label(lang, candidate),
                    format!("alq:{selection_id}"),
                )]);
            }
            let keyboard = InlineKeyboardMarkup::new(rows);
            let header =
                i18n::album_search_results(lang, &crate::bot::handlers::escape_html(query));
            let edited = bot
                .edit_message_text(msg.chat.id, progress.id, header.clone())
                .parse_mode(ParseMode::Html)
                .reply_markup(keyboard.clone())
                .await;
            if edited.is_err() {
                let _ = bot.delete_message(msg.chat.id, progress.id).await;
                bot.send_message(msg.chat.id, header)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;
            }
        }
        Command::Purge(input) => {
            if !is_admin {
                send_plain(bot, msg.chat.id, i18n::admin_only(lang)).await?;
                return Ok(());
            }
            let Some(input) = input else {
                send_plain(bot, msg.chat.id, i18n::invalid_purge_usage(lang)).await?;
                return Ok(());
            };
            let key = key_from_input(&input);
            let removed = state.manager.purge(&key).await?;
            let text = if removed {
                i18n::purge_ok(lang, &escape_html(key.as_str()))
            } else {
                i18n::purge_missing(lang).to_owned()
            };
            send_plain(bot, msg.chat.id, text).await?;
        }
    }
    Ok(())
}

/// Button label for an `/alb` candidate: "Artist — Title · N tracks · Deezer".
fn album_button_label(lang: Lang, candidate: &common::domain::AlbumSearchResult) -> String {
    let artist = candidate
        .artist
        .as_deref()
        .map(str::trim)
        .filter(|artist| !artist.is_empty());
    let mut label = match artist {
        Some(artist) => format!("{artist} — {}", candidate.title),
        None => candidate.title.clone(),
    };
    if let Some(count) = candidate.track_count {
        label.push_str(&i18n::album_tracks_count(lang, count));
    }
    label.push_str(" · ");
    label.push_str(candidate.provider.display_name());
    crate::bot::handlers::truncate(&label, 60)
}

/// Round the estimated wait up to whole minutes, never below one.
fn eta_minutes(avg_secs: f64, active: u32, queued_position: u32) -> u32 {
    ((avg_secs * f64::from(active + queued_position) / 60.0).ceil() as u64).max(1) as u32
}

/// Last.fm usernames: 2–15 chars, latin letters/digits/`_`/`-`.
fn is_valid_lastfm_username(value: &str) -> bool {
    (2..=15).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

async fn send_plain(
    bot: &Bot,
    chat: ChatId,
    text: impl Into<String>,
) -> Result<(), teloxide::RequestError> {
    // Some strings contain HTML markup; only parse those that need it.
    let text = text.into();
    let result = if text.contains('<') {
        crate::bot::send::with_retry_after(|| {
            bot.send_message(chat, text.clone())
                .parse_mode(ParseMode::Html)
        })
        .await
        .map(|_| ())
    } else {
        crate::bot::send::with_retry_after(|| bot.send_message(chat, text.clone()))
            .await
            .map(|_| ())
    };
    if let Err(error) = result {
        warn!(%error, "could not send command text; retrying without entities");
        // The retry keeps the same payload, so an overlong message would fail
        // again with Api(MessageIsTooLong). Telegram's limit is 4096 UTF-16
        // units, and astral chars (CJK locales use them) take two units each,
        // so a plain char count can overshoot: 2040 chars can never exceed
        // 4096 units.
        const MAX_TEXT_CHARS: usize = 2040;
        let safe = crate::bot::handlers::truncate(&text, MAX_TEXT_CHARS);
        crate::bot::send::with_retry_after(|| bot.send_message(chat, safe.clone()))
            .await
            .map(|_| ())?;
    }
    Ok(())
}

fn personalize(text: impl Into<String>, username: &str) -> String {
    if username.is_empty() {
        text.into()
    } else {
        text.into().replace("@this_bot", &format!("@{username}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_album_and_recognize_aliases() {
        assert_eq!(parse_command("/albums", "bot"), Some(Command::Albums));
        assert_eq!(parse_command("/альбоми", "bot"), Some(Command::Albums));
        assert_eq!(parse_command("/recognize", "bot"), Some(Command::Recognize));
        assert_eq!(
            parse_command("/розпізнати", "bot"),
            Some(Command::Recognize)
        );
    }

    #[test]
    fn queue_eta_rounds_up_and_floors_at_one() {
        assert_eq!(eta_minutes(60.0, 1, 2), 3);
        assert_eq!(eta_minutes(0.0, 0, 1), 1);
        assert_eq!(eta_minutes(30.5, 2, 1), 2);
    }

    #[test]
    fn parses_commands_and_suffix() {
        assert_eq!(parse_command("/start", "bot"), Some(Command::Start));
        assert_eq!(
            parse_command("/help@other", "bot"),
            None,
            "commands addressed to another bot are ignored"
        );
        assert_eq!(
            parse_command("/ban 42", "bot"),
            Some(Command::Ban(Some(42)))
        );
        assert_eq!(parse_command("/ban x", "bot"), Some(Command::Ban(None)));
        assert_eq!(
            parse_command("/purge youtube:abc", "bot"),
            Some(Command::Purge(Some("youtube:abc".into())))
        );
        assert_eq!(
            parse_command("/alb master of puppets", "bot"),
            Some(Command::AlbumSearch(Some("master of puppets".into())))
        );
        assert_eq!(
            parse_command("/alb", "bot"),
            Some(Command::AlbumSearch(None))
        );
        assert_eq!(parse_command("/changelog", "bot"), Some(Command::Changelog));
        assert_eq!(parse_command("/me", "bot"), Some(Command::UserStats));
        assert_eq!(
            parse_command("/broadcast hello everyone", "bot"),
            Some(Command::Broadcast(Some("hello everyone".into())))
        );
        assert_eq!(
            parse_command("/lyrics Artist — Title", "bot"),
            Some(Command::Lyrics(Some("Artist — Title".into())))
        );
        assert_eq!(
            parse_command("/history 7d", "bot"),
            Some(Command::History(Some("7d".into())))
        );
        assert_eq!(
            parse_command("/dynamics", "bot"),
            Some(Command::Dynamics(None))
        );
        assert_eq!(
            parse_command("/digest quiet 23-7", "bot"),
            Some(Command::Digest(Some("quiet 23-7".into())))
        );
    }
}
