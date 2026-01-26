//! Agent trait - unified interface for all agents

use async_trait::async_trait;
use std::path::Path;
use tokio::sync::mpsc;

use crate::agent::unified::{UnifiedConversationItem, UnifiedEvent};

/// Unified interface for all AI agents
#[allow(dead_code)]
#[async_trait]
pub trait Agent: Send + Sync {
    /// Get the agent identifier
    fn agent_id(&self) -> crate::agent::unified::AgentId;

    /// Connect to the agent for a workspace
    /// Returns the session ID for this connection
    async fn connect(&self, workspace_path: &Path) -> Result<String, String>;

    /// Disconnect from the agent
    async fn disconnect(&self, session_id: &str) -> Result<(), String>;

    /// Start a new session/thread
    async fn start_session(&self, session_id: &str, cwd: &Path) -> Result<(), String>;

    /// Send a user message
    async fn send_message(&self, session_id: &str, text: String) -> Result<(), String>;

    /// Get events stream (normalized to UnifiedEvent)
    /// Returns a receiver that will receive all events from this agent
    fn events_receiver(&self) -> mpsc::Receiver<UnifiedEvent>;

    /// Get conversation items for a session
    async fn get_conversation_items(
        &self,
        session_id: &str,
    ) -> Result<Vec<UnifiedConversationItem>, String>;
}
