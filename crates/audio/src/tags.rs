//! Audio tagging and cover preparation. Tagging is best-effort: a track is
//! still delivered if a container refuses a tag or the cover is unavailable.

use common::domain::TrackMeta;
use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{Accessor, Tag, TagExt, TagType};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, warn};

const MAX_COVER_BYTES: usize = 200_000;
const MAX_COVER_DOWNLOAD: u64 = 10_000_000;

pub async fn prepare_cover(
    client: &reqwest::Client,
    ffmpeg: &str,
    url: Option<&str>,
    dir: &Path,
    high_quality: bool,
) -> Option<PathBuf> {
    let url = url?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }

    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => {
            debug!(%error, "cover download failed");
            return None;
        }
    };
    if !response.status().is_success() {
        return None;
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_COVER_DOWNLOAD)
    {
        return None;
    }
    let bytes = match response.bytes().await {
        Ok(bytes) if bytes.len() as u64 <= MAX_COVER_DOWNLOAD => bytes,
        Ok(_) => return None,
        Err(error) => {
            debug!(%error, "cover body failed");
            return None;
        }
    };
    if bytes.is_empty() {
        return None;
    }

    let source = dir.join("cover_source");
    if let Err(error) = tokio::fs::write(&source, &bytes).await {
        debug!(%error, "could not write cover source");
        return None;
    }

    let output = dir.join("cover.jpg");
    if is_jpeg(&bytes) && bytes.len() <= MAX_COVER_BYTES {
        if tokio::fs::copy(&source, &output).await.is_ok() {
            return Some(output);
        }
        return None;
    }

    // Low quality (default): small 320 px thumbnail-friendly frame, hard size
    // budget. High quality (/settings): up to 1000 px, JPEG q2; if that still
    // exceeds Telegram's 200 KB thumbnail budget, step down to 640 px q4.
    let (scale, quality) = if high_quality {
        ("scale=1000:1000:force_original_aspect_ratio=decrease", "2")
    } else {
        ("scale=320:320:force_original_aspect_ratio=decrease", "4")
    };

    let hq_result = convert_cover(ffmpeg, &source, &output, scale, quality).await;
    match hq_result {
        Ok(()) => {}
        Err(()) => return None,
    }

    let size = tokio::fs::metadata(&output)
        .await
        .map(|meta| meta.len())
        .unwrap_or(u64::MAX);
    if size <= MAX_COVER_BYTES as u64 {
        return Some(output);
    }
    if !high_quality {
        warn!(size, "cover is still too large for Telegram");
        return None;
    }
    warn!(
        size,
        "1000px cover exceeds Telegram's budget; stepping down to 640px"
    );
    let _ = tokio::fs::remove_file(&output).await;
    if convert_cover(
        ffmpeg,
        &source,
        &output,
        "scale=640:640:force_original_aspect_ratio=decrease",
        "4",
    )
    .await
    .is_err()
    {
        return None;
    }
    let size = tokio::fs::metadata(&output)
        .await
        .map(|meta| meta.len())
        .unwrap_or(u64::MAX);
    if size <= MAX_COVER_BYTES as u64 {
        Some(output)
    } else {
        warn!(size, "640px cover is still too large for Telegram");
        None
    }
}

/// Run ffmpeg once for the cover. Returns Err(()) on any failure.
async fn convert_cover(
    ffmpeg: &str,
    source: &std::path::Path,
    output: &std::path::Path,
    scale: &str,
    quality: &str,
) -> Result<(), ()> {
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new(ffmpeg)
            .args(["-nostdin", "-y", "-loglevel", "error", "-i"])
            .arg(source)
            .args(["-frames:v", "1", "-vf", scale, "-q:v", quality])
            .arg(output)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => {
            warn!(
                stderr = %String::from_utf8_lossy(&output.stderr),
                "ffmpeg cover conversion failed"
            );
            Err(())
        }
        Ok(Err(error)) => {
            warn!(%error, "could not start ffmpeg for cover");
            Err(())
        }
        Err(_) => {
            warn!("ffmpeg cover conversion timed out");
            Err(())
        }
    }
}

fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

/// Write common tags and an optional front cover. Runs on a blocking thread.
pub async fn tag_audio(path: PathBuf, meta: TrackMeta, cover: Option<PathBuf>) {
    let result =
        tokio::task::spawn_blocking(move || tag_audio_sync(&path, &meta, cover.as_deref())).await;
    match result {
        Ok(Ok(())) => debug!("audio tags written"),
        Ok(Err(error)) => warn!(%error, "audio tagging skipped"),
        Err(error) => warn!(%error, "tagging task panicked"),
    }
}

fn tag_audio_sync(path: &Path, meta: &TrackMeta, cover: Option<&Path>) -> Result<(), String> {
    let tag_type = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("m4a" | "mp4" | "m4b" | "aac") => TagType::Mp4Ilst,
        Some("mp3") => TagType::Id3v2,
        Some("flac" | "ogg" | "opus") => TagType::VorbisComments,
        Some(other) => return Err(format!("no tag writer for .{other}")),
        None => return Err("audio file has no extension".to_owned()),
    };

    let mut tag = Tag::new(tag_type);
    if !meta.title.trim().is_empty() {
        tag.set_title(trim_tag(&meta.title));
    }
    if let Some(performer) = meta
        .performer
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        tag.set_artist(trim_tag(performer));
    }
    if let Some(album) = meta
        .album
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        tag.set_album(trim_tag(album));
    }

    if let Some(cover) = cover {
        let data = std::fs::read(cover).map_err(|error| format!("cover read: {error}"))?;
        if !data.is_empty() && data.len() <= MAX_COVER_BYTES {
            let picture = Picture::unchecked(data)
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Jpeg)
                .build();
            tag.push_picture(picture);
        }
    }

    tag.save_to_path(path, WriteOptions::default())
        .map_err(|error| format!("tag save: {error}"))
}

pub fn safe_filename(meta: &TrackMeta) -> String {
    let artist = meta.performer.as_deref().unwrap_or("").trim();
    let title = meta.title.trim();
    let mut name = if artist.is_empty() {
        title.to_owned()
    } else if title.is_empty() {
        artist.to_owned()
    } else {
        format!("{artist} - {title}")
    };

    name = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            control if control.is_control() => ' ',
            other => other,
        })
        .collect::<String>();
    name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    name = name.trim_matches(['.', ' ']).to_owned();
    name = truncate_chars(&name, 96);
    if name.is_empty() {
        name = "track".to_owned();
    }
    format!("{name}.m4a")
}

fn trim_tag(value: &str) -> String {
    truncate_chars(value.trim(), 256)
}

fn truncate_chars(value: &str, max: usize) -> String {
    let mut result = String::with_capacity(value.len().min(max));
    for (index, character) in value.chars().enumerate() {
        if index >= max {
            break;
        }
        result.push(character);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_is_safe() {
        let meta = TrackMeta {
            title: "A/B: \"C\"?".into(),
            performer: Some("Artist".into()),
            ..Default::default()
        };
        let name = safe_filename(&meta);
        assert_eq!(name, "Artist - A_B_ _C__.m4a");
        assert!(!name.contains('/'));
    }

    #[test]
    fn long_filename_is_bounded() {
        let meta = TrackMeta {
            title: "x".repeat(300),
            ..Default::default()
        };
        let name = safe_filename(&meta);
        assert!(name.chars().count() <= 102);
    }
}
