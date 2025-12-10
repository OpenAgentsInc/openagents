//! Multi-provider LLM abstraction layer
//!
//! LLM-001..024: Provider selection, chat, tool calling, streaming, token accounting
//!
//! # Example
//!
//! ```ignore
//! use llm::{LlmClient, Message, ProviderConfig};
//!
//! // Using Anthropic
//! let client = LlmClient::anthropic("sk-...")?;
//! let messages = vec![Message::user("Hello!")];
//! let response = client.chat(&messages, None).await?;
//!
//! // Using OpenAI
//! let openai = OpenAIProvider::new(ProviderConfig::new("sk-..."))?;
//! let response = openai.chat(&messages, None).await?;
//!
//! // With retry logic
//! use llm::{retry_with_backoff, RetryConfig, is_retryable_error};
//! let result = retry_with_backoff(
//!     || async { client.chat(&messages, None).await },
//!     RetryConfig::default(),
//!     is_retryable_error,
//! ).await;
//! ```

mod error;
mod message;
mod provider;
mod tool;
mod client;
mod anthropic;
mod openai;
mod retry;
mod models;

pub use error::*;
pub use message::*;
pub use provider::*;
pub use tool::*;
pub use client::*;
pub use anthropic::*;
pub use openai::*;
pub use retry::*;
pub use models::*;
