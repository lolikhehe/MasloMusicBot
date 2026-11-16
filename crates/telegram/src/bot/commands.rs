//! Command parsing and handlers. Commands are parsed manually so `@botname`
//! suffixes and admin arguments behave predictably in groups.

use crate::state::AppState;
use common::domain::{Lang, Provider};
use common::error::HandlerResult;
use common::i18n;
use providers::key_from_input;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, Message, ParseMode};
use tracing::warn;

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
    tags_enabled: bool,
    hq_cover: bool,
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
    rows.push(
        [Lang::En, Lang::Uk]
            .into_iter()
            .map(|candidate| {
                let mark = if candidate == lang { "✓ " } else { "" };
                InlineKeyboardButton::callback(
                    format!("{mark}{}", i18n::language_button(candidate)),
                    format!("s:lang:{}", candidate.code()),
                )
            })
            .collect(),
    );
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
                    user_record.tags_enabled,
                    user_record.hq_cover,
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
        Command::AlbumSearch(query) => {
            if !msg.chat.is_private() {
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
                    .album_ref_selections
                    .insert(selection_id.clone(), candidate.into_album_ref())
                    .await;
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
                i18n::purge_ok(lang, key.as_str())
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
        bot.send_message(chat, text.clone())
            .parse_mode(ParseMode::Html)
            .await
    } else {
        bot.send_message(chat, text.clone()).await
    };
    if let Err(error) = result {
        warn!(%error, "could not send command text; retrying without entities");
        bot.send_message(chat, text).await?;
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
    }
}
