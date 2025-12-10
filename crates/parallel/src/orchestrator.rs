//! Parallel orchestrator for running multiple agents
//!
//! Implements PAR-001..005: Parallel orchestrator

use crate::{
    AgentConfig, AgentPool, ParallelError, ParallelResult, PoolStats, TaskCompletion,
    WorktreeManager,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tasks::{TaskFilter, TaskRepository};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

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
    /// Whether to automatically merge completed work
    pub auto_merge: bool,
    /// Whether to use Claude Code (true) or local model (false)
    pub use_claude_code: bool,
    /// Safe mode - no destructive operations
    pub safe_mode: bool,
    /// Dry run mode - don't execute tools
    pub dry_run: bool,
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

/// Parallel orchestrator for running multiple agents
pub struct ParallelOrchestrator {
    /// Configuration
    config: ParallelConfig,
    /// Working directory (main repo)
    #[allow(dead_code)]
    working_dir: PathBuf,
    /// Worktree manager
    worktree_manager: Arc<RwLock<WorktreeManager>>,
    /// Agent pool
    agent_pool: Arc<AgentPool>,
    /// Task repository
    task_repo: Arc<dyn TaskRepository>,
    /// Current state
    state: Arc<RwLock<ParallelState>>,
    /// Start time
    started_at: Option<DateTime<Utc>>,
    /// Merged commits
    merged_commits: Arc<RwLock<Vec<String>>>,
}

impl ParallelOrchestrator {
    /// Create a new parallel orchestrator
    pub fn new(
        config: ParallelConfig,
        working_dir: PathBuf,
        task_repo: Arc<dyn TaskRepository>,
    ) -> ParallelResult<Self> {
        let worktree_manager = WorktreeManager::new(&working_dir)?;

        Ok(Self {
            agent_pool: Arc::new(AgentPool::new(config.max_agents)),
            config,
            working_dir,
            worktree_manager: Arc::new(RwLock::new(worktree_manager)),
            task_repo,
            state: Arc::new(RwLock::new(ParallelState::Idle)),
            started_at: None,
            merged_commits: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Initialize agents and worktrees
    ///
    /// PAR-001: Run multiple agents in parallel
    /// PAR-010: Create isolated worktrees for each agent
    pub async fn initialize(&mut self) -> ParallelResult<()> {
        info!(
            "Initializing parallel orchestrator with {} agents",
            self.config.max_agents
        );

        let mut wt_manager = self.worktree_manager.write().await;

        for i in 0..self.config.max_agents {
            let agent_id = format!("agent-{}", i);

            // Create worktree for this agent
            let worktree = wt_manager.create_worktree(&agent_id)?;

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

            // Get ready tasks
            let ready_tasks = self
                .task_repo
                .ready_tasks(TaskFilter::default())
                .map_err(|e| ParallelError::agent(e.to_string()))?;

            if ready_tasks.is_empty() {
                debug!("No ready tasks, checking if agents are done");

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

            // Try to assign tasks to available agents
            for task in ready_tasks.iter() {
                if let Some(agent_id) = self.agent_pool.get_available_agent().await {
                    // Mark task as in progress
                    if let Err(e) = self.task_repo.start(&task.id) {
                        warn!("Failed to start task {}: {}", task.id, e);
                        continue;
                    }

                    // Assign to agent
                    if let Err(e) = self.agent_pool.assign_task(&agent_id, &task.id).await {
                        warn!("Failed to assign task {} to {}: {}", task.id, agent_id, e);
                        continue;
                    }

                    info!("Assigned task {} to agent {}", task.id, agent_id);

                    // TODO: Actually spawn the agent execution
                    // For now, we'll simulate completion
                    // In real implementation, this would spawn a tokio task
                    // that runs the orchestrator in the worktree

                    // Simulate task completion (TODO: replace with real execution)
                    let completion = TaskCompletion {
                        agent_id: agent_id.clone(),
                        task_id: task.id.clone(),
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

            // Close task in repository
            if let Err(e) = self
                .task_repo
                .close(&completion.task_id, Some("Completed by parallel agent"), vec![])
            {
                warn!("Failed to close task {}: {}", completion.task_id, e);
            }

            info!(
                "Agent {} completed task {} (total: {})",
                completion.agent_id, completion.task_id, total_completed
            );
        } else {
            *total_failed += 1;

            // Block task in repository
            if let Err(e) = self.task_repo.block(
                &completion.task_id,
                completion.error.as_deref(),
            ) {
                warn!("Failed to block task {}: {}", completion.task_id, e);
            }

            error!(
                "Agent {} failed task {}: {:?}",
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

    /// Merge all completed agent work
    ///
    /// PAR-012: Merge completed work back to main
    async fn merge_all(&self) -> ParallelResult<Vec<AgentResult>> {
        info!("Merging completed agent work");

        let mut results = Vec::new();
        let agent_ids = self.agent_pool.agent_ids().await;
        let wt_manager = self.worktree_manager.write().await;
        let mut merged_commits = self.merged_commits.write().await;

        for agent_id in agent_ids {
            let _stats = self.agent_pool.stats().await;

            let mut result = AgentResult {
                agent_id: agent_id.clone(),
                tasks_completed: 0,  // TODO: get per-agent stats
                tasks_failed: 0,
                tokens_used: 0,
                branch: format!("agent/{}", agent_id),
                merged: false,
                error: None,
            };

            // Try to merge
            let commit_msg = format!("Merge work from agent {}", agent_id);
            match wt_manager.merge_to_main(&agent_id, &commit_msg) {
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

        Ok(results)
    }

    /// Cleanup worktrees and resources
    ///
    /// PAR-011: Manage worktree lifecycle (cleanup)
    async fn cleanup(&self) -> ParallelResult<()> {
        info!("Cleaning up parallel orchestrator");

        self.agent_pool.shutdown_all().await;

        let mut wt_manager = self.worktree_manager.write().await;
        wt_manager.cleanup_all()?;

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
