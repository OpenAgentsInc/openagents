//! Error types for bench-harness.

use thiserror::Error;

/// Result type for bench-harness operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in bench-harness.
#[derive(Error, Debug)]
pub enum Error {
    /// Task not found.
    #[error("task not found: {0}")]
    TaskNotFound(String),

    /// Method error.
    #[error("method error: {0}")]
    MethodError(String),

    /// Dataset error.
    #[error("dataset error: {0}")]
    DatasetError(String),

    /// Experiment error.
    #[error("experiment error: {0}")]
    ExperimentError(String),

    /// Checkpoint error.
    #[error("checkpoint error: {0}")]
    CheckpointError(String),

    /// LM router error.
    #[error("LM router error: {0}")]
    LmRouter(#[from] lm_router::Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
