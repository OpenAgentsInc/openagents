//! Error types for the ACP adapter

use thiserror::Error;

/// Result type for ACP operations
pub type Result<T> = std::result::Result<T, AcpError>;

/// Errors that can occur during ACP operations
#[derive(Error, Debug)]
pub enum AcpError {
    /// Failed to spawn agent subprocess
    #[error("Failed to spawn agent: {0}")]
    SpawnError(#[from] std::io::Error),

    /// Protocol error during ACP communication
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Agent executable not found
    #[error("Agent executable not found: {0}")]
    AgentNotFound(String),

    /// Initialization failed
    #[error("Initialization failed: {0}")]
    InitializationError(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// File operation error
    #[error("File operation error: {0}")]
    FileError(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Timeout
    #[error("Operation timed out")]
    Timeout,

    /// Generic error with message
    #[error("{0}")]
    Other(String),
}

impl From<String> for AcpError {
    fn from(s: String) -> Self {
        AcpError::Other(s)
    }
}

impl From<&str> for AcpError {
    fn from(s: &str) -> Self {
        AcpError::Other(s.to_string())
    }
}
