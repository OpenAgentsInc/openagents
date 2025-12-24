use crate::components::context::PaintContext;
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

/// Frame style variants inspired by Arwes sci-fi UI framework
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameStyle {
    Corners,
    Lines,
    Octagon,
    Underline,
    Nefrex,
    Kranox,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self::Corners
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameAnimation {
    Fade,
    Draw,
    Flicker,
    Assemble,
}

impl Default for FrameAnimation {
    fn default() -> Self {
        Self::Fade
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
    CenterOut,
    EdgesIn,
}

/// Corner configuration for Nefrex style
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CornerConfig {
    pub left_top: bool,
    pub left_bottom: bool,
    pub right_top: bool,
    pub right_bottom: bool,
}

impl CornerConfig {
    pub fn new() -> Self {
        Self {
            left_top: true,
            left_bottom: false,
            right_top: false,
            right_bottom: true,
        }
    }

    pub fn all() -> Self {
        Self {
            left_top: true,
            left_bottom: true,
            right_top: true,
            right_bottom: true,
        }
    }

    pub fn none() -> Self {
        Self::default()
    }

    pub fn diagonal() -> Self {
        Self::new()
    }
}

pub struct Frame {
    id: Option<ComponentId>,
    style: FrameStyle,
    line_color: Hsla,
    bg_color: Hsla,
    glow_color: Option<Hsla>,
    stroke_width: f32,
    corner_length: f32,
    small_line_length: f32,
    large_line_length: f32,
    square_size: f32,
    padding: f32,
    corner_config: CornerConfig,
    animation_progress: f32,
    animation_mode: FrameAnimation,
    draw_direction: DrawDirection,
    is_exiting: bool,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            id: None,
            style: FrameStyle::Corners,
            line_color: Hsla::new(180.0, 0.8, 0.6, 1.0),
            bg_color: Hsla::new(180.0, 0.3, 0.1, 0.3),
            glow_color: None,
            stroke_width: 2.0,
            corner_length: 20.0,
            small_line_length: 16.0,
            large_line_length: 64.0,
            square_size: 16.0,
            padding: 0.0,
            corner_config: CornerConfig::new(),
            animation_progress: 1.0,
            animation_mode: FrameAnimation::Fade,
            draw_direction: DrawDirection::LeftToRight,
            is_exiting: false,
        }
    }

    pub fn corners() -> Self {
        Self::new().style(FrameStyle::Corners)
    }

    pub fn lines() -> Self {
        Self::new().style(FrameStyle::Lines)
    }

    pub fn octagon() -> Self {
        Self::new().style(FrameStyle::Octagon)
    }

    pub fn underline() -> Self {
        Self::new().style(FrameStyle::Underline)
    }

    pub fn nefrex() -> Self {
        Self::new().style(FrameStyle::Nefrex)
    }

    pub fn kranox() -> Self {
        Self::new().style(FrameStyle::Kranox)
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn style(mut self, style: FrameStyle) -> Self {
        self.style = style;
        self
    }

    pub fn line_color(mut self, color: Hsla) -> Self {
        self.line_color = color;
        self
    }

    pub fn bg_color(mut self, color: Hsla) -> Self {
        self.bg_color = color;
        self
    }

    pub fn glow_color(mut self, color: Hsla) -> Self {
        self.glow_color = Some(color);
        self
    }

    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    pub fn corner_length(mut self, length: f32) -> Self {
        self.corner_length = length;
        self
    }

    pub fn small_line_length(mut self, length: f32) -> Self {
        self.small_line_length = length;
        self
    }

    pub fn large_line_length(mut self, length: f32) -> Self {
        self.large_line_length = length;
        self
    }

    pub fn square_size(mut self, size: f32) -> Self {
        self.square_size = size;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn corner_config(mut self, config: CornerConfig) -> Self {
        self.corner_config = config;
        self
    }

    pub fn animation_progress(mut self, progress: f32) -> Self {
        self.animation_progress = progress.clamp(0.0, 1.0);
        self
    }

    pub fn animation_mode(mut self, mode: FrameAnimation) -> Self {
        self.animation_mode = mode;
        self
    }

    pub fn draw_direction(mut self, direction: DrawDirection) -> Self {
        self.draw_direction = direction;
        self
    }

    pub fn is_exiting(mut self, exiting: bool) -> Self {
        self.is_exiting = exiting;
        self
    }

    fn compute_alpha(&self) -> f32 {
        let p = self.animation_progress;
        match self.animation_mode {
            FrameAnimation::Fade => p,
            FrameAnimation::Flicker => {
                if self.is_exiting {
                    if p < 0.33 { 1.0 - p * 3.0 }
                    else if p < 0.66 { 0.5 }
                    else { 0.5 - (p - 0.66) * 1.5 }
                } else {
                    if p < 0.33 { p * 3.0 }
                    else if p < 0.66 { 1.0 - (p - 0.33) * 1.5 }
                    else { 0.5 + (p - 0.66) * 1.5 }
                }
            }
            FrameAnimation::Draw | FrameAnimation::Assemble => 1.0,
        }
    }

    fn draw_animated_line(
        &self,
        cx: &mut PaintContext,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        horizontal: bool,
    ) {
        let color = self.line_color.with_alpha(self.line_color.a * self.compute_alpha());
        let p = self.animation_progress;

        match self.animation_mode {
            FrameAnimation::Fade | FrameAnimation::Flicker => {
                cx.scene.draw_quad(Quad::new(Bounds::new(x, y, w, h)).with_background(color));
            }
            FrameAnimation::Draw => {
                if horizontal {
                    let len = w * p;
                    let (ax, aw) = match self.draw_direction {
                        DrawDirection::LeftToRight | DrawDirection::TopToBottom => (x, len),
                        DrawDirection::RightToLeft | DrawDirection::BottomToTop => (x + w - len, len),
                        DrawDirection::CenterOut => (x + (w - len) / 2.0, len),
                        DrawDirection::EdgesIn => {
                            let half = len / 2.0;
                            cx.scene.draw_quad(Quad::new(Bounds::new(x, y, half, h)).with_background(color));
                            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - half, y, half, h)).with_background(color));
                            return;
                        }
                    };
                    cx.scene.draw_quad(Quad::new(Bounds::new(ax, y, aw, h)).with_background(color));
                } else {
                    let len = h * p;
                    let (ay, ah) = match self.draw_direction {
                        DrawDirection::TopToBottom | DrawDirection::LeftToRight => (y, len),
                        DrawDirection::BottomToTop | DrawDirection::RightToLeft => (y + h - len, len),
                        DrawDirection::CenterOut => (y + (h - len) / 2.0, len),
                        DrawDirection::EdgesIn => {
                            let half = len / 2.0;
                            cx.scene.draw_quad(Quad::new(Bounds::new(x, y, w, half)).with_background(color));
                            cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - half, w, half)).with_background(color));
                            return;
                        }
                    };
                    cx.scene.draw_quad(Quad::new(Bounds::new(x, ay, w, ah)).with_background(color));
                }
            }
            FrameAnimation::Assemble => {
                let offset = (1.0 - p) * 20.0;
                if horizontal {
                    cx.scene.draw_quad(Quad::new(Bounds::new(x - offset, y, w, h)).with_background(color));
                } else {
                    cx.scene.draw_quad(Quad::new(Bounds::new(x, y - offset, w, h)).with_background(color));
                }
            }
        }
    }

    fn draw_glow_line(&self, cx: &mut PaintContext, bx: f32, by: f32, bw: f32, bh: f32, glow: Hsla) {
        let layers = 4;
        for i in 0..layers {
            let spread = (i as f32 + 1.0) * 2.5;
            let alpha = glow.a * (1.0 - (i as f32 / layers as f32)).powi(2) * 0.2;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(bx - spread, by - spread, bw + spread * 2.0, bh + spread * 2.0))
                    .with_background(glow.with_alpha(alpha)),
            );
        }
    }

    fn paint_corners(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let cl = self.corner_length;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;

        let bg_alpha = self.bg_color.a * self.compute_alpha();
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + t, y + t, w - t * 2.0, h - t * 2.0))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        let glow_alpha = self.compute_alpha();
        if let Some(glow) = self.glow_color {
            let g = glow.with_alpha(glow.a * glow_alpha);
            self.draw_glow_line(cx, x, y, cl, t, g);
            self.draw_glow_line(cx, x, y, t, cl, g);
            self.draw_glow_line(cx, x + w - cl, y, cl, t, g);
            self.draw_glow_line(cx, x + w - t, y, t, cl, g);
            self.draw_glow_line(cx, x + w - cl, y + h - t, cl, t, g);
            self.draw_glow_line(cx, x + w - t, y + h - cl, t, cl, g);
            self.draw_glow_line(cx, x, y + h - t, cl, t, g);
            self.draw_glow_line(cx, x, y + h - cl, t, cl, g);
        }

        self.draw_animated_line(cx, x, y, cl, t, true);
        self.draw_animated_line(cx, x, y, t, cl, false);
        self.draw_animated_line(cx, x + w - cl, y, cl, t, true);
        self.draw_animated_line(cx, x + w - t, y, t, cl, false);
        self.draw_animated_line(cx, x + w - cl, y + h - t, cl, t, true);
        self.draw_animated_line(cx, x + w - t, y + h - cl, t, cl, false);
        self.draw_animated_line(cx, x, y + h - t, cl, t, true);
        self.draw_animated_line(cx, x, y + h - cl, t, cl, false);
    }

    fn paint_lines(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;

        let bg_alpha = self.bg_color.a * self.compute_alpha();
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + t, y + t, w - t * 2.0, h - t * 2.0))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        let glow_alpha = self.compute_alpha();
        if let Some(glow) = self.glow_color {
            let g = glow.with_alpha(glow.a * glow_alpha);
            self.draw_glow_line(cx, x, y, w, t, g);
            self.draw_glow_line(cx, x, y + h - t, w, t, g);
            self.draw_glow_line(cx, x, y, t, h, g);
            self.draw_glow_line(cx, x + w - t, y, t, h, g);
        }

        self.draw_animated_line(cx, x, y, w, t, true);
        self.draw_animated_line(cx, x, y + h - t, w, t, true);
        self.draw_animated_line(cx, x, y, t, h, false);
        self.draw_animated_line(cx, x + w - t, y, t, h, false);
    }

    fn paint_octagon(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let cut = self.corner_length;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;

        let bg_alpha = self.bg_color.a * self.compute_alpha();
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + t, y + t, w - t * 2.0, h - t * 2.0))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        let glow_alpha = self.compute_alpha();
        if let Some(glow) = self.glow_color {
            let g = glow.with_alpha(glow.a * glow_alpha);
            self.draw_glow_line(cx, x + cut, y, w - cut * 2.0, t, g);
            self.draw_glow_line(cx, x + cut, y + h - t, w - cut * 2.0, t, g);
            self.draw_glow_line(cx, x, y + cut, t, h - cut * 2.0, g);
            self.draw_glow_line(cx, x + w - t, y + cut, t, h - cut * 2.0, g);
        }

        self.draw_animated_line(cx, x + cut, y, w - cut * 2.0, t, true);
        self.draw_animated_line(cx, x + cut, y + h - t, w - cut * 2.0, t, true);
        self.draw_animated_line(cx, x, y + cut, t, h - cut * 2.0, false);
        self.draw_animated_line(cx, x + w - t, y + cut, t, h - cut * 2.0, false);

        self.draw_animated_line(cx, x, y + cut - t, cut, t, true);
        self.draw_animated_line(cx, x + cut - t, y, t, cut, false);
        self.draw_animated_line(cx, x + w - cut, y, cut, t, true);
        self.draw_animated_line(cx, x + w - t, y, t, cut, false);
        self.draw_animated_line(cx, x + w - cut, y + h - t, cut, t, true);
        self.draw_animated_line(cx, x + w - t, y + h - cut, t, cut, false);
        self.draw_animated_line(cx, x, y + h - t, cut, t, true);
        self.draw_animated_line(cx, x, y + h - cut, t, cut, false);
    }

    fn paint_underline(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let ss = self.square_size;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;

        let bg_alpha = self.bg_color.a * self.compute_alpha();
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, y, w, h))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        let glow_alpha = self.compute_alpha();
        if let Some(glow) = self.glow_color {
            let g = glow.with_alpha(glow.a * glow_alpha);
            self.draw_glow_line(cx, x, y + h - t, w - ss, t, g);
            self.draw_glow_line(cx, x + w - t, y + h - ss, t, ss, g);
        }

        self.draw_animated_line(cx, x, y + h - t, w - ss, t, true);
        self.draw_animated_line(cx, x + w - ss, y + h - t, ss - t, t, true);
        self.draw_animated_line(cx, x + w - t, y + h - ss, t, ss, false);
    }

    fn paint_nefrex(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let ss = self.square_size;
        let sll = self.small_line_length;
        let lll = self.large_line_length;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;
        let cfg = self.corner_config;

        let bg_alpha = self.bg_color.a * self.compute_alpha();
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + t, y + t, w - t * 2.0, h - t * 2.0))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        let glow_alpha = self.compute_alpha();
        if let Some(glow) = self.glow_color {
            let g = glow.with_alpha(glow.a * glow_alpha);
            if cfg.left_top {
                self.draw_glow_line(cx, x + ss, y, lll, t, g);
                self.draw_glow_line(cx, x, y + ss, t, sll, g);
            }
            if cfg.right_bottom {
                self.draw_glow_line(cx, x + w - ss - lll, y + h - t, lll, t, g);
                self.draw_glow_line(cx, x + w - t, y + h - ss - sll, t, sll, g);
            }
            if cfg.right_top {
                self.draw_glow_line(cx, x + w - ss - lll, y, lll, t, g);
                self.draw_glow_line(cx, x + w - t, y + ss, t, sll, g);
            }
            if cfg.left_bottom {
                self.draw_glow_line(cx, x + ss, y + h - t, lll, t, g);
                self.draw_glow_line(cx, x, y + h - ss - sll, t, sll, g);
            }
        }

        if cfg.left_top {
            self.draw_animated_line(cx, x, y + ss, t, sll, false);
            self.draw_animated_line(cx, x, y + ss - t, ss, t, true);
            self.draw_animated_line(cx, x + ss, y, lll, t, true);
            self.draw_animated_line(cx, x + ss - t, y, t, ss, false);
        }

        if cfg.right_top {
            self.draw_animated_line(cx, x + w - t, y + ss, t, sll, false);
            self.draw_animated_line(cx, x + w - ss, y + ss - t, ss, t, true);
            self.draw_animated_line(cx, x + w - ss - lll, y, lll, t, true);
            self.draw_animated_line(cx, x + w - ss, y, t, ss, false);
        }

        if cfg.left_bottom {
            self.draw_animated_line(cx, x, y + h - ss - sll, t, sll, false);
            self.draw_animated_line(cx, x, y + h - ss, ss, t, true);
            self.draw_animated_line(cx, x + ss, y + h - t, lll, t, true);
            self.draw_animated_line(cx, x + ss - t, y + h - ss, t, ss, false);
        }

        if cfg.right_bottom {
            self.draw_animated_line(cx, x + w - t, y + h - ss - sll, t, sll, false);
            self.draw_animated_line(cx, x + w - ss, y + h - ss, ss, t, true);
            self.draw_animated_line(cx, x + w - ss - lll, y + h - t, lll, t, true);
            self.draw_animated_line(cx, x + w - ss, y + h - ss, t, ss, false);
        }
    }

    fn paint_kranox(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let ss = self.square_size;
        let sll = self.small_line_length;
        let lll = self.large_line_length;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;

        let bg_alpha = self.bg_color.a * self.compute_alpha();
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + ss * 2.0, y + t * 2.0, w - ss * 4.0, h - t * 4.0))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        let glow_alpha = self.compute_alpha();
        if let Some(glow) = self.glow_color {
            let g = glow.with_alpha(glow.a * glow_alpha);
            self.draw_glow_line(cx, x + ss * 2.0, y, lll, t, g);
            self.draw_glow_line(cx, x + ss, y + ss, t, sll, g);
            self.draw_glow_line(cx, x + w - ss * 2.0 - lll, y + h - t, lll, t, g);
            self.draw_glow_line(cx, x + w - ss - t, y + h - ss - sll, t, sll, g);
        }

        self.draw_animated_line(cx, x + ss, y + ss, t, sll, false);
        self.draw_animated_line(cx, x + ss - t, y + ss - t, ss, t, true);
        self.draw_animated_line(cx, x + ss * 2.0 - t, y, t, ss, false);
        self.draw_animated_line(cx, x + ss * 2.0, y, lll, t, true);

        self.draw_animated_line(cx, x + w - ss - t, y + h - ss - sll, t, sll, false);
        self.draw_animated_line(cx, x + w - ss * 2.0, y + h - ss, ss, t, true);
        self.draw_animated_line(cx, x + w - ss * 2.0, y + h - ss, t, ss, false);
        self.draw_animated_line(cx, x + w - ss * 2.0 - lll, y + h - t, lll, t, true);

        self.draw_animated_line(cx, x, y + h - ss * 3.0 - sll - lll, t, lll, false);
        self.draw_animated_line(cx, x, y + h - ss * 2.0 - sll, ss, t, true);
        self.draw_animated_line(cx, x + ss, y + h - ss * 2.0 - sll, t, sll, false);

        self.draw_animated_line(cx, x + w - t, y + ss * 3.0 + sll, t, lll, false);
        self.draw_animated_line(cx, x + w - ss, y + ss * 2.0 + sll - t, ss, t, true);
        self.draw_animated_line(cx, x + w - ss - t, y + ss * 2.0, t, sll, false);
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Frame {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        match self.style {
            FrameStyle::Corners => self.paint_corners(bounds, cx),
            FrameStyle::Lines => self.paint_lines(bounds, cx),
            FrameStyle::Octagon => self.paint_octagon(bounds, cx),
            FrameStyle::Underline => self.paint_underline(bounds, cx),
            FrameStyle::Nefrex => self.paint_nefrex(bounds, cx),
            FrameStyle::Kranox => self.paint_kranox(bounds, cx),
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut crate::components::context::EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_new() {
        let frame = Frame::new();
        assert_eq!(frame.style, FrameStyle::Corners);
        assert_eq!(frame.animation_progress, 1.0);
    }

    #[test]
    fn test_frame_builders() {
        let corners = Frame::corners();
        assert_eq!(corners.style, FrameStyle::Corners);

        let lines = Frame::lines();
        assert_eq!(lines.style, FrameStyle::Lines);

        let octagon = Frame::octagon();
        assert_eq!(octagon.style, FrameStyle::Octagon);

        let underline = Frame::underline();
        assert_eq!(underline.style, FrameStyle::Underline);

        let nefrex = Frame::nefrex();
        assert_eq!(nefrex.style, FrameStyle::Nefrex);

        let kranox = Frame::kranox();
        assert_eq!(kranox.style, FrameStyle::Kranox);
    }

    #[test]
    fn test_frame_customization() {
        let frame = Frame::new()
            .with_id(1)
            .stroke_width(3.0)
            .corner_length(30.0)
            .padding(5.0);

        assert_eq!(frame.id, Some(1));
        assert_eq!(frame.stroke_width, 3.0);
        assert_eq!(frame.corner_length, 30.0);
        assert_eq!(frame.padding, 5.0);
    }

    #[test]
    fn test_corner_config() {
        let default = CornerConfig::new();
        assert!(default.left_top);
        assert!(!default.left_bottom);
        assert!(!default.right_top);
        assert!(default.right_bottom);

        let all = CornerConfig::all();
        assert!(all.left_top);
        assert!(all.left_bottom);
        assert!(all.right_top);
        assert!(all.right_bottom);

        let none = CornerConfig::none();
        assert!(!none.left_top);
        assert!(!none.left_bottom);
        assert!(!none.right_top);
        assert!(!none.right_bottom);
    }

    #[test]
    fn test_nefrex_with_corner_config() {
        let frame = Frame::nefrex()
            .corner_config(CornerConfig::all());
        assert_eq!(frame.style, FrameStyle::Nefrex);
        assert!(frame.corner_config.left_top);
        assert!(frame.corner_config.right_bottom);
    }

    #[test]
    fn test_kranox_dimensions() {
        let frame = Frame::kranox()
            .small_line_length(20.0)
            .large_line_length(80.0)
            .square_size(20.0);
        assert_eq!(frame.small_line_length, 20.0);
        assert_eq!(frame.large_line_length, 80.0);
        assert_eq!(frame.square_size, 20.0);
    }

    #[test]
    fn test_glow_color() {
        let frame = Frame::new()
            .glow_color(Hsla::new(180.0, 1.0, 0.5, 0.5));
        assert!(frame.glow_color.is_some());
    }

    #[test]
    fn test_animation_progress() {
        let frame = Frame::new().animation_progress(0.5);
        assert_eq!(frame.animation_progress, 0.5);

        let clamped = Frame::new().animation_progress(1.5);
        assert_eq!(clamped.animation_progress, 1.0);

        let clamped_neg = Frame::new().animation_progress(-0.5);
        assert_eq!(clamped_neg.animation_progress, 0.0);
    }

    #[test]
    fn test_animation_mode_builder() {
        let fade = Frame::new().animation_mode(FrameAnimation::Fade);
        assert_eq!(fade.animation_mode, FrameAnimation::Fade);

        let draw = Frame::new().animation_mode(FrameAnimation::Draw);
        assert_eq!(draw.animation_mode, FrameAnimation::Draw);

        let flicker = Frame::new().animation_mode(FrameAnimation::Flicker);
        assert_eq!(flicker.animation_mode, FrameAnimation::Flicker);

        let assemble = Frame::new().animation_mode(FrameAnimation::Assemble);
        assert_eq!(assemble.animation_mode, FrameAnimation::Assemble);
    }

    #[test]
    fn test_draw_direction_builder() {
        let ltr = Frame::new().draw_direction(DrawDirection::LeftToRight);
        assert_eq!(ltr.draw_direction, DrawDirection::LeftToRight);

        let rtl = Frame::new().draw_direction(DrawDirection::RightToLeft);
        assert_eq!(rtl.draw_direction, DrawDirection::RightToLeft);

        let ttb = Frame::new().draw_direction(DrawDirection::TopToBottom);
        assert_eq!(ttb.draw_direction, DrawDirection::TopToBottom);

        let btt = Frame::new().draw_direction(DrawDirection::BottomToTop);
        assert_eq!(btt.draw_direction, DrawDirection::BottomToTop);

        let center = Frame::new().draw_direction(DrawDirection::CenterOut);
        assert_eq!(center.draw_direction, DrawDirection::CenterOut);

        let edges = Frame::new().draw_direction(DrawDirection::EdgesIn);
        assert_eq!(edges.draw_direction, DrawDirection::EdgesIn);
    }

    #[test]
    fn test_is_exiting_builder() {
        let entering = Frame::new().is_exiting(false);
        assert!(!entering.is_exiting);

        let exiting = Frame::new().is_exiting(true);
        assert!(exiting.is_exiting);
    }

    #[test]
    fn test_compute_alpha_fade() {
        let frame_0 = Frame::new()
            .animation_mode(FrameAnimation::Fade)
            .animation_progress(0.0);
        assert_eq!(frame_0.compute_alpha(), 0.0);

        let frame_50 = Frame::new()
            .animation_mode(FrameAnimation::Fade)
            .animation_progress(0.5);
        assert_eq!(frame_50.compute_alpha(), 0.5);

        let frame_100 = Frame::new()
            .animation_mode(FrameAnimation::Fade)
            .animation_progress(1.0);
        assert_eq!(frame_100.compute_alpha(), 1.0);
    }

    #[test]
    fn test_compute_alpha_draw_and_assemble() {
        let draw = Frame::new()
            .animation_mode(FrameAnimation::Draw)
            .animation_progress(0.5);
        assert_eq!(draw.compute_alpha(), 1.0);

        let assemble = Frame::new()
            .animation_mode(FrameAnimation::Assemble)
            .animation_progress(0.5);
        assert_eq!(assemble.compute_alpha(), 1.0);
    }

    #[test]
    fn test_compute_alpha_flicker_entering() {
        let early = Frame::new()
            .animation_mode(FrameAnimation::Flicker)
            .animation_progress(0.1)
            .is_exiting(false);
        assert!(early.compute_alpha() > 0.0 && early.compute_alpha() < 0.5);

        let mid = Frame::new()
            .animation_mode(FrameAnimation::Flicker)
            .animation_progress(0.5)
            .is_exiting(false);
        assert!(mid.compute_alpha() > 0.5);

        let late = Frame::new()
            .animation_mode(FrameAnimation::Flicker)
            .animation_progress(0.9)
            .is_exiting(false);
        assert!(late.compute_alpha() > 0.8);
    }

    #[test]
    fn test_compute_alpha_flicker_exiting() {
        let early = Frame::new()
            .animation_mode(FrameAnimation::Flicker)
            .animation_progress(0.1)
            .is_exiting(true);
        assert!(early.compute_alpha() > 0.5);

        let late = Frame::new()
            .animation_mode(FrameAnimation::Flicker)
            .animation_progress(0.9)
            .is_exiting(true);
        assert!(late.compute_alpha() < 0.2);
    }

    #[test]
    fn test_animation_defaults() {
        let frame = Frame::new();
        assert_eq!(frame.animation_mode, FrameAnimation::Fade);
        assert_eq!(frame.draw_direction, DrawDirection::LeftToRight);
        assert!(!frame.is_exiting);
    }

    #[test]
    fn test_frame_animation_default() {
        assert_eq!(FrameAnimation::default(), FrameAnimation::Fade);
    }

    #[test]
    fn test_chained_animation_config() {
        let frame = Frame::corners()
            .animation_mode(FrameAnimation::Draw)
            .draw_direction(DrawDirection::CenterOut)
            .animation_progress(0.7)
            .is_exiting(false);

        assert_eq!(frame.style, FrameStyle::Corners);
        assert_eq!(frame.animation_mode, FrameAnimation::Draw);
        assert_eq!(frame.draw_direction, DrawDirection::CenterOut);
        assert_eq!(frame.animation_progress, 0.7);
        assert!(!frame.is_exiting);
    }
}
