//! Multi-provider LLM abstraction layer
//!
//! LLM-001..024: Provider selection, chat, tool calling, streaming, token accounting
//!
//! # Example
//!
//! ```ignore
//! use llm::{LlmClient, Message, Role};
//!
//! let client = LlmClient::anthropic("sk-...")?;
//! let messages = vec![Message::user("Hello!")];
//! let response = client.chat(&messages, None).await?;
//! ```

mod error;
mod message;
mod provider;
mod tool;
mod client;
mod anthropic;

pub use error::*;
pub use message::*;
pub use provider::*;
pub use tool::*;
pub use client::*;
pub use anthropic::*;
