//! Panel system for MechaCoder
//!
//! Provides collapsible panels for auxiliary functionality:
//! - Gym panel (Cmd+G): Terminal-Bench runs, TestGen, etc.

pub mod gym_panel;
pub mod tbench_runner;

pub use gym_panel::{GymPanel, GymPanelEvent};
pub use tbench_runner::{TBenchRunner, TBenchRunnerEvent, TBRunOptions};
