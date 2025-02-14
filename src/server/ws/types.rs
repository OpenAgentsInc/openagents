use axum::extract::ws::Message;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ConnectionState {
    pub user_id: i32,
    pub tx: mpsc::UnboundedSender<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatMessage {
    #[serde(rename = "user")]
    UserMessage { content: String },
    #[serde(rename = "assistant")]
    AssistantMessage { content: String },
    #[serde(rename = "error")]
    ErrorMessage { content: String },
}

// New JSON-specific solver types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SolverJsonMessage {
    StateUpdate {
        status: SolverJsonStatus,
        current_file: Option<String>,
        progress: Option<f32>,
        timestamp: String,
    },
    FileAnalysis {
        file_path: String,
        analysis: String,
        timestamp: String,
    },
    ChangeGenerated {
        file_path: String,
        changes: Vec<CodeJsonChange>,
        timestamp: String,
    },
    ChangeApplied {
        file_path: String,
        success: bool,
        error: Option<String>,
        timestamp: String,
    },
    Error {
        message: String,
        timestamp: String,
    },
    #[serde(rename = "solve_demo_repo")]
    SolveDemoRepo {
        timestamp: String,
    },
    #[serde(rename = "solve_repo")]
    SolveRepo {
        repository: String,
        issue_number: i32,
        timestamp: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolverJsonStatus {
    Initializing,
    AnalyzingFiles,
    GeneratingChanges,
    ApplyingChanges,
    Complete,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeJsonChange {
    pub id: String,
    pub search: String,
    pub replace: String,
    pub description: String,
}

#[derive(Debug)]
pub enum WebSocketError {
    AuthenticationError(String),
    NoSession,
    TokenValidationError(String),
    ConnectionError(String),
    MessageError(String),
}

impl std::fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSocketError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            WebSocketError::NoSession => write!(f, "No session error"),
            WebSocketError::TokenValidationError(msg) => {
                write!(f, "Token validation error: {}", msg)
            }
            WebSocketError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            WebSocketError::MessageError(msg) => write!(f, "Message error: {}", msg),
        }
    }
}

impl std::error::Error for WebSocketError {}
