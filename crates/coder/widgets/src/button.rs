//! Button widget - clickable button component.
//!
//! The Button widget handles click interactions and provides
//! visual feedback for hover and pressed states.

use crate::context::{EventContext, PaintContext};
use crate::widget::{AnyWidget, EventResult, Widget, WidgetId};
use wgpui::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad};

/// Button variant for different styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Primary action button.
    #[default]
    Primary,
    /// Secondary/outline button.
    Secondary,
    /// Ghost/text button.
    Ghost,
    /// Destructive action button.
    Danger,
}

/// Callback for button click.
pub type OnClick = Box<dyn FnMut()>;

/// A clickable button widget.
pub struct Button {
    /// Unique ID for this widget.
    id: Option<WidgetId>,
    /// Button label text.
    label: String,
    /// Button variant.
    variant: ButtonVariant,
    /// Whether the button is disabled.
    disabled: bool,
    /// Whether the button is currently hovered.
    hovered: bool,
    /// Whether the button is currently pressed.
    pressed: bool,
    /// Font size.
    font_size: f32,
    /// Padding.
    padding: (f32, f32), // (horizontal, vertical)
    /// Corner radius.
    corner_radius: f32,
    /// Custom icon widget (optional).
    icon: Option<AnyWidget>,
    /// Click callback.
    on_click: Option<OnClick>,
    /// Custom background color (overrides variant).
    custom_bg: Option<Hsla>,
    /// Custom text color (overrides variant).
    custom_text: Option<Hsla>,
}

impl Button {
    /// Create a new button with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: None,
            label: label.into(),
            variant: ButtonVariant::Primary,
            disabled: false,
            hovered: false,
            pressed: false,
            font_size: 14.0,
            padding: (16.0, 8.0),
            corner_radius: 4.0,
            icon: None,
            on_click: None,
            custom_bg: None,
            custom_text: None,
        }
    }

    /// Set custom background color.
    pub fn background(mut self, color: Hsla) -> Self {
        self.custom_bg = Some(color);
        self
    }

    /// Set custom text color.
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.custom_text = Some(color);
        self
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the button variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the button as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the padding.
    pub fn padding(mut self, horizontal: f32, vertical: f32) -> Self {
        self.padding = (horizontal, vertical);
        self
    }

    /// Set the corner radius.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Set an icon widget.
    pub fn icon<W: Widget + 'static>(mut self, widget: W) -> Self {
        self.icon = Some(AnyWidget::new(widget));
        self
    }

    /// Set the on_click callback.
    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    /// Get the button label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Set the button label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Get colors for the current variant and state.
    fn colors(&self) -> (Hsla, Hsla, Hsla) {
        // (background, text, border)
        // Use custom colors if set
        let bg = self.custom_bg.unwrap_or_else(|| match self.variant {
            ButtonVariant::Primary => wgpui::theme::accent::PRIMARY,
            ButtonVariant::Secondary => wgpui::theme::bg::SURFACE,
            ButtonVariant::Ghost => Hsla::transparent(),
            ButtonVariant::Danger => wgpui::theme::status::ERROR,
        });

        let text = self.custom_text.unwrap_or_else(|| {
            let on_accent = Hsla::new(0.0, 0.0, 0.0, 1.0);
            match self.variant {
                ButtonVariant::Primary | ButtonVariant::Danger => on_accent,
                ButtonVariant::Secondary | ButtonVariant::Ghost => wgpui::theme::text::PRIMARY,
            }
        });

        let border = bg; // Border matches background

        if self.disabled {
            return (
                bg.with_alpha(bg.a * 0.5),
                text.with_alpha(text.a * 0.5),
                border.with_alpha(border.a * 0.5),
            );
        }

        if self.pressed {
            return (bg.darken(0.15), text, border.darken(0.15));
        }

        if self.hovered {
            return (bg.lighten(0.1), text, border.lighten(0.1));
        }

        (bg, text, border)
    }
}

impl Default for Button {
    fn default() -> Self {
        Self::new("Button")
    }
}

impl Widget for Button {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let (bg_color, text_color, border_color) = self.colors();

        // Draw background
        let mut quad = Quad::new(bounds)
            .with_background(bg_color)
            .with_uniform_radius(self.corner_radius);

        // Add border for secondary variant
        if matches!(self.variant, ButtonVariant::Secondary) {
            quad = quad.with_border(border_color, 1.0);
        }

        cx.scene.draw_quad(quad);

        // Calculate content position
        let content_width = self.label.len() as f32 * self.font_size * 0.6;
        let text_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
        let text_y = bounds.origin.y + (bounds.size.height - self.font_size) / 2.0;

        // Draw icon if present
        if let Some(icon) = &mut self.icon {
            let icon_size = self.font_size;
            let icon_bounds = Bounds::new(
                bounds.origin.x + self.padding.0,
                bounds.origin.y + (bounds.size.height - icon_size) / 2.0,
                icon_size,
                icon_size,
            );
            icon.paint(icon_bounds, cx);
        }

        // Draw label
        if !self.label.is_empty() {
            let text_run = cx.text.layout(
                &self.label,
                Point::new(text_x, text_y),
                self.font_size,
                text_color,
            );
            cx.scene.draw_text(text_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if self.disabled {
            return EventResult::Ignored;
        }

        match event {
            InputEvent::MouseMove { position, .. } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(*position);

                if was_hovered != self.hovered {
                    // Would trigger repaint in a full implementation
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseDown {
                position, button, ..
            } => {
                if *button == MouseButton::Left && bounds.contains(*position) {
                    self.pressed = true;
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseUp {
                position, button, ..
            } => {
                if *button == MouseButton::Left && self.pressed {
                    self.pressed = false;

                    // Fire click if released over button
                    if bounds.contains(*position) {
                        if let Some(on_click) = &mut self.on_click {
                            on_click();
                        }
                    }

                    return EventResult::Handled;
                }
            }

            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let width = self.label.len() as f32 * self.font_size * 0.6 + self.padding.0 * 2.0;
        let height = self.font_size * 1.4 + self.padding.1 * 2.0;
        (Some(width), Some(height))
    }
}

// Helper trait for Hsla color manipulation
#[allow(dead_code)]
trait HslaExt {
    fn transparent() -> Self;
    fn darken(&self, amount: f32) -> Self;
    fn lighten(&self, amount: f32) -> Self;
    fn with_alpha(&self, alpha: f32) -> Self;
}

impl HslaExt for Hsla {
    fn transparent() -> Self {
        Hsla::new(0.0, 0.0, 0.0, 0.0)
    }

    fn darken(&self, amount: f32) -> Self {
        Hsla::new(self.h, self.s, (self.l - amount).max(0.0), self.a)
    }

    fn lighten(&self, amount: f32) -> Self {
        Hsla::new(self.h, self.s, (self.l + amount).min(1.0), self.a)
    }

    fn with_alpha(&self, alpha: f32) -> Self {
        Hsla::new(self.h, self.s, self.l, alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_creation() {
        let button = Button::new("Click me")
            .id(1)
            .variant(ButtonVariant::Primary)
            .disabled(false);

        assert_eq!(button.id, Some(1));
        assert_eq!(button.label, "Click me");
        assert_eq!(button.variant, ButtonVariant::Primary);
        assert!(!button.disabled);
    }

    #[test]
    fn test_button_variants() {
        let primary = Button::new("Primary").variant(ButtonVariant::Primary);
        let secondary = Button::new("Secondary").variant(ButtonVariant::Secondary);
        let ghost = Button::new("Ghost").variant(ButtonVariant::Ghost);
        let danger = Button::new("Danger").variant(ButtonVariant::Danger);

        assert_eq!(primary.variant, ButtonVariant::Primary);
        assert_eq!(secondary.variant, ButtonVariant::Secondary);
        assert_eq!(ghost.variant, ButtonVariant::Ghost);
        assert_eq!(danger.variant, ButtonVariant::Danger);
    }

    #[test]
    fn test_button_states() {
        let mut button = Button::new("Test");

        // Initial state
        assert!(!button.hovered);
        assert!(!button.pressed);

        // Simulate hover
        button.hovered = true;
        assert!(button.hovered);

        // Simulate press
        button.pressed = true;
        assert!(button.pressed);
    }

    #[test]
    fn test_button_size_hint() {
        let button = Button::new("Test Button")
            .font_size(14.0)
            .padding(16.0, 8.0);

        let (width, height) = button.size_hint();

        // Width should include text width + padding
        assert!(width.is_some());
        assert!(width.unwrap() > 32.0); // At least 2x horizontal padding

        // Height should include font height + padding
        assert!(height.is_some());
        assert!(height.unwrap() > 16.0); // At least 2x vertical padding
    }
}
