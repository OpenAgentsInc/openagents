//! # ATIF (Agent Trajectory Interchange Format) v1.4
//!
//! A standardized, JSON-based specification for logging the complete interaction
//! history of autonomous LLM agents.
//!
//! ## Usage
//!
//! ```rust
//! use atif::*;
//!
//! let trajectory = Trajectory {
//!     schema_version: "ATIF-v1.4".to_string(),
//!     session_id: "ABC123".to_string(),
//!     agent: Agent {
//!         name: "my-agent".to_string(),
//!         version: "1.0.0".to_string(),
//!         model_name: Some("claude-3-5-sonnet".to_string()),
//!         extra: None,
//!     },
//!     steps: vec![],
//!     notes: None,
//!     final_metrics: None,
//!     extra: None,
//! };
//!
//! let json = serde_json::to_string_pretty(&trajectory).unwrap();
//! ```

pub mod agent;
pub mod error;
pub mod final_metrics;
pub mod metrics;
pub mod observation;
pub mod observation_result;
pub mod step;
pub mod subagent_trajectory_ref;
pub mod tool_call;
pub mod trajectory;

// Re-export main types
pub use agent::Agent;
pub use error::AtifError;
pub use final_metrics::FinalMetrics;
pub use metrics::Metrics;
pub use observation::Observation;
pub use observation_result::ObservationResult;
pub use step::{Step, StepSource, ReasoningEffort};
pub use subagent_trajectory_ref::SubagentTrajectoryRef;
pub use tool_call::ToolCall;
pub use trajectory::Trajectory;

pub type Result<T> = std::result::Result<T, AtifError>;
