//! OpenAI provider implementation using the chat completions API.

use super::openai_common::{OpenAIStreamAdapter, build_openai_body};
use crate::message::CompletionRequest;
use crate::model::{self, ModelInfo};
use crate::provider::{LlmProvider, ProviderCapabilities, ProviderError};
use crate::stream::{CompletionStream, SseStream};
use async_trait::async_trait;
use reqwest::Client;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI chat completions provider.
pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider using `OPENAI_API_KEY`.
    pub fn new() -> Result<Self, ProviderError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| ProviderError::MissingCredentials("OPENAI_API_KEY".into()))?;

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url: std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
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

    /// Override the base URL (useful for proxies).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn id(&self) -> &'static str {
        "openai"
    }

    fn display_name(&self) -> &'static str {
        "OpenAI"
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(model::openai::all())
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
        let adapter = OpenAIStreamAdapter::new(sse_stream, model, "openai");

        Ok(Box::pin(adapter))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: false,
            prompt_caching: true,
            interleaved_thinking: false,
            fine_grained_tool_streaming: true,
        }
    }
}
