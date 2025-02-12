use crate::server::services::gateway::types::Message;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct OpenRouterRequest {
    pub model: String,
    pub messages: Vec<OpenRouterMessage>,
    pub stream: bool,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenRouterMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl From<Message> for OpenRouterMessage {
    fn from(msg: Message) -> Self {
        OpenRouterMessage {
            role: msg.role,
            content: msg.content,
            name: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterResponse {
    pub id: String,
    pub choices: Vec<OpenRouterChoice>,
    pub model: String,
    pub usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterChoice {
    pub message: OpenRouterMessage,
    pub finish_reason: Option<String>,
    pub index: i32,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterStreamResponse {
    pub id: String,
    pub choices: Vec<OpenRouterStreamChoice>,
    pub model: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterStreamChoice {
    pub delta: OpenRouterDelta,
    pub finish_reason: Option<String>,
    pub index: i32,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterDelta {
    #[serde(default)]
    pub role: String,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterError {
    pub error: OpenRouterErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterErrorDetail {
    pub message: String,
    pub r#type: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Clone)]
pub struct OpenRouterConfig {
    pub temperature: f32,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Option<Vec<String>>,
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIssueAnalysis {
    pub summary: String,
    pub priority: IssuePriority,
    pub estimated_effort: IssueEffort,
    pub tags: Vec<String>,
    pub action_items: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IssuePriority {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IssueEffort {
    #[serde(rename = "small")]
    Small,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "large")]
    Large,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterStructuredResponse<T> {
    pub id: String,
    pub choices: Vec<OpenRouterStructuredChoice<T>>,
    pub model: String,
    pub usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterStructuredChoice<T> {
    pub message: OpenRouterStructuredMessage<T>,
    pub finish_reason: Option<String>,
    pub index: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterStructuredMessage<T> {
    pub role: String,
    pub content: String,
    pub structured_output: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
