//! Command parsing and handlers. Commands are parsed manually so `@botname`
//! suffixes and admin arguments behave predictably in groups.

use crate::state::AppState;
use common::domain::Lang;
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
        "ban" => Command::Ban(args.parse::<i64>().ok()),
        "unban" => Command::Unban(args.parse::<i64>().ok()),
        "purge" => Command::Purge((!args.is_empty()).then_some(args)),
        other => Command::Unknown(other.to_owned()),
    };
    Some(command)
}

pub fn settings_keyboard(lang: Lang, quality: u32) -> InlineKeyboardMarkup {
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
    let user_record = state
        .db
        .ensure_user(user_id, state.config.default_lang)
        .await?;
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
            bot.send_message(
                msg.chat.id,
                i18n::settings_title(lang, user_record.quality, lang),
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(settings_keyboard(lang, user_record.quality))
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
    }
}
