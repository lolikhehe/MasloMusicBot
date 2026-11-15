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
    inflight: Mutex<HashMap<TrackKey, broadcast::Sender<InflightResult>>>,
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
        Self {
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_downloads)),
            config,
            db,
            ytdlp,
            external,
            cover_client,
            inflight: Mutex::new(HashMap::new()),
            search_cache,
        }
    }

    pub async fn get_cached(&self, key: &TrackKey) -> AppResult<Option<CachedTrack>> {
        self.db.get_track(key).await
    }

    pub async fn touch_cached(&self, key: &TrackKey) {
        if let Err(error) = self.db.touch_track(key).await {
            warn!(%error, key = %key, "could not touch cache entry");
        }
    }

    pub async fn purge(&self, key: &TrackKey) -> AppResult<bool> {
        self.db.purge_track(key).await
    }

    pub async fn record(
        &self,
        request: &TrackRequest,
        meta: &TrackMeta,
        file_id: String,
        file_unique_id: String,
    ) -> AppResult<()> {
        self.db
            .put_track(&CachedTrack {
                key: request.key.clone(),
                file_id,
                file_unique_id,
                source_url: request.source_url.clone(),
                provider: request.provider,
                meta: meta.clone(),
            })
            .await
    }

    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, DownloadError> {
        if !self.config.search_providers.contains(&Provider::YouTube) {
            return Ok(Vec::new());
        }
        let normalized = query.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(cached) = self.search_cache.get(&normalized).await {
            return Ok(cached);
        }
        let results = self.ytdlp.search(query, limit).await?;
        self.search_cache.insert(normalized, results.clone()).await;
        Ok(results)
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
                .resolve_album(album, self.config.max_album_tracks)
                .await
        } else {
            self.ytdlp
                .playlist(album, self.config.max_album_tracks)
                .await
        }
    }

    /// Download once per key, even when several requests arrive together.
    pub async fn prepare(
        &self,
        request: TrackRequest,
        quality: u32,
    ) -> Result<PreparedAudio, DownloadError> {
        if !self.config.provider_enabled(request.provider) {
            return Err(DownloadError::ProviderDisabled(request.provider));
        }

        let key = request.key.clone();
        let mut receiver = None;
        let mut sender = None;

        {
            let mut inflight = self.inflight.lock().await;
            if let Some(existing) = inflight.get(&key) {
                receiver = Some(existing.subscribe());
            } else {
                let (tx, _rx) = broadcast::channel(4);
                inflight.insert(key.clone(), tx.clone());
                sender = Some(tx);
            }
        }

        if let Some(mut rx) = receiver {
            loop {
                match rx.recv().await {
                    Ok(Ok(prepared)) => return Ok(prepared),
                    Ok(Err(error)) => return Err((*error).clone()),
                    Err(broadcast::error::RecvError::Closed) => {
                        // The leader disappeared without publishing. Try again.
                        return Box::pin(self.prepare(request, quality)).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        }

        let sender = sender.expect("leader sender must exist");
        let result = self.prepare_leader(request, quality).await;
        let broadcast_result = result.clone().map_err(|error| Arc::new(error.clone()));
        let _ = sender.send(broadcast_result);
        self.inflight.lock().await.remove(&key);
        result
    }

    async fn prepare_leader(
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
        let downloaded = self
            .ytdlp
            .download(
                &request.target,
                dir.path(),
                quality,
                self.config.max_duration_secs,
                self.config.max_file_bytes(),
            )
            .await?;

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
        assert_eq!(merged.duration_secs, Some(10));
    }
}
