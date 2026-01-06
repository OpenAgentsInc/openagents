//! LmRouter adapter for RlmEngine.
//!
//! This module provides an adapter that allows RlmEngine to use LmRouter
//! instead of FMClient directly, enabling unified backend management and
//! usage tracking across all methods.

#[cfg(feature = "lm-router")]
use std::sync::Arc;

#[cfg(feature = "lm-router")]
use lm_router::LmRouter;

#[cfg(feature = "lm-router")]
use crate::error::{Result, RlmError};

/// Adapter to use LmRouter with RlmEngine.
///
/// RlmEngine expects an FMClient-like interface, but we want to route through
/// LmRouter for unified backend management and usage tracking.
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
#[cfg(feature = "lm-router")]
#[derive(Clone)]
pub struct LmRouterClient {
    router: Arc<LmRouter>,
    model: String,
    default_max_tokens: usize,
}

#[cfg(feature = "lm-router")]
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

    /// Complete a prompt.
    ///
    /// This method is designed to match the FMClient interface so it can be
    /// used as a drop-in replacement in RlmEngine.
    pub async fn complete(
        &self,
        prompt: &str,
        max_tokens: Option<usize>,
    ) -> Result<LmRouterResponse> {
        let max = max_tokens.unwrap_or(self.default_max_tokens);

        let response = self
            .router
            .complete(&self.model, prompt, max)
            .await
            .map_err(|e| RlmError::LlmError(e.to_string()))?;

        // Convert LmResponse to LmRouterResponse (FMClient-compatible format)
        Ok(LmRouterResponse {
            choices: vec![LmRouterChoice {
                message: LmRouterMessage {
                    content: response.text,
                },
            }],
            usage: LmRouterUsage {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                total_tokens: response.usage.total_tokens,
            },
        })
    }
}

/// Response from LmRouterClient, matching FMClient format.
#[cfg(feature = "lm-router")]
#[derive(Debug, Clone)]
pub struct LmRouterResponse {
    pub choices: Vec<LmRouterChoice>,
    pub usage: LmRouterUsage,
}

/// A choice in the response.
#[cfg(feature = "lm-router")]
#[derive(Debug, Clone)]
pub struct LmRouterChoice {
    pub message: LmRouterMessage,
}

/// A message in the response.
#[cfg(feature = "lm-router")]
#[derive(Debug, Clone)]
pub struct LmRouterMessage {
    pub content: String,
}

/// Usage statistics.
#[cfg(feature = "lm-router")]
#[derive(Debug, Clone)]
pub struct LmRouterUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

#[cfg(all(test, feature = "lm-router"))]
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
}
