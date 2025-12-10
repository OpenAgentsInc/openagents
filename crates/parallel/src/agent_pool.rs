//! Agent pool management for parallel execution
//!
//! Implements PAR-001..005: Parallel orchestrator

use crate::{ParallelError, ParallelResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info};

/// Configuration for an agent in the pool
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Unique agent identifier
    pub id: String,
    /// Worktree path for this agent
    pub worktree_path: PathBuf,
    /// Branch name
    pub branch: String,
    /// Maximum tasks this agent can handle
    pub max_tasks: Option<usize>,
    /// Whether this agent uses Claude Code or local model
    pub use_claude_code: bool,
}

/// State of an agent in the pool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    /// Agent is idle, waiting for work
    Idle,
    /// Agent is working on a task
    Working,
    /// Agent has completed all assigned tasks
    Completed,
    /// Agent encountered an error
    Failed,
    /// Agent is being shut down
    ShuttingDown,
}

/// Runtime statistics for an agent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStats {
    /// Tasks completed by this agent
    pub tasks_completed: usize,
    /// Tasks failed by this agent
    pub tasks_failed: usize,
    /// Total tokens used
    pub tokens_used: u64,
    /// Total execution time in seconds
    pub execution_time_secs: u64,
    /// When the agent started
    pub started_at: Option<DateTime<Utc>>,
    /// When the agent finished
    pub finished_at: Option<DateTime<Utc>>,
}

/// An agent instance in the pool
#[derive(Debug)]
pub struct PoolAgent {
    /// Configuration
    pub config: AgentConfig,
    /// Current state
    pub state: AgentState,
    /// Runtime statistics
    pub stats: AgentStats,
    /// Current task ID (if working)
    pub current_task: Option<String>,
}

impl PoolAgent {
    /// Create a new pool agent
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            state: AgentState::Idle,
            stats: AgentStats::default(),
            current_task: None,
        }
    }

    /// Check if agent is available for work
    pub fn is_available(&self) -> bool {
        self.state == AgentState::Idle
    }

    /// Mark agent as working on a task
    pub fn start_task(&mut self, task_id: &str) {
        self.state = AgentState::Working;
        self.current_task = Some(task_id.to_string());
        if self.stats.started_at.is_none() {
            self.stats.started_at = Some(Utc::now());
        }
    }

    /// Mark task as completed
    pub fn complete_task(&mut self) {
        self.state = AgentState::Idle;
        self.current_task = None;
        self.stats.tasks_completed += 1;
    }

    /// Mark task as failed
    pub fn fail_task(&mut self) {
        self.state = AgentState::Idle;
        self.current_task = None;
        self.stats.tasks_failed += 1;
    }

    /// Mark agent as failed
    pub fn mark_failed(&mut self) {
        self.state = AgentState::Failed;
        self.stats.finished_at = Some(Utc::now());
    }

    /// Mark agent as completed
    pub fn mark_completed(&mut self) {
        self.state = AgentState::Completed;
        self.stats.finished_at = Some(Utc::now());
    }
}

/// Pool of agents for parallel execution
pub struct AgentPool {
    /// All agents in the pool
    agents: Arc<RwLock<HashMap<String, PoolAgent>>>,
    /// Maximum number of agents
    max_agents: usize,
    /// Channel for task completion notifications
    completion_tx: mpsc::Sender<TaskCompletion>,
    /// Channel for receiving task completions
    completion_rx: Arc<Mutex<mpsc::Receiver<TaskCompletion>>>,
}

/// Notification when a task completes
#[derive(Debug, Clone)]
pub struct TaskCompletion {
    /// Agent that completed the task
    pub agent_id: String,
    /// Task that was completed
    pub task_id: String,
    /// Whether the task succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Tokens used
    pub tokens_used: u64,
}

impl AgentPool {
    /// Create a new agent pool
    ///
    /// PAR-001: Run multiple agents in parallel
    pub fn new(max_agents: usize) -> Self {
        let (completion_tx, completion_rx) = mpsc::channel(100);

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            max_agents,
            completion_tx,
            completion_rx: Arc::new(Mutex::new(completion_rx)),
        }
    }

    /// Add an agent to the pool
    pub async fn add_agent(&self, config: AgentConfig) -> ParallelResult<()> {
        let mut agents = self.agents.write().await;

        if agents.len() >= self.max_agents {
            return Err(ParallelError::InvalidConfig(format!(
                "Pool is full (max {} agents)",
                self.max_agents
            )));
        }

        let id = config.id.clone();
        let agent = PoolAgent::new(config);
        agents.insert(id.clone(), agent);

        info!("Added agent {} to pool ({} total)", id, agents.len());
        Ok(())
    }

    /// Remove an agent from the pool
    pub async fn remove_agent(&self, agent_id: &str) -> ParallelResult<Option<PoolAgent>> {
        let mut agents = self.agents.write().await;
        let agent = agents.remove(agent_id);

        if agent.is_some() {
            info!("Removed agent {} from pool", agent_id);
        }

        Ok(agent)
    }

    /// Get an available agent
    ///
    /// PAR-002: Load balance tasks across agents
    pub async fn get_available_agent(&self) -> Option<String> {
        let agents = self.agents.read().await;

        // Simple strategy: return first idle agent with fewest completed tasks
        agents
            .iter()
            .filter(|(_, a)| a.is_available())
            .min_by_key(|(_, a)| a.stats.tasks_completed)
            .map(|(id, _)| id.clone())
    }

    /// Assign a task to an agent
    pub async fn assign_task(&self, agent_id: &str, task_id: &str) -> ParallelResult<()> {
        let mut agents = self.agents.write().await;

        let agent = agents
            .get_mut(agent_id)
            .ok_or_else(|| ParallelError::agent(format!("Agent {} not found", agent_id)))?;

        if !agent.is_available() {
            return Err(ParallelError::TaskAssignmentError(format!(
                "Agent {} is not available (state: {:?})",
                agent_id, agent.state
            )));
        }

        agent.start_task(task_id);
        debug!("Assigned task {} to agent {}", task_id, agent_id);

        Ok(())
    }

    /// Report task completion
    pub async fn report_completion(&self, completion: TaskCompletion) -> ParallelResult<()> {
        let mut agents = self.agents.write().await;

        if let Some(agent) = agents.get_mut(&completion.agent_id) {
            agent.stats.tokens_used += completion.tokens_used;

            if completion.success {
                agent.complete_task();
            } else {
                agent.fail_task();
            }
        }

        // Send notification
        self.completion_tx
            .send(completion)
            .await
            .map_err(|e| ParallelError::agent(format!("Failed to send completion: {}", e)))?;

        Ok(())
    }

    /// Wait for next task completion
    pub async fn wait_for_completion(&self) -> Option<TaskCompletion> {
        let mut rx = self.completion_rx.lock().await;
        rx.recv().await
    }

    /// Get completion sender for external use
    pub fn completion_sender(&self) -> mpsc::Sender<TaskCompletion> {
        self.completion_tx.clone()
    }

    /// Get pool statistics
    ///
    /// PAR-005: Report progress across all agents
    pub async fn stats(&self) -> PoolStats {
        let agents = self.agents.read().await;

        let mut stats = PoolStats::default();
        stats.total_agents = agents.len();

        for agent in agents.values() {
            match agent.state {
                AgentState::Idle => stats.idle_agents += 1,
                AgentState::Working => stats.working_agents += 1,
                AgentState::Completed => stats.completed_agents += 1,
                AgentState::Failed => stats.failed_agents += 1,
                AgentState::ShuttingDown => {}
            }

            stats.total_tasks_completed += agent.stats.tasks_completed;
            stats.total_tasks_failed += agent.stats.tasks_failed;
            stats.total_tokens_used += agent.stats.tokens_used;
        }

        stats
    }

    /// Get all agent IDs
    pub async fn agent_ids(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }

    /// Check if all agents are done (completed or failed)
    pub async fn all_done(&self) -> bool {
        let agents = self.agents.read().await;
        agents.values().all(|a| {
            matches!(
                a.state,
                AgentState::Completed | AgentState::Failed
            )
        })
    }

    /// Mark all agents as shutting down
    pub async fn shutdown_all(&self) {
        let mut agents = self.agents.write().await;
        for agent in agents.values_mut() {
            if agent.state == AgentState::Idle {
                agent.state = AgentState::ShuttingDown;
            }
        }
    }
}

/// Aggregate statistics for the pool
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PoolStats {
    /// Total agents in the pool
    pub total_agents: usize,
    /// Agents currently idle
    pub idle_agents: usize,
    /// Agents currently working
    pub working_agents: usize,
    /// Agents that completed successfully
    pub completed_agents: usize,
    /// Agents that failed
    pub failed_agents: usize,
    /// Total tasks completed across all agents
    pub total_tasks_completed: usize,
    /// Total tasks failed across all agents
    pub total_tasks_failed: usize,
    /// Total tokens used across all agents
    pub total_tokens_used: u64,
}

impl std::fmt::Display for PoolStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pool: {} agents ({} working, {} idle, {} done, {} failed) | Tasks: {} done, {} failed | Tokens: {}",
            self.total_agents,
            self.working_agents,
            self.idle_agents,
            self.completed_agents,
            self.failed_agents,
            self.total_tasks_completed,
            self.total_tasks_failed,
            self.total_tokens_used
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_pool_creation() {
        let pool = AgentPool::new(4);
        let stats = pool.stats().await;
        assert_eq!(stats.total_agents, 0);
    }

    #[tokio::test]
    async fn test_add_agent() {
        let pool = AgentPool::new(4);

        let config = AgentConfig {
            id: "agent-1".to_string(),
            worktree_path: PathBuf::from("/tmp/wt1"),
            branch: "agent/agent-1".to_string(),
            max_tasks: None,
            use_claude_code: true,
        };

        pool.add_agent(config).await.unwrap();

        let stats = pool.stats().await;
        assert_eq!(stats.total_agents, 1);
        assert_eq!(stats.idle_agents, 1);
    }

    #[tokio::test]
    async fn test_get_available_agent() {
        let pool = AgentPool::new(4);

        // No agents yet
        assert!(pool.get_available_agent().await.is_none());

        // Add agent
        let config = AgentConfig {
            id: "agent-1".to_string(),
            worktree_path: PathBuf::from("/tmp/wt1"),
            branch: "agent/agent-1".to_string(),
            max_tasks: None,
            use_claude_code: true,
        };

        pool.add_agent(config).await.unwrap();

        // Should get agent
        let agent_id = pool.get_available_agent().await;
        assert_eq!(agent_id, Some("agent-1".to_string()));
    }

    #[tokio::test]
    async fn test_task_assignment() {
        let pool = AgentPool::new(4);

        let config = AgentConfig {
            id: "agent-1".to_string(),
            worktree_path: PathBuf::from("/tmp/wt1"),
            branch: "agent/agent-1".to_string(),
            max_tasks: None,
            use_claude_code: true,
        };

        pool.add_agent(config).await.unwrap();

        // Assign task
        pool.assign_task("agent-1", "task-1").await.unwrap();

        // Agent should be working now
        let stats = pool.stats().await;
        assert_eq!(stats.working_agents, 1);
        assert_eq!(stats.idle_agents, 0);

        // Should not be available
        assert!(pool.get_available_agent().await.is_none());
    }
}
