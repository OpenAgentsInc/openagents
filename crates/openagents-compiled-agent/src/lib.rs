//! Rust-native compiled-agent substrate for the first narrow OpenAgents graph.

pub mod contracts;
pub mod eval;
pub mod graph;
pub mod manifest;
pub mod module;

pub use contracts::*;
pub use eval::*;
pub use graph::*;
pub use manifest::*;
pub use module::*;

