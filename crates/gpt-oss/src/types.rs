use serde::{Deserialize, Serialize};

/// Request to GPT-OSS Responses API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub stream: bool,
}

/// Response from GPT-OSS Responses API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssResponse {
    pub id: String,
    pub model: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageStats>,
}

/// Usage statistics from API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Streaming chunk from GPT-OSS Responses API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssStreamChunk {
    pub id: String,
    pub model: String,
    pub delta: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssModelInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_context_length")]
    pub context_length: usize,
}

fn default_context_length() -> usize {
    8192
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}
