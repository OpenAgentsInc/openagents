//! E2E Test Live Viewer for WGPUI
//!
//! A testing framework that lets users watch automated tests execute in real-time,
//! with an overlay showing mouse/keyboard input visualization.
//!
//! # Features
//!
//! - **Fluent DSL**: Write tests using a builder pattern like `test("Login").click("#button").expect("#result")`
//! - **Live Visualization**: Watch test execution in real-time
//! - **Input Overlay**: See cursor position, click ripples, and key presses
//! - **Playback Control**: Play/pause/step with configurable speed
//!
//! # Example
//!
//! ```rust,ignore
//! use wgpui::testing::{test, TestHarness};
//!
//! let login_test = test("Login Flow")
//!     .click("#email-input")
//!     .type_text("user@example.com")
//!     .press_key(Key::Named(NamedKey::Tab))
//!     .type_text("password123")
//!     .click("#login-button")
//!     .wait(500)
//!     .expect("#dashboard")
//!     .build();
//!
//! let harness = TestHarness::new(my_component)
//!     .with_runner(login_test)
//!     .show_overlay(true)
//!     .show_controls(true);
//! ```

mod assertion;
mod context;
mod dsl;
mod harness;
mod injection;
mod overlay;
mod runner;
mod step;

pub use assertion::{AssertionResult, TestAssertion};
pub use context::{ComponentRegistry, TestContext};
pub use dsl::{test, Test};
pub use harness::TestHarness;
pub use injection::{EventPlayer, EventSequence, TimedEvent};
pub use overlay::{ClickRipple, InputOverlay, KeyDisplay, KeyDisplayPosition};
pub use runner::{PlaybackSpeed, RunnerState, TestRunner};
pub use step::{ClickTarget, ElementSelector, TestStep};
