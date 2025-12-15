//! OpenRouter provider implementation.
//!
//! This module provides the OpenRouter provider which offers access to multiple
//! LLM providers through a unified OpenAI-compatible API.
//!
//! OpenRouter uses the OpenAI chat completions format:
//! - Base URL: https://openrouter.ai/api/v1
//! - Auth: Bearer token (OPENROUTER_API_KEY)
//! - Model format: provider/model-name (e.g., "anthropic/claude-3.5-sonnet")

use super::openai_common::{OpenAIStreamAdapter, build_openai_body};
use crate::message::CompletionRequest;
use crate::model::{ModelCapabilities, ModelInfo, ModelLimits, ModelPricing};
use crate::provider::{LlmProvider, ProviderCapabilities, ProviderError};
use crate::stream::{CompletionStream, SseStream};
use async_trait::async_trait;
use reqwest::Client;

/// Default API base URL.
const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// OpenRouter provider.
pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
    base_url: String,
    /// Optional site URL for OpenRouter attribution.
    site_url: Option<String>,
    /// Optional site name for OpenRouter attribution.
    site_name: Option<String>,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider.
    ///
    /// Reads the API key from `OPENROUTER_API_KEY` environment variable.
    pub fn new() -> Result<Self, ProviderError> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| ProviderError::MissingCredentials("OPENROUTER_API_KEY".into()))?;

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url: std::env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
            site_url: std::env::var("OPENROUTER_SITE_URL").ok(),
            site_name: std::env::var("OPENROUTER_SITE_NAME").ok(),
        })
    }

    /// Create with a custom API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.into(),
            site_url: None,
            site_name: None,
        }
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set site URL for attribution.
    pub fn site_url(mut self, url: impl Into<String>) -> Self {
        self.site_url = Some(url.into());
        self
    }

    /// Set site name for attribution.
    pub fn site_name(mut self, name: impl Into<String>) -> Self {
        self.site_name = Some(name.into());
        self
    }

    /// Build the request body for the OpenRouter API (OpenAI format).
    fn build_request(
        &self,
        request: &CompletionRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        build_openai_body(request)
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn id(&self) -> &'static str {
        "openrouter"
    }

    fn display_name(&self) -> &'static str {
        "OpenRouter"
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(openrouter_models())
    }

    async fn stream(&self, request: CompletionRequest) -> Result<CompletionStream, ProviderError> {
        let body = self.build_request(&request)?;
        let model = request.model.clone();

        let mut req = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        // Add optional OpenRouter headers
        if let Some(site_url) = &self.site_url {
            req = req.header("HTTP-Referer", site_url);
        }
        if let Some(site_name) = &self.site_name {
            req = req.header("X-Title", site_name);
        }

        let response = req
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

        // Create SSE stream and wrap in adapter
        let sse_stream = SseStream::new(response.bytes_stream());
        let adapter = OpenAIStreamAdapter::new(sse_stream, model, "openrouter");

        Ok(Box::pin(adapter))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            extended_thinking: false, // Depends on underlying model
            prompt_caching: false,    // Not directly supported
            interleaved_thinking: false,
            fine_grained_tool_streaming: true,
        }
    }
}

// OpenRouter Model Definitions
// ============================================================================

/// Get available OpenRouter models.
///
/// OpenRouter provides access to many models. This returns commonly used ones.
/// Users can still use any model available on OpenRouter by specifying the full ID.
fn openrouter_models() -> Vec<ModelInfo> {
    vec![
        // Anthropic via OpenRouter
        ModelInfo::builder("anthropic/claude-3.5-sonnet", "openrouter")
            .name("Claude 3.5 Sonnet (OpenRouter)")
            .family("claude-3.5")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(3.0, 15.0))
            .limits(ModelLimits::new(200_000, 8_192))
            .build(),
        // OpenAI via OpenRouter
        ModelInfo::builder("openai/gpt-4o", "openrouter")
            .name("GPT-4o (OpenRouter)")
            .family("gpt-4o")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(2.50, 10.0))
            .limits(ModelLimits::new(128_000, 16_384))
            .build(),
        // Google via OpenRouter
        ModelInfo::builder("google/gemini-2.0-flash-exp:free", "openrouter")
            .name("Gemini 2.0 Flash (Free)")
            .family("gemini-2.0")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(0.0, 0.0)) // Free tier
            .limits(ModelLimits::new(1_000_000, 8_192))
            .build(),
        // DeepSeek via OpenRouter
        ModelInfo::builder("deepseek/deepseek-chat", "openrouter")
            .name("DeepSeek Chat")
            .family("deepseek")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: false,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(0.14, 0.28))
            .limits(ModelLimits::new(64_000, 8_192))
            .build(),
        // Meta Llama via OpenRouter
        ModelInfo::builder("meta-llama/llama-3.3-70b-instruct", "openrouter")
            .name("Llama 3.3 70B")
            .family("llama-3.3")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: false,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: false,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(0.35, 0.40))
            .limits(ModelLimits::new(128_000, 8_192))
            .build(),
    ]
}

#[cfg(test)]
mod tests {
    use crate::message::{Message, Tool, ToolChoice};
    use crate::provider::openai_common::{
        transform_messages, transform_tool_choice, transform_tools,
    };

    #[test]
    fn test_transform_messages() {
        let messages = vec![Message::user("Hello"), Message::assistant("Hi there!")];

        let result = transform_messages(&messages, Some("You are helpful")).unwrap();
        let arr = result.as_array().unwrap();

        // Should have system + user + assistant
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["role"], "system");
        assert_eq!(arr[1]["role"], "user");
        assert_eq!(arr[2]["role"], "assistant");
    }

    #[test]
    fn test_transform_tools() {
        let tools = vec![Tool::new(
            "test_tool",
            "A test tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "arg": { "type": "string" }
                }
            }),
        )];

        let result = transform_tools(&tools).unwrap();
        let arr = result.as_array().unwrap();

        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "test_tool");
    }

    #[test]
    fn test_transform_tool_choice() {
        assert_eq!(transform_tool_choice(&ToolChoice::Auto).unwrap(), "auto");
        assert_eq!(transform_tool_choice(&ToolChoice::None).unwrap(), "none");
        assert_eq!(
            transform_tool_choice(&ToolChoice::Required).unwrap(),
            "required"
        );
    }
}
