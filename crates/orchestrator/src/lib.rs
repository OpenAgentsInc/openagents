//! Golden Loop Agent Orchestrator
//!
//! ORCH-001..082: Task selection, subagent execution, verification, git operations,
//! session management, error recovery, safe mode, and sandbox.
//!
//! # The Golden Loop
//!
//! The orchestrator implements the "Golden Loop" - the core execution cycle:
//!
//! ```text
//! 1. SELECT ready task from queue
//! 2. DECOMPOSE into subtasks if needed
//! 3. EXECUTE each subtask via tool calls
//! 4. VERIFY results against acceptance criteria
//! 5. COMMIT changes with proper attribution
//! 6. Repeat
//! ```
//!
//! # Example
//!
//! ```ignore
//! use orchestrator::{Orchestrator, OrchestratorConfig};
//!
//! let config = OrchestratorConfig::default();
//! let orchestrator = Orchestrator::new(config).await?;
//! orchestrator.run_loop().await?;
//! ```

mod error;
mod event;
mod session;
mod executor;
mod verifier;
mod orchestrator;

pub use error::*;
pub use event::*;
pub use session::*;
pub use executor::*;
pub use verifier::*;
pub use orchestrator::*;
