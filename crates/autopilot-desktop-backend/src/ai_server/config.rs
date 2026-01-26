//! AI Server Configuration
//!
//! Configuration types and utilities for the AI Gateway server.

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiServerConfig {
    /// Server host (default: localhost)
    pub host: String,
    /// Server port (default: 3001)
    pub port: u16,
    /// Vercel AI Gateway API key
    pub api_key: String,
    /// Base URL for Vercel AI Gateway
    pub base_url: String,
    /// Default LLM provider
    pub default_provider: String,
    /// Default LLM model
    pub default_model: String,
    /// Fallback LLM model
    pub fallback_model: String,
}

impl AiServerConfig {
    /// Create a new configuration from environment variables
    pub fn from_env() -> Result<Self, String> {
        let api_key = env::var("AI_GATEWAY_API_KEY")
            .map_err(|_| "AI_GATEWAY_API_KEY environment variable is required".to_string())?;

        if api_key.is_empty() {
            return Err("AI_GATEWAY_API_KEY cannot be empty".to_string());
        }

        Ok(Self {
            host: env::var("AI_SERVER_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("AI_SERVER_PORT")
                .unwrap_or_else(|_| "3001".to_string())
                .parse()
                .map_err(|_| "Invalid AI_SERVER_PORT value".to_string())?,
            api_key,
            base_url: env::var("AI_GATEWAY_BASE_URL")
                .unwrap_or_else(|_| "https://ai-gateway.vercel.sh/v1".to_string()),
            default_provider: env::var("DEFAULT_LLM_PROVIDER")
                .unwrap_or_else(|_| "anthropic".to_string()),
            default_model: env::var("DEFAULT_LLM_MODEL")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-4.5".to_string()),
            fallback_model: env::var("FALLBACK_LLM_MODEL")
                .unwrap_or_else(|_| "openai/gpt-4o".to_string()),
        })
    }

    /// Create a configuration with custom values
    pub fn new(host: String, port: u16, api_key: String) -> Self {
        Self {
            host,
            port,
            api_key,
            base_url: "https://ai-gateway.vercel.sh/v1".to_string(),
            default_provider: "anthropic".to_string(),
            default_model: "anthropic/claude-sonnet-4.5".to_string(),
            fallback_model: "openai/gpt-4o".to_string(),
        }
    }

    /// Get the server endpoint URL
    pub fn server_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    /// Get the health check URL
    pub fn health_url(&self) -> String {
        format!("{}/health", self.server_url())
    }

    /// Get the analytics URL
    pub fn analytics_url(&self) -> String {
        format!("{}/analytics", self.server_url())
    }

    /// Get the chat completions URL
    pub fn chat_url(&self) -> String {
        format!("{}/v1/chat/completions", self.server_url())
    }

    /// Get the DSPy predict URL
    pub fn dspy_url(&self) -> String {
        format!("{}/dspy/predict", self.server_url())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.api_key.is_empty() {
            return Err("API key cannot be empty".to_string());
        }

        if self.port == 0 {
            return Err("Port must be greater than 0".to_string());
        }

        if self.host.is_empty() {
            return Err("Host cannot be empty".to_string());
        }

        Ok(())
    }
}

impl Default for AiServerConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 3001,
            api_key: String::new(),
            base_url: "https://ai-gateway.vercel.sh/v1".to_string(),
            default_provider: "anthropic".to_string(),
            default_model: "anthropic/claude-sonnet-4.5".to_string(),
            fallback_model: "openai/gpt-4o".to_string(),
        }
    }
}

/// DSPy-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspyConfig {
    /// LM endpoint URL
    pub endpoint: String,
    /// Maximum tokens for generation
    pub max_tokens: u32,
    /// Temperature for generation
    pub temperature: f32,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl DspyConfig {
    /// Create DSPy config from AI server config
    pub fn from_ai_server_config(ai_config: &AiServerConfig) -> Self {
        Self {
            endpoint: ai_config.dspy_url(),
            max_tokens: env::var("DSPY_MAX_TOKENS")
                .unwrap_or_else(|_| "4096".to_string())
                .parse()
                .unwrap_or(4096),
            temperature: env::var("DSPY_TEMPERATURE")
                .unwrap_or_else(|_| "0.7".to_string())
                .parse()
                .unwrap_or(0.7),
            timeout_secs: env::var("DSPY_TIMEOUT_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
        }
    }
}

impl Default for DspyConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:3001/dspy/predict".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            timeout_secs: 30,
        }
    }
}
