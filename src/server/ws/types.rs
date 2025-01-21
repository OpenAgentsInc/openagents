use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    #[serde(rename = "chat")]
    Chat(ChatMessage),
    #[serde(rename = "solver")]
    Solver(SolverMessage),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatMessage {
    #[serde(rename = "user_message")]
    UserMessage { content: String },
    #[serde(rename = "agent_response")]
    AgentResponse { content: String },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SolverMessage {
    #[serde(rename = "progress")]
    Progress { stage: String, message: String },
    #[serde(rename = "files_reasoning")]
    FilesReasoning { content: String },
    #[serde(rename = "solution")]
    Solution { content: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "complete")]
    Complete { summary: String },
}