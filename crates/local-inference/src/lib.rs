mod backend;
mod error;
mod types;

pub use backend::{LocalModelBackend, LocalModelBackendExt};
pub use error::LocalModelError;
pub use types::{CompletionRequest, CompletionResponse, ModelInfo, StreamChunk, UsageInfo};

pub type Result<T> = std::result::Result<T, LocalModelError>;
