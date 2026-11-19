//! AcoustID / Chromaprint recognition: identify an uploaded audio file by its
//! sound. `fpcalc` computes the fingerprint locally; one HTTPS lookup to
//! acoustid.org resolves it to MusicBrainz recordings.
//!
//! Disabled in practice when no API key is configured: every call fails at the
//! lookup step, so nothing is ever sent. Kept as a separate provider so the
//! caller decides when to offer "recognize this audio".

use common::error::DownloadError;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

const LOOKUP_URL: &str = "https://api.acoustid.org/v2/lookup";

/// One recognized recording returned by AcoustID.
#[derive(Debug, Clone)]
pub struct AcoustMatch {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub score: f32,
}

#[derive(Clone)]
pub struct AcoustId {
    api_key: String,
    client: reqwest::Client,
}

impl AcoustId {
    /// Built from the operator's AcoustID API key, like the other metadata
    /// clients (`LastFmClient::new`).
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: crate::http::client(Duration::from_secs(10)).unwrap_or_default(),
        }
    }

    /// Recognize a local audio file. Runs `fpcalc` as a subprocess, then
    /// resolves the fingerprint with one AcoustID lookup. Returns matches
    /// sorted by score, highest first, with (title, artist) duplicates removed.
    /// An empty vector means "could not identify"; errors carry "acoustid".
    pub async fn identify(&self, audio_path: &Path) -> Result<Vec<AcoustMatch>, DownloadError> {
        let stdout = run_fpcalc(audio_path).await?;
        let (duration, fingerprint) = parse_fpcalc(&stdout)?;
        let duration_str = duration.to_string();
        let body = self
            .client
            .get(LOOKUP_URL)
            .query(&[
                ("client", self.api_key.as_str()),
                ("duration", duration_str.as_str()),
                ("fingerprint", fingerprint.as_str()),
                ("meta", "recordings releasegroups"),
            ])
            .send()
            .await
            .map_err(map_reqwest)?
            .error_for_status()
            .map_err(map_reqwest)?
            .text()
            .await
            .map_err(map_reqwest)?;
        parse_lookup(&body)
    }
}

/// Run `fpcalc -json -length 120 <path>` with a hard 60s ceiling. The child is
/// killed if the future is dropped (e.g. the timeout fires). No shell is used:
/// the path is passed as a single, separate argument.
async fn run_fpcalc(audio_path: &Path) -> Result<String, DownloadError> {
    let output = tokio::time::timeout(
        Duration::from_secs(60),
        tokio::process::Command::new("fpcalc")
            .args(["-json", "-length", "120"])
            .arg(audio_path)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| DownloadError::Audio("acoustid: fpcalc timed out".to_owned()))?
    .map_err(|error| DownloadError::Audio(format!("acoustid: cannot run fpcalc: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DownloadError::Audio(format!(
            "acoustid: fpcalc failed: {}",
            stderr.trim()
        )));
    }
    String::from_utf8(output.stdout).map_err(|error| {
        DownloadError::Audio(format!("acoustid: fpcalc output is not UTF-8: {error}"))
    })
}

/// Pure parser for `fpcalc -json` output, so it can be tested without the
/// binary. Returns (duration, fingerprint).
fn parse_fpcalc(json: &str) -> Result<(u32, String), DownloadError> {
    #[derive(Debug, Deserialize)]
    struct FpcalcOutput {
        #[serde(default)]
        duration: Option<u32>,
        #[serde(default)]
        fingerprint: Option<String>,
    }

    let output: FpcalcOutput = serde_json::from_str(json).map_err(|error| {
        DownloadError::Audio(format!("acoustid: invalid fpcalc output: {error}"))
    })?;
    let fingerprint = output
        .fingerprint
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            DownloadError::Audio("acoustid: fpcalc returned no fingerprint".to_owned())
        })?;
    Ok((output.duration.unwrap_or(0), fingerprint))
}

/// Parse an AcoustID v2 lookup body into sorted, de-duplicated matches. Kept
/// pure (takes the raw body) so the response shapes can be unit-tested offline.
fn parse_lookup(body: &str) -> Result<Vec<AcoustMatch>, DownloadError> {
    let response: LookupResponse = serde_json::from_str(body).map_err(|error| {
        DownloadError::SourceUnavailable(format!("acoustid: invalid response: {error}"))
    })?;
    if response.status.as_deref() != Some("ok") {
        let message = response
            .message
            .clone()
            .or_else(|| {
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "unknown error".to_owned());
        return Err(DownloadError::SourceUnavailable(format!(
            "acoustid error: {message}"
        )));
    }

    let mut matches = Vec::new();
    for result in response.results {
        for recording in result.recordings {
            let title = recording.title.as_deref().unwrap_or("").trim().to_owned();
            if title.is_empty() {
                continue;
            }
            let artist = recording
                .artists
                .first()
                .and_then(|artist| artist.name.as_deref())
                .unwrap_or("")
                .trim()
                .to_owned();
            let album = recording
                .releasegroups
                .first()
                .and_then(|group| group.title.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            let year = year_from_recording(&recording);
            matches.push(AcoustMatch {
                title,
                artist,
                album,
                year,
                score: result.score,
            });
        }
    }
    matches.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
    });
    let mut seen = HashSet::new();
    matches.retain(|matched| seen.insert((matched.title.clone(), matched.artist.clone())));
    Ok(matches)
}

/// The year comes from a release date if trivially present — either directly
/// on the recording or on its first release group. No extra HTTP calls.
fn year_from_recording(recording: &Recording) -> Option<i32> {
    recording
        .releases
        .first()
        .and_then(|release| release.date.as_deref())
        .or_else(|| {
            recording
                .releasegroups
                .first()
                .and_then(|group| group.releases.first())
                .and_then(|release| release.date.as_deref())
        })
        .and_then(year_from_date)
}

/// AcoustID dates are "YYYY", "YYYY-MM" or "YYYY-MM-DD"; take the leading year.
fn year_from_date(date: &str) -> Option<i32> {
    date.get(..4)?
        .parse::<i32>()
        .ok()
        .filter(|year| (1900..=2100).contains(year))
}

fn map_reqwest(error: reqwest::Error) -> DownloadError {
    DownloadError::SourceUnavailable(format!("acoustid request failed: {error}"))
}

#[derive(Debug, Deserialize)]
struct LookupResponse {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    results: Vec<LookupResult>,
    #[serde(default)]
    error: Option<serde_json::Value>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LookupResult {
    #[serde(default)]
    score: f32,
    #[serde(default)]
    recordings: Vec<Recording>,
}

#[derive(Debug, Deserialize)]
struct Recording {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artists: Vec<Named>,
    #[serde(default)]
    releasegroups: Vec<ReleaseGroup>,
    #[serde(default)]
    releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
struct Named {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseGroup {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
struct Release {
    #[serde(default)]
    date: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fpcalc_output() {
        let json = r#"{"duration":123,"fingerprint":"AQADtGabc"}"#;
        let (duration, fingerprint) = parse_fpcalc(json).expect("fpcalc output");
        assert_eq!(duration, 123);
        assert_eq!(fingerprint, "AQADtGabc");

        assert!(parse_fpcalc(r#"{"duration":1}"#).is_err(), "no fingerprint");
        assert!(parse_fpcalc(r#"{"duration":1,"fingerprint":"  "}"#).is_err());
        assert!(parse_fpcalc("not json").is_err());
    }

    #[test]
    fn parses_lookup_sorted_deduped_with_missing_album() {
        let json = r#"{
            "status": "ok",
            "results": [
                {"score": 0.95, "recordings": [{
                    "title": "Song A",
                    "artists": [{"name": "Artist X"}],
                    "releasegroups": [{"title": "Album One"}],
                    "releases": [{"date": "2001-05-06"}]
                }]},
                {"score": 0.80, "recordings": [
                    {"title": "Song B", "artists": [{"name": "Artist Y"}], "releasegroups": []},
                    {"title": "Song A", "artists": [{"name": "Artist X"}], "releasegroups": []}
                ]}
            ]
        }"#;
        let matches = parse_lookup(json).expect("lookup");
        assert_eq!(matches.len(), 2, "Song A must be de-duplicated");
        assert_eq!(matches[0].title, "Song A");
        assert_eq!(matches[0].artist, "Artist X");
        assert_eq!(matches[0].album.as_deref(), Some("Album One"));
        assert_eq!(matches[0].year, Some(2001));
        assert!((matches[0].score - 0.95).abs() < f32::EPSILON);
        assert_eq!(matches[1].title, "Song B");
        assert_eq!(matches[1].artist, "Artist Y");
        assert_eq!(matches[1].album, None, "no releasegroup -> no album");
        assert_eq!(matches[1].year, None);
    }

    #[test]
    fn lookup_error_status_is_err() {
        let json = r#"{"status":"error","error":{"code":404,"message":"invalid fingerprint"}}"#;
        let error = parse_lookup(json).expect_err("error status must fail");
        assert!(error.to_string().contains("acoustid"), "{error}");
    }

    #[test]
    fn empty_results_are_ok() {
        assert!(
            parse_lookup(r#"{"status":"ok","results":[]}"#)
                .expect("lookup")
                .is_empty()
        );
        assert!(
            parse_lookup(r#"{"status":"ok","results":[{"score":0.5,"recordings":[]}]}"#)
                .expect("lookup")
                .is_empty()
        );
    }

    #[test]
    fn year_from_nested_release_group_date() {
        let recording: Recording =
            serde_json::from_str(r#"{"releasegroups":[{"releases":[{"date":"1986"}]}]}"#)
                .expect("recording");
        assert_eq!(year_from_recording(&recording), Some(1986));
    }
}
