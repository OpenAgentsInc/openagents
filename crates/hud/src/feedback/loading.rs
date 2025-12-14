//! Loading spinner/indicator component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_cubic;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size};

/// Loading indicator component.
pub struct Loading {
    animator: HudAnimator,
    rotation: f32,
    pulse: f32,

    // Styling
    color: Hsla,
    size: f32,
    line_width: f32,
    dot_count: usize,
}

impl Loading {
    /// Create a new loading indicator.
    pub fn new() -> Self {
        Self {
            animator: HudAnimator::new().enter_duration(15),
            rotation: 0.0,
            pulse: 0.0,
            color: colors::FRAME_BRIGHT,
            size: 32.0,
            line_width: 2.0,
            dot_count: 8,
        }
    }

    /// Set color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set size.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Set number of dots.
    pub fn dots(mut self, count: usize) -> Self {
        self.dot_count = count.max(4);
        self
    }

    /// Start enter animation.
    pub fn enter(&mut self) {
        self.animator.enter();
    }

    /// Start exit animation.
    pub fn exit(&mut self) {
        self.animator.exit();
    }

    /// Update animation state.
    pub fn tick(&mut self) {
        self.animator.tick();

        // Continuous rotation
        self.rotation += 0.05;
        if self.rotation >= std::f32::consts::TAU {
            self.rotation -= std::f32::consts::TAU;
        }

        // Pulse animation
        self.pulse += 0.08;
        if self.pulse >= std::f32::consts::TAU {
            self.pulse -= std::f32::consts::TAU;
        }
    }

    /// Paint the loading indicator.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = ease_out_cubic(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let center_x = bounds.x() + bounds.width() / 2.0;
        let center_y = bounds.y() + bounds.height() / 2.0;
        let radius = self.size / 2.0 - self.line_width;
        let dot_size = self.line_width * 2.0;

        // Draw dots in a circle with fading trail
        for i in 0..self.dot_count {
            let angle = (i as f32 / self.dot_count as f32) * std::f32::consts::TAU + self.rotation;

            let x = center_x + angle.cos() * radius - dot_size / 2.0;
            let y = center_y + angle.sin() * radius - dot_size / 2.0;

            // Calculate opacity based on position in trail
            let trail_position = i as f32 / self.dot_count as f32;
            let base_alpha = 0.2 + 0.8 * trail_position;

            // Add pulse effect
            let pulse_factor = 1.0 + 0.2 * (self.pulse + i as f32 * 0.5).sin();

            let alpha = self.color.a * progress * base_alpha;
            let size = dot_size * pulse_factor;

            let dot_bounds = Bounds::from_origin_size(
                Point::new(x - (size - dot_size) / 2.0, y - (size - dot_size) / 2.0),
                Size::new(size, size),
            );

            scene.draw_quad(
                wgpui::Quad::new(dot_bounds)
                    .with_background(Hsla::new(self.color.h, self.color.s, self.color.l, alpha))
                    .with_uniform_radius(size / 2.0),
            );
        }

        // Draw center dot
        let center_pulse = 0.8 + 0.2 * self.pulse.sin();
        let center_size = dot_size * center_pulse;
        let center_bounds = Bounds::from_origin_size(
            Point::new(center_x - center_size / 2.0, center_y - center_size / 2.0),
            Size::new(center_size, center_size),
        );
        scene.draw_quad(
            wgpui::Quad::new(center_bounds)
                .with_background(Hsla::new(self.color.h, self.color.s, self.color.l, self.color.a * progress * 0.5))
                .with_uniform_radius(center_size / 2.0),
        );
    }
}

impl Default for Loading {
    fn default() -> Self {
        Self::new()
    }
}
