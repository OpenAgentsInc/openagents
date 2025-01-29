use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub role: String,
    pub content: String,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallResponse>>,
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
    ToolCalls(Vec<ToolCallResponse>),
    Error(String),
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub id: String,
    pub name: String,
    pub arguments: String,
}