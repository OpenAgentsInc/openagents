//! Agent Manager - manages multiple agents and provides unified event stream

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

use crate::agent::acp_agent::AcpAgent;
use crate::agent::adjutant::AdjutantAgent;
use crate::agent::resolver::{resolve_codex_config, resolve_gemini_config};
use crate::agent::trait_def::Agent;
use crate::agent::unified::{AgentId, UnifiedEvent};
use tauri::{AppHandle, Emitter};

/// Manages multiple agents and provides a unified interface
pub struct AgentManager {
    agents: Arc<Mutex<HashMap<AgentId, Arc<dyn Agent>>>>,
    active_sessions: Arc<Mutex<HashMap<String, AgentId>>>, // session_id -> agent_id
    unified_events_tx: Arc<broadcast::Sender<UnifiedEvent>>,
}

impl AgentManager {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);

        Self {
            agents: Arc::new(Mutex::new(HashMap::new())),
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            unified_events_tx: Arc::new(tx),
        }
    }

    /// Register an agent
    #[allow(dead_code)]
    pub async fn register_agent(&self, agent_id: AgentId, agent: Arc<dyn Agent>) {
        self.agents.lock().await.insert(agent_id, agent);
    }

    /// Get unified events broadcast receiver
    pub fn get_unified_events_receiver(&self) -> broadcast::Receiver<UnifiedEvent> {
        self.unified_events_tx.subscribe()
    }

    /// Connect an agent for a workspace
    pub async fn connect_agent(
        &self,
        agent_id: AgentId,
        workspace_path: &Path,
        workspace_id: String,
        app: AppHandle,
        codex_home: Option<std::path::PathBuf>,
    ) -> Result<String, String> {
        match agent_id {
            AgentId::Adjutant => {
                // Native DSPy agent - no ACP needed
                let agent = Arc::new(AdjutantAgent::new());
                let session_id = agent.connect(workspace_path).await?;

                let agent_dyn: Arc<dyn Agent> = agent.clone();

                // Store agent and session mapping
                self.agents.lock().await.insert(agent_id, agent_dyn.clone());
                self.active_sessions
                    .lock()
                    .await
                    .insert(session_id.clone(), agent_id);

                // Set up event forwarding for Adjutant
                let unified_tx = self.unified_events_tx.clone();
                let mut agent_rx = agent_dyn.events_receiver();

                tokio::spawn(async move {
                    while let Some(event) = agent_rx.recv().await {
                        let _ = unified_tx.send(event);
                    }
                });

                let app_clone = app.clone();
                let mut ui_rx = agent.ui_events_receiver().await;
                tokio::spawn(async move {
                    while let Some(event) = ui_rx.recv().await {
                        let _ = app_clone.emit("ui-event", &event);
                    }
                });

                Ok(session_id)
            }
            AgentId::Codex | AgentId::Gemini => {
                // ACP-based agents
                let (command, args, env) = match agent_id {
                    AgentId::Codex => resolve_codex_config(codex_home)
                        .await
                        .map_err(|e| format!("Failed to resolve Codex: {}", e))?,
                    AgentId::Gemini => resolve_gemini_config()
                        .await
                        .map_err(|e| format!("Failed to resolve Gemini: {}", e))?,
                    _ => unreachable!(),
                };

                let agent = Arc::new(AcpAgent::new(
                    agent_id,
                    workspace_id.clone(),
                    app.clone(),
                    command,
                    args,
                    env,
                ));

                let session_id = agent.connect(workspace_path).await?;

                // Store agent
                self.agents.lock().await.insert(agent_id, agent.clone());

                // Store session mapping
                self.active_sessions
                    .lock()
                    .await
                    .insert(session_id.clone(), agent_id);

                // Set up event forwarding from agent to unified stream
                let unified_tx = self.unified_events_tx.clone();
                let agent_broadcast_rx = agent.events_broadcast_receiver();

                tokio::spawn(async move {
                    let mut rx = agent_broadcast_rx;
                    loop {
                        match rx.recv().await {
                            Ok(event) => {
                                let _ = unified_tx.send(event);
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                eprintln!("Agent event channel closed");
                                break;
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                eprintln!("Agent event channel lagged by {} messages", n);
                            }
                        }
                    }
                });

                Ok(session_id)
            }
            _ => Err(format!("Agent {:?} not yet implemented", agent_id)),
        }
    }

    /// Get agent for a session
    pub async fn get_agent_for_session(&self, session_id: &str) -> Option<AgentId> {
        self.active_sessions.lock().await.get(session_id).copied()
    }

    /// Update session mapping (e.g., when actual ACP session ID is received)
    #[allow(dead_code)]
    pub async fn update_session_mapping(&self, old_session_id: &str, new_session_id: &str) {
        let mut sessions = self.active_sessions.lock().await;
        if let Some(agent_id) = sessions.remove(old_session_id) {
            sessions.insert(new_session_id.to_string(), agent_id);
            eprintln!(
                "Updated session mapping: {} -> {} (agent: {:?})",
                old_session_id, new_session_id, agent_id
            );
        }
    }

    /// Get agent by ID
    pub async fn get_agent(&self, agent_id: AgentId) -> Option<Arc<dyn Agent>> {
        self.agents.lock().await.get(&agent_id).cloned()
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}
