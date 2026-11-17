//! Thin async wrapper around the yt-dlp process. Commands are built as argv
//! arrays (never through a shell) and every invocation has a hard timeout.

use crate::url::{ParsedInput, parse_input};
use common::config::Config;
use common::domain::{
    Album, AlbumRef, DownloadTarget, Provider, SearchResult, TrackKey, TrackMeta, TrackRequest,
};
use common::error::DownloadError;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, warn};

#[derive(Clone)]
pub struct YtDlp {
    binary: String,
    ffmpeg: String,
    metadata_timeout: Duration,
    download_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub meta: TrackMeta,
    pub source_url: String,
}

#[derive(Debug)]
pub struct DownloadedAudio {
    pub audio_path: PathBuf,
    pub meta: TrackMeta,
}

impl YtDlp {
    pub fn new(config: &Config) -> Self {
        Self {
            binary: config.ytdlp_path.clone(),
            ffmpeg: config.ffmpeg_path.clone(),
            metadata_timeout: config.metadata_timeout,
            download_timeout: config.download_timeout,
        }
    }

    /// YouTube search (`ytsearchN:`).
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, DownloadError> {
        self.search_prefixed("ytsearch", Provider::YouTube, query, limit)
            .await
    }

    /// SoundCloud search (`scsearchN:`); audio itself is downloaded directly
    /// from SoundCloud, no `ytsearch` re-match involved.
    pub async fn search_soundcloud(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, DownloadError> {
        self.search_prefixed("scsearch", Provider::SoundCloud, query, limit)
            .await
    }

    async fn search_prefixed(
        &self,
        prefix: &str,
        provider: Provider,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, DownloadError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 20);
        let target = format!("{prefix}{limit}:{query}");
        let limit_arg = limit.to_string();
        let output = self
            .run(
                &[
                    "--ignore-config",
                    "--dump-single-json",
                    "--flat-playlist",
                    "--skip-download",
                    "--no-warnings",
                    "--ignore-errors",
                    "--playlist-end",
                    &limit_arg,
                    &target,
                ],
                self.metadata_timeout,
            )
            .await?;

        if !output.status.success() {
            return Err(classify_failure(
                &String::from_utf8_lossy(&output.stderr),
                self.metadata_timeout,
            ));
        }

        let info: Info = serde_json::from_slice(&output.stdout).map_err(|error| {
            DownloadError::SourceUnavailable(format!("invalid yt-dlp JSON: {error}"))
        })?;
        let entries = info.entries.unwrap_or_default();
        let mut seen = HashSet::new();
        let mut results = Vec::new();
        for entry in entries {
            if let Some(result) = search_result_from_entry(entry, provider)
                && seen.insert(result.key.clone())
            {
                results.push(result);
            }
        }
        Ok(results)
    }

    pub async fn metadata(&self, url: &str) -> Result<MediaInfo, DownloadError> {
        let output = self
            .run(
                &[
                    "--ignore-config",
                    "--dump-single-json",
                    "--no-playlist",
                    "--skip-download",
                    "--no-warnings",
                    url,
                ],
                self.metadata_timeout,
            )
            .await?;
        if !output.status.success() {
            return Err(classify_failure(
                &String::from_utf8_lossy(&output.stderr),
                self.metadata_timeout,
            ));
        }
        let info: Info = serde_json::from_slice(&output.stdout).map_err(|error| {
            DownloadError::SourceUnavailable(format!("invalid yt-dlp JSON: {error}"))
        })?;
        if info.title.as_deref().unwrap_or_default().trim().is_empty() {
            return Err(DownloadError::NotFound);
        }
        let source_url = info.webpage_url.clone().unwrap_or_else(|| url.to_owned());
        Ok(MediaInfo {
            meta: info.into_meta(),
            source_url,
        })
    }

    /// Expand an album/playlist link through yt-dlp's flat playlist mode.
    pub async fn playlist(&self, album: &AlbumRef, limit: usize) -> Result<Album, DownloadError> {
        let url = album
            .url
            .as_deref()
            .ok_or_else(|| DownloadError::Unsupported("album has no URL".to_owned()))?;
        let limit = limit.clamp(1, 50);
        let output = self
            .run(
                &[
                    "--ignore-config",
                    "--dump-single-json",
                    "--flat-playlist",
                    "--skip-download",
                    "--no-warnings",
                    "--ignore-errors",
                    "--playlist-end",
                    &limit.to_string(),
                    url,
                ],
                self.metadata_timeout,
            )
            .await?;
        if !output.status.success() {
            return Err(classify_failure(
                &String::from_utf8_lossy(&output.stderr),
                self.metadata_timeout,
            ));
        }
        let info: Info = serde_json::from_slice(&output.stdout).map_err(|error| {
            DownloadError::SourceUnavailable(format!("invalid yt-dlp JSON: {error}"))
        })?;
        let title = info
            .title
            .clone()
            .or_else(|| album.title.clone())
            .unwrap_or_else(|| album.provider.display_name().to_owned());
        let mut tracks = Vec::new();
        for entry in info.entries.unwrap_or_default().into_iter().take(limit) {
            if let Some(track) = album_track_from_entry(entry, album.provider, &title) {
                tracks.push(track);
            }
        }
        if tracks.is_empty() {
            return Err(DownloadError::NotFound);
        }
        Ok(Album {
            title,
            provider: album.provider,
            tracks,
            source_url: album.url.clone(),
        })
    }

    pub async fn download(
        &self,
        target: &DownloadTarget,
        dir: &Path,
        quality: u32,
        max_duration_secs: u64,
        max_bytes: u64,
    ) -> Result<DownloadedAudio, DownloadError> {
        // Aligned with the command line in the spec (§9 етап 3): prefer m4a,
        // parallel fragments, socket timeouts, bounded retries, and `--` so a
        // target that starts with `-` can never be parsed as an option.
        // `--print after_move:filepath` prints the final (post-converted) path,
        // which we use verbatim; a slow fallback re-detects it by extension.
        // Lossless switch: quality == 0 means "best available source, keep it
        // in a FLAC container". YouTube never serves true lossless, so this is
        // a container/format decision (no re-encode quality settings), not a
        // promise of hi-res audio; the README says so.
        let lossless = quality == 0;
        let format_selector = if lossless {
            "bestaudio[acodec=flac]/bestaudio/best"
        } else {
            "bestaudio[ext=m4a]/bestaudio"
        };
        let audio_format = if lossless { "flac" } else { "m4a" };
        let output_template = dir.join("audio.%(ext)s");
        let target_arg = target.ytdlp_argument();
        let mut args: Vec<&str> = vec![
            "--ignore-config",
            "--no-playlist",
            "--no-warnings",
            "--no-progress",
            "--no-part",
            "-f",
            format_selector,
            "--extract-audio",
            "--audio-format",
            audio_format,
        ];
        let quality_arg;
        if !lossless {
            quality_arg = format!("{}K", quality.clamp(64, 320));
            args.extend(["--audio-quality", quality_arg.as_str()]);
        }
        let max_bytes_arg = max_bytes.to_string();
        let output_template_str = output_template.to_string_lossy();
        let ffmpeg_location = &self.ffmpeg;
        let target = &target_arg;
        args.extend([
            "-N",
            "4",
            "--socket-timeout",
            "15",
            "--retries",
            "3",
            "--max-filesize",
            max_bytes_arg.as_str(),
            "--write-info-json",
            "--ffmpeg-location",
            ffmpeg_location,
            "--output",
            &output_template_str,
            "--print",
            "after_move:filepath",
            "--",
            target,
        ]);
        let output = self.run(&args, self.download_timeout).await?;
        let printed_path = stdout_last_line(&output.stdout);

        if !output.status.success() {
            return Err(classify_failure(
                &String::from_utf8_lossy(&output.stderr),
                self.download_timeout,
            ));
        }

        // Prefer the path yt-dlp printed via --print after_move:filepath; the
        // directory scan is the fallback for unexpected yt-dlp versions.
        let (audio_path, info_path) = match printed_path
            .as_deref()
            .filter(|path| Path::new(path).is_file())
        {
            Some(path) => {
                let info_path = find_info_only(dir).await;
                (PathBuf::from(path), info_path)
            }
            None => find_output(dir).await?,
        };
        let info = read_info(&info_path).await?;
        let meta = info.as_ref().map(Info::clone_meta).unwrap_or_default();

        if let Some(duration) = meta.duration_secs
            && u64::from(duration) > max_duration_secs
        {
            return Err(DownloadError::TooLong {
                limit: max_duration_secs,
                actual: u64::from(duration),
            });
        }

        let actual_bytes = tokio::fs::metadata(&audio_path).await?.len();
        if actual_bytes > max_bytes {
            return Err(DownloadError::TooLarge {
                limit: max_bytes,
                actual: actual_bytes,
            });
        }

        Ok(DownloadedAudio { audio_path, meta })
    }

    async fn run(
        &self,
        args: &[&str],
        timeout: Duration,
    ) -> Result<std::process::Output, DownloadError> {
        debug!(binary = %self.binary, ?args, "running yt-dlp");
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let output = tokio::time::timeout(timeout, command.output())
            .await
            .map_err(|_| {
                DownloadError::SourceUnavailable(format!(
                    "yt-dlp did not finish within {}s",
                    timeout.as_secs()
                ))
            })?
            .map_err(|error| {
                DownloadError::SourceUnavailable(format!("cannot start yt-dlp: {error}"))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(status = %output.status, stderr = %trim_log(&stderr), "yt-dlp failed");
        }
        Ok(output)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Info {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    track: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    creator: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    thumbnail: Option<String>,
    #[serde(default)]
    webpage_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    entries: Option<Vec<Info>>,
    #[serde(default)]
    thumbnails: Vec<Thumbnail>,
    /// yt-dlp exposes a `genres` array for some extractors (e.g. SoundCloud).
    #[serde(default)]
    genres: Option<Vec<String>>,
}

impl Info {
    fn into_meta(self) -> TrackMeta {
        let title = self
            .track
            .clone()
            .or_else(|| self.title.clone())
            .unwrap_or_default();
        let performer = self
            .artist
            .clone()
            .or_else(|| self.creator.clone())
            .or_else(|| self.channel.clone())
            .or_else(|| self.uploader.clone())
            .filter(|value| !value.trim().is_empty());
        let thumbnail_url = self
            .thumbnail
            .clone()
            .or_else(|| best_thumbnail(&self.thumbnails));
        let genres: Vec<String> = self
            .genres
            .unwrap_or_default()
            .into_iter()
            .map(|genre| genre.trim().to_owned())
            .filter(|genre| !genre.is_empty())
            .take(4)
            .collect();
        TrackMeta {
            title: title.trim().to_owned(),
            performer,
            album: self.album,
            duration_secs: duration_secs(self.duration),
            thumbnail_url,
            genres,
        }
    }

    fn clone_meta(&self) -> TrackMeta {
        self.clone().into_meta()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Thumbnail {
    url: String,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

fn best_thumbnail(thumbnails: &[Thumbnail]) -> Option<String> {
    thumbnails
        .iter()
        .max_by_key(|thumbnail| {
            thumbnail
                .width
                .unwrap_or(0)
                .saturating_mul(thumbnail.height.unwrap_or(0))
        })
        .map(|thumbnail| thumbnail.url.clone())
}

fn duration_secs(duration: Option<f64>) -> Option<u32> {
    duration
        .filter(|value| *value > 0.0 && value.is_finite())
        .map(|value| value.round() as u32)
}

fn search_result_from_entry(entry: Info, provider: Provider) -> Option<SearchResult> {
    let key_prefix = provider.as_str();
    let source_url = entry
        .webpage_url
        .clone()
        .or_else(|| entry.url.clone().filter(|url| url.starts_with("http")))
        .or_else(|| {
            entry.id.clone().map(|id| match provider {
                Provider::SoundCloud => format!("https://soundcloud.com/tracks/{id}"),
                _ => format!("https://www.youtube.com/watch?v={id}"),
            })
        })?;
    if !source_url.starts_with("http") {
        return None;
    }
    let key = match parse_input(&source_url) {
        ParsedInput::Track(track) => track.key,
        _ => TrackKey::new(format!("{key_prefix}:url:{source_url}")),
    };
    let meta = entry.into_meta();
    if meta.title.is_empty() {
        return None;
    }
    Some(SearchResult {
        key,
        provider,
        target: DownloadTarget::Url(source_url.clone()),
        source_url,
        meta,
    })
}

fn album_track_from_entry(entry: Info, provider: Provider, album: &str) -> Option<TrackRequest> {
    let source_url = entry
        .webpage_url
        .clone()
        .or_else(|| entry.url.clone())
        .or_else(|| {
            entry
                .id
                .clone()
                .map(|id| format!("https://www.youtube.com/watch?v={id}"))
        })?;
    if !source_url.starts_with("http") {
        return None;
    }
    let mut meta = entry.into_meta();
    if meta.title.is_empty() {
        return None;
    }
    if meta.album.is_none() {
        meta.album = Some(album.to_owned());
    }
    let parsed = parse_input(&source_url);
    match parsed {
        ParsedInput::Track(mut track) => {
            track.meta = Some(meta);
            track.target = DownloadTarget::Url(source_url.clone());
            track.source_url = Some(source_url);
            Some(track)
        }
        _ => Some(TrackRequest {
            key: TrackKey::new(format!("{provider}:url:{source_url}")),
            provider,
            source_url: Some(source_url.clone()),
            meta: Some(meta),
            target: DownloadTarget::Url(source_url),
        }),
    }
}

async fn find_info_only(dir: &Path) -> Option<PathBuf> {
    // The audio path is known; we only need the .info.json sibling.
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".info.json") {
            return Some(entry.path());
        }
    }
    None
}

fn stdout_last_line(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    let mut last = None;
    for line in text.lines() {
        let line = line.trim();
        if !line.is_empty() {
            last = Some(line.to_owned());
        }
    }
    last
}

async fn find_output(dir: &Path) -> Result<(PathBuf, Option<PathBuf>), DownloadError> {
    let mut entries = tokio::fs::read_dir(dir).await?;
    let mut audio = Vec::new();
    let mut info = None;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".info.json") {
            info = Some(path);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("m4a" | "mp3" | "opus" | "ogg" | "flac" | "webm")
        ) {
            audio.push(path);
        }
    }
    audio.sort_by_key(|path| match path.extension().and_then(|ext| ext.to_str()) {
        Some("m4a") => 0,
        Some("mp3") => 1,
        Some("opus") => 2,
        Some("ogg") => 3,
        Some("flac") => 4,
        _ => 5,
    });
    audio
        .into_iter()
        .next()
        .map(|audio| (audio, info))
        .ok_or_else(|| DownloadError::Audio("yt-dlp produced no audio file".to_owned()))
}

async fn read_info(path: &Option<PathBuf>) -> Result<Option<Info>, DownloadError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let data = tokio::fs::read(path).await?;
    match serde_json::from_slice::<Info>(&data) {
        Ok(info) => Ok(Some(info)),
        Err(error) => {
            warn!(%error, "could not parse yt-dlp info json");
            Ok(None)
        }
    }
}

fn classify_failure(stderr: &str, timeout: Duration) -> DownloadError {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("private video")
        || lower.contains("private")
        || lower.contains("login")
        || lower.contains("sign in")
        || lower.contains("age-restricted")
        || lower.contains("age restricted")
    {
        DownloadError::Private
    } else if lower.contains("not available in your country")
        || lower.contains("geo-restricted")
        || lower.contains("geo restricted")
        || lower.contains("region")
    {
        DownloadError::RegionLocked
    } else if lower.contains("max-filesize") || lower.contains("file is larger") {
        DownloadError::TooLarge {
            limit: 0,
            actual: 0,
        }
    } else if lower.contains("video unavailable")
        || lower.contains("not found")
        || lower.contains("does not exist")
        || lower.contains("has been removed")
        || lower.contains("no video formats")
    {
        DownloadError::NotFound
    } else if lower.contains("timed out") || lower.contains("timeout") {
        DownloadError::SourceUnavailable(format!("yt-dlp timed out after {}s", timeout.as_secs()))
    } else {
        DownloadError::SourceUnavailable(trim_log(stderr))
    }
}

fn trim_log(value: &str) -> String {
    const KEEP: usize = 2000;
    let value = value.trim();
    if value.len() <= KEEP {
        return value.to_owned();
    }
    // Byte slicing must land on a char boundary: stderr can contain non-ASCII
    // titles and yt-dlp messages, and slicing inside a code point panics.
    let mut start = value.len() - KEEP;
    while !value.is_char_boundary(start) {
        start += 1;
    }
    value[start..].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_private_errors() {
        assert!(matches!(
            classify_failure("ERROR: Private video", Duration::from_secs(1)),
            DownloadError::Private
        ));
    }

    #[test]
    fn maps_region_errors() {
        assert!(matches!(
            classify_failure("not available in your country", Duration::from_secs(1)),
            DownloadError::RegionLocked
        ));
    }

    #[test]
    fn duration_only_accepts_positive() {
        assert_eq!(duration_secs(Some(12.6)), Some(13));
        assert_eq!(duration_secs(Some(0.0)), None);
        assert_eq!(duration_secs(Some(f64::NAN)), None);
    }

    #[test]
    fn trim_log_never_slices_inside_utf8() {
        // Total length 2002, so the old `len - 2000` start landed inside the
        // two-byte `ї` and panicked.
        let value = format!("a{}{}", "ї", "b".repeat(1999));
        assert_eq!(value.len(), 2002);
        let trimmed = trim_log(&value);
        assert!(trimmed.starts_with('b'));
        assert_eq!(trimmed.len(), 1999);
        assert_eq!(trim_log("short"), "short");
    }
}
