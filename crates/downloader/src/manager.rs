//! Cold-path orchestration: metadata resolution, in-flight deduplication,
//! bounded downloads, cover/tag preparation and cache recording.

use crate::zip::{MAX_ZIP_BYTES, ZipOutcome, ZipWriter, exceeds_track_cap, zip_entry_name};
use audio;
use common::config::Config;
use common::domain::{
    Album, AlbumRef, AlbumSearchResult, CachedTrack, DownloadTarget, Provider, SearchResult,
    TrackKey, TrackMeta, TrackRequest,
};
use common::error::{AppResult, DownloadError};
use moka::future::Cache;
use providers::external::{AlbumFacts, ExternalResolver};
use providers::ytdlp::YtDlp;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use storage::Db;
use tempfile::TempDir;
use tokio::sync::{Semaphore, broadcast};
use tracing::{info, warn};

#[derive(Clone)]
pub struct PreparedAudio {
    pub meta: TrackMeta,
    pub audio_path: PathBuf,
    /// Full-size cover embedded into the audio tags.
    pub cover_path: Option<PathBuf>,
    /// Small (<200 KB) cover for Telegram's `thumbnail` parameter.
    pub thumb_path: Option<PathBuf>,
    /// Holds the temporary directory alive until the audio file is consumed.
    _dir: Arc<TempDir>,
    /// Keeps the singleflight entry alive until this audio (and every waiter's
    /// clone) is dropped, i.e. after the file id has been recorded.
    _inflight_guard: Option<Arc<InflightGuard>>,
}

impl PreparedAudio {
    pub fn file_name(&self) -> String {
        let extension = self
            .audio_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("m4a");
        audio::safe_filename(&self.meta, extension)
    }
}

/// Optional ffmpeg post-processing. Failures are non-fatal: the original
/// prepared audio is kept so a bad ffmpeg invocation cannot lose a download.
pub async fn postprocess_audio(
    prepared: &mut PreparedAudio,
    format: &str,
    normalize: bool,
    ffmpeg_path: &str,
) {
    let extension: String = match format {
        "m4a" => "m4a".to_owned(),
        "mp3" => "mp3".to_owned(),
        "flac" => "flac".to_owned(),
        "opus" => "opus".to_owned(),
        "auto" | "" => {
            if !normalize {
                return;
            }
            prepared
                .audio_path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("m4a")
                .to_owned()
        }
        _ => return,
    };

    let stem = prepared
        .audio_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("audio");
    let target = prepared
        .audio_path
        .with_file_name(format!("{stem}.processed.{extension}"));

    let mut command = tokio::process::Command::new(ffmpeg_path);
    command
        .arg("-y")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(&prepared.audio_path);
    if normalize {
        command.arg("-af").arg("loudnorm=I=-16:TP=-1.5:LRA=11");
    }
    match extension.as_str() {
        "mp3" => {
            command.args(["-c:a", "libmp3lame", "-b:a", "192k"]);
        }
        "m4a" => {
            command.args(["-c:a", "aac", "-b:a", "192k"]);
        }
        "flac" => {
            command.args(["-c:a", "flac"]);
        }
        "opus" => {
            command.args(["-c:a", "libopus", "-b:a", "128k"]);
        }
        _ => {}
    }
    command.kill_on_drop(true).arg(&target);

    match tokio::time::timeout(Duration::from_secs(30), command.output()).await {
        Ok(Ok(output))
            if output.status.success()
                && tokio::fs::metadata(&target)
                    .await
                    .is_ok_and(|metadata| metadata.len() > 0) =>
        {
            prepared.audio_path = target;
            metrics::counter!("music_bot_audio_postprocessed_total").increment(1);
        }
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(error = %stderr.trim(), "ffmpeg post-processing failed; keeping original audio");
        }
        Ok(Err(error)) => {
            warn!(%error, "could not spawn ffmpeg; keeping original audio");
        }
        Err(_) => {
            warn!("ffmpeg post-processing timed out; keeping original audio");
        }
    }
}

/// Aborts a detached cover task on every exit path. Dropping a bare
/// `JoinHandle` only detaches it, which would leave an ffmpeg child writing
/// into a `TempDir` that is deleted as soon as the download fails.
struct AbortOnDrop<T>(Option<tokio::task::JoinHandle<T>>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.0.as_ref() {
            handle.abort();
        }
    }
}

impl<T> AbortOnDrop<T> {
    /// Await in place so the handle stays owned by the guard (and is aborted
    /// by its Drop if this wait is cancelled by a timeout).
    async fn wait(&mut self) -> Result<T, tokio::task::JoinError> {
        let handle = self.0.as_mut().expect("cover join handle");
        std::future::poll_fn(|cx| std::pin::Pin::new(&mut *handle).poll(cx)).await
    }
}

type InflightResult = Result<PreparedAudio, Arc<DownloadError>>;
/// (track, quality, hq_cover) — cover quality is per-request, so a piggybacker
/// must not inherit the leader's cover choice.
type InflightKey = (TrackKey, u32, bool);

/// ZIP downloads share one sentinel user bucket; the real limiter is the
/// dedicated `zip_semaphore` held for the whole `zip_album` build.
const ZIP_USER_ID: i64 = 0;

/// How many recent (bytes, download_secs) samples feed the speed estimate.
const SPEED_SAMPLE_CAPACITY: usize = 16;

/// Mean bytes/sec over the recent samples, or `None` with no data.
fn mean_bps(samples: &VecDeque<(u64, u64)>) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let bytes: u64 = samples.iter().map(|(bytes, _)| *bytes).sum();
    let secs: u64 = samples.iter().map(|(_, secs)| *secs).sum();
    (secs > 0).then(|| bytes as f64 / secs as f64)
}

/// Pure ETA formula: bytes = duration_secs * quality_kbps * 1000 / 8, so
/// seconds = bytes / speed, floored at 1. `quality_kbps == 0` is the
/// FLAC-lossless sentinel — estimate those with a typical lossless rate
/// instead of multiplying by zero.
fn download_secs_for(duration_secs: u32, quality_kbps: u32, speed_bps: f64) -> f64 {
    let kbps = if quality_kbps == 0 {
        1000
    } else {
        quality_kbps
    };
    let bytes = f64::from(duration_secs) * f64::from(kbps) * 125.0;
    (bytes / speed_bps).max(1.0)
}

/// Same ETA formula as [`DownloadManager::estimate_download_secs`] but driven by
/// a speed published by another replica, for queue-average estimates.
pub fn estimate_secs_from_bps(duration_secs: u32, quality_kbps: u32, speed_bps: f64) -> f64 {
    download_secs_for(duration_secs, quality_kbps, speed_bps)
}

/// Decrements the inflight gauge on **every** exit path, including cancellation
/// of the leader future (a plain decrement after `.await` leaks on cancel).
struct InflightMetricGuard;

impl InflightMetricGuard {
    fn new() -> Self {
        metrics::gauge!("music_bot_downloads_inflight").increment(1.0);
        Self
    }
}

impl Drop for InflightMetricGuard {
    fn drop(&mut self) {
        metrics::gauge!("music_bot_downloads_inflight").decrement(1.0);
    }
}

/// Removes this in-flight entry on **every** exit path, including a panic in
/// the leader. The leader holds one armed guard while downloading; on success
/// it disarms that guard and an identical guard travels inside the returned
/// `PreparedAudio`, keeping the entry alive until every consumer (which shares
/// it via `Arc`) has dropped its copy — i.e. after the file id is recorded.
struct InflightGuard {
    inflight: Arc<Mutex<HashMap<InflightKey, broadcast::Sender<InflightResult>>>>,
    key: InflightKey,
    armed: bool,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut map = self
            .inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.remove(&self.key);
        metrics::gauge!("music_bot_downloads_queued").set(map.len() as f64);
    }
}

pub struct DownloadManager {
    config: Arc<Config>,
    db: Db,
    ytdlp: YtDlp,
    external: ExternalResolver,
    cover_client: reqwest::Client,
    semaphore: Arc<Semaphore>,
    /// Caps whole-album ZIP builds: several concurrent 900 MB archives OOM the
    /// 2 GB container, so only one is allowed to build at a time.
    zip_semaphore: Arc<Semaphore>,
    /// Recent honest network-speed samples: (bytes, download-phase seconds).
    speed_samples: Arc<Mutex<VecDeque<(u64, u64)>>>,
    inflight: Arc<Mutex<HashMap<InflightKey, broadcast::Sender<InflightResult>>>>,
    user_semaphores: Cache<i64, Arc<Semaphore>>,
    search_cache: Cache<String, Vec<SearchResult>>,
    album_search_cache: Cache<String, Vec<AlbumSearchResult>>,
}

impl DownloadManager {
    pub fn new(config: Arc<Config>, db: Db, ytdlp: YtDlp, external: ExternalResolver) -> Self {
        let cover_client = providers::http::client(Duration::from_secs(20)).unwrap_or_default();
        let search_cache = Cache::builder()
            .max_capacity(1_000)
            .time_to_live(Duration::from_secs(600))
            .build();
        let album_search_cache = Cache::builder()
            .max_capacity(1_000)
            .time_to_live(Duration::from_secs(600))
            .build();
        let user_semaphores = Cache::builder().max_capacity(10_000).build();
        Self {
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_downloads)),
            zip_semaphore: Arc::new(Semaphore::new(1)),
            speed_samples: Arc::new(Mutex::new(VecDeque::with_capacity(SPEED_SAMPLE_CAPACITY))),
            config,
            db,
            ytdlp,
            external,
            cover_client,
            inflight: Arc::new(Mutex::new(HashMap::new())),
            user_semaphores,
            search_cache,
            album_search_cache,
        }
    }

    /// Mean bytes/sec measured over the last few completed downloads (network
    /// phase only: queue wait, cover work and Telegram upload are excluded).
    /// `None` until at least one download has been recorded.
    pub fn download_speed_bps(&self) -> Option<f64> {
        let samples = self
            .speed_samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        mean_bps(&samples)
    }

    /// Estimated download time in seconds for a track of `duration_secs` at
    /// `quality_kbps`, derived from the recent network speed. `None` when no
    /// samples exist yet. Never below 1 second.
    pub fn estimate_download_secs(&self, duration_secs: u32, quality_kbps: u32) -> Option<f64> {
        self.download_speed_bps()
            .map(|speed| download_secs_for(duration_secs, quality_kbps, speed))
    }

    fn record_speed_sample(&self, bytes: u64, download_secs: u64) {
        let mut samples = self
            .speed_samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if samples.len() == SPEED_SAMPLE_CAPACITY {
            samples.pop_front();
        }
        // Floor at 1s so a sub-second download cannot divide by zero later.
        samples.push_back((bytes, download_secs.max(1)));
    }

    pub async fn get_cached(&self, key: &TrackKey, quality: u32) -> AppResult<Option<CachedTrack>> {
        self.db.get_track(key, quality).await
    }

    pub async fn touch_cached(&self, key: &TrackKey, quality: u32) {
        // The reply to the user must not wait for this bookkeeping write:
        // it can take as long as a Postgres roundtrip takes on a busy host.
        let db = self.db.clone();
        let key = key.clone();
        tokio::spawn(async move {
            if let Err(error) = db.touch_track(&key, quality).await {
                warn!(%error, key = %key, "could not touch cache entry");
            }
        });
    }

    pub async fn purge(&self, key: &TrackKey) -> AppResult<bool> {
        self.db.purge_track(key).await
    }

    /// Record a track in the cache. `size_bytes` is the actual audio size.
    /// `chat_id`/`message_id` point at the message that carried the audio so a
    /// later cache hit can copy it instead of resending by file_id.
    #[allow(clippy::too_many_arguments)]
    pub async fn record(
        &self,
        request: &TrackRequest,
        meta: &TrackMeta,
        file_id: String,
        file_unique_id: String,
        quality: u32,
        size_bytes: Option<i64>,
        chat_id: Option<i64>,
        message_id: Option<i64>,
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
                    chat_id,
                    message_id,
                },
                quality,
                size_bytes,
                chat_id,
                message_id,
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
        // The limit is part of the key: page turning re-asks with a bigger
        // limit and must not get the shorter first page back.
        let cache_key = format!("{}:{limit}:{normalized}", source.as_str());
        if let Some(cached) = self.search_cache.get(&cache_key).await {
            return Ok(cached);
        }
        // L2: Postgres-backed search cache survives restarts (10 min TTL).
        // The limit rides in the DB key's query slot.
        if let Ok(Some(payload)) = self
            .db
            .get_search_cache(source.as_str(), &format!("{limit}:{normalized}"))
            .await
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
                    &format!("{limit}:{normalized}"),
                    &payload,
                    Duration::from_secs(600),
                )
                .await;
        }
        Ok(results)
    }

    /// Public playlist search for `/playlist <name>` (Deezer, no token).
    pub async fn search_playlists(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<AlbumSearchResult>, DownloadError> {
        self.external.deezer_playlist_search(query, limit).await
    }

    /// Search public album catalogs (Deezer + Apple Music) for `/alb <name>`.
    /// Failures on one source don't sink the other; both failing surfaces an
    /// error so the user knows the catalogs are unreachable, not empty.
    /// Candidates are deduplicated by (artist, title) and capped at `limit`.
    pub async fn search_albums(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<AlbumSearchResult>, DownloadError> {
        let normalized = query.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 20);
        // The limit is part of the key: page turning re-asks with a bigger
        // limit and must not get the shorter first page back.
        let cache_key = format!("{limit}:{normalized}");
        if let Some(cached) = self.album_search_cache.get(&cache_key).await {
            return Ok(cached);
        }
        let (deezer, apple) = tokio::join!(
            self.external.deezer_album_search(query, limit),
            self.external.apple_album_search(query, limit),
        );
        let mut last_error = None;
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for entry in [deezer, apple] {
            match entry {
                Ok(results) => {
                    for result in results {
                        if seen.insert(result.dedupe_key()) {
                            out.push(result);
                        }
                    }
                }
                Err(error) => {
                    warn!(%error, "album catalog search failed");
                    last_error = Some(error);
                }
            }
        }
        if out.is_empty()
            && let Some(error) = last_error
        {
            return Err(error);
        }
        out.truncate(limit);
        self.album_search_cache.insert(cache_key, out.clone()).await;
        Ok(out)
    }

    /// iTunes tag lookup for the per-user album/genre tags feature; one
    /// public API call, metadata only. Bounded to 5 s so a slow API never
    /// holds the send. Returns None on any failure so the track still
    /// sends without tags.
    pub async fn itunes_tags(
        &self,
        query: &str,
        duration_secs: Option<u32>,
    ) -> Option<(String, Option<String>)> {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.external.itunes_tags(query, duration_secs),
        )
        .await
        {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => {
                warn!(%error, "iTunes tag lookup failed");
                None
            }
            Err(_) => {
                warn!("iTunes tag lookup timed out");
                None
            }
        }
    }

    /// Resolve missing collage artwork through the public Deezer catalog. One
    /// short metadata request, bounded to 6 s; `None` on any failure so the
    /// collage still renders with a placeholder cell.
    pub async fn artwork_url(
        &self,
        kind: providers::external::ArtworkKind,
        query: &str,
    ) -> Option<String> {
        match tokio::time::timeout(
            Duration::from_secs(6),
            self.external.deezer_artwork(kind, query),
        )
        .await
        {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => {
                warn!(%error, "Deezer artwork lookup failed");
                None
            }
            Err(_) => {
                warn!("Deezer artwork lookup timed out");
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

    /// Best-effort catalog facts for the album-info message: `None` when the
    /// source has nothing (non-Deezer albums, playlists, catalog misses).
    pub async fn album_facts(&self, album: &Album) -> Option<AlbumFacts> {
        if album.provider != Provider::Deezer {
            return None;
        }
        match self
            .external
            .deezer_album_facts(album.source_url.as_deref())
            .await
        {
            Ok(facts) => facts,
            Err(error) => {
                warn!(%error, "album facts unavailable");
                None
            }
        }
    }

    /// `/artist <name>`: the Deezer catalog's top tracks for the best match.
    pub async fn artist_top(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Option<Vec<SearchResult>>, DownloadError> {
        self.external.artist_top(query, limit).await
    }

    /// Download once per key, even when several requests arrive together.
    /// `user_id` enforces the per-user cap on concurrently active downloads;
    /// cached sends never reach here and are not throttled by it.
    pub async fn prepare(
        &self,
        request: TrackRequest,
        quality: u32,
        user_id: i64,
        hq_cover: bool,
    ) -> Result<PreparedAudio, DownloadError> {
        if !self.config.provider_enabled(request.provider) {
            return Err(DownloadError::ProviderDisabled(request.provider));
        }

        let cache_key = (request.key.clone(), quality, hq_cover);
        let (sender, receiver, queued) = {
            let mut inflight = self
                .inflight
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let (sender, receiver) = if let Some(existing) = inflight.get(&cache_key) {
                (None, Some(existing.subscribe()))
            } else {
                let (tx, _rx) = broadcast::channel(4);
                inflight.insert(cache_key.clone(), tx.clone());
                (Some(tx), None)
            };
            (sender, receiver, inflight.len())
        };
        metrics::gauge!("music_bot_downloads_queued").set(queued as f64);

        if let Some(mut rx) = receiver {
            // A leader that hangs (or whose process dies mid-download) must not
            // block its waiters forever. Bound the wait by the same timeout the
            // download itself has, plus a small grace margin.
            let wait_timeout = self.config.download_timeout + Duration::from_secs(10);
            loop {
                match tokio::time::timeout(wait_timeout, rx.recv()).await {
                    Ok(Ok(Ok(prepared))) => return Ok(prepared),
                    Ok(Ok(Err(error))) => return Err((*error).clone()),
                    Ok(Err(broadcast::error::RecvError::Closed)) => {
                        // A concurrent request may have finished and cached the
                        // track while we waited: prefer that source over a blind
                        // re-download. Followers hold no per-user permit here,
                        // so nothing is pinned across the retry (the old code
                        // held it and deadlocked both user permits).
                        let mut retry = request;
                        if let Ok(Some(cached)) = self.get_cached(&retry.key, quality).await
                            && let Some(url) = cached.source_url
                        {
                            retry.provider = cached.provider;
                            retry.target = DownloadTarget::Url(url);
                            retry.meta = Some(cached.meta);
                        }
                        return Box::pin(self.prepare(retry, quality, user_id, hq_cover)).await;
                    }
                    Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                    Err(_) => {
                        return Err(DownloadError::SourceUnavailable(
                            "timed out waiting for the running download".to_owned(),
                        ));
                    }
                }
            }
        }

        let sender = sender.expect("leader sender must exist");
        // Armed until the download resolves: on failure (or a leader panic)
        // this removes the entry so waiters can retry. On success it is
        // disarmed and an identical guard rides along in `PreparedAudio`,
        // keeping the entry alive through the handler's file-id recording.
        // Created before the permit acquire so a closed semaphore cannot leak
        // the inflight entry.
        let mut guard = InflightGuard {
            inflight: Arc::clone(&self.inflight),
            key: cache_key.clone(),
            armed: true,
        };
        // Only the leader takes a per-user slot: followers subscribe above
        // without ever waiting on this semaphore (fixes followers blocking on
        // the permit instead of subscribing to the running download).
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

        let result = match self.prepare_leader(request, quality, hq_cover).await {
            Ok(mut prepared) => {
                guard.armed = false;
                prepared._inflight_guard = Some(Arc::new(InflightGuard {
                    inflight: Arc::clone(&self.inflight),
                    key: cache_key,
                    armed: true,
                }));
                Ok(prepared)
            }
            Err(error) => Err(error),
        };
        let broadcast_result = result.clone().map_err(Arc::new);
        let _ = sender.send(broadcast_result);
        result
    }

    async fn prepare_leader(
        &self,
        request: TrackRequest,
        quality: u32,
        hq_cover: bool,
    ) -> Result<PreparedAudio, DownloadError> {
        // RAII so a cancelled leader cannot drift the gauge.
        let _metric = InflightMetricGuard::new();
        self.prepare_leader_inner(request, quality, hq_cover).await
    }

    async fn prepare_leader_inner(
        &self,
        mut request: TrackRequest,
        quality: u32,
        hq_cover: bool,
    ) -> Result<PreparedAudio, DownloadError> {
        if request.provider.is_external()
            && (request.meta.is_none() || is_empty_search_query(&request.target))
        {
            self.external.resolve_track(&mut request).await?;
        }
        if is_empty_search_query(&request.target) {
            let query = request
                .meta
                .as_ref()
                .map(TrackMeta::search_query)
                .ok_or_else(|| {
                    DownloadError::Internal("cannot build a search query for the track".to_owned())
                })?;
            request.target = DownloadTarget::Search(query);
        }

        // Bound the acquire itself: an unbounded wait here stalls the whole
        // queue behind one stuck download.
        let _permit = tokio::time::timeout(self.config.download_timeout, self.semaphore.acquire())
            .await
            .map_err(|_| {
                DownloadError::SourceUnavailable("timed out waiting for a download slot".to_owned())
            })?
            .map_err(|_| DownloadError::Internal("download semaphore closed".to_owned()))?;

        let dir = tempfile::Builder::new()
            .prefix(&format!("track-{}", uuid::Uuid::new_v4()))
            .tempdir_in(&self.config.tmp_dir)
            .map_err(|error| {
                DownloadError::Internal(format!("cannot create temp directory: {error}"))
            })?;

        // Overlap the cover download+resize with the audio download when the
        // thumbnail URL is already known (search picks always carry one).
        let mut cover_task = request
            .meta
            .as_ref()
            .and_then(|meta| meta.thumbnail_url.clone())
            .map(|url| {
                let client = self.cover_client.clone();
                let ffmpeg = self.config.ffmpeg_path.clone();
                let dir_path = dir.path().to_owned();
                AbortOnDrop(Some(tokio::spawn(async move {
                    audio::prepare_cover(&client, &ffmpeg, Some(&url), &dir_path, hq_cover).await
                })))
            });

        info!(key = %request.key, target = ?request.target, "starting download");
        // Network-phase window: starts with the download, stops as soon as the
        // file is on disk — before cover/tag work or Telegram upload.
        let download_started = Instant::now();
        // Fallback chain: the same track on other openly available catalogs.
        // A bare restricted link with no metadata gives us nothing to search
        // for, so it gets only its own attempt; known tracks roll on to
        // YouTube and SoundCloud search until one serves the audio.
        let candidates = fallback_candidates(&request);
        let mut downloaded: Option<providers::ytdlp::DownloadedAudio> = None;
        let mut last_error: Option<DownloadError> = None;
        for (attempt, target) in candidates.iter().enumerate() {
            if attempt > 0 {
                metrics::counter!("music_bot_download_fallbacks_total").increment(1);
                warn!(key = %request.key, ?target, "source refused the track; trying another catalog");
                clear_dir(dir.path()).await;
            }
            match download_attempt(
                &self.ytdlp,
                target,
                dir.path(),
                quality,
                self.config.max_duration_secs,
                self.config.max_file_bytes(),
            )
            .await
            {
                Ok(value) => {
                    downloaded = Some(value);
                    break;
                }
                Err(error) if fallback_triggers(&error) => {
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }
        let Some(downloaded) = downloaded else {
            return Err(last_error.unwrap_or_else(|| {
                DownloadError::SourceUnavailable(
                    "all configured sources refused the track".to_owned(),
                )
            }));
        };

        // Snapshot honest network speed now: the audio exists on disk and no
        // cover/tag/upload time has been folded in yet.
        if let Ok(metadata) = tokio::fs::metadata(&downloaded.audio_path).await
            && metadata.len() > 0
        {
            self.record_speed_sample(metadata.len(), download_started.elapsed().as_secs().max(1));
        }

        let merged_meta = merge_meta(request.meta.as_ref(), &downloaded.meta);
        if let Some(duration) = merged_meta.duration_secs
            && u64::from(duration) > self.config.max_duration_secs
        {
            return Err(DownloadError::TooLong {
                limit: self.config.max_duration_secs,
                actual: u64::from(duration),
            });
        }
        let covers = match cover_task.as_mut() {
            // HQ may fetch a second image URL and re-encode twice, so allow a
            // little more time than the old single-thumbnail path did.
            Some(guard) => tokio::time::timeout(Duration::from_secs(16), guard.wait())
                .await
                .ok()
                .and_then(|join| join.ok())
                .unwrap_or_default(),
            None => {
                audio::prepare_cover(
                    &self.cover_client,
                    &self.config.ffmpeg_path,
                    merged_meta.thumbnail_url.as_deref(),
                    dir.path(),
                    hq_cover,
                )
                .await
            }
        };

        let audio_path = downloaded.audio_path.clone();
        audio::tag_audio(
            audio_path.clone(),
            merged_meta.clone(),
            covers.embedded.clone(),
        )
        .await;
        drop(_permit);

        Ok(PreparedAudio {
            meta: merged_meta,
            audio_path,
            cover_path: covers.embedded,
            thumb_path: covers.thumbnail,
            _dir: Arc::new(dir),
            _inflight_guard: None,
        })
    }

    /// Download the given tracks and pack them into a single in-memory ZIP.
    /// A dedicated semaphore allows only one archive build at a time (several
    /// concurrent 900 MB buffers OOM the container). Downloads run two at a
    /// time to disk; the output Vec is preallocated to the exact sum of the
    /// entry sizes before any bytes are copied into it.
    pub async fn zip_album(
        &self,
        keys: &[TrackKey],
        quality: u32,
        hq_cover: bool,
    ) -> AppResult<ZipOutcome> {
        if exceeds_track_cap(keys.len()) {
            return Err(DownloadError::SourceUnavailable(format!(
                "too many tracks ({}) to ZIP; at most {} are allowed",
                keys.len(),
                crate::zip::MAX_ZIP_TRACKS
            ))
            .into());
        }

        // Hold for the whole build: two 900 MB archives at once exhausts RAM.
        let _zip_permit = self
            .zip_semaphore
            .acquire()
            .await
            .map_err(|_| DownloadError::Internal("zip semaphore closed".to_owned()))?;

        // Download first, keeping each TempDir alive on disk, so the archive
        // buffer can be reserved once instead of doubling as it grows.
        let mut prepared_tracks: Vec<(String, PreparedAudio)> = Vec::new();
        for (chunk, pair) in keys.chunks(2).enumerate() {
            let base = chunk * 2;
            let first = self.prepare_zip_track(&pair[0], quality, hq_cover);
            let results = match pair.get(1) {
                Some(key) => {
                    let (a, b) =
                        tokio::join!(first, self.prepare_zip_track(key, quality, hq_cover));
                    vec![(base, a), (base + 1, b)]
                }
                None => vec![(base, first.await)],
            };

            for (idx, result) in results {
                match result {
                    Ok(prepared) => {
                        let ext = prepared
                            .audio_path
                            .extension()
                            .and_then(|value| value.to_str())
                            .unwrap_or("m4a");
                        let name = zip_entry_name(idx, &prepared.meta.title, ext);
                        prepared_tracks.push((name, prepared));
                    }
                    Err(error) => {
                        warn!(%error, "album ZIP: could not download track; skipping");
                    }
                }
            }
        }

        let mut capacity: usize = 0;
        for (_, prepared) in &prepared_tracks {
            if let Ok(metadata) = tokio::fs::metadata(&prepared.audio_path).await {
                capacity = capacity.saturating_add(metadata.len() as usize);
            }
        }
        let mut writer =
            ZipWriter::with_capacity(MAX_ZIP_BYTES, capacity.min(MAX_ZIP_BYTES as usize));
        let mut total_bytes: u64 = 0;
        let mut tracks = 0usize;

        for (name, prepared) in prepared_tracks {
            let bytes = match tokio::fs::read(&prepared.audio_path).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    warn!(%error, "album ZIP: could not read downloaded track; skipping");
                    continue;
                }
            };
            // Releasing the PreparedAudio drops its TempDir so the file is gone
            // before the next track's bytes are read into memory.
            drop(prepared);

            let size = bytes.len() as u64;
            if total_bytes.saturating_add(size) > MAX_ZIP_BYTES {
                return Err(DownloadError::SourceUnavailable(format!(
                    "album too large: {} bytes already archived plus {size} more exceeds the {MAX_ZIP_BYTES} byte limit",
                    total_bytes
                ))
                .into());
            }
            writer.add_file(&name, &bytes).await?;
            total_bytes += size;
            tracks += 1;
        }

        if tracks == 0 {
            return Err(DownloadError::SourceUnavailable(
                "no track could be downloaded".to_owned(),
            )
            .into());
        }

        Ok(ZipOutcome {
            bytes: writer.finish().await?,
            tracks,
        })
    }

    /// Resolve the request from the cache entry for this key (as the replay
    /// path does), then download through the same singleflight path used for
    /// normal delivery. Cover processing is untouched; `hq_cover` still forms
    /// part of the singleflight key.
    async fn prepare_zip_track(
        &self,
        key: &TrackKey,
        quality: u32,
        hq_cover: bool,
    ) -> Result<PreparedAudio, DownloadError> {
        let cached = self
            .get_cached(key, quality)
            .await
            .map_err(|error| DownloadError::Internal(error.to_string()))?
            .ok_or(DownloadError::NotFound)?;
        // A cached track knows its real source (SoundCloud/Deezer keys are not
        // on YouTube): go straight to it instead of paying a failing search.
        let target = match &cached.source_url {
            Some(url) => DownloadTarget::Url(url.clone()),
            None => DownloadTarget::Search(cached.meta.search_query()),
        };
        let request = TrackRequest {
            key: cached.key.clone(),
            provider: cached.provider,
            source_url: cached.source_url,
            meta: Some(cached.meta),
            target,
        };
        self.prepare(request, quality, ZIP_USER_ID, hq_cover).await
    }
}

/// Transient failures worth one retry: timeouts, resets, rate limiting and
/// YouTube's intermittent HTTP 403 (rate limiting as often as real refusals;
/// the fallback catalogs still run after a failed retry).
fn is_transient(error: &DownloadError) -> bool {
    match error {
        // Only the classified source-failure text carries HTTP status codes.
        // Match them word-ish so a digit run in a byte count (e.g. "14035456
        // bytes") can't be mistaken for a 403/429.
        DownloadError::SourceUnavailable(message) => {
            let lower = message.to_ascii_lowercase();
            lower.contains("timed out")
                || lower.contains("timeout")
                || lower.contains("connection")
                || lower.contains("reset")
                || lower.contains("too many requests")
                || has_http_status(&lower, 403)
                || has_http_status(&lower, 429)
        }
        // Size/duration/policy failures are final. Their Display text also
        // contains digit runs (sizes, seconds), so never substring-match it.
        _ => false,
    }
}

fn has_http_status(message: &str, code: u16) -> bool {
    message.contains(&format!("http {code}"))
        || message.contains(&format!(" {code} "))
        || message.starts_with(&format!("{code} "))
        || message.ends_with(&format!(" {code}"))
}

fn is_empty_search_query(target: &DownloadTarget) -> bool {
    matches!(
        target,
        DownloadTarget::Search(query) | DownloadTarget::SoundCloudSearch(query)
            if query.trim().is_empty()
    )
}

/// Only "this catalog can't serve it" failures roll on to the next source;
/// duration, size and policy failures are final.
fn fallback_triggers(error: &DownloadError) -> bool {
    matches!(
        error,
        DownloadError::Private
            | DownloadError::RegionLocked
            | DownloadError::NotFound
            | DownloadError::SourceUnavailable(_)
    )
}

/// Ordered candidates for the same track: original target first, then search
/// the open catalogs. No query (bare restricted link without metadata) means
/// there is nothing to search, so the original target is the only attempt.
fn fallback_candidates(request: &TrackRequest) -> Vec<DownloadTarget> {
    let mut candidates = vec![request.target.clone()];
    let Some(query) = fallback_query(request) else {
        return candidates;
    };
    let youtube = DownloadTarget::Search(query.clone());
    let soundcloud = DownloadTarget::SoundCloudSearch(query);
    match &request.target {
        DownloadTarget::Url(_) => {
            candidates.push(youtube);
            candidates.push(soundcloud);
        }
        DownloadTarget::Search(_) => candidates.push(soundcloud),
        DownloadTarget::SoundCloudSearch(_) => candidates.push(youtube),
    }
    candidates
}

/// The query to hunt the track elsewhere with: the search query itself, or —
/// for a URL with resolved metadata — "Artist Title".
fn fallback_query(request: &TrackRequest) -> Option<String> {
    match &request.target {
        DownloadTarget::Search(query) | DownloadTarget::SoundCloudSearch(query) => {
            let query = query.trim();
            (!query.is_empty()).then(|| query.to_owned())
        }
        DownloadTarget::Url(_) => request.meta.as_ref().and_then(|meta| {
            let query = meta.search_query();
            let query = query.trim();
            (!query.is_empty()).then(|| query.to_owned())
        }),
    }
}

async fn download_attempt(
    ytdlp: &providers::ytdlp::YtDlp,
    target: &DownloadTarget,
    dir: &Path,
    quality: u32,
    max_duration_secs: u64,
    max_bytes: u64,
) -> Result<providers::ytdlp::DownloadedAudio, DownloadError> {
    match ytdlp
        .download(target, dir, quality, max_duration_secs, max_bytes)
        .await
    {
        Ok(downloaded) => Ok(downloaded),
        Err(error) if is_transient(&error) => {
            warn!(?target, error = %error, "transient download failure; retrying once");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            ytdlp
                .download(target, dir, quality, max_duration_secs, max_bytes)
                .await
        }
        Err(error) => Err(error),
    }
}

/// Remove leftover files from a failed attempt so the next candidate starts
/// on a clean directory (yt-dlp output template is fixed).
async fn clear_dir(dir: &Path) {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(_) => return,
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.file_type().await.is_ok_and(|kind| kind.is_file()) {
            // The early cover task may have written cover.jpg / cover_*.jpg
            // while the audio attempt was running; deleting them here would
            // race tagging.
            if entry.file_name().to_string_lossy().starts_with("cover") {
                continue;
            }
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

fn merge_meta(requested: Option<&TrackMeta>, downloaded: &TrackMeta) -> TrackMeta {
    let Some(requested) = requested else {
        return downloaded.clone().without_artist_prefix();
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
    .without_artist_prefix()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflight_guard_removes_entry_on_drop_and_panic() {
        let map = Arc::new(Mutex::new(HashMap::<
            InflightKey,
            broadcast::Sender<InflightResult>,
        >::new()));
        let key = (TrackKey::new("youtube:guard"), 192u32, false);
        let (tx, _rx) = broadcast::channel::<InflightResult>(1);
        map.lock().expect("lock").insert(key.clone(), tx);

        {
            let _guard = InflightGuard {
                inflight: Arc::clone(&map),
                key: key.clone(),
                armed: true,
            };
            assert_eq!(map.lock().expect("lock").len(), 1);
        }
        assert!(
            map.lock().expect("lock").is_empty(),
            "guard must clean up on normal exit"
        );

        // Panic path: the guard still runs Drop while the stack unwinds.
        let (tx, _rx) = broadcast::channel::<InflightResult>(1);
        map.lock().expect("lock").insert(key.clone(), tx);
        let map_for_panic = Arc::clone(&map);
        let key_for_panic = key.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = InflightGuard {
                inflight: map_for_panic,
                key: key_for_panic,
                armed: true,
            };
            panic!("simulated leader panic");
        }));
        assert!(result.is_err());
        assert!(
            map.lock().expect("lock").is_empty(),
            "guard must clean up on panic too"
        );
    }

    #[test]
    fn inflight_key_includes_hq_cover() {
        let key = TrackKey::new("youtube:key");
        assert_ne!((key.clone(), 320u32, false), (key, 320u32, true));
    }

    #[test]
    fn disarmed_inflight_guard_keeps_entry_for_consumers() {
        let map = Arc::new(Mutex::new(HashMap::<
            InflightKey,
            broadcast::Sender<InflightResult>,
        >::new()));
        let key = (TrackKey::new("youtube:handoff"), 320u32, true);
        let (tx, _rx) = broadcast::channel::<InflightResult>(1);
        map.lock().expect("lock").insert(key.clone(), tx);

        {
            // The leader's guard is disarmed on success: no eviction here.
            let _guard = InflightGuard {
                inflight: Arc::clone(&map),
                key: key.clone(),
                armed: false,
            };
        }
        assert_eq!(map.lock().expect("lock").len(), 1);

        // The shared guard in PreparedAudio outlives the leader and any single
        // waiter; the entry is removed only after the last consumer drops it.
        let shared = Arc::new(InflightGuard {
            inflight: Arc::clone(&map),
            key: key.clone(),
            armed: true,
        });
        let waiter_copy = Arc::clone(&shared);
        drop(shared);
        assert_eq!(map.lock().expect("lock").len(), 1);
        drop(waiter_copy);
        assert!(map.lock().expect("lock").is_empty());
    }

    #[test]
    fn is_transient_uses_typed_variants_not_substrings() {
        // A byte count / duration containing "403" must never be transient.
        assert!(!is_transient(&DownloadError::TooLarge {
            limit: 403,
            actual: 14_035_456,
        }));
        assert!(!is_transient(&DownloadError::TooLong {
            limit: 10_000,
            actual: 10_403,
        }));
        // Classified source text with a real status code / timeout is transient.
        assert!(is_transient(&DownloadError::SourceUnavailable(
            "HTTP 429 Too Many Requests".to_owned()
        )));
        assert!(is_transient(&DownloadError::SourceUnavailable(
            "yt-dlp timed out after 30s".to_owned()
        )));
        assert!(is_transient(&DownloadError::SourceUnavailable(
            "ERROR: HTTP 403 Forbidden".to_owned()
        )));
        // Digit runs inside a source message are not status codes.
        assert!(!is_transient(&DownloadError::SourceUnavailable(
            "server returned 14035456 bytes".to_owned()
        )));
    }

    #[tokio::test]
    async fn abort_on_drop_aborts_cover_task() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Flag(Arc<AtomicBool>);
        impl Drop for Flag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let observed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&observed);
        let guard = AbortOnDrop(Some(tokio::spawn(async move {
            let _flag = Flag(flag);
            std::future::pending::<()>().await;
        })));
        tokio::task::yield_now().await;
        drop(guard);
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(
            observed.load(Ordering::SeqCst),
            "dropping the guard must abort the task"
        );
    }

    #[test]
    fn fallback_chain_starts_with_original_then_open_catalogs() {
        let request = TrackRequest {
            key: TrackKey::new("youtube:locked"),
            provider: Provider::YouTube,
            source_url: Some("https://youtu.be/locked".into()),
            meta: Some(TrackMeta {
                title: "Song".into(),
                performer: Some("Artist".into()),
                ..Default::default()
            }),
            target: DownloadTarget::Url("https://youtu.be/locked".into()),
        };
        assert_eq!(
            fallback_candidates(&request),
            vec![
                DownloadTarget::Url("https://youtu.be/locked".into()),
                DownloadTarget::Search("Artist Song".into()),
                DownloadTarget::SoundCloudSearch("Artist Song".into()),
            ]
        );

        let soundcloud = TrackRequest {
            target: DownloadTarget::SoundCloudSearch("Artist Song".into()),
            ..request.clone()
        };
        assert_eq!(
            fallback_candidates(&soundcloud),
            vec![
                DownloadTarget::SoundCloudSearch("Artist Song".into()),
                DownloadTarget::Search("Artist Song".into()),
            ]
        );

        // No metadata and a bare URL: nothing to search for, no fallback.
        let bare = TrackRequest {
            meta: None,
            ..request
        };
        assert_eq!(
            fallback_candidates(&bare),
            vec![DownloadTarget::Url("https://youtu.be/locked".into())]
        );
    }

    #[test]
    fn fallback_triggers_only_on_catalog_refusals() {
        assert!(fallback_triggers(&DownloadError::Private));
        assert!(fallback_triggers(&DownloadError::RegionLocked));
        assert!(fallback_triggers(&DownloadError::NotFound));
        assert!(fallback_triggers(&DownloadError::SourceUnavailable(
            "x".into()
        )));
        assert!(!fallback_triggers(&DownloadError::TooLong {
            limit: 100,
            actual: 101,
        }));
        assert!(!fallback_triggers(&DownloadError::TooLarge {
            limit: 100,
            actual: 101,
        }));
    }

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

    #[test]
    fn speed_ring_mean_and_empty() {
        let mut samples = VecDeque::new();
        assert_eq!(mean_bps(&samples), None);
        samples.push_back((1_000, 2));
        samples.push_back((3_000, 2));
        // 4000 bytes over 4s = 1000 B/s (mean over totals, not per-sample avg).
        assert_eq!(mean_bps(&samples), Some(1_000.0));
    }

    #[test]
    fn estimate_formula_matches_kbps_math_and_floors_at_one() {
        // 200s at 192 kbps = 200*192*125 = 4_800_000 bytes; / 1_000_000 B/s = 4.8s.
        let secs = download_secs_for(200, 192, 1_000_000.0);
        assert!((secs - 4.8).abs() < 1e-9, "got {secs}");
        // Tiny file at huge speed still reports at least one second.
        assert_eq!(download_secs_for(1, 8, 1e12), 1.0);
    }
}
