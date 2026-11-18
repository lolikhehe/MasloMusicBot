//! `/lyrics` command: public, token-free lyrics lookup, with an optional
//! downloadable `.lrc` file and a "other source" button.

use crate::bot::{handlers, send};
use crate::state::AppState;
use common::domain::Lang;
use common::error::HandlerResult;
use common::i18n;
use providers::lyrics::{Lyrics, LyricsSource};
use serde::{Deserialize, Serialize};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, InputFile, Message, ParseMode};

/// Serializable lyrics snapshot stored in Postgres for button callbacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsSelection {
    pub artist: Option<String>,
    pub title: String,
    pub lang: String,
    pub text: String,
    pub source: String,
    pub synced: bool,
    pub lrc: Option<String>,
}

/// Resolve a lyrics query from, in priority order: explicit arguments, a reply
/// to an audio message, the last track sent to this user.
async fn resolve_query(
    state: &AppState,
    msg: &Message,
    query: Option<String>,
) -> Option<(Option<String>, String)> {
    if let Some(query) = query {
        let query = query.trim();
        if !query.is_empty() {
            return Some(split_query(query));
        }
    }

    if let Some(audio) = msg.reply_to_message().and_then(Message::audio)
        && let Some(title) = audio
            .title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
    {
        return Some((audio.performer.clone(), title.trim().to_owned()));
    }

    let user_id = msg.from.as_ref()?;
    let last = state.last_tracks.get(&(user_id.id.0 as i64)).await?;
    Some((last.performer, last.title))
}

/// Accept both `Artist — Title` and plain `Song name`. Only a separator with
/// spaces around it is treated as artist/title so hyphens in names survive.
pub fn split_query(query: &str) -> (Option<String>, String) {
    let query = query.trim();
    for separator in [" — ", " – ", " - ", " | ", " —", " – "] {
        if let Some((artist, title)) = query.split_once(separator) {
            let artist = artist.trim();
            let title = title.trim();
            if !artist.is_empty() && !title.is_empty() {
                return (
                    Some(artist.chars().take(100).collect()),
                    title.chars().take(150).collect(),
                );
            }
        }
    }
    (None, query.chars().take(150).collect())
}

fn selection(artist: Option<&str>, title: &str, lang: Lang, found: &Lyrics) -> LyricsSelection {
    LyricsSelection {
        artist: artist.map(str::to_owned),
        title: title.to_owned(),
        lang: lang.code().to_owned(),
        text: found.text.clone(),
        source: found.source.as_str().to_owned(),
        synced: found.is_synced(),
        lrc: found.lrc.clone(),
    }
}

fn body(selection: &LyricsSelection) -> String {
    let lang = Lang::from_code(&selection.lang).unwrap_or(Lang::En);
    let mut text = i18n::lyrics_header(lang, &selection.source, selection.synced);
    text.push_str(&handlers::escape_html(&selection.text));
    text.push_str(&i18n::lyrics_footer(lang));
    text
}

fn keyboard(
    lang: Lang,
    selection_id: &str,
    has_lrc: bool,
    has_other: bool,
) -> InlineKeyboardMarkup {
    let mut buttons: Vec<InlineKeyboardButton> = Vec::new();
    if has_lrc {
        buttons.push(InlineKeyboardButton::callback(
            i18n::lyrics_lrc_button(lang),
            format!("ly:lrc:{selection_id}"),
        ));
    }
    if has_other {
        buttons.push(InlineKeyboardButton::callback(
            i18n::lyrics_other_source_button(lang),
            format!("ly:alt:{selection_id}"),
        ));
    }
    InlineKeyboardMarkup::new(vec![buttons])
}

pub async fn handle_lyrics(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    query: Option<String>,
    lang: Lang,
) -> HandlerResult {
    let Some((artist, title)) = resolve_query(state, msg, query).await else {
        send::send_html(bot, msg.chat.id, i18n::lyrics_no_last(lang)).await?;
        return Ok(());
    };

    let found = state.lyrics.fetch(artist.as_deref(), &title).await;
    let Some(found) = found else {
        send::send_html(bot, msg.chat.id, i18n::lyrics_not_found(lang)).await?;
        return Ok(());
    };
    send_lyrics(state, bot, msg.chat.id, &artist, &title, lang, &found).await
}

/// Send a lyrics answer and store the buttons' payload in Postgres so any
/// replica can serve the follow-up callback.
pub async fn send_lyrics(
    state: &AppState,
    bot: &Bot,
    chat: ChatId,
    artist: &Option<String>,
    title: &str,
    lang: Lang,
    found: &Lyrics,
) -> HandlerResult {
    let snapshot = selection(artist.as_deref(), title, lang, found);
    let selection_id = state.new_selection_id();
    state
        .put_selection(
            &selection_id,
            crate::state::selection::LYRICS,
            chat.0,
            &snapshot,
        )
        .await?;

    let has_lrc = snapshot.lrc.is_some();
    let has_other = found.source != LyricsSource::LyricsOvh;
    let text = body(&snapshot);
    let result = bot
        .send_message(chat, text.clone())
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard(lang, &selection_id, has_lrc, has_other))
        .await;
    if result.is_err() {
        let mut plain = format!("📝 {} — {title}\n\n{}", snapshot.source, snapshot.text);
        plain.push_str(&i18n::lyrics_footer(lang));
        send::send_html(bot, chat, plain).await?;
    }
    Ok(())
}

pub async fn handle_lrc_callback(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    id: &str,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let Some(saved) = state
        .get_selection::<LyricsSelection>(id, crate::state::selection::LYRICS)
        .await?
    else {
        return Ok(());
    };
    let Some(lrc) = saved.lrc.clone() else {
        return Ok(());
    };
    let lang = Lang::from_code(&saved.lang).unwrap_or(Lang::En);
    let file_name = format!(
        "{}.lrc",
        saved
            .title
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            })
            .collect::<String>()
    );
    let input = InputFile::memory(lrc.into_bytes()).file_name(file_name);
    bot.send_document(message.chat.id, input)
        .caption(i18n::lyrics_lrc_sent(lang))
        .await?;
    Ok(())
}

pub async fn handle_alt_source_callback(
    state: &AppState,
    bot: &Bot,
    query: &CallbackQuery,
    id: &str,
) -> HandlerResult {
    let _ = bot.answer_callback_query(query.id.clone()).await;
    let Some(message) = query.regular_message() else {
        return Ok(());
    };
    let Some(saved) = state
        .get_selection::<LyricsSelection>(id, crate::state::selection::LYRICS)
        .await?
    else {
        return Ok(());
    };
    let lang = Lang::from_code(&saved.lang).unwrap_or(Lang::En);
    let alternate = if saved.source.starts_with("LRCLIB") {
        state
            .lyrics
            .fetch_ovh(saved.artist.as_deref(), &saved.title)
            .await
    } else {
        state
            .lyrics
            .fetch_lrclib(saved.artist.as_deref(), &saved.title)
            .await
    };
    let Some(found) = alternate else {
        bot.edit_message_text(
            message.chat.id,
            message.id,
            i18n::lyrics_no_other_source(lang),
        )
        .await?;
        return Ok(());
    };
    let snapshot = selection(saved.artist.as_deref(), &saved.title, lang, &found);
    let has_lrc = snapshot.lrc.is_some();
    let text = body(&snapshot);
    let _ = bot
        .edit_message_text(message.chat.id, message.id, text)
        .parse_mode(ParseMode::Html)
        // Keep the toggle so the user can switch back to the first source.
        .reply_markup(keyboard(lang, id, has_lrc, true))
        .await;
    state
        .put_selection(
            id,
            crate::state::selection::LYRICS,
            query.from.id.0 as i64,
            &snapshot,
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::split_query;

    #[test]
    fn splits_artist_and_title() {
        assert_eq!(
            split_query("Dark Synapse — Neon Rain"),
            (Some("Dark Synapse".into()), "Neon Rain".into())
        );
        assert_eq!(
            split_query("Jay-Z - Empire State of Mind"),
            (Some("Jay-Z".into()), "Empire State of Mind".into())
        );
        assert_eq!(split_query("Just a song"), (None, "Just a song".into()));
    }
}
