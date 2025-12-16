//! FrameCircle - circular frame border.

use wgpui::{Bounds, Hsla, Quad, Scene};

use crate::animator::HudAnimator;
use crate::theme::hud;

/// Circular frame border.
///
/// Creates a circular frame by drawing many small quads arranged
/// in a circle. Animates by drawing the circle from top, spreading
/// clockwise and counter-clockwise simultaneously.
///
/// # Example
///
/// ```ignore
/// let mut frame = FrameCircle::new()
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
pub struct FrameCircle {
    animator: HudAnimator,
    /// Line width.
    line_width: f32,
    /// Line color.
    color: Hsla,
    /// Number of segments to draw the circle.
    segments: u32,
}

impl Default for FrameCircle {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCircle {
    /// Create a new circle frame with default settings.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            line_width: 1.0,
            color: hud::FRAME_NORMAL,
            segments: 64,
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

    /// Set number of segments (more = smoother circle).
    pub fn segments(mut self, segments: u32) -> Self {
        self.segments = segments.max(8);
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

    /// Paint the circular frame.
    ///
    /// Uses the smaller dimension of bounds as diameter.
    /// Animates by drawing from top (12 o'clock) spreading both ways.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        let color = Hsla::new(
            self.color.h,
            self.color.s,
            self.color.l,
            self.color.a * progress,
        );
        let w = self.line_width;

        // Center and radius
        let cx = bounds.origin.x + bounds.size.width / 2.0;
        let cy = bounds.origin.y + bounds.size.height / 2.0;
        let radius = (bounds.size.width.min(bounds.size.height) / 2.0) - w / 2.0;

        if radius <= 0.0 {
            return;
        }

        // Calculate how many segments to draw based on progress
        // Progress 0->1 maps to 0->full circle
        let segments_to_draw = ((self.segments as f32) * progress).ceil() as u32;
        let half_segments = segments_to_draw / 2;

        // Draw segments from top (angle = -PI/2), spreading both directions
        let segment_angle = std::f32::consts::TAU / self.segments as f32;

        // Draw clockwise half
        for i in 0..=half_segments {
            let angle = -std::f32::consts::FRAC_PI_2 + (i as f32) * segment_angle;
            let x = cx + angle.cos() * radius;
            let y = cy + angle.sin() * radius;

            scene.draw_quad(
                Quad::new(Bounds::new(x - w / 2.0, y - w / 2.0, w, w))
                    .with_background(color)
                    .with_uniform_radius(w / 2.0),
            );
        }

        // Draw counter-clockwise half
        for i in 1..=half_segments {
            let angle = -std::f32::consts::FRAC_PI_2 - (i as f32) * segment_angle;
            let x = cx + angle.cos() * radius;
            let y = cy + angle.sin() * radius;

            scene.draw_quad(
                Quad::new(Bounds::new(x - w / 2.0, y - w / 2.0, w, w))
                    .with_background(color)
                    .with_uniform_radius(w / 2.0),
            );
        }
    }
}
