//! Event marker - timeline event indicator

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Pulse, VizPrimitive};

/// An event marker for timelines
pub struct EventMarker {
    intensity: f32,
    decay_ms: u32,
    color: Hsla,
    size: f32,
}

impl EventMarker {
    pub fn new() -> Self {
        Self {
            intensity: 0.0,
            decay_ms: 300,
            color: Hsla::new(200.0 / 360.0, 0.8, 0.6, 1.0),
            size: 8.0,
        }
    }

    pub fn with_color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl Default for EventMarker {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for EventMarker {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Decay the intensity
        self.intensity = (self.intensity - 0.02).max(0.0);

        let center = Point {
            x: bounds.origin.x + bounds.size.width / 2.0,
            y: bounds.origin.y + bounds.size.height / 2.0,
        };

        // Draw the marker as a diamond shape using 4 quads
        let alpha = 0.3 + 0.7 * self.intensity;
        let color = Hsla::new(self.color.h, self.color.s, self.color.l, alpha);
        let s = self.size * (0.8 + 0.4 * self.intensity);

        // Center quad
        let marker_bounds = Bounds {
            origin: Point {
                x: center.x - s / 2.0,
                y: center.y - s / 2.0,
            },
            size: Size {
                width: s,
                height: s,
            },
        };
        cx.scene.draw_quad(Quad::new(marker_bounds).with_background(color));

        // Glow effect when triggered
        if self.intensity > 0.5 {
            let glow_alpha = (self.intensity - 0.5) * 0.4;
            let glow_color = Hsla::new(self.color.h, self.color.s, self.color.l, glow_alpha);
            let gs = s * 1.8;
            let glow_bounds = Bounds {
                origin: Point {
                    x: center.x - gs / 2.0,
                    y: center.y - gs / 2.0,
                },
                size: Size {
                    width: gs,
                    height: gs,
                },
            };
            cx.scene.draw_quad(Quad::new(glow_bounds).with_background(glow_color));
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let s = self.size * 3.0;
        (Some(s), Some(s))
    }
}

impl VizPrimitive for EventMarker {
    fn update(&mut self, value: f32) {
        self.intensity = value.clamp(0.0, 1.0);
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.intensity = value.clamp(0.0, 1.0);
    }
}

impl Pulse for EventMarker {
    fn trigger(&mut self) {
        self.intensity = 1.0;
    }

    fn set_decay(&mut self, decay_ms: u32) {
        self.decay_ms = decay_ms;
    }
}
