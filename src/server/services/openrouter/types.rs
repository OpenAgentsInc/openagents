use serde::{Deserialize, Serialize};
use crate::server::services::gateway::types::Message;

#[derive(Debug, Serialize)]
pub struct OpenRouterRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    pub temperature: f32,
    pub max_tokens: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterResponse {
    pub choices: Vec<OpenRouterChoice>,
}

#[derive(Debug, Deserialize)]
pub struct OpenRouterChoice {
    pub message: Message,
    pub finish_reason: Option<String>,
}