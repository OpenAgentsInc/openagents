//! Test step types for specifying test actions.
//!
//! A test is composed of a sequence of `TestStep`s that describe user interactions
//! and assertions to verify.

use crate::{Bounds, Key, Modifiers, MouseButton, Point};
use std::time::Duration;

/// A single step in a test sequence.
#[derive(Clone, Debug)]
pub enum TestStep {
    /// Click on a target with the specified mouse button.
    Click {
        target: ClickTarget,
        button: MouseButton,
    },
    /// Double-click on a target.
    DoubleClick {
        target: ClickTarget,
        button: MouseButton,
    },
    /// Type text with optional delay between characters.
    Type {
        text: String,
        delay_per_char: Option<Duration>,
    },
    /// Press a key with optional modifiers.
    KeyPress { key: Key, modifiers: Modifiers },
    /// Scroll at a target location.
    Scroll {
        target: ClickTarget,
        dx: f32,
        dy: f32,
    },
    /// Move the mouse to a target.
    MoveTo { target: ClickTarget },
    /// Wait for a fixed duration.
    Wait { duration: Duration },
    /// Wait for an element to appear (with timeout).
    WaitFor {
        selector: ElementSelector,
        timeout: Duration,
    },
    /// Assert that an element exists.
    Expect { selector: ElementSelector },
    /// Assert that an element contains specific text.
    ExpectText {
        selector: ElementSelector,
        text: String,
    },
    /// Assert that an element is visible.
    ExpectVisible { selector: ElementSelector },
}

/// How to identify an element for interaction or assertion.
#[derive(Clone, Debug, PartialEq)]
pub enum ElementSelector {
    /// Select by ComponentId.
    Id(u64),
    /// Select by visible text content.
    Text(String),
    /// Select by screen bounds.
    Bounds(Bounds),
    /// Query string: "#123" for ComponentId, "text:Hello" for text search.
    Query(String),
}

impl ElementSelector {
    /// Parse a query string into an ElementSelector.
    ///
    /// Formats:
    /// - `#123` - ComponentId(123)
    /// - `text:Hello` - Text("Hello")
    /// - Otherwise treated as text search
    pub fn parse(query: &str) -> Self {
        if let Some(id_str) = query.strip_prefix('#') {
            if let Ok(id) = id_str.parse::<u64>() {
                return Self::Id(id);
            }
        }
        if let Some(text) = query.strip_prefix("text:") {
            return Self::Text(text.to_string());
        }
        Self::Query(query.to_string())
    }
}

/// Where to click or interact.
#[derive(Clone, Debug)]
pub enum ClickTarget {
    /// Click the center of an element.
    Element(ElementSelector),
    /// Click at absolute screen coordinates.
    Position(Point),
    /// Click at an offset from an element's origin.
    ElementOffset {
        selector: ElementSelector,
        offset: Point,
    },
}

impl ClickTarget {
    /// Create a click target from a selector string.
    pub fn from_selector(query: &str) -> Self {
        Self::Element(ElementSelector::parse(query))
    }

    /// Create a click target at specific coordinates.
    pub fn at(x: f32, y: f32) -> Self {
        Self::Position(Point::new(x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_selector_parse_id() {
        assert_eq!(ElementSelector::parse("#123"), ElementSelector::Id(123));
        assert_eq!(ElementSelector::parse("#0"), ElementSelector::Id(0));
    }

    #[test]
    fn test_element_selector_parse_text() {
        assert_eq!(
            ElementSelector::parse("text:Hello"),
            ElementSelector::Text("Hello".to_string())
        );
        assert_eq!(
            ElementSelector::parse("text:Click me"),
            ElementSelector::Text("Click me".to_string())
        );
    }

    #[test]
    fn test_element_selector_parse_query() {
        assert_eq!(
            ElementSelector::parse("some-query"),
            ElementSelector::Query("some-query".to_string())
        );
        // Invalid ID format falls back to query
        assert_eq!(
            ElementSelector::parse("#notanumber"),
            ElementSelector::Query("#notanumber".to_string())
        );
    }

    #[test]
    fn test_click_target_from_selector() {
        let target = ClickTarget::from_selector("#42");
        match target {
            ClickTarget::Element(ElementSelector::Id(42)) => {}
            _ => panic!("Expected Element with Id(42)"),
        }
    }

    #[test]
    fn test_click_target_at() {
        let target = ClickTarget::at(100.0, 200.0);
        match target {
            ClickTarget::Position(p) => {
                assert_eq!(p.x, 100.0);
                assert_eq!(p.y, 200.0);
            }
            _ => panic!("Expected Position"),
        }
    }
}
