//! Adjutant Agent - DSPy-native ACP Agent
//!
//! A native DSPy agent with first-class planning mode support through dsrs signatures.

pub mod agent;
pub mod config;
pub mod lm_client;
pub mod plan_mode_metrics;
pub mod plan_mode_optimizer;
pub mod plan_mode_signatures;
pub mod plan_mode_training;
pub mod planning;

pub use agent::AdjutantAgent;
pub use config::PlanModeConfig;
