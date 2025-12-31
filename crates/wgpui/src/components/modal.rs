use crate::components::context::{EventContext, PaintContext};
use crate::components::{AnyComponent, Component, ComponentId, EventResult, Text};
use crate::input::{Key, NamedKey};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

pub struct Modal {
    id: Option<ComponentId>,
    title: Option<String>,
    content: Option<AnyComponent>,
    open: bool,
    width: f32,
    height: Option<f32>,
    backdrop_color: Hsla,
    background: Hsla,
    border_color: Hsla,
    close_on_backdrop: bool,
    close_on_escape: bool,
    show_close_button: bool,
    on_close: Option<Box<dyn FnMut()>>,
}

impl Modal {
    pub fn new() -> Self {
        Self {
            id: None,
            title: None,
            content: None,
            open: false,
            width: 400.0,
            height: None,
            backdrop_color: Hsla::new(0.0, 0.0, 0.0, 0.5),
            background: theme::bg::SURFACE,
            border_color: theme::border::DEFAULT,
            close_on_backdrop: true,
            close_on_escape: true,
            show_close_button: true,
            on_close: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn content<C: Component + 'static>(mut self, component: C) -> Self {
        self.content = Some(AnyComponent::new(component));
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn backdrop_color(mut self, color: Hsla) -> Self {
        self.backdrop_color = color;
        self
    }

    pub fn background(mut self, color: Hsla) -> Self {
        self.background = color;
        self
    }

    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    pub fn show_close_button(mut self, show: bool) -> Self {
        self.show_close_button = show;
        self
    }

    pub fn on_close<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_close = Some(Box::new(f));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    pub fn show(&mut self) {
        self.open = true;
    }

    pub fn hide(&mut self) {
        self.open = false;
    }

    fn close(&mut self) {
        self.open = false;
        if let Some(on_close) = &mut self.on_close {
            on_close();
        }
    }

    fn modal_bounds(&self, viewport: Bounds) -> Bounds {
        let modal_height = self.height.unwrap_or(300.0);
        let x = (viewport.size.width - self.width) / 2.0;
        let y = (viewport.size.height - modal_height) / 2.0;
        Bounds::new(
            viewport.origin.x + x,
            viewport.origin.y + y,
            self.width,
            modal_height,
        )
    }
}

impl Default for Modal {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Modal {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.open {
            return;
        }

        cx.scene
            .draw_quad(Quad::new(bounds).with_background(self.backdrop_color));

        let modal_bounds = self.modal_bounds(bounds);

        cx.scene.draw_quad(
            Quad::new(modal_bounds)
                .with_background(self.background)
                .with_border(self.border_color, 1.0),
        );

        let padding = theme::spacing::MD;
        let header_height = if self.title.is_some() || self.show_close_button {
            36.0
        } else {
            0.0
        };

        if let Some(title) = &self.title {
            let title_y = modal_bounds.origin.y + padding;
            let mut title_text = Text::new(title.clone())
                .font_size(theme::font_size::LG)
                .bold();

            title_text.paint(
                Bounds::new(
                    modal_bounds.origin.x + padding,
                    title_y,
                    modal_bounds.size.width - padding * 2.0 - 32.0,
                    24.0,
                ),
                cx,
            );
        }

        if self.show_close_button {
            let close_size = 24.0;
            let close_x = modal_bounds.origin.x + modal_bounds.size.width - padding - close_size;
            let close_y = modal_bounds.origin.y + padding;

            let mut close_text = Text::new("X")
                .font_size(theme::font_size::SM)
                .color(theme::text::MUTED);

            close_text.paint(Bounds::new(close_x, close_y, close_size, close_size), cx);
        }

        if let Some(content) = &mut self.content {
            let content_bounds = Bounds::new(
                modal_bounds.origin.x + padding,
                modal_bounds.origin.y + padding + header_height,
                modal_bounds.size.width - padding * 2.0,
                modal_bounds.size.height - padding * 2.0 - header_height,
            );
            content.paint(content_bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        if !self.open {
            return EventResult::Ignored;
        }

        match event {
            InputEvent::KeyDown { key, .. } => {
                if let Key::Named(NamedKey::Escape) = key
                    && self.close_on_escape
                {
                    self.close();
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let modal_bounds = self.modal_bounds(bounds);
                    let click_point = Point::new(*x, *y);

                    if self.show_close_button {
                        let padding = theme::spacing::MD;
                        let close_size = 24.0;
                        let close_bounds = Bounds::new(
                            modal_bounds.origin.x + modal_bounds.size.width - padding - close_size,
                            modal_bounds.origin.y + padding,
                            close_size,
                            close_size,
                        );

                        if close_bounds.contains(click_point) {
                            self.close();
                            return EventResult::Handled;
                        }
                    }

                    if !modal_bounds.contains(click_point) && self.close_on_backdrop {
                        self.close();
                        return EventResult::Handled;
                    }

                    return EventResult::Handled;
                }
            }

            _ => {}
        }

        let modal_bounds = self.modal_bounds(bounds);
        if let Some(content) = &mut self.content {
            return content.event(event, modal_bounds, cx);
        }

        EventResult::Handled
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Div;

    #[test]
    fn test_modal_new() {
        let modal = Modal::new();
        assert!(!modal.open);
        assert!(modal.title.is_none());
        assert!(modal.close_on_backdrop);
        assert!(modal.close_on_escape);
    }

    #[test]
    fn test_modal_builder() {
        let modal = Modal::new()
            .with_id(42)
            .title("Test Modal")
            .width(500.0)
            .height(400.0)
            .open(true);

        assert_eq!(modal.id, Some(42));
        assert_eq!(modal.title, Some("Test Modal".to_string()));
        assert_eq!(modal.width, 500.0);
        assert_eq!(modal.height, Some(400.0));
        assert!(modal.open);
    }

    #[test]
    fn test_modal_show_hide() {
        let mut modal = Modal::new();
        assert!(!modal.is_open());

        modal.show();
        assert!(modal.is_open());

        modal.hide();
        assert!(!modal.is_open());
    }

    #[test]
    fn test_modal_set_open() {
        let mut modal = Modal::new();
        modal.set_open(true);
        assert!(modal.is_open());

        modal.set_open(false);
        assert!(!modal.is_open());
    }

    #[test]
    fn test_modal_with_content() {
        let content = Div::new().background(theme::bg::MUTED);
        let modal = Modal::new().content(content);

        assert!(modal.content.is_some());
    }

    #[test]
    fn test_modal_bounds_centered() {
        let modal = Modal::new().width(200.0).height(100.0);
        let viewport = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let modal_bounds = modal.modal_bounds(viewport);

        assert_eq!(modal_bounds.origin.x, 300.0);
        assert_eq!(modal_bounds.origin.y, 250.0);
        assert_eq!(modal_bounds.size.width, 200.0);
        assert_eq!(modal_bounds.size.height, 100.0);
    }
}
