//! Error types for the ACP crate.

use thiserror::Error;

/// Result type alias using AcpError.
pub type Result<T> = std::result::Result<T, AcpError>;

/// Errors that can occur during ACP operations.
#[derive(Error, Debug)]
pub enum AcpError {
    /// Protocol version not supported
    #[error("Unsupported protocol version")]
    UnsupportedVersion,

    /// Authentication is required
    #[error("Authentication required: {message}")]
    AuthRequired {
        message: String,
        description: Option<String>,
    },

    /// Session not found
    #[error("Session not found: {session_id}")]
    SessionNotFound { session_id: String },

    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Process spawn failed
    #[error("Failed to spawn Claude Code process: {0}")]
    SpawnFailed(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// ACP protocol error
    #[error("ACP protocol error: {0}")]
    Protocol(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl AcpError {
    /// Create a new AuthRequired error.
    pub fn auth_required(message: impl Into<String>) -> Self {
        Self::AuthRequired {
            message: message.into(),
            description: None,
        }
    }

    /// Create a new AuthRequired error with description.
    pub fn auth_required_with_description(
        message: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self::AuthRequired {
            message: message.into(),
            description: Some(description.into()),
        }
    }
}
