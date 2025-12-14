//! GridLinesBackground - animated grid line pattern.

use wgpui::{Bounds, Hsla, Quad, Scene};

use crate::animator::HudAnimator;

/// Animated grid line background pattern.
///
/// Creates a grid of horizontal and vertical lines that fade in
/// during animation. Perfect for sci-fi HUD backgrounds.
///
/// # Example
///
/// ```ignore
/// let mut bg = GridLinesBackground::new()
///     .spacing(50.0)
///     .line_width(1.0)
///     .color(theme::hud::FRAME_DIM);
///
/// bg.animator_mut().enter();
///
/// // In update:
/// bg.tick();
///
/// // In paint:
/// bg.paint(screen_bounds, &mut scene);
/// ```
pub struct GridLinesBackground {
    animator: HudAnimator,
    /// Spacing between grid lines.
    spacing: f32,
    /// Line width.
    line_width: f32,
    /// Line color.
    color: Hsla,
    /// Whether to draw horizontal lines.
    horizontal: bool,
    /// Whether to draw vertical lines.
    vertical: bool,
}

impl Default for GridLinesBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl GridLinesBackground {
    /// Create a new grid lines background with default settings.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            spacing: 50.0,
            line_width: 1.0,
            color: Hsla::new(0.0, 0.0, 1.0, 0.1), // Very subtle white
            horizontal: true,
            vertical: true,
        }
    }

    /// Set grid spacing.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(10.0);
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

    /// Enable/disable horizontal lines.
    pub fn horizontal(mut self, enabled: bool) -> Self {
        self.horizontal = enabled;
        self
    }

    /// Enable/disable vertical lines.
    pub fn vertical(mut self, enabled: bool) -> Self {
        self.vertical = enabled;
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

    /// Paint the grid lines.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let color = Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a * progress);
        let w = self.line_width;

        let x = bounds.origin.x;
        let y = bounds.origin.y;
        let width = bounds.size.width;
        let height = bounds.size.height;

        // Draw vertical lines
        if self.vertical {
            let mut line_x = x;
            while line_x <= x + width {
                scene.draw_quad(
                    Quad::new(Bounds::new(line_x - w / 2.0, y, w, height))
                        .with_background(color),
                );
                line_x += self.spacing;
            }
        }

        // Draw horizontal lines
        if self.horizontal {
            let mut line_y = y;
            while line_y <= y + height {
                scene.draw_quad(
                    Quad::new(Bounds::new(x, line_y - w / 2.0, width, w))
                        .with_background(color),
                );
                line_y += self.spacing;
            }
        }
    }
}
