//! Telegram audio upload helpers.

use common::domain::CachedTrack;
use downloader::PreparedAudio;
use std::future::IntoFuture;
use teloxide::prelude::*;
use teloxide::types::{FileId, InlineKeyboardMarkup, InputFile, Message, MessageId, ParseMode};
use tracing::debug;

/// Retry a send once after Telegram's 429 `RetryAfter`; a second failure is
/// returned to the caller. The closure rebuilds the request because awaiting
/// consumes the teloxide request builder.
pub(crate) async fn with_retry_after<T, F, Fut>(mut make: F) -> Result<T, teloxide::RequestError>
where
    F: FnMut() -> Fut,
    Fut: IntoFuture<Output = Result<T, teloxide::RequestError>>,
{
    match make().await {
        Err(teloxide::RequestError::RetryAfter(seconds)) => {
            tokio::time::sleep(seconds.duration()).await;
            make().await
        }
        other => other,
    }
}

pub async fn send_html(
    bot: &Bot,
    chat: ChatId,
    text: impl Into<String>,
) -> Result<Message, teloxide::RequestError> {
    let text = text.into();
    with_retry_after(|| {
        bot.send_message(chat, text.clone())
            .parse_mode(ParseMode::Html)
    })
    .await
}

pub async fn send_cached(
    bot: &Bot,
    chat: ChatId,
    track: &CachedTrack,
    caption: Option<String>,
    markup: Option<InlineKeyboardMarkup>,
) -> Result<MessageId, teloxide::RequestError> {
    // The local Bot API rebuilds a file_id audio from its own record and drops
    // title/performer. Copying the original message keeps the rendered
    // attributes. Only copy when we have a source message AND a fresh caption:
    // without one Telegram would keep the original recipient's caption, which
    // must never leak to a different chat.
    if let (Some(from_chat_id), Some(from_message_id)) = (track.chat_id, track.message_id)
        && caption.is_some()
        && let Ok(message_id) = i32::try_from(from_message_id)
    {
        let copy_caption = caption.clone().unwrap_or_default();
        let copy_markup = markup.clone();
        let copied = with_retry_after(|| {
            let mut request = bot
                .copy_message(chat, ChatId(from_chat_id), MessageId(message_id))
                .caption(copy_caption.clone());
            if let Some(markup) = &copy_markup {
                request = request.reply_markup(markup.clone());
            }
            request
        })
        .await;
        match copied {
            Ok(id) => return Ok(id),
            Err(error) => {
                // Any copy failure (stale source message, permissions, ...) is
                // non-fatal: fall back to the ordinary file_id resend.
                debug!(%error, "copy_message failed; falling back to file_id");
            }
        }
    }

    let title = truncate(&track.meta.stripped_title(), 128);
    let performer = track.meta.performer.as_deref().map(|p| truncate(p, 128));
    let duration = track.meta.duration_secs;
    with_retry_after(|| {
        let mut request = bot
            .send_audio(chat, InputFile::file_id(FileId(track.file_id.clone())))
            .title(title.clone());
        if let Some(performer) = performer.as_deref() {
            request = request.performer(performer.to_owned());
        }
        if let Some(duration) = duration {
            request = request.duration(duration);
        }
        if let Some(caption) = caption.as_deref() {
            request = request.caption(caption.to_owned());
        }
        if let Some(markup) = &markup {
            request = request.reply_markup(markup.clone());
        }
        request
    })
    .await
    .map(|message| message.id)
}

pub async fn send_prepared(
    bot: &Bot,
    chat: ChatId,
    prepared: &PreparedAudio,
    caption: Option<String>,
    markup: Option<InlineKeyboardMarkup>,
) -> Result<Message, teloxide::RequestError> {
    let title = truncate(&prepared.meta.stripped_title(), 128);
    let performer = prepared.meta.performer.as_deref().map(|p| truncate(p, 128));
    let duration = prepared.meta.duration_secs;
    // Telegram caps audio thumbnails at 200 KB; only the small variant is
    // safe to send. The full-size cover stays embedded in the file's tags.
    let thumb = prepared.thumb_path.clone();
    with_retry_after(|| {
        let input = InputFile::file(prepared.audio_path.clone()).file_name(prepared.file_name());
        let mut request = bot.send_audio(chat, input).title(title.clone());
        if let Some(performer) = performer.as_deref() {
            request = request.performer(performer.to_owned());
        }
        if let Some(duration) = duration {
            request = request.duration(duration);
        }
        if let Some(cover) = &thumb {
            request = request.thumbnail(InputFile::file(cover.clone()));
        }
        if let Some(caption) = caption.as_deref() {
            request = request.caption(caption.to_owned());
        }
        if let Some(markup) = &markup {
            request = request.reply_markup(markup.clone());
        }
        request
    })
    .await
}

pub fn extract_file_id(message: &Message) -> Option<(String, String)> {
    message
        .audio()
        .map(|audio| (audio.file.id.0.clone(), audio.file.unique_id.0.clone()))
}

pub fn is_invalid_file_id(error: &teloxide::RequestError) -> bool {
    let text = error.to_string().to_ascii_lowercase();
    text.contains("file_id")
        || text.contains("file identifier")
        || text.contains("wrong file")
        || text.contains("invalid file")
        || text.contains("audio_content_type_invalid")
}

pub fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}
