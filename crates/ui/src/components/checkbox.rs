//! Checkbox component

use gpui::*;
use theme::ui::checkbox;

/// A checkbox input component
///
/// # Example
/// ```
/// Checkbox::new().checked(true)
/// Checkbox::new().on_change(|checked, _, _| println!("{}", checked))
/// ```
pub struct Checkbox {
    checked: bool,
    disabled: bool,
    on_change: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl Checkbox {
    /// Create a new unchecked checkbox
    pub fn new() -> Self {
        Self {
            checked: false,
            disabled: false,
            on_change: None,
        }
    }

    /// Set the checked state
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set whether the checkbox is disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the change handler
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg, border) = if self.checked {
            (checkbox::CHECKED_BG, checkbox::CHECKED_BORDER)
        } else {
            (checkbox::UNCHECKED_BG, checkbox::UNCHECKED_BORDER)
        };

        let checked = self.checked;

        let mut el = div()
            .id("checkbox")
            .w(px(16.0))
            .h(px(16.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(border)
            .bg(bg)
            .flex()
            .items_center()
            .justify_center();

        // Show checkmark when checked
        if self.checked {
            el = el.child(
                div()
                    .text_color(checkbox::CHECK_ICON)
                    .text_xs()
                    .child("✓")
            );
        }

        // Handle disabled state
        if self.disabled {
            el = el.opacity(0.5).cursor_not_allowed();
        } else {
            el = el.cursor_pointer();

            // Handle click
            if let Some(handler) = self.on_change {
                el = el.on_click(move |_, window, cx| {
                    handler(!checked, window, cx);
                });
            }
        }

        el
    }
}

impl IntoElement for Checkbox {
    type Element = <Self as RenderOnce>::Element;

    fn into_element(self) -> Self::Element {
        self.render_once()
    }
}
