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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

    /// Cutoff for local genre events; `None` means "all time".
    pub fn window_secs(self) -> Option<i64> {
        match self {
            Self::Week => Some(7 * 86_400),
            Self::Month => Some(30 * 86_400),
            Self::ThreeMonths => Some(90 * 86_400),
            Self::SixMonths => Some(180 * 86_400),
            Self::Year => Some(365 * 86_400),
            Self::Overall => None,
        }
    }

    pub fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (
                Self::Week,
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be,
            ) => "7 days",
            (Self::Week, Lang::Uk) => "7 днів",
            (
                Self::Month,
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be,
            ) => "1 month",
            (Self::Month, Lang::Uk) => "1 місяць",
            (
                Self::ThreeMonths,
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be,
            ) => "3 months",
            (Self::ThreeMonths, Lang::Uk) => "3 місяці",
            (
                Self::SixMonths,
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be,
            ) => "6 months",
            (Self::SixMonths, Lang::Uk) => "6 місяців",
            (
                Self::Year,
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be,
            ) => "12 months",
            (Self::Year, Lang::Uk) => "12 місяців",
            (
                Self::Overall,
                Lang::En
                | Lang::Pl
                | Lang::De
                | Lang::Cn
                | Lang::Jp
                | Lang::Fr
                | Lang::Nl
                | Lang::Cz
                | Lang::Be,
            ) => "all time",
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

/// Last.fm serves a grey star for items without artwork. Those URLs are real
/// images (so a plain "is the URL non-empty" check passes) but must never be
/// drawn as covers; the caller falls back to a public catalog lookup instead.
pub fn is_placeholder_image(url: &str) -> bool {
    const PLACEHOLDERS: &[&str] = &[
        "2a96cbd8b46e442fc41c2b86b821562f",
        "b26d6fd11de240a1c045dfb5c5d9fe65",
        "c6f59c1e5e7240a4c0d427abd71f3dbb",
        "d41d8cd98f00b204e9800998ecf8427e",
    ];
    PLACEHOLDERS.iter().any(|hash| url.contains(hash))
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
/// Error code 29 means the key is being throttled. Back off so a burst of
/// parallel charts can't get the operator's key banned.
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct LastFmClient {
    api_key: String,
    client: reqwest::Client,
    /// Instant a code-29 reply was last seen; all calls refuse until it expires.
    rate_limited_at: Arc<Mutex<Option<Instant>>>,
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
    /// A failed builder must surface as an error: `Client::default()` would have
    /// no timeout and let a hung Last.fm call block a request forever.
    pub fn new(api_key: String) -> Result<Self, reqwest::Error> {
        Ok(Self {
            api_key,
            client: crate::http::client(Duration::from_secs(5))?,
            rate_limited_at: Arc::new(Mutex::new(None)),
        })
    }

    fn ensure_ready(&self) -> Result<(), DownloadError> {
        let recorded = self
            .rate_limited_at
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if recorded.is_some_and(|at| at.elapsed() < RATE_LIMIT_COOLDOWN) {
            return Err(DownloadError::SourceUnavailable(
                "last.fm: rate limited, cooling down".to_owned(),
            ));
        }
        Ok(())
    }

    fn note_rate_limit(&self, code: Option<u16>) {
        if code == Some(29) {
            let mut recorded = self
                .rate_limited_at
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            *recorded = Some(Instant::now());
        }
    }

    fn check_error(&self, error: &LastFmError) -> Result<(), DownloadError> {
        self.note_rate_limit(error.error);
        error.check()
    }

    /// Parse a JSON reply from a bounded body: `.json()` buffers whatever the
    /// server sends, while `fetch_bytes` stops at the cap (256 KB here).
    async fn fetch_json<T: serde::de::DeserializeOwned>(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<T, DownloadError> {
        let url = builder.build().map_err(map_reqwest)?.url().clone();
        let bytes = crate::http::fetch_bytes(&self.client, url.as_str(), 256 * 1024)
            .await
            .ok_or_else(|| {
                DownloadError::SourceUnavailable(
                    "lastfm response empty, oversized, or failed".to_owned(),
                )
            })?;
        serde_json::from_slice(&bytes).map_err(|error| {
            DownloadError::SourceUnavailable(format!("lastfm invalid response: {error}"))
        })
    }

    /// Fetch the user's latest track. Prefers the `nowplaying` entry, falls
    /// back to the most recent scrobbled track.
    pub async fn now_playing(&self, user: &str) -> Result<Option<NowPlaying>, DownloadError> {
        self.ensure_ready()?;
        let body: RecentTracksResponse = self
            .fetch_json(self.client.get(API_URL).query(&[
                ("method", "user.getRecentTracks"),
                ("user", user),
                ("api_key", &self.api_key),
                ("format", "json"),
                ("limit", "1"),
            ]))
            .await?;
        if let Some(error) = body.error {
            // Public API error payload, e.g. {"error":6,"message":"User not found"}.
            self.note_rate_limit(u16::try_from(error).ok());
            return Err(DownloadError::SourceUnavailable(format!(
                "lastfm error {error}: {}",
                body.message.unwrap_or_else(|| "unknown".to_owned())
            )));
        }
        let Some(track) = body.recent_tracks.and_then(first_named_track) else {
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
        self.ensure_ready()?;
        let artist = artist.trim();
        let title = title.trim();
        if title.is_empty() || artist.is_empty() {
            return Ok(Vec::new());
        }
        let info: TrackInfoResponse = self
            .fetch_json(self.client.get(API_URL).query(&[
                ("method", "track.getInfo"),
                ("artist", artist),
                ("track", title),
                ("api_key", &self.api_key),
                ("format", "json"),
                ("autocorrect", "1"),
            ]))
            .await?;
        self.check_error(&info.error)?;
        let tags = info
            .track
            .and_then(|track| track.toptags)
            .map(TagList::into_names)
            .unwrap_or_default();
        if !tags.is_empty() {
            return Ok(tags);
        }

        let top: ArtistTagsResponse = self
            .fetch_json(self.client.get(API_URL).query(&[
                ("method", "artist.getTopTags"),
                ("artist", artist),
                ("api_key", &self.api_key),
                ("format", "json"),
            ]))
            .await?;
        self.check_error(&top.error)?;
        Ok(top.toptags.map(TagList::into_names).unwrap_or_default())
    }

    /// Full artist biography from `artist.getinfo` (`bio.content`, falling
    /// back to `bio.summary`) with the "Read more on Last.fm" link stripped.
    /// `None` when Last.fm has no biography for the name.
    pub async fn artist_bio(&self, artist: &str) -> Result<Option<String>, DownloadError> {
        self.ensure_ready()?;
        let artist = artist.trim();
        if artist.is_empty() {
            return Ok(None);
        }
        let body: ArtistInfoResponse = self
            .fetch_json(self.client.get(API_URL).query(&[
                ("method", "artist.getinfo"),
                ("artist", artist),
                ("api_key", &self.api_key),
                ("format", "json"),
                ("autocorrect", "1"),
            ]))
            .await?;
        self.check_error(&body.error)?;
        let bio = body
            .artist
            .and_then(|artist| artist.bio)
            .and_then(|bio| pick_bio(bio.content, bio.summary));
        Ok(bio)
    }

    /// Album description from `album.getinfo` (`wiki.content`, falling back to
    /// `wiki.summary`) with links stripped. `None` when Last.fm has no
    /// editorial for the release.
    pub async fn album_wiki(
        &self,
        artist: &str,
        album: &str,
    ) -> Result<Option<String>, DownloadError> {
        self.ensure_ready()?;
        let artist = artist.trim();
        let album = album.trim();
        if artist.is_empty() || album.is_empty() {
            return Ok(None);
        }
        let body: AlbumInfoResponse = self
            .fetch_json(self.client.get(API_URL).query(&[
                ("method", "album.getinfo"),
                ("artist", artist),
                ("album", album),
                ("api_key", &self.api_key),
                ("format", "json"),
                ("autocorrect", "1"),
            ]))
            .await?;
        self.check_error(&body.error)?;
        let wiki = body
            .album
            .and_then(|album| album.wiki)
            .and_then(|wiki| {
                wiki.content
                    .filter(|content| !content.trim().is_empty())
                    .or(wiki.summary)
            })
            .map(|text| strip_read_more(&text))
            .filter(|text| !text.is_empty());
        Ok(wiki)
    }

    /// Up to 10 `(artist, title)` pairs similar to a track, from
    /// `track.getsimilar`. Empty when Last.fm knows nothing similar.
    pub async fn similar_tracks(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<Vec<(String, String)>, DownloadError> {
        self.ensure_ready()?;
        let artist = artist.trim();
        let title = title.trim();
        if artist.is_empty() || title.is_empty() {
            return Ok(Vec::new());
        }
        let body: SimilarResponse = self
            .fetch_json(self.client.get(API_URL).query(&[
                ("method", "track.getsimilar"),
                ("artist", artist),
                ("track", title),
                ("api_key", &self.api_key),
                ("format", "json"),
                ("autocorrect", "1"),
                ("limit", "10"),
            ]))
            .await?;
        self.check_error(&body.error)?;
        let similar = body
            .similar
            .map(|similar| similar.track)
            .unwrap_or_default();
        Ok(similar
            .into_iter()
            .filter(|track| {
                !track.name.trim().is_empty()
                    && track
                        .artist
                        .as_ref()
                        .is_some_and(|artist| !artist.name.trim().is_empty())
            })
            .map(|track| {
                (
                    track.artist.map(|artist| artist.name).unwrap_or_default(),
                    track.name,
                )
            })
            .collect())
    }

    /// Public profile totals: scrobbles, library size and country.
    pub async fn profile(&self, user: &str) -> Result<ProfileSummary, DownloadError> {
        self.ensure_ready()?;
        let body: UserInfoResponse = self
            .fetch_json(self.client.get(API_URL).query(&[
                ("method", "user.getInfo"),
                ("user", user),
                ("api_key", &self.api_key),
                ("format", "json"),
            ]))
            .await?;
        self.check_error(&body.error)?;
        let info = body
            .user
            .ok_or_else(|| DownloadError::SourceUnavailable("Last.fm user not found".to_owned()))?;
        Ok(ProfileSummary {
            name: info.name,
            playcount: u64::from(info.playcount.unwrap_or(0)),
            tracks: u64::from(info.track_count.unwrap_or(0)),
            albums: u64::from(info.album_count.unwrap_or(0)),
            artists: u64::from(info.artist_count.unwrap_or(0)),
            // Last.fm sends the literal "None" for users without a country.
            country: info.country.filter(|value| {
                let value = value.trim();
                !value.is_empty() && !value.eq_ignore_ascii_case("none")
            }),
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
        self.ensure_ready()?;
        let method = match kind {
            StatsKind::Tracks => "user.getTopTracks",
            StatsKind::Albums => "user.getTopAlbums",
            StatsKind::Artists => "user.getTopArtists",
        };
        let limit = limit.clamp(1, 100).to_string();
        let response: serde_json::Value = self
            .fetch_json(self.client.get(API_URL).query(&[
                ("method", method),
                ("user", user),
                ("period", period.as_str()),
                ("limit", limit.as_str()),
                ("api_key", &self.api_key),
                ("format", "json"),
            ]))
            .await?;
        self.note_rate_limit(
            response
                .get("error")
                .and_then(|value| value.as_u64())
                .and_then(|code| u16::try_from(code).ok()),
        );
        check_value_error(&response)?;
        let mut items = parse_top_items(kind, &response);
        items.sort_by_key(|item| std::cmp::Reverse(item.playcount));
        Ok(items)
    }
}

/// Pure parser for `user.getTop*` payloads, kept separate so the placeholder
/// filtering can be unit-tested without network access.
fn parse_top_items(kind: StatsKind, response: &serde_json::Value) -> Vec<ScrobbleItem> {
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
    response
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
            // is the biggest available. Skip the grey-star placeholder:
            // for top tracks/artists it is frequently the only image, and
            // drawing it looked like broken artwork in collages.
            let image_url = row
                .get("image")
                .and_then(|images| images.as_array())
                .and_then(|images| {
                    images
                        .iter()
                        .filter_map(|image| image.get("#text")?.as_str())
                        .rfind(|url| !url.is_empty() && !is_placeholder_image(url))
                })
                .map(str::to_owned);
            Some(ScrobbleItem {
                name,
                secondary,
                playcount,
                image_url,
            })
        })
        .collect()
}

/// Never use Display here: reqwest embeds the URL, including `api_key`.
fn map_reqwest(error: reqwest::Error) -> DownloadError {
    match error.status() {
        Some(status) => {
            DownloadError::SourceUnavailable(format!("lastfm request failed: {status}"))
        }
        None => DownloadError::SourceUnavailable("lastfm request failed: network error".to_owned()),
    }
}

/// Last.fm reports application errors (e.g. code 29 "Rate limit exceeded")
/// inside an HTTP 200 JSON body; without this every failure parses as "no
/// data" and the user is told there are no results.
#[derive(Debug, Default, Deserialize)]
struct LastFmError {
    #[serde(default)]
    error: Option<u16>,
    #[serde(default)]
    message: Option<String>,
}

impl LastFmError {
    fn check(&self) -> Result<(), DownloadError> {
        if self.error.is_some() {
            let message = self
                .message
                .clone()
                .unwrap_or_else(|| "unknown error".to_owned());
            return Err(DownloadError::SourceUnavailable(format!(
                "last.fm: {message}"
            )));
        }
        Ok(())
    }
}

/// Value-based counterpart of [`LastFmError::check`] for endpoints that parse
/// the raw JSON (top_items).
fn check_value_error(value: &serde_json::Value) -> Result<(), DownloadError> {
    serde_json::from_value::<LastFmError>(value.clone())
        .unwrap_or_default()
        .check()
}

/// Last.fm appends `<a href="...">Read more on Last.fm</a>` to bios and wikis.
/// Cut at that trailing anchor — not the first `<a` in the body, which may be
/// an ordinary link and would lose the whole article.
fn strip_read_more(text: &str) -> String {
    const MARKER: &str = "Read more on Last.fm";
    let end = text
        .find(MARKER)
        .and_then(|marker| text[..marker].rfind("<a "))
        .or_else(|| text.find(MARKER))
        .unwrap_or(text.len());
    text[..end].trim().to_owned()
}

/// Strip the full text first: a `content` that is only a "Read more" link
/// becomes empty and must fall back to `summary`, not hide the whole bio.
fn pick_bio(content: Option<String>, summary: Option<String>) -> Option<String> {
    let content = content
        .map(|text| strip_read_more(&text))
        .filter(|text| !text.is_empty());
    content.or_else(|| {
        summary
            .map(|text| strip_read_more(&text))
            .filter(|text| !text.is_empty())
    })
}

/// Last.fm can include empty-name rows (e.g. `@attr`-only placeholders); skip
/// them and prefer a row explicitly marked as now playing.
fn first_named_track(page: RecentTracksPage) -> Option<RecentTrack> {
    let mut tracks = page.track.into_iter().filter(|track| {
        track
            .name
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty())
    });
    let first = tracks.next()?;
    if first.is_now_playing() {
        return Some(first);
    }
    tracks.find(|track| track.is_now_playing()).or(Some(first))
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
        // Try the array first: with `One` first, a one-element array whose
        // first field is a map type parses as a struct-from-seq and yields a
        // phantom row, and `[]` yields a single defaulted row.
        Many(Vec<T>),
        One(T),
    }
    Ok(match Option::<OneOrMany<T>>::deserialize(deserializer)? {
        Some(OneOrMany::Many(values)) => values,
        Some(OneOrMany::One(value)) => vec![value],
        None => Vec::new(),
    })
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    #[serde(default)]
    user: Option<UserInfo>,
    #[serde(flatten)]
    error: LastFmError,
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
    #[serde(flatten)]
    error: LastFmError,
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
    #[serde(flatten)]
    error: LastFmError,
}

#[derive(Debug, Deserialize)]
struct ArtistInfoResponse {
    #[serde(default)]
    artist: Option<ArtistBioInfo>,
    #[serde(flatten)]
    error: LastFmError,
}

#[derive(Debug, Deserialize)]
struct ArtistBioInfo {
    #[serde(default)]
    bio: Option<ArtistBio>,
}

#[derive(Debug, Deserialize)]
struct ArtistBio {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AlbumInfoResponse {
    #[serde(default)]
    album: Option<AlbumWikiInfo>,
    #[serde(flatten)]
    error: LastFmError,
}

#[derive(Debug, Deserialize)]
struct AlbumWikiInfo {
    #[serde(default)]
    wiki: Option<AlbumWiki>,
}

#[derive(Debug, Deserialize)]
struct AlbumWiki {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SimilarResponse {
    // Last.fm roots this payload as "similartracks", not "similar" — the
    // wrong name silently parsed every reply as "no similar tracks".
    #[serde(default, rename = "similartracks")]
    similar: Option<SimilarTracks>,
    #[serde(flatten)]
    error: LastFmError,
}

#[derive(Debug, Deserialize)]
struct SimilarTracks {
    #[serde(default, deserialize_with = "one_or_many")]
    track: Vec<SimilarTrack>,
}

#[derive(Debug, Deserialize)]
struct SimilarTrack {
    // Field order no longer matters: one_or_many tries the array arm first,
    // so a one-element array can't be misread as a struct-from-seq (which used
    // to leave `name` empty when a map-typed field came first).
    #[serde(default)]
    name: String,
    #[serde(default)]
    artist: Option<SimilarArtist>,
}

#[derive(Debug, Deserialize)]
struct SimilarArtist {
    #[serde(default)]
    name: String,
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
        tags.sort_by_key(|tag| std::cmp::Reverse(tag.0));
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
    // A one-item page is a bare object (now_playing uses limit=1), not an array.
    #[serde(default, deserialize_with = "one_or_many")]
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
    fn is_now_playing(&self) -> bool {
        self.attr
            .as_ref()
            .and_then(|attr| attr.nowplaying.as_deref())
            .is_some_and(|value| value == "true")
    }

    fn into_now_playing(self) -> NowPlaying {
        let is_now_playing = self.is_now_playing();
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
            is_now_playing,
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
    fn parses_similartracks_root() {
        // Real payload shape of track.getsimilar: the collection sits under
        // a "similartracks" root, not "similar".
        let json = r##"{
            "similartracks": {
                "track": [
                    {"name": "One", "artist": {"name": "Metallica"}, "match": "0.9"}
                ]
            }
        }"##;
        let body: SimilarResponse = serde_json::from_str(json).expect("json");
        let similar = body.similar.expect("similartracks").track;
        assert_eq!(similar.len(), 1);
        assert_eq!(similar[0].name, "One");
        assert_eq!(
            similar[0]
                .artist
                .as_ref()
                .map(|artist| artist.name.as_str()),
            Some("Metallica")
        );
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

    #[test]
    fn top_items_drop_lastfm_placeholder_artwork() {
        // Live shape: every image size points at the grey-star placeholder.
        let json = r##"{
            "toptracks": { "track": [
                {
                    "name": "Sultans of Swing",
                    "artist": { "name": "Dire Straits" },
                    "playcount": "42",
                    "image": [
                        { "#text": "https://lastfm-img.freetls.fastly.net/i/u/34s/2a96cbd8b46e442fc41c2b86b821562f.png", "size": "small" },
                        { "#text": "https://lastfm-img.freetls.fastly.net/i/u/300x300/2a96cbd8b46e442fc41c2b86b821562f.png", "size": "extralarge" }
                    ]
                },
                {
                    "name": "Money for Nothing",
                    "artist": { "name": "Dire Straits" },
                    "playcount": 7,
                    "image": [
                        { "#text": "https://x/small.jpg", "size": "small" },
                        { "#text": "https://x/mega.jpg", "size": "mega" }
                    ]
                }
            ] }
        }"##;
        let response: serde_json::Value = serde_json::from_str(json).expect("json");
        let items = parse_top_items(StatsKind::Tracks, &response);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].image_url, None);
        assert_eq!(items[0].playcount, 42);
        assert_eq!(items[1].image_url.as_deref(), Some("https://x/mega.jpg"));
        assert_eq!(items[1].playcount, 7);
    }

    #[test]
    fn recent_tracks_single_object_is_accepted() {
        // now_playing requests limit=1, so Last.fm sends one bare object.
        let json = r##"{
            "recenttracks": {
                "track": {
                    "name": "Walk",
                    "artist": {"#text": "Pantera"},
                    "@attr": {"nowplaying": "true"}
                }
            }
        }"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        let tracks = body.recent_tracks.expect("page").track;
        assert_eq!(tracks.len(), 1);
        let now = first_named_track(RecentTracksPage { track: tracks })
            .expect("track")
            .into_now_playing();
        assert_eq!(now.meta.title, "Walk");
        assert_eq!(now.meta.performer.as_deref(), Some("Pantera"));
        assert!(now.is_now_playing);
    }

    #[test]
    fn empty_track_array_has_no_phantom_row() {
        // With `One` tried first, `[]` used to yield one defaulted track.
        let json = r##"{"recenttracks":{"track":[]}}"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        assert!(body.recent_tracks.expect("page").track.is_empty());
    }

    #[test]
    fn one_element_track_array_parses_to_one_row() {
        // Reordering the enum must not break the normal single-item array.
        let json = r##"{"recenttracks":{"track":[{"name":"Walk","artist":{"#text":"Pantera"}}]}}"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        let tracks = body.recent_tracks.expect("page").track;
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].name.as_deref(), Some("Walk"));
    }

    #[test]
    fn empty_name_rows_are_skipped() {
        let json = r##"{"recenttracks":{"track":[{"name":""},{"name":"Walk","artist":{"#text":"Pantera"}}]}}"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        let page = body.recent_tracks.expect("page");
        let track = first_named_track(page).expect("named track");
        assert_eq!(track.name.as_deref(), Some("Walk"));
    }

    #[test]
    fn rate_limit_error_is_surfaced() {
        let json = r##"{"error":29,"message":"Rate limit exceeded"}"##;
        let body: SimilarResponse = serde_json::from_str(json).expect("json");
        let error = body.error.check().expect_err("error 29 must fail");
        assert!(error.to_string().contains("Rate limit exceeded"));
        // Value-based path used by top_items.
        let value: serde_json::Value = serde_json::from_str(json).expect("json");
        let error = check_value_error(&value).expect_err("error 29 must fail");
        assert!(error.to_string().contains("Rate limit exceeded"));
    }

    #[test]
    fn strip_read_more_keeps_body_links() {
        let text = "<p>See <a href=\"https://x\">this band</a> for context.</p> \
                    <a href=\"https://www.last.fm/music/X\">Read more on Last.fm</a>";
        let stripped = strip_read_more(text);
        assert!(stripped.contains("this band"), "body link must survive");
        assert!(!stripped.contains("Read more on Last.fm"));
        assert!(
            !stripped.contains("last.fm/music/X"),
            "trailing Read-more anchor must be cut"
        );
        assert_eq!(strip_read_more("plain text").as_str(), "plain text");
    }

    #[test]
    fn profile_error_envelope_is_surfaced() {
        let json = r##"{"error":6,"message":"User not found"}"##;
        let body: UserInfoResponse = serde_json::from_str(json).expect("json");
        let error = body.error.check().expect_err("error must fail");
        assert!(error.to_string().contains("User not found"));
    }

    #[test]
    fn rate_limit_cooldown_blocks_then_expires() {
        let client = LastFmClient::new("key".to_owned()).expect("client");
        client.note_rate_limit(Some(29));
        let error = client.ensure_ready().expect_err("should be cooling down");
        assert!(error.to_string().contains("cooling down"), "{error}");
        // Rewind the timestamp instead of sleeping a full minute.
        *client
            .rate_limited_at
            .lock()
            .unwrap_or_else(|error| error.into_inner()) =
            Some(Instant::now() - RATE_LIMIT_COOLDOWN - Duration::from_secs(1));
        assert!(client.ensure_ready().is_ok());
        client.note_rate_limit(Some(6));
        assert!(client.ensure_ready().is_ok(), "other codes don't cool down");
    }

    #[test]
    fn nowplaying_row_is_preferred_over_first_named() {
        let json = r##"{"recenttracks":{"track":[
            {"name":"Old","artist":{"#text":"A"}},
            {"name":"Live","artist":{"#text":"B"},"@attr":{"nowplaying":"true"}}
        ]}}"##;
        let body: RecentTracksResponse = serde_json::from_str(json).expect("json");
        let track = first_named_track(body.recent_tracks.expect("page")).expect("track");
        assert_eq!(track.name.as_deref(), Some("Live"));
    }

    #[test]
    fn bio_uses_summary_when_content_strips_to_nothing() {
        let only_link =
            Some("<a href=\"https://www.last.fm/music/X\">Read more on Last.fm</a>".to_owned());
        let summary = Some("Real biography text.".to_owned());
        assert_eq!(
            pick_bio(only_link.clone(), summary.clone()).as_deref(),
            Some("Real biography text.")
        );
        assert_eq!(
            pick_bio(Some("Full biography.".to_owned()), summary).as_deref(),
            Some("Full biography.")
        );
    }
}
