//! Error types. Download errors are deliberately cloneable because one failed
//! cold request is broadcast to every waiter subscribed to the same track.

use crate::domain::Provider;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum DownloadError {
    #[error("track is private or requires an account")]
    Private,
    #[error("track is region locked")]
    RegionLocked,
    #[error("track is too long: {actual}s (limit {limit}s)")]
    TooLong { limit: u64, actual: u64 },
    #[error("file is too large: {actual} bytes (limit {limit} bytes)")]
    TooLarge { limit: u64, actual: u64 },
    #[error("track was not found")]
    NotFound,
    #[error("unsupported input: {0}")]
    Unsupported(String),
    #[error("provider {0} is disabled")]
    ProviderDisabled(Provider),
    #[error("source is temporarily unavailable: {0}")]
    SourceUnavailable(String),
    #[error("audio processing failed: {0}")]
    Audio(String),
    #[error("download pipeline failed: {0}")]
    Internal(String),
}

impl From<std::io::Error> for DownloadError {
    fn from(error: std::io::Error) -> Self {
        DownloadError::Internal(format!("io error: {error}"))
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("download error: {0}")]
    Download(#[from] DownloadError),
    #[error("configuration error: {0}")]
    Config(String),
    // Catch-all for foreign library errors (sqlx, reqwest, teloxide, serde_json).
    // `common` must not depend on those crates, so the `#[from]` impl below
    // boxes them instead of naming them.
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("{0}")]
    Message(String),
}

pub type AppResult<T> = Result<T, AppError>;
pub type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_error_is_cloneable_and_displayable() {
        let error = DownloadError::TooLong {
            limit: 1200,
            actual: 1300,
        };
        let copied = error.clone();
        assert_eq!(error.to_string(), copied.to_string());
    }
}
