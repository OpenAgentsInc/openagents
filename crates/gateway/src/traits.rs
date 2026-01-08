use async_trait::async_trait;

use crate::error::Result;
use crate::inference::types::{ChatRequest, ChatResponse};
use crate::types::{Capability, GatewayHealth, ModelInfo};

/// Base trait for all gateways
pub trait Gateway: Send + Sync {
    /// Gateway type identifier (e.g., "inference", "embedding")
    fn gateway_type(&self) -> &str;

    /// Provider name (e.g., "cerebras", "openai")
    fn provider(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Check if the gateway is properly configured
    fn is_configured(&self) -> bool;

    /// Get capabilities this gateway provides
    fn capabilities(&self) -> Vec<Capability>;
}

/// Trait for inference gateways (LLM providers)
#[async_trait]
pub trait InferenceGateway: Gateway {
    /// List available models
    async fn models(&self) -> Result<Vec<ModelInfo>>;

    /// Perform a chat completion
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;

    /// Check gateway health
    async fn health(&self) -> GatewayHealth;
}
