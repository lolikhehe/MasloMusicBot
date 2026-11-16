//! Minimal Last.fm client (public API) used for the "now playing" inline
//! suggestion. The bot never stores Last.fm passwords — the user only
//! provides their public username, and we read the public "recent tracks"
//! feed with the operator's API key.
//!
//! Disabled entirely when LASTFM_API_KEY is not set (no key in the URL, the
//! API refuses every call, so nothing is fetched at all).

use common::domain::TrackMeta;
use common::error::DownloadError;
use serde::Deserialize;

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
}

fn map_reqwest(error: reqwest::Error) -> DownloadError {
    DownloadError::SourceUnavailable(format!("lastfm request failed: {error}"))
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
}
