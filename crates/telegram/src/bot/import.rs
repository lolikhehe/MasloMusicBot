//! Spotify playlist *transfer* import. We do not use Spotify's API (a playlist
//! track list needs credentials); instead we accept the exports that transfer
//! tools produce: plain `Artist – Title` lines, Spotify privacy-export CSV, or
//! JSON from TuneMyMusic / Soundiiz, and match each entry through the existing
//! public search providers. Entries are matched one at a time to avoid a burst
//! of requests on the operator's network.

use crate::bot::handlers::confirm_album;
use crate::state::AppState;
use common::domain::{Album, Lang, Provider};
use common::error::HandlerResult;
use common::i18n;
use teloxide::net::Download;
use teloxide::prelude::*;
use teloxide::types::{Document, Message, ParseMode};
use tracing::warn;

const MAX_TRACKS: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    artist: Option<String>,
    title: String,
}

/// Minimal CSV reader with quote support — enough for Spotify/TuneMyMusic
/// exports, without pulling a whole crate into the binary.
fn csv_rows(input: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if quoted && chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = !quoted;
                }
            }
            ',' if !quoted => row.push(std::mem::take(&mut field)),
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                if row.iter().any(|value| !value.trim().is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            '\r' => {}
            _ => field.push(ch),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

fn title_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "title" | "name" | "track" | "track_name" | "trackname" | "song"
    )
}

fn artist_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "artist" | "artist_name" | "artistname" | "artists" | "performer"
    )
}

fn from_csv(input: &str) -> Vec<Entry> {
    let rows = csv_rows(input);
    let Some(header) = rows.first() else {
        return Vec::new();
    };
    let title_index = header.iter().position(|key| title_key(key));
    let artist_index = header.iter().position(|key| artist_key(key));
    let (title_index, artist_index, skip_header) = match (title_index, artist_index) {
        (Some(title), artist) => (title, artist, true),
        (None, _) => (0, if header.len() > 1 { Some(1) } else { None }, false),
    };
    rows.into_iter()
        .skip(usize::from(skip_header))
        .filter_map(|row| {
            let title = row.get(title_index)?.trim().to_owned();
            if title.is_empty() {
                return None;
            }
            let artist = artist_index
                .and_then(|index| row.get(index))
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
            Some(Entry { artist, title })
        })
        .collect()
}

fn collect_json(value: &serde_json::Value, out: &mut Vec<Entry>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            let title = map
                .iter()
                .find(|(key, _)| title_key(key))
                .and_then(|(_, value)| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some(title) = title {
                let artist = map
                    .iter()
                    .find(|(key, _)| artist_key(key))
                    .and_then(|(_, value)| match value {
                        serde_json::Value::String(text) => Some(text.as_str()),
                        serde_json::Value::Object(_) => {
                            value.get("name").and_then(|name| name.as_str())
                        }
                        serde_json::Value::Array(_) => value
                            .get(0)
                            .and_then(|first| first.get("name").or(Some(first)))
                            .and_then(|item| item.as_str()),
                        _ => None,
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                out.push(Entry {
                    artist,
                    title: title.to_owned(),
                });
            }
            for (key, child) in map {
                // Only descend into known collection keys; an `album` object
                // also has a `name` but is not a track.
                if matches!(
                    key.to_ascii_lowercase().as_str(),
                    "tracks" | "items" | "data" | "songs" | "results" | "playlist" | "entries"
                ) {
                    collect_json(child, out);
                }
            }
        }
        _ => {}
    }
}

fn from_lines(input: &str) -> Vec<Entry> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            for separator in [" – ", " — ", " - ", " –", " — "] {
                if let Some((artist, title)) = line.split_once(separator) {
                    let artist = artist.trim();
                    let title = title.trim();
                    if !artist.is_empty() && !title.is_empty() {
                        return Entry {
                            artist: Some(artist.to_owned()),
                            title: title.to_owned(),
                        };
                    }
                }
            }
            Entry {
                artist: None,
                title: line.to_owned(),
            }
        })
        .collect()
}

/// Parse all supported transfer formats. Kept pure so it can be unit-tested
/// without a Telegram or network dependency.
fn parse_entries(content: &str) -> Vec<Entry> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if (trimmed.starts_with('[') || trimmed.starts_with('{'))
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
    {
        let mut entries = Vec::new();
        collect_json(&value, &mut entries);
        if !entries.is_empty() {
            return entries;
        }
    }
    if trimmed.contains(',')
        && trimmed
            .lines()
            .next()
            .is_some_and(|line| line.contains(','))
    {
        let entries = from_csv(trimmed);
        if !entries.is_empty() {
            return entries;
        }
    }
    from_lines(trimmed)
}

/// `/import` with no arguments: wait for the next message from this user.
/// Telegram cannot send a command and a document in one update, so the playlist
/// always arrives separately.
pub async fn handle_import_prompt(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    lang: Lang,
) -> HandlerResult {
    if let Some(user) = msg.from.as_ref() {
        state.begin_import(user.id.0 as i64).await?;
    }
    bot.send_message(msg.chat.id, i18n::import_waiting(lang))
        .parse_mode(ParseMode::Html)
        .await?;
    Ok(())
}

pub async fn handle_import_args(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    arg: Option<&str>,
    lang: Lang,
) -> HandlerResult {
    let Some(content) = arg.map(str::trim).filter(|value| !value.is_empty()) else {
        return handle_import_prompt(state, bot, msg, lang).await;
    };
    process(state, bot, msg, lang, content).await
}

fn is_import_cancel(text: &str) -> bool {
    matches!(
        text.trim().trim_start_matches('/').to_lowercase().as_str(),
        "cancel" | "off" | "скасувати" | "скасуй"
    )
}

/// The message that followed a bare `/import`. Plain text is treated as
/// `Artist – Title` lines; files go through `handle_document` instead.
pub async fn handle_import_text(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    text: &str,
    lang: Lang,
) -> HandlerResult {
    if is_import_cancel(text) {
        bot.send_message(msg.chat.id, i18n::import_cancelled(lang))
            .await?;
        return Ok(());
    }
    process(state, bot, msg, lang, text.trim()).await
}

pub async fn handle_document(
    state: AppState,
    bot: Bot,
    msg: Message,
    document: &Document,
) -> HandlerResult {
    let lang = common::domain::Lang::from_telegram_hint(
        msg.from
            .as_ref()
            .and_then(|user| user.language_code.as_deref()),
        state.config.default_lang,
    );
    if !msg.chat.is_private() && !state.chat_allowed(msg.chat.id.0).await? {
        return Ok(());
    }
    let name = document
        .file_name
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = document
        .mime_type
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let supported = name.ends_with(".csv")
        || name.ends_with(".txt")
        || name.ends_with(".json")
        || mime.starts_with("text/")
        || mime.contains("json");
    if !supported {
        return Ok(());
    }
    let file = bot.get_file(document.file.id.clone()).await?;
    let mut bytes = Vec::new();
    bot.download_file(&file.path, &mut bytes).await?;
    if bytes.len() > 1_000_000 {
        bot.send_message(msg.chat.id, i18n::import_failed(lang))
            .await?;
        return Ok(());
    }
    let content = String::from_utf8_lossy(&bytes);
    process(&state, &bot, &msg, lang, &content).await
}

async fn process(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    lang: Lang,
    content: &str,
) -> HandlerResult {
    let entries = parse_entries(content);
    if entries.is_empty() {
        bot.send_message(msg.chat.id, i18n::import_no_tracks(lang))
            .await?;
        return Ok(());
    }
    if entries.len() > MAX_TRACKS {
        bot.send_message(msg.chat.id, i18n::import_too_many(lang, MAX_TRACKS))
            .await?;
        return Ok(());
    }
    let user = state
        .db
        .ensure_user(
            msg.from
                .as_ref()
                .map(|user| user.id.0 as i64)
                .unwrap_or_default(),
            lang,
        )
        .await?;
    let progress = bot
        .send_message(msg.chat.id, i18n::import_progress(lang, entries.len()))
        .await?;
    let mut tracks = Vec::with_capacity(entries.len());
    for entry in &entries {
        let query = match &entry.artist {
            Some(artist) => format!("{artist} {}", entry.title),
            None => entry.title.clone(),
        };
        match state.manager.search(&query, 1, user.search_source).await {
            Ok(results) => {
                if let Some(result) = results.into_iter().next() {
                    tracks.push(result.into_request());
                }
            }
            Err(error) => warn!(%error, query = %query, "playlist entry lookup failed"),
        }
    }
    if tracks.is_empty() {
        bot.edit_message_text(msg.chat.id, progress.id, i18n::import_no_tracks(lang))
            .await?;
        return Ok(());
    }
    let album = Album {
        title: "Imported playlist".to_owned(),
        provider: Provider::YouTube,
        tracks,
        source_url: None,
    };
    let user_id = msg
        .from
        .as_ref()
        .map(|user| user.id.0 as i64)
        .unwrap_or_default();
    confirm_album(state, bot, msg.chat.id, progress.id, lang, &album, user_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_import_cancel_words() {
        for text in ["cancel", "/cancel", "OFF", " скасувати ", "Скасуй"] {
            assert!(is_import_cancel(text), "{text} must cancel the prompt");
        }
        for text in ["Metallica – One", "cancel my subscription", ""] {
            assert!(!is_import_cancel(text), "{text} is playlist content");
        }
    }

    #[test]
    fn parses_plain_lines() {
        let entries = parse_entries(
            "Metallica – Enter Sandman\nNirvana - Smells Like Teen Spirit\nJust Title",
        );
        assert_eq!(
            entries,
            vec![
                Entry {
                    artist: Some("Metallica".into()),
                    title: "Enter Sandman".into()
                },
                Entry {
                    artist: Some("Nirvana".into()),
                    title: "Smells Like Teen Spirit".into()
                },
                Entry {
                    artist: None,
                    title: "Just Title".into()
                },
            ]
        );
    }

    #[test]
    fn parses_spotify_csv() {
        let entries = parse_entries(
            "track_name,artist_name,album_name\nOne,Metallica,Metallica\nEnter Sandman,Metallica,Metallica",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "One");
        assert_eq!(entries[0].artist.as_deref(), Some("Metallica"));
    }

    #[test]
    fn parses_transfer_json() {
        let json = r##"{"tracks":[{"name":"Walk","artist":{"name":"Pantera"}},{"title":"One","artist":"Metallica"}]}"##;
        let entries = parse_entries(json);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Walk");
        assert_eq!(entries[0].artist.as_deref(), Some("Pantera"));
        assert_eq!(entries[1].artist.as_deref(), Some("Metallica"));
    }
}
