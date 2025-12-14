//! Status light indicator component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_cubic;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size};

/// Status state for the indicator.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum StatusState {
    /// Offline/inactive state.
    #[default]
    Offline,
    /// Online/active state.
    Online,
    /// Warning state.
    Warning,
    /// Error/critical state.
    Error,
    /// Processing/busy state (animated).
    Busy,
}

impl StatusState {
    /// Get the color for this state.
    pub fn color(&self) -> Hsla {
        match self {
            StatusState::Offline => Hsla::new(0.0, 0.0, 0.4, 0.5),
            StatusState::Online => Hsla::new(0.35, 0.8, 0.5, 1.0),   // Green
            StatusState::Warning => Hsla::new(0.12, 0.8, 0.5, 1.0),  // Orange
            StatusState::Error => Hsla::new(0.0, 0.8, 0.5, 1.0),     // Red
            StatusState::Busy => Hsla::new(0.6, 0.8, 0.6, 1.0),      // Cyan
        }
    }
}

/// Status light indicator (LED-style).
pub struct StatusLight {
    state: StatusState,
    animator: HudAnimator,
    pulse_phase: f32,

    // Styling
    size: f32,
    glow_radius: f32,
    show_ring: bool,
}

impl StatusLight {
    /// Create a new status light.
    pub fn new() -> Self {
        Self {
            state: StatusState::Offline,
            animator: HudAnimator::new().enter_duration(15),
            pulse_phase: 0.0,
            size: 12.0,
            glow_radius: 4.0,
            show_ring: true,
        }
    }

    /// Set the status state.
    pub fn state(mut self, state: StatusState) -> Self {
        self.state = state;
        self
    }

    /// Set the light size.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Show/hide the outer ring.
    pub fn ring(mut self, show: bool) -> Self {
        self.show_ring = show;
        self
    }

    /// Update the current state.
    pub fn set_state(&mut self, state: StatusState) {
        self.state = state;
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

        // Pulse animation for busy state
        if self.state == StatusState::Busy {
            self.pulse_phase += 0.1;
            if self.pulse_phase >= std::f32::consts::TAU {
                self.pulse_phase -= std::f32::consts::TAU;
            }
        }
    }

    /// Paint the status light.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene) {
        let progress = ease_out_cubic(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let center_x = bounds.x() + bounds.width() / 2.0;
        let center_y = bounds.y() + bounds.height() / 2.0;
        let color = self.state.color();

        // Calculate pulse effect
        let pulse = if self.state == StatusState::Busy {
            0.7 + 0.3 * self.pulse_phase.sin()
        } else {
            1.0
        };

        // Draw glow (for active states)
        if self.state != StatusState::Offline {
            let glow_size = self.size + self.glow_radius * 2.0;
            let glow_bounds = Bounds::from_origin_size(
                Point::new(center_x - glow_size / 2.0, center_y - glow_size / 2.0),
                Size::new(glow_size, glow_size),
            );
            scene.draw_quad(
                wgpui::Quad::new(glow_bounds)
                    .with_background(Hsla::new(color.h, color.s, color.l, 0.2 * progress * pulse))
                    .with_uniform_radius(glow_size / 2.0),
            );
        }

        // Draw outer ring
        if self.show_ring {
            let ring_size = self.size + 4.0;
            let ring_bounds = Bounds::from_origin_size(
                Point::new(center_x - ring_size / 2.0, center_y - ring_size / 2.0),
                Size::new(ring_size, ring_size),
            );
            scene.draw_quad(
                wgpui::Quad::new(ring_bounds)
                    .with_border(Hsla::new(colors::FRAME_DIM.h, colors::FRAME_DIM.s, colors::FRAME_DIM.l, colors::FRAME_DIM.a * progress), 1.0)
                    .with_uniform_radius(ring_size / 2.0),
            );
        }

        // Draw main light
        let light_bounds = Bounds::from_origin_size(
            Point::new(center_x - self.size / 2.0, center_y - self.size / 2.0),
            Size::new(self.size, self.size),
        );
        scene.draw_quad(
            wgpui::Quad::new(light_bounds)
                .with_background(Hsla::new(color.h, color.s, color.l, color.a * progress * pulse))
                .with_uniform_radius(self.size / 2.0),
        );

        // Draw highlight
        let highlight_size = self.size * 0.3;
        let highlight_bounds = Bounds::from_origin_size(
            Point::new(center_x - highlight_size, center_y - highlight_size),
            Size::new(highlight_size, highlight_size),
        );
        scene.draw_quad(
            wgpui::Quad::new(highlight_bounds)
                .with_background(Hsla::new(0.0, 0.0, 1.0, 0.4 * progress * pulse))
                .with_uniform_radius(highlight_size / 2.0),
        );
    }
}

impl Default for StatusLight {
    fn default() -> Self {
        Self::new()
    }
}
