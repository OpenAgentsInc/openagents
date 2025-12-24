use crate::components::context::PaintContext;
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Quad};

/// Frame style variants inspired by Arwes sci-fi UI framework
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameStyle {
    /// L-shaped corner accents (default)
    Corners,
    /// Full border lines
    Lines,
    /// Cut corners (octagon shape)
    Octagon,
    /// Bottom underline with corner cut
    Underline,
    /// Configurable corner cuts (diagonal corners)
    Nefrex,
    /// Complex angular corners with squares and lines
    Kranox,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self::Corners
    }
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

/// Arwes-style sci-fi frame component
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
    /// Animation progress (0.0 = hidden, 1.0 = fully visible)
    animation_progress: f32,
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

    fn draw_glow_line(&self, cx: &mut PaintContext, bx: f32, by: f32, bw: f32, bh: f32, glow: Hsla) {
        let layers = 4;
        for i in 0..layers {
            let spread = (i as f32 + 1.0) * 2.5;
            let alpha = glow.a * (1.0 - (i as f32 / layers as f32)).powi(2) * 0.06;
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

        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + t, y + t, w - t * 2.0, h - t * 2.0))
                .with_background(self.bg_color),
        );

        if let Some(glow) = self.glow_color {
            self.draw_glow_line(cx, x, y, cl, t, glow);
            self.draw_glow_line(cx, x, y, t, cl, glow);
            self.draw_glow_line(cx, x + w - cl, y, cl, t, glow);
            self.draw_glow_line(cx, x + w - t, y, t, cl, glow);
            self.draw_glow_line(cx, x + w - cl, y + h - t, cl, t, glow);
            self.draw_glow_line(cx, x + w - t, y + h - cl, t, cl, glow);
            self.draw_glow_line(cx, x, y + h - t, cl, t, glow);
            self.draw_glow_line(cx, x, y + h - cl, t, cl, glow);
        }

        cx.scene.draw_quad(Quad::new(Bounds::new(x, y, cl, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y, t, cl)).with_background(self.line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - cl, y, cl, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y, t, cl)).with_background(self.line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - cl, y + h - t, cl, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y + h - cl, t, cl)).with_background(self.line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - t, cl, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - cl, t, cl)).with_background(self.line_color));
    }

    fn paint_lines(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + t, y + t, w - t * 2.0, h - t * 2.0))
                .with_background(self.bg_color),
        );

        if let Some(glow) = self.glow_color {
            self.draw_glow_line(cx, x, y, w, t, glow);
            self.draw_glow_line(cx, x, y + h - t, w, t, glow);
            self.draw_glow_line(cx, x, y, t, h, glow);
            self.draw_glow_line(cx, x + w - t, y, t, h, glow);
        }

        cx.scene.draw_quad(Quad::new(Bounds::new(x, y, w, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - t, w, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y, t, h)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y, t, h)).with_background(self.line_color));
    }

    fn paint_octagon(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let cut = self.corner_length;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + t, y + t, w - t * 2.0, h - t * 2.0))
                .with_background(self.bg_color),
        );

        if let Some(glow) = self.glow_color {
            self.draw_glow_line(cx, x + cut, y, w - cut * 2.0, t, glow);
            self.draw_glow_line(cx, x + cut, y + h - t, w - cut * 2.0, t, glow);
            self.draw_glow_line(cx, x, y + cut, t, h - cut * 2.0, glow);
            self.draw_glow_line(cx, x + w - t, y + cut, t, h - cut * 2.0, glow);
        }

        cx.scene.draw_quad(Quad::new(Bounds::new(x + cut, y, w - cut * 2.0, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + cut, y + h - t, w - cut * 2.0, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + cut, t, h - cut * 2.0)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y + cut, t, h - cut * 2.0)).with_background(self.line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + cut - t, cut, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + cut - t, y, t, cut)).with_background(self.line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - cut, y, cut, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y, t, cut)).with_background(self.line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - cut, y + h - t, cut, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y + h - cut, t, cut)).with_background(self.line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - t, cut, t)).with_background(self.line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - cut, t, cut)).with_background(self.line_color));
    }

    fn paint_underline(&self, bounds: Bounds, cx: &mut PaintContext) {
        let p = self.padding;
        let t = self.stroke_width;
        let ss = self.square_size;
        let x = bounds.origin.x + p;
        let y = bounds.origin.y + p;
        let w = bounds.size.width - p * 2.0;
        let h = bounds.size.height - p * 2.0;

        let progress = self.animation_progress;
        let bg_alpha = self.bg_color.a * progress;
        let line_alpha = self.line_color.a * progress;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, y, w, h))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        if let Some(glow) = self.glow_color {
            self.draw_glow_line(cx, x, y + h - t, w - ss, t, glow);
            self.draw_glow_line(cx, x + w - t, y + h - ss, t, ss, glow);
        }

        let line_color = self.line_color.with_alpha(line_alpha);
        let line_w = (w - ss) * progress;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, y + h - t, line_w, t))
                .with_background(line_color),
        );

        if progress > 0.5 {
            let corner_progress = (progress - 0.5) * 2.0;
            let corner_h = ss * corner_progress;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x + w - ss, y + h - t, ss - t, t))
                    .with_background(line_color),
            );
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x + w - t, y + h - corner_h, t, corner_h))
                    .with_background(line_color),
            );
        }
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

        let progress = self.animation_progress;
        let bg_alpha = self.bg_color.a * progress;
        let line_alpha = self.line_color.a * progress;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + t, y + t, w - t * 2.0, h - t * 2.0))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        if let Some(glow) = self.glow_color {
            if cfg.left_top {
                self.draw_glow_line(cx, x + ss, y, lll, t, glow);
                self.draw_glow_line(cx, x, y + ss, t, sll, glow);
            }
            if cfg.right_bottom {
                self.draw_glow_line(cx, x + w - ss - lll, y + h - t, lll, t, glow);
                self.draw_glow_line(cx, x + w - t, y + h - ss - sll, t, sll, glow);
            }
            if cfg.right_top {
                self.draw_glow_line(cx, x + w - ss - lll, y, lll, t, glow);
                self.draw_glow_line(cx, x + w - t, y + ss, t, sll, glow);
            }
            if cfg.left_bottom {
                self.draw_glow_line(cx, x + ss, y + h - t, lll, t, glow);
                self.draw_glow_line(cx, x, y + h - ss - sll, t, sll, glow);
            }
        }

        let line_color = self.line_color.with_alpha(line_alpha);

        if cfg.left_top {
            cx.scene.draw_quad(Quad::new(Bounds::new(x, y + ss, t, sll)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x, y + ss - t, ss, t)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + ss, y, lll, t)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + ss - t, y, t, ss)).with_background(line_color));
        }

        if cfg.right_top {
            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y + ss, t, sll)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss, y + ss - t, ss, t)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss - lll, y, lll, t)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss, y, t, ss)).with_background(line_color));
        }

        if cfg.left_bottom {
            cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - ss - sll, t, sll)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - ss, ss, t)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + ss, y + h - t, lll, t)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + ss - t, y + h - ss, t, ss)).with_background(line_color));
        }

        if cfg.right_bottom {
            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y + h - ss - sll, t, sll)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss, y + h - ss, ss, t)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss - lll, y + h - t, lll, t)).with_background(line_color));
            cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss, y + h - ss, t, ss)).with_background(line_color));
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

        let progress = self.animation_progress;
        let bg_alpha = self.bg_color.a * progress;
        let line_alpha = self.line_color.a * progress;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(x + ss * 2.0, y + t * 2.0, w - ss * 4.0, h - t * 4.0))
                .with_background(self.bg_color.with_alpha(bg_alpha)),
        );

        if let Some(glow) = self.glow_color {
            self.draw_glow_line(cx, x + ss * 2.0, y, lll, t, glow);
            self.draw_glow_line(cx, x + ss, y + ss, t, sll, glow);
            self.draw_glow_line(cx, x + w - ss * 2.0 - lll, y + h - t, lll, t, glow);
            self.draw_glow_line(cx, x + w - ss - t, y + h - ss - sll, t, sll, glow);
        }

        let line_color = self.line_color.with_alpha(line_alpha);

        cx.scene.draw_quad(Quad::new(Bounds::new(x + ss, y + ss, t, sll)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + ss - t, y + ss - t, ss, t)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + ss * 2.0 - t, y, t, ss)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + ss * 2.0, y, lll, t)).with_background(line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss - t, y + h - ss - sll, t, sll)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss * 2.0, y + h - ss, ss, t)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss * 2.0, y + h - ss, t, ss)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss * 2.0 - lll, y + h - t, lll, t)).with_background(line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - ss * 3.0 - sll - lll, t, lll)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y + h - ss * 2.0 - sll, ss, t)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + ss, y + h - ss * 2.0 - sll, t, sll)).with_background(line_color));

        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - t, y + ss * 3.0 + sll, t, lll)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss, y + ss * 2.0 + sll - t, ss, t)).with_background(line_color));
        cx.scene.draw_quad(Quad::new(Bounds::new(x + w - ss - t, y + ss * 2.0, t, sll)).with_background(line_color));
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
}
