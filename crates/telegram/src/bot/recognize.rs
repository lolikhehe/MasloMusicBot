//! `/recognize`: identify a replied-to audio file through AcoustID and show
//! the matching track through the regular search-result card.

use crate::bot::handlers::{edit_html, escape_html, search_result_rows};
use crate::state::AppState;
use common::domain::{Lang, UserRecord};
use common::error::HandlerResult;
use common::i18n;
use teloxide::net::Download;
use teloxide::prelude::*;
use teloxide::types::{FileId, InlineKeyboardMarkup, Message, ParseMode};
use tracing::warn;

const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
const RECOGNIZE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(150);

/// Removes the recognition scratch directory on every exit path, including
/// cancellation of the surrounding timeout.
struct TempGuard(std::path::PathBuf);

impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub async fn handle_recognize(
    state: &AppState,
    bot: &Bot,
    msg: &Message,
    user: &UserRecord,
    user_id: i64,
) -> HandlerResult {
    // Mirrors `/convert` / `/lyrics`: an audio or document reply is required.
    let file_id = msg.reply_to_message().and_then(|m| {
        m.audio()
            .map(|audio| audio.file.id.0.clone())
            .or_else(|| m.document().map(|doc| doc.file.id.0.clone()))
    });
    let Some(file_id) = file_id else {
        send_usage(bot, msg, user.lang).await;
        return Ok(());
    };
    let Some(acoustid) = state.acoustid.clone() else {
        send_usage_text(bot, msg.chat.id, i18n::recognize_not_configured(user.lang)).await;
        return Ok(());
    };

    let status = bot
        .send_message(msg.chat.id, i18n::recognize_listening(user.lang))
        .await?;
    let chat = msg.chat.id;
    let state = state.clone();
    let bot = bot.clone();
    let lang = user.lang;
    let search_source = user.search_source;
    tokio::spawn(async move {
        let work = async {
            let bytes = download(&bot, &file_id).await?;
            if bytes.len() > MAX_FILE_BYTES {
                return Ok::<_, Box<dyn std::error::Error + Send + Sync>>(None);
            }
            // TempGuard removes the whole subtree on every exit path,
            // including errors and cancellation of the surrounding timeout.
            let subdir = state
                .config
                .tmp_dir
                .join(format!("recognize-{}", uuid::Uuid::new_v4().simple()));
            tokio::fs::create_dir_all(&subdir).await?;
            let _guard = TempGuard(subdir.clone());
            let path = subdir.join("audio");
            tokio::fs::write(&path, &bytes).await?;
            let matches = acoustid.identify(&path).await?;
            Ok(matches.into_iter().max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }))
        };

        let outcome = tokio::time::timeout(RECOGNIZE_TIMEOUT, work).await;
        let best = match outcome {
            Ok(Ok(best)) => best,
            Ok(Err(error)) => {
                warn!(%error, "audio recognition failed");
                None
            }
            Err(_) => None,
        };
        let Some(best) = best else {
            let _ = edit_html(&bot, chat, status.id, i18n::recognize_not_found(lang)).await;
            return;
        };

        let mut text =
            i18n::recognize_result(lang, &escape_html(&best.artist), &escape_html(&best.title));
        if let Some(album) = best.album.filter(|album| !album.trim().is_empty()) {
            let year = best
                .year
                .map(|year| year.to_string())
                .unwrap_or_else(|| "-".to_owned());
            text.push('\n');
            text.push_str(&i18n::recognize_album_line(
                lang,
                &escape_html(&album),
                &year,
            ));
        }

        // Run the recognized title through the same search pipeline as a typed
        // query, and render the single best hit as a normal q: result card.
        let query = format!("{} - {}", best.artist, best.title);
        let markup = match state.manager.search(&query, 1, search_source).await {
            Ok(results) if !results.is_empty() => {
                let keys: Vec<String> = results.iter().map(|r| r.key.to_string()).collect();
                let heard = state
                    .db
                    .heard_before(user_id, &keys)
                    .await
                    .unwrap_or_default();
                match search_result_rows(&state, user_id, &results, &heard, 0).await {
                    Ok(rows) => Some(InlineKeyboardMarkup::new(rows)),
                    Err(error) => {
                        warn!(%error, "could not build recognition result card");
                        None
                    }
                }
            }
            _ => None,
        };
        match markup {
            Some(markup) => {
                let _ = bot
                    .edit_message_text(chat, status.id, text)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(markup)
                    .await;
            }
            None => {
                let _ = edit_html(&bot, chat, status.id, text).await;
            }
        }
    });
    Ok(())
}

async fn send_usage(bot: &Bot, msg: &Message, lang: Lang) {
    send_usage_text(bot, msg.chat.id, i18n::recognize_usage(lang)).await;
}

async fn send_usage_text(bot: &Bot, chat: ChatId, text: impl Into<String>) {
    let _ = crate::bot::send::send_html(bot, chat, text).await;
}

/// Local Bot API servers return an absolute path on the shared volume; the
/// HTTP `/file` endpoint is unavailable in that mode. Fall back to Teloxide's
/// downloader for cloud API paths.
async fn download(
    bot: &Bot,
    file_id: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let file = bot.get_file(FileId(file_id.to_owned())).await?;
    if std::path::Path::new(&file.path).is_absolute() {
        Ok(tokio::fs::read(&file.path).await?)
    } else {
        let mut bytes = Vec::new();
        bot.download_file(&file.path, &mut bytes).await?;
        Ok(bytes)
    }
}
