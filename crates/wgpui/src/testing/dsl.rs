//! Fluent DSL for writing E2E tests.
//!
//! Provides a builder pattern for creating test sequences with an intuitive,
//! chainable API.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use wgpui::testing::{test, Test};
//!
//! let my_test = test("Login Flow")
//!     .click("#email-input")
//!     .type_text("user@example.com")
//!     .press_tab()
//!     .type_text("password123")
//!     .click("#login-button")
//!     .wait(500)
//!     .expect("#dashboard")
//!     .build();
//! ```
//!
//! # Element Selectors
//!
//! Elements can be selected using string patterns:
//!
//! - `#123` - Select by ComponentId (the number after #)
//! - `text:Hello` - Select by visible text content
//! - `Hello` - Plain strings are treated as text search
//!
//! ```rust,ignore
//! test("Selectors")
//!     .click("#42")              // Click element with ComponentId 42
//!     .click("text:Submit")      // Click element showing "Submit"
//!     .click("Cancel")           // Also clicks element with text "Cancel"
//!     .build();
//! ```
//!
//! # Mouse Actions
//!
//! ```rust,ignore
//! test("Mouse Actions")
//!     .click("#button")              // Left click
//!     .click_at(100.0, 200.0)        // Click at coordinates
//!     .right_click("#menu-trigger")  // Right click (context menu)
//!     .double_click("#file")         // Double click
//!     .hover("#tooltip-target")      // Move without clicking
//!     .build();
//! ```
//!
//! # Keyboard Actions
//!
//! ```rust,ignore
//! use wgpui::{Key, Modifiers, NamedKey};
//!
//! test("Keyboard Actions")
//!     .type_text("Hello!")           // Type with 50ms delay per char
//!     .type_instant("Fast typing")   // Type immediately
//!     .type_with_delay("Slow", 100)  // Custom delay (100ms)
//!     .press_enter()                 // Common keys have shortcuts
//!     .press_tab()
//!     .press_escape()
//!     .press_backspace()
//!     .press_key(Key::Named(NamedKey::ArrowDown))
//!     .press_key_with(
//!         Key::Character("s".to_string()),
//!         Modifiers { ctrl: true, ..Default::default() }
//!     )  // Ctrl+S
//!     .build();
//! ```
//!
//! # Scrolling
//!
//! ```rust,ignore
//! test("Scrolling")
//!     .scroll("#container", 0.0, 100.0)  // Scroll by dx, dy
//!     .scroll_down("#list", 50.0)        // Scroll down
//!     .scroll_up("#list", 25.0)          // Scroll up
//!     .build();
//! ```
//!
//! # Timing
//!
//! ```rust,ignore
//! use std::time::Duration;
//!
//! test("Timing")
//!     .click("#async-action")
//!     .wait(1000)                              // Wait 1 second
//!     .wait_duration(Duration::from_secs(2))   // Wait 2 seconds
//!     .wait_for("#loading-done")               // Wait for element (5s timeout)
//!     .wait_for_timeout("#slow-load", 15000)   // Custom timeout (15s)
//!     .build();
//! ```
//!
//! # Assertions
//!
//! ```rust,ignore
//! test("Assertions")
//!     .click("#submit")
//!     .expect("#success-message")          // Element exists
//!     .expect_text("#status", "Complete")  // Has specific text
//!     .expect_visible("#main-content")     // Is visible on screen
//!     .build();
//! ```
//!
//! # Complete Example
//!
//! ```rust,ignore
//! use wgpui::testing::{test, TestHarness, PlaybackSpeed};
//!
//! // Build the test
//! let counter_test = test("Counter Operations")
//!     .click("#increment")
//!     .expect_text("#count", "1")
//!     .click("#increment")
//!     .click("#increment")
//!     .expect_text("#count", "3")
//!     .click("#decrement")
//!     .expect_text("#count", "2")
//!     .click("#reset")
//!     .expect_text("#count", "0")
//!     .build();
//!
//! // Run it
//! counter_test.set_speed(PlaybackSpeed::SLOW);
//! let harness = TestHarness::new(my_counter)
//!     .with_runner(counter_test)
//!     .show_overlay(true);
//! ```

use crate::testing::runner::TestRunner;
use crate::testing::step::{ClickTarget, ElementSelector, TestStep};
use crate::{Key, Modifiers, MouseButton, NamedKey};
use std::time::Duration;

/// Create a new test builder with a name.
///
/// This is the entry point for the fluent test DSL. Chain methods to add
/// steps, then call `.build()` to get a [`TestRunner`].
///
/// # Example
///
/// ```rust,ignore
/// use wgpui::testing::test;
///
/// let runner = test("My Test")
///     .click("#button")
///     .expect("#result")
///     .build();
/// ```
pub fn test(name: impl Into<String>) -> Test {
    Test::new(name)
}

/// Fluent test builder.
///
/// Use the [`test()`] function to create instances. Chain methods to add
/// test steps, and call [`build()`](Test::build) to create a [`TestRunner`].
///
/// # Example
///
/// ```rust,ignore
/// use wgpui::testing::test;
///
/// let runner = test("Form Submission")
///     .click("#name-input")
///     .type_text("John Doe")
///     .click("#email-input")
///     .type_text("john@example.com")
///     .click("#submit")
///     .wait(500)
///     .expect("#success-message")
///     .build();
///
/// // Access test metadata before building
/// let builder = test("Counter")
///     .click("#increment")
///     .click("#increment");
/// println!("Test has {} steps", builder.step_count());
/// let runner = builder.build();
/// ```
pub struct Test {
    name: String,
    steps: Vec<TestStep>,
}

impl Test {
    /// Create a new test with a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
        }
    }

    /// Add a step to the test.
    pub fn step(mut self, step: TestStep) -> Self {
        self.steps.push(step);
        self
    }

    // --- Click methods ---

    /// Click on an element (left mouse button).
    ///
    /// The selector can be:
    /// - `"#123"` - ComponentId 123
    /// - `"text:Submit"` - Element with visible text "Submit"
    /// - `"Submit"` - Also searches by text
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// test("Clicks")
    ///     .click("#submit-button")
    ///     .click("text:Cancel")
    ///     .click("OK")
    ///     .build();
    /// ```
    pub fn click(self, selector: impl Into<String>) -> Self {
        let target = ClickTarget::from_selector(&selector.into());
        self.step(TestStep::Click {
            target,
            button: MouseButton::Left,
        })
    }

    /// Click at specific coordinates.
    pub fn click_at(self, x: f32, y: f32) -> Self {
        self.step(TestStep::Click {
            target: ClickTarget::at(x, y),
            button: MouseButton::Left,
        })
    }

    /// Right-click on an element.
    pub fn right_click(self, selector: impl Into<String>) -> Self {
        let target = ClickTarget::from_selector(&selector.into());
        self.step(TestStep::Click {
            target,
            button: MouseButton::Right,
        })
    }

    /// Double-click on an element.
    pub fn double_click(self, selector: impl Into<String>) -> Self {
        let target = ClickTarget::from_selector(&selector.into());
        self.step(TestStep::DoubleClick {
            target,
            button: MouseButton::Left,
        })
    }

    // --- Type methods ---

    /// Type text with default delay (50ms per character).
    pub fn type_text(self, text: impl Into<String>) -> Self {
        self.step(TestStep::Type {
            text: text.into(),
            delay_per_char: Some(Duration::from_millis(50)),
        })
    }

    /// Type text instantly (no delay).
    pub fn type_instant(self, text: impl Into<String>) -> Self {
        self.step(TestStep::Type {
            text: text.into(),
            delay_per_char: None,
        })
    }

    /// Type text with custom delay per character.
    pub fn type_with_delay(self, text: impl Into<String>, delay_ms: u64) -> Self {
        self.step(TestStep::Type {
            text: text.into(),
            delay_per_char: Some(Duration::from_millis(delay_ms)),
        })
    }

    // --- Key methods ---

    /// Press a key.
    pub fn press_key(self, key: Key) -> Self {
        self.step(TestStep::KeyPress {
            key,
            modifiers: Modifiers::default(),
        })
    }

    /// Press a key with modifiers.
    pub fn press_key_with(self, key: Key, modifiers: Modifiers) -> Self {
        self.step(TestStep::KeyPress { key, modifiers })
    }

    /// Press Enter key.
    pub fn press_enter(self) -> Self {
        self.press_key(Key::Named(NamedKey::Enter))
    }

    /// Press Tab key.
    pub fn press_tab(self) -> Self {
        self.press_key(Key::Named(NamedKey::Tab))
    }

    /// Press Escape key.
    pub fn press_escape(self) -> Self {
        self.press_key(Key::Named(NamedKey::Escape))
    }

    /// Press Backspace key.
    pub fn press_backspace(self) -> Self {
        self.press_key(Key::Named(NamedKey::Backspace))
    }

    // --- Navigation methods ---

    /// Hover over an element (move mouse without clicking).
    pub fn hover(self, selector: impl Into<String>) -> Self {
        let target = ClickTarget::from_selector(&selector.into());
        self.step(TestStep::MoveTo { target })
    }

    /// Move mouse to specific coordinates.
    pub fn move_to(self, x: f32, y: f32) -> Self {
        self.step(TestStep::MoveTo {
            target: ClickTarget::at(x, y),
        })
    }

    /// Scroll at an element.
    pub fn scroll(self, selector: impl Into<String>, dx: f32, dy: f32) -> Self {
        let target = ClickTarget::from_selector(&selector.into());
        self.step(TestStep::Scroll { target, dx, dy })
    }

    /// Scroll down at an element.
    pub fn scroll_down(self, selector: impl Into<String>, amount: f32) -> Self {
        self.scroll(selector, 0.0, amount)
    }

    /// Scroll up at an element.
    pub fn scroll_up(self, selector: impl Into<String>, amount: f32) -> Self {
        self.scroll(selector, 0.0, -amount)
    }

    // --- Wait methods ---

    /// Wait for a duration (in milliseconds).
    pub fn wait(self, ms: u64) -> Self {
        self.step(TestStep::Wait {
            duration: Duration::from_millis(ms),
        })
    }

    /// Wait for a duration.
    pub fn wait_duration(self, duration: Duration) -> Self {
        self.step(TestStep::Wait { duration })
    }

    /// Wait for an element to appear (default 5s timeout).
    pub fn wait_for(self, selector: impl Into<String>) -> Self {
        self.step(TestStep::WaitFor {
            selector: ElementSelector::parse(&selector.into()),
            timeout: Duration::from_secs(5),
        })
    }

    /// Wait for an element with custom timeout (in milliseconds).
    pub fn wait_for_timeout(self, selector: impl Into<String>, timeout_ms: u64) -> Self {
        self.step(TestStep::WaitFor {
            selector: ElementSelector::parse(&selector.into()),
            timeout: Duration::from_millis(timeout_ms),
        })
    }

    // --- Assertion methods ---

    /// Assert that an element exists.
    pub fn expect(self, selector: impl Into<String>) -> Self {
        self.step(TestStep::Expect {
            selector: ElementSelector::parse(&selector.into()),
        })
    }

    /// Assert that an element contains specific text.
    pub fn expect_text(self, selector: impl Into<String>, text: impl Into<String>) -> Self {
        self.step(TestStep::ExpectText {
            selector: ElementSelector::parse(&selector.into()),
            text: text.into(),
        })
    }

    /// Assert that an element is visible.
    pub fn expect_visible(self, selector: impl Into<String>) -> Self {
        self.step(TestStep::ExpectVisible {
            selector: ElementSelector::parse(&selector.into()),
        })
    }

    /// Build the test into a [`TestRunner`].
    ///
    /// This consumes the builder and creates an executable test runner.
    /// The runner can be passed to a [`TestHarness`](super::TestHarness)
    /// for live visualization.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use wgpui::testing::{test, TestHarness, PlaybackSpeed};
    ///
    /// let runner = test("My Test")
    ///     .click("#button")
    ///     .expect("#result")
    ///     .build();
    ///
    /// // Configure the runner
    /// runner.set_speed(PlaybackSpeed::SLOW);
    ///
    /// // Use with harness
    /// let harness = TestHarness::new(component).with_runner(runner);
    /// ```
    pub fn build(self) -> TestRunner {
        TestRunner::new(self.name, self.steps)
    }

    /// Get the test name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the current step count.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::step::{ElementSelector, TestStep};

    #[test]
    fn test_builder_basic() {
        let t = test("My Test").click("#button").wait(100).expect("#result");

        assert_eq!(t.name(), "My Test");
        assert_eq!(t.step_count(), 3);
    }

    #[test]
    fn test_builder_click_methods() {
        let t = test("Clicks")
            .click("#left")
            .click_at(100.0, 200.0)
            .right_click("#context")
            .double_click("#file");

        assert_eq!(t.step_count(), 4);
    }

    #[test]
    fn test_builder_type_methods() {
        let t = test("Typing")
            .type_text("hello")
            .type_instant("world")
            .type_with_delay("slow", 100);

        assert_eq!(t.step_count(), 3);
    }

    #[test]
    fn test_builder_key_methods() {
        let t = test("Keys")
            .press_enter()
            .press_tab()
            .press_escape()
            .press_backspace()
            .press_key(Key::Character("a".to_string()));

        assert_eq!(t.step_count(), 5);
    }

    #[test]
    fn test_builder_navigation_methods() {
        let t = test("Navigation")
            .hover("#menu")
            .move_to(50.0, 50.0)
            .scroll("#list", 0.0, 100.0)
            .scroll_down("#list", 50.0)
            .scroll_up("#list", 25.0);

        assert_eq!(t.step_count(), 5);
    }

    #[test]
    fn test_builder_wait_methods() {
        let t = test("Waits")
            .wait(100)
            .wait_duration(Duration::from_secs(1))
            .wait_for("#element")
            .wait_for_timeout("#slow", 10000);

        assert_eq!(t.step_count(), 4);
    }

    #[test]
    fn test_builder_assertion_methods() {
        let t = test("Assertions")
            .expect("#element")
            .expect_text("#message", "Hello")
            .expect_visible("#panel");

        assert_eq!(t.step_count(), 3);
    }

    #[test]
    fn test_builder_builds_runner() {
        let runner = test("Complete Test")
            .click("#submit")
            .wait(500)
            .expect("#success")
            .build();

        assert_eq!(runner.name(), "Complete Test");
        assert_eq!(runner.total_steps(), 3);
    }

    #[test]
    fn test_login_flow_example() {
        let runner = test("Login Flow")
            .click("#email-input")
            .type_text("user@example.com")
            .press_tab()
            .type_text("password123")
            .click("#login-button")
            .wait(500)
            .expect("#dashboard")
            .expect_text("#welcome", "Welcome")
            .build();

        assert_eq!(runner.name(), "Login Flow");
        assert_eq!(runner.total_steps(), 8);
    }

    #[test]
    fn test_builder_type_text_default_delay() {
        let runner = test("Typing").type_text("hi").build();

        match &runner.steps()[0] {
            TestStep::Type {
                text,
                delay_per_char,
            } => {
                assert_eq!(text, "hi");
                assert_eq!(*delay_per_char, Some(Duration::from_millis(50)));
            }
            _ => panic!("Expected Type step"),
        }
    }

    #[test]
    fn test_builder_expect_parses_selector() {
        let runner = test("Expect").expect("#42").build();

        match &runner.steps()[0] {
            TestStep::Expect {
                selector: ElementSelector::Id(42),
            } => {}
            _ => panic!("Expected Expect step with Id selector"),
        }
    }

    #[test]
    fn test_builder_wait_for_default_timeout() {
        let runner = test("Wait").wait_for("#7").build();

        match &runner.steps()[0] {
            TestStep::WaitFor {
                selector: ElementSelector::Id(7),
                timeout,
            } => {
                assert_eq!(*timeout, Duration::from_secs(5));
            }
            _ => panic!("Expected WaitFor step with default timeout"),
        }
    }

    #[test]
    fn test_builder_wait_for_custom_timeout() {
        let runner = test("Wait").wait_for_timeout("#7", 1500).build();

        match &runner.steps()[0] {
            TestStep::WaitFor {
                selector: ElementSelector::Id(7),
                timeout,
            } => {
                assert_eq!(*timeout, Duration::from_millis(1500));
            }
            _ => panic!("Expected WaitFor step with custom timeout"),
        }
    }
}
