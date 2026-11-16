//! Environment-only configuration. Invalid or unsafe values fail fast at
//! startup instead of surfacing in the middle of a download.

use crate::domain::{Lang, Provider};
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub token: String,
    pub telegram_api_url: Option<String>,
    pub database_url: String,
    pub tmp_dir: PathBuf,
    pub enabled_providers: HashSet<Provider>,
    pub search_providers: HashSet<Provider>,
    pub max_duration_secs: u64,
    pub max_file_mb: u64,
    /// Cap on tracks per collection (album/playlist) per the spec (default 25).
    pub max_collection_tracks: usize,
    /// Per-user cap on concurrently active downloads (spec: e.g. 2).
    pub max_active_downloads_per_user: usize,
    pub rate_limit_per_min: u32,
    /// Inline queries fire once per keystroke; a separate (looser) budget.
    pub rate_limit_inline_per_min: u32,
    pub max_concurrent_downloads: usize,
    pub admins: HashSet<i64>,
    pub default_lang: Lang,
    pub storage_chat_id: Option<i64>,
    pub ytdlp_path: String,
    pub ffmpeg_path: String,
    pub log_level: String,
    pub log_json: bool,
    pub metrics_addr: Option<SocketAddr>,
    pub download_timeout: Duration,
    pub metadata_timeout: Duration,
    pub bot_api_wait: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let token = env_required("TELOXIDE_TOKEN")?;
        if token.trim().is_empty() {
            bail!("TELOXIDE_TOKEN is empty");
        }

        let telegram_api_url = env_optional("TELEGRAM_API_URL").map(|value| {
            if value.starts_with("http://") || value.starts_with("https://") {
                value.trim_end_matches('/').to_owned()
            } else {
                format!("http://{}", value.trim_end_matches('/'))
            }
        });

        let enabled_providers = parse_providers(
            &env_optional("ENABLED_PROVIDERS").unwrap_or_else(|| "all".to_owned()),
            Provider::ALL.as_slice(),
        )?;
        // Search sources the operator allows bot-wide. Default is everything
        // that can be searched anonymously; set SEARCH_PROVIDERS=youtube to
        // pin the old behavior. Spotify is not searchable without OAuth.
        let search_providers = parse_providers(
            &env_optional("SEARCH_PROVIDERS")
                .unwrap_or_else(|| "youtube,soundcloud,deezer".to_owned()),
            &[
                Provider::YouTube,
                Provider::YouTubeMusic,
                Provider::SoundCloud,
                Provider::Deezer,
            ],
        )?;

        // MAX_COLLECTION_TRACKS is the spec name (§9 stage 6), default 25;
        // MAX_ALBUM_TRACKS is the legacy alias kept for compatibility.
        let collection_default = env_optional("MAX_ALBUM_TRACKS")
            .map(|v| v.parse::<usize>().context("MAX_ALBUM_TRACKS"))
            .transpose()?
            .unwrap_or(25);
        let max_collection_tracks = env_parse("MAX_COLLECTION_TRACKS", collection_default)?;
        let max_active_downloads_per_user = env_parse("MAX_ACTIVE_DOWNLOADS_PER_USER", 2usize)?;
        let max_concurrent_downloads = env_parse("MAX_CONCURRENT_DOWNLOADS", 4usize)?;
        if max_collection_tracks == 0 || max_collection_tracks > 50 {
            bail!("MAX_COLLECTION_TRACKS must be between 1 and 50");
        }
        if max_active_downloads_per_user == 0 || max_active_downloads_per_user > 16 {
            bail!("MAX_ACTIVE_DOWNLOADS_PER_USER must be between 1 and 16");
        }
        if max_concurrent_downloads == 0 || max_concurrent_downloads > 64 {
            bail!("MAX_CONCURRENT_DOWNLOADS must be between 1 and 64");
        }

        let metrics_addr = match env_optional("METRICS_ADDR") {
            None => Some("0.0.0.0:9090".parse().context("METRICS_ADDR")?),
            Some(value) if value.is_empty() || value.eq_ignore_ascii_case("off") => None,
            Some(value) => Some(value.parse().context("METRICS_ADDR must be host:port")?),
        };

        let admins = env_optional("ADMINS")
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(|part| {
                        part.parse::<i64>()
                            .with_context(|| format!("invalid admin id: {part}"))
                    })
                    .collect::<Result<HashSet<_>>>()
            })
            .transpose()?
            .unwrap_or_default();

        let storage_chat_id = env_optional("STORAGE_CHAT_ID")
            .map(|value| {
                value
                    .trim()
                    .parse::<i64>()
                    .with_context(|| format!("STORAGE_CHAT_ID must be a numeric chat id: {value}"))
            })
            .transpose()?;

        let mut tmp_dir =
            PathBuf::from(env_optional("TMP_DIR").unwrap_or_else(|| "./data/tmp".to_owned()));
        if tmp_dir.as_os_str().is_empty() {
            tmp_dir = PathBuf::from("./data/tmp");
        }

        Ok(Self {
            token,
            telegram_api_url,
            database_url: env_optional("DATABASE_URL")
                .unwrap_or_else(|| "sqlite://data/bot.db".to_owned()),
            tmp_dir,
            enabled_providers,
            search_providers,
            max_duration_secs: env_parse("MAX_DURATION_SECS", 1200u64)?,
            max_file_mb: env_parse("MAX_FILE_MB", 100u64)?.clamp(1, 2000),
            max_collection_tracks,
            max_active_downloads_per_user,
            rate_limit_per_min: env_parse("RATE_LIMIT_PER_MIN", 10u32)?,
            rate_limit_inline_per_min: env_parse("RATE_LIMIT_INLINE_PER_MIN", 120u32)?,
            max_concurrent_downloads,
            admins,
            storage_chat_id,
            default_lang: env_optional("DEFAULT_LANG")
                .and_then(|value| Lang::from_code(&value))
                .unwrap_or(Lang::En),
            ytdlp_path: env_optional("YTDLP_PATH").unwrap_or_else(|| "yt-dlp".to_owned()),
            ffmpeg_path: env_optional("FFMPEG_PATH").unwrap_or_else(|| "ffmpeg".to_owned()),
            log_level: env_optional("LOG_LEVEL").unwrap_or_else(|| "info".to_owned()),
            log_json: env_optional("LOG_FORMAT")
                .map(|value| value.eq_ignore_ascii_case("json"))
                .unwrap_or(false),
            metrics_addr,
            download_timeout: Duration::from_secs(env_parse("DOWNLOAD_TIMEOUT_SECS", 300u64)?),
            metadata_timeout: Duration::from_secs(env_parse("METADATA_TIMEOUT_SECS", 30u64)?),
            bot_api_wait: Duration::from_secs(env_parse("BOT_API_WAIT_SECS", 20u64)?),
        })
    }

    pub fn is_admin(&self, user_id: i64) -> bool {
        self.admins.contains(&user_id)
    }

    pub fn provider_enabled(&self, provider: Provider) -> bool {
        self.enabled_providers.contains(&provider)
    }

    pub fn max_file_bytes(&self) -> u64 {
        self.max_file_mb.saturating_mul(1_000_000)
    }
}

fn env_optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn env_required(key: &str) -> Result<String> {
    env_optional(key).with_context(|| format!("{key} is required"))
}

fn env_parse<T>(key: &str, default: T) -> Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match env_optional(key) {
        Some(value) => value
            .parse::<T>()
            .map_err(|error| anyhow::anyhow!("{key}: {error}")),
        None => Ok(default),
    }
}

fn parse_providers(value: &str, allowed: &[Provider]) -> Result<HashSet<Provider>> {
    let mut result = HashSet::new();
    if value.trim().eq_ignore_ascii_case("all") {
        return Ok(allowed.iter().copied().collect());
    }
    for part in value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let provider = Provider::from_str(part)
            .map_err(|_| anyhow::anyhow!("unknown provider in ENABLED_PROVIDERS: {part}"))?;
        if !allowed.contains(&provider) {
            bail!("provider {} is not supported here", provider);
        }
        result.insert(provider);
    }
    if result.is_empty() {
        bail!("at least one provider must be enabled");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_lists() {
        let parsed = parse_providers("youtube, soundcloud,spotify", Provider::ALL.as_slice())
            .expect("providers");
        assert!(parsed.contains(&Provider::YouTube));
        assert!(parsed.contains(&Provider::SoundCloud));
        assert!(parsed.contains(&Provider::Spotify));
        assert!(!parsed.contains(&Provider::Deezer));
    }

    #[test]
    fn search_only_allows_youtube() {
        assert!(parse_providers("soundcloud", &[Provider::YouTube]).is_err());
    }
}
