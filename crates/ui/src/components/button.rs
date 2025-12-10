//! Button component with variants and sizes

use gpui::prelude::*;
use gpui::*;
use theme::ui::button;

/// Button variant determines the visual style
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Primary action button (cyan background)
    #[default]
    Default,
    /// Destructive action (red background)
    Destructive,
    /// Outline style (transparent with border)
    Outline,
    /// Secondary/muted style (gray background)
    Secondary,
    /// Ghost style (transparent, hover reveals)
    Ghost,
    /// Link style (text link appearance)
    Link,
}

/// Button size variants
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    /// Default size: h-9 px-4 py-2
    #[default]
    Default,
    /// Small size: h-8 px-3
    Sm,
    /// Large size: h-10 px-6
    Lg,
    /// Icon button: h-9 w-9 (square)
    Icon,
}

/// A button component with shadcn-style variants
///
/// # Example
/// ```
/// Button::new("Save")
///     .variant(ButtonVariant::Default)
///     .size(ButtonSize::Default)
///     .on_click(|_, _| println!("clicked"))
/// ```
pub struct Button {
    label: SharedString,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Button {
    /// Create a new button with a label
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::Default,
            size: ButtonSize::Default,
            disabled: false,
            on_click: None,
        }
    }

    /// Create an icon-only button (use with ButtonSize::Icon)
    pub fn icon(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::Ghost,
            size: ButtonSize::Icon,
            disabled: false,
            on_click: None,
        }
    }

    /// Set the button variant
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the button size
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Set whether the button is disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the click handler
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg, text_color, border_color, hover_bg) = match self.variant {
            ButtonVariant::Default => (
                button::DEFAULT_BG,
                button::DEFAULT_TEXT,
                None,
                Some(button::DEFAULT_HOVER_BG),
            ),
            ButtonVariant::Destructive => (
                button::DESTRUCTIVE_BG,
                button::DESTRUCTIVE_TEXT,
                None,
                Some(button::DESTRUCTIVE_HOVER_BG),
            ),
            ButtonVariant::Outline => (
                button::OUTLINE_BG,
                button::OUTLINE_TEXT,
                Some(button::OUTLINE_BORDER),
                Some(button::OUTLINE_HOVER_BG),
            ),
            ButtonVariant::Secondary => (
                button::SECONDARY_BG,
                button::SECONDARY_TEXT,
                None,
                Some(button::SECONDARY_HOVER_BG),
            ),
            ButtonVariant::Ghost => (
                button::GHOST_BG,
                button::GHOST_TEXT,
                None,
                Some(button::GHOST_HOVER_BG),
            ),
            ButtonVariant::Link => (
                gpui::transparent_black(),
                button::LINK_TEXT,
                None,
                None,
            ),
        };

        let (height, padding_x, padding_y, font_size) = match self.size {
            ButtonSize::Default => (px(36.0), px(16.0), px(8.0), px(14.0)),
            ButtonSize::Sm => (px(32.0), px(12.0), px(6.0), px(12.0)),
            ButtonSize::Lg => (px(40.0), px(24.0), px(10.0), px(16.0)),
            ButtonSize::Icon => (px(36.0), px(0.0), px(0.0), px(14.0)),
        };

        let is_icon = matches!(self.size, ButtonSize::Icon);

        let mut el = div()
            .id(ElementId::Name(self.label.clone()))
            .h(height)
            .when(is_icon, |d| d.w(height))
            .when(!is_icon, |d| d.px(padding_x).py(padding_y))
            .bg(bg)
            .text_color(text_color)
            .text_size(font_size)
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .font_weight(FontWeight::MEDIUM);

        // Apply border if needed
        if let Some(border) = border_color {
            el = el.border_1().border_color(border);
        }

        // Apply hover state if not disabled and not link
        if !self.disabled {
            if let Some(hover) = hover_bg {
                el = el.hover(|style| style.bg(hover));
            }
            if matches!(self.variant, ButtonVariant::Link) {
                el = el.hover(|style| style.text_color(button::LINK_HOVER_TEXT));
            }
            el = el.cursor_pointer();
        }

        // Apply disabled state
        if self.disabled {
            el = el.opacity(0.5).cursor_not_allowed();
        }

        // Apply click handler
        if let Some(handler) = self.on_click {
            if !self.disabled {
                el = el.on_click(move |_, window, cx| handler(window, cx));
            }
        }

        el.child(self.label)
    }
}
