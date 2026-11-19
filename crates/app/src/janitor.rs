//! Background cleanup of orphaned temp directories.
//!
//! Active downloads live in `TMP_DIR/track-<uuid>` and remove themselves when
//! their `TempDir` drops. Crashed runs leave those behind, so on a fixed
//! interval we sweep any directory with that prefix far older than any live
//! download could possibly be. yt-dlp (a PyInstaller onefile binary) also
//! unpacks itself into `TMP_DIR/_MEIxxxx` on every run; killed or timed-out
//! runs leave those behind too, so they are swept by the same rule.
//! User files elsewhere in the directory are never touched.
//! The same tick also prunes expired coordination rows and the old job ledger
//! from Postgres; a failure there is logged and never stops the loop.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use storage::Db;

const SWEEP_INTERVAL: Duration = Duration::from_secs(10 * 60);
/// Any download older than this is certainly not in progress.
const MAX_AGE: Duration = Duration::from_secs(30 * 60);
/// Finished jobs older than this are dropped from the ledger.
const JOB_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// yt-dlp unpacks here; a live download keeps its dir younger than MAX_AGE.
const DIR_PREFIXES: &[&str] = &["track-", "_MEI"];

/// True when a temp entry has a prefix owned by the download pipeline and may
/// be swept once old enough. Anything else (user files, caches, deno dir) is
/// left alone.
fn is_orphan_prefix(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    DIR_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// Spawn the janitor as a detached task; the process can stop at any time.
/// Takes the app's own `Db` so no second `Db::connect` (and therefore no
/// second `migrate()`) runs mid-boot.
pub fn spawn(db: Db, tmp_dir: PathBuf) {
    tokio::spawn(async move {
        let db = db;
        loop {
            sweep(&tmp_dir).await;
            cleanup_db(&db).await;
            tokio::time::sleep(SWEEP_INTERVAL).await;
        }
    });
}

async fn cleanup_db(db: &Db) {
    if let Err(error) = db.delete_expired_selections().await {
        tracing::warn!(%error, "janitor: failed to delete expired selections");
    }
    if let Err(error) = db.delete_expired_search_cache().await {
        tracing::warn!(%error, "janitor: failed to delete expired search cache");
    }
    if let Err(error) = db
        .delete_old_finished_jobs(JOB_RETENTION.as_secs() as i64)
        .await
    {
        tracing::warn!(%error, "janitor: failed to delete old finished jobs");
    }
}

async fn sweep(tmp_dir: &Path) {
    let cutoff = SystemTime::now() - MAX_AGE;
    let mut entries = match tokio::fs::read_dir(tmp_dir).await {
        Ok(entries) => entries,
        Err(error) => {
            tracing::warn!(dir = %tmp_dir.display(), %error, "could not read temp dir for janitor sweep");
            return;
        }
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let name = entry.file_name();
        if !is_orphan_prefix(&name) {
            continue;
        }
        match entry.file_type().await {
            Ok(kind) if kind.is_dir() => {}
            _ => continue,
        }
        match entry.metadata().await.and_then(|meta| meta.modified()) {
            Ok(modified) if modified < cutoff => {
                tracing::info!(path = %path.display(), "removing orphaned temp directory");
                if let Err(error) = tokio::fs::remove_dir_all(&path).await {
                    tracing::warn!(path = %path.display(), %error, "could not remove orphaned temp directory");
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_orphan_prefix;
    use std::ffi::OsStr;

    #[test]
    fn sweeps_download_and_yt_dlp_dirs() {
        assert!(is_orphan_prefix(OsStr::new("track-9f2c-4b1a")));
        assert!(is_orphan_prefix(OsStr::new("_MEIulfLjs")));
    }

    #[test]
    fn leaves_user_and_cache_entries_alone() {
        for name in [
            "cache",
            "deno",
            "audio",
            "probe.sh",
            "bot.db",
            "flac-check.flac",
        ] {
            assert!(
                !is_orphan_prefix(OsStr::new(name)),
                "should not sweep {name}"
            );
        }
    }
}
