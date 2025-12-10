/// Types for Foundation Model API requests and responses

use serde::{Deserialize, Serialize};

// ============================================================================
// Request Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CompletionOptions {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: i32,
    pub message: ChatMessage,
    #[serde(default)]
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    #[serde(rename = "tool_calls")]
    ToolCalls,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

// ============================================================================
// Streaming Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub text: String,
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StreamChoice {
    delta: Option<StreamDelta>,
    finish_reason: Option<FinishReason>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StreamDelta {
    content: Option<String>,
    #[allow(dead_code)]
    role: Option<String>,
}

impl StreamResponse {
    pub fn into_chunk(self) -> Option<StreamChunk> {
        self.choices.first().map(|choice| StreamChunk {
            text: choice.delta.as_ref().and_then(|d| d.content.clone()).unwrap_or_default(),
            finish_reason: choice.finish_reason,
        })
    }
}

// ============================================================================
// Model Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

// ============================================================================
// Health Check
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}
