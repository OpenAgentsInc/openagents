//! Dynamic tool registry with async execution and cancellation support.
//!
//! This crate provides:
//! - `Tool` trait for implementing tools with async execution
//! - `ToolContext` with cancellation token and working directory
//! - `ToolRegistry` for managing available tools
//! - JSON Schema generation for tool parameters
//! - Tool wrappers for existing implementations

mod context;
mod error;
mod registry;
mod schema;
mod tool;
pub mod wrappers;

pub use context::*;
pub use error::*;
pub use registry::*;
pub use schema::*;
pub use tool::*;

// Re-export for convenience
pub use schemars::JsonSchema;
