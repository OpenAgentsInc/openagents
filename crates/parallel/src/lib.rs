//! Parallel agent execution with container isolation
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
//! ## Container Isolation (PAR-010..013)
//! - PAR-010: Create isolated containers for each agent
//! - PAR-011: Manage container lifecycle (provision/cleanup)
//! - PAR-012: Push completed work to agent branches
//! - PAR-013: Handle container failures
//!
//! # Architecture
//!
//! ```text
//! ParallelOrchestrator
//!     ├── ContainerManager (creates isolated containers with fresh git clones)
//!     │   └── Backend: Docker | macOS Container
//!     ├── AgentPool (manages N agent instances)
//!     │   ├── Agent[0] → Container[0] → agent/agent-0 branch
//!     │   ├── Agent[1] → Container[1] → agent/agent-1 branch
//!     │   └── Agent[N] → Container[N] → agent/agent-N branch
//!     └── ResultAggregator (tracks completions, reports progress)
//!
//! # Isolation Model
//!
//! Each agent gets:
//! - Fresh `git clone` of the repository
//! - Isolated container environment
//! - Dedicated branch (agent/<id>)
//! - Push to remote for PR-based merging
//! ```

mod worktree;
mod agent_pool;
mod orchestrator;
mod container_manager;
mod error;

pub use worktree::*;
pub use agent_pool::*;
pub use orchestrator::*;
pub use container_manager::*;
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
