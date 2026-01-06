//! Error types for bench-datasets.

use thiserror::Error;

/// Result type for bench-datasets operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in bench-datasets.
#[derive(Error, Debug)]
pub enum Error {
    /// Dataset not found.
    #[error("dataset not found: {0}")]
    NotFound(String),

    /// Invalid dataset format.
    #[error("invalid dataset format: {0}")]
    InvalidFormat(String),

    /// Missing required field.
    #[error("missing required field: {0}")]
    MissingField(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
