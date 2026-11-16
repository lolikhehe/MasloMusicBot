//! Cold-path orchestration: metadata resolution, in-flight deduplication,
//! bounded downloads, cover/tag preparation and cache recording.

use audio;
use common::config::Config;
use common::domain::{
    Album, AlbumRef, CachedTrack, DownloadTarget, Provider, SearchResult, TrackKey, TrackMeta,
    TrackRequest,
};
use common::error::{AppResult, DownloadError};
use moka::future::Cache;
use providers::external::ExternalResolver;
use providers::ytdlp::YtDlp;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use storage::Db;
use tempfile::TempDir;
use tokio::sync::{Mutex, Semaphore, broadcast};
use tracing::{info, warn};

#[derive(Clone)]
pub struct PreparedAudio {
    pub meta: TrackMeta,
    pub audio_path: PathBuf,
    pub cover_path: Option<PathBuf>,
    /// Holds the temporary directory alive until the audio file is consumed.
    _dir: Arc<TempDir>,
}

impl PreparedAudio {
    pub fn file_name(&self) -> String {
        audio::safe_filename(&self.meta)
    }
}

type InflightResult = Result<PreparedAudio, Arc<DownloadError>>;

pub struct DownloadManager {
    config: Arc<Config>,
    db: Db,
    ytdlp: YtDlp,
    external: ExternalResolver,
    cover_client: reqwest::Client,
    semaphore: Arc<Semaphore>,
    inflight: Mutex<HashMap<(TrackKey, u32), broadcast::Sender<InflightResult>>>,
    user_semaphores: Cache<i64, Arc<Semaphore>>,
    search_cache: Cache<String, Vec<SearchResult>>,
}

impl DownloadManager {
    pub fn new(config: Arc<Config>, db: Db, ytdlp: YtDlp, external: ExternalResolver) -> Self {
        let cover_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("music-bot/0.1")
            .build()
            .unwrap_or_default();
        let search_cache = Cache::builder()
            .max_capacity(1_000)
            .time_to_live(Duration::from_secs(600))
            .build();
        let user_semaphores = Cache::builder().max_capacity(10_000).build();
        Self {
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_downloads)),
            config,
            db,
            ytdlp,
            external,
            cover_client,
            inflight: Mutex::new(HashMap::new()),
            user_semaphores,
            search_cache,
        }
    }

    pub async fn get_cached(&self, key: &TrackKey, quality: u32) -> AppResult<Option<CachedTrack>> {
        self.db.get_track(key, quality).await
    }

    pub async fn touch_cached(&self, key: &TrackKey, quality: u32) {
        if let Err(error) = self.db.touch_track(key, quality).await {
            warn!(%error, key = %key, "could not touch cache entry");
        }
    }

    pub async fn purge(&self, key: &TrackKey) -> AppResult<bool> {
        self.db.purge_track(key).await
    }

    /// Record a track in the cache. `size_bytes` is the actual audio size.
    pub async fn record(
        &self,
        request: &TrackRequest,
        meta: &TrackMeta,
        file_id: String,
        file_unique_id: String,
        quality: u32,
        size_bytes: Option<i64>,
    ) -> AppResult<()> {
        self.db
            .put_track(
                &CachedTrack {
                    key: request.key.clone(),
                    file_id,
                    file_unique_id,
                    source_url: request.source_url.clone(),
                    provider: request.provider,
                    meta: meta.clone(),
                },
                quality,
                size_bytes,
            )
            .await
    }

    /// Search for tracks. `preferred` is the user's `/settings` choice; it is
    /// honored when allowed by the operator's SEARCH_PROVIDERS allow-list,
    /// otherwise the first allowed source (YouTube → SoundCloud → Deezer) is
    /// used instead.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        preferred: Provider,
    ) -> Result<Vec<SearchResult>, DownloadError> {
        let preferred_source = [
            preferred,
            Provider::YouTube,
            Provider::SoundCloud,
            Provider::Deezer,
        ]
        .into_iter()
        .find(|candidate| self.config.search_providers.contains(candidate));
        if !self.config.search_providers.contains(&preferred)
            && let Some(fallback) = preferred_source
        {
            warn!(%preferred, %fallback, "user's search source not allowed by SEARCH_PROVIDERS; falling back");
        }
        let Some(source) = preferred_source else {
            warn!("no search source allowed by SEARCH_PROVIDERS");
            return Ok(Vec::new());
        };
        let normalized = query.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        let cache_key = format!("{}:{normalized}", source.as_str());
        if let Some(cached) = self.search_cache.get(&cache_key).await {
            return Ok(cached);
        }
        // L2: SQLite-backed search cache survives restarts (10-30 min TTL).
        if let Ok(Some(payload)) = self.db.get_search_cache(source.as_str(), &normalized).await
            && let Ok(parsed) = serde_json::from_str::<Vec<SearchResult>>(&payload)
        {
            self.search_cache.insert(cache_key, parsed.clone()).await;
            return Ok(parsed);
        }
        let results = match source {
            Provider::SoundCloud => self.ytdlp.search_soundcloud(query, limit).await?,
            Provider::Deezer => self.external.deezer_search(query, limit).await?,
            _ => self.ytdlp.search(query, limit).await?,
        };
        self.search_cache.insert(cache_key, results.clone()).await;
        if let Ok(payload) = serde_json::to_string(&results) {
            let _ = self
                .db
                .put_search_cache(
                    source.as_str(),
                    &normalized,
                    &payload,
                    Duration::from_secs(600),
                )
                .await;
        }
        Ok(results)
    }

    /// iTunes tag lookup for the per-user album/genre tags feature; one
    /// public API call, metadata only. Returns None on any failure so the
    /// track still sends without tags.
    pub async fn itunes_tags(
        &self,
        query: &str,
        duration_secs: Option<u32>,
    ) -> Option<(String, Option<String>)> {
        match self.external.itunes_tags(query, duration_secs).await {
            Ok(value) => value,
            Err(error) => {
                warn!(%error, "iTunes tag lookup failed");
                None
            }
        }
    }

    /// Resolve just enough metadata to show a track in inline results without
    /// downloading it.
    pub async fn describe(&self, mut request: TrackRequest) -> Result<TrackRequest, DownloadError> {
        if !self.config.provider_enabled(request.provider) {
            return Err(DownloadError::ProviderDisabled(request.provider));
        }
        if request.provider.is_external() {
            self.external.resolve_track(&mut request).await?;
            return Ok(request);
        }
        if request.meta.is_none()
            && let DownloadTarget::Url(url) = &request.target
        {
            let info = self.ytdlp.metadata(url).await?;
            request.source_url.get_or_insert(info.source_url);
            request.meta = Some(info.meta);
        }
        Ok(request)
    }

    pub async fn resolve_album(&self, album: &AlbumRef) -> Result<Album, DownloadError> {
        if !self.config.provider_enabled(album.provider) {
            return Err(DownloadError::ProviderDisabled(album.provider));
        }
        if album.provider.is_external() {
            self.external
                .resolve_album(album, self.config.max_collection_tracks)
                .await
        } else {
            self.ytdlp
                .playlist(album, self.config.max_collection_tracks)
                .await
        }
    }

    /// Download once per key, even when several requests arrive together.
    /// `user_id` enforces the per-user cap on concurrently active downloads;
    /// cached sends never reach here and are not throttled by it.
    pub async fn prepare(
        &self,
        request: TrackRequest,
        quality: u32,
        user_id: i64,
    ) -> Result<PreparedAudio, DownloadError> {
        if !self.config.provider_enabled(request.provider) {
            return Err(DownloadError::ProviderDisabled(request.provider));
        }

        let user_sem = self
            .user_semaphores
            .get_with(user_id, async {
                Arc::new(Semaphore::new(self.config.max_active_downloads_per_user))
            })
            .await;
        let _user_permit = user_sem
            .acquire_owned()
            .await
            .map_err(|_| DownloadError::Internal("user download semaphore closed".to_owned()))?;

        let cache_key = (request.key.clone(), quality);
        let mut receiver = None;
        let mut sender = None;

        {
            let mut inflight = self.inflight.lock().await;
            if let Some(existing) = inflight.get(&cache_key) {
                receiver = Some(existing.subscribe());
            } else {
                let (tx, _rx) = broadcast::channel(4);
                inflight.insert(cache_key.clone(), tx.clone());
                sender = Some(tx);
            }
            metrics::gauge!("music_bot_downloads_queued").set(inflight.len() as f64);
        }

        if let Some(mut rx) = receiver {
            loop {
                match rx.recv().await {
                    Ok(Ok(prepared)) => return Ok(prepared),
                    Ok(Err(error)) => return Err((*error).clone()),
                    Err(broadcast::error::RecvError::Closed) => {
                        // The leader disappeared without publishing. Try again.
                        return Box::pin(self.prepare(request, quality, user_id)).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        }

        let sender = sender.expect("leader sender must exist");
        let result = self.prepare_leader(request, quality).await;
        let broadcast_result = result.clone().map_err(|error| Arc::new(error.clone()));
        let _ = sender.send(broadcast_result);
        let mut inflight = self.inflight.lock().await;
        inflight.remove(&cache_key);
        metrics::gauge!("music_bot_downloads_queued").set(inflight.len() as f64);
        drop(inflight);
        result
    }

    async fn prepare_leader(
        &self,
        request: TrackRequest,
        quality: u32,
    ) -> Result<PreparedAudio, DownloadError> {
        metrics::gauge!("music_bot_downloads_inflight").increment(1.0);
        let result = self.prepare_leader_inner(request, quality).await;
        metrics::gauge!("music_bot_downloads_inflight").decrement(1.0);
        result
    }

    async fn prepare_leader_inner(
        &self,
        mut request: TrackRequest,
        quality: u32,
    ) -> Result<PreparedAudio, DownloadError> {
        if request.provider.is_external()
            && (request.meta.is_none()
                || matches!(&request.target, DownloadTarget::Search(query) if query.trim().is_empty()))
        {
            self.external.resolve_track(&mut request).await?;
        }
        if matches!(&request.target, DownloadTarget::Search(query) if query.trim().is_empty()) {
            let query = request
                .meta
                .as_ref()
                .map(TrackMeta::search_query)
                .ok_or_else(|| {
                    DownloadError::Internal("cannot build a search query for the track".to_owned())
                })?;
            request.target = DownloadTarget::Search(query);
        }

        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| DownloadError::Internal("download semaphore closed".to_owned()))?;

        let dir = tempfile::Builder::new()
            .prefix(&format!("track-{}", uuid::Uuid::new_v4()))
            .tempdir_in(&self.config.tmp_dir)
            .map_err(|error| {
                DownloadError::Internal(format!("cannot create temp directory: {error}"))
            })?;

        info!(key = %request.key, target = ?request.target, "starting download");
        let downloaded = {
            // One retry for transient network/RateLimit failures with
            // exponential backoff (spec §9 етап 3). Permanent failures
            // (private/region/too large) are never retried.
            match self
                .ytdlp
                .download(
                    &request.target,
                    dir.path(),
                    quality,
                    self.config.max_duration_secs,
                    self.config.max_file_bytes(),
                )
                .await
            {
                Ok(downloaded) => downloaded,
                Err(error) if is_transient(&error) => {
                    warn!(key = %request.key, error = %error, "transient download failure; retrying once");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    self.ytdlp
                        .download(
                            &request.target,
                            dir.path(),
                            quality,
                            self.config.max_duration_secs,
                            self.config.max_file_bytes(),
                        )
                        .await?
                }
                Err(error) => return Err(error),
            }
        };

        let merged_meta = merge_meta(request.meta.as_ref(), &downloaded.meta);
        if let Some(duration) = merged_meta.duration_secs
            && u64::from(duration) > self.config.max_duration_secs
        {
            return Err(DownloadError::TooLong {
                limit: self.config.max_duration_secs,
                actual: u64::from(duration),
            });
        }
        let cover_path = audio::prepare_cover(
            &self.cover_client,
            &self.config.ffmpeg_path,
            merged_meta.thumbnail_url.as_deref(),
            dir.path(),
        )
        .await;

        let audio_path = downloaded.audio_path.clone();
        audio::tag_audio(audio_path.clone(), merged_meta.clone(), cover_path.clone()).await;
        drop(_permit);

        Ok(PreparedAudio {
            meta: merged_meta,
            audio_path,
            cover_path,
            _dir: Arc::new(dir),
        })
    }
}

/// Transient failures worth one retry: timeouts, resets and rate limiting.
fn is_transient(error: &DownloadError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("timed out")
        || message.contains("timeout")
        || message.contains("connection")
        || message.contains("reset")
        || message.contains("too many requests")
        || message.contains("429")
}

fn merge_meta(requested: Option<&TrackMeta>, downloaded: &TrackMeta) -> TrackMeta {
    let Some(requested) = requested else {
        return downloaded.clone();
    };
    TrackMeta {
        title: if downloaded.title.trim().is_empty() {
            requested.title.clone()
        } else {
            downloaded.title.clone()
        },
        performer: downloaded
            .performer
            .clone()
            .or_else(|| requested.performer.clone()),
        album: requested.album.clone().or_else(|| downloaded.album.clone()),
        duration_secs: downloaded.duration_secs.or(requested.duration_secs),
        thumbnail_url: downloaded
            .thumbnail_url
            .clone()
            .or_else(|| requested.thumbnail_url.clone()),
        genres: if downloaded.genres.is_empty() {
            requested.genres.clone()
        } else {
            downloaded.genres.clone()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_requested_and_downloaded_metadata() {
        let requested = TrackMeta {
            title: "Requested".into(),
            performer: Some("Artist".into()),
            album: Some("Album".into()),
            duration_secs: Some(10),
            thumbnail_url: Some("https://x/cover.jpg".into()),
            genres: vec!["Rock".into()],
        };
        let downloaded = TrackMeta {
            title: "Downloaded".into(),
            performer: Some("Uploader".into()),
            ..Default::default()
        };
        let merged = merge_meta(Some(&requested), &downloaded);
        assert_eq!(merged.title, "Downloaded");
        assert_eq!(merged.performer.as_deref(), Some("Uploader"));
        assert_eq!(merged.album.as_deref(), Some("Album"));
        assert_eq!(merged.genres, vec!["Rock".to_owned()]);
        assert_eq!(merged.duration_secs, Some(10));
    }
}
