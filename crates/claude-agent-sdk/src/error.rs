//! Error types for the Claude Agent SDK.

use thiserror::Error;

/// Errors that can occur when using the Claude Agent SDK.
#[derive(Error, Debug)]
pub enum Error {
    /// Failed to spawn the Claude Code CLI process.
    #[error("failed to spawn claude process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    /// Claude Code CLI executable not found.
    #[error("claude executable not found: {0}")]
    ExecutableNotFound(String),

    /// Failed to serialize/deserialize JSON.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The CLI process exited unexpectedly.
    #[error("process exited unexpectedly with code: {0:?}")]
    ProcessExited(Option<i32>),

    /// Failed to write to stdin.
    #[error("failed to write to stdin: {0}")]
    StdinWrite(std::io::Error),

    /// Failed to read from stdout.
    #[error("failed to read from stdout: {0}")]
    StdoutRead(std::io::Error),

    /// The query was aborted.
    #[error("query aborted")]
    Aborted,

    /// Invalid protocol message received.
    #[error("invalid protocol message: {0}")]
    InvalidMessage(String),

    /// Control request timed out.
    #[error("control request timed out")]
    ControlTimeout,

    /// Permission denied for tool use.
    #[error("permission denied for tool: {tool}")]
    PermissionDenied { tool: String },

    /// Initialization failed.
    #[error("initialization failed: {0}")]
    InitializationFailed(String),

    /// Hook callback failed.
    #[error("hook callback failed: {0}")]
    HookCallbackFailed(String),

    /// MCP server error.
    #[error("MCP server error: {0}")]
    McpError(String),
}

/// Result type alias for the Claude Agent SDK.
pub type Result<T> = std::result::Result<T, Error>;
