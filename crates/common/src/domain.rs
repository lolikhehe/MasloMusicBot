//! Core value types shared by the bot, providers, cache and downloader.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// User interface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Lang {
    #[default]
    En,
    Uk,
}

impl Lang {
    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Uk => "uk",
        }
    }

    pub fn from_code(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "en" | "english" => Some(Self::En),
            "uk" | "ua" | "ukrainian" => Some(Self::Uk),
            _ => None,
        }
    }

    /// Map a Telegram `language_code` to a supported UI language, falling
    /// back to `fallback` when unknown (spec §9 етап 6: мова з налаштувань,
    /// за відсутності — з language_code Telegram).
    pub fn from_telegram_hint(code: Option<&str>, fallback: Self) -> Self {
        match code {
            Some(value) => {
                let value = value.trim().to_ascii_lowercase();
                if value.starts_with("uk") || value.starts_with("ru") || value.starts_with("be") {
                    Self::Uk
                } else if value.starts_with("en") {
                    Self::En
                } else {
                    fallback
                }
            }
            None => fallback,
        }
    }
}

/// A supported music source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    YouTube,
    YouTubeMusic,
    SoundCloud,
    Bandcamp,
    Spotify,
    AppleMusic,
    Deezer,
}

impl Provider {
    pub const ALL: [Self; 7] = [
        Self::YouTube,
        Self::YouTubeMusic,
        Self::SoundCloud,
        Self::Bandcamp,
        Self::Spotify,
        Self::AppleMusic,
        Self::Deezer,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::YouTube => "youtube",
            Self::YouTubeMusic => "youtube_music",
            Self::SoundCloud => "soundcloud",
            Self::Bandcamp => "bandcamp",
            Self::Spotify => "spotify",
            Self::AppleMusic => "apple_music",
            Self::Deezer => "deezer",
        }
    }

    /// External providers supply metadata only; the audio is matched on an
    /// open source (YouTube by default).
    pub const fn is_external(self) -> bool {
        matches!(self, Self::Spotify | Self::AppleMusic | Self::Deezer)
    }

    /// Sources the user can pick for inline/text search in `/settings`.
    /// Spotify is deliberately absent: there is no anonymous search API.
    pub const SEARCH_CHOICES: [Self; 3] = [Self::YouTube, Self::SoundCloud, Self::Deezer];

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::YouTube => "YouTube",
            Self::YouTubeMusic => "YouTube Music",
            Self::SoundCloud => "SoundCloud",
            Self::Bandcamp => "Bandcamp",
            Self::Spotify => "Spotify",
            Self::AppleMusic => "Apple Music",
            Self::Deezer => "Deezer",
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "youtube" | "yt" => Ok(Self::YouTube),
            "youtube_music" | "youtube-music" | "ytmusic" | "ytm" => Ok(Self::YouTubeMusic),
            "soundcloud" | "sc" => Ok(Self::SoundCloud),
            "bandcamp" | "bc" => Ok(Self::Bandcamp),
            "spotify" | "sp" => Ok(Self::Spotify),
            "apple_music" | "apple-music" | "applemusic" | "apple" => Ok(Self::AppleMusic),
            "deezer" | "dz" => Ok(Self::Deezer),
            _ => Err(()),
        }
    }
}

/// Stable cache identity for a track. It is derived offline from the input,
/// never from network metadata, so a cache lookup needs no network access.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackKey(pub String);

impl TrackKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TrackKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Human-readable metadata shown by Telegram and written into audio tags.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMeta {
    pub title: String,
    pub performer: Option<String>,
    pub album: Option<String>,
    pub duration_secs: Option<u32>,
    pub thumbnail_url: Option<String>,
    /// Genre names as reported by the source (yt-dlp / Deezer / iTunes);
    /// empty when the source does not provide them.
    #[serde(default)]
    pub genres: Vec<String>,
}

impl TrackMeta {
    pub fn display_title(&self) -> String {
        match &self.performer {
            Some(performer) if !performer.trim().is_empty() => {
                format!("{performer} — {}", self.title)
            }
            _ => self.title.clone(),
        }
    }

    pub fn search_query(&self) -> String {
        match &self.performer {
            Some(performer) if !performer.trim().is_empty() => {
                format!("{} {}", performer.trim(), self.title.trim())
            }
            _ => self.title.trim().to_owned(),
        }
    }

    /// Caption for the sent audio when the per-user tags setting is on:
    /// the exact album title (when known) and genre hashtags. Language
    /// neutral on purpose. `None` when there is nothing to show.
    pub fn tags_caption(&self) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();
        if let Some(album) = self
            .album
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
        {
            lines.push(format!("💿 {album}"));
        }
        let tags: Vec<String> = self
            .genres
            .iter()
            .take(4)
            .filter_map(|genre| genre_hashtag(genre))
            .collect();
        if !tags.is_empty() {
            lines.push(tags.join(" "));
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }
}

/// Sanitize a genre name into a Telegram hashtag, keeping only alphanumeric
/// characters and underscores (`Hip Hop` → `#HipHop`, `r&b` → `#Rb`).
fn genre_hashtag(genre: &str) -> Option<String> {
    let cleaned: String = genre.chars().filter(|c| c.is_alphanumeric()).collect();
    if cleaned.is_empty() {
        return None;
    }
    Some(format!("#{cleaned}"))
}

/// What yt-dlp should be pointed at. URLs are validated by the provider layer;
/// `Search` is passed as a `ytsearch1:` pseudo-URL, `SoundCloudSearch` as
/// `scsearch1:` — the alternative-source fallback uses it when YouTube refuses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadTarget {
    Url(String),
    Search(String),
    SoundCloudSearch(String),
}

impl DownloadTarget {
    pub fn ytdlp_argument(&self) -> String {
        match self {
            Self::Url(url) => url.clone(),
            Self::Search(query) => format!("ytsearch1:{query}"),
            Self::SoundCloudSearch(query) => format!("scsearch1:{query}"),
        }
    }
}

/// A fully resolved request for one track. `key` is stable across restarts.
#[derive(Debug, Clone)]
pub struct TrackRequest {
    pub key: TrackKey,
    pub provider: Provider,
    pub source_url: Option<String>,
    pub meta: Option<TrackMeta>,
    pub target: DownloadTarget,
}

/// A playlist/album input that still needs to be expanded into tracks.
#[derive(Debug, Clone)]
pub struct AlbumRef {
    pub provider: Provider,
    pub url: Option<String>,
    pub title: Option<String>,
}

/// Expanded album. Tracks are downloaded one by one to keep memory bounded.
#[derive(Debug, Clone)]
pub struct Album {
    pub title: String,
    pub provider: Provider,
    pub tracks: Vec<TrackRequest>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub lang: Lang,
    pub quality: u32,
    pub banned: bool,
    /// Per-user search source used for inline and text search.
    pub search_source: Provider,
    /// Per-user switch for the album/genre tags caption on sent tracks.
    pub tags_enabled: bool,
    /// Connected public Last.fm username (inline "now playing" suggestions).
    pub lastfm_user: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CachedTrack {
    pub key: TrackKey,
    pub file_id: String,
    pub file_unique_id: String,
    pub source_url: Option<String>,
    pub provider: Provider,
    pub meta: TrackMeta,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub tracks: i64,
    pub users: i64,
    pub sends: i64,
}

/// One search result before the user picks it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub key: TrackKey,
    pub provider: Provider,
    pub source_url: String,
    pub meta: TrackMeta,
    pub target: DownloadTarget,
}

impl SearchResult {
    pub fn into_request(self) -> TrackRequest {
        TrackRequest {
            key: self.key,
            provider: self.provider,
            source_url: Some(self.source_url),
            meta: Some(self.meta),
            target: self.target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_round_trip() {
        for provider in Provider::ALL {
            assert_eq!(provider.as_str().parse::<Provider>(), Ok(provider));
        }
    }

    #[test]
    fn tags_caption_builds_album_and_hashtags() {
        let meta = TrackMeta {
            title: "Enter Sandman".into(),
            performer: Some("Metallica".into()),
            album: Some("Metallica (Remastered 2021)".into()),
            genres: vec!["Heavy Metal".into(), "r&b".into()],
            ..Default::default()
        };
        assert_eq!(
            meta.tags_caption().as_deref(),
            Some("💿 Metallica (Remastered 2021)\n#HeavyMetal #rb")
        );
        let bare = TrackMeta {
            title: "x".into(),
            album: None,
            genres: vec![],
            ..Default::default()
        };
        assert!(bare.tags_caption().is_none());
        let album_only = TrackMeta {
            title: "x".into(),
            album: Some("  ".into()),
            genres: vec!["Metal".into()],
            ..Default::default()
        };
        assert_eq!(album_only.tags_caption().as_deref(), Some("#Metal"));
    }

    #[test]
    fn search_choices_roundtrip_through_str() {
        for provider in Provider::SEARCH_CHOICES {
            assert_eq!(provider.as_str().parse::<Provider>(), Ok(provider));
        }
        assert!(!Provider::SEARCH_CHOICES.contains(&Provider::Spotify));
    }

    #[test]
    fn display_title_uses_em_dash() {
        let meta = TrackMeta {
            title: "Song".into(),
            performer: Some("Artist".into()),
            ..Default::default()
        };
        assert_eq!(meta.display_title(), "Artist — Song");
    }
}
