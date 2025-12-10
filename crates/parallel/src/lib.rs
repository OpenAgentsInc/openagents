//! Parallel agent execution with git worktree isolation
//!
//! Implements user stories PAR-001 through PAR-013:
//!
//! ## Parallel Orchestrator (PAR-001..005)
//! - PAR-001: Run multiple agents in parallel
//! - PAR-002: Load balance tasks across agents
//! - PAR-003: Handle agent failures gracefully
//! - PAR-004: Aggregate results from parallel runs
//! - PAR-005: Report progress across all agents
//!
//! ## Worktree Isolation (PAR-010..013)
//! - PAR-010: Create isolated worktrees for each agent
//! - PAR-011: Manage worktree lifecycle (create/cleanup)
//! - PAR-012: Merge completed work back to main
//! - PAR-013: Handle merge conflicts
//!
//! # Architecture
//!
//! ```text
//! ParallelOrchestrator
//!     ├── WorktreeManager (creates isolated git worktrees)
//!     ├── AgentPool (manages N agent instances)
//!     │   ├── Agent[0] → Worktree[0] → Task queue
//!     │   ├── Agent[1] → Worktree[1] → Task queue
//!     │   └── Agent[N] → Worktree[N] → Task queue
//!     └── ResultAggregator (merges work, reports progress)
//! ```

mod worktree;
mod agent_pool;
mod orchestrator;
mod error;

pub use worktree::*;
pub use agent_pool::*;
pub use orchestrator::*;
pub use error::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_config_default() {
        let config = ParallelConfig::default();
        assert_eq!(config.max_agents, 2);
        assert!(config.auto_merge);
    }
}
