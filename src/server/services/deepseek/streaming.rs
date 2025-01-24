use crate::server::services::deepseek::types::ToolCallResponse;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct StreamChoice {
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct StreamDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallResponse>>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StreamResponse {
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Clone)]
pub enum StreamUpdate {
    Content(String),
    Reasoning(String),
    ToolCalls(Vec<ToolCallResponse>),
    Done,
}
