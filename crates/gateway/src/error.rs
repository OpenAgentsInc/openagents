use thiserror::Error;

/// Gateway error types
#[derive(Error, Debug)]
pub enum GatewayError {
    /// Gateway not configured (missing API key, etc.)
    #[error("Gateway not configured: {0}")]
    NotConfigured(String),

    /// HTTP request failed
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// API returned an error response
    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimited,

    /// Request timeout
    #[error("Request timeout")]
    Timeout,

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),
}

/// Result type for gateway operations
pub type Result<T> = std::result::Result<T, GatewayError>;
