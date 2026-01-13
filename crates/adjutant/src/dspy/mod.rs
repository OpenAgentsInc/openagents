//! DSPy integration for Adjutant.
//!
//! This module provides typed DSPy signatures for Adjutant's 3-phase
//! tiered execution, replacing hardcoded string prompts with optimizable
//! signatures that can be improved via MIPROv2.
//!
//! # Components
//!
//! - **module**: AdjutantModule composite module with signatures, implementing Module/Evaluator/Optimizable
//! - **metrics**: Evaluation metrics for each phase
//! - **training**: Training data collection and storage
//! - **lm_config**: Cerebras LM configuration for dsrs
//! - **decision_pipelines**: Decision signatures and pipeline wrappers for routing (complexity, delegation, RLM trigger)
//! - **sessions**: Session tracking for autopilot runs (self-improvement feedback loop)

pub mod auto_optimizer;
pub mod decision_pipelines;
pub mod issue_suggestion;
pub mod lm_config;
pub mod metrics;
pub mod module;
pub mod outcome_feedback;
pub mod performance;
pub mod sessions;
pub mod situation;
pub mod staleness;
pub mod training;
pub mod tool_step_utility;

pub use auto_optimizer::*;
pub use decision_pipelines::*;
pub use issue_suggestion::*;
pub use lm_config::*;
pub use metrics::*;
pub use module::*;
pub use outcome_feedback::*;
pub use performance::*;
pub use sessions::*;
pub use situation::*;
pub use staleness::*;
pub use training::*;
pub use tool_step_utility::tool_step_utility_predict;
