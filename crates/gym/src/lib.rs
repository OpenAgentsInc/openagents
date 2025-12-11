//! Gym - Terminal-Bench Command Center
//!
//! A comprehensive workbench for Terminal-Bench with multi-view navigation,
//! real-time HillClimber visualization, TestGen monitoring, and TBCC integration.

pub mod actions;
pub mod types;
pub mod gym_screen;
pub mod trajectory_view;
pub mod trajectory_detail;

// TBCC sub-modules
pub mod tbcc;

// HillClimber visualization sub-modules
pub mod hillclimber;

// TestGen visualization sub-modules
pub mod testgen;

// RegexCrusade - laser-focused regex-log solver
pub mod regex_crusade;

// Data layer
pub mod websocket_client;
pub mod data_loader;
pub mod event_protocol;
pub mod services;

// Re-exports
pub use gym_screen::GymScreen;
pub use trajectory_view::TrajectoryView;
pub use types::GymTab;
pub use actions::register_actions;

// Tests
#[cfg(test)]
mod tests;
