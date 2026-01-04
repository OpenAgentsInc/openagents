//! Stream - data streaming visualization (tape/conveyor belt)

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Flow, VizPrimitive};

/// A streaming data visualization
pub struct Stream {
    throughput: f32,
    phase: f32,
    source: Point,
    target: Point,
    particle_count: u32,
    particle_size: f32,
    active_color: Hsla,
    inactive_color: Hsla,
}

impl Stream {
    pub fn new() -> Self {
        Self {
            throughput: 0.0,
            phase: 0.0,
            source: Point::ZERO,
            target: Point::ZERO,
            particle_count: 8,
            particle_size: 6.0,
            active_color: Hsla::new(200.0 / 360.0, 0.8, 0.5, 1.0),
            inactive_color: Hsla::new(0.0, 0.0, 0.2, 1.0),
        }
    }

    pub fn with_particle_count(mut self, count: u32) -> Self {
        self.particle_count = count;
        self
    }

    pub fn with_color(mut self, color: Hsla) -> Self {
        self.active_color = color;
        self
    }
}

impl Default for Stream {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Stream {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Advance phase based on throughput
        self.phase += 0.02 * (0.5 + self.throughput);
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        // Draw track
        let track_height = 4.0;
        let track_y = bounds.origin.y + (bounds.size.height - track_height) / 2.0;
        let track_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: track_y,
            },
            size: Size {
                width: bounds.size.width,
                height: track_height,
            },
        };
        cx.scene.draw_quad(Quad::new(track_bounds).with_background(self.inactive_color));

        // Draw particles
        let spacing = 1.0 / self.particle_count as f32;
        for i in 0..self.particle_count {
            let base_pos = i as f32 * spacing;
            let pos = (base_pos + self.phase) % 1.0;

            let x = bounds.origin.x + pos * bounds.size.width - self.particle_size / 2.0;
            let y = bounds.origin.y + (bounds.size.height - self.particle_size) / 2.0;

            // Fade based on throughput
            let alpha = 0.3 + 0.7 * self.throughput;
            let color = Hsla::new(
                self.active_color.h,
                self.active_color.s,
                self.active_color.l,
                alpha,
            );

            let particle_bounds = Bounds {
                origin: Point { x, y },
                size: Size {
                    width: self.particle_size,
                    height: self.particle_size,
                },
            };

            cx.scene.draw_quad(Quad::new(particle_bounds).with_background(color));
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(120.0), Some(20.0))
    }
}

impl VizPrimitive for Stream {
    fn update(&mut self, value: f32) {
        self.throughput = value.clamp(0.0, 1.0);
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.throughput = value.clamp(0.0, 1.0);
    }
}

impl Flow for Stream {
    fn set_source(&mut self, point: Point) {
        self.source = point;
    }

    fn set_target(&mut self, point: Point) {
        self.target = point;
    }

    fn set_throughput(&mut self, value: f32) {
        self.throughput = value.clamp(0.0, 1.0);
    }
}
