//! Parallel autopilot orchestration
//!
//! This module enables running multiple autopilot instances simultaneously in isolated
//! Docker containers, dramatically increasing throughput for large issue queues.
//!
//! # Overview
//!
//! When there are 50+ open issues, a single agent processes them sequentially. The parallel
//! system solves this by running 3-10 agents concurrently (depending on available resources),
//! with each agent claiming and working on different issues atomically.
//!
//! # Key Features
//!
//! - **Atomic Issue Claiming**: All containers share `autopilot.db` via bind mount.
//!   SQLite's WAL mode + atomic operations prevent race conditions.
//! - **Git Isolation**: Each agent works in its own git worktree with a separate branch
//!   (`agent/001`, `agent/002`, etc.), preventing merge conflicts.
//! - **Resource Management**: Platform-aware memory/CPU limits (12GB/4 cores on Linux,
//!   3GB/2 cores on macOS).
//! - **Automatic Recovery**: 15-minute claim expiry means crashed agents release issues
//!   automatically.
//!
//! # Architecture
//!
//! ```text
//! Host Machine
//! ├── .git/                    (shared object database)
//! ├── autopilot.db             (SHARED - atomic issue claiming)
//! ├── .worktrees/
//! │   ├── agent-001/           (worktree → branch agent/001)
//! │   ├── agent-002/           (worktree → branch agent/002)
//! │   └── agent-00N/
//! └── docs/logs/               (shared log output)
//!
//! Docker Containers
//! ├── autopilot-001 → /workspace mounted from .worktrees/agent-001
//! ├── autopilot-002 → /workspace mounted from .worktrees/agent-002
//! └── autopilot-00N → each mounts shared autopilot.db
//! ```
//!
//! # Quick Start
//!
//! ## CLI Usage
//!
//! ```bash
//! # Start 3 agents (default)
//! ./scripts/parallel-autopilot.sh start
//!
//! # Start 5 agents
//! ./scripts/parallel-autopilot.sh start 5
//!
//! # Check status
//! ./scripts/parallel-autopilot.sh status
//!
//! # View issue queue
//! ./scripts/parallel-autopilot.sh queue
//!
//! # Stop all agents
//! ./scripts/parallel-autopilot.sh stop
//! ```
//!
//! ## Programmatic Usage
//!
//! ```no_run
//! use autopilot::parallel::{start_agents, stop_agents, list_agents, Platform};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Auto-detect platform and start agents
//! let platform = Platform::detect();
//! println!("Platform: {:?}, max agents: {}", platform, platform.max_agents());
//!
//! // Start 5 agents
//! let agents = start_agents(5).await?;
//! println!("Started {} agents", agents.len());
//!
//! // Check status
//! let agents = list_agents().await?;
//! for agent in agents {
//!     println!("Agent {}: {:?}", agent.id, agent.status);
//! }
//!
//! // Stop all agents
//! stop_agents().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Performance Guidelines
//!
//! | Platform       | CPU           | RAM   | Recommended | Max |
//! |----------------|---------------|-------|-------------|-----|
//! | Linux Desktop  | i7-14700K     | 128GB | 6-8 agents  | 10  |
//! | Linux Server   | AMD EPYC      | 256GB | 10-15 agents| 20  |
//! | MacBook Pro    | M2 Pro        | 16GB  | 3 agents    | 5   |
//! | MacBook Pro    | M3 Max        | 64GB  | 5-7 agents  | 10  |
//!
//! # Modules
//!
//! - [`docker`]: Docker Compose wrapper for starting/stopping containers
//! - [`worktree`]: Git worktree creation and cleanup
//!
//! # See Also
//!
//! - [Parallel Autopilot Documentation](../../docs/development/parallel-autopilot.md)
//! - [`crates/issues`](../issues/index.html): Atomic issue claiming
//! - [`scripts/parallel-autopilot.sh`](https://github.com/OpenAgentsInc/openagents/blob/main/scripts/parallel-autopilot.sh)

mod docker;
mod worktree;

pub use docker::{AgentInfo, AgentStatus, get_logs, list_agents, start_agents, stop_agents};
pub use worktree::{create_worktrees, list_worktrees, remove_worktrees};

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
