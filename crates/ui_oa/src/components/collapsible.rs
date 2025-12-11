//! Collapsible component for expandable content

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, text};

/// A collapsible section with trigger and content
///
/// Note: This is a stateless component - parent must manage open/closed state
///
/// # Example
/// ```
/// Collapsible::new()
///     .open(is_open)
///     .trigger("Click to expand")
///     .content(div().child("Hidden content"))
///     .on_toggle(|open, _, _| set_open(open))
/// ```
#[derive(IntoElement)]
pub struct Collapsible {
    trigger_label: SharedString,
    content: Option<AnyElement>,
    open: bool,
    on_toggle: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl Collapsible {
    /// Create a new collapsible
    pub fn new() -> Self {
        Self {
            trigger_label: "Toggle".into(),
            content: None,
            open: false,
            on_toggle: None,
        }
    }

    /// Set the trigger label text
    pub fn trigger(mut self, label: impl Into<SharedString>) -> Self {
        self.trigger_label = label.into();
        self
    }

    /// Set the collapsible content
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Set the open/closed state
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Set the toggle handler
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }
}

impl Default for Collapsible {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Collapsible {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let current_open = self.open;
        let chevron = if self.open { "▼" } else { "▶" };

        let mut container = div().flex().flex_col().gap(px(4.0));

        // Trigger button
        let mut trigger = div()
            .id("collapsible-trigger")
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .text_color(text::PRIMARY)
            .hover(|s| s.bg(bg::HOVER))
            .child(div().text_xs().child(chevron))
            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(self.trigger_label));

        if let Some(handler) = self.on_toggle {
            trigger = trigger.on_click(move |_, window, cx| {
                handler(!current_open, window, cx);
            });
        }

        container = container.child(trigger);

        // Content (only shown when open)
        if self.open {
            if let Some(content) = self.content {
                container = container.child(
                    div()
                        .pl(px(24.0))
                        .child(content)
                );
            }
        }

        container
    }
}
