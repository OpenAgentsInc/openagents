//! Heartbeat - periodic pulse indicator

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Pulse, VizPrimitive};

/// A pulsing heartbeat indicator
pub struct Heartbeat {
    phase: f32,
    rate: f32, // pulses per second
    intensity: f32,
    decay_ms: u32,
    color: Hsla,
    active: bool,
}

impl Heartbeat {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            rate: 1.0,
            intensity: 0.5,
            decay_ms: 200,
            color: Hsla::new(0.0, 0.9, 0.5, 1.0),
            active: true,
        }
    }

    pub fn with_rate(mut self, rate: f32) -> Self {
        self.rate = rate;
        self
    }

    pub fn with_color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }
}

impl Default for Heartbeat {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Heartbeat {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.active {
            self.intensity = (self.intensity - 0.05).max(0.0);
        } else {
            // Advance phase
            self.phase += 0.016 * self.rate; // ~60fps
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }

            // Heartbeat curve: quick rise, slow fall
            self.intensity = if self.phase < 0.15 {
                // First beat
                (self.phase / 0.15).min(1.0)
            } else if self.phase < 0.25 {
                // First fall
                1.0 - (self.phase - 0.15) / 0.1 * 0.6
            } else if self.phase < 0.35 {
                // Second beat (smaller)
                0.4 + (self.phase - 0.25) / 0.1 * 0.4
            } else if self.phase < 0.5 {
                // Second fall
                0.8 - (self.phase - 0.35) / 0.15 * 0.8
            } else {
                // Rest
                0.0
            };
        }

        let center = Point {
            x: bounds.origin.x + bounds.size.width / 2.0,
            y: bounds.origin.y + bounds.size.height / 2.0,
        };

        let base_size = bounds.size.width.min(bounds.size.height) / 2.0;
        let size = base_size * (0.6 + 0.4 * self.intensity);

        // Draw pulsing square (approximating circle)
        let alpha = 0.3 + 0.7 * self.intensity;
        let color = Hsla::new(self.color.h, self.color.s, self.color.l, alpha);

        let pulse_bounds = Bounds {
            origin: Point {
                x: center.x - size / 2.0,
                y: center.y - size / 2.0,
            },
            size: Size {
                width: size,
                height: size,
            },
        };

        cx.scene.draw_quad(Quad::new(pulse_bounds).with_background(color).with_corner_radius(size / 2.0));
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(32.0), Some(32.0))
    }
}

impl VizPrimitive for Heartbeat {
    fn update(&mut self, value: f32) {
        self.rate = value;
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.rate = value;
    }
}

impl Pulse for Heartbeat {
    fn trigger(&mut self) {
        self.phase = 0.0;
        self.active = true;
    }

    fn set_decay(&mut self, decay_ms: u32) {
        self.decay_ms = decay_ms;
    }
}
