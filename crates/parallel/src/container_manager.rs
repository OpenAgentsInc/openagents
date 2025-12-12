//! Container lifecycle management for parallel agents
//!
//! Replaces WorktreeManager with full container isolation.
//! Each agent gets a fresh git clone in an isolated container.

use crate::{ParallelError, ParallelResult};
use sandbox::{ContainerBackend, ContainerConfig, ContainerRunResult, CredentialMount};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for container-based agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerAgentConfig {
    /// Unique agent identifier
    pub agent_id: String,
    /// Docker image to use (e.g., "openagents/agent:latest")
    pub image: String,
    /// Git remote URL to clone
    pub remote_url: String,
    /// Branch name for this agent's work
    pub branch: String,
    /// Memory limit (e.g., "8G")
    pub memory_limit: Option<String>,
    /// CPU limit
    pub cpu_limit: Option<f32>,
    /// Timeout in seconds
    pub timeout_secs: Option<u64>,
}

impl ContainerAgentConfig {
    /// Create a new agent config
    pub fn new(
        agent_id: impl Into<String>,
        image: impl Into<String>,
        remote_url: impl Into<String>,
    ) -> Self {
        let id = agent_id.into();
        Self {
            branch: format!("agent/{}", id),
            agent_id: id,
            image: image.into(),
            remote_url: remote_url.into(),
            memory_limit: Some("8G".to_string()),
            cpu_limit: Some(2.0),
            timeout_secs: Some(3600), // 1 hour default
        }
    }

    /// Set memory limit
    pub fn memory_limit(mut self, limit: impl Into<String>) -> Self {
        self.memory_limit = Some(limit.into());
        self
    }

    /// Set CPU limit
    pub fn cpu_limit(mut self, cpus: f32) -> Self {
        self.cpu_limit = Some(cpus);
        self
    }

    /// Set timeout in seconds
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set branch name
    pub fn branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = branch.into();
        self
    }
}

/// State of a container agent
#[derive(Debug, Clone)]
pub struct ContainerAgentState {
    /// Configuration
    pub config: ContainerAgentConfig,
    /// Container ID (once running)
    pub container_id: Option<String>,
    /// Current task being executed
    pub current_task: Option<String>,
    /// Workspace directory on host (temporary)
    pub workspace_dir: PathBuf,
    /// Whether the agent has been provisioned
    pub provisioned: bool,
    /// Last error message
    pub last_error: Option<String>,
}

/// Manages container lifecycle for parallel agents
pub struct ContainerManager {
    /// Container backend (Docker, macOS Container, etc.)
    backend: Arc<dyn ContainerBackend>,
    /// Active containers by agent ID
    containers: Arc<RwLock<HashMap<String, ContainerAgentState>>>,
    /// Base directory for temporary workspaces
    workspace_base: PathBuf,
    /// Default image to use
    default_image: String,
    /// Credential mount (shared across all containers)
    credential_mount: Option<CredentialMount>,
}

impl ContainerManager {
    /// Create a new container manager
    pub async fn new(workspace_base: PathBuf, default_image: String) -> ParallelResult<Self> {
        let backend = sandbox::detect_backend().await;

        // Check if backend is available
        if !backend.is_available().await {
            return Err(ParallelError::container_not_available(format!(
                "No container runtime available (tried {})",
                backend.name()
            )));
        }

        // Extract credentials for Claude API
        let credential_mount = match sandbox::create_credential_mount().await {
            Ok(mount) => {
                info!("Credential mount created at {:?}", mount.host_dir);
                Some(mount)
            }
            Err(e) => {
                warn!("Failed to create credential mount: {}. Containers will run without Claude credentials.", e);
                None
            }
        };

        // Ensure workspace base exists
        tokio::fs::create_dir_all(&workspace_base).await?;

        Ok(Self {
            backend,
            containers: Arc::new(RwLock::new(HashMap::new())),
            workspace_base,
            default_image,
            credential_mount,
        })
    }

    /// Get the backend name
    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    /// Provision a new container for an agent
    ///
    /// This creates a temporary workspace, clones the repo, and prepares the container.
    pub async fn provision(&self, config: ContainerAgentConfig) -> ParallelResult<String> {
        let agent_id = config.agent_id.clone();
        info!("Provisioning container for agent {}", agent_id);

        // Create temporary workspace directory
        let workspace = self.workspace_base.join(&agent_id);
        tokio::fs::create_dir_all(&workspace).await?;

        // Clone repository into workspace
        self.git_clone(&config.remote_url, &workspace, &config.branch)
            .await?;

        // Store state
        let state = ContainerAgentState {
            config,
            container_id: None,
            current_task: None,
            workspace_dir: workspace.clone(),
            provisioned: true,
            last_error: None,
        };

        self.containers
            .write()
            .await
            .insert(agent_id.clone(), state);

        info!(
            "Agent {} provisioned with workspace at {:?}",
            agent_id, workspace
        );

        Ok(agent_id)
    }

    /// Run a task in a container
    ///
    /// Executes the Claude CLI with the given task in the agent's container.
    pub async fn execute_task(
        &self,
        agent_id: &str,
        task_id: &str,
        task_description: &str,
    ) -> ParallelResult<ContainerRunResult> {
        let containers = self.containers.read().await;
        let state = containers.get(agent_id).ok_or_else(|| {
            ParallelError::container_not_available(format!("Agent {} not provisioned", agent_id))
        })?;

        if !state.provisioned {
            return Err(ParallelError::container_not_available(format!(
                "Agent {} not provisioned",
                agent_id
            )));
        }

        info!(
            "Executing task {} on agent {} in container",
            task_id, agent_id
        );

        // Build the agent execution command
        // Escape single quotes in task description
        let escaped_desc = task_description.replace('\'', "'\\''");
        let command = vec![
            "bash".to_string(),
            "-c".to_string(),
            format!(
                "cd /workspace && claude --dangerously-skip-permissions --print '{}' 2>&1",
                escaped_desc
            ),
        ];

        // Build container config
        let mut config = ContainerConfig::new(&state.config.image, &state.workspace_dir)
            .workdir("/workspace")
            .env("TASK_ID", task_id)
            .env("AGENT_ID", agent_id)
            .env("GIT_BRANCH", &state.config.branch);

        // Apply resource limits
        if let Some(mem) = &state.config.memory_limit {
            config = config.memory_limit(mem);
        }
        if let Some(cpus) = state.config.cpu_limit {
            config = config.cpu_limit(cpus);
        }
        if let Some(secs) = state.config.timeout_secs {
            config = config.timeout(Duration::from_secs(secs));
        }

        // Add credential mount if available
        if let Some(cred) = &self.credential_mount {
            config = config.volume_mount(&cred.volume_mount);
        }

        // Run container
        let result = self.backend.run(&command, &config).await.map_err(|e| {
            ParallelError::container_execution_failed(format!(
                "Container execution failed for agent {}: {}",
                agent_id, e
            ))
        })?;

        debug!(
            "Task {} completed with exit code {} on agent {}",
            task_id, result.exit_code, agent_id
        );

        Ok(result)
    }

    /// Commit and push changes from agent to remote branch
    ///
    /// Returns the commit SHA if successful.
    pub async fn push_changes(
        &self,
        agent_id: &str,
        commit_message: &str,
    ) -> ParallelResult<String> {
        let containers = self.containers.read().await;
        let state = containers.get(agent_id).ok_or_else(|| {
            ParallelError::container_not_available(format!("Agent {} not provisioned", agent_id))
        })?;

        info!("Pushing changes for agent {} to branch {}", agent_id, state.config.branch);

        // Escape single quotes in commit message
        let escaped_msg = commit_message.replace('\'', "'\\''");

        // Run git commands in container
        let push_cmd = vec![
            "bash".to_string(),
            "-c".to_string(),
            format!(
                r#"cd /workspace && \
                   git add -A && \
                   git diff --cached --quiet || git commit -m '{}' && \
                   git push origin {} 2>&1"#,
                escaped_msg, state.config.branch
            ),
        ];

        let config = ContainerConfig::new(&state.config.image, &state.workspace_dir)
            .workdir("/workspace")
            .timeout(Duration::from_secs(300)); // 5 min timeout for push

        let result = self.backend.run(&push_cmd, &config).await.map_err(|e| {
            ParallelError::push_failed(
                &state.config.branch,
                format!("Push command failed: {}", e),
            )
        })?;

        if result.exit_code != 0 {
            return Err(ParallelError::push_failed(
                &state.config.branch,
                format!("Exit code {}: {}", result.exit_code, result.stderr),
            ));
        }

        // Get the commit SHA
        let sha_cmd = vec![
            "bash".to_string(),
            "-c".to_string(),
            "cd /workspace && git rev-parse HEAD".to_string(),
        ];

        let sha_result = self.backend.run(&sha_cmd, &config).await.map_err(|e| {
            ParallelError::container_execution_failed(format!("Failed to get commit SHA: {}", e))
        })?;

        let sha = sha_result.stdout.trim().to_string();
        info!("Agent {} pushed commit {}", agent_id, sha);

        Ok(sha)
    }

    /// Clean up container and workspace for an agent
    pub async fn cleanup(&self, agent_id: &str) -> ParallelResult<()> {
        info!("Cleaning up agent {}", agent_id);

        let mut containers = self.containers.write().await;

        if let Some(state) = containers.remove(agent_id) {
            // Remove workspace directory
            if let Err(e) = tokio::fs::remove_dir_all(&state.workspace_dir).await {
                warn!(
                    "Failed to remove workspace for agent {}: {}",
                    agent_id, e
                );
            }
        }

        Ok(())
    }

    /// Clean up all containers and workspaces
    pub async fn cleanup_all(&self) -> ParallelResult<Vec<String>> {
        info!("Cleaning up all containers");

        let agent_ids: Vec<String> = {
            self.containers.read().await.keys().cloned().collect()
        };

        let mut results = Vec::new();
        for agent_id in &agent_ids {
            if let Err(e) = self.cleanup(agent_id).await {
                warn!("Failed to cleanup agent {}: {}", agent_id, e);
            } else {
                results.push(agent_id.clone());
            }
        }

        // Cleanup credential mount
        if let Some(mount) = &self.credential_mount {
            if let Err(e) = sandbox::cleanup_credential_mount(mount).await {
                warn!("Failed to cleanup credential mount: {}", e);
            }
        }

        Ok(results)
    }

    /// Check health of all containers and return lost agent IDs
    pub async fn health_check(&self) -> Vec<String> {
        let containers = self.containers.read().await;
        let mut lost_agents = Vec::new();

        for (agent_id, state) in containers.iter() {
            // Check if workspace still exists
            if !state.workspace_dir.exists() {
                error!("Workspace for agent {} no longer exists", agent_id);
                lost_agents.push(agent_id.clone());
                continue;
            }

            // Could add more health checks here:
            // - Check if container is still running (if using persistent containers)
            // - Check git repo integrity
            // - Check disk space
        }

        lost_agents
    }

    /// Get list of all agent IDs
    pub async fn agent_ids(&self) -> Vec<String> {
        self.containers.read().await.keys().cloned().collect()
    }

    /// Get state for a specific agent
    pub async fn get_state(&self, agent_id: &str) -> Option<ContainerAgentState> {
        self.containers.read().await.get(agent_id).cloned()
    }

    /// Update the current task for an agent
    pub async fn set_current_task(&self, agent_id: &str, task_id: Option<String>) {
        if let Some(state) = self.containers.write().await.get_mut(agent_id) {
            state.current_task = task_id;
        }
    }

    /// Clone a repository into a workspace
    async fn git_clone(
        &self,
        url: &str,
        path: &PathBuf,
        branch: &str,
    ) -> ParallelResult<()> {
        info!("Cloning {} to {:?} (branch: {})", url, path, branch);

        // First try to clone with the branch
        let output = tokio::process::Command::new("git")
            .args(["clone", "--branch", branch, "--single-branch", "--depth", "1", url])
            .arg(path)
            .output()
            .await?;

        if output.status.success() {
            return Ok(());
        }

        // If branch doesn't exist, clone default branch and create the branch
        let clone_output = tokio::process::Command::new("git")
            .args(["clone", "--depth", "1", url])
            .arg(path)
            .output()
            .await?;

        if !clone_output.status.success() {
            return Err(ParallelError::clone_failed(
                url,
                String::from_utf8_lossy(&clone_output.stderr).to_string(),
            ));
        }

        // Create and checkout the new branch
        let checkout_output = tokio::process::Command::new("git")
            .current_dir(path)
            .args(["checkout", "-b", branch])
            .output()
            .await?;

        if !checkout_output.status.success() {
            warn!(
                "Failed to create branch {}: {}",
                branch,
                String::from_utf8_lossy(&checkout_output.stderr)
            );
            // Not fatal - might already be on correct branch
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_agent_config() {
        let config = ContainerAgentConfig::new(
            "agent-0",
            "openagents/agent:latest",
            "https://github.com/example/repo.git",
        )
        .memory_limit("16G")
        .cpu_limit(4.0)
        .timeout_secs(7200);

        assert_eq!(config.agent_id, "agent-0");
        assert_eq!(config.branch, "agent/agent-0");
        assert_eq!(config.memory_limit, Some("16G".to_string()));
        assert_eq!(config.cpu_limit, Some(4.0));
        assert_eq!(config.timeout_secs, Some(7200));
    }

    #[test]
    fn test_custom_branch() {
        let config = ContainerAgentConfig::new(
            "agent-0",
            "ubuntu:latest",
            "https://github.com/example/repo.git",
        )
        .branch("feature/my-branch");

        assert_eq!(config.branch, "feature/my-branch");
    }
}
