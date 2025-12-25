//! Fluent DSL for writing tests.
//!
//! Provides a builder pattern for creating test sequences.
//!
//! # Example
//!
//! ```rust,ignore
//! use wgpui::testing::{test, Test};
//!
//! let my_test = test("Login Flow")
//!     .click("#email-input")
//!     .type_text("user@example.com")
//!     .press_key(Key::Named(NamedKey::Tab))
//!     .type_text("password123")
//!     .click("#login-button")
//!     .wait(500)
//!     .expect("#dashboard")
//!     .build();
//! ```

use crate::testing::runner::TestRunner;
use crate::testing::step::{ClickTarget, ElementSelector, TestStep};
use crate::{Key, Modifiers, MouseButton, NamedKey};
use std::time::Duration;

/// Create a new test builder with a name.
pub fn test(name: impl Into<String>) -> Test {
    Test::new(name)
}

/// Fluent test builder.
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

    /// Click on an element (left button).
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

    /// Build the test into a TestRunner.
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

    #[test]
    fn test_builder_basic() {
        let t = test("My Test")
            .click("#button")
            .wait(100)
            .expect("#result");

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
}
