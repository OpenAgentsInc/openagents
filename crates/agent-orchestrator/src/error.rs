use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Agent not found: {name}")]
    AgentNotFound { name: String },

    #[error("Agent is disabled: {name}")]
    AgentDisabled { name: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Hook blocked execution: {message}")]
    HookBlocked { message: String },

    #[error("Task not found: {id}")]
    TaskNotFound { id: String },

    #[error("Task already completed: {id}")]
    TaskAlreadyCompleted { id: String },
}

pub type Result<T> = std::result::Result<T, Error>;
