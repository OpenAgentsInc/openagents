//! Error types for the Moltbook API client.

use thiserror::Error;

/// Errors returned by the Moltbook client.
#[derive(Error, Debug)]
pub enum MoltbookError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// API returned an error payload.
    #[error("API error: {error}{}", hint.as_ref().map(|h| format!(" (hint: {h})")).unwrap_or_default())]
    Api {
        /// HTTP status code.
        status: u16,
        /// Error message from the API.
        error: String,
        /// Optional hint from the API.
        hint: Option<String>,
    },

    /// Rate limited; retry after the given number of minutes.
    #[error("Rate limited; retry after {retry_after_minutes} minutes")]
    RateLimited {
        /// Minutes until the client can retry.
        retry_after_minutes: u32,
    },

    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type for Moltbook operations.
pub type Result<T> = std::result::Result<T, MoltbookError>;
