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
///     let response = client.complete_simple("gpt-4o-mini", "What is Rust?").await?;
///     println!("Response: {}", response);
///     Ok(())
/// }
/// ```
pub mod client;
pub mod error;
pub mod types;
pub mod backend;

// Re-export main types
pub use client::{GptOssClient, GptOssClientBuilder};
pub use error::{GptOssError, Result};
pub use types::{GptOssRequest, GptOssResponse, GptOssStreamChunk};
