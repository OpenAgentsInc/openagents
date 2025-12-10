/// Error types for the FM Bridge client

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FMError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Timeout")]
    Timeout,

    #[error("Invalid response format")]
    InvalidResponse,
}

pub type Result<T> = std::result::Result<T, FMError>;
