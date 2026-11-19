//! Worker-side job ledger guard. Ported from `telegram::bot::jobs::JobGuard`:
//! the worker receives an already-claimed job instead of enqueuing one, but
//! keeps the same owner-only `Drop` behaviour (an abandoned job is marked
//! failed from the current runtime).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use storage::Db;
use storage::db::ClaimedJob;

pub struct JobGuard {
    db: Db,
    pub id: String,
    pub user_id: i64,
    worker_id: String,
    active: Arc<AtomicBool>,
    /// Only the original guard reports an abandoned job on drop; clones live
    /// inside in-flight track tasks and must not finish the shared ledger row.
    owner: bool,
}

impl Clone for JobGuard {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            id: self.id.clone(),
            user_id: self.user_id,
            worker_id: self.worker_id.clone(),
            active: self.active.clone(),
            owner: false,
        }
    }
}

impl JobGuard {
    /// Wrap a job that `claim_next_job` already handed to this worker.
    pub fn from_claimed(job: &ClaimedJob, db: Db, worker_id: String) -> Self {
        Self {
            db,
            id: job.id.clone(),
            user_id: job.user_id,
            worker_id,
            active: Arc::new(AtomicBool::new(true)),
            owner: true,
        }
    }

    pub async fn cancelled(&self) -> bool {
        self.db
            .job_cancel_requested(&self.id)
            .await
            .unwrap_or(false)
    }

    pub async fn finish(&self, status: &str, error: Option<&str>) {
        self.active.store(false, Ordering::Release);
        if let Err(error) = self
            .db
            .finish_job(&self.id, status, error, &self.worker_id)
            .await
        {
            tracing::warn!(%error, job_id = %self.id, "could not finish job");
        }
    }

    /// Refresh the lease and move the row to a new phase (`uploading`), without
    /// changing the eventual result.
    pub async fn heartbeat(&self, status: &str) {
        if let Err(error) = self.db.touch_job(&self.id, status, &self.worker_id).await {
            tracing::warn!(%error, job_id = %self.id, "could not heartbeat job");
        }
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        // A `?`-fail would leave the row "downloading" until the lease requeue.
        // Best-effort correction from the owner only, when a runtime is alive.
        if self.owner
            && self.active.swap(false, Ordering::AcqRel)
            && let Ok(handle) = tokio::runtime::Handle::try_current()
        {
            let db = self.db.clone();
            let id = self.id.clone();
            let worker_id = self.worker_id.clone();
            handle.spawn(async move {
                if let Err(error) = db.finish_job(&id, "failed", None, &worker_id).await {
                    tracing::warn!(%error, job_id = %id, "could not mark abandoned job failed");
                }
            });
        }
    }
}
