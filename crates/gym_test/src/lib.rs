//! Test fixtures and assertions for the Gym crate
//!
//! This crate provides:
//! - Page Object Model fixtures for each Gym component
//! - Fluent assertion helpers
//! - Test utilities for GPUI component testing

pub mod fixtures;

pub use fixtures::*;

// Re-export types from gym for convenient test imports
pub mod types {
    pub use gym::GymTab;
    pub use gym::tbcc::TBCCTab;
    pub use gym::hillclimber::monitor::{HCMode, HCSession, HCSessionStatus};
    pub use gym::testgen::visualizer::{TestGenStatus, TestGenSession};
}
