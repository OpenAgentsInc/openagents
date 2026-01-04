//! Flash - one-shot highlight effect

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Quad};

use crate::grammar::{Pulse, VizPrimitive};

/// A flash effect for highlighting
pub struct Flash {
    intensity: f32,
    decay_rate: f32,
    color: Hsla,
}

impl Flash {
    pub fn new() -> Self {
        Self {
            intensity: 0.0,
            decay_rate: 0.05,
            color: Hsla::white(),
        }
    }

    pub fn with_color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }
}

impl Default for Flash {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Flash {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.intensity = (self.intensity - self.decay_rate).max(0.0);

        if self.intensity > 0.01 {
            let color = Hsla::new(
                self.color.h,
                self.color.s,
                self.color.l,
                self.intensity * 0.8,
            );
            cx.scene.draw_quad(Quad::new(bounds).with_background(color));
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

impl VizPrimitive for Flash {
    fn update(&mut self, value: f32) {
        self.intensity = value.clamp(0.0, 1.0);
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.intensity = value.clamp(0.0, 1.0);
    }
}

impl Pulse for Flash {
    fn trigger(&mut self) {
        self.intensity = 1.0;
    }

    fn set_decay(&mut self, decay_ms: u32) {
        // Convert ms to rate (assuming ~60fps)
        self.decay_rate = 1.0 / (decay_ms as f32 / 16.0);
    }
}
