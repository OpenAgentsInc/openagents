//! Shared Testing Utilities
//!
//! This crate provides common testing infrastructure used across OpenAgents test suites.
//! It eliminates duplication and provides consistent patterns for:
//!
//! - TestApp for integration testing
//! - Mock implementations of common traits
//! - Test fixtures and factories
//! - Assertion helpers

pub mod fixtures;
pub mod mocks;
pub mod test_app;

// Re-export commonly used testing utilities
pub use fixtures::*;
pub use mocks::*;
pub use test_app::*;
