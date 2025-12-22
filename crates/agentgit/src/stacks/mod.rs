//! Stacked diffs support module
//!
//! Provides functionality for managing stacked PRs (pull requests that depend on each other).

pub mod graph;
pub mod restack;

pub use graph::StackGraph;
pub use restack::restack_layers;
