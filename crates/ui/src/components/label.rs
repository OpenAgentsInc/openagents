//! Label component for form inputs

use gpui::prelude::*;
use gpui::*;
use theme::ui::label;

/// A text label component, typically used with form inputs
///
/// # Example
/// ```
/// Label::new("Email address")
/// Label::new("Password").disabled(true)
/// ```
#[derive(IntoElement)]
pub struct Label {
    text: SharedString,
    disabled: bool,
}

impl Label {
    /// Create a new label with text
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            disabled: false,
        }
    }

    /// Set whether the label appears disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let text_color = if self.disabled {
            label::DISABLED
        } else {
            label::TEXT
        };

        div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .text_color(text_color)
            .when(self.disabled, |d| d.cursor_not_allowed())
            .child(self.text)
    }
}
