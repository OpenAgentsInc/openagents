//! Skeleton loading placeholder component

use gpui::prelude::*;
use gpui::*;
use theme::ui::skeleton;

/// A skeleton loading placeholder
///
/// # Example
/// ```
/// Skeleton::new().w(px(100.0)).h(px(20.0))
/// Skeleton::new().rounded_full()  // For circular avatars
/// ```
#[derive(IntoElement)]
pub struct Skeleton {
    width: Option<Pixels>,
    height: Option<Pixels>,
    rounded_full: bool,
}

impl Skeleton {
    /// Create a new skeleton placeholder
    pub fn new() -> Self {
        Self {
            width: None,
            height: None,
            rounded_full: false,
        }
    }

    /// Set the width
    pub fn w(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the height
    pub fn h(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// Make fully rounded (for circular shapes)
    pub fn rounded_full(mut self) -> Self {
        self.rounded_full = true;
        self
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .bg(skeleton::BG)
            .overflow_hidden();

        if let Some(w) = self.width {
            el = el.w(w);
        } else {
            el = el.w_full();
        }

        if let Some(h) = self.height {
            el = el.h(h);
        } else {
            el = el.h(px(20.0));
        }

        if self.rounded_full {
            el = el.rounded_full();
        } else {
            el = el.rounded(px(4.0));
        }

        el
    }
}
