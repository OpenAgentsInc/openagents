//! Test assertions and results.
//!
//! Provides types for defining and evaluating test assertions.

use crate::testing::step::ElementSelector;
use std::fmt;

/// The result of evaluating a test assertion.
#[derive(Clone, Debug)]
pub enum AssertionResult {
    /// Assertion passed.
    Passed,
    /// Assertion failed with a reason.
    Failed { reason: String },
    /// Assertion could not be evaluated (e.g., element not found).
    Error { message: String },
}

impl AssertionResult {
    /// Returns true if the assertion passed.
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }

    /// Returns true if the assertion failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns true if there was an error evaluating the assertion.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Create a failed result with a reason.
    pub fn failed(reason: impl Into<String>) -> Self {
        Self::Failed {
            reason: reason.into(),
        }
    }

    /// Create an error result with a message.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }
}

impl fmt::Display for AssertionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Passed => write!(f, "PASSED"),
            Self::Failed { reason } => write!(f, "FAILED: {}", reason),
            Self::Error { message } => write!(f, "ERROR: {}", message),
        }
    }
}

/// A test assertion to verify.
#[derive(Clone, Debug)]
pub enum TestAssertion {
    /// Assert that an element exists.
    ElementExists { selector: ElementSelector },
    /// Assert that an element contains specific text.
    ElementHasText {
        selector: ElementSelector,
        expected_text: String,
    },
    /// Assert that an element is visible (not hidden/collapsed).
    ElementVisible { selector: ElementSelector },
    /// Assert that an element has specific bounds.
    ElementInBounds {
        selector: ElementSelector,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        tolerance: f32,
    },
}

impl TestAssertion {
    /// Create an assertion that an element exists.
    pub fn exists(selector: ElementSelector) -> Self {
        Self::ElementExists { selector }
    }

    /// Create an assertion that an element has specific text.
    pub fn has_text(selector: ElementSelector, expected_text: impl Into<String>) -> Self {
        Self::ElementHasText {
            selector,
            expected_text: expected_text.into(),
        }
    }

    /// Create an assertion that an element is visible.
    pub fn visible(selector: ElementSelector) -> Self {
        Self::ElementVisible { selector }
    }

    /// Get a human-readable description of this assertion.
    pub fn description(&self) -> String {
        match self {
            Self::ElementExists { selector } => {
                format!("Element {:?} exists", selector)
            }
            Self::ElementHasText {
                selector,
                expected_text,
            } => {
                format!("Element {:?} has text \"{}\"", selector, expected_text)
            }
            Self::ElementVisible { selector } => {
                format!("Element {:?} is visible", selector)
            }
            Self::ElementInBounds {
                selector,
                x,
                y,
                width,
                height,
                ..
            } => {
                format!(
                    "Element {:?} is at ({}, {}) with size {}x{}",
                    selector, x, y, width, height
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assertion_result_passed() {
        let result = AssertionResult::Passed;
        assert!(result.is_passed());
        assert!(!result.is_failed());
        assert!(!result.is_error());
        assert_eq!(format!("{}", result), "PASSED");
    }

    #[test]
    fn test_assertion_result_failed() {
        let result = AssertionResult::failed("Element not found");
        assert!(!result.is_passed());
        assert!(result.is_failed());
        assert!(!result.is_error());
        assert_eq!(format!("{}", result), "FAILED: Element not found");
    }

    #[test]
    fn test_assertion_result_error() {
        let result = AssertionResult::error("Timeout");
        assert!(!result.is_passed());
        assert!(!result.is_failed());
        assert!(result.is_error());
        assert_eq!(format!("{}", result), "ERROR: Timeout");
    }

    #[test]
    fn test_assertion_exists() {
        let assertion = TestAssertion::exists(ElementSelector::Id(42));
        assert!(assertion.description().contains("42"));
        assert!(assertion.description().contains("exists"));
    }

    #[test]
    fn test_assertion_has_text() {
        let assertion = TestAssertion::has_text(ElementSelector::Id(42), "Hello");
        assert!(assertion.description().contains("Hello"));
        assert!(assertion.description().contains("text"));
    }
}
