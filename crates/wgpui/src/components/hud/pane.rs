use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, EventResult};
use crate::{Bounds, Button, ButtonVariant, Hsla, InputEvent, Point, Quad, SvgQuad, theme};

const CLOSE_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M135.5 169C126.1 159.6 126.1 144.4 135.5 135.1C144.9 125.8 160.1 125.7 169.4 135.1L320.4 286.1L471.4 135.1C480.8 125.7 496 125.7 505.3 135.1C514.6 144.5 514.7 159.7 505.3 169L354.3 320L505.3 471C514.7 480.4 514.7 495.6 505.3 504.9C495.9 514.2 480.7 514.3 471.4 504.9L320.4 353.9L169.4 504.9C160 514.3 144.8 514.3 135.5 504.9C126.2 495.5 126.1 480.3 135.5 471L286.5 320L135.5 169z"/></svg>"##;
const FULLSCREEN_ICON_SVG_RAW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 640"><path fill="#FFFFFF" d="M112 112H272V160H160V272H112V112zM368 112H528V272H480V160H368V112zM112 368H160V480H272V528H112V368zM480 368H528V528H368V480H480V368z"/></svg>"##;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneHeaderAction {
    Fullscreen,
}

pub struct PaneFrame {
    title: String,
    active: bool,
    dismissable: bool,
    header_action: Option<PaneHeaderAction>,
    title_height: f32,
    padding: f32,
    corner_radius: f32,
    header_action_button: Button,
    header_action_bounds: Bounds,
    close_button: Button,
    close_bounds: Bounds,
    content_bounds: Bounds,
    title_bounds: Bounds,
    header_action_icon_hover_t: f32,
    close_icon_hover_t: f32,
    pending_header_action: std::rc::Rc<std::cell::RefCell<Option<PaneHeaderAction>>>,
    pending_close: std::rc::Rc<std::cell::RefCell<bool>>,
}

impl PaneFrame {
    pub fn new() -> Self {
        let pending_header_action = std::rc::Rc::new(std::cell::RefCell::new(None));
        let pending_header_action_click = pending_header_action.clone();
        let header_action_button = Button::new("")
            .variant(ButtonVariant::Ghost)
            .font_size(theme::font_size::SM)
            .padding(6.0, 2.0)
            .corner_radius(6.0)
            .on_click(move || {
                *pending_header_action_click.borrow_mut() = Some(PaneHeaderAction::Fullscreen);
            });
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
            header_action: None,
            title_height: 36.0,
            padding: 8.0,
            corner_radius: 6.0,
            header_action_button,
            header_action_bounds: Bounds::ZERO,
            close_button,
            close_bounds: Bounds::ZERO,
            content_bounds: Bounds::ZERO,
            title_bounds: Bounds::ZERO,
            header_action_icon_hover_t: 0.0,
            close_icon_hover_t: 0.0,
            pending_header_action,
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

    pub fn header_action(mut self, header_action: Option<PaneHeaderAction>) -> Self {
        self.header_action = header_action;
        self
    }

    pub fn set_header_action(&mut self, header_action: Option<PaneHeaderAction>) {
        self.header_action = header_action;
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

    pub fn header_action_bounds(&self) -> Bounds {
        self.header_action_bounds
    }

    pub fn take_header_action_clicked(&mut self) -> Option<PaneHeaderAction> {
        self.pending_header_action.borrow_mut().take()
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

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::APP)
                .with_border(border_color, pane_border_width)
                .with_corner_radius(self.corner_radius),
        );
        // Reinforce the outer shell edges so the bottom rounded corners close cleanly.
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y + self.corner_radius,
                pane_border_width,
                (bounds.size.height - self.corner_radius * 2.0).max(0.0),
            ))
            .with_background(border_color),
        );
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.max_x() - pane_border_width,
                bounds.origin.y + self.corner_radius,
                pane_border_width,
                (bounds.size.height - self.corner_radius * 2.0).max(0.0),
            ))
            .with_background(border_color),
        );
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + self.corner_radius,
                bounds.max_y() - pane_border_width,
                (bounds.size.width - self.corner_radius * 2.0).max(0.0),
                pane_border_width,
            ))
            .with_background(border_color),
        );

        let title_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            self.title_height,
        );
        self.title_bounds = title_bounds;
        let header_bg = theme::bg::APP;

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

        let button_size = (self.title_height - theme::spacing::SM).max(18.0);
        let button_y = title_bounds.origin.y + (title_bounds.size.height - button_size) * 0.5;
        let right_edge = title_bounds.origin.x + title_bounds.size.width - theme::spacing::SM;
        let action_slot_width = button_size + theme::spacing::XS;
        let mut title_clip_right = right_edge;
        if self.dismissable {
            title_clip_right -= action_slot_width;
        }
        if self.header_action.is_some() {
            title_clip_right -= action_slot_width;
        }
        let title_font_size = theme::font_size::SM + 2.0;
        let title_text_x = title_bounds.origin.x + self.padding;
        let title_clip_bounds = Bounds::new(
            title_text_x,
            title_bounds.origin.y,
            (title_clip_right - title_text_x).max(0.0),
            title_bounds.size.height,
        );
        let mut title_run = cx.text.layout_mono(
            &self.title,
            Point::ZERO,
            title_font_size,
            theme::text::PRIMARY,
        );
        let title_run_bounds = title_run.bounds();
        let title_visual_nudge_y = -2.0;
        title_run.origin = Point::new(
            title_text_x - title_run_bounds.origin.x,
            title_bounds.origin.y
                + ((title_bounds.size.height - title_run_bounds.size.height).max(0.0) * 0.5)
                - title_run_bounds.origin.y
                + title_visual_nudge_y,
        );
        cx.scene.push_clip(title_clip_bounds);
        cx.scene.draw_text(title_run);
        cx.scene.pop_clip();

        if self.dismissable {
            let close_bounds =
                Bounds::new(right_edge - button_size, button_y, button_size, button_size);
            self.close_bounds = close_bounds;
            self.close_button.paint(close_bounds, cx);
            paint_header_icon(
                cx,
                close_bounds,
                &self.close_button,
                &mut self.close_icon_hover_t,
                CLOSE_ICON_SVG_RAW,
            );
        } else {
            self.close_bounds = Bounds::ZERO;
            self.close_icon_hover_t = 0.0;
        }

        if self.header_action.is_some() {
            let button_x = if self.dismissable {
                self.close_bounds.origin.x - theme::spacing::XS - button_size
            } else {
                right_edge - button_size
            };
            let action_bounds = Bounds::new(button_x, button_y, button_size, button_size);
            self.header_action_bounds = action_bounds;
            self.header_action_button.paint(action_bounds, cx);
            paint_header_icon(
                cx,
                action_bounds,
                &self.header_action_button,
                &mut self.header_action_icon_hover_t,
                FULLSCREEN_ICON_SVG_RAW,
            );
        } else {
            self.header_action_bounds = Bounds::ZERO;
            self.header_action_icon_hover_t = 0.0;
        }

        self.content_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + self.title_height,
            bounds.size.width,
            (bounds.size.height - self.title_height).max(0.0),
        );
    }

    fn event(&mut self, event: &InputEvent, _bounds: Bounds, cx: &mut EventContext) -> EventResult {
        if self.header_action.is_some() {
            let handled = self
                .header_action_button
                .event(event, self.header_action_bounds, cx)
                .is_handled();
            if handled {
                return EventResult::Handled;
            }
        }

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

fn paint_header_icon(
    cx: &mut PaintContext,
    button_bounds: Bounds,
    button: &Button,
    hover_t: &mut f32,
    icon_svg_raw: &'static str,
) {
    let icon_size = 16.0;
    let icon_bounds = Bounds::new(
        button_bounds.origin.x + (button_bounds.size.width - icon_size) * 0.5,
        button_bounds.origin.y + (button_bounds.size.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );
    let hover_active = button.is_hovered() || button.is_pressed();
    let target_hover_t = if hover_active { 1.0 } else { 0.0 };
    *hover_t += (target_hover_t - *hover_t) * 0.4;
    if (*hover_t - target_hover_t).abs() < 0.01 {
        *hover_t = target_hover_t;
    }
    let base = theme::text::MUTED;
    let hover = theme::accent::PRIMARY;
    let icon_tint = Hsla::new(
        base.h + (hover.h - base.h) * *hover_t,
        base.s + (hover.s - base.s) * *hover_t,
        base.l + (hover.l - base.l) * *hover_t,
        1.0,
    );
    cx.scene.draw_svg(
        SvgQuad::new(
            icon_bounds,
            std::sync::Arc::<[u8]>::from(icon_svg_raw.as_bytes()),
        )
        .with_tint(icon_tint),
    );
}
