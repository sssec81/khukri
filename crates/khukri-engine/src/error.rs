use thiserror::Error;

#[derive(Debug, Error)]
pub enum KhukriError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("Database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Permanent HTTP error: status {status} for {url}")]
    PermanentError { status: u16, url: String },

    #[error("Cannot pre-allocate {bytes} bytes on disk")]
    DiskSpaceError { bytes: u64 },

    #[error("Server does not support range requests — falling back to single thread")]
    NoRangeSupport,

    #[error("Max retries exceeded after {attempts} attempt(s)")]
    MaxRetriesExceeded { attempts: u8 },

    #[error("Download cancelled")]
    Cancelled,

    #[error("Missing Content-Length header")]
    NoContentLength,

    #[error("Task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Invalid configuration for '{field}': {reason}")]
    InvalidConfig { field: &'static str, reason: String },
}

pub type Result<T> = std::result::Result<T, KhukriError>;
