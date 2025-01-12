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
pub struct Plan {
    // Unique identifier for the plan
    pub id: Uuid,
    // Reference to the agent that created this plan
    pub agent_id: Uuid,
    // Human readable name of the plan
    pub name: String,
    // Detailed description of what this plan aims to accomplish
    pub description: String,
    // Current status of the plan
    pub status: PlanStatus,
    // Ordered list of task IDs that make up this plan
    pub task_ids: Vec<Uuid>,
    // Unix timestamp when plan was created
    pub created_at: i64,
    // Unix timestamp when plan was completed or cancelled
    pub ended_at: Option<i64>,
    // JSON metadata specific to this plan
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    // Unique identifier for the task
    pub id: Uuid,
    // Reference to the plan this task belongs to
    pub plan_id: Uuid,
    // Reference to the agent instance executing this task
    pub instance_id: Uuid,
    // Type of task (e.g., "analyze_data", "send_email", etc)
    pub task_type: String,
    // Current status of the task
    pub status: TaskStatus,
    // Priority level (higher number = higher priority)
    pub priority: u8,
    // Input data required for the task
    pub input: serde_json::Value,
    // Output data produced by the task
    pub output: Option<serde_json::Value>,
    // Unix timestamp when task was created
    pub created_at: i64,
    // Unix timestamp when task execution started
    pub started_at: Option<i64>,
    // Unix timestamp when task completed or failed
    pub ended_at: Option<i64>,
    // Error message if task failed
    pub error: Option<String>,
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

#[derive(Debug, Serialize, Deserialize)]
pub enum PlanStatus {
    Created,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Scheduled,
    Running,
    Completed,
    Failed,
    Cancelled,
}