pub mod client;
pub mod error;
pub mod sessions;
pub mod streaming;
pub mod tools;
pub mod types;

// Re-export main types
pub use client::{FMClient, FMClientBuilder};
pub use error::{FMError, Result};
pub use sessions::{
    CreateSessionRequest, CreateSessionResponse, DeleteSessionResponse, ListSessionsResponse,
    SessionClient, SessionInfo, TranscriptResponse,
};
pub use streaming::{Delta, StreamChunk, StreamChoice, StreamingClient};
pub use tools::{
    FunctionCall, ListToolsResponse, PropertyDefinition, RegisterToolsResponse,
    RemoveToolsResponse, ToolCall, ToolClient, ToolDefinition, ToolParameters,
};
pub use types::{
    ChatMessage, Choice, CompletionOptions, CompletionRequest, CompletionResponse, Guardrails,
    HealthResponse, ModelInfo, ModelsResponse, ResponseFormat, SamplingMode, Usage, UseCase,
};
