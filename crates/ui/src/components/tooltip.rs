//! Tooltip component for hover hints
//!
//! Note: Full tooltip positioning requires overlay/portal support.
//! This is a simplified version that shows tooltip content inline.

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// Tooltip position relative to trigger
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum TooltipSide {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// A tooltip component
///
/// Note: This is a simplified tooltip that appears on hover.
/// Full positioning/portal support would require GPUI overlay features.
///
/// # Example
/// ```
/// Tooltip::new("Helpful hint")
///     .child(Button::new("Hover me"))
/// ```
#[derive(IntoElement)]
pub struct Tooltip {
    content: SharedString,
    trigger: Option<AnyElement>,
    side: TooltipSide,
}

impl Tooltip {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            trigger: None,
            side: TooltipSide::Top,
        }
    }

    pub fn child(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    pub fn side(mut self, side: TooltipSide) -> Self {
        self.side = side;
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Tooltip popup styling
        let tooltip_popup = div()
            .absolute()
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .bg(bg::ELEVATED)
            .border_1()
            .border_color(border::DEFAULT)
            .text_xs()
            .text_color(text::PRIMARY)
            .invisible()
            .child(self.content);

        // Position tooltip based on side
        let tooltip_popup = match self.side {
            TooltipSide::Top => tooltip_popup.bottom(px(100.0)).left(px(50.0)),
            TooltipSide::Bottom => tooltip_popup.top(px(100.0)).left(px(50.0)),
            TooltipSide::Left => tooltip_popup.right(px(100.0)).top(px(50.0)),
            TooltipSide::Right => tooltip_popup.left(px(100.0)).top(px(50.0)),
        };

        let mut container = div()
            .relative()
            .group("tooltip");

        if let Some(trigger) = self.trigger {
            container = container.child(trigger);
        }

        // Show tooltip on hover (using group-hover)
        container = container.child(
            tooltip_popup.group_hover("tooltip", |s| s.visible())
        );

        container
    }
}
