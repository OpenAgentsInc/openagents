//! Test fixtures for setting up domain and UI state.
//!
//! Fixtures provide reusable test setup and teardown logic.

mod domain;
mod mock_chat;
mod registry;

pub use domain::DomainFixture;
pub use mock_chat::MockChatService;
pub use registry::FixtureRegistry;

use std::any::Any;

/// Trait for test fixtures.
///
/// Fixtures are lazily initialized and cached for the duration
/// of a test. They provide a way to set up complex state that
/// may be shared across multiple story steps.
pub trait Fixture: Send + Sync {
    /// Set up the fixture (called on first access).
    fn setup(&mut self) {}

    /// Tear down the fixture (called when test ends).
    fn teardown(&mut self) {}

    /// Get as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Get as mutable Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Blanket implementation for fixtures that implement Default.
impl<T: Default + Send + Sync + 'static> Fixture for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
