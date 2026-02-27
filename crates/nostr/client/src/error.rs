//! Client error types.

use thiserror::Error;

/// Client error type.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("connection error: {0}")]
    Connection(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("relay error: {0}")]
    RelayError(String),

    #[error("subscription error: {0}")]
    Subscription(String),

    #[error("timeout error: {0}")]
    Timeout(String),

    #[error("not connected")]
    NotConnected,

    #[error("already connected")]
    AlreadyConnected,

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Client result type.
pub type Result<T> = std::result::Result<T, ClientError>;
