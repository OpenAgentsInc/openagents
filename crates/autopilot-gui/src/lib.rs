//! Autopilot GUI - Visual interface for OpenAgents Autopilot
//!
//! This crate provides a native GUI application that wraps the autopilot
//! functionality with a rich visual interface for:
//! - Real-time agent execution monitoring
//! - Visual permission management
//! - Session browsing and resumption
//! - Context inspection
//! - Multi-agent orchestration

pub mod server;
pub mod views;
pub mod window;

pub use server::Server;
pub use window::Window;
