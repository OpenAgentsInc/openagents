//! Adjutant Agent - DSPy-native ACP Agent
//!
//! A native DSPy agent with first-class planning mode support through dsrs signatures.

pub mod agent;
pub mod signatures;
pub mod planning;
pub mod lm_client;

pub use agent::AdjutantAgent;
