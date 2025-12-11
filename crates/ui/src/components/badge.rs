//! Badge component for status indicators

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border, status};

/// Badge variant styles
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Outline,
    Destructive,
}

/// A badge/tag component for displaying status or labels
///
/// # Example
/// ```
/// Badge::new("New")
/// Badge::new("Error").variant(BadgeVariant::Destructive)
/// ```
#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    variant: BadgeVariant,
}

impl Badge {
    /// Create a new badge with the given label
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            variant: BadgeVariant::Default,
        }
    }

    /// Set the badge variant
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg_color, text_color, border_color) = match self.variant {
            BadgeVariant::Default => (bg::ELEVATED, text::PRIMARY, Some(border::DEFAULT)),
            BadgeVariant::Secondary => (bg::HOVER, text::PRIMARY, None),
            BadgeVariant::Outline => (bg::SURFACE, text::PRIMARY, Some(border::DEFAULT)),
            BadgeVariant::Destructive => (status::ERROR_BG, status::ERROR, None),
        };

        let mut el = div()
            .px(px(8.0))
            .py(px(2.0))
            .rounded(px(9999.0))  // pill shape
            .bg(bg_color)
            .text_color(text_color)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .child(self.label);

        if let Some(border) = border_color {
            el = el.border_1().border_color(border);
        }

        el
    }
}
