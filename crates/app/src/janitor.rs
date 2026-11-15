//! Background cleanup of orphaned download temp directories.
//!
//! Active downloads live in `TMP_DIR/track-<uuid>` and remove themselves when
//! their `TempDir` drops. Crashed runs leave those behind, so on a fixed
//! interval we sweep any directory with that prefix far older than any live
//! download could possibly be. User files elsewhere in the directory are never
//! touched.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const SWEEP_INTERVAL: Duration = Duration::from_secs(10 * 60);
/// Any download older than this is certainly not in progress.
const MAX_AGE: Duration = Duration::from_secs(30 * 60);
const DIR_PREFIX: &str = "track-";

/// Spawn the janitor as a detached task; the process can stop at any time.
pub fn spawn(tmp_dir: PathBuf) {
    tokio::spawn(async move {
        loop {
            sweep(&tmp_dir).await;
            tokio::time::sleep(SWEEP_INTERVAL).await;
        }
    });
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
        if !name.to_string_lossy().starts_with(DIR_PREFIX) {
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
