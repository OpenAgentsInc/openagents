//! Panel system for MechaCoder
//!
//! Provides collapsible panels for auxiliary functionality:
//! - Gym panel (Cmd+G): Terminal-Bench runs, TestGen, etc.
//! - Claude panel (Cmd+C): SDK configuration and status

pub mod claude_panel;
pub mod docker_runner;
pub mod gym_panel;
pub mod harbor_runner;
pub mod runner_event;
pub mod testgen_wrapper;
pub mod verifier;

pub use claude_panel::{ClaudePanel, ClaudePanelEvent, CostTracker};
pub use docker_runner::{DockerEvent, DockerRunConfig, DockerRunResult, DockerRunner};
pub use gym_panel::{GymPanel, GymPanelEvent};
pub use harbor_runner::{HarborRunConfig, HarborRunError, HarborRunner};
pub use runner_event::TB2RunnerEvent;
pub use testgen_wrapper::TestGenWrapper;
pub use verifier::{TB2Verifier, VerificationResult};
