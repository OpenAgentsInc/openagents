//! Error types for the Codex Agent SDK.

use thiserror::Error;

/// Errors that can occur when using the Codex Agent SDK.
#[derive(Error, Debug)]
pub enum Error {
    /// Failed to spawn the codex process.
    #[error("failed to spawn codex process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    /// The codex executable was not found.
    #[error("codex executable not found: {0}")]
    ExecutableNotFound(String),

    /// Failed to parse JSON from the codex output.
    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// The codex process exited unexpectedly.
    #[error("process exited unexpectedly with code: {0:?}")]
    ProcessExited(Option<i32>),

    /// Failed to write to the process stdin.
    #[error("failed to write to stdin: {0}")]
    StdinWrite(std::io::Error),

    /// Failed to read from the process stdout.
    #[error("failed to read from stdout: {0}")]
    StdoutRead(std::io::Error),

    /// A turn failed with an error from the agent.
    #[error("turn failed: {0}")]
    TurnFailed(String),

    /// The thread has not been started yet.
    #[error("thread not started")]
    ThreadNotStarted,

    /// Failed to create a temporary file for output schema.
    #[error("failed to create output schema file: {0}")]
    OutputSchemaFile(std::io::Error),
}

/// A specialized Result type for Codex Agent SDK operations.
pub type Result<T> = std::result::Result<T, Error>;
