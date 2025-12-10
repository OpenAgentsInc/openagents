//! Keyboard key display component

use gpui::prelude::*;
use gpui::*;
use theme::ui::kbd;

/// Displays a keyboard key or shortcut
///
/// # Example
/// ```
/// Kbd::new("⌘K")
/// Kbd::new("Enter")
/// Kbd::new("Ctrl+C")
/// ```
pub struct Kbd {
    text: SharedString,
}

impl Kbd {
    /// Create a new keyboard key display
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .px(px(6.0))
            .py(px(2.0))
            .bg(kbd::BG)
            .border_1()
            .border_color(kbd::BORDER)
            .border_b_2()
            .rounded(px(4.0))
            .text_color(kbd::TEXT)
            .text_xs()
            .font_family("Berkeley Mono")
            .child(self.text)
    }
}
