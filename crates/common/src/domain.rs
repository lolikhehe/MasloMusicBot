//! Core value types shared by the bot, providers, cache and downloader.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
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

    /// Telegram renders "performer – title"; strip a duplicated artist prefix
    /// that downloaded video titles often carry ("Artist - Title",
    /// "Artist Title") so the chat doesn't show the name twice.
    pub fn without_artist_prefix(&self) -> Self {
        match self.stripped_title() {
            Cow::Borrowed(_) => self.clone(),
            Cow::Owned(title) => {
                let mut meta = self.clone();
                meta.title = title;
                meta
            }
        }
    }

    /// Borrowing variant of [`Self::without_artist_prefix`] for send paths that
    /// only need the title. Avoids cloning the whole metadata (including the
    /// genre vector) on every cached delivery.
    pub fn stripped_title(&self) -> Cow<'_, str> {
        const SEPARATORS: &str = "-–—:|•/\\";
        let title = self.title.trim();
        let artist = self
            .performer
            .as_deref()
            .map(str::trim)
            .filter(|artist| !artist.is_empty());
        let Some(artist) = artist else {
            return Cow::Borrowed(&self.title);
        };
        let artist_len = artist.chars().count();
        if title.chars().count() <= artist_len
            || !title.to_lowercase().starts_with(&artist.to_lowercase())
        {
            return Cow::Borrowed(&self.title);
        }
        let rest: String = title.chars().skip(artist_len).collect();
        let rest = rest
            .trim_start()
            .trim_start_matches(|ch| SEPARATORS.contains(ch))
            .trim_start();
        if rest.is_empty() {
            Cow::Borrowed(&self.title)
        } else {
            Cow::Owned(rest.to_owned())
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
            // A blank line keeps the album title visually separate from the
            // hashtag block in Telegram's caption rendering.
            lines.push(String::new());
        }
        let tags = self.hashtags(8);
        if !tags.is_empty() {
            lines.push(tags.join(" "));
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    /// Up to `max` genre hashtags, in source order. Shared by the full tags
    /// caption and the compact inline caption.
    pub fn hashtags(&self, max: usize) -> Vec<String> {
        self.genres
            .iter()
            .take(max)
            .filter_map(|genre| genre_hashtag(genre))
            .collect()
    }
}

/// Sanitize a genre name into a readable Telegram hashtag: split on any
/// separator, title-case each word (`alternative rock` → `#AlternativeRock`).
fn genre_hashtag(genre: &str) -> Option<String> {
    if genre.trim().eq_ignore_ascii_case("r&b") {
        return Some("#RnB".to_owned());
    }
    let cleaned: String = genre
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &characters.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        Some(format!("#{cleaned}"))
    }
}

/// Last.fm tags include a lot of user noise ("seen live", "favorites", ...).
/// Drop it, de-duplicate case-insensitively, preserve source order, and keep at
/// most `max` useful genres.
pub fn clean_genres(tags: Vec<String>, max: usize) -> Vec<String> {
    const NOISE: &[&str] = &[
        "seen live",
        "favorites",
        "favourites",
        "favorite",
        "favourite",
        "own",
        "awesome",
        "liked",
        "love",
        "good",
        "best",
        "cool",
        "music",
        "songs",
        "seen",
        "want",
        "all time favorite",
        "my top",
        "top",
        "chill",
    ];
    let mut result: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.trim().to_owned();
        let lowered = tag.to_lowercase();
        if tag.is_empty()
            || tag.chars().count() > 30
            || NOISE.contains(&lowered.as_str())
            || result
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&tag))
        {
            continue;
        }
        result.push(tag);
        if result.len() >= max {
            break;
        }
    }
    result
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
/// Serialization lets selections live in Postgres so webhook callbacks can be
/// handled by any replica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRequest {
    pub key: TrackKey,
    pub provider: Provider,
    pub source_url: Option<String>,
    pub meta: Option<TrackMeta>,
    pub target: DownloadTarget,
}

/// A playlist/album input that still needs to be expanded into tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumRef {
    pub provider: Provider,
    pub url: Option<String>,
    pub title: Option<String>,
}

/// Expanded album. Tracks are downloaded one by one to keep memory bounded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub title: String,
    pub provider: Provider,
    pub tracks: Vec<TrackRequest>,
    pub source_url: Option<String>,
}

/// A public-catalog album candidate for `/alb <name>`. Deezer and Apple Music
/// are searched in parallel; candidates are shown as a keyboard and the
/// chosen one is resolved through the regular album pipeline.
#[derive(Debug, Clone)]
pub struct AlbumSearchResult {
    pub provider: Provider,
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub track_count: Option<u32>,
}

impl AlbumSearchResult {
    /// Normalized identity of the album, used to merge Deezer/Apple listings.
    pub fn dedupe_key(&self) -> String {
        let artist = self.artist.as_deref().unwrap_or("").trim().to_lowercase();
        format!("{artist}|{}", self.title.trim().to_lowercase())
    }

    pub fn into_album_ref(&self) -> AlbumRef {
        AlbumRef {
            provider: self.provider,
            url: Some(self.url.clone()),
            title: Some(self.title.clone()),
        }
    }
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
    /// Per-user switch for high-quality audio covers (1000px, JPEG q2).
    pub hq_cover: bool,
    /// Per-user switch for Last.fm scrobble stats (/profile, /collage).
    pub lastfm_stats: bool,
    /// Per-user switch for the genre analyzer (/genres).
    pub genre_analysis: bool,
    /// Output container chosen by `/convert`: `auto`, `m4a`, `mp3`, `flac`, `opus`.
    pub audio_format: String,
    /// Opt-in ffmpeg loudnorm pass.
    pub normalize_audio: bool,
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

/// Per-user counters for `/me`. Only tracks counted by the opt-in genre
/// analyzer are known individually; `listens` comes from the aggregate table.
#[derive(Debug, Clone, Default)]
pub struct UserStats {
    pub tracks: i64,
    pub listens: i64,
    pub genres: i64,
    /// Sum of the best-known duration of each distinct counted track, in secs.
    pub seconds: i64,
}

/// One row of the listening history (`/history`). Metadata only.
#[derive(Debug, Clone)]
pub struct ListenEntry {
    pub title: String,
    pub performer: Option<String>,
    pub created_at: i64,
}

/// A user's durable digest planner row (`digest_schedule`).
#[derive(Debug, Clone)]
pub struct DigestSchedule {
    /// `daily` or `weekly`.
    pub period: String,
    pub enabled: bool,
    pub next_send_at: i64,
    pub last_sent_at: Option<i64>,
    /// Half-open UTC hour window [start, end); equal values mean "no quiet hours".
    pub quiet_start_utc: i16,
    pub quiet_end_utc: i16,
}

/// A due digest claimed by the scheduler, joined with the user's language.
#[derive(Debug, Clone)]
pub struct DigestDue {
    pub user_id: i64,
    pub lang: Lang,
    pub period: String,
    pub next_send_at: i64,
    pub quiet_start_utc: i16,
    pub quiet_end_utc: i16,
}

/// Aggregated payload used to compose one digest message.
#[derive(Debug, Clone, Default)]
pub struct DigestSummary {
    pub listens: i64,
    pub tracks: i64,
    pub genres: Vec<(String, i64)>,
    pub top_tracks: Vec<(String, Option<String>, i64)>,
}

/// One `(time bucket, genre)` cell of the genre dynamics chart.
#[derive(Debug, Clone)]
pub struct DynamicsCell {
    pub bucket: i64,
    pub genre: String,
    pub listens: i64,
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
    fn artist_prefix_is_stripped_from_title() {
        let base = TrackMeta {
            title: "First Fragment - Mort Éphémère".into(),
            performer: Some("First Fragment".into()),
            ..Default::default()
        };
        assert_eq!(base.without_artist_prefix().title, "Mort Éphémère");

        let spaced = TrackMeta {
            title: "First Fragment Mort Éphémère".into(),
            ..base.clone()
        };
        assert_eq!(spaced.without_artist_prefix().title, "Mort Éphémère");

        let dash = TrackMeta {
            title: "Metallica – Master of Puppets".into(),
            performer: Some("metallica".into()),
            ..Default::default()
        };
        assert_eq!(dash.without_artist_prefix().title, "Master of Puppets");

        // No artist prefix at all: untouched.
        let plain = TrackMeta {
            title: "Master of Puppets".into(),
            performer: Some("Metallica".into()),
            ..Default::default()
        };
        assert_eq!(plain.without_artist_prefix().title, "Master of Puppets");

        // Title is only the artist: keep it (better than an empty title).
        let only_artist = TrackMeta {
            title: "Metallica".into(),
            performer: Some("Metallica".into()),
            ..Default::default()
        };
        assert_eq!(only_artist.without_artist_prefix().title, "Metallica");

        // No performer: untouched.
        let anon = TrackMeta {
            title: "First Fragment Mort Éphémère".into(),
            performer: None,
            ..Default::default()
        };
        assert_eq!(
            anon.without_artist_prefix().title,
            "First Fragment Mort Éphémère"
        );
    }

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
            Some("💿 Metallica (Remastered 2021)\n\n#HeavyMetal #RnB")
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
    fn clean_genres_drops_noise_and_duplicates() {
        let cleaned = super::clean_genres(
            vec![
                " Thrash Metal ".into(),
                "seen live".into(),
                "THRASH METAL".into(),
                "heavy metal".into(),
                "favorites".into(),
                "hard rock".into(),
            ],
            3,
        );
        assert_eq!(cleaned, ["Thrash Metal", "heavy metal", "hard rock"]);
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
