//! Mock platform implementations for testing.
//!
//! Provides platform abstractions that don't require actual
//! system resources like GPU, clipboard, etc.

mod browser;
mod mock;

pub use browser::MockBrowserAPI;
pub use mock::MockPlatform;
