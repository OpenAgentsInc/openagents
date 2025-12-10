//! Gym - Terminal-Bench Command Center
//!
//! A comprehensive workbench for Terminal-Bench with multi-view navigation,
//! real-time HillClimber visualization, TestGen monitoring, and TBCC integration.

pub mod actions;
pub mod types;
pub mod gym_screen;
pub mod sidebar;
pub mod trajectory_view;
pub mod trajectory_detail;

// TBCC sub-modules
pub mod tbcc;

// HillClimber visualization sub-modules
pub mod hillclimber;

// TestGen visualization sub-modules
pub mod testgen;

// Data layer
pub mod websocket_client;
pub mod data_loader;
pub mod event_protocol;

// Re-exports
pub use gym_screen::GymScreen;
pub use sidebar::Sidebar;
pub use trajectory_view::TrajectoryView;
pub use types::{GymTab, TreeNode, TreeItemKind, ItemStatus};
pub use actions::register_actions;
