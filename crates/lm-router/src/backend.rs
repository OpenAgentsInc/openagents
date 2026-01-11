//! LM backend trait definition.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::usage::LmUsage;

/// Response from an LM backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LmResponse {
    /// The generated text.
    pub text: String,
    /// Token usage statistics.
    pub usage: LmUsage,
    /// The model that generated this response.
    pub model: String,
    /// Reason the generation stopped (e.g., "stop", "length").
    pub finish_reason: Option<String>,
    /// Latency in milliseconds.
    pub latency_ms: u64,
}

impl LmResponse {
    /// Create a new LM response.
    pub fn new(text: impl Into<String>, model: impl Into<String>, usage: LmUsage) -> Self {
        Self {
            text: text.into(),
            model: model.into(),
            usage,
            finish_reason: None,
            latency_ms: 0,
        }
    }

    /// Set the finish reason.
    pub fn with_finish_reason(mut self, reason: impl Into<String>) -> Self {
        self.finish_reason = Some(reason.into());
        self
    }

    /// Set the latency.
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }
}

/// Trait for LM backends.
///
/// Implementations provide access to different LLM inference sources:
/// - Local models (Apple FM, Ollama)
/// - API providers (OpenAI, OpenAI)
/// - Distributed networks (NIP-90 swarm)
#[async_trait]
pub trait LmBackend: Send + Sync {
    /// Get the backend name (e.g., "fm-bridge", "swarm-sim").
    fn name(&self) -> &str;

    /// Get the list of models supported by this backend.
    fn supported_models(&self) -> Vec<String>;

    /// Check if this backend supports a specific model.
    fn supports_model(&self, model: &str) -> bool {
        self.supported_models().iter().any(|m| m == model)
    }

    /// Complete a prompt.
    ///
    /// # Arguments
    ///
    /// * `model` - The model to use for completion
    /// * `prompt` - The prompt to complete
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    ///
    /// The completion response with text, usage, and metadata.
    async fn complete(&self, model: &str, prompt: &str, max_tokens: usize) -> Result<LmResponse>;

    /// Check if the backend is healthy and ready to accept requests.
    async fn health_check(&self) -> bool;
}
