use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionState {
    pub user_id: i32,
    pub tx: tokio::sync::mpsc::UnboundedSender<axum::extract::ws::Message>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatMessage {
    #[serde(rename = "user")]
    UserMessage {
        content: String,
    },
    #[serde(rename = "assistant")]
    AssistantMessage {
        content: String,
    },
    #[serde(rename = "error")]
    ErrorMessage {
        content: String,
    },
}

#[derive(Debug)]
pub enum WebSocketError {
    AuthenticationError(String),
    ConnectionError(String),
    MessageError(String),
}

impl std::fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSocketError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            WebSocketError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            WebSocketError::MessageError(msg) => write!(f, "Message error: {}", msg),
        }
    }
}

impl std::error::Error for WebSocketError {}