//! Gateway abstraction for external AI services
//!
//! This crate provides a unified interface for accessing various AI service providers
//! like Cerebras, OpenAI, OpenAI, and others.
//!
//! # Example
//!
//! ```no_run
//! use gateway::{CerebrasGateway, InferenceGateway, ChatRequest, Message};
//!
//! #[tokio::main]
//! async fn main() -> gateway::Result<()> {
//!     // Create gateway from environment (reads CEREBRAS_API_KEY)
//!     let gateway = CerebrasGateway::from_env()?;
//!
//!     // Create a chat request
//!     let request = ChatRequest::new(
//!         "zai-glm-4.7",
//!         vec![Message::user("Hello, how are you?")],
//!     );
//!
//!     // Send request
//!     let response = gateway.chat(request).await?;
//!     println!("Response: {:?}", response.content());
//!
//!     Ok(())
//! }
//! ```

#[cfg(feature = "dspy")]
pub mod dspy;
pub mod error;
pub mod inference;
pub mod traits;
pub mod types;

// Re-export commonly used items
#[cfg(feature = "dspy")]
pub use dspy::{GatewayDspyConfig, GatewayDspyExt, GatewayLM, query_with_signature};
pub use error::{GatewayError, Result};
pub use inference::{
    ChatRequest, ChatResponse, Choice, Message, Role, Usage, cerebras::CerebrasGateway,
};
pub use traits::{Gateway, InferenceGateway};
pub use types::{Capability, GatewayHealth, ModelInfo, ModelPricing};
