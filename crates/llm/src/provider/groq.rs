//! Groq provider implementation using OpenAI-compatible chat completions API.
//!
//! Groq provides ultra-fast inference for open-source models like Llama.
//! API docs: https://console.groq.com/docs/api-reference

use super::openai_common::{build_openai_body, OpenAIStreamAdapter};
use crate::message::CompletionRequest;
use crate::model::{self, ModelInfo};
use crate::provider::{LlmProvider, ProviderCapabilities, ProviderError};
use crate::stream::{CompletionStream, SseStream};
use async_trait::async_trait;
use reqwest::Client;

const DEFAULT_BASE_URL: &str = "https://api.groq.com/openai/v1";

/// Groq provider using OpenAI-compatible API.
pub struct GroqProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl GroqProvider {
    /// Create a new Groq provider using `GROQ_API_KEY`.
    pub fn new() -> Result<Self, ProviderError> {
        let api_key = std::env::var("GROQ_API_KEY")
            .map_err(|_| ProviderError::MissingCredentials("GROQ_API_KEY".into()))?;

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url: std::env::var("GROQ_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
        })
    }

    /// Create with a custom API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.into(),
        }
    }

    /// Override the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[async_trait]
impl LlmProvider for GroqProvider {
    fn id(&self) -> &'static str {
        "groq"
    }

    fn display_name(&self) -> &'static str {
        "Groq"
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(model::groq::all())
    }

    async fn stream(&self, request: CompletionRequest) -> Result<CompletionStream, ProviderError> {
        let body = build_openai_body(&request)?;
        let model = request.model.clone();

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError {
                status,
                message: error_body,
            });
        }

        let sse_stream = SseStream::new(response.bytes_stream());
        let adapter = OpenAIStreamAdapter::new(sse_stream, model, "groq");

        Ok(Box::pin(adapter))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: false, // Groq currently doesn't support vision
            extended_thinking: false,
            prompt_caching: false,
            interleaved_thinking: false,
            fine_grained_tool_streaming: true,
        }
    }
}
