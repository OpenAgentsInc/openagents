use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub delta: StreamMessage,
    pub finish_reason: Option<String>,
    pub index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamResponse {
    pub id: String,
    pub choices: Vec<StreamChoice>,
    pub model: String,
}

#[derive(Debug, Clone)]
pub enum StreamUpdate {
    Content(String),
    ReasoningContent(String),
    Error(String),
    Done,
}