//! Popover component for click-triggered overlays

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border};

/// Popover side/position
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum PopoverSide {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

/// Popover component
///
/// Note: This is a simplified popover. Full positioning would require
/// overlay/portal support from GPUI.
///
/// # Example
/// ```
/// Popover::new()
///     .trigger(Button::new("Open"))
///     .content(div().child("Popover content"))
///     .open(is_open)
/// ```
#[derive(IntoElement)]
pub struct Popover {
    trigger: Option<AnyElement>,
    content: Option<AnyElement>,
    open: bool,
    side: PopoverSide,
}

impl Popover {
    pub fn new() -> Self {
        Self {
            trigger: None,
            content: None,
            open: false,
            side: PopoverSide::Bottom,
        }
    }

    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn side(mut self, side: PopoverSide) -> Self {
        self.side = side;
        self
    }
}

impl Default for Popover {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Popover {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div().relative();

        // Trigger
        if let Some(trigger) = self.trigger {
            container = container.child(trigger);
        }

        // Content (shown when open)
        if self.open {
            let mut content_wrapper = div()
                .absolute()
                .p(px(16.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(border::DEFAULT)
                .bg(bg::ELEVATED);

            // Position based on side
            content_wrapper = match self.side {
                PopoverSide::Bottom => content_wrapper.top(px(40.0)).left_0(),
                PopoverSide::Top => content_wrapper.bottom(px(40.0)).left_0(),
                PopoverSide::Left => content_wrapper.right(px(100.0)).top_0(),
                PopoverSide::Right => content_wrapper.left(px(100.0)).top_0(),
            };

            if let Some(content) = self.content {
                content_wrapper = content_wrapper.child(content);
            }

            container = container.child(content_wrapper);
        }

        container
    }
}
