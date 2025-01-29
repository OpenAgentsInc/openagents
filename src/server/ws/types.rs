use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatMessage {
    #[serde(rename = "user")]
    UserMessage {
        content: String,
    },
    #[serde(rename = "ai")]
    AIMessage {
        content: String,
        status: String,
    },
    #[serde(rename = "system")]
    SystemMessage {
        content: String,
        status: String,
    },
}