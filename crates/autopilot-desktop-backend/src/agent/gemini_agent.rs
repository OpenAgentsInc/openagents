use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::mpsc;

use crate::agent::acp_agent::AcpAgent;
use crate::agent::trait_def::Agent;
use crate::agent::unified::{AgentId, UnifiedConversationItem, UnifiedEvent};

pub struct GeminiAgent {
    acp_agent: Arc<AcpAgent>,
}

impl GeminiAgent {
    pub fn new(
        workspace_id: String,
        app: AppHandle,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        let acp_agent = Arc::new(AcpAgent::new(
            AgentId::Gemini,
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
impl Agent for GeminiAgent {
    fn agent_id(&self) -> AgentId {
        AgentId::Gemini
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
