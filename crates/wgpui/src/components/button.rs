//! Button component - clickable button with visual feedback.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{AnyComponent, Component, ComponentId, EventResult};
use crate::layout::{LayoutEngine, LayoutStyle, length, px};
use crate::styled::{StyleRefinement, Styled};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, Size, theme};
use taffy::{AlignItems, JustifyContent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
}

pub type OnClick = Box<dyn FnMut()>;

pub struct Button {
    id: Option<ComponentId>,
    label: String,
    variant: ButtonVariant,
    disabled: bool,
    hovered: bool,
    pressed: bool,
    pub(crate) style: StyleRefinement,
    font_size: f32,
    padding: (f32, f32),
    corner_radius: f32,
    icon: Option<AnyComponent>,
    on_click: Option<OnClick>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: None,
            label: label.into(),
            variant: ButtonVariant::Primary,
            disabled: false,
            hovered: false,
            pressed: false,
            style: StyleRefinement::default(),
            font_size: theme::font_size::SM,
            padding: (theme::spacing::LG, theme::spacing::SM),
            corner_radius: 8.0,
            icon: None,
            on_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn padding(mut self, horizontal: f32, vertical: f32) -> Self {
        self.padding = (horizontal, vertical);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn background(mut self, color: Hsla) -> Self {
        self.style.background = Some(color);
        self
    }

    pub fn text_color(mut self, color: Hsla) -> Self {
        self.style.text_color = Some(color);
        self
    }

    pub fn icon<C: Component + 'static>(mut self, component: C) -> Self {
        self.icon = Some(AnyComponent::new(component));
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    fn colors(&self) -> (Hsla, Hsla, Hsla) {
        let mut bg = self.style.background.unwrap_or_else(|| match self.variant {
            ButtonVariant::Primary => theme::accent::PRIMARY,
            ButtonVariant::Secondary => theme::bg::MUTED,
            ButtonVariant::Ghost => Hsla::transparent(),
            ButtonVariant::Danger => theme::status::ERROR,
        });

        let text = self.style.text_color.unwrap_or_else(|| match self.variant {
            ButtonVariant::Primary => theme::bg::APP,
            ButtonVariant::Danger => Hsla::new(0.0, 0.0, 1.0, 1.0),
            ButtonVariant::Secondary | ButtonVariant::Ghost => theme::text::PRIMARY,
        });

        let mut border = match self.variant {
            ButtonVariant::Secondary => theme::border::DEFAULT,
            _ => bg,
        };

        if self.disabled {
            return (
                bg.with_alpha(bg.a * 0.5),
                text.with_alpha(text.a * 0.5),
                border.with_alpha(border.a * 0.5),
            );
        }

        if self.pressed {
            match self.variant {
                ButtonVariant::Ghost => {
                    bg = theme::bg::MUTED;
                }
                _ => {
                    bg = bg.darken(0.15);
                    if matches!(self.variant, ButtonVariant::Secondary) {
                        border = border.darken(0.15);
                    } else {
                        border = bg;
                    }
                }
            }
            return (bg, text, border);
        }

        if self.hovered {
            match self.variant {
                ButtonVariant::Ghost => {
                    bg = theme::bg::SURFACE;
                }
                _ => {
                    bg = bg.darken(0.08);
                    if matches!(self.variant, ButtonVariant::Secondary) {
                        border = border.darken(0.08);
                    } else {
                        border = bg;
                    }
                }
            }
            return (bg, text, border);
        }

        (bg, text, border)
    }
}

impl Default for Button {
    fn default() -> Self {
        Self::new("Button")
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Component for Button {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let (bg_color, text_color, border_color) = self.colors();
        let font_size = self.style.font_size.unwrap_or(self.font_size);
        let font_style = FontStyle::default();

        let mut quad = Quad::new(bounds).with_background(bg_color);

        if matches!(self.variant, ButtonVariant::Secondary) {
            quad = quad.with_border(border_color, 1.0);
        }

        if self.corner_radius > 0.0 {
            quad = quad.with_corner_radius(self.corner_radius);
        }

        cx.scene.draw_quad(quad);

        let mut label_run = if self.label.is_empty() {
            None
        } else {
            Some(cx.text.layout_styled_mono(
                &self.label,
                Point::ZERO,
                font_size,
                text_color,
                font_style,
            ))
        };
        let label_bounds = label_run
            .as_ref()
            .map(|run| run.bounds())
            .unwrap_or(Bounds::ZERO);
        let label_size = if label_run.is_some() {
            Size::new(label_bounds.size.width, label_bounds.size.height)
        } else {
            Size::ZERO
        };

        let icon_size = if self.icon.is_some() { font_size } else { 0.0 };
        let mut layout = LayoutEngine::new();
        let mut children = Vec::new();

        let icon_id = if icon_size > 0.0 {
            Some(
                layout.request_leaf(
                    &LayoutStyle::new()
                        .width(px(icon_size))
                        .height(px(icon_size)),
                ),
            )
        } else {
            None
        };
        if let Some(id) = icon_id {
            children.push(id);
        }

        let label_id = if label_size.width > 0.0 && label_size.height > 0.0 {
            Some(
                layout.request_leaf(
                    &LayoutStyle::new()
                        .width(px(label_size.width))
                        .height(px(label_size.height)),
                ),
            )
        } else {
            None
        };
        if let Some(id) = label_id {
            children.push(id);
        }

        if !children.is_empty() {
            let content_bounds = Bounds::new(
                bounds.origin.x + self.padding.0,
                bounds.origin.y + self.padding.1,
                (bounds.size.width - self.padding.0 * 2.0).max(0.0),
                (bounds.size.height - self.padding.1 * 2.0).max(0.0),
            );
            let root_style = LayoutStyle::new()
                .width(px(content_bounds.size.width))
                .height(px(content_bounds.size.height))
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .gap(length(theme::spacing::SM));

            let root = layout.request_layout(&root_style, &children);
            layout.compute_layout(root, content_bounds.size);

            if let (Some(icon_id), Some(icon)) = (icon_id, &mut self.icon) {
                let icon_bounds = layout.layout(icon_id);
                icon.paint(
                    Bounds::new(
                        content_bounds.origin.x + icon_bounds.origin.x,
                        content_bounds.origin.y + icon_bounds.origin.y,
                        icon_bounds.size.width,
                        icon_bounds.size.height,
                    ),
                    cx,
                );
            }

            if let (Some(label_id), Some(mut label_run)) = (label_id, label_run.take()) {
                let label_layout = layout.layout(label_id);
                label_run.origin = Point::new(
                    content_bounds.origin.x + label_layout.origin.x - label_bounds.origin.x,
                    content_bounds.origin.y + label_layout.origin.y - label_bounds.origin.y,
                );
                cx.scene.draw_text(label_run);
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if self.disabled {
            return EventResult::Ignored;
        }

        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));

                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    self.pressed = true;
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseUp { button, x, y } => {
                if *button == MouseButton::Left && self.pressed {
                    self.pressed = false;

                    if bounds.contains(Point::new(*x, *y))
                        && let Some(on_click) = &mut self.on_click
                    {
                        on_click();
                    }

                    return EventResult::Handled;
                }
            }

            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let font_size = self.style.font_size.unwrap_or(self.font_size);
        let width = self.label.chars().count() as f32 * font_size * 0.6 + self.padding.0 * 2.0;
        let height = font_size * 1.4 + self.padding.1 * 2.0;
        (Some(width), Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_new() {
        let button = Button::new("Click me");
        assert_eq!(button.label(), "Click me");
        assert_eq!(button.variant, ButtonVariant::Primary);
        assert!(!button.disabled);
    }

    #[test]
    fn test_button_builder() {
        let button = Button::new("Test")
            .with_id(42)
            .variant(ButtonVariant::Secondary)
            .disabled(true);

        assert_eq!(Component::id(&button), Some(42));
        assert_eq!(button.variant, ButtonVariant::Secondary);
        assert!(button.disabled);
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

        assert!(!button.is_hovered());
        assert!(!button.is_pressed());

        button.hovered = true;
        assert!(button.is_hovered());

        button.pressed = true;
        assert!(button.is_pressed());
    }

    #[test]
    fn test_button_set_label() {
        let mut button = Button::new("Original");
        assert_eq!(button.label(), "Original");

        button.set_label("Updated");
        assert_eq!(button.label(), "Updated");
    }

    #[test]
    fn test_button_size_hint() {
        let button = Button::new("Test Button")
            .font_size(14.0)
            .padding(16.0, 8.0);

        let (width, height) = button.size_hint();

        assert!(width.is_some());
        assert!(width.unwrap() > 32.0);

        assert!(height.is_some());
        assert!(height.unwrap() > 16.0);
    }

    #[test]
    fn test_button_default() {
        let button = Button::default();
        assert_eq!(button.label(), "Button");
    }

    #[test]
    fn test_button_custom_colors() {
        let button = Button::new("Custom")
            .background(theme::bg::MUTED)
            .text_color(theme::text::SECONDARY);

        assert!(button.style.background.is_some());
        assert!(button.style.text_color.is_some());
    }
}
