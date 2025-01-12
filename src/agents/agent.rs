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
    // Whether this agent definition is enabled for creating new instances
    pub enabled: bool,
    // JSON configuration settings
    pub config: serde_json::Value,
    // Unix timestamp of agent creation
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentInstance {
    // Unique identifier for this instance
    pub id: Uuid,
    // Reference to the parent agent
    pub agent_id: Uuid,
    // Current operational status of this instance
    pub status: InstanceStatus,
    // Unix timestamp when this instance started running
    pub created_at: i64,
    // Unix timestamp when this instance stopped running (None if still running)
    pub ended_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum InstanceStatus {
    Starting,
    Running,
    Paused,
    Stopping,
    Stopped,
    Error,
}