//! DotGridBackground - animated dot grid pattern.

use wgpui::{Bounds, Hsla, Point, Quad, Scene};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Animated dot grid background pattern.
///
/// Creates a regular grid of dots that fade in during animation.
/// Perfect for sci-fi HUD backgrounds.
///
/// # Example
///
/// ```ignore
/// let mut bg = DotGridBackground::new()
///     .spacing(25.0)
///     .dot_radius(1.5)
///     .color(theme::hud::DOT_GRID);
///
/// bg.animator_mut().enter();
///
/// // In update:
/// bg.tick();
///
/// // In paint:
/// bg.paint(screen_bounds, &mut scene);
/// ```
pub struct DotGridBackground {
    animator: HudAnimator,
    /// Spacing between dots.
    spacing: f32,
    /// Dot radius.
    dot_radius: f32,
    /// Dot color.
    color: Hsla,
    /// Offset for scrolling/animation effects.
    offset: Point,
}

impl Default for DotGridBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl DotGridBackground {
    /// Create a new dot grid with default settings.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            spacing: 20.0,
            dot_radius: 1.0,
            color: hud::DOT_GRID,
            offset: Point::new(0.0, 0.0),
        }
    }

    /// Set dot spacing.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(1.0);
        self
    }

    /// Set dot radius.
    pub fn dot_radius(mut self, radius: f32) -> Self {
        self.dot_radius = radius.max(0.5);
        self
    }

    /// Set dot color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set grid offset (for scrolling effects).
    pub fn offset(mut self, offset: Point) -> Self {
        self.offset = offset;
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

    /// Update the offset (for scrolling effects).
    pub fn set_offset(&mut self, offset: Point) {
        self.offset = offset;
    }

    /// Tick the animation.
    pub fn tick(&mut self) -> bool {
        self.animator.tick()
    }

    /// Paint the dot grid.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let alpha = self.color.a * progress;
        let color = Hsla::new(self.color.h, self.color.s, self.color.l, alpha);

        // Calculate grid aligned to spacing
        let offset_x = self.offset.x.rem_euclid(self.spacing);
        let offset_y = self.offset.y.rem_euclid(self.spacing);

        let start_x = bounds.origin.x + offset_x;
        let start_y = bounds.origin.y + offset_y;

        // Calculate number of dots needed
        let cols = ((bounds.size.width - offset_x) / self.spacing).ceil() as i32 + 1;
        let rows = ((bounds.size.height - offset_y) / self.spacing).ceil() as i32 + 1;

        let r = self.dot_radius;
        let diameter = r * 2.0;

        for row in 0..rows {
            for col in 0..cols {
                let x = start_x + col as f32 * self.spacing;
                let y = start_y + row as f32 * self.spacing;

                // Skip dots outside bounds
                if x < bounds.origin.x - r
                    || x > bounds.origin.x + bounds.size.width + r
                    || y < bounds.origin.y - r
                    || y > bounds.origin.y + bounds.size.height + r
                {
                    continue;
                }

                // Draw circular dot using rounded quad
                scene.draw_quad(
                    Quad::new(Bounds::new(x - r, y - r, diameter, diameter))
                        .with_background(color)
                        .with_uniform_radius(r),
                );
            }
        }
    }
}
