//! LmRouter adapter for RlmEngine.
//!
//! This module provides an adapter that allows RlmEngine to use LmRouter
//! instead of FMClient directly, enabling unified backend management and
//! usage tracking across all methods.

use std::sync::Arc;

use async_trait::async_trait;
use lm_router::LmRouter;

use crate::client::{LlmChoice, LlmClient, LlmMessage, LlmResponse, LlmUsage};
use crate::error::{Result, RlmError};

/// Adapter to use LmRouter with RlmEngine.
///
/// This client implements `LlmClient` and routes requests through `LmRouter`,
/// enabling any configured backend (OpenAI, OpenRouter, FM Bridge, etc.).
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use rlm::LmRouterClient;
/// use lm_router::LmRouter;
///
/// let router = Arc::new(LmRouter::builder()
///     .add_backend(backend)
///     .build());
///
/// let client = LmRouterClient::new(router, "model-name");
/// let response = client.complete("Hello, world!", None).await?;
/// ```
#[derive(Clone)]
pub struct LmRouterClient {
    router: Arc<LmRouter>,
    model: String,
    default_max_tokens: usize,
}

impl LmRouterClient {
    /// Create a new LmRouterClient.
    pub fn new(router: Arc<LmRouter>, model: impl Into<String>) -> Self {
        Self {
            router,
            model: model.into(),
            default_max_tokens: 4096,
        }
    }

    /// Set the default max tokens for completions.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.default_max_tokens = max_tokens;
        self
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get a reference to the underlying router.
    pub fn router(&self) -> &Arc<LmRouter> {
        &self.router
    }
}

#[async_trait]
impl LlmClient for LmRouterClient {
    async fn complete(
        &self,
        prompt: &str,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse> {
        let max = max_tokens.unwrap_or(self.default_max_tokens);

        let response = self
            .router
            .complete(&self.model, prompt, max)
            .await
            .map_err(|e| RlmError::LlmError(e.to_string()))?;

        // Convert LmResponse to LlmResponse
        Ok(LlmResponse {
            choices: vec![LlmChoice {
                message: LlmMessage {
                    content: response.text,
                },
            }],
            usage: Some(LlmUsage {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                total_tokens: response.usage.total_tokens,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lm_router::backends::MockBackend;

    #[tokio::test]
    async fn test_lm_router_client() {
        let mock = MockBackend::new()
            .with_model("test-model")
            .with_response("Test response");

        let router = Arc::new(
            LmRouter::builder()
                .add_backend(mock)
                .default_backend("mock")
                .build(),
        );

        let client = LmRouterClient::new(router, "test-model");
        let response = client.complete("Hello", None).await.unwrap();

        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.content, "Test response");
    }

    #[tokio::test]
    async fn test_with_max_tokens() {
        let mock = MockBackend::new()
            .with_model("test-model")
            .with_response("Response");

        let router = Arc::new(
            LmRouter::builder()
                .add_backend(mock)
                .default_backend("mock")
                .build(),
        );

        let client = LmRouterClient::new(router, "test-model").with_max_tokens(2048);

        assert_eq!(client.default_max_tokens, 2048);
    }

    #[tokio::test]
    async fn test_llm_client_trait() {
        let mock = MockBackend::new()
            .with_model("test-model")
            .with_response("Trait test");

        let router = Arc::new(
            LmRouter::builder()
                .add_backend(mock)
                .default_backend("mock")
                .build(),
        );

        let client: Box<dyn LlmClient> = Box::new(LmRouterClient::new(router, "test-model"));
        let response = client.complete("Hello", None).await.unwrap();

        assert_eq!(response.content(), "Trait test");
    }
}
