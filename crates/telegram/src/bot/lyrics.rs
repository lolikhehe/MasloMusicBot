//! `/lyrics` command: public, token-free lyrics lookup.

use crate::bot::{handlers, send};
use crate::state::AppState;
use common::domain::Lang;
use common::error::HandlerResult;
use common::i18n;
use providers::lyrics::{Lyrics, LyricsSource};
use teloxide::prelude::*;
use teloxide::types::{Message, ParseMode};
use tracing::warn;

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

    let key = format!("{}|{}", artist.as_deref().unwrap_or(""), title).to_lowercase();
    let found = if let Some(cached) = state.lyrics_cache.get(&key).await {
        cached
    } else {
        let fetched = state.lyrics.fetch(artist.as_deref(), &title).await;
        if fetched.is_none() {
            warn!(title = %title, "no lyrics found in public sources");
        }
        state.lyrics_cache.insert(key, fetched.clone()).await;
        fetched
    };

    let Some(Lyrics { text, source }) = found else {
        send::send_html(bot, msg.chat.id, i18n::lyrics_not_found(lang)).await?;
        return Ok(());
    };

    let mut body = i18n::lyrics_header(
        lang,
        source.as_str(),
        matches!(source, LyricsSource::LrclibSynced),
    );
    body.push_str(&handlers::escape_html(&text));
    body.push_str(&i18n::lyrics_footer(lang));
    match bot
        .send_message(msg.chat.id, body)
        .parse_mode(ParseMode::Html)
        .await
    {
        Ok(_) => {}
        Err(error) => {
            warn!(%error, "could not send lyrics with HTML; retrying as plain text");
            let mut plain = format!("📝 {} — {title}\n\n{text}", source.as_str());
            plain.push_str(&i18n::lyrics_footer(lang));
            send::send_html(bot, msg.chat.id, plain).await?;
        }
    }
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
