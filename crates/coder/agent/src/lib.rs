//! Agent definitions and configuration for Coder.
//!
//! This crate provides:
//! - Agent definition types
//! - Built-in agent configurations (general, explore, plan, build)
//! - Agent registry for managing available agents
//! - Permission defaults per agent type

mod definition;
mod permission;
mod registry;

pub use definition::*;
pub use permission::*;
pub use registry::*;
