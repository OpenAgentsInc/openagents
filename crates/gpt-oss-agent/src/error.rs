use thiserror::Error;

#[derive(Error, Debug)]
pub enum GptOssAgentError {
    #[error("GPT-OSS client error: {0}")]
    ClientError(#[from] gpt_oss::GptOssError),

    #[error("Local inference error: {0}")]
    InferenceError(#[from] local_inference::LocalModelError),

    #[error("Tool execution error: {0}")]
    ToolError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, GptOssAgentError>;
