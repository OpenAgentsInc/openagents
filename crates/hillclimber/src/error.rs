//! Error types for the HillClimber system.

use thiserror::Error;

/// HillClimber error types
#[derive(Debug, Error)]
pub enum HillClimberError {
    /// Database operation failed
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// FM Bridge error (LLM calls)
    #[error("FM Bridge error: {0}")]
    FmBridge(#[from] fm_bridge::FMError),

    /// TestGen error
    #[error("TestGen error: {0}")]
    TestGen(#[from] testgen::TestGenError),

    /// Docker error
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),

    /// Configuration not found
    #[error("Configuration not found: {0}")]
    ConfigNotFound(String),

    /// Configuration error (LLM provider setup)
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Task not found
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// Action rejected by monitor
    #[error("Action rejected: {0}")]
    ActionRejected(String),

    /// Workspace error (file operations)
    #[error("Workspace error: {0}")]
    Workspace(String),

    /// Timeout exceeded
    #[error("Timeout exceeded: {0}")]
    Timeout(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<String> for HillClimberError {
    fn from(s: String) -> Self {
        HillClimberError::Other(s)
    }
}

impl From<&str> for HillClimberError {
    fn from(s: &str) -> Self {
        HillClimberError::Other(s.to_string())
    }
}

/// Result alias for HillClimber operations
pub type Result<T> = std::result::Result<T, HillClimberError>;
