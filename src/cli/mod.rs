//! CLI subcommands for unified OpenAgents binary
//!
//! Each module wraps the CLI functionality from its respective crate.

pub mod agent;
pub mod auth;
pub mod autopilot;
pub mod gitafter;
pub mod marketplace;
pub mod pylon;
#[cfg(feature = "fm-bridge")]
pub mod rlm;
pub mod wallet;
