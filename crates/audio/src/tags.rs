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
/// The embedded cover is not subject to Telegram's thumbnail budget; a 1000 px
/// JPEG at q2 can be a few hundred KB, which is fine inside an audio file.
const MAX_EMBEDDED_COVER_BYTES: u64 = 1_500_000;
const MAX_COVER_DOWNLOAD: u64 = 10_000_000;
const HQ_SCALE: &str = "scale=1000:1000:force_original_aspect_ratio=decrease";
const LQ_SCALE: &str = "scale=320:320:force_original_aspect_ratio=decrease";

/// Two files are needed: Telegram's `thumbnail` is hard-capped at 200 KB and
/// always displayed small, while music players show the cover embedded in the
/// tags at full size. In HQ mode they must therefore be generated separately.
#[derive(Debug, Clone, Default)]
pub struct CoverFiles {
    pub embedded: Option<PathBuf>,
    pub thumbnail: Option<PathBuf>,
}

pub async fn prepare_cover(
    client: &reqwest::Client,
    ffmpeg: &str,
    url: Option<&str>,
    dir: &Path,
    high_quality: bool,
) -> CoverFiles {
    let Some(url) = url else {
        return CoverFiles::default();
    };
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return CoverFiles::default();
    }

    // YouTube search results only expose hqdefault.jpg (480x360). In HQ mode
    // try the full-res variant first and verify the pixels actually arrived
    // (YouTube serves a 120x90 placeholder for videos without maxres).
    let mut best: Option<(Vec<u8>, usize)> = None;
    for candidate in cover_candidates(url, high_quality) {
        let Some(bytes) = download_cover(client, &candidate).await else {
            continue;
        };
        let side = image_long_side(&bytes).unwrap_or(0);
        let is_better = best.as_ref().is_none_or(|(_, best_side)| side > *best_side);
        if is_better {
            best = Some((bytes, side));
        }
        if !high_quality || side >= 900 {
            break;
        }
    }
    let Some((bytes, source_side)) = best else {
        return CoverFiles::default();
    };
    debug!(source_side, high_quality, "cover source selected");

    let source = dir.join("cover_source");
    if let Err(error) = tokio::fs::write(&source, &bytes).await {
        debug!(%error, "could not write cover source");
        return CoverFiles::default();
    }

    // Default quality: one small file serves both purposes.
    if !high_quality {
        let output = dir.join("cover.jpg");
        if is_jpeg(&bytes) && bytes.len() <= MAX_COVER_BYTES {
            if tokio::fs::copy(&source, &output).await.is_ok() {
                return CoverFiles {
                    embedded: Some(output.clone()),
                    thumbnail: Some(output),
                };
            }
            return CoverFiles::default();
        }
        if convert_cover(ffmpeg, &source, &output, LQ_SCALE, "4")
            .await
            .is_err()
        {
            return CoverFiles::default();
        }
        let size = file_size(&output).await;
        if size > MAX_COVER_BYTES as u64 {
            warn!(size, "cover is still too large for Telegram");
            return CoverFiles::default();
        }
        return CoverFiles {
            embedded: Some(output.clone()),
            thumbnail: Some(output),
        };
    }

    // HQ: a real 1000 px cover embedded in the audio, independent of the
    // 200 KB ceiling on Telegram's thumbnail.
    let embedded = dir.join("cover_embedded.jpg");
    if convert_cover(ffmpeg, &source, &embedded, HQ_SCALE, "2")
        .await
        .is_err()
    {
        return CoverFiles::default();
    }
    if file_size(&embedded).await > MAX_EMBEDDED_COVER_BYTES {
        warn!("1000px cover is unusually large; re-encoding at lower quality");
        if convert_cover(ffmpeg, &source, &embedded, HQ_SCALE, "4")
            .await
            .is_err()
        {
            return CoverFiles::default();
        }
    }

    let thumbnail = dir.join("cover_thumb.jpg");
    if convert_cover(ffmpeg, &embedded, &thumbnail, LQ_SCALE, "4")
        .await
        .is_err()
    {
        return CoverFiles {
            embedded: Some(embedded),
            thumbnail: None,
        };
    }
    let thumbnail = if file_size(&thumbnail).await <= MAX_COVER_BYTES as u64 {
        thumbnail
    } else {
        let smaller = dir.join("cover_thumb_small.jpg");
        if convert_cover(
            ffmpeg,
            &embedded,
            &smaller,
            "scale=240:240:force_original_aspect_ratio=decrease",
            "5",
        )
        .await
        .is_err()
            || file_size(&smaller).await > MAX_COVER_BYTES as u64
        {
            return CoverFiles {
                embedded: Some(embedded),
                thumbnail: None,
            };
        }
        smaller
    };
    CoverFiles {
        embedded: Some(embedded),
        thumbnail: Some(thumbnail),
    }
}

async fn download_cover(client: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    let mut response = client.get(url).send().await.ok()?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_COVER_DOWNLOAD)
    {
        return None;
    }
    // Stream instead of buffering blindly: a server with no content-length can
    // otherwise make us allocate an unbounded body before the size check.
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or(64 * 1024)
            .min(256 * 1024) as usize,
    );
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() as u64 + chunk.len() as u64 > MAX_COVER_DOWNLOAD {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    (!bytes.is_empty()).then_some(bytes)
}

async fn file_size(path: &Path) -> u64 {
    tokio::fs::metadata(path)
        .await
        .map(|meta| meta.len())
        .unwrap_or(u64::MAX)
}

/// Full-resolution replacements for the low-res thumbnails search results
/// expose. YouTube's default is 480x360; iTunes artworkUrl100 is 100x100.
fn cover_candidates(url: &str, high_quality: bool) -> Vec<String> {
    if !high_quality {
        return vec![url.to_owned()];
    }
    let mut candidates = Vec::new();
    for prefix in ["https://i.ytimg.com/vi/", "https://i.ytimg.com/vi_webp/"] {
        if let Some(rest) = url.strip_prefix(prefix)
            && let Some((id, tail)) = rest.split_once('/')
            && ["hqdefault", "mqdefault", "sddefault"]
                .iter()
                .any(|name| tail.starts_with(name))
        {
            // The JPEG maxres endpoint lives under /vi/, not /vi_webp/.
            candidates.push(format!("https://i.ytimg.com/vi/{id}/maxresdefault.jpg"));
        }
    }
    if url.contains("100x100bb") {
        candidates.push(url.replace("100x100bb", "1000x1000bb"));
    }
    if !candidates.iter().any(|candidate| candidate == url) {
        candidates.push(url.to_owned());
    }
    candidates
}

/// Long side of a JPEG, read straight from its SOF marker. Avoids shelling out
/// to ffprobe and lets us reject YouTube's placeholder "maxres" images.
fn image_long_side(bytes: &[u8]) -> Option<usize> {
    if !is_jpeg(bytes) {
        return None;
    }
    let mut index = 2;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xFF {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            index += 2;
            continue;
        }
        let length = u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]) as usize;
        if length < 2 || index + 2 + length > bytes.len() {
            break;
        }
        if matches!(
            marker,
            0xC0 | 0xC1
                | 0xC2
                | 0xC3
                | 0xC5
                | 0xC6
                | 0xC7
                | 0xC9
                | 0xCA
                | 0xCB
                | 0xCD
                | 0xCE
                | 0xCF
        ) {
            let height = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as usize;
            let width = u16::from_be_bytes([bytes[index + 7], bytes[index + 8]]) as usize;
            return Some(width.max(height));
        }
        index += 2 + length;
    }
    None
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

/// The embedded cover follows the audio file's own budget (1.5 MB), not
/// Telegram's 200 KB thumbnail cap. Using the thumbnail limit here silently
/// dropped HQ covers above ~200 KB.
fn embeddable_cover(len: usize) -> bool {
    len > 0 && len as u64 <= MAX_EMBEDDED_COVER_BYTES
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
        if embeddable_cover(data.len()) {
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

pub fn safe_filename(meta: &TrackMeta, extension: &str) -> String {
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
    format!("{name}.{extension}")
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
        let name = safe_filename(&meta, "m4a");
        assert_eq!(name, "Artist - A_B_ _C__.m4a");
        assert!(!name.contains('/'));
    }

    #[test]
    fn long_filename_is_bounded() {
        let meta = TrackMeta {
            title: "x".repeat(300),
            ..Default::default()
        };
        let name = safe_filename(&meta, "flac");
        assert!(name.ends_with(".flac"));
        assert!(name.chars().count() <= 102);
    }

    #[test]
    fn hq_upgrades_low_res_search_thumbnails() {
        let hq = cover_candidates("https://i.ytimg.com/vi/abc123/hqdefault.jpg", true);
        assert_eq!(hq[0], "https://i.ytimg.com/vi/abc123/maxresdefault.jpg");
        assert_eq!(hq[1], "https://i.ytimg.com/vi/abc123/hqdefault.jpg");

        let webp = cover_candidates("https://i.ytimg.com/vi_webp/abc123/hqdefault.webp", true);
        assert_eq!(webp[0], "https://i.ytimg.com/vi/abc123/maxresdefault.jpg");

        let apple = cover_candidates("https://is1-ssl.mzstatic.com/100x100bb.jpg", true);
        assert_eq!(apple[0], "https://is1-ssl.mzstatic.com/1000x1000bb.jpg");

        let lq = cover_candidates("https://i.ytimg.com/vi/abc123/hqdefault.jpg", false);
        assert_eq!(lq, ["https://i.ytimg.com/vi/abc123/hqdefault.jpg"]);
    }

    #[test]
    fn reads_jpeg_dimensions_from_sof_marker() {
        // Minimal JPEG header with a SOF0 marker declaring 1000x640.
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&[
            0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0,
        ]);
        jpeg.extend_from_slice(&[
            0xFF, 0xC0, 0x00, 0x11, 0x08, 0x02, 0x80, 0x03, 0xE8, 0x03, 0x01, 0x22, 0x00, 0x02,
            0x11, 0x01, 0x03, 0x11, 0x01,
        ]);
        assert_eq!(image_long_side(&jpeg), Some(1000));
        assert_eq!(image_long_side(b"not a jpeg"), None);
    }

    #[test]
    fn embedded_cover_uses_audio_budget_not_thumbnail_budget() {
        // Regression: a 300 KB HQ cover used to be dropped because the
        // embedded check reused Telegram's 200 KB thumbnail cap.
        assert!(embeddable_cover(300_000));
        assert!(embeddable_cover(MAX_EMBEDDED_COVER_BYTES as usize));
        assert!(!embeddable_cover(MAX_EMBEDDED_COVER_BYTES as usize + 1));
        assert!(!embeddable_cover(0));
    }
}
