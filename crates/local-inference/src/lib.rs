mod error;
mod types;
mod backend;

pub use error::LocalModelError;
pub use types::{CompletionRequest, CompletionResponse, StreamChunk, ModelInfo, UsageInfo};
pub use backend::LocalModelBackend;

pub type Result<T> = std::result::Result<T, LocalModelError>;
