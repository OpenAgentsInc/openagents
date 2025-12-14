//! Test reporters for outputting results.
//!
//! Provides console and JSON reporters for different output needs.

mod console;
mod json;

pub use console::ConsoleReporter;
pub use json::JsonReporter;

use crate::runner::{StoryResult, TestResults};

/// Trait for test result reporters.
pub trait Reporter {
    /// Called when a test run starts.
    fn on_run_start(&self, total: usize);

    /// Called when a story starts executing.
    fn on_story_start(&self, name: &str);

    /// Called when a story completes.
    fn on_story_complete(&self, result: &StoryResult);

    /// Called when the entire run completes.
    fn on_run_complete(&self, results: &TestResults);
}
