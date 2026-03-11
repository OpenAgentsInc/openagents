use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, EventResult};
use crate::{Bounds, Button, ButtonVariant, Hsla, InputEvent, Point, Quad, SvgQuad, theme};

const CLOSE_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M135.5 169C126.1 159.6 126.1 144.4 135.5 135.1C144.9 125.8 160.1 125.7 169.4 135.1L320.4 286.1L471.4 135.1C480.8 125.7 496 125.7 505.3 135.1C514.6 144.5 514.7 159.7 505.3 169L354.3 320L505.3 471C514.7 480.4 514.7 495.6 505.3 504.9C495.9 514.2 480.7 514.3 471.4 504.9L320.4 353.9L169.4 504.9C160 514.3 144.8 514.3 135.5 504.9C126.2 495.5 126.1 480.3 135.5 471L286.5 320L135.5 169z"/></svg>"##;

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
    close_icon_hover_t: f32,
    pending_close: std::rc::Rc<std::cell::RefCell<bool>>,
}

impl PaneFrame {
    pub fn new() -> Self {
        let pending_close = std::rc::Rc::new(std::cell::RefCell::new(false));
        let pending_close_click = pending_close.clone();
        let close_button = Button::new("")
            .variant(ButtonVariant::Ghost)
            .font_size(theme::font_size::SM)
            .padding(6.0, 2.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_close_click.borrow_mut() = true;
            });

        Self {
            title: String::new(),
            active: false,
            dismissable: true,
            title_height: 36.0,
            padding: 8.0,
            corner_radius: 6.0,
            close_button,
            close_bounds: Bounds::ZERO,
            content_bounds: Bounds::ZERO,
            title_bounds: Bounds::ZERO,
            close_icon_hover_t: 0.0,
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
        let pane_border_width = 1.0;
        let border_color = if self.active {
            theme::border::ACTIVE
        } else {
            theme::border::STRONG
        };
        if self.active {
            let glow_outer_spread = 6.0;
            let glow_inner_spread = 3.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x - glow_outer_spread,
                    bounds.origin.y - glow_outer_spread,
                    bounds.size.width + glow_outer_spread * 2.0,
                    bounds.size.height + glow_outer_spread * 2.0,
                ))
                .with_border(theme::border::ACTIVE.with_alpha(0.16), 1.0)
                .with_corner_radius(self.corner_radius + glow_outer_spread),
            );
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x - glow_inner_spread,
                    bounds.origin.y - glow_inner_spread,
                    bounds.size.width + glow_inner_spread * 2.0,
                    bounds.size.height + glow_inner_spread * 2.0,
                ))
                .with_border(theme::border::ACTIVE.with_alpha(0.30), 1.0)
                .with_corner_radius(self.corner_radius + glow_inner_spread),
            );
        }

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::APP.with_alpha(0.99))
                .with_border(border_color, pane_border_width)
                .with_corner_radius(self.corner_radius),
        );

        let title_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            self.title_height,
        );
        self.title_bounds = title_bounds;
        let header_bg = theme::bg::APP.with_alpha(0.99);

        cx.scene.draw_quad(
            Quad::new(title_bounds)
                .with_background(header_bg)
                .with_border(border_color, pane_border_width)
                .with_corner_radius(self.corner_radius),
        );
        // Keep the header border on top/left/right only by masking out the bottom edge.
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                title_bounds.origin.x,
                title_bounds.max_y() - pane_border_width,
                title_bounds.size.width,
                pane_border_width,
            ))
            .with_background(header_bg),
        );
        // Remove bottom corner rounding while preserving the rounded top corners.
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                title_bounds.origin.x,
                title_bounds.max_y() - self.corner_radius,
                self.corner_radius,
                self.corner_radius,
            ))
            .with_background(header_bg),
        );
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                title_bounds.max_x() - self.corner_radius,
                title_bounds.max_y() - self.corner_radius,
                self.corner_radius,
                self.corner_radius,
            ))
            .with_background(header_bg),
        );
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                title_bounds.origin.x,
                title_bounds.origin.y + self.corner_radius,
                pane_border_width,
                (title_bounds.size.height - self.corner_radius).max(0.0),
            ))
            .with_background(border_color),
        );
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                title_bounds.max_x() - pane_border_width,
                title_bounds.origin.y + self.corner_radius,
                pane_border_width,
                (title_bounds.size.height - self.corner_radius).max(0.0),
            ))
            .with_background(border_color),
        );

        let title_font_size = theme::font_size::SM + 2.0;
        let title_text_x = title_bounds.origin.x + self.padding;
        let title_text_y =
            title_bounds.origin.y + (title_bounds.size.height - title_font_size) * 0.5 - 2.0;
        let title_run = cx.text.layout_mono(
            &self.title,
            Point::new(title_text_x, title_text_y),
            title_font_size,
            theme::text::PRIMARY,
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
            let icon_size = 16.0;
            let icon_bounds = Bounds::new(
                close_bounds.origin.x + (close_bounds.size.width - icon_size) * 0.5,
                close_bounds.origin.y + (close_bounds.size.height - icon_size) * 0.5,
                icon_size,
                icon_size,
            );
            let hover_active = self.close_button.is_hovered() || self.close_button.is_pressed();
            let target_hover_t = if hover_active { 1.0 } else { 0.0 };
            self.close_icon_hover_t += (target_hover_t - self.close_icon_hover_t) * 0.4;
            if (self.close_icon_hover_t - target_hover_t).abs() < 0.01 {
                self.close_icon_hover_t = target_hover_t;
            }
            let base = theme::text::MUTED;
            let hover = theme::accent::PRIMARY;
            let icon_tint = Hsla::new(
                base.h + (hover.h - base.h) * self.close_icon_hover_t,
                base.s + (hover.s - base.s) * self.close_icon_hover_t,
                base.l + (hover.l - base.l) * self.close_icon_hover_t,
                1.0,
            );
            cx.scene.draw_svg(
                SvgQuad::new(
                    icon_bounds,
                    std::sync::Arc::<[u8]>::from(CLOSE_ICON_SVG_RAW.as_bytes()),
                )
                .with_tint(icon_tint),
            );
        } else {
            self.close_bounds = Bounds::ZERO;
            self.close_icon_hover_t = 0.0;
        }

        self.content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + self.title_height,
            bounds.size.width,
            (bounds.size.height - self.title_height).max(0.0),
        );
    }

    fn event(&mut self, event: &InputEvent, _bounds: Bounds, cx: &mut EventContext) -> EventResult {
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
