//! Spinner/loading indicator component

use gpui::prelude::*;
use gpui::*;
use theme::accent;

/// A loading spinner indicator
///
/// # Example
/// ```
/// Spinner::new()
/// Spinner::new().size(SpinnerSize::Lg)
/// ```
#[derive(IntoElement)]
pub struct Spinner {
    size: Pixels,
}

impl Spinner {
    /// Create a new spinner with default size
    pub fn new() -> Self {
        Self { size: px(16.0) }
    }

    /// Small spinner (12px)
    pub fn sm() -> Self {
        Self { size: px(12.0) }
    }

    /// Medium spinner (16px, default)
    pub fn md() -> Self {
        Self { size: px(16.0) }
    }

    /// Large spinner (24px)
    pub fn lg() -> Self {
        Self { size: px(24.0) }
    }

    /// Set a custom size
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Simple spinner using a partial border
        // In a real implementation, this would animate
        div()
            .w(self.size)
            .h(self.size)
            .rounded_full()
            .border_2()
            .border_color(accent::PRIMARY)
    }
}
