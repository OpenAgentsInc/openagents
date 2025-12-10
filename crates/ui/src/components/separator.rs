//! Separator component for dividing content

use gpui::prelude::*;
use gpui::*;
use theme::ui::separator;

/// Orientation for the separator
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SeparatorOrientation {
    #[default]
    Horizontal,
    Vertical,
}

/// A separator line for dividing content
///
/// # Example
/// ```
/// Separator::horizontal()
/// Separator::vertical()
/// ```
pub struct Separator {
    orientation: SeparatorOrientation,
}

impl Separator {
    /// Create a horizontal separator (default)
    pub fn horizontal() -> Self {
        Self {
            orientation: SeparatorOrientation::Horizontal,
        }
    }

    /// Create a vertical separator
    pub fn vertical() -> Self {
        Self {
            orientation: SeparatorOrientation::Vertical,
        }
    }
}

impl RenderOnce for Separator {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        match self.orientation {
            SeparatorOrientation::Horizontal => div()
                .w_full()
                .h(px(1.0))
                .bg(separator::DEFAULT),
            SeparatorOrientation::Vertical => div()
                .h_full()
                .w(px(1.0))
                .bg(separator::DEFAULT),
        }
    }
}
