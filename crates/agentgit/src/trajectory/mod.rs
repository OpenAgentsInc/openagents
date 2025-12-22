//! Trajectory handling for AgentGit
//!
//! This module handles NIP-SA trajectory events for agent work verification.

pub mod verifier;
pub mod diff_compare;

pub use verifier::*;
pub use diff_compare::*;
