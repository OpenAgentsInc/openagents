//! Ollama provider using the OpenAI-compatible API.

use super::openai_common::{OpenAIStreamAdapter, build_openai_body};
use crate::message::CompletionRequest;
use crate::model::{self, ModelInfo};
use crate::provider::{LlmProvider, ProviderCapabilities, ProviderError};
use crate::stream::{CompletionStream, SseStream};
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";

/// Local Ollama provider (OpenAI-compatible).
pub struct OllamaProvider {
    client: Client,
    base_url: String,
}

impl OllamaProvider {
    /// Create a new provider using `OLLAMA_BASE_URL` or `OLLAMA_HOST`.
    pub fn new() -> Result<Self, ProviderError> {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .or_else(|_| std::env::var("OLLAMA_HOST"))
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        Ok(Self { client, base_url })
    }

    /// Override the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn display_name(&self) -> &'static str {
        "Ollama"
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        self.client
            .get(url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .is_ok()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(model::ollama::all())
    }

    async fn stream(&self, request: CompletionRequest) -> Result<CompletionStream, ProviderError> {
        let body = build_openai_body(&request)?;
        let model = request.model.clone();

        let response = self
            .client
            .post(format!(
                "{}/chat/completions",
                self.base_url.trim_end_matches('/')
            ))
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
        let adapter = OpenAIStreamAdapter::new(sse_stream, model, "ollama");

        Ok(Box::pin(adapter))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: false,
            extended_thinking: false,
            prompt_caching: false,
            interleaved_thinking: false,
            fine_grained_tool_streaming: true,
        }
    }
}
