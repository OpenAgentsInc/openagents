//! Error types for lm-router.

use thiserror::Error;

/// Result type for lm-router operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in lm-router.
#[derive(Error, Debug)]
pub enum Error {
    /// Backend not found for the given model.
    #[error("no backend found for model: {0}")]
    BackendNotFound(String),

    /// Backend is unavailable.
    #[error("backend {0} is unavailable")]
    BackendUnavailable(String),

    /// LLM request failed.
    #[error("LLM request failed: {0}")]
    RequestFailed(String),

    /// Timeout waiting for response.
    #[error("request timed out after {0}ms")]
    Timeout(u64),

    /// Simulated failure (for swarm testing).
    #[error("simulated failure: {0}")]
    SimulatedFailure(String),

    /// FM Bridge error.
    #[error("FM Bridge error: {0}")]
    FmBridge(#[from] fm_bridge::FMError),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
