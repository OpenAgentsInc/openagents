use serde::{Deserialize, Serialize};

/// Capabilities a gateway can provide
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    TextGeneration,
    ChatCompletion,
    Streaming,
    FunctionCalling,
    Vision,
    Embedding,
    ImageGeneration,
    SpeechToText,
    TextToSpeech,
    Reasoning,
}

/// Gateway health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayHealth {
    /// Whether the gateway is available
    pub available: bool,
    /// Latency of health check in milliseconds
    pub latency_ms: Option<u64>,
    /// Error message if unavailable
    pub error: Option<String>,
    /// Timestamp of last check (Unix timestamp)
    pub last_check: i64,
}

impl Default for GatewayHealth {
    fn default() -> Self {
        Self {
            available: false,
            latency_ms: None,
            error: None,
            last_check: 0,
        }
    }
}

/// Model pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// USD per million input tokens
    pub input_per_million: f64,
    /// USD per million output tokens
    pub output_per_million: f64,
}

/// Information about a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g., "zai-glm-4.7")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Provider name
    pub provider: String,
    /// Maximum context length in tokens
    pub context_length: u32,
    /// Capabilities this model supports
    pub capabilities: Vec<Capability>,
    /// Pricing information
    pub pricing: Option<ModelPricing>,
}
