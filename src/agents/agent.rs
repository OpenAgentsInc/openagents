use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Agent {
    // Unique identifier for the agent
    pub id: Uuid,
    // Display name of the agent
    pub name: String,
    // Detailed description of the agent's purpose and capabilities
    pub description: String,
    // Nostr public key for agent identification and messaging
    pub pubkey: String,
    // Current operational status (running, stopped, etc)
    pub status: AgentStatus,
    // JSON configuration settings
    pub config: serde_json::Value,
    // Unix timestamp of agent creation
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentInstance {
    // Unix timestamp when this instance started running
    pub created_at: i64,
    // Unix timestamp when this instance stopped running (None if still running)
    pub ended_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AgentStatus {
    Running,
    Stopped,
    Paused,
    Error,
}