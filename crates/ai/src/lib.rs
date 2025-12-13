//! Centralized AI model definitions
//!
//! This crate provides a single source of truth for all allowed AI models
//! across different providers (Anthropic Claude, Grok, OpenAI).
//!
//! # Allowed Models
//!
//! **Claude (4 models):**
//! - `ClaudeHaiku45` - Fast, cost-effective
//! - `ClaudeSonnet45` - Balanced speed/quality
//! - `ClaudeOpus45` - Best quality, slowest
//! - `ClaudeOpus41` - Extended thinking
//!
//! **Grok (11 models):**
//! - All variants from `Grok3` to `Grok41FastReasoning`
//!
//! **OpenAI (14 models):**
//! - All variants from `Gpt4o` to `Gpt51`
//!
//! # Usage
//!
//! ```rust,no_run
//! use ai::Model;
//!
//! // Get default model (Claude Haiku 4.5)
//! let model = Model::default();
//! println!("Using: {}", model.display_name());
//!
//! // Parse from string
//! let sonnet = Model::from_id("claude-sonnet-4-5-20250929").unwrap();
//!
//! // Get provider
//! let provider = model.provider();
//! println!("API Key: {}", provider.api_key_env_var());
//! ```

pub mod metadata;
pub mod model;
pub mod provider;

pub use metadata::{Capabilities, ModelMetadata, Pricing};
pub use model::{Model, ModelError};
pub use provider::Provider;
