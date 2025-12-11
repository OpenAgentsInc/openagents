//! Test fixtures for Gym components
//!
//! Each fixture follows the Page Object Model pattern:
//! - Static methods for creating components
//! - Static methods for querying state
//! - Static methods for mutations/actions

mod gym_screen_fixture;
mod tbcc_fixture;
mod hillclimber_fixture;
mod testgen_fixture;
mod trajectory_fixture;
mod assertions;

pub use gym_screen_fixture::*;
pub use tbcc_fixture::*;
pub use hillclimber_fixture::*;
pub use testgen_fixture::*;
pub use trajectory_fixture::*;
pub use assertions::*;

// Re-export types from gym for convenient test imports
pub mod types {
    pub use crate::GymTab;
    pub use crate::tbcc::TBCCTab;
    pub use crate::hillclimber::monitor::{HCMode, HCSession, HCSessionStatus};
    pub use crate::testgen::visualizer::{TestGenStatus, TestGenSession, GenerationStatus};
    pub use crate::testgen::category_progress::TestCategory;
}
