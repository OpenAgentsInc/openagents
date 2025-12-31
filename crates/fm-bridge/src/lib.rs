pub mod backend;
/// Foundation Model API Bridge
///
/// Rust client for Apple Foundation Models via HTTP bridge.
///
/// # Example
///
/// ```no_run
/// use fm_bridge::{FMClient, CompletionOptions};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = FMClient::new()?;
///
///     let response = client
///         .complete("What is the capital of France?", None)
///         .await?;
///
///     println!("Response: {}", response.choices[0].message.content);
///     Ok(())
/// }
/// ```
pub mod client;
pub mod error;
pub mod types;

// Re-export main types
pub use client::{FMClient, FMClientBuilder};
pub use error::{FMError, Result};
pub use types::{
    ChatMessage, Choice, CompletionOptions, CompletionRequest, CompletionResponse, FinishReason,
    ModelInfo, StreamChunk, Usage,
};
