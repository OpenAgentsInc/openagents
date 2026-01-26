//! Codex-specific agent implementation
//!
//! This provides a Codex agent using the unified architecture.
//! For now, it wraps the existing codex app-server, but will migrate to codex-acp.

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::mpsc;

use crate::agent::acp_agent::AcpAgent;
use crate::agent::trait_def::Agent;
use crate::agent::unified::{AgentId, UnifiedConversationItem, UnifiedEvent};

use std::collections::HashMap;

/// Codex agent implementation
///
/// Currently uses ACP (codex-acp) for unified protocol support.
/// In the future, this will be the primary interface for Codex.
#[allow(dead_code)]
pub struct CodexAgent {
    acp_agent: Arc<AcpAgent>,
}

impl CodexAgent {
    #[allow(dead_code)]
    pub fn new(
        workspace_id: String,
        app: AppHandle,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        let acp_agent = Arc::new(AcpAgent::new(
            AgentId::Codex,
            workspace_id,
            app,
            command,
            args,
            env,
        ));

        Self { acp_agent }
    }
}

#[async_trait]
impl Agent for CodexAgent {
    fn agent_id(&self) -> AgentId {
        AgentId::Codex
    }

    async fn connect(&self, workspace_path: &Path) -> Result<String, String> {
        self.acp_agent.connect(workspace_path).await
    }

    async fn disconnect(&self, session_id: &str) -> Result<(), String> {
        self.acp_agent.disconnect(session_id).await
    }

    async fn start_session(&self, session_id: &str, cwd: &Path) -> Result<(), String> {
        self.acp_agent.start_session(session_id, cwd).await
    }

    async fn send_message(&self, session_id: &str, text: String) -> Result<(), String> {
        self.acp_agent.send_message(session_id, text).await
    }

    fn events_receiver(&self) -> mpsc::Receiver<UnifiedEvent> {
        self.acp_agent.events_receiver()
    }

    async fn get_conversation_items(
        &self,
        session_id: &str,
    ) -> Result<Vec<UnifiedConversationItem>, String> {
        self.acp_agent.get_conversation_items(session_id).await
    }
}
