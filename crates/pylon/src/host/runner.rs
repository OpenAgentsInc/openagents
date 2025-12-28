//! Agent runner for host mode
//!
//! Spawns and manages agent-runner subprocesses.
//! Each agent runs in its own process for isolation and fault tolerance.

use crate::db::agents::{Agent, LifecycleState};
use crate::db::PylonDb;
use agent::AgentRegistry;
use anyhow::{anyhow, Result};
use compute::domain::UnifiedIdentity;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Handle to a running agent subprocess
pub struct AgentHandle {
    pub npub: String,
    pub name: String,
    pub state: LifecycleState,
    process: Option<Child>,
}

impl AgentHandle {
    /// Check if the agent process is still running
    pub fn is_running(&self) -> bool {
        if let Some(ref child) = self.process {
            // Try to get exit status without blocking
            match Command::new("kill")
                .args(["-0", &child.id().to_string()])
                .output()
            {
                Ok(output) => output.status.success(),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Stop the agent process
    pub fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Agent runner manages multiple agent subprocesses
pub struct AgentRunner {
    /// Database for persistence
    db: Arc<PylonDb>,
    /// Agent registry
    registry: AgentRegistry,
    /// Path to agent-runner binary
    runner_binary: String,
    /// Running agent handles
    agents: Arc<RwLock<HashMap<String, AgentHandle>>>,
}

impl AgentRunner {
    /// Create a new agent runner
    pub fn new(db: Arc<PylonDb>, _relay_url: String) -> Result<Self> {
        let registry = AgentRegistry::new()?;

        // Find agent-runner binary
        // In development, look in target/debug or target/release
        // In production, should be in PATH
        let runner_binary = std::env::var("PYLON_AGENT_RUNNER")
            .ok()
            .or_else(|| {
                // Try to find in target directory
                let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
                let runner = exe_dir.join("agent-runner");
                if runner.exists() {
                    return Some(runner.to_string_lossy().to_string());
                }
                None
            })
            .unwrap_or_else(|| "agent-runner".to_string());

        Ok(Self {
            db,
            registry,
            runner_binary,
            agents: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Load agents from registry and start active ones
    pub async fn start_all_active(&self) -> Result<()> {
        // Load agents from the registry
        let configs = self.registry.list()?;

        tracing::info!("Found {} agents in registry", configs.len());

        for config in configs {
            if config.state == agent::LifecycleState::Active {
                match self.start_agent(&config.name).await {
                    Ok(_) => {
                        tracing::info!("Started agent: {}", config.name);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to start agent {}: {}", config.name, e);
                    }
                }
            } else {
                tracing::info!(
                    "Skipping {} agent: {}",
                    format!("{:?}", config.state).to_lowercase(),
                    config.name
                );
            }
        }

        Ok(())
    }

    /// Start a specific agent by name
    pub async fn start_agent(&self, name: &str) -> Result<()> {
        // Check if already running
        {
            let agents = self.agents.read().await;
            if let Some(handle) = agents.get(name) {
                if handle.is_running() {
                    return Err(anyhow!("Agent {} is already running", name));
                }
            }
        }

        // Load config from registry
        let config = self.registry.load(name)?;

        if config.state == agent::LifecycleState::Dormant {
            return Err(anyhow!(
                "Agent {} is dormant. Fund it first: pylon agent fund {}",
                name,
                name
            ));
        }

        // Create identity to get npub
        let identity = UnifiedIdentity::from_mnemonic(&config.mnemonic_encrypted, "")
            .map_err(|e| anyhow!("Failed to load identity: {}", e))?;

        let npub = identity
            .npub()
            .map_err(|e| anyhow!("Failed to get npub: {}", e))?;

        // Ensure agent exists in database
        let db_agent = Agent {
            npub: npub.clone(),
            name: config.name.clone(),
            lifecycle_state: to_db_state(config.state),
            balance_sats: 0,
            tick_count: 0,
            last_tick_at: None,
            memory_json: None,
            goals_json: None,
            created_at: now(),
            updated_at: now(),
        };
        self.db.upsert_agent(&db_agent)?;

        // Spawn agent-runner subprocess
        let child = Command::new(&self.runner_binary)
            .args(["--agent", name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn agent-runner: {}", e))?;

        tracing::info!(
            "Spawned agent {} (PID: {})",
            name,
            child.id()
        );

        // Store handle
        {
            let mut agents = self.agents.write().await;
            agents.insert(
                name.to_string(),
                AgentHandle {
                    npub,
                    name: config.name.clone(),
                    state: LifecycleState::Active,
                    process: Some(child),
                },
            );
        }

        Ok(())
    }

    /// Stop a specific agent
    pub async fn stop_agent(&self, name: &str) -> Result<()> {
        let mut agents = self.agents.write().await;

        if let Some(mut handle) = agents.remove(name) {
            handle.stop();
            tracing::info!("Stopped agent: {}", name);
            Ok(())
        } else {
            Err(anyhow!("Agent {} is not running", name))
        }
    }

    /// Stop all agents
    pub async fn stop_all(&self) {
        let mut agents = self.agents.write().await;

        for (name, mut handle) in agents.drain() {
            tracing::info!("Stopping agent: {}", name);
            handle.stop();
        }
    }

    /// List running agents
    pub async fn list_running(&self) -> Vec<(String, String, bool)> {
        let agents = self.agents.read().await;

        agents
            .iter()
            .map(|(name, handle)| (name.clone(), handle.npub.clone(), handle.is_running()))
            .collect()
    }
}

/// Convert agent crate lifecycle state to db lifecycle state
fn to_db_state(state: agent::LifecycleState) -> LifecycleState {
    match state {
        agent::LifecycleState::Spawning => LifecycleState::Embryonic,
        agent::LifecycleState::Active => LifecycleState::Active,
        agent::LifecycleState::LowBalance => LifecycleState::Active, // Still active, just low
        agent::LifecycleState::Hibernating => LifecycleState::Dormant,
        agent::LifecycleState::Dormant => LifecycleState::Dormant,
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
