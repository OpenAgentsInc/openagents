//! Error types for the Nostr relay

use thiserror::Error;

/// Result type alias for relay operations
pub type Result<T> = std::result::Result<T, RelayError>;

/// Errors that can occur in the relay
#[derive(Debug, Error)]
pub enum RelayError {
    /// Database error
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Connection pool error
    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),

    /// JSON serialization error
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Event validation error
    #[error("invalid event: {0}")]
    InvalidEvent(String),

    /// WebSocket error
    #[error("websocket error: {0}")]
    WebSocket(String),

    /// I/O error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration error
    #[error("config error: {0}")]
    Config(String),

    /// Event not found
    #[error("event not found: {0}")]
    EventNotFound(String),

    /// Subscription error
    #[error("subscription error: {0}")]
    Subscription(String),
}
