//! FrameLines - edge line frames with center gaps.

use wgpui::{Bounds, Hsla, Quad, Scene};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Which sides to draw.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl FrameSides {
    /// All sides enabled.
    pub fn all() -> Self {
        Self {
            top: true,
            right: true,
            bottom: true,
            left: true,
        }
    }

    /// No sides enabled.
    pub fn none() -> Self {
        Self::default()
    }

    /// Only horizontal sides (top and bottom).
    pub fn horizontal() -> Self {
        Self {
            top: true,
            bottom: true,
            ..Self::default()
        }
    }

    /// Only vertical sides (left and right).
    pub fn vertical() -> Self {
        Self {
            left: true,
            right: true,
            ..Self::default()
        }
    }
}

/// Simple line-based frame - draws straight lines on edges.
///
/// Lines have an optional gap in the center for a cyber aesthetic.
/// They animate by growing from the center outward.
///
/// # Example
///
/// ```ignore
/// let mut frame = FrameLines::new()
///     .sides(FrameSides::horizontal())
///     .gap(40.0)
///     .line_width(1.0);
///
/// frame.animator_mut().enter();
/// ```
pub struct FrameLines {
    animator: HudAnimator,
    sides: FrameSides,
    /// Gap in the middle of each line.
    gap: f32,
    /// Line thickness.
    line_width: f32,
    /// Line color.
    color: Hsla,
    /// Inset from bounds edge.
    padding: f32,
}

impl Default for FrameLines {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameLines {
    /// Create a new FrameLines with default settings.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            sides: FrameSides::all(),
            gap: 0.0,
            line_width: 1.0,
            color: hud::FRAME_NORMAL,
            padding: 0.0,
        }
    }

    /// Set which sides to draw.
    pub fn sides(mut self, sides: FrameSides) -> Self {
        self.sides = sides;
        self
    }

    /// Set the center gap size.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Set line thickness.
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width;
        self
    }

    /// Set line color.
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

    /// Paint the frame lines.
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

        let alpha = progress;
        let color = Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a * alpha);

        let w = self.line_width;
        let half_gap = self.gap / 2.0;

        // Draw top
        if self.sides.top {
            self.draw_horizontal_line(
                scene,
                inner.origin.x,
                inner.origin.y,
                inner.size.width,
                w,
                half_gap,
                progress,
                color,
            );
        }

        // Draw bottom
        if self.sides.bottom {
            self.draw_horizontal_line(
                scene,
                inner.origin.x,
                inner.origin.y + inner.size.height - w,
                inner.size.width,
                w,
                half_gap,
                progress,
                color,
            );
        }

        // Draw left
        if self.sides.left {
            self.draw_vertical_line(
                scene,
                inner.origin.x,
                inner.origin.y,
                w,
                inner.size.height,
                half_gap,
                progress,
                color,
            );
        }

        // Draw right
        if self.sides.right {
            self.draw_vertical_line(
                scene,
                inner.origin.x + inner.size.width - w,
                inner.origin.y,
                w,
                inner.size.height,
                half_gap,
                progress,
                color,
            );
        }
    }

    fn draw_horizontal_line(
        &self,
        scene: &mut Scene,
        x: f32,
        y: f32,
        total_width: f32,
        height: f32,
        half_gap: f32,
        progress: f32,
        color: Hsla,
    ) {
        let center_x = x + total_width / 2.0;

        if half_gap > 0.0 {
            // Two segments with gap
            let segment_width = (total_width / 2.0 - half_gap) * progress;

            if segment_width > 0.0 {
                // Left segment (grows from center-gap leftward)
                let left_x = center_x - half_gap - segment_width;
                scene.draw_quad(
                    Quad::new(Bounds::new(left_x, y, segment_width, height)).with_background(color),
                );

                // Right segment (grows from center+gap rightward)
                let right_x = center_x + half_gap;
                scene.draw_quad(
                    Quad::new(Bounds::new(right_x, y, segment_width, height))
                        .with_background(color),
                );
            }
        } else {
            // Single line, animate from center outward
            let line_width = total_width * progress;
            let line_x = center_x - line_width / 2.0;
            scene.draw_quad(
                Quad::new(Bounds::new(line_x, y, line_width, height)).with_background(color),
            );
        }
    }

    fn draw_vertical_line(
        &self,
        scene: &mut Scene,
        x: f32,
        y: f32,
        width: f32,
        total_height: f32,
        half_gap: f32,
        progress: f32,
        color: Hsla,
    ) {
        let center_y = y + total_height / 2.0;

        if half_gap > 0.0 {
            // Two segments with gap
            let segment_height = (total_height / 2.0 - half_gap) * progress;

            if segment_height > 0.0 {
                // Top segment (grows from center-gap upward)
                let top_y = center_y - half_gap - segment_height;
                scene.draw_quad(
                    Quad::new(Bounds::new(x, top_y, width, segment_height)).with_background(color),
                );

                // Bottom segment (grows from center+gap downward)
                let bottom_y = center_y + half_gap;
                scene.draw_quad(
                    Quad::new(Bounds::new(x, bottom_y, width, segment_height))
                        .with_background(color),
                );
            }
        } else {
            // Single line, animate from center outward
            let line_height = total_height * progress;
            let line_y = center_y - line_height / 2.0;
            scene.draw_quad(
                Quad::new(Bounds::new(x, line_y, width, line_height)).with_background(color),
            );
        }
    }
}
