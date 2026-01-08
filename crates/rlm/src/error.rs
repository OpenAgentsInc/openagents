//! Error types for the RLM engine.

use thiserror::Error;

/// Errors that can occur during RLM execution.
#[derive(Debug, Error)]
pub enum RlmError {
    /// Error from the LLM client.
    #[error("LLM error: {0}")]
    LlmError(String),

    /// Error during code execution.
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// Failed to parse LLM response.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Exceeded maximum iteration count.
    #[error("Max iterations exceeded: {0}")]
    MaxIterationsExceeded(u32),

    /// FM Bridge connection error (only available with fm-bridge feature).
    #[cfg(feature = "fm-bridge")]
    #[error("FM Bridge error: {0}")]
    FmBridgeError(#[from] fm_bridge::FMError),

    /// Shell command execution error.
    #[error("Shell error: {0}")]
    ShellError(String),

    /// Context loading error.
    #[error("Context error: {0}")]
    ContextError(String),

    /// Sub-query error.
    #[error("Sub-query error: {0}")]
    SubQueryError(String),

    /// Model is stuck in a loop.
    #[error("Model stuck: {0}")]
    Stuck(String),

    /// Client error (e.g., Claude SDK error).
    #[error("Client error: {0}")]
    ClientError(String),
}

/// Result type for RLM operations.
pub type Result<T> = std::result::Result<T, RlmError>;
