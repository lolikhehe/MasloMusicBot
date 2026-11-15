//! Offline input parsing. Nothing here performs a network request, so a cache
//! hit can be answered without contacting a provider.

use common::domain::{AlbumRef, DownloadTarget, Provider, TrackKey, TrackRequest};
use url::Url;

#[derive(Debug, Clone)]
pub enum ParsedInput {
    Track(TrackRequest),
    Album(AlbumRef),
    Search(String),
    Unsupported(String),
}

pub fn parse_input(raw: &str) -> ParsedInput {
    let input = raw.trim();
    if input.is_empty() {
        return ParsedInput::Search(String::new());
    }

    if input.starts_with("spotify:") {
        return parse_spotify_uri(input);
    }

    let Some(url) = http_url(input) else {
        return ParsedInput::Search(input.to_owned());
    };

    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();

    match host.as_str() {
        "youtube.com"
        | "m.youtube.com"
        | "music.youtube.com"
        | "youtu.be"
        | "youtube-nocookie.com" => parse_youtube(&url, &host),
        "soundcloud.com" | "m.soundcloud.com" | "on.soundcloud.com" => {
            parse_soundcloud(&url, &host)
        }
        "open.spotify.com" | "play.spotify.com" => parse_spotify(&url),
        "music.apple.com" | "itunes.apple.com" => parse_apple(&url),
        "deezer.com" | "www.deezer.com" => parse_deezer(&url),
        _ if host.ends_with(".bandcamp.com") => parse_bandcamp(&url, &host),
        _ => ParsedInput::Unsupported(format!("unsupported host: {host}")),
    }
}

/// Resolve a literal cache key or a track URL to the key used by the cache.
pub fn key_from_input(raw: &str) -> TrackKey {
    match parse_input(raw) {
        ParsedInput::Track(track) => track.key,
        _ => TrackKey::new(raw.trim()),
    }
}

fn http_url(raw: &str) -> Option<Url> {
    // Fast path: the whole (trimmed) input is a URL. A URL containing
    // whitespace is not a URL, so treat it as prose and fall through to the
    // embedded-URL scan (Url::parse accepts spaces in query strings).
    if !raw.chars().any(char::is_whitespace)
        && let Ok(url) = Url::parse(raw)
        && matches!(url.scheme(), "http" | "https")
    {
        return Some(url);
    }
    // Otherwise scan for the first embedded http(s) URL — users paste links
    // inside prose ("listen to https://youtu.be/xyz please").
    for marker in ["https://", "http://"] {
        let mut search_from = 0usize;
        while let Some(found) = raw[search_from..].find(marker) {
            let start = search_from + found;
            let candidate = raw[start..]
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_end_matches([
                    '.', ',', ';', ':', '!', '?', ')', ']', '}', '<', '>', '"', '\'', '»', '〉',
                ]);
            if let Ok(url) = Url::parse(candidate) {
                return Some(url);
            }
            search_from = start + marker.len();
        }
    }
    None
}

fn valid_id(value: &str) -> bool {
    (2..=128).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn youtube_key(id: &str) -> TrackKey {
    TrackKey::new(format!("youtube:{id}"))
}

fn parse_youtube(url: &Url, host: &str) -> ParsedInput {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();

    let video_id = if host == "youtu.be" {
        segments.first().map(|value| (*value).to_owned())
    } else if segments.first() == Some(&"watch") {
        query_value(url, "v")
    } else if matches!(
        segments.first().copied(),
        Some("shorts" | "live" | "embed" | "v")
    ) {
        segments.get(1).map(|value| (*value).to_owned())
    } else {
        None
    };

    if let Some(id) = video_id.as_deref().filter(|id| valid_id(id)) {
        let is_music = host == "music.youtube.com";
        let provider = if is_music {
            Provider::YouTubeMusic
        } else {
            Provider::YouTube
        };
        let canonical = if is_music {
            format!("https://music.youtube.com/watch?v={id}")
        } else {
            format!("https://www.youtube.com/watch?v={id}")
        };
        return ParsedInput::Track(TrackRequest {
            key: youtube_key(id),
            provider,
            source_url: Some(canonical.clone()),
            meta: None,
            target: DownloadTarget::Url(canonical),
        });
    }

    if let Some(list) = query_value(url, "list")
        .as_deref()
        .filter(|id| valid_id(id))
    {
        let is_music = host == "music.youtube.com";
        return ParsedInput::Album(AlbumRef {
            provider: if is_music {
                Provider::YouTubeMusic
            } else {
                Provider::YouTube
            },
            url: Some(format!("https://www.youtube.com/playlist?list={list}")),
            title: None,
        });
    }

    ParsedInput::Unsupported("YouTube URL without a video or playlist id".to_owned())
}

fn query_value(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

fn parse_soundcloud(url: &Url, host: &str) -> ParsedInput {
    let path = url.path().trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return ParsedInput::Unsupported("SoundCloud profile URL is not a track".to_owned());
    }
    let key = format!("soundcloud:{host}{path}");
    if path.contains("/sets/") {
        ParsedInput::Album(AlbumRef {
            provider: Provider::SoundCloud,
            url: Some(url.as_str().to_owned()),
            title: None,
        })
    } else if path.split('/').filter(|part| !part.is_empty()).count() == 1 {
        // A bare profile is not directly downloadable.
        ParsedInput::Unsupported("SoundCloud profile URL is not a track".to_owned())
    } else {
        ParsedInput::Track(TrackRequest {
            key: TrackKey::new(key),
            provider: Provider::SoundCloud,
            source_url: Some(url.as_str().to_owned()),
            meta: None,
            target: DownloadTarget::Url(url.as_str().to_owned()),
        })
    }
}

fn parse_bandcamp(url: &Url, host: &str) -> ParsedInput {
    let path = url.path().trim_end_matches('/');
    let key = format!("bandcamp:{host}{path}");
    if path.starts_with("/track/") {
        ParsedInput::Track(TrackRequest {
            key: TrackKey::new(key),
            provider: Provider::Bandcamp,
            source_url: Some(url.as_str().to_owned()),
            meta: None,
            target: DownloadTarget::Url(url.as_str().to_owned()),
        })
    } else if path.starts_with("/album/") {
        ParsedInput::Album(AlbumRef {
            provider: Provider::Bandcamp,
            url: Some(url.as_str().to_owned()),
            title: None,
        })
    } else {
        ParsedInput::Unsupported("Bandcamp URL is not a track or album".to_owned())
    }
}

fn parse_spotify_uri(input: &str) -> ParsedInput {
    let parts: Vec<&str> = input.split(':').collect();
    match parts.as_slice() {
        ["spotify", "track", id] if valid_id(id) => external_track(Provider::Spotify, id),
        ["spotify", "album" | "playlist", id] if valid_id(id) => ParsedInput::Album(AlbumRef {
            provider: Provider::Spotify,
            url: Some(format!("https://open.spotify.com/{}/{}", parts[1], id)),
            title: None,
        }),
        _ => ParsedInput::Unsupported("invalid Spotify URI".to_owned()),
    }
}

fn parse_spotify(url: &Url) -> ParsedInput {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    match segments.as_slice() {
        ["track", id] if valid_id(id) => external_track(Provider::Spotify, id),
        ["album" | "playlist", id] if valid_id(id) => ParsedInput::Album(AlbumRef {
            provider: Provider::Spotify,
            url: Some(format!("https://open.spotify.com/{}/{}", segments[0], id)),
            title: None,
        }),
        _ => ParsedInput::Unsupported("Spotify URL is not a track or album".to_owned()),
    }
}

fn external_track(provider: Provider, id: &str) -> ParsedInput {
    let source_url = match provider {
        Provider::Spotify => format!("https://open.spotify.com/track/{id}"),
        Provider::AppleMusic => format!("https://music.apple.com/song/{id}"),
        Provider::Deezer => format!("https://www.deezer.com/track/{id}"),
        _ => String::new(),
    };
    ParsedInput::Track(TrackRequest {
        key: TrackKey::new(format!("{}:track:{id}", provider.as_str())),
        provider,
        source_url: Some(source_url),
        meta: None,
        target: DownloadTarget::Search(String::new()),
    })
}

fn parse_apple(url: &Url) -> ParsedInput {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();

    if let Some(track_id) = query_value(url, "i").filter(|id| !id.is_empty()) {
        return external_track(Provider::AppleMusic, &track_id);
    }
    if let Some(id) = segments
        .iter()
        .position(|part| *part == "song")
        .and_then(|index| segments.get(index + 1))
        .filter(|id| id.chars().all(|c| c.is_ascii_digit()))
    {
        return external_track(Provider::AppleMusic, id);
    }
    if segments
        .iter()
        .any(|part| matches!(*part, "album" | "playlist"))
        && segments
            .iter()
            .rev()
            .any(|id| id.chars().all(|c| c.is_ascii_digit()))
    {
        return ParsedInput::Album(AlbumRef {
            provider: Provider::AppleMusic,
            url: Some(url.as_str().to_owned()),
            title: None,
        });
    }
    ParsedInput::Unsupported("Apple Music URL is not a track or album".to_owned())
}

fn parse_deezer(url: &Url) -> ParsedInput {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    let Some(kind) = segments
        .iter()
        .position(|part| matches!(*part, "track" | "album" | "playlist"))
    else {
        return ParsedInput::Unsupported("Deezer URL is not a track or album".to_owned());
    };
    let Some(id) = segments.get(kind + 1).copied() else {
        return ParsedInput::Unsupported("Deezer URL has no id".to_owned());
    };
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return ParsedInput::Unsupported("invalid Deezer URL".to_owned());
    }
    if segments[kind] == "track" {
        external_track(Provider::Deezer, id)
    } else {
        ParsedInput::Album(AlbumRef {
            provider: Provider::Deezer,
            url: Some(format!("https://www.deezer.com/{}/{}", segments[kind], id)),
            title: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_embedded_urls_from_prose() {
        for raw in [
            "listen to https://youtu.be/dQw4w9WgXcQ please",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ now)",
            "beat: https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC (good)",
        ] {
            match parse_input(raw) {
                ParsedInput::Track(_) => {}
                other => panic!("expected track for {raw:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parses_youtube_variants_to_one_key() {
        for raw in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ&t=10",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
        ] {
            match parse_input(raw) {
                ParsedInput::Track(track) => {
                    assert_eq!(track.key, TrackKey::new("youtube:dQw4w9WgXcQ"))
                }
                other => panic!("expected track, got {other:?}"),
            }
        }
    }

    #[test]
    fn parses_youtube_playlist_as_album() {
        match parse_input("https://www.youtube.com/playlist?list=PL123abc") {
            ParsedInput::Album(album) => assert_eq!(album.provider, Provider::YouTube),
            other => panic!("expected album, got {other:?}"),
        }
    }

    #[test]
    fn parses_soundcloud_set_and_track() {
        assert!(matches!(
            parse_input("https://soundcloud.com/artist/sets/album"),
            ParsedInput::Album(_)
        ));
        assert!(matches!(
            parse_input("https://soundcloud.com/artist/song"),
            ParsedInput::Track(_)
        ));
    }

    #[test]
    fn parses_external_tracks() {
        for (raw, provider) in [
            ("https://open.spotify.com/track/abc123", Provider::Spotify),
            (
                "https://music.apple.com/us/album/name/123?i=456",
                Provider::AppleMusic,
            ),
            ("https://www.deezer.com/track/789", Provider::Deezer),
        ] {
            match parse_input(raw) {
                ParsedInput::Track(track) => assert_eq!(track.provider, provider),
                other => panic!("expected track, got {other:?}"),
            }
        }
    }

    #[test]
    fn plain_text_is_search() {
        assert!(matches!(
            parse_input("Daft Punk — One More Time"),
            ParsedInput::Search(_)
        ));
    }

    #[test]
    fn javascript_scheme_is_search_not_a_url() {
        assert!(matches!(
            parse_input("javascript:alert(1)"),
            ParsedInput::Search(_)
        ));
    }
}
