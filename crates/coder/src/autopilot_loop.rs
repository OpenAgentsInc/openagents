//! Autonomous autopilot loop for continuous task execution.
//!
//! This module re-exports the core AutopilotLoop from adjutant and provides
//! Coder-specific output implementations for the desktop UI.

// Re-export core types from adjutant
pub use adjutant::autopilot_loop::{
    AutopilotConfig, AutopilotLoop, AutopilotOutput, AutopilotResult, ChannelOutput, CliOutput,
    Verification,
};
