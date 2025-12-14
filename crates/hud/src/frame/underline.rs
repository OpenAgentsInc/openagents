//! FrameUnderline - simple bottom line frame.

use wgpui::{Bounds, Hsla, Quad, Scene};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Simple underline frame.
///
/// Creates a single animated line at the bottom of the bounds,
/// useful for text underlines or simple separators.
///
/// # Example
///
/// ```ignore
/// let mut frame = FrameUnderline::new()
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
pub struct FrameUnderline {
    animator: HudAnimator,
    /// Line width (thickness).
    line_width: f32,
    /// Line color.
    color: Hsla,
    /// Offset from bottom of bounds.
    offset: f32,
    /// Whether to animate from left (true) or center (false).
    from_left: bool,
}

impl Default for FrameUnderline {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameUnderline {
    /// Create a new underline frame.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            line_width: 1.0,
            color: hud::FRAME_NORMAL,
            offset: 0.0,
            from_left: false,
        }
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

    /// Set offset from bottom.
    pub fn offset(mut self, offset: f32) -> Self {
        self.offset = offset;
        self
    }

    /// Set animation direction (from left vs from center).
    pub fn from_left(mut self, from_left: bool) -> Self {
        self.from_left = from_left;
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

    /// Paint the underline.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let color = Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a * progress);
        let w = self.line_width;

        let x = bounds.origin.x;
        let y = bounds.origin.y + bounds.size.height - w - self.offset;
        let width = bounds.size.width;

        let line_width = width * progress;

        if self.from_left {
            // Animate from left edge
            scene.draw_quad(
                Quad::new(Bounds::new(x, y, line_width, w))
                    .with_background(color),
            );
        } else {
            // Animate from center
            let center = x + width / 2.0;
            let half = line_width / 2.0;
            scene.draw_quad(
                Quad::new(Bounds::new(center - half, y, line_width, w))
                    .with_background(color),
            );
        }
    }
}
