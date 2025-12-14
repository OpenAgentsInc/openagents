//! Story DSL for behavior-driven testing.
//!
//! This module provides a fluent API for writing tests in a
//! given/when/then format that maps directly to user stories.

mod builder;
mod context;
mod inventory;

pub use builder::StoryBuilder;
pub use context::TestContext;
pub use inventory::{Story, StoryInventory, StoryOutcome};
