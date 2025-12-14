//! Comprehensive testing framework for Coder.
//!
//! This crate provides a fully custom, Rust-only testing framework supporting:
//! - Unit tests
//! - Feature tests with story DSL
//! - Integration tests
//! - E2E tests (optional)
//! - Headless widget testing without GPU
//!
//! # Story DSL
//!
//! ```rust,ignore
//! use coder_test::prelude::*;
//!
//! story!("User can send a message")
//!     .given(|cx| {
//!         cx.fixture::<DomainFixture>()
//!             .with_session("test");
//!     })
//!     .when(|cx| {
//!         cx.actions()
//!             .type_text("Hello!")
//!             .press_key(Key::Enter);
//!     })
//!     .then(|cx| {
//!         cx.assert_scene_contains_text("Hello!");
//!     })
//!     .run();
//! ```

pub mod actions;
pub mod assertions;
pub mod fixtures;
pub mod harness;
pub mod platform;
pub mod reactive;
pub mod report;
pub mod runner;
pub mod story;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::actions::UserActions;
    pub use crate::assertions::SceneAssertions;
    pub use crate::fixtures::{DomainFixture, Fixture, MockChatService};
    pub use crate::harness::{MountedWidget, TestHarness};
    pub use crate::platform::MockPlatform;
    pub use crate::reactive::{EffectTracker, MemoTracker, SignalTracker};
    pub use crate::report::{ConsoleReporter, JsonReporter, Reporter};
    pub use crate::runner::{Parallelism, RunnerConfig, TestResults, TestRunner};
    pub use crate::story::{Story, StoryBuilder, TestContext};
    pub use crate::{assert_scene, story};

    // Re-export commonly used types from dependencies
    pub use wgpui::{
        Bounds, InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey, Point, Scene, Size,
    };
}

/// Create a new story test.
///
/// # Example
///
/// ```rust,ignore
/// story!("User can create a session")
///     .given(|cx| { /* setup */ })
///     .when(|cx| { /* action */ })
///     .then(|cx| { /* assertion */ })
///     .run();
/// ```
#[macro_export]
macro_rules! story {
    ($name:expr) => {
        $crate::story::StoryBuilder::new($name)
    };
}

/// Assert properties about a scene.
///
/// # Example
///
/// ```rust,ignore
/// assert_scene!(scene, contains_text "Hello");
/// assert_scene!(scene, quad_count 5);
/// ```
#[macro_export]
macro_rules! assert_scene {
    ($scene:expr, contains_text $text:expr) => {{
        use $crate::assertions::SceneAssertions;
        assert!(
            $scene.contains_text($text),
            "Expected scene to contain text '{}', but it didn't.\nScene has {} text runs.",
            $text,
            $scene.text_run_count()
        );
    }};
    ($scene:expr, not contains_text $text:expr) => {{
        use $crate::assertions::SceneAssertions;
        assert!(
            !$scene.contains_text($text),
            "Expected scene NOT to contain text '{}', but it did.",
            $text
        );
    }};
    ($scene:expr, quad_count $count:expr) => {{
        use $crate::assertions::SceneAssertions;
        let actual = $scene.quad_count();
        assert_eq!(
            actual, $count,
            "Expected scene to have {} quads, but found {}.",
            $count, actual
        );
    }};
    ($scene:expr, text_run_count $count:expr) => {{
        use $crate::assertions::SceneAssertions;
        let actual = $scene.text_run_count();
        assert_eq!(
            actual, $count,
            "Expected scene to have {} text runs, but found {}.",
            $count, actual
        );
    }};
}
