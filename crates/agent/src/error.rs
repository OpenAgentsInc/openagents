//! Agent error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Lock error: {0}")]
    Lock(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Task error: {0}")]
    Task(String),

    #[error("Subtask error: {0}")]
    Subtask(String),

    #[error("Verification failed: {0}")]
    Verification(String),

    #[error("Init script failed: {0}")]
    InitScript(String),

    #[error("Orchestrator error: {0}")]
    Orchestrator(String),

    #[error("Subagent error: {0}")]
    Subagent(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("LLM error: {0}")]
    Llm(#[from] llm::LlmError),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Aborted: {0}")]
    Aborted(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(String),
}

pub type AgentResult<T> = Result<T, AgentError>;
