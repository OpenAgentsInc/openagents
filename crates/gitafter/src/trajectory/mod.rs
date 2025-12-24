//! Trajectory handling for GitAfter
//!
//! This module handles NIP-SA trajectory events for agent work verification.

pub mod diff_compare;
pub mod fetch;
pub mod verifier;

pub use diff_compare::*;
pub use fetch::*;
pub use verifier::*;
