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

pub mod decision_pipelines;
pub mod lm_config;
pub mod metrics;
pub mod module;
pub mod training;

pub use decision_pipelines::*;
pub use lm_config::*;
pub use metrics::*;
pub use module::*;
pub use training::*;
