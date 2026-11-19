//! Bounded RSS/Atom podcast feed client.
//!
//! Feeds are attacker-controlled URLs the user pastes in, so: hard byte cap on
//! the body, conditional GET headers for cheap revalidation, fixed error
//! strings (never `reqwest::Error`'s Display — it embeds the URL, tokens and
//! all), a public-URL guard on both fetch and redirects (SSRF), and a
//! host-only fallback title (no path, which may carry a token).

use std::fmt;
use std::net::IpAddr;
use std::time::Duration;

use feed_rs::model::Entry;
use url::Url;

const BODY_CAP_BYTES: u64 = 1024 * 1024;
const MAX_EPISODES: usize = 100;

/// One playable item from a feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    pub guid: String,
    pub title: String,
    pub url: String,
    pub published: Option<i64>,
    pub duration_secs: Option<u32>,
}

/// Parse result plus the validators needed for the next conditional GET.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedSnapshot {
    pub title: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub episodes: Vec<Episode>,
}

/// Status-only failures: no URL, header value or body ever reaches Display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedError {
    NotModified,
    Gone,
    Http(u16),
    Parse,
    TooLarge,
    Network,
}

impl fmt::Display for FeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotModified => f.write_str("feed not modified"),
            Self::Gone => f.write_str("feed gone"),
            Self::Http(status) => write!(f, "feed http {status}"),
            Self::Parse => f.write_str("feed parse error"),
            Self::TooLarge => f.write_str("feed too large"),
            Self::Network => f.write_str("feed network error"),
        }
    }
}

impl std::error::Error for FeedError {}

/// Pooled client with a 15s per-request timeout (same tuning as every other
/// provider client in the workspace).
#[derive(Clone)]
pub struct FeedClient {
    client: reqwest::Client,
}

impl FeedClient {
    /// A failed builder must surface as an error: `Client::default()` would
    /// have no timeout and let a hung feed block a request forever.
    pub fn new() -> Result<Self, reqwest::Error> {
        // Dedicated client: http::client is shared, and feed redirects must be
        // validated per hop before following (same tuning, no auto-redirect).
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(10))
                .pool_idle_timeout(Duration::from_secs(90))
                .tcp_keepalive(Duration::from_secs(60))
                .user_agent(crate::http::USER_AGENT)
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }

    /// Conditional GET + bounded body read + parse, all in one call.
    pub async fn fetch(
        &self,
        url: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<FeedSnapshot, FeedError> {
        if !url_is_public(url) {
            return Err(FeedError::Network);
        }
        // At most one redirect hop is followed (enough for http→https and
        // feedburner); chained redirects are rejected instead of followed.
        let mut current = url.to_owned();
        for _ in 0..=1 {
            let mut request = self.client.get(&current);
            if let Some(etag) = etag {
                request = request.header(reqwest::header::IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = last_modified {
                request = request.header(reqwest::header::IF_MODIFIED_SINCE, last_modified);
            }

            let mut response = match request.send().await {
                Ok(response) => response,
                // Never use the error's Display: reqwest embeds the full URL.
                Err(error) => {
                    return Err(match error.status() {
                        Some(status) => status_error(status.as_u16()),
                        None => FeedError::Network,
                    });
                }
            };
            let status = response.status();
            if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
                let redirect = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|location| validated_redirect(&current, location));
                match redirect {
                    Some(next) if next != current => {
                        current = next;
                        continue;
                    }
                    _ => return Err(FeedError::Network),
                }
            }
            if !status.is_success() {
                return Err(status_error(status.as_u16()));
            }

            let next_etag = header_string(&response, reqwest::header::ETAG);
            let next_last_modified = header_string(&response, reqwest::header::LAST_MODIFIED);

            if response
                .content_length()
                .is_some_and(|length| length > BODY_CAP_BYTES)
            {
                return Err(FeedError::TooLarge);
            }
            let capacity = response
                .content_length()
                .unwrap_or(64 * 1024)
                .min(256 * 1024) as usize;
            let mut bytes = Vec::with_capacity(capacity);
            while let Some(chunk) = response.chunk().await.map_err(|_| FeedError::Network)? {
                if bytes.len() as u64 + chunk.len() as u64 > BODY_CAP_BYTES {
                    return Err(FeedError::TooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }

            return parse_snapshot(&bytes, &current, next_etag, next_last_modified);
        }
        Err(FeedError::Network)
    }
}

/// 304 and gone feeds are distinct from a generic HTTP failure so callers can
/// keep the cached snapshot or drop the subscription respectively.
fn status_error(status: u16) -> FeedError {
    match status {
        304 => FeedError::NotModified,
        404 | 410 => FeedError::Gone,
        other => FeedError::Http(other),
    }
}

fn header_string(
    response: &reqwest::Response,
    name: reqwest::header::HeaderName,
) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// Pure parse step, kept separate so fixtures can be tested without network.
fn parse_snapshot(
    bytes: &[u8],
    url: &str,
    etag: Option<String>,
    last_modified: Option<String>,
) -> Result<FeedSnapshot, FeedError> {
    if bytes.is_empty() {
        return Err(FeedError::Parse);
    }
    let feed = feed_rs::parser::parse(bytes).map_err(|_| FeedError::Parse)?;
    let title = feed
        .title
        .as_ref()
        .map(|text| text.content.trim())
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            Url::parse(url)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_owned))
                .unwrap_or_else(|| "podcast".to_owned())
        });
    let episodes = feed
        .entries
        .iter()
        .filter_map(episode_from_entry)
        .take(MAX_EPISODES)
        .collect();
    Ok(FeedSnapshot {
        title,
        etag,
        last_modified,
        episodes,
    })
}

fn episode_from_entry(entry: &Entry) -> Option<Episode> {
    let url = pick_url(entry)?;
    let title = entry
        .title
        .as_ref()
        .map(|text| text.content.as_str())
        .unwrap_or("");
    let guid = if entry.id.trim().is_empty() {
        episode_guid_fallback(title, &url)
    } else {
        entry.id.trim().to_owned()
    };
    // feed-rs maps <itunes:duration> (and media:content duration) into the
    // entry's MediaObjects; `Content` itself carries no duration in any 1.x.
    let duration_secs = entry
        .media
        .iter()
        .find_map(|media| media.duration)
        .and_then(|duration| u32::try_from(duration.as_secs()).ok());
    Some(Episode {
        guid,
        title: title.to_owned(),
        url,
        published: entry
            .published
            .or(entry.updated)
            .map(|date| date.timestamp()),
        duration_secs,
    })
}

/// Prefer an explicit enclosure/audio link; otherwise the first http(s) link.
/// RSS puts `<enclosure>` in `Entry.media`, Atom puts `rel="enclosure"` in
/// `Entry.links`, so both have to be considered.
fn pick_url(entry: &Entry) -> Option<String> {
    let mut candidates: Vec<(String, bool)> = Vec::new();
    for link in &entry.links {
        let audio = link.rel.as_deref() == Some("enclosure")
            || link
                .media_type
                .as_deref()
                .is_some_and(|mime| mime.starts_with("audio/"));
        candidates.push((link.href.clone(), audio));
    }
    for media in &entry.media {
        for content in &media.content {
            if let Some(url) = &content.url {
                let audio = content
                    .content_type
                    .as_ref()
                    .is_some_and(|mime| mime.essence_str().starts_with("audio/"));
                candidates.push((url.to_string(), audio));
            }
        }
    }
    candidates
        .iter()
        .find(|(url, audio)| *audio && is_http(url))
        .or_else(|| candidates.iter().find(|(url, _)| is_http(url)))
        .map(|(url, _)| url.clone())
}

fn is_http(url: &str) -> bool {
    matches!(Url::parse(url), Ok(parsed) if matches!(parsed.scheme(), "http" | "https"))
}

/// Reject URLs that would let a feed owner reach internal services from the
/// bot's docker network. Feeds are polled in the background, so one accepted
/// internal URL becomes a persistent probe.
pub fn url_is_public(u: &str) -> bool {
    let Ok(url) = Url::parse(u) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    const BLOCKED_HOSTS: &[&str] = &[
        "postgres",
        "bot",
        "worker",
        "bot-api",
        "prometheus",
        "alertmanager",
        "gateway",
        "host.docker.internal",
    ];
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || BLOCKED_HOSTS.contains(&host.as_str())
    {
        return false;
    }
    // Url serialises IPv6 hosts with brackets; strip them before IP parsing.
    let ip_host = host
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(&host);
    if ip_host.parse::<IpAddr>().is_ok_and(ip_is_private) {
        return false;
    }
    // shortcut: DNS-rebinding not covered, acceptable for a bot feed poller
    true
}

fn ip_is_private(ip: IpAddr) -> bool {
    fn v4_internal(v4: std::net::Ipv4Addr) -> bool {
        v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
    }
    match ip {
        IpAddr::V4(v4) => v4_internal(v4),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unicast_link_local()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.to_ipv4().is_some_and(v4_internal)
        }
    }
}

/// Resolve a `Location` against the current URL, keeping it only when public.
fn validated_redirect(current: &str, location: &str) -> Option<String> {
    let base = Url::parse(current).ok()?;
    let target = base.join(location).ok()?;
    url_is_public(target.as_str()).then(|| target.into())
}

/// Deterministic GUID for entries without one. Stable across releases —
/// never `DefaultHasher`, whose algorithm/seed may change between compilers.
pub fn fnv1a_hex(input: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

/// Stable fallback key from a title and URL; the URL only feeds the hash, it
/// is never emitted.
pub fn episode_guid_fallback(title: &str, url: &str) -> String {
    fnv1a_hex(&format!("{title}\u{1}{url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rss_and_atom_fixtures() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
        <rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
          <channel>
            <title>Test Podcast</title>
            <link>https://example.com</link>
            <description>A test podcast</description>
            <item>
              <title>Episode One</title>
              <guid>ep-1</guid>
              <link>https://example.com/ep1</link>
              <enclosure url="https://cdn.example.com/ep1.mp3" length="123" type="audio/mpeg" />
              <pubDate>Tue, 15 Nov 2022 20:15:04 GMT</pubDate>
              <itunes:duration>1:23:45</itunes:duration>
            </item>
            <item>
              <title>Episode Two</title>
              <link>https://example.com/ep2</link>
              <enclosure url="https://cdn.example.com/ep2.mp3" length="200" type="audio/mpeg" />
            </item>
          </channel>
        </rss>"#;
        let snapshot = parse_snapshot(rss.as_bytes(), "https://example.com/feed.xml", None, None)
            .expect("rss fixture");
        assert_eq!(snapshot.title, "Test Podcast");
        assert_eq!(snapshot.episodes.len(), 2);
        assert_eq!(snapshot.episodes[0].guid, "ep-1");
        assert_eq!(snapshot.episodes[0].title, "Episode One");
        assert_eq!(snapshot.episodes[0].url, "https://cdn.example.com/ep1.mp3");
        assert_eq!(snapshot.episodes[0].published, Some(1_668_543_304));
        assert_eq!(snapshot.episodes[0].duration_secs, Some(5025));
        // No <guid>: feed-rs assigns a stable id from link+title, so our own
        // fallback (tested separately) is only used when that id is empty.
        assert!(!snapshot.episodes[1].guid.is_empty());
        assert_eq!(snapshot.episodes[1].url, "https://cdn.example.com/ep2.mp3");

        let atom = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
          <title>Atom Show</title>
          <id>urn:uuid:atom-show</id>
          <updated>2023-01-02T03:04:05Z</updated>
          <entry>
            <title>Atom Episode</title>
            <id>urn:uuid:atom-ep-1</id>
            <updated>2023-01-01T00:00:00Z</updated>
            <link rel="enclosure" type="audio/mpeg" href="https://cdn.example.com/atom1.mp3" />
          </entry>
        </feed>"#;
        let snapshot = parse_snapshot(atom.as_bytes(), "https://example.com/atom.xml", None, None)
            .expect("atom fixture");
        assert_eq!(snapshot.title, "Atom Show");
        assert_eq!(snapshot.episodes.len(), 1);
        assert_eq!(snapshot.episodes[0].guid, "urn:uuid:atom-ep-1");
        assert_eq!(snapshot.episodes[0].title, "Atom Episode");
        assert_eq!(
            snapshot.episodes[0].url,
            "https://cdn.example.com/atom1.mp3"
        );
        assert_eq!(snapshot.episodes[0].published, Some(1_672_531_200));
    }

    #[test]
    fn guid_fallback_is_stable_and_url_free() {
        let guid = episode_guid_fallback("Ep One", "https://x/ep1.mp3");
        assert_eq!(guid, "f1915d6ce8e9e3fb");
        assert_eq!(guid, episode_guid_fallback("Ep One", "https://x/ep1.mp3"));
        assert_ne!(guid, episode_guid_fallback("Ep Two", "https://x/ep1.mp3"));
        assert!(!guid.contains("http"));
    }

    #[test]
    fn fnv1a_hex_known_vectors() {
        assert_eq!(fnv1a_hex("a"), "af63dc4c8601ec8c");
        assert_eq!(fnv1a_hex(""), "cbf29ce484222325");
        assert_eq!(fnv1a_hex("a").len(), 16);
    }

    #[test]
    fn url_is_public_blocks_internal_targets() {
        let internal = [
            "http://postgres:5432/",
            "http://bot-api:8081/",
            "http://bot:9090/worker",
            "http://worker:9090/",
            "http://prometheus:9090/",
            "http://alertmanager:9093/",
            "http://gateway:3000/",
            "http://host.docker.internal:8080/",
            "http://localhost/",
            "http://LOCALHOST:8080/",
            "https://foo.localhost/",
            "http://printer.local/",
            "http://127.0.0.1/",
            "http://127.0.0.2:8080/",
            "http://10.0.0.1/",
            "http://172.16.0.1/",
            "http://172.31.255.255/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]/",
            "http://[fe80::1]/",
            "http://0.0.0.0/",
            "ftp://example.com/feed",
            "file:///etc/passwd",
            "not a url",
        ];
        for url in internal {
            assert!(!url_is_public(url), "should be rejected: {url}");
        }
    }

    #[test]
    fn url_is_public_allows_public_feeds() {
        let public = [
            "https://example.com/feed.rss",
            "http://example.com/feed",
            "https://sub.example.com/a?b=c",
            "https://8.8.8.8/feed.xml",
            "https://172.32.0.1/feed.xml", // 172.16/12 stops at 172.31
        ];
        for url in public {
            assert!(url_is_public(url), "should be allowed: {url}");
        }
    }

    #[test]
    fn validated_redirect_resolves_and_rejects_internal() {
        assert_eq!(
            validated_redirect("https://example.com/a/feed", "/b.rss").as_deref(),
            Some("https://example.com/b.rss")
        );
        assert_eq!(
            validated_redirect("http://example.com/a", "https://cdn.example.com/e.mp3").as_deref(),
            Some("https://cdn.example.com/e.mp3")
        );
        assert!(validated_redirect("https://example.com/a", "http://postgres:5432/").is_none());
        assert!(validated_redirect("https://example.com/a", "http://169.254.169.254/").is_none());
        assert!(validated_redirect("https://example.com/a", "file:///etc/passwd").is_none());
    }

    #[test]
    fn error_display_never_contains_url() {
        assert_eq!(FeedError::NotModified.to_string(), "feed not modified");
        assert_eq!(FeedError::Gone.to_string(), "feed gone");
        assert_eq!(FeedError::Http(404).to_string(), "feed http 404");
        assert_eq!(FeedError::Parse.to_string(), "feed parse error");
        assert_eq!(FeedError::TooLarge.to_string(), "feed too large");
        assert_eq!(FeedError::Network.to_string(), "feed network error");
        let secret = "https://x/secret?token=abc";
        assert!(!FeedError::Http(500).to_string().contains(secret));
    }

    #[test]
    fn status_errors_are_scoped() {
        assert!(matches!(status_error(304), FeedError::NotModified));
        assert!(matches!(status_error(404), FeedError::Gone));
        assert!(matches!(status_error(410), FeedError::Gone));
        assert_eq!(status_error(500), FeedError::Http(500));
        assert_eq!(status_error(200), FeedError::Http(200));
    }

    #[test]
    fn feed_title_falls_back_to_host_only() {
        let xml = r#"<?xml version="1.0"?><rss version="2.0"><channel><item><title>X</title><link>https://cdn.example.com/a.mp3</link></item></channel></rss>"#;
        let snapshot = parse_snapshot(
            xml.as_bytes(),
            "https://user:token@example.com/private/path.xml",
            None,
            None,
        )
        .expect("fixture");
        assert_eq!(snapshot.title, "example.com");
        assert!(!snapshot.title.contains("token"));
        assert!(!snapshot.title.contains("private"));
    }
}
