//! Error types for the Daytona API client.

use thiserror::Error;

/// Errors that can occur when interacting with the Daytona API.
#[derive(Debug, Error)]
pub enum DaytonaError {
    /// No authentication credentials configured
    #[error("Not configured: {0}")]
    NotConfigured(String),

    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    /// API returned an error response
    #[error("API error ({status}): {message}")]
    ApiError {
        /// HTTP status code
        status: u16,
        /// Error message from API
        message: String,
    },

    /// Sandbox not found
    #[error("Sandbox not found: {0}")]
    SandboxNotFound(String),

    /// Invalid response from API
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Authentication failed (401)
    #[error("Authentication failed")]
    Unauthorized,

    /// Forbidden (403)
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Toolbox operation failed
    #[error("Toolbox operation failed: {0}")]
    ToolboxError(String),

    /// Timeout waiting for sandbox state
    #[error("Timeout waiting for sandbox state")]
    StateTimeout,

    /// URL parsing error
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
}

/// Result type alias for Daytona operations.
pub type Result<T> = std::result::Result<T, DaytonaError>;
