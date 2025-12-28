//! Host mode for Pylon
//!
//! Runs sovereign agents on your machine. Each agent:
//! - Has its own identity and wallet
//! - Executes tick cycles autonomously
//! - Pays for compute with its Bitcoin wallet
//! - Goes dormant when funds run out

pub mod runner;

pub use runner::{AgentHandle, AgentRunner};
