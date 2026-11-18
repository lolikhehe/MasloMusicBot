//! Persistent download jobs backed by Postgres.
//!
//! A job is enqueued and claimed in one transaction, so only one replica ever
//! downloads a given request. Cancellation is cooperative: the delivery path
//! checks the flag between steps and finishes the ledger row on every exit.

use crate::state::AppState;
use common::error::{AppError, AppResult};
use serde_json::Value;
use storage::Db;

#[derive(Clone)]
pub struct JobGuard {
    db: Db,
    pub id: String,
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
        }))
    }

    pub async fn cancelled(&self) -> bool {
        self.db
            .job_cancel_requested(&self.id)
            .await
            .unwrap_or(false)
    }

    pub async fn finish(&self, status: &str, error: Option<&str>) {
        if let Err(error) = self.db.finish_job(&self.id, status, error).await {
            tracing::warn!(%error, job_id = %self.id, "could not finish job");
        }
    }

    pub fn cancelled_error() -> AppError {
        AppError::Message("download cancelled by user".to_owned())
    }
}
