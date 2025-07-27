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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    
    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),
    
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

// Unified session history types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    Local,  // From local Claude Code CLI files
    Convex, // From Convex database
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSession {
    pub id: String,
    pub title: String,
    pub timestamp: DateTime<Utc>,
    pub project_path: Option<String>,
    pub working_directory: Option<String>,
    pub first_message: Option<String>,
    pub message_count: Option<usize>,
    pub summary: Option<String>,
    pub source: SessionSource,
    pub file_path: Option<String>, // Only for local sessions
    pub status: Option<String>,    // Only for Convex sessions
    pub created_by: Option<String>, // Only for Convex sessions
}

// Convex session response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexSession {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_creationTime")]
    pub creation_time: f64,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "projectPath")]
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "createdBy")]
    pub created_by: Option<String>,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<f64>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
}

impl From<ClaudeConversation> for UnifiedSession {
    fn from(conv: ClaudeConversation) -> Self {
        Self {
            id: conv.id.clone(),
            title: format!("{} - {}", conv.project_name, 
                          conv.first_message.chars().take(50).collect::<String>()),
            timestamp: conv.timestamp,
            project_path: Some(conv.project_name),
            working_directory: Some(conv.working_directory),
            first_message: Some(conv.first_message),
            message_count: Some(conv.message_count),
            summary: conv.summary,
            source: SessionSource::Local,
            file_path: Some(conv.file_path),
            status: None,
            created_by: None,
        }
    }
}

// Convex message response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexMessage {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_creationTime")]
    pub creation_time: f64,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub content: String,
    pub role: String, // "user", "assistant", "system"
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub status: Option<String>,
    #[serde(rename = "timestamp")]
    pub timestamp: Option<f64>,
}

impl From<ConvexSession> for UnifiedSession {
    fn from(session: ConvexSession) -> Self {
        // Convert creation time from milliseconds to DateTime
        let timestamp = DateTime::from_timestamp_millis(session.creation_time as i64)
            .unwrap_or_else(|| Utc::now());
        
        Self {
            id: session.session_id.clone(),
            title: session.title.unwrap_or_else(|| {
                format!("Session {}", session.session_id.chars().take(8).collect::<String>())
            }),
            timestamp,
            project_path: session.project_path,
            working_directory: None,
            first_message: None,
            message_count: None,
            summary: None,
            source: SessionSource::Convex,
            file_path: None,
            status: session.status,
            created_by: session.created_by,
        }
    }
}