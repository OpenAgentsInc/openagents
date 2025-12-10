//! TBCC (TerminalBench Command Center) - Native GPUI implementation
//!
//! A 4-tab interface for Terminal-Bench execution and monitoring.

pub mod types;
pub mod dashboard;
pub mod task_browser;
pub mod run_browser;
pub mod settings;

// Re-exports
pub use types::*;
