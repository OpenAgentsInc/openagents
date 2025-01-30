use serde::{Deserialize, Serialize};
use crate::server::services::gateway::types::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            model: "llama2".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaChatResponse {
    pub model: String,
    pub message: Message,
    pub done: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}