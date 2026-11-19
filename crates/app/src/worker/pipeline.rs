//! Execution of a claimed job. The payload contract is owned by the bot-side
//! enqueue path; this module only decodes it and drives the *shared* delivery
//! functions in `telegram::bot::handlers`.

use crate::WorkerState;
use crate::guard::JobGuard;
use common::domain::{Album, AlbumRef, Provider, TrackRequest};
use common::error::{AppError, DownloadError};
use common::i18n;
use downloader::MAX_ZIP_TRACKS;
use serde::Deserialize;
use std::fmt::Display;
use std::time::Duration;
use storage::Db;
use storage::db::ClaimedJob;
use telegram::bot::handlers::{
    self, deliver_album, deliver_track_inner, edit_html, escape_html, spawn_download_countdown,
};
use telegram::bot::send;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, InputFile, MessageId, ParseMode,
};
use tokio::time::MissedTickBehavior;
use tracing::warn;

pub enum JobSpec {
    Track {
        request: TrackRequest,
        chat_id: i64,
    },
    Album {
        album: Album,
        chat_id: i64,
        card_id: i32,
    },
    Zip {
        source: String,
        id: String,
        chat_id: i64,
        card_id: i32,
    },
}

#[derive(Deserialize)]
struct TrackPayload {
    chat_id: i64,
    request: TrackRequest,
}

#[derive(Deserialize)]
struct AlbumPayload {
    chat_id: i64,
    card_id: i32,
    album: Album,
}

#[derive(Deserialize)]
struct ZipPayload {
    chat_id: i64,
    card_id: i32,
    source: String,
    id: String,
}

/// Decode a claimed row into a typed job. Extra keys (`key`, `title`) in the
/// payload are deliberately ignored: the ledger row itself is the dedup key.
pub fn decode_claimed(job: &ClaimedJob) -> anyhow::Result<JobSpec> {
    let payload = job.payload.clone();
    match job.kind.as_str() {
        "track" => {
            let parsed: TrackPayload = serde_json::from_value(payload)?;
            Ok(JobSpec::Track {
                request: parsed.request,
                chat_id: parsed.chat_id,
            })
        }
        "album" => {
            let parsed: AlbumPayload = serde_json::from_value(payload)?;
            Ok(JobSpec::Album {
                album: parsed.album,
                chat_id: parsed.chat_id,
                card_id: parsed.card_id,
            })
        }
        "zip" => {
            let parsed: ZipPayload = serde_json::from_value(payload)?;
            Ok(JobSpec::Zip {
                source: parsed.source,
                id: parsed.id,
                chat_id: parsed.chat_id,
                card_id: parsed.card_id,
            })
        }
        other => anyhow::bail!("unknown job kind: {other}"),
    }
}

/// yt-dlp stderr can embed enclosure URLs carrying tokens; never persist or log
/// them verbatim. Keep only the first line, redact URLs, cap at 200 chars.
fn short_error(e: &impl Display) -> String {
    let raw = e.to_string();
    let first_line = raw.lines().next().unwrap_or("");
    let mut safe = String::with_capacity(first_line.len());
    let mut rest = first_line;
    while let Some(pos) = ["http://", "https://"]
        .iter()
        .filter_map(|scheme| rest.find(scheme))
        .min()
    {
        safe.push_str(&rest[..pos]);
        let token = &rest[pos..];
        let end = token.find(char::is_whitespace).unwrap_or(token.len());
        safe.push_str("[url]");
        rest = &token[end..];
    }
    safe.push_str(rest);
    if safe.chars().count() > 200 {
        safe = safe.chars().take(200).collect();
    }
    safe
}

/// Run one claimed job. The guard's `Drop` marks an unwind as failed, so a
/// panic anywhere below still settles the ledger row.
pub async fn execute(ws: WorkerState, job: ClaimedJob) -> anyhow::Result<()> {
    // Guard first so even a decode error settles the ledger row via Drop.
    let guard = JobGuard::from_claimed(&job, ws.app.db.clone(), ws.app.worker_id.clone());
    // Long jobs (e.g. a 25-track album) outlive the 900s lease; touch it every
    // 60s so a process restart cannot requeue a row we are still delivering.
    let hb_guard = guard.clone();
    let hb = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            hb_guard.heartbeat("downloading").await;
        }
    });
    let result = async {
        let spec = decode_claimed(&job)?;
        match spec {
            JobSpec::Track { request, chat_id } => {
                execute_track(&ws, &guard, request, chat_id).await
            }
            JobSpec::Album {
                album,
                chat_id,
                card_id,
            } => execute_album(&ws, &guard, album, chat_id, card_id).await,
            JobSpec::Zip {
                source,
                id,
                chat_id,
                card_id,
            } => execute_zip(&ws, &guard, source, id, chat_id, card_id).await,
        }
    }
    .await;
    hb.abort();
    result
}

/// Best-effort speed sample publication: the manager's rolling mean is global,
/// so one sample per completed prepare is enough for the dashboard.
async fn publish_speed(ws: &WorkerState, db: &Db) {
    if let Some(bps) = ws.app.manager.download_speed_bps()
        && let Err(error) = db.publish_download_speed(bps).await
    {
        warn!(%error, "could not publish download speed");
    }
}

async fn execute_track(
    ws: &WorkerState,
    guard: &JobGuard,
    request: TrackRequest,
    chat_id: i64,
) -> anyhow::Result<()> {
    let db = &ws.app.db;
    let Some(user) = db.user_by_id(guard.user_id).await? else {
        guard.finish("failed", Some("user not found")).await;
        return Ok(());
    };
    let lang = user.lang;
    let chat = ChatId(chat_id);
    let status = send::send_html(&ws.bot, chat, i18n::downloading(lang)).await?;

    let outcome = deliver_track_inner(
        &ws.app,
        &ws.bot,
        chat,
        request,
        user.quality,
        guard.user_id,
        user.tags_enabled,
        user.hq_cover,
        user.genre_analysis,
        Some((guard.id.clone(), db.clone())),
    )
    .await;

    match &outcome {
        Ok(_) => {
            publish_speed(ws, db).await;
            guard.finish("done", None).await;
            let _ = ws.bot.delete_message(chat, status.id).await;
        }
        Err(error) => {
            if guard.cancelled().await {
                guard.finish("cancelled", None).await;
                let _ = edit_html(&ws.bot, chat, status.id, i18n::job_cancelled(lang)).await;
            } else {
                let safe_error = short_error(error);
                guard.finish("failed", Some(&safe_error)).await;
                let text = handlers::error_text(lang, error);
                let _ = edit_html(&ws.bot, chat, status.id, text).await;
                warn!(error = %safe_error, job_id = %guard.id, "worker track delivery failed");
            }
        }
    }
    Ok(())
}

async fn execute_album(
    ws: &WorkerState,
    guard: &JobGuard,
    album: Album,
    chat_id: i64,
    card_id: i32,
) -> anyhow::Result<()> {
    let db = &ws.app.db;
    let Some(user) = db.user_by_id(guard.user_id).await? else {
        guard.finish("failed", Some("user not found")).await;
        return Ok(());
    };
    let lang = user.lang;
    let chat = ChatId(chat_id);
    let status = send::send_html(
        &ws.bot,
        chat,
        i18n::album_started(lang, &album.title, album.tracks.len()),
    )
    .await?;
    // Stop the source card from being a second tap target (the row is the dedup).
    strip_card_buttons(&ws.bot, chat, card_id).await;

    let result = deliver_album(
        &ws.app,
        &ws.bot,
        chat,
        status.id,
        lang,
        user.quality,
        guard.user_id,
        album,
        user.tags_enabled,
        user.hq_cover,
        user.genre_analysis,
        user.album_info,
        Some((guard.id.clone(), db.clone())),
    )
    .await;

    // Cancellation always wins over the delivery result (mirrors the bot path).
    if guard.cancelled().await {
        guard.finish("cancelled", None).await;
        let _ = edit_html(&ws.bot, chat, status.id, i18n::job_cancelled(lang)).await;
    } else {
        match &result {
            Ok(delivered) if *delivered > 0 => {
                publish_speed(ws, db).await;
                guard.finish("done", None).await;
            }
            Ok(_) => guard.finish("failed", None).await,
            Err(error) => {
                let safe_error = short_error(error);
                guard.finish("failed", Some(&safe_error)).await;
                let text = handlers::error_text(lang, error);
                let _ = edit_html(&ws.bot, chat, status.id, text).await;
                warn!(error = %safe_error, job_id = %guard.id, "worker album delivery failed");
            }
        }
    }
    let _ = ws.bot.delete_message(chat, status.id).await;
    Ok(())
}

/// Panic-safe bookkeeping for a ZIP job: on unwind the ledger row is settled
/// and the temporary status message removed. Both are best-effort and
/// idempotent, so overlapping with the normal tail cleanup is harmless.
struct ZipCleanupGuard {
    db: Db,
    bot: Bot,
    chat: ChatId,
    status_id: MessageId,
    job_id: String,
    worker_id: String,
}

impl Drop for ZipCleanupGuard {
    fn drop(&mut self) {
        let db = self.db.clone();
        let bot = self.bot.clone();
        let chat = self.chat;
        let status_id = self.status_id;
        let job_id = self.job_id.clone();
        let worker_id = self.worker_id.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = db.finish_job(&job_id, "failed", None, &worker_id).await;
                let _ = bot.delete_message(chat, status_id).await;
            });
        }
    }
}

async fn strip_card_buttons(bot: &Bot, chat: ChatId, card_id: i32) {
    let _ = bot
        .edit_message_reply_markup(chat, MessageId(card_id))
        .reply_markup(InlineKeyboardMarkup::new(
            Vec::<Vec<InlineKeyboardButton>>::new(),
        ))
        .await;
}

async fn execute_zip(
    ws: &WorkerState,
    guard: &JobGuard,
    source: String,
    id: String,
    chat_id: i64,
    card_id: i32,
) -> anyhow::Result<()> {
    let db = &ws.app.db;
    let Some(user) = db.user_by_id(guard.user_id).await? else {
        guard.finish("failed", Some("user not found")).await;
        return Ok(());
    };
    let lang = user.lang;
    let quality = user.quality;
    let hq_cover = user.hq_cover;
    let chat = ChatId(chat_id);
    strip_card_buttons(&ws.bot, chat, card_id).await;

    let status = match send::send_html(&ws.bot, chat, i18n::album_packing(lang)).await {
        Ok(message) => message,
        Err(error) => {
            warn!(%error, job_id = %guard.id, "could not create ZIP status message");
            guard.finish("failed", Some(&error.to_string())).await;
            return Ok(());
        }
    };
    let status_id = status.id;
    let _cleanup = ZipCleanupGuard {
        db: db.clone(),
        bot: ws.bot.clone(),
        chat,
        status_id,
        job_id: guard.id.clone(),
        worker_id: ws.app.worker_id.clone(),
    };

    // Inner work returns error text to report (`None` on success). All exit
    // paths converge on the tail cleanup below.
    let failure: Option<String> = async {
        // Resolve first, bounded: the ETA must reflect the real track count.
        let resolved = tokio::time::timeout(Duration::from_secs(20), async {
            let provider: Provider = source
                .parse()
                .map_err(|_| DownloadError::Unsupported("unknown album source".to_owned()))?;
            let album = ws
                .app
                .manager
                .resolve_album(&AlbumRef {
                    provider,
                    url: Some(id.clone()),
                    title: None,
                })
                .await?;
            let all_keys: Vec<_> = album.tracks.iter().map(|track| track.key.clone()).collect();
            // `zip_album` rejects >MAX_ZIP_TRACKS keys; pack the first ones.
            let selected = all_keys.len().min(MAX_ZIP_TRACKS);
            let keys: Vec<_> = all_keys[..selected].to_vec();
            let durations: Vec<u32> = album.tracks[..selected]
                .iter()
                .filter_map(|track| track.meta.as_ref()?.duration_secs)
                .collect();
            let mut cached = 0usize;
            for key in &keys {
                if ws.app.manager.get_cached(key, quality).await?.is_some() {
                    cached += 1;
                }
            }
            Ok::<_, AppError>((album.title, keys, cached, durations))
        })
        .await;
        let (title, keys, cached, durations) = match resolved {
            Ok(Ok(resolved)) => resolved,
            Ok(Err(error)) => {
                return Some(i18n::error(
                    lang,
                    &DownloadError::Internal(error.to_string()),
                ));
            }
            Err(_) => {
                return Some(i18n::error(
                    lang,
                    &DownloadError::Internal("album resolve timed out".to_owned()),
                ));
            }
        };
        if cached == 0 {
            return Some(i18n::album_pack_no_tracks(lang).to_owned());
        }
        let base = i18n::album_packing(lang).to_owned();
        let estimate = if durations.is_empty() {
            None
        } else {
            let total: f64 = durations
                .iter()
                .filter_map(|&duration| ws.app.manager.estimate_download_secs(duration, quality))
                .sum();
            (total > 0.0).then(|| (total / 2.0).max(1.0))
        };
        let countdown = match estimate {
            Some(estimate) => {
                let eta = i18n::download_eta(lang, estimate);
                let _ = edit_html(&ws.bot, chat, status_id, format!("{base} {eta}")).await;
                Some(spawn_download_countdown(
                    &ws.bot,
                    chat,
                    status_id,
                    base,
                    (estimate.ceil() as u64).max(1),
                    lang,
                    true,
                ))
            }
            None => {
                let _ = edit_html(&ws.bot, chat, status_id, base).await;
                None
            }
        };
        let result = tokio::time::timeout(
            Duration::from_secs(10 * 60),
            ws.app.manager.zip_album(&keys, quality, hq_cover),
        )
        .await;
        if let Some(handle) = &countdown {
            handle.abort();
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        match result {
            Ok(Ok(outcome)) => {
                let safe: String = title
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || "-_. ".contains(c) {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let filename = format!("{}.zip", handlers::truncate(&safe, 80));
                let caption = i18n::albums_item(lang, &escape_html(&title), outcome.tracks as u32);
                let input = InputFile::memory(outcome.bytes).file_name(filename);
                match ws
                    .bot
                    .send_document(chat, input)
                    .caption(caption)
                    .parse_mode(ParseMode::Html)
                    .await
                {
                    Ok(message) => {
                        ws.app.maybe_track_deletion(guard.user_id, chat, message.id);
                    }
                    Err(error) => {
                        warn!(%error, job_id = %guard.id, "album ZIP upload failed");
                        return Some(i18n::error(
                            lang,
                            &DownloadError::Internal(error.to_string()),
                        ));
                    }
                }
                None
            }
            Ok(Err(error)) => {
                let lower = error.to_string().to_ascii_lowercase();
                Some(if lower.contains("too large") {
                    i18n::album_pack_too_large(lang, 900)
                } else if lower.contains("no track could be downloaded") {
                    i18n::album_pack_no_tracks(lang).to_owned()
                } else {
                    i18n::error(lang, &DownloadError::Internal(error.to_string()))
                })
            }
            Err(_) => Some(i18n::error(
                lang,
                &DownloadError::Internal("ZIP timed out".to_owned()),
            )),
        }
    }
    .await;

    let _ = ws.bot.delete_message(chat, status_id).await;
    if guard.cancelled().await {
        guard.finish("cancelled", None).await;
    } else if failure.is_none() {
        publish_speed(ws, db).await;
        guard.finish("done", None).await;
    } else {
        guard.finish("failed", None).await;
        if let Some(text) = failure {
            let _ = send::send_html(&ws.bot, chat, text).await;
        }
    }
    Ok(())
}
