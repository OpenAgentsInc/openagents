//! LLM Provider Abstraction Layer
//!
//! This crate provides a unified interface for interacting with various LLM providers
//! (Anthropic, OpenAI, Ollama, etc.) with streaming support, tool calling, and
//! provider-specific features.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    LlmProvider trait                        │
//! │  stream() -> CompletionStream                               │
//! │  capabilities() -> ProviderCapabilities                     │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!          ┌───────────────────┼───────────────────┐
//!          │                   │                   │
//!          ▼                   ▼                   ▼
//!    ┌──────────┐       ┌──────────┐       ┌──────────┐
//!    │ Anthropic │       │  OpenAI  │       │  Ollama  │
//!    └──────────┘       └──────────┘       └──────────┘
//! ```

pub mod message;
pub mod model;
pub mod provider;
pub mod stream;

// Re-exports
pub use message::{
    CompletionRequest, ContentBlock, ImageSource, Message, ProviderOptions, ResponseFormat, Role,
    Tool, ToolChoice, ToolResultContent,
};
pub use model::{ModelCapabilities, ModelInfo, ModelLimits, ModelPricing, ModelStatus};
pub use provider::{LlmProvider, ProviderCapabilities, ProviderError, ProviderRegistry};
pub use stream::{CompletionStream, FinishReason, StreamEvent, Usage};
