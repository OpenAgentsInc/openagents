//! LLM Provider abstraction.
//!
//! This module defines the core `LlmProvider` trait that all providers implement,
//! as well as the `ProviderRegistry` for managing multiple providers.

mod anthropic;
mod openrouter;

pub use anthropic::AnthropicProvider;
pub use openrouter::OpenRouterProvider;

use crate::message::CompletionRequest;
use crate::model::ModelInfo;
use crate::stream::CompletionStream;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Core provider trait - all LLM providers implement this.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider identifier (e.g., "anthropic", "openai").
    fn id(&self) -> &'static str;

    /// Display name for UI.
    fn display_name(&self) -> &'static str;

    /// Check if provider is available (has credentials, is reachable).
    async fn is_available(&self) -> bool;

    /// Get available models for this provider.
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    /// Create a streaming completion.
    async fn stream(&self, request: CompletionRequest) -> Result<CompletionStream, ProviderError>;

    /// Provider-specific capabilities.
    fn capabilities(&self) -> ProviderCapabilities;
}

/// Provider capabilities.
#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilities {
    /// Supports streaming responses.
    pub streaming: bool,
    /// Supports tool/function calling.
    pub tool_calling: bool,
    /// Supports vision (image input).
    pub vision: bool,
    /// Supports extended thinking/reasoning.
    pub extended_thinking: bool,
    /// Supports prompt caching.
    pub prompt_caching: bool,
    /// Supports interleaved thinking.
    pub interleaved_thinking: bool,
    /// Supports fine-grained tool streaming.
    pub fine_grained_tool_streaming: bool,
}

/// Errors that can occur with providers.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Missing credentials.
    #[error("Missing credentials: {0}")]
    MissingCredentials(String),

    /// Network error.
    #[error("Network error: {0}")]
    Network(String),

    /// API error with status code.
    #[error("API error ({status}): {message}")]
    ApiError {
        /// HTTP status code.
        status: u16,
        /// Error message.
        message: String,
    },

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Invalid request.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Provider not found.
    #[error("Provider not found: {0}")]
    NotFound(String),

    /// Stream error.
    #[error("Stream error: {0}")]
    Stream(String),

    /// Authentication error.
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Other error.
    #[error("{0}")]
    Other(String),
}

/// Registry of available LLM providers.
pub struct ProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn LlmProvider>>>,
    model_cache: RwLock<HashMap<String, ModelInfo>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            model_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Initialize with default providers.
    ///
    /// Attempts to initialize each provider and registers those with valid credentials.
    pub async fn init_defaults(&self) -> Result<(), ProviderError> {
        // Try Anthropic
        if let Ok(anthropic) = AnthropicProvider::new() {
            if anthropic.is_available().await {
                self.register(Arc::new(anthropic)).await;
                tracing::info!("Registered Anthropic provider");
            }
        }

        // Try OpenRouter
        if let Ok(openrouter) = OpenRouterProvider::new() {
            if openrouter.is_available().await {
                self.register(Arc::new(openrouter)).await;
                tracing::info!("Registered OpenRouter provider");
            }
        }

        // Future: OpenAI, Ollama, etc.

        Ok(())
    }

    /// Register a provider.
    pub async fn register(&self, provider: Arc<dyn LlmProvider>) {
        let id = provider.id().to_string();
        let mut providers = self.providers.write().await;
        providers.insert(id.clone(), provider.clone());

        // Cache models
        if let Ok(models) = provider.list_models().await {
            let mut cache = self.model_cache.write().await;
            for model in models {
                cache.insert(format!("{}/{}", id, model.id), model);
            }
        }
    }

    /// Get a provider by ID.
    pub async fn get(&self, id: &str) -> Option<Arc<dyn LlmProvider>> {
        let providers = self.providers.read().await;
        providers.get(id).cloned()
    }

    /// Get provider for a model spec (e.g., "anthropic/claude-sonnet-4-5-20250929").
    pub async fn provider_for_model(&self, model_spec: &str) -> Option<Arc<dyn LlmProvider>> {
        let (provider_id, _model_id) = parse_model_spec(model_spec)?;
        self.get(&provider_id).await
    }

    /// Get model info for a model spec.
    pub async fn model_info(&self, model_spec: &str) -> Option<ModelInfo> {
        let cache = self.model_cache.read().await;
        cache.get(model_spec).cloned()
    }

    /// List all available provider IDs.
    pub async fn list_available(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        let mut available = Vec::new();

        for (id, provider) in providers.iter() {
            if provider.is_available().await {
                available.push(id.clone());
            }
        }

        available
    }

    /// List all registered provider IDs.
    pub async fn list_all(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }

    /// List all cached models.
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        let cache = self.model_cache.read().await;
        cache.values().cloned().collect()
    }

    /// Stream a completion using the appropriate provider.
    pub async fn stream(
        &self,
        model_spec: &str,
        request: CompletionRequest,
    ) -> Result<CompletionStream, ProviderError> {
        let provider = self
            .provider_for_model(model_spec)
            .await
            .ok_or_else(|| ProviderError::NotFound(model_spec.to_string()))?;

        provider.stream(request).await
    }
}

/// Parse a model spec into (provider_id, model_id).
fn parse_model_spec(spec: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = spec.splitn(2, '/').collect();
    if parts.len() == 2 {
        // Check for OpenRouter-style model IDs (provider/model)
        // These contain a nested provider prefix like "anthropic/claude-3.5-sonnet"
        let first = parts[0];
        let second = parts[1];

        // Known OpenRouter provider prefixes
        let openrouter_prefixes = [
            "anthropic",
            "openai",
            "google",
            "meta-llama",
            "deepseek",
            "mistralai",
            "cohere",
            "perplexity",
            "microsoft",
        ];

        // If first part is an OpenRouter provider prefix and we're using openrouter
        if openrouter_prefixes.contains(&first) {
            // This is an OpenRouter model ID
            return Some(("openrouter".to_string(), spec.to_string()));
        }

        Some((first.to_string(), second.to_string()))
    } else {
        // If no provider specified, try to infer from model name
        let model = parts[0];
        let provider = if model.starts_with("claude") {
            "anthropic"
        } else if model.starts_with("gpt") || model.starts_with("o1") || model.starts_with("o3") {
            "openai"
        } else if model.starts_with("llama") || model.starts_with("qwen") {
            "ollama"
        } else {
            return None;
        };
        Some((provider.to_string(), model.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_spec() {
        // Direct provider/model format - non-OpenRouter providers
        // Note: anthropic/model is detected as OpenRouter format now
        assert_eq!(
            parse_model_spec("ollama/llama3.2"),
            Some(("ollama".to_string(), "llama3.2".to_string()))
        );

        // Bare model names - infer provider
        assert_eq!(
            parse_model_spec("claude-sonnet-4-5-20250929"),
            Some((
                "anthropic".to_string(),
                "claude-sonnet-4-5-20250929".to_string()
            ))
        );

        assert_eq!(
            parse_model_spec("gpt-4o"),
            Some(("openai".to_string(), "gpt-4o".to_string()))
        );

        assert_eq!(
            parse_model_spec("llama3.2"),
            Some(("ollama".to_string(), "llama3.2".to_string()))
        );
    }

    #[test]
    fn test_parse_openrouter_model_spec() {
        // OpenRouter-style model IDs route to openrouter provider
        assert_eq!(
            parse_model_spec("anthropic/claude-3.5-sonnet"),
            Some((
                "openrouter".to_string(),
                "anthropic/claude-3.5-sonnet".to_string()
            ))
        );

        assert_eq!(
            parse_model_spec("openai/gpt-4o"),
            Some(("openrouter".to_string(), "openai/gpt-4o".to_string()))
        );

        assert_eq!(
            parse_model_spec("meta-llama/llama-3.3-70b-instruct"),
            Some((
                "openrouter".to_string(),
                "meta-llama/llama-3.3-70b-instruct".to_string()
            ))
        );

        assert_eq!(
            parse_model_spec("google/gemini-2.0-flash-exp:free"),
            Some((
                "openrouter".to_string(),
                "google/gemini-2.0-flash-exp:free".to_string()
            ))
        );
    }
}
