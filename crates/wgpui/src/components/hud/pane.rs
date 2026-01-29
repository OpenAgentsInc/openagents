use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, EventResult};
use crate::text::FontStyle;
use crate::{Bounds, Button, ButtonVariant, InputEvent, Point, Quad, theme};

pub struct PaneFrame {
    title: String,
    active: bool,
    dismissable: bool,
    title_height: f32,
    padding: f32,
    corner_radius: f32,
    close_button: Button,
    close_bounds: Bounds,
    content_bounds: Bounds,
    title_bounds: Bounds,
    pending_close: std::rc::Rc<std::cell::RefCell<bool>>,
}

impl PaneFrame {
    pub fn new() -> Self {
        let pending_close = std::rc::Rc::new(std::cell::RefCell::new(false));
        let pending_close_click = pending_close.clone();
        let close_button = Button::new("x")
            .variant(ButtonVariant::Ghost)
            .font_size(theme::font_size::SM)
            .padding(6.0, 2.0)
            .corner_radius(4.0)
            .on_click(move || {
                *pending_close_click.borrow_mut() = true;
            });

        Self {
            title: String::new(),
            active: false,
            dismissable: true,
            title_height: 28.0,
            padding: 8.0,
            corner_radius: 0.0,
            close_button,
            close_bounds: Bounds::ZERO,
            content_bounds: Bounds::ZERO,
            title_bounds: Bounds::ZERO,
            pending_close,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn dismissable(mut self, dismissable: bool) -> Self {
        self.dismissable = dismissable;
        self
    }

    pub fn set_dismissable(&mut self, dismissable: bool) {
        self.dismissable = dismissable;
    }

    pub fn title_height(mut self, height: f32) -> Self {
        self.title_height = height;
        self
    }

    pub fn set_title_height(&mut self, height: f32) {
        self.title_height = height;
    }

    pub fn content_bounds(&self) -> Bounds {
        self.content_bounds
    }

    pub fn title_bounds(&self) -> Bounds {
        self.title_bounds
    }

    pub fn close_bounds(&self) -> Bounds {
        self.close_bounds
    }

    pub fn is_close_hovered(&self) -> bool {
        self.close_button.is_hovered()
    }

    pub fn take_close_clicked(&mut self) -> bool {
        let mut pending = self.pending_close.borrow_mut();
        let value = *pending;
        *pending = false;
        value
    }
}

impl Default for PaneFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for PaneFrame {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let border_color = if self.active {
            theme::accent::PRIMARY
        } else {
            theme::border::DEFAULT
        };

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::APP.with_alpha(0.9))
                .with_border(border_color, 1.0)
                .with_corner_radius(self.corner_radius),
        );

        let title_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            self.title_height,
        );
        self.title_bounds = title_bounds;

        cx.scene.draw_quad(
            Quad::new(title_bounds)
                .with_background(theme::bg::APP.with_alpha(0.9))
                .with_border(border_color, 1.0),
        );

        let title_text_x = title_bounds.origin.x + self.padding;
        let title_text_y = title_bounds.origin.y + title_bounds.size.height * 0.5
            - theme::font_size::XS * 0.55;
        let title_run = cx.text.layout_styled_mono(
            &self.title,
            Point::new(title_text_x, title_text_y),
            theme::font_size::XS,
            theme::text::PRIMARY,
            FontStyle::default(),
        );
        cx.scene.draw_text(title_run);

        if self.dismissable {
            let button_size = (self.title_height - theme::spacing::SM).max(18.0);
            let close_bounds = Bounds::new(
                title_bounds.origin.x + title_bounds.size.width - button_size - theme::spacing::SM,
                title_bounds.origin.y + (title_bounds.size.height - button_size) * 0.5,
                button_size,
                button_size,
            );
            self.close_bounds = close_bounds;
            self.close_button.paint(close_bounds, cx);
        } else {
            self.close_bounds = Bounds::ZERO;
        }

        self.content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + self.title_height,
            bounds.size.width,
            (bounds.size.height - self.title_height).max(0.0),
        );
    }

    fn event(
        &mut self,
        event: &InputEvent,
        _bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        if self.dismissable {
            let handled = self
                .close_button
                .event(event, self.close_bounds, cx)
                .is_handled();
            if handled {
                return EventResult::Handled;
            }
        }

        EventResult::Ignored
    }
}
