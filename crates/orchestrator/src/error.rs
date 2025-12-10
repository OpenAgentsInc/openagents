//! Orchestrator error types

use thiserror::Error;

/// Result type for orchestrator operations
pub type OrchestratorResult<T> = Result<T, OrchestratorError>;

/// Errors that can occur during orchestration
#[derive(Error, Debug)]
pub enum OrchestratorError {
    /// Task-related error
    #[error("Task error: {0}")]
    TaskError(#[from] tasks::TaskError),

    /// Tool execution error
    #[error("Tool error: {0}")]
    ToolError(String),

    /// LLM-related error
    #[error("LLM error: {0}")]
    LlmError(#[from] llm::LlmError),

    /// Session error
    #[error("Session error: {0}")]
    SessionError(String),

    /// Verification failed
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    /// Git operation error
    #[error("Git error: {0}")]
    GitError(String),

    /// Safe mode violation
    #[error("Safe mode violation: {0}")]
    SafeModeViolation(String),

    /// Sandbox error
    #[error("Sandbox error: {0}")]
    SandboxError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Timeout
    #[error("Operation timeout: {0}")]
    Timeout(String),

    /// Maximum retries exceeded
    #[error("Maximum retries exceeded: {0}")]
    MaxRetriesExceeded(String),

    /// Cycle detected in task dependencies
    #[error("Dependency cycle detected: {0}")]
    CycleDetected(String),

    /// Invalid state transition
    #[error("Invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    /// IO error
    #[error("IO error: {0}")]
    IoError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<std::io::Error> for OrchestratorError {
    fn from(err: std::io::Error) -> Self {
        OrchestratorError::IoError(err.to_string())
    }
}

impl From<tools::ToolError> for OrchestratorError {
    fn from(err: tools::ToolError) -> Self {
        OrchestratorError::ToolError(err.to_string())
    }
}
