//! Telegram audio upload helpers.

use common::domain::CachedTrack;
use downloader::PreparedAudio;
use teloxide::prelude::*;
use teloxide::types::{FileId, InputFile, Message, ParseMode};

pub async fn send_html(
    bot: &Bot,
    chat: ChatId,
    text: impl Into<String>,
) -> Result<Message, teloxide::RequestError> {
    bot.send_message(chat, text)
        .parse_mode(ParseMode::Html)
        .await
}

pub async fn send_cached(
    bot: &Bot,
    chat: ChatId,
    track: &CachedTrack,
    caption: Option<String>,
) -> Result<Message, teloxide::RequestError> {
    let title = track.meta.stripped_title();
    let mut request = bot
        .send_audio(chat, InputFile::file_id(FileId(track.file_id.clone())))
        .title(truncate(&title, 128));
    if let Some(performer) = track.meta.performer.as_deref() {
        request = request.performer(truncate(performer, 128));
    }
    if let Some(duration) = track.meta.duration_secs {
        request = request.duration(duration);
    }
    if let Some(caption) = caption {
        request = request.caption(caption);
    }
    request.await
}

pub async fn send_prepared(
    bot: &Bot,
    chat: ChatId,
    prepared: &PreparedAudio,
    caption: Option<String>,
) -> Result<Message, teloxide::RequestError> {
    let input = InputFile::file(prepared.audio_path.clone()).file_name(prepared.file_name());
    let title = prepared.meta.stripped_title();
    let mut request = bot.send_audio(chat, input).title(truncate(&title, 128));
    if let Some(performer) = prepared.meta.performer.as_deref() {
        request = request.performer(truncate(performer, 128));
    }
    if let Some(duration) = prepared.meta.duration_secs {
        request = request.duration(duration);
    }
    // Telegram caps audio thumbnails at 200 KB. The HQ cover is embedded in
    // the tags for full-size display; only the small variant goes here.
    if let Some(cover) = prepared
        .thumb_path
        .as_ref()
        .or(prepared.cover_path.as_ref())
    {
        request = request.thumbnail(InputFile::file(cover.clone()));
    }
    if let Some(caption) = caption {
        request = request.caption(caption);
    }
    request.await
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
}

fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}
