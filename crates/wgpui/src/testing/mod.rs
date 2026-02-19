//! # E2E Test Live Viewer for WGPUI
//!
//! A testing framework that lets users watch automated tests execute in real-time,
//! with an overlay showing mouse/keyboard input visualization.
//!
//! ## Features
//!
//! - **Fluent DSL**: Write tests using a builder pattern
//! - **Live Visualization**: Watch test execution in real-time
//! - **Input Overlay**: See cursor position, click ripples, and key presses
//! - **Playback Control**: Play/pause/step with configurable speed
//! - **Assertions**: Verify element existence, text content, visibility
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use wgpui::testing::{test, TestHarness, PlaybackSpeed};
//!
//! // 1. Define a test using the fluent DSL
//! let login_test = test("Login Flow")
//!     .click("#email-input")
//!     .type_text("user@example.com")
//!     .press_tab()
//!     .type_text("password123")
//!     .click("#login-button")
//!     .wait(500)
//!     .expect("#dashboard")
//!     .expect_text("#welcome", "Welcome back!")
//!     .build();
//!
//! // 2. Wrap your component in a TestHarness
//! let harness = TestHarness::new(my_component)
//!     .with_runner(login_test)
//!     .show_overlay(true)
//!     .show_controls(true);
//!
//! // 3. Render as normal - the harness handles test execution
//! ```
//!
//! ## DSL Methods
//!
//! ### Mouse Actions
//!
//! | Method | Description |
//! |--------|-------------|
//! | `click(selector)` | Left-click on element |
//! | `click_at(x, y)` | Click at coordinates |
//! | `right_click(selector)` | Right-click on element |
//! | `double_click(selector)` | Double-click on element |
//! | `hover(selector)` | Move to element |
//!
//! ### Keyboard Actions
//!
//! | Method | Description |
//! |--------|-------------|
//! | `type_text(text)` | Type with 50ms delay |
//! | `type_instant(text)` | Type immediately |
//! | `press_key(key)` | Press a key |
//! | `press_enter()` | Press Enter |
//! | `press_tab()` | Press Tab |
//! | `press_escape()` | Press Escape |
//!
//! ### Timing
//!
//! | Method | Description |
//! |--------|-------------|
//! | `wait(ms)` | Wait milliseconds |
//! | `wait_for(selector)` | Wait for element (5s timeout) |
//! | `wait_for_timeout(sel, ms)` | Wait with custom timeout |
//!
//! ### Assertions
//!
//! | Method | Description |
//! |--------|-------------|
//! | `expect(selector)` | Assert element exists |
//! | `expect_text(sel, text)` | Assert element has text |
//! | `expect_visible(selector)` | Assert element visible |
//!
//! ## Element Selectors
//!
//! Elements can be selected using:
//!
//! - `#123` - By ComponentId (e.g., `"#42"`)
//! - `text:Content` - By visible text (e.g., `"text:Submit"`)
//! - Plain string - Treated as text search (e.g., `"Submit"`)
//!
//! ## Playback Speed
//!
//! ```rust,ignore
//! use wgpui::testing::PlaybackSpeed;
//!
//! runner.set_speed(PlaybackSpeed::SLOW);    // 0.5x
//! runner.set_speed(PlaybackSpeed::NORMAL);  // 1.0x
//! runner.set_speed(PlaybackSpeed::FAST);    // 2.0x
//! runner.set_speed(PlaybackSpeed::INSTANT); // 10.0x
//! ```
//!
//! ## Control Bar Shortcuts
//!
//! When `show_controls(true)` is enabled:
//!
//! - `P` - Start/resume playback
//! - `S` - Execute single step
//! - `Space` - Pause/resume
//! - `Escape` - Abort test
//! - `1-4` - Set speed (Slow/Normal/Fast/Instant)
//!
//! ## Input Overlay
//!
//! The overlay visualizes test input:
//!
//! - **Cursor**: Crosshair at current mouse position
//! - **Click Ripples**: Expanding circles on click (400ms animation)
//! - **Key Display**: Recent key presses in corner (fades after 800ms)
//!
//! ## Module Organization
//!
//! - [`step`] - TestStep, ElementSelector, ClickTarget types
//! - [`runner`] - TestRunner state machine and playback control
//! - [`dsl`] - Fluent Test builder API
//! - [`harness`] - TestHarness wrapper component
//! - [`overlay`] - InputOverlay for visualization
//! - [`injection`] - EventSequence for synthetic events
//! - [`context`] - ComponentRegistry for element lookup
//! - [`assertion`] - TestAssertion and AssertionResult

mod assertion;
mod context;
mod dsl;
mod harness;
mod injection;
mod overlay;
mod recorder;
mod runner;
mod step;

#[cfg(test)]
mod chat_tests;

#[cfg(test)]
mod component_tests;

#[cfg(test)]
mod framework_tests;

#[cfg(test)]
mod snapshot_tests;

pub use assertion::{AssertionResult, TestAssertion};
pub use context::{ComponentRegistry, TestContext};
pub use dsl::{Test, test};
pub use harness::TestHarness;
pub use injection::{EventPlayer, EventSequence, TimedEvent, generate_step_events};
pub use overlay::{ClickRipple, InputOverlay, KeyDisplay, KeyDisplayPosition};
pub use recorder::TestRecorder;
pub use runner::{PlaybackSpeed, RunnerState, StepResult, TestRunner};
pub use step::{ClickTarget, ElementSelector, TestStep};
