pub mod backend;
/// GPT-OSS Responses API Client
///
/// Rust client for OpenAI's open-weight models via Responses API.
///
/// # Example
///
/// ```no_run
/// use gpt_oss::GptOssClient;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = GptOssClient::builder()
///         .base_url("http://localhost:8000")
///         .build()?;
///
///     let response = client.complete_simple("gpt-oss-20b", "What is Rust?").await?;
///     println!("Response: {}", response);
///     Ok(())
/// }
/// ```
pub mod client;
pub mod error;
pub mod harmony;
pub mod types;

// Re-export main types
pub use client::{GptOssClient, GptOssClientBuilder};
pub use error::{GptOssError, Result};
pub use harmony::{
    HarmonyPromptConfig, HarmonyRenderer, HarmonyRole, HarmonyToolSpec, HarmonyTurn,
};
pub use types::{
    GptOssReasoningEffort, GptOssRequest, GptOssResponse, GptOssResponsesRequest,
    GptOssResponsesResponse, GptOssStreamChunk, GptOssToolCall, GptOssToolChoice,
    GptOssToolChoiceFunction, GptOssToolDefinition, GptOssToolFunction,
};
