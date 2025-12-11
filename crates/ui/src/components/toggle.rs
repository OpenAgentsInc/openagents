//! Toggle button component

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// Toggle size variants
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ToggleSize {
    Sm,
    #[default]
    Default,
    Lg,
}

/// Toggle variant styles
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ToggleVariant {
    #[default]
    Default,
    Outline,
}

/// A toggle button that can be pressed/unpressed
///
/// # Example
/// ```
/// Toggle::new("B").pressed(true)  // Bold toggle
/// Toggle::new("I").on_change(|pressed, _, _| println!("{}", pressed))
/// ```
#[derive(IntoElement)]
pub struct Toggle {
    label: SharedString,
    pressed: bool,
    disabled: bool,
    variant: ToggleVariant,
    size: ToggleSize,
    on_change: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl Toggle {
    /// Create a new toggle with a label
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            pressed: false,
            disabled: false,
            variant: ToggleVariant::Default,
            size: ToggleSize::Default,
            on_change: None,
        }
    }

    /// Set the pressed state
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the variant
    pub fn variant(mut self, variant: ToggleVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the size
    pub fn size(mut self, size: ToggleSize) -> Self {
        self.size = size;
        self
    }

    /// Set the change handler
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (px_size, text_size) = match self.size {
            ToggleSize::Sm => (28.0, 12.0),
            ToggleSize::Default => (36.0, 14.0),
            ToggleSize::Lg => (44.0, 16.0),
        };

        let bg_color = if self.pressed {
            bg::ELEVATED
        } else {
            bg::SURFACE
        };

        let current_pressed = self.pressed;

        let mut el = div()
            .id("toggle")
            .w(px(px_size))
            .h(px(px_size))
            .rounded(px(6.0))
            .bg(bg_color)
            .text_color(text::PRIMARY)
            .text_size(px(text_size))
            .font_weight(FontWeight::MEDIUM)
            .flex()
            .items_center()
            .justify_center()
            .child(self.label);

        // Add border for outline variant
        if matches!(self.variant, ToggleVariant::Outline) {
            el = el.border_1().border_color(border::DEFAULT);
        }

        // Handle disabled/interactive states
        if self.disabled {
            el = el.opacity(0.5).cursor_not_allowed();
        } else {
            el = el.cursor_pointer().hover(|s| s.bg(bg::HOVER));

            if let Some(handler) = self.on_change {
                el = el.on_click(move |_, window, cx| {
                    handler(!current_pressed, window, cx);
                });
            }
        }

        el
    }
}
