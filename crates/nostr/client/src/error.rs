//! Client error types

use thiserror::Error;

/// Client error type
#[derive(Error, Debug)]
pub enum ClientError {
    /// WebSocket error
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Invalid URL
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// URL parse error
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// Relay error response
    #[error("Relay error: {0}")]
    RelayError(String),

    /// Subscription error
    #[error("Subscription error: {0}")]
    Subscription(String),

    /// Timeout error
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// Not connected
    #[error("Not connected to relay")]
    NotConnected,

    /// Already connected
    #[error("Already connected to relay")]
    AlreadyConnected,

    /// Circuit breaker is open (too many failures)
    #[error("Circuit breaker open: {0}")]
    CircuitOpen(String),

    /// Event publish failed
    #[error("Event publish failed: {0}")]
    PublishFailed(String),

    /// Invalid event
    #[error("Invalid event: {0}")]
    InvalidEvent(String),

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Protocol error
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Client result type
pub type Result<T> = std::result::Result<T, ClientError>;
