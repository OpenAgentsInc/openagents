//! Illuminator - mouse-following radial glow effect.

use wgpui::{Bounds, Hsla, Point, Quad, Scene};

use crate::animator::HudAnimator;

/// Mouse-following radial glow effect.
///
/// Creates a soft radial glow that follows the mouse cursor,
/// adding an interactive sci-fi ambiance to the UI.
///
/// # Example
///
/// ```ignore
/// let mut illuminator = Illuminator::new()
///     .size(200.0)
///     .color(Hsla::new(0.0, 0.0, 1.0, 0.15));
///
/// illuminator.animator_mut().enter();
///
/// // On mouse move:
/// illuminator.set_position(mouse_x, mouse_y);
///
/// // In update:
/// illuminator.tick();
///
/// // In paint:
/// illuminator.paint(screen_bounds, &mut scene);
/// ```
pub struct Illuminator {
    animator: HudAnimator,
    /// Current position (mouse position).
    position: Point,
    /// Target position (for smooth following).
    target: Point,
    /// Size of the illumination (diameter).
    size: f32,
    /// Center color (most intense).
    color: Hsla,
    /// Number of gradient rings to draw.
    rings: u32,
    /// Smoothing factor for mouse following (0-1, lower = smoother).
    smoothing: f32,
}

impl Default for Illuminator {
    fn default() -> Self {
        Self::new()
    }
}

impl Illuminator {
    /// Create a new illuminator effect.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new(),
            position: Point::new(0.0, 0.0),
            target: Point::new(0.0, 0.0),
            size: 150.0,
            color: Hsla::new(0.0, 0.0, 1.0, 0.12),
            rings: 8,
            smoothing: 0.15,
        }
    }

    /// Set the illumination size (diameter).
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(10.0);
        self
    }

    /// Set the illumination color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set the number of gradient rings.
    pub fn rings(mut self, rings: u32) -> Self {
        self.rings = rings.max(2);
        self
    }

    /// Set the smoothing factor (0-1, lower = more lag).
    pub fn smoothing(mut self, smoothing: f32) -> Self {
        self.smoothing = smoothing.clamp(0.01, 1.0);
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

    /// Set the target position (typically mouse position).
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.target = Point::new(x, y);
    }

    /// Tick the animation and update position.
    pub fn tick(&mut self) -> bool {
        // Smooth follow
        self.position.x += (self.target.x - self.position.x) * self.smoothing;
        self.position.y += (self.target.y - self.position.y) * self.smoothing;

        self.animator.tick()
    }

    /// Paint the illuminator effect.
    ///
    /// Draws concentric circles with decreasing opacity to create
    /// a soft radial gradient effect.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = self.animator.progress();
        if progress <= 0.0 {
            return;
        }

        // Only draw if position is within or near bounds
        let margin = self.size;
        if self.position.x < bounds.origin.x - margin
            || self.position.x > bounds.origin.x + bounds.size.width + margin
            || self.position.y < bounds.origin.y - margin
            || self.position.y > bounds.origin.y + bounds.size.height + margin
        {
            return;
        }

        let base_alpha = self.color.a * progress;
        let radius = self.size / 2.0;

        // Draw rings from outside in (larger to smaller)
        for i in 0..self.rings {
            let t = (self.rings - 1 - i) as f32 / (self.rings - 1) as f32;
            let ring_radius = radius * (1.0 - t * 0.9); // Inner rings are smaller
            let ring_alpha = base_alpha * t * t; // Quadratic falloff

            if ring_alpha < 0.001 {
                continue;
            }

            let color = Hsla::new(self.color.h, self.color.s, self.color.l, ring_alpha);
            let diameter = ring_radius * 2.0;

            scene.draw_quad(
                Quad::new(Bounds::new(
                    self.position.x - ring_radius,
                    self.position.y - ring_radius,
                    diameter,
                    diameter,
                ))
                .with_background(color)
                .with_uniform_radius(ring_radius),
            );
        }
    }
}
