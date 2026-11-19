//! Cold-path orchestration: metadata resolution, in-flight deduplication,
//! bounded downloads, cover/tag preparation and cache recording.

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
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
    command.arg(&target);

    match command.output().await {
        Ok(output) if output.status.success() && target.exists() => {
            prepared.audio_path = target;
            metrics::counter!("music_bot_audio_postprocessed_total").increment(1);
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(error = %stderr.trim(), "ffmpeg post-processing failed; keeping original audio");
        }
        Err(error) => {
            warn!(%error, "could not spawn ffmpeg; keeping original audio");
        }
    }
}

type InflightResult = Result<PreparedAudio, Arc<DownloadError>>;

/// Removes this `(track, quality)` entry from the in-flight map on **every**
/// exit path, including a panic in the leader. Without the guard a panicked
/// leader would leave waiters blocked forever on a broadcast nobody sends to.
struct InflightGuard<'a> {
    inflight: &'a Mutex<HashMap<(TrackKey, u32), broadcast::Sender<InflightResult>>>,
    key: (TrackKey, u32),
}

impl Drop for InflightGuard<'_> {
    fn drop(&mut self) {
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
    inflight: Mutex<HashMap<(TrackKey, u32), broadcast::Sender<InflightResult>>>,
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
            config,
            db,
            ytdlp,
            external,
            cover_client,
            inflight: Mutex::new(HashMap::new()),
            user_semaphores,
            search_cache,
            album_search_cache,
        }
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
        // L2: Postgres-backed search cache survives restarts (10 min TTL).
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
        if let Some(cached) = self.album_search_cache.get(&normalized).await {
            return Ok(cached);
        }
        let limit = limit.clamp(1, 20);
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
        self.album_search_cache
            .insert(normalized, out.clone())
            .await;
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
            let mut inflight = self
                .inflight
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
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
            // A leader that hangs (or whose process dies mid-download) must not
            // block its waiters forever. Bound the wait by the same timeout the
            // download itself has, plus a small grace margin.
            let wait_timeout = self.config.download_timeout + Duration::from_secs(10);
            loop {
                match tokio::time::timeout(wait_timeout, rx.recv()).await {
                    Ok(Ok(Ok(prepared))) => return Ok(prepared),
                    Ok(Ok(Err(error))) => return Err((*error).clone()),
                    Ok(Err(broadcast::error::RecvError::Closed)) => {
                        // The leader disappeared without publishing. Try again.
                        return Box::pin(self.prepare(request, quality, user_id, hq_cover)).await;
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
        // Guard must outlive the send so waiters receive the result before the
        // entry is removed; dropping at function end does exactly that.
        let _guard = InflightGuard {
            inflight: &self.inflight,
            key: cache_key.clone(),
        };
        let result = self.prepare_leader(request, quality, hq_cover).await;
        let broadcast_result = result.clone().map_err(|error| Arc::new(error.clone()));
        let _ = sender.send(broadcast_result);
        result
    }

    async fn prepare_leader(
        &self,
        request: TrackRequest,
        quality: u32,
        hq_cover: bool,
    ) -> Result<PreparedAudio, DownloadError> {
        metrics::gauge!("music_bot_downloads_inflight").increment(1.0);
        let result = self.prepare_leader_inner(request, quality, hq_cover).await;
        metrics::gauge!("music_bot_downloads_inflight").decrement(1.0);
        result
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

        // Overlap the cover download+resize with the audio download when the
        // thumbnail URL is already known (search picks always carry one).
        let cover_task = request
            .meta
            .as_ref()
            .and_then(|meta| meta.thumbnail_url.clone())
            .map(|url| {
                let client = self.cover_client.clone();
                let ffmpeg = self.config.ffmpeg_path.clone();
                let dir_path = dir.path().to_owned();
                tokio::spawn(async move {
                    audio::prepare_cover(&client, &ffmpeg, Some(&url), &dir_path, hq_cover).await
                })
            });

        info!(key = %request.key, target = ?request.target, "starting download");
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

        let merged_meta = merge_meta(request.meta.as_ref(), &downloaded.meta);
        if let Some(duration) = merged_meta.duration_secs
            && u64::from(duration) > self.config.max_duration_secs
        {
            return Err(DownloadError::TooLong {
                limit: self.config.max_duration_secs,
                actual: u64::from(duration),
            });
        }
        let covers = match cover_task {
            // HQ may fetch a second image URL and re-encode twice, so allow a
            // little more time than the old single-thumbnail path did.
            Some(task) => tokio::time::timeout(std::time::Duration::from_secs(16), task)
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
        })
    }
}

/// Transient failures worth one retry: timeouts, resets, rate limiting and
/// YouTube's intermittent HTTP 403 (rate limiting as often as real refusals;
/// the fallback catalogs still run after a failed retry).
fn is_transient(error: &DownloadError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("timed out")
        || message.contains("timeout")
        || message.contains("connection")
        || message.contains("reset")
        || message.contains("too many requests")
        || message.contains("429")
        || message.contains("403")
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
        let map: Mutex<HashMap<(TrackKey, u32), broadcast::Sender<InflightResult>>> =
            Mutex::new(HashMap::new());
        let key = (TrackKey::new("youtube:guard"), 192u32);
        let (tx, _rx) = broadcast::channel::<InflightResult>(1);
        map.lock().expect("lock").insert(key.clone(), tx);

        {
            let _guard = InflightGuard {
                inflight: &map,
                key: key.clone(),
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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = InflightGuard {
                inflight: &map,
                key: key.clone(),
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
}
