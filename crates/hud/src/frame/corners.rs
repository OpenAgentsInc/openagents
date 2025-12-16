//! FrameCorners - bracket-style corner frames.

use wgpui::{Bounds, Hsla, Quad, Scene};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Which corner of a rectangle.
#[derive(Debug, Clone, Copy)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Corner bracket frame - draws `[ ]` style corners.
///
/// Each corner consists of two perpendicular line segments forming
/// an L-shape. The corners animate by growing from zero to full length.
///
/// # Example
///
/// ```ignore
/// let mut frame = FrameCorners::new()
///     .corner_length(30.0)
///     .line_width(2.0)
///     .color(theme::hud::FRAME_NORMAL);
///
/// frame.animator_mut().enter();
///
/// // In update loop:
/// frame.tick();
///
/// // In paint:
/// frame.paint(bounds, &mut scene);
/// ```
pub struct FrameCorners {
    animator: HudAnimator,
    /// Length of each corner bracket arm.
    corner_length: f32,
    /// Thickness of bracket lines.
    line_width: f32,
    /// Color of brackets.
    color: Hsla,
    /// Inset from bounds edge.
    padding: f32,
}

impl Default for FrameCorners {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCorners {
    /// Create a new FrameCorners with default settings.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            corner_length: 20.0,
            line_width: 1.0,
            color: hud::FRAME_NORMAL,
            padding: 0.0,
        }
    }

    /// Set the corner arm length.
    pub fn corner_length(mut self, len: f32) -> Self {
        self.corner_length = len;
        self
    }

    /// Set the line thickness.
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width;
        self
    }

    /// Set the bracket color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set padding from bounds edge.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Get the animator.
    pub fn animator(&self) -> &HudAnimator {
        &self.animator
    }

    /// Get mutable animator.
    pub fn animator_mut(&mut self) -> &mut HudAnimator {
        &mut self.animator
    }

    /// Tick the animation.
    pub fn tick(&mut self) -> bool {
        self.animator.tick()
    }

    /// Paint the frame corners.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        // Apply padding
        let inner = Bounds::new(
            bounds.origin.x + self.padding,
            bounds.origin.y + self.padding,
            bounds.size.width - self.padding * 2.0,
            bounds.size.height - self.padding * 2.0,
        );

        // Animate both length and alpha
        let alpha = progress;
        let color = Hsla::new(
            self.color.h,
            self.color.s,
            self.color.l,
            self.color.a * alpha,
        );

        // Animate corner length from 0 to full
        let animated_length = self.corner_length * progress;

        // Draw all four corners
        self.draw_corner(scene, &inner, Corner::TopLeft, animated_length, color);
        self.draw_corner(scene, &inner, Corner::TopRight, animated_length, color);
        self.draw_corner(scene, &inner, Corner::BottomLeft, animated_length, color);
        self.draw_corner(scene, &inner, Corner::BottomRight, animated_length, color);
    }

    fn draw_corner(
        &self,
        scene: &mut Scene,
        bounds: &Bounds,
        corner: Corner,
        length: f32,
        color: Hsla,
    ) {
        let w = self.line_width;
        let half_w = w / 2.0;

        let (origin_x, origin_y, h_dir, v_dir) = match corner {
            Corner::TopLeft => (bounds.origin.x, bounds.origin.y, 1.0_f32, 1.0_f32),
            Corner::TopRight => (
                bounds.origin.x + bounds.size.width,
                bounds.origin.y,
                -1.0,
                1.0,
            ),
            Corner::BottomLeft => (
                bounds.origin.x,
                bounds.origin.y + bounds.size.height,
                1.0,
                -1.0,
            ),
            Corner::BottomRight => (
                bounds.origin.x + bounds.size.width,
                bounds.origin.y + bounds.size.height,
                -1.0,
                -1.0,
            ),
        };

        // Horizontal line segment
        let h_x = if h_dir > 0.0 {
            origin_x
        } else {
            origin_x - length
        };
        let h_y = if v_dir > 0.0 {
            origin_y - half_w
        } else {
            origin_y - half_w
        };

        let h_bounds = Bounds::new(h_x, h_y, length, w);
        scene.draw_quad(Quad::new(h_bounds).with_background(color));

        // Vertical line segment
        let v_x = if h_dir > 0.0 {
            origin_x - half_w
        } else {
            origin_x - half_w
        };
        let v_y = if v_dir > 0.0 {
            origin_y
        } else {
            origin_y - length
        };

        let v_bounds = Bounds::new(v_x, v_y, w, length);
        scene.draw_quad(Quad::new(v_bounds).with_background(color));
    }
}
