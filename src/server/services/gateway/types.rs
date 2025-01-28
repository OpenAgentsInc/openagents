use serde::{Deserialize, Serialize};

/// Metadata describing a gateway's capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayMetadata {
    /// Name of the gateway provider
    pub name: String,
    /// Whether this gateway is OpenAI API compatible
    pub openai_compatible: bool,
    /// List of supported features (e.g. "chat", "streaming", "tools")
    pub supported_features: Vec<String>,
    /// Default model to use if none specified
    pub default_model: String,
    /// List of available models
    pub available_models: Vec<String>,
}

/// Common message type used across gateways
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Common chat request type used across gateways
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    pub temperature: f32,
    pub max_tokens: Option<i32>,
}
