//! Apple Foundation Models provider via the FM bridge (OpenAI-compatible).

use super::openai_common::{OpenAIStreamAdapter, build_openai_body};
use crate::message::CompletionRequest;
use crate::model::{self, ModelInfo};
use crate::provider::{LlmProvider, ProviderCapabilities, ProviderError};
use crate::stream::{CompletionStream, SseStream};
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "http://localhost:3030";

/// Apple Foundation Models provider (via local bridge).
pub struct AppleProvider {
    client: Client,
    base_url: String,
}

impl AppleProvider {
    /// Create a new provider using `FM_BRIDGE_URL` as override.
    pub fn new() -> Result<Self, ProviderError> {
        let base_url = std::env::var("FM_BRIDGE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());

        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        Ok(Self { client, base_url })
    }

    /// Override base URL (for testing).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[async_trait]
impl LlmProvider for AppleProvider {
    fn id(&self) -> &'static str {
        "apple"
    }

    fn display_name(&self) -> &'static str {
        "Apple FM"
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/health", self.base_url.trim_end_matches('/'));
        match self
            .client
            .get(url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(res) => res.status().is_success(),
            Err(_) => false,
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(model::apple::all())
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
        let adapter = OpenAIStreamAdapter::new(sse_stream, model, "apple");

        Ok(Box::pin(adapter))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            // Tool calling support on Apple FM is limited; disable to avoid sending tools by default.
            tool_calling: false,
            vision: false,
            extended_thinking: false,
            prompt_caching: false,
            interleaved_thinking: false,
            fine_grained_tool_streaming: false,
        }
    }
}
