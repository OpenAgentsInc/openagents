//! High-level LLM client

use crate::{
    AnthropicProvider, ChatOptions, ChatResponse, ChatStream, LlmProvider, LlmResult, Message,
    ModelInfo, ProviderConfig,
};
use std::sync::Arc;

/// High-level LLM client that wraps providers
pub struct LlmClient {
    provider: Arc<dyn LlmProvider>,
}

impl LlmClient {
    /// Create a client with a custom provider
    pub fn new(provider: impl LlmProvider + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    /// Create an Anthropic client
    pub fn anthropic(api_key: impl Into<String>) -> LlmResult<Self> {
        let config = ProviderConfig::new(api_key);
        let provider = AnthropicProvider::new(config)?;
        Ok(Self::new(provider))
    }

    /// Create an Anthropic client with custom configuration
    pub fn anthropic_with_config(config: ProviderConfig) -> LlmResult<Self> {
        let provider = AnthropicProvider::new(config)?;
        Ok(Self::new(provider))
    }

    /// Get the provider name
    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    /// Get the default model
    pub fn default_model(&self) -> &str {
        self.provider.default_model()
    }

    /// List available models
    pub async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        self.provider.list_models().await
    }

    /// Get information about a specific model
    pub async fn model_info(&self, model: &str) -> LlmResult<ModelInfo> {
        self.provider.model_info(model).await
    }

    /// Send a chat completion request
    pub async fn chat(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatResponse> {
        self.provider.chat(messages, options).await
    }

    /// Send a streaming chat completion request
    pub async fn chat_stream(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatStream> {
        self.provider.chat_stream(messages, options).await
    }

    /// Simple text completion
    pub async fn complete(&self, prompt: &str) -> LlmResult<String> {
        let messages = vec![Message::user(prompt)];
        let response = self.chat(&messages, None).await?;
        Ok(response.text())
    }

    /// Text completion with system prompt
    pub async fn complete_with_system(&self, system: &str, prompt: &str) -> LlmResult<String> {
        let messages = vec![Message::user(prompt)];
        let options = ChatOptions::default().system(system);
        let response = self.chat(&messages, Some(options)).await?;
        Ok(response.text())
    }

    /// Check provider health
    pub async fn health_check(&self) -> LlmResult<bool> {
        self.provider.health_check().await
    }

    /// Count tokens in text
    pub fn count_tokens(&self, text: &str) -> u32 {
        self.provider.count_tokens(text)
    }

    /// Count tokens in messages
    pub fn count_message_tokens(&self, messages: &[Message]) -> u32 {
        self.provider.count_message_tokens(messages)
    }
}

/// Builder for creating LLM clients with configuration
pub struct LlmClientBuilder {
    provider_type: ProviderType,
    config: ProviderConfig,
}

/// Supported provider types
#[derive(Debug, Clone, Copy)]
pub enum ProviderType {
    Anthropic,
    // Future providers:
    // OpenAI,
    // OpenRouter,
    // AppleFM,
}

impl LlmClientBuilder {
    /// Create a builder for Anthropic
    pub fn anthropic(api_key: impl Into<String>) -> Self {
        Self {
            provider_type: ProviderType::Anthropic,
            config: ProviderConfig::new(api_key),
        }
    }

    /// Set the base URL
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.config.base_url = Some(url.into());
        self
    }

    /// Set the default model
    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.config.default_model = Some(model.into());
        self
    }

    /// Set the request timeout
    pub fn timeout(mut self, secs: u64) -> Self {
        self.config.timeout_secs = secs;
        self
    }

    /// Set the max retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = retries;
        self
    }

    /// Build the client
    pub fn build(self) -> LlmResult<LlmClient> {
        match self.provider_type {
            ProviderType::Anthropic => LlmClient::anthropic_with_config(self.config),
        }
    }
}

/// Token counter for cost estimation
pub struct TokenCounter {
    input_tokens: u32,
    output_tokens: u32,
    cache_creation_tokens: u32,
    cache_read_tokens: u32,
}

impl TokenCounter {
    /// Create a new token counter
    pub fn new() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
        }
    }

    /// Add usage from a response
    pub fn add(&mut self, usage: &crate::Usage) {
        self.input_tokens += usage.input_tokens;
        self.output_tokens += usage.output_tokens;
        self.cache_creation_tokens += usage.cache_creation_input_tokens;
        self.cache_read_tokens += usage.cache_read_input_tokens;
    }

    /// Get total input tokens
    pub fn input_tokens(&self) -> u32 {
        self.input_tokens
    }

    /// Get total output tokens
    pub fn output_tokens(&self) -> u32 {
        self.output_tokens
    }

    /// Get total tokens
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }

    /// Estimate cost in USD (for Claude 3.5 Sonnet pricing)
    pub fn estimate_cost_usd(&self) -> f64 {
        // Claude 3.5 Sonnet pricing (as of 2024)
        let input_cost_per_mtok = 3.0; // $3 per 1M input tokens
        let output_cost_per_mtok = 15.0; // $15 per 1M output tokens
        let cache_write_cost_per_mtok = 3.75; // $3.75 per 1M cache write tokens
        let cache_read_cost_per_mtok = 0.30; // $0.30 per 1M cache read tokens

        let input_cost = (self.input_tokens as f64 / 1_000_000.0) * input_cost_per_mtok;
        let output_cost = (self.output_tokens as f64 / 1_000_000.0) * output_cost_per_mtok;
        let cache_write_cost =
            (self.cache_creation_tokens as f64 / 1_000_000.0) * cache_write_cost_per_mtok;
        let cache_read_cost =
            (self.cache_read_tokens as f64 / 1_000_000.0) * cache_read_cost_per_mtok;

        input_cost + output_cost + cache_write_cost + cache_read_cost
    }

    /// Reset the counter
    pub fn reset(&mut self) {
        self.input_tokens = 0;
        self.output_tokens = 0;
        self.cache_creation_tokens = 0;
        self.cache_read_tokens = 0;
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}
