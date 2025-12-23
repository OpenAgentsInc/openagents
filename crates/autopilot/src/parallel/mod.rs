//! Parallel autopilot orchestration
//!
//! This module provides functionality to run multiple autopilot instances
//! in isolated Docker containers with git worktrees for parallel issue resolution.
//!
//! # Architecture
//!
//! - Each agent runs in a Docker container
//! - Each container has a git worktree mounted at /workspace
//! - All containers share autopilot.db via bind mount
//! - SQLite atomic claiming prevents race conditions
//!
//! # Usage
//!
//! ```no_run
//! use autopilot::parallel::{start_agents, stop_agents, list_agents};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Start 5 agents
//! start_agents(5).await?;
//!
//! // Check status
//! let agents = list_agents().await?;
//! for agent in agents {
//!     println!("{}: {}", agent.id, agent.status);
//! }
//!
//! // Stop all agents
//! stop_agents().await?;
//! # Ok(())
//! # }
//! ```

mod docker;
mod worktree;

pub use docker::{start_agents, stop_agents, list_agents, get_logs, AgentInfo, AgentStatus};
pub use worktree::{create_worktrees, remove_worktrees, list_worktrees};

use serde::{Deserialize, Serialize};

/// Platform detection for resource limits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Linux,
    MacOS,
}

impl Platform {
    /// Detect current platform
    pub fn detect() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        }
    }

    /// Maximum recommended agents for this platform
    pub fn max_agents(&self) -> usize {
        match self {
            Platform::Linux => 10,
            Platform::MacOS => 5,
        }
    }

    /// Default memory limit per agent
    pub fn default_memory(&self) -> &'static str {
        match self {
            Platform::Linux => "12G",
            Platform::MacOS => "3G",
        }
    }

    /// Default CPU limit per agent
    pub fn default_cpus(&self) -> &'static str {
        match self {
            Platform::Linux => "4",
            Platform::MacOS => "2",
        }
    }
}

/// Configuration for parallel agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelConfig {
    /// Number of agents to run
    pub agent_count: usize,
    /// Memory limit per agent (e.g., "12G")
    pub memory_limit: String,
    /// CPU limit per agent (e.g., "4")
    pub cpu_limit: String,
    /// Model to use (e.g., "sonnet")
    pub model: String,
    /// Project root path
    pub project_root: std::path::PathBuf,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        let platform = Platform::detect();
        Self {
            agent_count: 3,
            memory_limit: platform.default_memory().to_string(),
            cpu_limit: platform.default_cpus().to_string(),
            model: "sonnet".to_string(),
            project_root: std::env::current_dir().unwrap_or_default(),
        }
    }
}

impl ParallelConfig {
    /// Create config with platform-appropriate defaults
    pub fn for_platform(platform: Platform, count: usize) -> Self {
        Self {
            agent_count: count.min(platform.max_agents()),
            memory_limit: platform.default_memory().to_string(),
            cpu_limit: platform.default_cpus().to_string(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();
        // Should return a valid platform
        assert!(matches!(platform, Platform::Linux | Platform::MacOS));
    }

    #[test]
    fn test_platform_limits() {
        assert_eq!(Platform::Linux.max_agents(), 10);
        assert_eq!(Platform::MacOS.max_agents(), 5);
        assert_eq!(Platform::Linux.default_memory(), "12G");
        assert_eq!(Platform::MacOS.default_memory(), "3G");
    }

    #[test]
    fn test_config_default() {
        let config = ParallelConfig::default();
        assert_eq!(config.agent_count, 3);
        assert_eq!(config.model, "sonnet");
    }

    #[test]
    fn test_config_for_platform() {
        let config = ParallelConfig::for_platform(Platform::MacOS, 10);
        // Should cap at platform max
        assert_eq!(config.agent_count, 5);
        assert_eq!(config.memory_limit, "3G");
    }
}
