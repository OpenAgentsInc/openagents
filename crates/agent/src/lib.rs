//! Autonomous coding agent with orchestrator/subagent architecture
//!
//! AG-001..056: Session management, agent loop, orchestrator, subagent
//!
//! # Architecture
//!
//! Following Anthropic's "Effective Harnesses for Long-Running Agents" pattern:
//! - **Orchestrator**: Manages task selection, decomposition, verification, session coordination
//! - **Subagent**: Minimal coding agent that implements one subtask at a time
//!
//! # Example
//!
//! ```ignore
//! use agent::{Session, SessionConfig, write_session_start, load_session};
//!
//! // Create a new session
//! let config = SessionConfig {
//!     model: Some("claude-3".to_string()),
//!     max_turns: Some(10),
//!     ..Default::default()
//! };
//! let session = Session::new(config, "Implement feature X");
//!
//! // Write session events
//! let path = std::path::Path::new("/tmp/session.jsonl");
//! write_session_start(path, &session)?;
//!
//! // Later, load the session
//! let loaded = load_session(path)?;
//! ```

mod agent_lock;
mod agent_loop;
mod checkpoint;
mod claude_code_detector;
mod claude_code_mcp;
mod decompose;
mod error;
mod git;
mod golden_loop_fixture;
mod init_script;
mod install_deps;
mod orchestrator;
mod progress;
mod recovery;
mod session;
mod step_results;
mod subagent;
mod subagent_router;
mod tool_log_buffer;
mod types;
mod verification;
mod worktree;
mod worktree_guards;
mod sandbox_runner;
mod worktree_runner;

pub use agent_lock::*;
pub use agent_loop::*;
pub use checkpoint::*;
pub use claude_code_detector::*;
pub use claude_code_mcp::*;
pub use decompose::*;
pub use error::*;
pub use git::*;
pub use golden_loop_fixture::*;
pub use init_script::*;
pub use install_deps::*;
pub use orchestrator::*;
pub use progress::*;
pub use recovery::*;
pub use session::*;
pub use step_results::*;
pub use subagent::*;
pub use subagent_router::*;
pub use tool_log_buffer::*;
pub use types::*;
pub use verification::*;
pub use worktree::*;
pub use worktree_guards::*;
pub use sandbox_runner::*;
pub use worktree_runner::*;
