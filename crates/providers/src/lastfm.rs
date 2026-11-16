//! Minimal Last.fm client (public API) used for the "now playing" inline
//! suggestion. The bot never stores Last.fm passwords — the user only
//! provides their public username, and we read the public "recent tracks"
//! feed with the operator's API key.
//!
//! Disabled entirely when LASTFM_API_KEY is not set (no key in the URL, the
//! API refuses every call, so nothing is fetched at all).

use common::domain::{Lang, TrackMeta};
use common::error::DownloadError;
use serde::Deserialize;

/// Period accepted by Last.fm chart endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsPeriod {
    Week,
    Month,
    ThreeMonths,
    SixMonths,
    Year,
    Overall,
}

impl StatsPeriod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Week => "7day",
            Self::Month => "1month",
            Self::ThreeMonths => "3month",
            Self::SixMonths => "6month",
            Self::Year => "12month",
            Self::Overall => "overall",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "7day" | "7days" | "week" | "7d" => Some(Self::Week),
            "1month" | "1months" | "month" | "1m" => Some(Self::Month),
            "3month" | "3months" | "3m" => Some(Self::ThreeMonths),
            "6month" | "6months" | "6m" => Some(Self::SixMonths),
            "12month" | "12months" | "12m" | "year" | "1y" => Some(Self::Year),
            "overall" | "all" | "alltime" => Some(Self::Overall),
            _ => None,
        }
    }

    pub fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Self::Week, Lang::En) => "7 days",
            (Self::Week, Lang::Uk) => "7 днів",
            (Self::Month, Lang::En) => "1 month",
            (Self::Month, Lang::Uk) => "1 місяць",
            (Self::ThreeMonths, Lang::En) => "3 months",
            (Self::ThreeMonths, Lang::Uk) => "3 місяці",
            (Self::SixMonths, Lang::En) => "6 months",
            (Self::SixMonths, Lang::Uk) => "6 місяців",
            (Self::Year, Lang::En) => "12 months",
            (Self::Year, Lang::Uk) => "12 місяців",
            (Self::Overall, Lang::En) => "all time",
            (Self::Overall, Lang::Uk) => "весь час",
        }
    }
}

/// Which chart to fetch for a Last.fm profile / collage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsKind {
    Tracks,
    Albums,
    Artists,
}

impl StatsKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tracks => "tracks",
            Self::Albums => "albums",
            Self::Artists => "artists",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "track" | "tracks" | "trek" | "треки" | "т" => Some(Self::Tracks),
            "album" | "albums" | "альбом" | "альбоми" | "а" => Some(Self::Albums),
            "artist" | "artists" | "виконавець" | "виконавці" | "в" => {
                Some(Self::Artists)
            }
            _ => None,
        }
    }
}

/// One row of a Last.fm chart. `secondary` is the artist for tracks/albums.
#[derive(Debug, Clone)]
pub struct ScrobbleItem {
    pub name: String,
    pub secondary: Option<String>,
    pub playcount: u64,
    /// Largest artwork URL Last.fm returned for this item (album/track/artist
    /// cover). Used to draw real covers in collages instead of empty cells.
    pub image_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProfileSummary {
    pub name: String,
    pub playcount: u64,
    pub tracks: u64,
    pub albums: u64,
    pub artists: u64,
    pub country: Option<String>,
}

const API_URL: &str = "https://ws.audioscrobbler.com/2.0/";

#[derive(Clone)]
pub struct LastFmClient {
    api_key: String,
    client: reqwest::Client,
}

/// One recently/now playing track.
#[derive(Debug, Clone)]
pub struct NowPlaying {
    pub meta: TrackMeta,
    /// `true` when the track has the `@attr.nowplaying` marker.
    pub is_now_playing: bool,
    /// Public Last.fm page for the track.
    pub url: Option<String>,
}

impl LastFmClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .user_agent("music-bot/0.1 (Telegram music bot)")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Fetch the user's latest track. Prefers the `nowplaying` entry, falls
    /// back to the most recent scrobbled track.
    pub async fn now_playing(&self, user: &str) -> Result<Option<NowPlaying>, DownloadError> {
        let url = "https://ws.audioscrobbler.com/2.0/";
        let response = self
            .client
            .get(url)
            .query(&[
                ("method", "user.getRecentTracks"),
                ("user", user),
                ("api_key", &self.api_key),
                ("format", "json"),
                ("limit", "1"),
            ])
            .send()
            .await
            .map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Err(DownloadError::SourceUnavailable(format!(
                "lastfm rejected the request: {}",
                response.status()
            )));
        }
        let body: RecentTracksResponse = response.json().await.map_err(map_reqwest)?;
        if let Some(error) = body.error {
            // Public API error payload, e.g. {"error":6,"message":"User not found"}.
            return Err(DownloadError::SourceUnavailable(format!(
                "lastfm error {error}: {}",
                body.message.unwrap_or_else(|| "unknown".to_owned())
            )));
        }
        let Some(track) = body
            .recent_tracks
            .and_then(|page| page.track.into_iter().next())
        else {
            return Ok(None);
        };
        Ok(Some(track.into_now_playing()))
    }

    /// Community-curated genre tags for a track — far richer and more accurate
    /// than the single coarse genre Apple/iTunes exposes. `track.getInfo`
    /// returns up to five toptags; `artist.getTopTags` is the fallback when the
    /// track page has none (e.g. obscure uploads). Returns names sorted by
    /// Last.fm's tag count, already trimmed and de-duplicated.
    pub async fn track_tags(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<Vec<String>, DownloadError> {
        let artist = artist.trim();
        let title = title.trim();
        if title.is_empty() || artist.is_empty() {
            return Ok(Vec::new());
        }
        let url = "https://ws.audioscrobbler.com/2.0/";
        let info: TrackInfoResponse = self
            .client
            .get(url)
            .query(&[
                ("method", "track.getInfo"),
                ("artist", artist),
                ("track", title),
                ("api_key", &self.api_key),
                ("format", "json"),
                ("autocorrect", "1"),
            ])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;
        let tags = info
            .track
            .and_then(|track| track.toptags)
            .map(TagList::into_names)
            .unwrap_or_default();
        if !tags.is_empty() {
            return Ok(tags);
        }

        let top: ArtistTagsResponse = self
            .client
            .get(url)
            .query(&[
                ("method", "artist.getTopTags"),
                ("artist", artist),
                ("api_key", &self.api_key),
                ("format", "json"),
            ])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;
        Ok(top.toptags.map(TagList::into_names).unwrap_or_default())
    }

    /// Public profile totals: scrobbles, library size and country.
    pub async fn profile(&self, user: &str) -> Result<ProfileSummary, DownloadError> {
        let body: UserInfoResponse = self
            .client
            .get(API_URL)
            .query(&[
                ("method", "user.getInfo"),
                ("user", user),
                ("api_key", &self.api_key),
                ("format", "json"),
            ])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;
        let info = body
            .user
            .ok_or_else(|| DownloadError::SourceUnavailable("Last.fm user not found".to_owned()))?;
        Ok(ProfileSummary {
            name: info.name,
            playcount: u64::from(info.playcount.unwrap_or(0)),
            tracks: u64::from(info.track_count.unwrap_or(0)),
            albums: u64::from(info.album_count.unwrap_or(0)),
            artists: u64::from(info.artist_count.unwrap_or(0)),
            country: info.country.filter(|value| !value.is_empty()),
        })
    }

    /// Top tracks / albums / artists for a user in the given period, ordered by
    /// playcount as returned by Last.fm. `limit` is clamped to 100.
    pub async fn top_items(
        &self,
        user: &str,
        period: StatsPeriod,
        kind: StatsKind,
        limit: usize,
    ) -> Result<Vec<ScrobbleItem>, DownloadError> {
        let method = match kind {
            StatsKind::Tracks => "user.getTopTracks",
            StatsKind::Albums => "user.getTopAlbums",
            StatsKind::Artists => "user.getTopArtists",
        };
        let limit = limit.clamp(1, 100).to_string();
        let response = self
            .client
            .get(API_URL)
            .query(&[
                ("method", method),
                ("user", user),
                ("period", period.as_str()),
                ("limit", limit.as_str()),
                ("api_key", &self.api_key),
                ("format", "json"),
            ])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json::<serde_json::Value>()
            .await
            .map_err(map_reqwest)?;
        let root_key = match kind {
            StatsKind::Tracks => "toptracks",
            StatsKind::Albums => "topalbums",
            StatsKind::Artists => "topartists",
        };
        let row_key = match kind {
            StatsKind::Tracks => "track",
            StatsKind::Albums => "album",
            StatsKind::Artists => "artist",
        };
        let mut items: Vec<ScrobbleItem> = response
            .get(root_key)
            .and_then(|page| page.get(row_key))
            .and_then(|rows| rows.as_array())
            .into_iter()
            .flatten()
            .filter_map(|row| {
                let name = row.get("name")?.as_str()?.trim().to_owned();
                if name.is_empty() {
                    return None;
                }
                let secondary = row
                    .get("artist")
                    .and_then(|artist| {
                        artist
                            .get("name")
                            .and_then(|name| name.as_str())
                            .or_else(|| artist.as_str())
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let playcount = row
                    .get("playcount")
                    .and_then(|value| {
                        value
                            .as_str()
                            .and_then(|text| text.parse::<u64>().ok())
                            .or_else(|| value.as_u64())
                    })
                    .unwrap_or(0);
                // Last.fm orders images small → extralarge/mega; the last one
                // is the biggest available.
                let image_url = row
                    .get("image")
                    .and_then(|images| images.as_array())
                    .and_then(|images| {
                        images
                            .iter()
                            .filter_map(|image| image.get("#text")?.as_str())
                            .filter(|url| !url.is_empty())
                            .last()
                    })
                    .map(str::to_owned);
                Some(ScrobbleItem {
                    name,
                    secondary,
                    playcount,
                    image_url,
                })
            })
            .collect();
        items.sort_by(|left, right| right.playcount.cmp(&left.playcount));
        Ok(items)
    }
}

fn map_reqwest(error: reqwest::Error) -> DownloadError {
    DownloadError::SourceUnavailable(format!("lastfm request failed: {error}"))
}

/// Last.fm returns a single object instead of an array when there is only one
/// item — handle both shapes so a one-tag response doesn't fail to parse.
fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        One(T),
        Many(Vec<T>),
    }
    Ok(match Option::<OneOrMany<T>>::deserialize(deserializer)? {
        Some(OneOrMany::One(value)) => vec![value],
        Some(OneOrMany::Many(values)) => values,
        None => Vec::new(),
    })
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    #[serde(default)]
    user: Option<UserInfo>,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    #[serde(default)]
    name: String,
    #[serde(default, deserialize_with = "optional_number_string")]
    playcount: Option<u32>,
    #[serde(
        default,
        rename = "track_count",
        deserialize_with = "optional_number_string"
    )]
    track_count: Option<u32>,
    #[serde(
        default,
        rename = "album_count",
        deserialize_with = "optional_number_string"
    )]
    album_count: Option<u32>,
    #[serde(
        default,
        rename = "artist_count",
        deserialize_with = "optional_number_string"
    )]
    artist_count: Option<u32>,
    #[serde(default)]
    country: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrackInfoResponse {
    #[serde(default)]
    track: Option<TrackInfo>,
    #[serde(default)]
    error: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct TrackInfo {
    #[serde(default)]
    toptags: Option<TagList>,
}

#[derive(Debug, Deserialize)]
struct ArtistTagsResponse {
    #[serde(default)]
    toptags: Option<TagList>,
    #[serde(default)]
    error: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct TagList {
    #[serde(default, deserialize_with = "one_or_many")]
    tag: Vec<LastFmTag>,
}

/// Last.fm serializes tag counts as strings ("90"), occasionally numbers.
fn optional_number_string<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Number(u32),
        Text(String),
    }
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::Number(number)) => Some(number),
        Some(Value::Text(text)) => text.parse().ok(),
        None => None,
    })
}

#[derive(Debug, Deserialize)]
struct LastFmTag {
    #[serde(default)]
    name: String,
    #[serde(default, deserialize_with = "optional_number_string")]
    count: Option<u32>,
}

impl TagList {
    fn into_names(self) -> Vec<String> {
        let mut tags: Vec<(u32, String)> = self
            .tag
            .into_iter()
            .filter(|tag| !tag.name.trim().is_empty())
            .map(|tag| (tag.count.unwrap_or(0), tag.name.trim().to_owned()))
            .collect();
        tags.sort_by(|left, right| right.0.cmp(&left.0));
        tags.into_iter().map(|(_, name)| name).collect()
    }
}

#[derive(Debug, Deserialize)]
struct RecentTracksResponse {
    #[serde(default, rename = "recenttracks", alias = "recentTracks")]
    recent_tracks: Option<RecentTracksPage>,
    #[serde(default)]
    error: Option<i32>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecentTracksPage {
    #[serde(default)]
    track: Vec<RecentTrack>,
}

#[derive(Debug, Deserialize)]
struct RecentTrack {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    artist: Option<LastFmArtist>,
    #[serde(default)]
    album: Option<LastFmAlbum>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    image: Vec<LastFmImage>,
    #[serde(default)]
    duration: Option<String>,
    #[serde(default, rename = "@attr")]
    attr: Option<TrackAttr>,
}

#[derive(Debug, Deserialize)]
struct TrackAttr {
    #[serde(default)]
    nowplaying: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LastFmArtist {
    #[serde(default, rename = "#text")]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LastFmAlbum {
    #[serde(default, rename = "#text")]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LastFmImage {
    #[serde(default, rename = "#text")]
    url: Option<String>,
    #[serde(default)]
    size: Option<String>,
}

impl LastFmImage {
    fn rank(&self) -> u8 {
        match self.size.as_deref() {
            Some("mega") => 4,
            Some("extralarge") => 3,
            Some("large") => 2,
            Some("medium") => 1,
            _ => 0,
        }
    }
}

impl RecentTrack {
    fn into_now_playing(self) -> NowPlaying {
        let title = self.name.unwrap_or_default().trim().to_owned();
        let artist = self
            .artist
            .and_then(|artist| artist.text)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let album = self
            .album
            .and_then(|album| album.text)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let thumbnail_url = self
            .image
            .into_iter()
            .max_by_key(|image| image.rank())
            .and_then(|image| image.url.filter(|url| !url.is_empty()));
        let duration_secs = self
            .duration
            .as_deref()
            .and_then(|value| value.parse::<u32>().ok());
        NowPlaying {
            meta: TrackMeta {
                title,
                performer: artist,
                album,
                duration_secs,
                thumbnail_url,
                ..Default::default()
            },
            is_now_playing: self
                .attr
                .and_then(|attr| attr.nowplaying)
                .is_some_and(|value| value == "true"),
            url: self.url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lowercase_collection_key() {
        // Real payload shape captured from ws.audioscrobbler.com (top-level
        // collection key is lowercase, not camelCase).
        let json = r##"{
            "recenttracks": {
                "track": [{
                    "name": "Walk",
                    "artist": {"#text": "Pantera", "mbid": ""},
                    "album": {"#text": "Vulgar Display of Power", "mbid": ""},
                    "duration": "327",
                    "@attr": {"nowplaying": "true"}
                }]
            },
            "@attr": {"user": "someone", "total": "1"}
        }"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        let now = body
            .recent_tracks
            .expect("page")
            .track
            .into_iter()
            .next()
            .expect("track")
            .into_now_playing();
        assert!(now.is_now_playing);
        assert_eq!(now.meta.title, "Walk");
        assert_eq!(now.meta.performer.as_deref(), Some("Pantera"));
    }

    #[test]
    fn parses_now_playing_track() {
        let json = r##"{
            "recentTracks": { "track": [ {
                "name": "Enter Sandman",
                "artist": {"#text": "Metallica", "mbid": ""},
                "album": {"#text": "Metallica", "mbid": ""},
                "url": "https://www.last.fm/music/Metallica/_/Enter%2BSandman",
                "image": [
                    {"#text": "https://x/small.jpg", "size": "small"},
                    {"#text": "https://x/mega.jpg", "size": "mega"}
                ],
                "duration": "331",
                "@attr": {"nowplaying": "true"}
            } ] }
        }"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        let tracks = body.recent_tracks.expect("page").track;
        let now = tracks.into_iter().next().expect("track").into_now_playing();
        assert!(now.is_now_playing);
        assert_eq!(now.meta.title, "Enter Sandman");
        assert_eq!(now.meta.performer.as_deref(), Some("Metallica"));
        assert_eq!(now.meta.album.as_deref(), Some("Metallica"));
        assert_eq!(now.meta.duration_secs, Some(331));
        assert_eq!(
            now.meta.thumbnail_url.as_deref(),
            Some("https://x/mega.jpg")
        );
        assert_eq!(
            now.url.as_deref(),
            Some("https://www.last.fm/music/Metallica/_/Enter%2BSandman")
        );
    }

    #[test]
    fn parses_last_played_without_nowplaying_marker() {
        let json = r##"{
            "recentTracks": { "track": [ {
                "name": "One",
                "artist": {"#text": "Metallica"},
                "album": {"#text": "…And Justice for All"},
                "url": "https://www.last.fm/music/Metallica/_/One"
            } ] }
        }"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        let now = body
            .recent_tracks
            .expect("page")
            .track
            .into_iter()
            .next()
            .expect("track")
            .into_now_playing();
        assert!(!now.is_now_playing);
        assert_eq!(now.meta.title, "One");
        assert!(now.meta.duration_secs.is_none());
        assert!(now.meta.thumbnail_url.is_none());
    }

    #[test]
    fn surfaces_api_error_payload() {
        let json = r##"{"error":6,"message":"User not found"}"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        assert_eq!(body.error, Some(6));
        assert_eq!(body.message.as_deref(), Some("User not found"));
    }

    #[test]
    fn track_toptags_are_sorted_by_count() {
        let json = r##"{
            "track": {
                "name": "Enter Sandman",
                "toptags": {"tag": [
                    {"name": "metal", "count": "90"},
                    {"name": "thrash metal", "count": "75"},
                    {"name": "hard rock", "count": "40"}
                ]}
            }
        }"##;
        let body: TrackInfoResponse = serde_json::from_str(json).expect("json");
        let tags = body
            .track
            .expect("track")
            .toptags
            .expect("toptags")
            .into_names();
        assert_eq!(tags, ["metal", "thrash metal", "hard rock"]);
    }

    #[test]
    fn single_tag_object_is_accepted() {
        // Last.fm serializes one tag as an object, not a one-element array.
        let json = r##"{"toptags":{"tag":{"name":"alternative rock","count":"12"}}}"##;
        let body: ArtistTagsResponse = serde_json::from_str(json).expect("json");
        assert_eq!(
            body.toptags.expect("toptags").into_names(),
            ["alternative rock"]
        );
    }

    #[test]
    fn stats_periods_parse_all_supported_aliases() {
        assert_eq!(StatsPeriod::parse("6m"), Some(StatsPeriod::SixMonths));
        assert_eq!(StatsPeriod::parse("6month"), Some(StatsPeriod::SixMonths));
        assert_eq!(StatsPeriod::parse("1y"), Some(StatsPeriod::Year));
        assert_eq!(StatsPeriod::parse("12month"), Some(StatsPeriod::Year));
        assert_eq!(StatsPeriod::parse("overall"), Some(StatsPeriod::Overall));
        assert_eq!(StatsPeriod::parse("all"), Some(StatsPeriod::Overall));
        assert_eq!(StatsPeriod::parse("3M"), Some(StatsPeriod::ThreeMonths));
        assert_eq!(StatsPeriod::parse("garbage"), None);
        assert_eq!(StatsPeriod::Overall.as_str(), "overall");
    }

    #[test]
    fn missing_track_yields_no_tags() {
        let json = r##"{"error":6,"message":"Track not found"}"##;
        let body: TrackInfoResponse = serde_json::from_str(json).expect("json");
        assert!(body.track.is_none());
    }
}
