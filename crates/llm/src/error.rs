//! LLM error types

use thiserror::Error;

/// Result type for LLM operations
pub type LlmResult<T> = Result<T, LlmError>;

/// Errors that can occur during LLM operations
#[derive(Error, Debug)]
pub enum LlmError {
    /// Invalid API key or authentication failure
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    /// Request timeout
    #[error("Request timeout: {0}")]
    TimeoutError(String),

    /// Invalid request (bad parameters)
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Model not found or not available
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Context length exceeded
    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),

    /// Content filtering triggered
    #[error("Content filtered: {0}")]
    ContentFiltered(String),

    /// Tool execution error
    #[error("Tool error: {0}")]
    ToolError(String),

    /// Provider-specific error
    #[error("Provider error ({provider}): {message}")]
    ProviderError { provider: String, message: String },

    /// Network or HTTP error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Streaming error
    #[error("Stream error: {0}")]
    StreamError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Unknown error
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<reqwest::Error> for LlmError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            LlmError::TimeoutError(err.to_string())
        } else if err.is_connect() {
            LlmError::NetworkError(format!("Connection error: {}", err))
        } else {
            LlmError::NetworkError(err.to_string())
        }
    }
}

impl From<serde_json::Error> for LlmError {
    fn from(err: serde_json::Error) -> Self {
        LlmError::SerializationError(err.to_string())
    }
}

/// Error response from an API
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApiErrorResponse {
    #[serde(alias = "error")]
    pub error: ApiError,
}

/// API error details
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApiError {
    #[serde(alias = "type", default)]
    pub error_type: String,
    pub message: String,
    #[serde(default)]
    pub code: Option<String>,
}
