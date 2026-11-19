//! Persistent download jobs backed by Postgres.
//!
//! A job is enqueued and claimed in one transaction, so only one replica ever
//! downloads a given request. Cancellation is cooperative: the delivery path
//! checks the flag between steps and finishes the ledger row on every exit.

use crate::state::AppState;
use common::error::{AppError, AppResult};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use storage::Db;

pub struct JobGuard {
    db: Db,
    pub id: String,
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
            active: self.active.clone(),
            owner: false,
        }
    }
}

impl JobGuard {
    /// Enqueue and claim a job. `None` means another replica already owns it.
    pub async fn start(
        state: &AppState,
        user_id: i64,
        kind: &str,
        payload: Value,
    ) -> AppResult<Option<Self>> {
        let id = state.new_selection_id();
        let claimed = state
            .db
            .enqueue_and_claim_job(&id, user_id, kind, payload, &state.worker_id)
            .await?;
        Ok(claimed.then_some(Self {
            db: state.db.clone(),
            id,
            active: Arc::new(AtomicBool::new(true)),
            owner: true,
        }))
    }

    pub async fn cancelled(&self) -> bool {
        self.db
            .job_cancel_requested(&self.id)
            .await
            .unwrap_or(false)
    }

    pub async fn finish(&self, status: &str, error: Option<&str>) {
        self.active.store(false, Ordering::Release);
        if let Err(error) = self.db.finish_job(&self.id, status, error).await {
            tracing::warn!(%error, job_id = %self.id, "could not finish job");
        }
    }

    pub fn cancelled_error() -> AppError {
        AppError::Message("download cancelled by user".to_owned())
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        // A `?`-fail after start() would leave the row "downloading" until
        // restart. Best-effort correction from the owner only, when a runtime
        // is alive; the storage agent's guarded finish_job makes mislabels
        // from a concurrent normal finish self-correcting.
        if self.owner
            && self.active.swap(false, Ordering::AcqRel)
            && let Ok(handle) = tokio::runtime::Handle::try_current()
        {
            let db = self.db.clone();
            let id = self.id.clone();
            handle.spawn(async move {
                if let Err(error) = db.finish_job(&id, "failed", None).await {
                    tracing::warn!(%error, job_id = %id, "could not mark abandoned job failed");
                }
            });
        }
    }
}
