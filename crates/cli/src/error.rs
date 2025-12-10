//! CLI error types

use thiserror::Error;

/// CLI result type
pub type CliResult<T> = Result<T, CliError>;

/// CLI errors
#[derive(Error, Debug)]
pub enum CliError {
    /// Task not found
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Invalid argument
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Task repository error
    #[error("Task error: {0}")]
    TaskError(#[from] tasks::TaskError),

    /// Orchestrator error
    #[error("Orchestrator error: {0}")]
    OrchestratorError(#[from] orchestrator::OrchestratorError),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Other error
    #[error("{0}")]
    Other(String),
}

impl CliError {
    /// Create a new "other" error
    pub fn other(msg: impl Into<String>) -> Self {
        CliError::Other(msg.into())
    }
}
