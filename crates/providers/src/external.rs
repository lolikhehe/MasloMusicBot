//! Metadata-only integrations for Spotify, Apple Music and Deezer.
//!
//! These APIs never provide audio. We only read public metadata, then look the
//! same track up on an open source (usually YouTube via `ytsearch1`). Nothing
//! here attempts to bypass DRM or paid access.

use common::domain::{
    Album, AlbumRef, DownloadTarget, Provider, SearchResult, TrackKey, TrackMeta, TrackRequest,
};
use common::error::DownloadError;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

#[derive(Clone)]
pub struct ExternalResolver {
    client: reqwest::Client,
}

impl ExternalResolver {
    pub fn new(timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("music-bot/0.1 (+https://github.com/)")
            .build()?;
        Ok(Self { client })
    }

    /// Fill metadata for an external track and turn its target into a search.
    pub async fn resolve_track(&self, request: &mut TrackRequest) -> Result<(), DownloadError> {
        if !request.provider.is_external() {
            return Ok(());
        }
        let id = external_id(request)?;
        let meta = match request.provider {
            Provider::Spotify => {
                self.spotify_track(request.source_url.as_deref(), id)
                    .await?
            }
            Provider::AppleMusic => self.apple_track(id).await?,
            Provider::Deezer => self.deezer_track(id).await?,
            _ => unreachable!(),
        };
        request.target = DownloadTarget::Search(meta.search_query());
        request.meta = Some(meta);
        Ok(())
    }

    pub async fn resolve_album(
        &self,
        album: &AlbumRef,
        limit: usize,
    ) -> Result<Album, DownloadError> {
        match album.provider {
            Provider::Deezer => self.deezer_album(album, limit).await,
            Provider::AppleMusic => self.apple_album(album, limit).await,
            Provider::Spotify => Err(DownloadError::Unsupported(
                "Spotify albums and playlists need an authenticated API and are not supported; send individual track links or search by name".to_owned(),
            )),
            _ => Err(DownloadError::Unsupported(
                "this provider does not expose albums through metadata APIs".to_owned(),
            )),
        }
    }

    async fn spotify_track(
        &self,
        source_url: Option<&str>,
        id: &str,
    ) -> Result<TrackMeta, DownloadError> {
        let track_url = source_url
            .map(str::to_owned)
            .unwrap_or_else(|| format!("https://open.spotify.com/track/{id}"));
        let response = self
            .client
            .get("https://open.spotify.com/oembed")
            .query(&[("url", track_url.as_str())])
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(DownloadError::NotFound);
        }
        if !status.is_success() {
            return Err(DownloadError::SourceUnavailable(format!(
                "Spotify oEmbed returned {status}"
            )));
        }
        let body: SpotifyOEmbed = response.json().await.map_err(map_reqwest)?;
        let title = clean_title(&body.title);
        if title.is_empty() {
            return Err(DownloadError::NotFound);
        }
        Ok(TrackMeta {
            title,
            performer: body.author_name.filter(|value| !value.trim().is_empty()),
            album: None,
            duration_secs: None,
            thumbnail_url: body.thumbnail_url,
            genres: Vec::new(),
        })
    }

    async fn apple_track(&self, id: &str) -> Result<TrackMeta, DownloadError> {
        let body: ItunesResponse = self
            .client
            .get("https://itunes.apple.com/lookup")
            .query(&[("id", id), ("entity", "song"), ("country", "us")])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;
        let Some(track) = body.results.into_iter().next() else {
            return Err(DownloadError::NotFound);
        };
        let Some(title) = track.track_name else {
            return Err(DownloadError::NotFound);
        };
        Ok(TrackMeta {
            title,
            performer: track.artist_name,
            album: track.collection_name,
            duration_secs: track
                .track_time_millis
                .and_then(|millis| u32::try_from(millis / 1000).ok()),
            thumbnail_url: track.artwork_url100.map(upgrade_artwork),
            genres: Vec::new(),
        })
    }

    async fn deezer_track(&self, id: &str) -> Result<TrackMeta, DownloadError> {
        let track: DeezerTrack = self
            .client
            .get(format!("https://api.deezer.com/track/{id}"))
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;
        if track.title.trim().is_empty() {
            return Err(DownloadError::NotFound);
        }
        let performer = track.artist.as_ref().map(|artist| artist.name.clone());
        let album = track.album.as_ref().and_then(|album| album.title.clone());
        let thumbnail_url = track
            .album
            .as_ref()
            .and_then(|album| album.cover_xl.clone().or_else(|| album.cover_big.clone()))
            .or(track.cover_xl.clone());
        Ok(TrackMeta {
            title: track.title,
            performer,
            album,
            duration_secs: track.duration,
            thumbnail_url,
            genres: Vec::new(),
        })
    }

    /// Public Deezer search API (no auth). Results carry enough metadata to
    /// display inline; the audio itself is still matched on YouTube via the
    /// search query, exactly like single Spotify/Deezer track links.
    pub async fn deezer_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, DownloadError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 20).to_string();
        let page: DeezerTrackList = self
            .client
            .get("https://api.deezer.com/search")
            .query(&[("q", query), ("limit", limit.as_str())])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;
        Ok(deezer_results_from_page(page))
    }

    /// Public iTunes Search API (no auth), used only when the user opted into
    /// album/genre tags and the source had no genre of its own. Picks a song
    /// within ±15 s of the candidate duration when duration is known; otherwise
    /// takes the first track with a genre. Returns (genre, album).
    pub async fn itunes_tags(
        &self,
        query: &str,
        duration_secs: Option<u32>,
    ) -> Result<Option<(String, Option<String>)>, DownloadError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(None);
        }
        let body: ItunesResponse = self
            .client
            .get("https://itunes.apple.com/search")
            .query(&[
                ("term", query),
                ("entity", "song"),
                ("country", "US"),
                ("limit", "3"),
            ])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;
        for track in body.results {
            if track.wrapper_type.as_deref() != Some("track") {
                continue;
            }
            let Some(genre) = track
                .primary_genre_name
                .as_deref()
                .map(str::trim)
                .filter(|genre| !genre.is_empty())
            else {
                continue;
            };
            match duration_secs {
                None => return Ok(Some((genre.to_owned(), track.collection_name))),
                Some(wanted) => {
                    let distance = track
                        .track_time_millis
                        .map(|ms| (ms / 1000).abs_diff(u64::from(wanted)))
                        .unwrap_or(u64::MAX);
                    if distance <= 15 {
                        return Ok(Some((genre.to_owned(), track.collection_name)));
                    }
                }
            }
        }
        Ok(None)
    }

    async fn deezer_album(&self, album: &AlbumRef, limit: usize) -> Result<Album, DownloadError> {
        let id = album_id_from_url(album.url.as_deref(), "deezer.com", &["album", "playlist"])
            .ok_or_else(|| DownloadError::Unsupported("invalid Deezer album URL".to_owned()))?;
        let endpoint = if album
            .url
            .as_deref()
            .is_some_and(|url| url.contains("/playlist/"))
        {
            "playlist"
        } else {
            "album"
        };
        let body: DeezerAlbum = self
            .client
            .get(format!("https://api.deezer.com/{endpoint}/{id}"))
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;

        let cover = body.cover_xl.or(body.cover_big);
        let album_name = body.title.clone();
        let mut tracks = Vec::new();
        for track in body.tracks.data.into_iter().take(limit) {
            let meta = TrackMeta {
                title: track.title,
                performer: track.artist.as_ref().map(|artist| artist.name.clone()),
                album: Some(album_name.clone()),
                duration_secs: track.duration,
                thumbnail_url: cover.clone(),
                genres: Vec::new(),
            };
            let track_id = track.id.to_string();
            tracks.push(external_request(Provider::Deezer, &track_id, meta));
        }
        if tracks.is_empty() {
            return Err(DownloadError::NotFound);
        }
        Ok(Album {
            title: album_name,
            provider: Provider::Deezer,
            tracks,
            source_url: album.url.clone(),
        })
    }

    async fn apple_album(&self, album: &AlbumRef, limit: usize) -> Result<Album, DownloadError> {
        let id = album_id_from_url(album.url.as_deref(), "music.apple.com", &["album"])
            .ok_or_else(|| {
                DownloadError::Unsupported("invalid Apple Music album URL".to_owned())
            })?;
        let body: ItunesResponse = self
            .client
            .get("https://itunes.apple.com/lookup")
            .query(&[
                ("id", id.as_str()),
                ("entity", "song"),
                ("country", "us"),
                ("limit", "200"),
            ])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .json()
            .await
            .map_err(map_reqwest)?;
        let mut title = album
            .title
            .clone()
            .unwrap_or_else(|| "Apple Music album".to_owned());
        let mut tracks = Vec::new();
        for track in body
            .results
            .into_iter()
            .filter(|item| item.wrapper_type.as_deref() == Some("track"))
        {
            let (Some(track_name), Some(track_id)) = (track.track_name, track.track_id) else {
                continue;
            };
            if let Some(collection) = track.collection_name.as_ref() {
                title = collection.clone();
            }
            let meta = TrackMeta {
                title: track_name,
                performer: track.artist_name,
                album: track.collection_name,
                duration_secs: track
                    .track_time_millis
                    .and_then(|millis| u32::try_from(millis / 1000).ok()),
                thumbnail_url: track.artwork_url100.map(upgrade_artwork),
                genres: Vec::new(),
            };
            tracks.push(external_request(
                Provider::AppleMusic,
                &track_id.to_string(),
                meta,
            ));
            if tracks.len() >= limit {
                break;
            }
        }
        if tracks.is_empty() {
            return Err(DownloadError::NotFound);
        }
        Ok(Album {
            title,
            provider: Provider::AppleMusic,
            tracks,
            source_url: album.url.clone(),
        })
    }
}

fn external_request(provider: Provider, id: &str, meta: TrackMeta) -> TrackRequest {
    let source_url = match provider {
        Provider::Spotify => format!("https://open.spotify.com/track/{id}"),
        Provider::AppleMusic => format!("https://music.apple.com/song/{id}"),
        Provider::Deezer => format!("https://www.deezer.com/track/{id}"),
        _ => String::new(),
    };
    let target = DownloadTarget::Search(meta.search_query());
    TrackRequest {
        key: TrackKey::new(format!("{}:track:{id}", provider.as_str())),
        provider,
        source_url: Some(source_url),
        meta: Some(meta),
        target,
    }
}

fn external_id(request: &TrackRequest) -> Result<&str, DownloadError> {
    let prefix = format!("{}:track:", request.provider.as_str());
    request
        .key
        .as_str()
        .strip_prefix(&prefix)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| DownloadError::Internal("cannot determine external track id".to_owned()))
}

fn clean_title(title: &str) -> String {
    let title = title.trim();
    // Spotify oEmbed sometimes returns "Song - song by Artist".
    title
        .split(" - song by ")
        .next()
        .unwrap_or(title)
        .trim()
        .to_owned()
}

fn upgrade_artwork(url: String) -> String {
    url.replace("100x100bb", "600x600bb")
}

fn album_id_from_url(url: Option<&str>, host: &str, kinds: &[&str]) -> Option<String> {
    let url = Url::parse(url?).ok()?;
    if !url.host_str()?.contains(host) {
        return None;
    }
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let index = segments.iter().position(|part| kinds.contains(part))?;
    segments
        .iter()
        .skip(index + 1)
        .rev()
        .find(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        .map(|id| (*id).to_owned())
}

fn deezer_results_from_page(page: DeezerTrackList) -> Vec<SearchResult> {
    let mut results = Vec::with_capacity(page.data.len());
    for track in page.data {
        let title = track.title.trim().to_owned();
        if title.is_empty() {
            continue;
        }
        let performer = track.artist.as_ref().map(|artist| artist.name.clone());
        let album = track.album.as_ref().and_then(|album| album.title.clone());
        let thumbnail_url = track
            .album
            .as_ref()
            .and_then(|album| album.cover_xl.clone().or_else(|| album.cover_big.clone()))
            .or_else(|| track.cover_xl.clone());
        let meta = TrackMeta {
            title,
            performer,
            album,
            duration_secs: track.duration,
            thumbnail_url,
            genres: Vec::new(),
        };
        let id = track.id.to_string();
        let source_url = format!("https://www.deezer.com/track/{id}");
        let target = DownloadTarget::Search(meta.search_query());
        results.push(SearchResult {
            key: TrackKey::new(format!("deezer:track:{id}")),
            provider: Provider::Deezer,
            source_url,
            meta,
            target,
        });
    }
    results
}

fn map_reqwest(error: reqwest::Error) -> DownloadError {
    if error.status() == Some(reqwest::StatusCode::NOT_FOUND) {
        DownloadError::NotFound
    } else {
        DownloadError::SourceUnavailable(format!("metadata request failed: {error}"))
    }
}

#[derive(Debug, Deserialize)]
struct SpotifyOEmbed {
    title: String,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ItunesResponse {
    #[serde(default)]
    results: Vec<ItunesTrack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesTrack {
    #[serde(default)]
    wrapper_type: Option<String>,
    #[serde(default)]
    track_id: Option<u64>,
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    collection_name: Option<String>,
    #[serde(default)]
    track_time_millis: Option<u64>,
    #[serde(default)]
    artwork_url100: Option<String>,
    #[serde(default)]
    primary_genre_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeezerArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct DeezerAlbumInfo {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cover_xl: Option<String>,
    #[serde(default)]
    cover_big: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeezerTrack {
    id: u64,
    title: String,
    #[serde(default)]
    duration: Option<u32>,
    #[serde(default)]
    artist: Option<DeezerArtist>,
    #[serde(default)]
    album: Option<DeezerAlbumInfo>,
    #[serde(default)]
    cover_xl: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DeezerTrackList {
    #[serde(default)]
    data: Vec<DeezerTrack>,
}

#[derive(Debug, Deserialize)]
struct DeezerAlbum {
    title: String,
    #[serde(default)]
    cover_xl: Option<String>,
    #[serde(default)]
    cover_big: Option<String>,
    #[serde(default)]
    tracks: DeezerTrackList,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_deezer_search_page() {
        // Shape captured from GET https://api.deezer.com/search?q=...&limit=2
        let json = r#"{
            "data": [
                {
                    "id": 1900001,
                    "title": "Metallica",
                    "link": "https://www.deezer.com/track/1900001",
                    "duration": 331,
                    "artist": { "id": 119, "name": "Metallica" },
                    "album": { "id": 256250622, "title": "Master Of Puppets",
                               "cover_medium": "https://x/250.jpg",
                               "cover_big": "https://x/500.jpg",
                               "cover_xl": "https://x/1000.jpg" }
                }
            ],
            "total": 1
        }"#;
        let page: DeezerTrackList = serde_json::from_str(json).expect("page");
        let results = deezer_results_from_page(page);
        assert_eq!(results.len(), 1);
        let track = &results[0];
        assert_eq!(track.key, TrackKey::new("deezer:track:1900001"));
        assert_eq!(track.provider, Provider::Deezer);
        assert_eq!(track.meta.performer.as_deref(), Some("Metallica"));
        assert_eq!(track.meta.title, "Metallica");
        assert_eq!(track.meta.duration_secs, Some(331));
        assert_eq!(track.meta.album.as_deref(), Some("Master Of Puppets"));
        assert_eq!(
            track.meta.thumbnail_url.as_deref(),
            Some("https://x/1000.jpg")
        );
        assert_eq!(
            track.target,
            DownloadTarget::Search("Metallica Metallica".to_owned())
        );
        assert_eq!(track.source_url, "https://www.deezer.com/track/1900001");
    }

    #[test]
    fn skips_empty_deezer_titles() {
        let json = r#"{ "data": [ { "id": 1, "title": "   " } ] }"#;
        let page: DeezerTrackList = serde_json::from_str(json).expect("page");
        assert!(deezer_results_from_page(page).is_empty());
    }

    #[test]
    fn strips_spotify_suffix() {
        assert_eq!(clean_title("Song - song by Artist"), "Song");
        assert_eq!(clean_title("Plain"), "Plain");
    }

    #[test]
    fn upgrades_artwork() {
        assert_eq!(
            upgrade_artwork("https://x/100x100bb.jpg".into()),
            "https://x/600x600bb.jpg"
        );
    }

    #[test]
    fn parses_album_id() {
        assert_eq!(
            album_id_from_url(
                Some("https://www.deezer.com/en/album/12345"),
                "deezer.com",
                &["album"]
            ),
            Some("12345".into())
        );
    }
}
