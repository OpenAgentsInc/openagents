//! Error types for the runtime core.

use thiserror::Error;

/// Runtime result type.
pub type Result<T> = std::result::Result<T, AgentError>;

/// Storage result type.
pub type StorageResult<T> = std::result::Result<T, StorageError>;

/// Errors raised during agent execution.
#[derive(Debug, Error)]
pub enum AgentError {
    /// Storage failure.
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// State migration required.
    #[error("state migration required from {from_version} to {to_version}")]
    StateMigrationRequired {
        /// Stored schema version.
        from_version: u32,
        /// Current schema version.
        to_version: u32,
    },

    /// Generic tick failure.
    #[error("tick error: {0}")]
    Tick(String),
}

/// Errors raised by storage implementations.
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite failure.
    #[cfg(feature = "local")]
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A requested entry was not found.
    #[error("not found")]
    NotFound,

    /// Generic storage failure.
    #[error("{0}")]
    Other(String),
}

impl From<&str> for AgentError {
    fn from(value: &str) -> Self {
        AgentError::Tick(value.to_string())
    }
}

impl From<String> for AgentError {
    fn from(value: String) -> Self {
        AgentError::Tick(value)
    }
}
