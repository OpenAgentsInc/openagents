//! Parallel orchestrator for running multiple agents
//!
//! Implements PAR-001..005: Parallel orchestrator

use crate::{
    AgentConfig, AgentPool, ContainerAgentConfig, ContainerManager, ParallelError, ParallelResult,
    PoolStats, TaskCompletion, WorktreeManager,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use taskmaster::{IssueFilter, IssueRepository};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Actor ID for parallel orchestrator operations
const PARALLEL_ACTOR: &str = "parallel-orchestrator";

/// Configuration for parallel execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelConfig {
    /// Maximum number of agents to run
    pub max_agents: usize,
    /// Maximum tasks per agent (None = unlimited)
    pub max_tasks_per_agent: Option<usize>,
    /// Maximum total tasks (None = unlimited)
    pub max_total_tasks: Option<usize>,
    /// Maximum tokens per agent
    pub max_tokens_per_agent: Option<u64>,
    /// Maximum duration in seconds
    pub max_duration_secs: Option<u64>,
    /// Whether to automatically merge completed work (only for worktree mode)
    pub auto_merge: bool,
    /// Whether to use Claude Code (true) or local model (false)
    pub use_claude_code: bool,
    /// Safe mode - no destructive operations
    pub safe_mode: bool,
    /// Dry run mode - don't execute tools
    pub dry_run: bool,

    // Container execution fields

    /// Use container isolation (default: true)
    /// When true, each agent runs in an isolated container with a fresh git clone.
    /// When false, uses legacy worktree approach (shared git object database).
    #[serde(default = "default_true")]
    pub use_containers: bool,
    /// Docker image for agent containers (default: "openagents/agent:latest")
    #[serde(default = "default_container_image")]
    pub container_image: String,
    /// Git remote URL for cloning (required for container mode)
    #[serde(default)]
    pub git_remote_url: String,
    /// Memory limit per container (e.g., "8G")
    #[serde(default = "default_container_memory")]
    pub container_memory: Option<String>,
    /// CPU limit per container
    #[serde(default = "default_container_cpus")]
    pub container_cpus: Option<f32>,
}

fn default_true() -> bool {
    true
}

fn default_container_image() -> String {
    "openagents/agent:latest".to_string()
}

fn default_container_memory() -> Option<String> {
    Some("8G".to_string())
}

fn default_container_cpus() -> Option<f32> {
    Some(2.0)
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_agents: 2,
            max_tasks_per_agent: None,
            max_total_tasks: None,
            max_tokens_per_agent: None,
            max_duration_secs: None,
            auto_merge: true,
            use_claude_code: true,
            safe_mode: false,
            dry_run: false,
            use_containers: true,
            container_image: default_container_image(),
            git_remote_url: String::new(),
            container_memory: default_container_memory(),
            container_cpus: default_container_cpus(),
        }
    }
}

/// State of the parallel orchestrator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParallelState {
    /// Not started
    Idle,
    /// Running agents
    Running,
    /// Paused
    Paused,
    /// Completed successfully
    Completed,
    /// Failed
    Failed,
}

/// Result of parallel execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelResult_ {
    /// Final state
    pub state: ParallelState,
    /// Total tasks completed
    pub tasks_completed: usize,
    /// Total tasks failed
    pub tasks_failed: usize,
    /// Total tokens used
    pub tokens_used: u64,
    /// Total duration in seconds
    pub duration_secs: u64,
    /// Merged commit SHAs
    pub merged_commits: Vec<String>,
    /// Agent results
    pub agent_results: Vec<AgentResult>,
}

/// Result from a single agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Agent ID
    pub agent_id: String,
    /// Tasks completed
    pub tasks_completed: usize,
    /// Tasks failed
    pub tasks_failed: usize,
    /// Tokens used
    pub tokens_used: u64,
    /// Branch name
    pub branch: String,
    /// Whether work was merged
    pub merged: bool,
    /// Error if failed
    pub error: Option<String>,
}

/// Execution backend for parallel agents
pub enum ExecutionBackend {
    /// Worktree-based isolation (legacy, shared git object database)
    Worktree(Arc<RwLock<WorktreeManager>>),
    /// Container-based isolation (fresh git clone per agent)
    Container(Arc<RwLock<ContainerManager>>),
}

/// Parallel orchestrator for running multiple agents
pub struct ParallelOrchestrator {
    /// Configuration
    config: ParallelConfig,
    /// Working directory (main repo)
    #[allow(dead_code)]
    working_dir: PathBuf,
    /// Execution backend (worktree or container)
    backend: ExecutionBackend,
    /// Agent pool
    agent_pool: Arc<AgentPool>,
    /// Issue repository (taskmaster)
    issue_repo: Arc<dyn IssueRepository>,
    /// Current state
    state: Arc<RwLock<ParallelState>>,
    /// Start time
    started_at: Option<DateTime<Utc>>,
    /// Merged commits / pushed commits
    merged_commits: Arc<RwLock<Vec<String>>>,
}

impl ParallelOrchestrator {
    /// Create a new parallel orchestrator with worktree backend (legacy)
    pub fn new(
        config: ParallelConfig,
        working_dir: PathBuf,
        issue_repo: Arc<dyn IssueRepository>,
    ) -> ParallelResult<Self> {
        let worktree_manager = WorktreeManager::new(&working_dir)?;
        let backend = ExecutionBackend::Worktree(Arc::new(RwLock::new(worktree_manager)));

        Ok(Self {
            agent_pool: Arc::new(AgentPool::new(config.max_agents)),
            config,
            working_dir,
            backend,
            issue_repo,
            state: Arc::new(RwLock::new(ParallelState::Idle)),
            started_at: None,
            merged_commits: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Create a new parallel orchestrator with container backend
    ///
    /// This is the recommended approach for parallel agent execution.
    /// Each agent gets a fresh git clone in an isolated container.
    pub async fn new_with_containers(
        config: ParallelConfig,
        working_dir: PathBuf,
        issue_repo: Arc<dyn IssueRepository>,
    ) -> ParallelResult<Self> {
        if config.git_remote_url.is_empty() {
            return Err(ParallelError::InvalidConfig(
                "git_remote_url is required for container mode".to_string(),
            ));
        }

        let workspace_base = working_dir.join(".containers");
        let container_manager =
            ContainerManager::new(workspace_base, config.container_image.clone()).await?;
        let backend = ExecutionBackend::Container(Arc::new(RwLock::new(container_manager)));

        Ok(Self {
            agent_pool: Arc::new(AgentPool::new(config.max_agents)),
            config,
            working_dir,
            backend,
            issue_repo,
            state: Arc::new(RwLock::new(ParallelState::Idle)),
            started_at: None,
            merged_commits: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Check if using container backend
    pub fn is_container_mode(&self) -> bool {
        matches!(self.backend, ExecutionBackend::Container(_))
    }

    /// Initialize agents and execution environments
    ///
    /// PAR-001: Run multiple agents in parallel
    /// PAR-010: Create isolated environments for each agent
    pub async fn initialize(&mut self) -> ParallelResult<()> {
        info!(
            "Initializing parallel orchestrator with {} agents (mode: {})",
            self.config.max_agents,
            if self.is_container_mode() {
                "container"
            } else {
                "worktree"
            }
        );

        match &self.backend {
            ExecutionBackend::Worktree(wt_manager) => {
                let mut manager = wt_manager.write().await;
                for i in 0..self.config.max_agents {
                    let agent_id = format!("agent-{}", i);

                    // Create worktree for this agent
                    let worktree = manager.create_worktree(&agent_id)?;

                    // Create agent config
                    let agent_config = AgentConfig {
                        id: agent_id.clone(),
                        worktree_path: worktree.path.clone(),
                        branch: worktree.branch.clone(),
                        max_tasks: self.config.max_tasks_per_agent,
                        use_claude_code: self.config.use_claude_code,
                    };

                    // Add to pool
                    self.agent_pool.add_agent(agent_config).await?;

                    info!(
                        "Created agent {} with worktree at {:?}",
                        agent_id, worktree.path
                    );
                }
            }
            ExecutionBackend::Container(container_manager) => {
                let manager = container_manager.write().await;
                for i in 0..self.config.max_agents {
                    let agent_id = format!("agent-{}", i);

                    // Create container config for this agent
                    let mut container_config = ContainerAgentConfig::new(
                        &agent_id,
                        &self.config.container_image,
                        &self.config.git_remote_url,
                    );

                    if let Some(mem) = &self.config.container_memory {
                        container_config = container_config.memory_limit(mem);
                    }
                    if let Some(cpus) = self.config.container_cpus {
                        container_config = container_config.cpu_limit(cpus);
                    }

                    // Provision container
                    manager.provision(container_config.clone()).await?;

                    // Create agent config (for pool tracking)
                    let agent_config = AgentConfig {
                        id: agent_id.clone(),
                        worktree_path: PathBuf::new(), // Not used in container mode
                        branch: container_config.branch.clone(),
                        max_tasks: self.config.max_tasks_per_agent,
                        use_claude_code: self.config.use_claude_code,
                    };

                    // Add to pool
                    self.agent_pool.add_agent(agent_config).await?;

                    info!(
                        "Created agent {} with container (branch: {})",
                        agent_id, container_config.branch
                    );
                }
            }
        }

        Ok(())
    }

    /// Run parallel execution
    ///
    /// PAR-002: Load balance tasks across agents
    /// PAR-004: Aggregate results from parallel runs
    pub async fn run(&mut self) -> ParallelResult<ParallelResult_> {
        self.started_at = Some(Utc::now());
        *self.state.write().await = ParallelState::Running;

        info!("Starting parallel execution");

        // Track total tasks completed
        let mut total_completed = 0usize;
        let mut total_failed = 0usize;

        // Main loop: assign tasks to available agents
        loop {
            // Check termination conditions
            if self.should_stop(total_completed).await {
                break;
            }

            // Get ready issues
            let ready_issues = self
                .issue_repo
                .ready(IssueFilter::default())
                .map_err(|e| ParallelError::agent(e.to_string()))?;

            if ready_issues.is_empty() {
                debug!("No ready issues, checking if agents are done");

                // Wait for any running agents to complete
                if self.agent_pool.stats().await.working_agents == 0 {
                    info!("No more tasks and no agents working, stopping");
                    break;
                }

                // Wait for a completion
                if let Some(completion) = self.agent_pool.wait_for_completion().await {
                    self.handle_completion(&completion, &mut total_completed, &mut total_failed)
                        .await?;
                }

                continue;
            }

            // Try to assign issues to available agents
            for issue in ready_issues.iter() {
                if let Some(agent_id) = self.agent_pool.get_available_agent().await {
                    // Mark issue as in progress
                    if let Err(e) = self.issue_repo.start(&issue.id, Some(PARALLEL_ACTOR)) {
                        warn!("Failed to start issue {}: {}", issue.id, e);
                        continue;
                    }

                    // Assign to agent
                    if let Err(e) = self.agent_pool.assign_task(&agent_id, &issue.id).await {
                        warn!("Failed to assign issue {} to {}: {}", issue.id, agent_id, e);
                        continue;
                    }

                    info!("Assigned issue {} to agent {}", issue.id, agent_id);

                    // TODO: Actually spawn the agent execution
                    // For now, we'll simulate completion
                    // In real implementation, this would spawn a tokio task
                    // that runs the orchestrator in the worktree

                    // Simulate task completion (TODO: replace with real execution)
                    let completion = TaskCompletion {
                        agent_id: agent_id.clone(),
                        task_id: issue.id.clone(),
                        success: true,
                        error: None,
                        tokens_used: 1000,
                    };

                    self.agent_pool.report_completion(completion).await?;
                } else {
                    // No available agents, wait for one
                    debug!("No available agents, waiting for completion");
                    if let Some(completion) = self.agent_pool.wait_for_completion().await {
                        self.handle_completion(&completion, &mut total_completed, &mut total_failed)
                            .await?;
                    }
                    break;
                }
            }

            // Brief yield to prevent tight loop
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Merge completed work if configured
        let mut agent_results = Vec::new();
        if self.config.auto_merge {
            agent_results = self.merge_all().await?;
        }

        // Cleanup
        self.cleanup().await?;

        // Determine final state
        let final_state = if total_failed > 0 && total_completed == 0 {
            ParallelState::Failed
        } else {
            ParallelState::Completed
        };

        *self.state.write().await = final_state;

        let pool_stats = self.agent_pool.stats().await;
        let merged_commits = self.merged_commits.read().await.clone();

        let duration = self
            .started_at
            .map(|s| (Utc::now() - s).num_seconds() as u64)
            .unwrap_or(0);

        Ok(ParallelResult_ {
            state: final_state,
            tasks_completed: pool_stats.total_tasks_completed,
            tasks_failed: pool_stats.total_tasks_failed,
            tokens_used: pool_stats.total_tokens_used,
            duration_secs: duration,
            merged_commits,
            agent_results,
        })
    }

    /// Handle a task completion notification
    async fn handle_completion(
        &self,
        completion: &TaskCompletion,
        total_completed: &mut usize,
        total_failed: &mut usize,
    ) -> ParallelResult<()> {
        if completion.success {
            *total_completed += 1;

            // Close issue in repository
            if let Err(e) = self.issue_repo.close(
                &completion.task_id,
                Some("Completed by parallel agent"),
                vec![],
                Some(PARALLEL_ACTOR),
            ) {
                warn!("Failed to close issue {}: {}", completion.task_id, e);
            }

            info!(
                "Agent {} completed issue {} (total: {})",
                completion.agent_id, completion.task_id, total_completed
            );
        } else {
            *total_failed += 1;

            // Block issue in repository
            if let Err(e) = self.issue_repo.block(
                &completion.task_id,
                completion.error.as_deref(),
                Some(PARALLEL_ACTOR),
            ) {
                warn!("Failed to block issue {}: {}", completion.task_id, e);
            }

            error!(
                "Agent {} failed issue {}: {:?}",
                completion.agent_id, completion.task_id, completion.error
            );
        }

        Ok(())
    }

    /// Check if we should stop execution
    async fn should_stop(&self, tasks_completed: usize) -> bool {
        // Check max tasks
        if let Some(max) = self.config.max_total_tasks {
            if tasks_completed >= max {
                info!("Reached max tasks limit ({})", max);
                return true;
            }
        }

        // Check duration
        if let Some(max_secs) = self.config.max_duration_secs {
            if let Some(started) = self.started_at {
                let elapsed = (Utc::now() - started).num_seconds() as u64;
                if elapsed >= max_secs {
                    info!("Reached max duration ({}s)", max_secs);
                    return true;
                }
            }
        }

        // Check if all agents failed
        let stats = self.agent_pool.stats().await;
        if stats.failed_agents == stats.total_agents && stats.total_agents > 0 {
            error!("All agents have failed");
            return true;
        }

        false
    }

    /// Merge or push all completed agent work
    ///
    /// PAR-012: Merge completed work back to main (worktree mode)
    ///          Push to agent branches (container mode)
    async fn merge_all(&self) -> ParallelResult<Vec<AgentResult>> {
        info!(
            "{} completed agent work",
            if self.is_container_mode() {
                "Pushing"
            } else {
                "Merging"
            }
        );

        let mut results = Vec::new();
        let agent_ids = self.agent_pool.agent_ids().await;
        let mut merged_commits = self.merged_commits.write().await;

        match &self.backend {
            ExecutionBackend::Worktree(wt_manager) => {
                let manager = wt_manager.write().await;
                for agent_id in agent_ids {
                    let mut result = AgentResult {
                        agent_id: agent_id.clone(),
                        tasks_completed: 0, // TODO: get per-agent stats
                        tasks_failed: 0,
                        tokens_used: 0,
                        branch: format!("agent/{}", agent_id),
                        merged: false,
                        error: None,
                    };

                    // Try to merge
                    let commit_msg = format!("Merge work from agent {}", agent_id);
                    match manager.merge_to_main(&agent_id, &commit_msg) {
                        Ok(sha) => {
                            info!("Merged agent {} (commit {})", agent_id, sha);
                            result.merged = true;
                            merged_commits.push(sha);
                        }
                        Err(ParallelError::MergeConflict { files }) => {
                            warn!("Merge conflict for agent {} in {:?}", agent_id, files);
                            result.error = Some(format!("Merge conflict in: {:?}", files));
                        }
                        Err(e) => {
                            warn!("Failed to merge agent {}: {}", agent_id, e);
                            result.error = Some(e.to_string());
                        }
                    }

                    results.push(result);
                }
            }
            ExecutionBackend::Container(container_manager) => {
                let manager = container_manager.write().await;
                for agent_id in agent_ids {
                    let mut result = AgentResult {
                        agent_id: agent_id.clone(),
                        tasks_completed: 0, // TODO: get per-agent stats
                        tasks_failed: 0,
                        tokens_used: 0,
                        branch: format!("agent/{}", agent_id),
                        merged: false, // In container mode, we push to branches, not merge
                        error: None,
                    };

                    // Try to push changes
                    let commit_msg = format!(
                        "Agent {} work\n\nGenerated with [OpenAgents](https://openagents.com)\n\nCo-Authored-By: OpenAgents Agent <agent@openagents.com>",
                        agent_id
                    );
                    match manager.push_changes(&agent_id, &commit_msg).await {
                        Ok(sha) => {
                            info!("Pushed agent {} work (commit {})", agent_id, sha);
                            result.merged = true; // Treat push as "merged" for result reporting
                            merged_commits.push(sha);
                        }
                        Err(e) => {
                            warn!("Failed to push agent {} work: {}", agent_id, e);
                            result.error = Some(e.to_string());
                        }
                    }

                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    /// Cleanup execution environments and resources
    ///
    /// PAR-011: Manage environment lifecycle (cleanup)
    async fn cleanup(&self) -> ParallelResult<()> {
        info!("Cleaning up parallel orchestrator");

        self.agent_pool.shutdown_all().await;

        match &self.backend {
            ExecutionBackend::Worktree(wt_manager) => {
                let mut manager = wt_manager.write().await;
                manager.cleanup_all()?;
            }
            ExecutionBackend::Container(container_manager) => {
                let manager = container_manager.write().await;
                manager.cleanup_all().await?;
            }
        }

        Ok(())
    }

    /// Get current pool statistics
    ///
    /// PAR-005: Report progress across all agents
    pub async fn stats(&self) -> PoolStats {
        self.agent_pool.stats().await
    }

    /// Get current state
    pub async fn state(&self) -> ParallelState {
        *self.state.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_config_default() {
        let config = ParallelConfig::default();
        assert_eq!(config.max_agents, 2);
        assert!(config.auto_merge);
        assert!(config.use_claude_code);
        assert!(!config.safe_mode);
        assert!(!config.dry_run);
    }
}
