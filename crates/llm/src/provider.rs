//! LLM provider trait and common types

use crate::{ChatResponse, LlmResult, Message, ToolChoice, ToolDefinition, Usage};
use async_trait::async_trait;
use std::pin::Pin;
use futures::Stream;

/// A streaming chunk from an LLM response
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// Start of response with metadata
    Start {
        id: String,
        model: String,
    },
    /// Text delta
    Text(String),
    /// Tool use start
    ToolUseStart {
        id: String,
        name: String,
    },
    /// Tool input delta (JSON fragment)
    ToolInputDelta(String),
    /// Tool use complete
    ToolUseEnd,
    /// Response complete with usage
    Done {
        stop_reason: Option<crate::StopReason>,
        usage: Usage,
    },
    /// Error during streaming
    Error(String),
}

/// Chat completion options
#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    /// Model to use (if not set, uses provider default)
    pub model: Option<String>,
    /// System prompt (prepended to messages)
    pub system: Option<String>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    /// Top-p sampling
    pub top_p: Option<f32>,
    /// Stop sequences
    pub stop_sequences: Vec<String>,
    /// Available tools
    pub tools: Vec<ToolDefinition>,
    /// Tool choice configuration
    pub tool_choice: Option<ToolChoice>,
    /// Request timeout in seconds
    pub timeout_secs: Option<u64>,
    /// Whether to enable thinking/reasoning (for models that support it)
    pub enable_thinking: bool,
    /// Custom metadata
    pub metadata: Option<serde_json::Value>,
}

impl ChatOptions {
    /// Create options with a specific model
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: Some(model.into()),
            ..Default::default()
        }
    }

    /// Set the system prompt
    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Set max tokens
    pub fn max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = Some(max);
        self
    }

    /// Set temperature
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set top-p
    pub fn top_p(mut self, p: f32) -> Self {
        self.top_p = Some(p);
        self
    }

    /// Add a stop sequence
    pub fn stop_sequence(mut self, seq: impl Into<String>) -> Self {
        self.stop_sequences.push(seq.into());
        self
    }

    /// Set tools
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    /// Set tool choice
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Set timeout
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Enable thinking/reasoning
    pub fn with_thinking(mut self) -> Self {
        self.enable_thinking = true;
        self
    }
}

/// Stream type for streaming responses
pub type ChatStream = Pin<Box<dyn Stream<Item = LlmResult<StreamChunk>> + Send>>;

/// Model information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Maximum context length in tokens
    pub context_length: u32,
    /// Provider-specific capabilities
    pub capabilities: ModelCapabilities,
}

/// Model capabilities
#[derive(Debug, Clone, Default)]
pub struct ModelCapabilities {
    /// Supports function/tool calling
    pub tool_use: bool,
    /// Supports vision (image input)
    pub vision: bool,
    /// Supports streaming
    pub streaming: bool,
    /// Supports extended thinking
    pub thinking: bool,
    /// Input cost per 1M tokens (in USD)
    pub input_cost_per_mtok: Option<f64>,
    /// Output cost per 1M tokens (in USD)
    pub output_cost_per_mtok: Option<f64>,
}

/// Provider trait for LLM implementations
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &'static str;

    /// Get the default model for this provider
    fn default_model(&self) -> &str;

    /// List available models
    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>>;

    /// Get information about a specific model
    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo>;

    /// Send a chat completion request
    async fn chat(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatResponse>;

    /// Send a streaming chat completion request
    async fn chat_stream(
        &self,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> LlmResult<ChatStream>;

    /// Check if the provider is healthy/reachable
    async fn health_check(&self) -> LlmResult<bool>;

    /// Count tokens in text (approximation if not available)
    fn count_tokens(&self, text: &str) -> u32 {
        // Default approximation: ~4 characters per token
        (text.len() / 4) as u32
    }

    /// Count tokens in messages
    fn count_message_tokens(&self, messages: &[Message]) -> u32 {
        messages.iter().map(|m| self.count_tokens(&m.text())).sum()
    }
}

/// Provider configuration
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// API key
    pub api_key: String,
    /// Base URL (for custom endpoints)
    pub base_url: Option<String>,
    /// Organization ID (for some providers)
    pub organization_id: Option<String>,
    /// Default model to use
    pub default_model: Option<String>,
    /// Default max tokens
    pub default_max_tokens: Option<u32>,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retries on failure
    pub max_retries: u32,
}

impl ProviderConfig {
    /// Create a new config with API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: None,
            organization_id: None,
            default_model: None,
            default_max_tokens: None,
            timeout_secs: 120,
            max_retries: 3,
        }
    }

    /// Create config for Anthropic from environment variables
    ///
    /// Reads from:
    /// - `ANTHROPIC_API_KEY` (required)
    /// - `ANTHROPIC_BASE_URL` (optional)
    /// - `ANTHROPIC_MODEL` (optional, default: claude-sonnet-4-20250514)
    pub fn anthropic_from_env() -> Option<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok()?;
        let mut config = Self::new(api_key);

        if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
            config = config.base_url(base_url);
        }
        if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
            config = config.default_model(model);
        }

        Some(config)
    }

    /// Create config for OpenAI from environment variables
    ///
    /// Reads from:
    /// - `OPENAI_API_KEY` (required)
    /// - `OPENAI_BASE_URL` (optional)
    /// - `OPENAI_ORGANIZATION` (optional)
    /// - `OPENAI_MODEL` (optional, default: gpt-4o)
    pub fn openai_from_env() -> Option<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").ok()?;
        let mut config = Self::new(api_key);

        if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
            config = config.base_url(base_url);
        }
        if let Ok(org) = std::env::var("OPENAI_ORGANIZATION") {
            config.organization_id = Some(org);
        }
        if let Ok(model) = std::env::var("OPENAI_MODEL") {
            config = config.default_model(model);
        }

        Some(config)
    }

    /// Create config for any provider from its environment variable name
    ///
    /// For provider "foo", reads FOO_API_KEY, FOO_BASE_URL, FOO_MODEL
    pub fn from_env(provider_name: &str) -> Option<Self> {
        let prefix = provider_name.to_uppercase();
        let api_key = std::env::var(format!("{}_API_KEY", prefix)).ok()?;
        let mut config = Self::new(api_key);

        if let Ok(base_url) = std::env::var(format!("{}_BASE_URL", prefix)) {
            config = config.base_url(base_url);
        }
        if let Ok(model) = std::env::var(format!("{}_MODEL", prefix)) {
            config = config.default_model(model);
        }

        Some(config)
    }

    /// Set base URL
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set default model
    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Set timeout
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set max retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}
