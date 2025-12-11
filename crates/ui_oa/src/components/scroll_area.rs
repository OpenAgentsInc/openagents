//! ScrollArea component for scrollable content
//!
//! Note: This is a simplified scroll area. Full scrollbar customization
//! would require additional GPUI scrollbar styling support.

use gpui::prelude::*;
use gpui::*;

/// Scroll direction
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

/// A scrollable area container
///
/// # Example
/// ```
/// ScrollArea::new()
///     .max_height(px(300.0))
///     .child(long_content)
/// ```
#[derive(IntoElement)]
pub struct ScrollArea {
    direction: ScrollDirection,
    max_height: Option<Pixels>,
    max_width: Option<Pixels>,
    content: Option<AnyElement>,
}

impl ScrollArea {
    pub fn new() -> Self {
        Self {
            direction: ScrollDirection::Vertical,
            max_height: None,
            max_width: None,
            content: None,
        }
    }

    pub fn direction(mut self, direction: ScrollDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = Some(height);
        self
    }

    pub fn max_width(mut self, width: Pixels) -> Self {
        self.max_width = Some(width);
        self
    }

    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div()
            .relative();

        // Apply max dimensions
        if let Some(h) = self.max_height {
            container = container.max_h(h);
        }
        if let Some(w) = self.max_width {
            container = container.max_w(w);
        }

        // Simple overflow handling - use overflow_hidden as base
        // Note: Full scroll behavior would need GPUI's scroll view
        container = container.overflow_hidden();

        // Content wrapper
        let mut content_wrapper = div();
        if let Some(content) = self.content {
            content_wrapper = content_wrapper.child(content);
        }

        container.child(content_wrapper)
    }
}
