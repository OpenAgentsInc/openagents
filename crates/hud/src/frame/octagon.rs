//! FrameOctagon - 8-sided frame with clipped corners.

use wgpui::{Bounds, Hsla, Quad, Scene};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Octagonal frame with animated edges.
///
/// Creates an 8-sided frame by drawing lines along each edge,
/// with corners cut at 45 degrees. Animates by growing lines
/// from their centers outward.
///
/// # Example
///
/// ```ignore
/// let mut frame = FrameOctagon::new()
///     .corner_size(20.0)
///     .line_width(2.0)
///     .color(theme::hud::FRAME_NORMAL);
///
/// frame.animator_mut().enter();
///
/// // In update:
/// frame.tick();
///
/// // In paint:
/// frame.paint(bounds, &mut scene);
/// ```
pub struct FrameOctagon {
    animator: HudAnimator,
    /// Size of the corner cut (how much is clipped from each corner).
    corner_size: f32,
    /// Line width.
    line_width: f32,
    /// Line color.
    color: Hsla,
}

impl Default for FrameOctagon {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameOctagon {
    /// Create a new octagon frame with default settings.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            corner_size: 15.0,
            line_width: 1.0,
            color: hud::FRAME_NORMAL,
        }
    }

    /// Set the corner cut size.
    pub fn corner_size(mut self, size: f32) -> Self {
        self.corner_size = size.max(1.0);
        self
    }

    /// Set line width.
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width.max(0.5);
        self
    }

    /// Set line color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
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

    /// Paint the octagon frame.
    ///
    /// The octagon has 8 edges:
    /// - Top (horizontal, shortened by corners)
    /// - Top-right diagonal
    /// - Right (vertical, shortened by corners)
    /// - Bottom-right diagonal
    /// - Bottom (horizontal, shortened by corners)
    /// - Bottom-left diagonal
    /// - Left (vertical, shortened by corners)
    /// - Top-left diagonal
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let color = Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a * progress);
        let w = self.line_width;
        let c = self.corner_size;

        let x = bounds.origin.x;
        let y = bounds.origin.y;
        let width = bounds.size.width;
        let height = bounds.size.height;

        // Clamp corner size to fit within bounds
        let c = c.min(width / 3.0).min(height / 3.0);

        // Calculate key points
        // Top edge: from (x + c, y) to (x + width - c, y)
        // Top-right diagonal: from (x + width - c, y) to (x + width, y + c)
        // Right edge: from (x + width, y + c) to (x + width, y + height - c)
        // etc.

        // Draw each edge as a quad, animating length from center

        // Top edge (horizontal)
        let top_start_x = x + c;
        let top_end_x = x + width - c;
        let top_len = top_end_x - top_start_x;
        let top_center = top_start_x + top_len / 2.0;
        let top_half = (top_len / 2.0) * progress;
        scene.draw_quad(
            Quad::new(Bounds::new(top_center - top_half, y, top_half * 2.0, w))
                .with_background(color),
        );

        // Bottom edge (horizontal)
        let bot_start_x = x + c;
        let bot_end_x = x + width - c;
        let bot_len = bot_end_x - bot_start_x;
        let bot_center = bot_start_x + bot_len / 2.0;
        let bot_half = (bot_len / 2.0) * progress;
        scene.draw_quad(
            Quad::new(Bounds::new(bot_center - bot_half, y + height - w, bot_half * 2.0, w))
                .with_background(color),
        );

        // Left edge (vertical)
        let left_start_y = y + c;
        let left_end_y = y + height - c;
        let left_len = left_end_y - left_start_y;
        let left_center = left_start_y + left_len / 2.0;
        let left_half = (left_len / 2.0) * progress;
        scene.draw_quad(
            Quad::new(Bounds::new(x, left_center - left_half, w, left_half * 2.0))
                .with_background(color),
        );

        // Right edge (vertical)
        let right_start_y = y + c;
        let right_end_y = y + height - c;
        let right_len = right_end_y - right_start_y;
        let right_center = right_start_y + right_len / 2.0;
        let right_half = (right_len / 2.0) * progress;
        scene.draw_quad(
            Quad::new(Bounds::new(x + width - w, right_center - right_half, w, right_half * 2.0))
                .with_background(color),
        );

        // Diagonal corners - draw as rotated quads approximated with small rectangles
        // For simplicity, we'll draw the diagonals as a series of small steps

        // Top-left diagonal: from (x, y + c) to (x + c, y)
        self.draw_diagonal(scene, x, y + c, x + c, y, w, color, progress);

        // Top-right diagonal: from (x + width - c, y) to (x + width, y + c)
        self.draw_diagonal(scene, x + width - c, y, x + width, y + c, w, color, progress);

        // Bottom-right diagonal: from (x + width, y + height - c) to (x + width - c, y + height)
        self.draw_diagonal(scene, x + width, y + height - c, x + width - c, y + height, w, color, progress);

        // Bottom-left diagonal: from (x + c, y + height) to (x, y + height - c)
        self.draw_diagonal(scene, x + c, y + height, x, y + height - c, w, color, progress);
    }

    /// Draw a diagonal line from (x1, y1) to (x2, y2) using small quads.
    fn draw_diagonal(
        &self,
        scene: &mut Scene,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: Hsla,
        progress: f32,
    ) {
        // Calculate the diagonal length and direction
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();

        if len < 0.1 {
            return;
        }

        // Animate from center of diagonal outward
        let segments = (len / 2.0).ceil() as i32;
        let mid_x = (x1 + x2) / 2.0;
        let mid_y = (y1 + y2) / 2.0;

        // Draw from center, extending based on progress
        let draw_len = (len / 2.0) * progress;
        let unit_x = dx / len;
        let unit_y = dy / len;

        // Draw small quads along the diagonal
        let step_size = 2.0_f32.max(width);
        let steps = (draw_len / step_size).ceil() as i32;

        for i in -steps..=steps {
            let t = i as f32 * step_size;
            if t.abs() > draw_len {
                continue;
            }
            let px = mid_x + unit_x * t;
            let py = mid_y + unit_y * t;

            scene.draw_quad(
                Quad::new(Bounds::new(px - width / 2.0, py - width / 2.0, width, width))
                    .with_background(color),
            );
        }
    }
}
