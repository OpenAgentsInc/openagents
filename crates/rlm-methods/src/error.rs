//! Error types for rlm-methods.

use thiserror::Error;

/// Result type for rlm-methods operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in rlm-methods.
#[derive(Error, Debug)]
pub enum Error {
    /// LM router error.
    #[error("LM error: {0}")]
    Lm(#[from] lm_router::Error),

    /// Method execution error.
    #[error("method error: {0}")]
    Method(String),

    /// Timeout error.
    #[error("timeout: {0}")]
    Timeout(String),

    /// Parse error.
    #[error("parse error: {0}")]
    Parse(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<Error> for bench_harness::Error {
    fn from(e: Error) -> Self {
        bench_harness::Error::MethodError(e.to_string())
    }
}
