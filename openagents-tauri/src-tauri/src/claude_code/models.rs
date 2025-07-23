use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub message_type: MessageType,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub tool_info: Option<ToolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    User,
    Assistant,
    ToolUse,
    ToolResult,
    Error,
    Summary,
    Thinking,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub tool_name: String,
    pub tool_use_id: String,
    pub input: HashMap<String, serde_json::Value>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeConversation {
    pub id: String,
    pub project_name: String,
    pub timestamp: DateTime<Utc>,
    pub first_message: String,
    pub message_count: usize,
    pub file_path: String,
    pub working_directory: String,
    pub summary: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClaudeError {
    #[error("Claude Code binary not found")]
    BinaryNotFound,
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

// Incoming message structures for parsing Claude Code output
#[derive(Debug, Deserialize)]
pub struct IncomingMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct OutgoingUserMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub message: UserMessageContent,
}

#[derive(Debug, Serialize)]
pub struct UserMessageContent {
    pub role: String,
    pub content: Vec<ContentItem>,
}

#[derive(Debug, Serialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    pub item_type: String,
    pub text: String,
}