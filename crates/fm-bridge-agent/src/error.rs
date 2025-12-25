use thiserror::Error;

#[derive(Error, Debug)]
pub enum FmBridgeAgentError {
    #[error("FM Bridge client error: {0}")]
    ClientError(#[from] fm_bridge::FMError),

    #[error("Local inference error: {0}")]
    InferenceError(#[from] local_inference::LocalModelError),

    #[error("Tool execution error: {0}")]
    ToolError(String),

    #[error("Tool backend error: {0}")]
    ToolBackendError(#[from] gpt_oss_agent::GptOssAgentError),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, FmBridgeAgentError>;
