//! Public, token-free lyrics sources.
//!
//! LRCLIB is the primary source (it often has time-synced LRC text); lyrics.ovh
//! is the fallback. Both are public read-only HTTP APIs, so no credentials,
//! scraping of HTML pages, or DRM/paywall circumvention is involved. Bodies are
//! read through the shared capped `fetch_bytes` helper: a misbehaving host
//! cannot make the bot allocate an unbounded response.

use crate::http::fetch_bytes;
use serde::Deserialize;
use std::time::Duration;

/// Hard ceiling for a lyrics response body. Lyrics are text: half a megabyte is
/// far beyond any real song and keeps a hostile host from exhausting memory.
const MAX_BODY_BYTES: u64 = 512 * 1024;
/// Telegram caps a message at 4096 characters; leave room for the header/footer.
const MAX_LYRICS_CHARS: usize = 3500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsSource {
    LrclibSynced,
    LrclibPlain,
    LyricsOvh,
}

impl LyricsSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LrclibSynced => "LRCLIB (synced)",
            Self::LrclibPlain => "LRCLIB",
            Self::LyricsOvh => "lyrics.ovh",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lyrics {
    /// Plain text for chat messages (LRC timestamps stripped).
    pub text: String,
    pub source: LyricsSource,
    /// Original time-synced text, kept only for the optional `.lrc` download.
    /// Never written to disk by the bot.
    pub lrc: Option<String>,
}

impl Lyrics {
    pub fn is_synced(&self) -> bool {
        self.source == LyricsSource::LrclibSynced
    }
}

#[derive(Clone)]
pub struct LyricsClient {
    http: reqwest::Client,
    lrclib_base: String,
    ovh_base: String,
}

#[derive(Debug, Deserialize)]
struct LrclibRecord {
    #[serde(default, rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(default, rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

#[derive(Debug, Deserialize)]
struct OvhRecord {
    lyrics: Option<String>,
}

impl LyricsClient {
    pub fn new(http: reqwest::Client, lrclib_base: String, ovh_base: String) -> Self {
        Self {
            http,
            lrclib_base: lrclib_base.trim_end_matches('/').to_owned(),
            ovh_base: ovh_base.trim_end_matches('/').to_owned(),
        }
    }

    /// Fetch lyrics for an artist/title pair. `artist` may be `None` when the
    /// user supplied a free-text query; LRCLIB's `q` search handles that case.
    pub async fn fetch(&self, artist: Option<&str>, title: &str) -> Option<Lyrics> {
        let title = title.trim();
        if title.is_empty() {
            return None;
        }

        if let Some(lyrics) = self.fetch_lrclib(artist, title).await {
            return Some(lyrics);
        }
        self.ovh(artist, title).await
    }

    /// LRCLIB only — used by the "other source" button.
    pub async fn fetch_lrclib(&self, artist: Option<&str>, title: &str) -> Option<Lyrics> {
        self.lrclib_get(artist, title)
            .await
            .or(self.lrclib_search(artist, title).await)
    }

    /// lyrics.ovh only — used by the "other source" button.
    pub async fn fetch_ovh(&self, artist: Option<&str>, title: &str) -> Option<Lyrics> {
        self.ovh(artist, title).await
    }

    fn lrclib_url(&self, path: &str) -> Option<reqwest::Url> {
        reqwest::Url::parse(&format!("{}/{path}", self.lrclib_base)).ok()
    }

    async fn lrclib_get(&self, artist: Option<&str>, title: &str) -> Option<Lyrics> {
        let mut url = self.lrclib_url("api/get")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("track_name", title);
            if let Some(artist) = artist.filter(|artist| !artist.trim().is_empty()) {
                query.append_pair("artist_name", artist.trim());
            }
        }
        let bytes = fetch_bytes(&self.http, url.as_str(), MAX_BODY_BYTES).await?;
        let record: LrclibRecord = serde_json::from_slice(&bytes).ok()?;
        lyrics_from_lrclib(record)
    }

    async fn lrclib_search(&self, artist: Option<&str>, title: &str) -> Option<Lyrics> {
        let mut url = self.lrclib_url("api/search")?;
        {
            let mut query = url.query_pairs_mut();
            let q = match artist.filter(|artist| !artist.trim().is_empty()) {
                Some(artist) => format!("{} {}", artist.trim(), title),
                None => title.to_owned(),
            };
            query.append_pair("q", &q);
        }
        let bytes = fetch_bytes(&self.http, url.as_str(), MAX_BODY_BYTES).await?;
        let records: Vec<LrclibRecord> = serde_json::from_slice(&bytes).ok()?;
        records.into_iter().find_map(lyrics_from_lrclib)
    }

    async fn ovh(&self, artist: Option<&str>, title: &str) -> Option<Lyrics> {
        let artist = artist.filter(|artist| !artist.trim().is_empty())?;
        let mut url = reqwest::Url::parse(&format!("{}/", self.ovh_base)).ok()?;
        url.path_segments_mut()
            .ok()?
            .pop_if_empty()
            .push(artist.trim())
            .push(title);
        let bytes = fetch_bytes(&self.http, url.as_str(), MAX_BODY_BYTES).await?;
        let record: OvhRecord = serde_json::from_slice(&bytes).ok()?;
        let text = clean(&record.lyrics?);
        (!text.is_empty()).then_some(Lyrics {
            text,
            source: LyricsSource::LyricsOvh,
            lrc: None,
        })
    }
}

fn lyrics_from_lrclib(record: LrclibRecord) -> Option<Lyrics> {
    // Synced text is preferred: users can read it as plain text too after the
    // timestamps are stripped. An explicit "instrumental" flag with no text is
    // a real answer, not a failure.
    if let Some(synced) = record.synced_lyrics {
        let text = clean(&synced);
        if !text.is_empty() {
            return Some(Lyrics {
                text,
                source: LyricsSource::LrclibSynced,
                lrc: lrc_from_raw(&synced),
            });
        }
    }
    if let Some(plain) = record.plain_lyrics {
        let text = clean(&plain);
        if !text.is_empty() {
            return Some(Lyrics {
                text,
                source: LyricsSource::LrclibPlain,
                lrc: None,
            });
        }
    }
    let _ = record.instrumental;
    None
}

/// Keep the original LRC tags for a downloadable `.lrc` file. Only control
/// characters are dropped and the size is capped; in particular timestamps
/// are NOT stripped here (unlike [`clean`], which prepares chat messages).
pub fn lrc_from_raw(raw: &str) -> Option<String> {
    const MAX_LRC_CHARS: usize = 8_000;
    let filtered: String = raw
        .lines()
        .map(|line| {
            line.chars()
                .filter(|ch| *ch == '\t' || !ch.is_control())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let filtered = filtered.trim();
    if filtered.is_empty() {
        return None;
    }
    let mut result: String = filtered.chars().take(MAX_LRC_CHARS).collect();
    result.push('\n');
    Some(result)
}

/// Remove LRC timestamps/metadata tags and normalize whitespace. Multi-line
/// strings from the APIs are untrusted: control characters are dropped and the
/// result is capped so it always fits a Telegram message.
pub fn clean(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(raw.lines().count());
    for raw_line in raw.lines() {
        let mut line = raw_line.trim();
        // A line can carry several timestamps: `[00:12.00][00:48.00]Text`.
        while let Some(rest) = line.strip_prefix('[') {
            let Some((tag, after)) = rest.split_once(']') else {
                break;
            };
            if !is_lrc_tag(tag) {
                break;
            }
            line = after.trim_start();
        }
        let filtered: String = line
            .chars()
            .filter(|ch| *ch == '\n' || *ch == '\t' || !ch.is_control())
            .collect();
        let filtered = filtered.trim_end();
        if filtered.is_empty() {
            if lines.last().is_some_and(|last| !last.is_empty()) {
                lines.push(String::new());
            }
        } else {
            lines.push(filtered.to_owned());
        }
    }
    while lines.last().is_some_and(|last| last.is_empty()) {
        lines.pop();
    }

    let joined = lines.join("\n");
    if joined.chars().count() <= MAX_LYRICS_CHARS {
        return joined;
    }
    let mut result: String = joined.chars().take(MAX_LYRICS_CHARS).collect();
    result.push_str("\n…");
    result
}

/// `[mm:ss.xx]`, `[hh:mm:ss]`, `[ar:Artist]`, `[ti:Title]`, `[offset:…]`, …
fn is_lrc_tag(tag: &str) -> bool {
    let tag = tag.trim();
    if tag.is_empty() {
        return false;
    }
    if tag
        .chars()
        .all(|ch| ch.is_ascii_digit() || matches!(ch, ':' | '.' | '(' | ')' | '-'))
    {
        return true;
    }
    matches!(
        tag.split(':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "ar" | "ti" | "al" | "by" | "length" | "offset" | "re" | "ve" | "la"
    )
}

/// Default timeout for lyrics lookups. Short: a slow lyrics host must never
/// hold a Telegram handler open for the whole download timeout.
pub fn default_timeout() -> Duration {
    Duration::from_secs(8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Tiny single-purpose HTTP server. It returns canned JSON by path so the
    /// provider tests never touch the public internet.
    async fn mock_server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut buffer = vec![0u8; 8192];
                let Ok(read) = socket.read(&mut buffer).await else {
                    continue;
                };
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");
                let body = if path.starts_with("/api/get") {
                    "{\"plainLyrics\":null,\"syncedLyrics\":null,\"instrumental\":false}"
                } else if path.starts_with("/api/search") && path.contains("fallback") {
                    "[]"
                } else if path.starts_with("/api/search") {
                    "[{\"plainLyrics\":null,\"syncedLyrics\":\"[00:01.00]Hello\\r\\n[00:02.50]World\",\"instrumental\":false}]"
                } else if path.contains("fallback") {
                    "{\"lyrics\":\"From ovh\\nline two\"}"
                } else {
                    "{}"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        (format!("http://{addr}"), task)
    }

    fn client(base: &str, ovh: &str) -> LyricsClient {
        LyricsClient::new(
            crate::http::client(Duration::from_secs(2)).expect("client"),
            base.to_owned(),
            ovh.to_owned(),
        )
    }

    #[tokio::test]
    async fn strips_lrc_timestamps_and_uses_the_search_result() {
        let (base, task) = mock_server().await;
        let lyrics = client(&base, &base)
            .fetch(Some("Artist"), "Title")
            .await
            .expect("lyrics");
        assert_eq!(lyrics.text, "Hello\nWorld");
        assert_eq!(lyrics.source, LyricsSource::LrclibSynced);
        task.abort();
    }

    #[tokio::test]
    async fn falls_back_to_lyrics_ovh() {
        let (base, task) = mock_server().await;
        let lyrics = client(&base, &format!("{base}/fallback"))
            .fetch(Some("fallback"), "Title")
            .await
            .expect("lyrics");
        assert_eq!(lyrics.text, "From ovh\nline two");
        assert_eq!(lyrics.source, LyricsSource::LyricsOvh);
        task.abort();
    }

    #[test]
    fn lrc_file_keeps_timestamps_but_drops_control_chars() {
        let raw = "[ar:Artist]\n[00:01.00]Hello\r\n[00:02.00]World\u{7}";
        let lrc = lrc_from_raw(raw).expect("lrc");
        assert!(lrc.contains("[00:01.00]Hello"));
        assert!(lrc.contains("[00:02.00]World"));
        assert!(!lrc.contains('\r'));
        assert!(!lrc.contains('\u{7}'));
        assert!(lrc.ends_with('\n'));
    }

    #[test]
    fn clean_handles_metadata_tags_and_blank_lines() {
        assert_eq!(
            clean("[ar:Artist]\n[ti:Title]\n[00:01.23]one\n[00:02.00]two\n\n\n"),
            "one\ntwo"
        );
    }
}
