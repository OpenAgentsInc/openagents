use serde::{Deserialize, Serialize};
use crate::server::services::gateway::types::Message;

#[derive(Debug, Serialize)]
pub struct OpenRouterRequest {
    pub model: String,
    pub messages: Vec<OpenRouterMessage>,
    pub stream: bool,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
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