//! Progress bar - linear fill indicator

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Fill, VizPrimitive};

/// A horizontal or vertical progress bar
pub struct Bar {
    value: f32,
    target: f32,
    min: f32,
    max: f32,
    warning_threshold: f32,
    critical_threshold: f32,
    vertical: bool,

    bg_color: Hsla,
    normal_color: Hsla,
    warning_color: Hsla,
    critical_color: Hsla,
}

impl Bar {
    pub fn new() -> Self {
        Self {
            value: 0.0,
            target: 0.0,
            min: 0.0,
            max: 1.0,
            warning_threshold: 0.7,
            critical_threshold: 0.9,
            vertical: false,
            bg_color: Hsla::new(0.0, 0.0, 0.15, 1.0),
            normal_color: Hsla::new(145.0 / 360.0, 0.8, 0.4, 1.0),
            warning_color: Hsla::new(45.0 / 360.0, 1.0, 0.5, 1.0),
            critical_color: Hsla::new(0.0, 0.9, 0.5, 1.0),
        }
    }

    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    fn normalized_value(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    fn current_color(&self) -> Hsla {
        let v = self.normalized_value();
        if v >= self.critical_threshold {
            self.critical_color
        } else if v >= self.warning_threshold {
            self.warning_color
        } else {
            self.normal_color
        }
    }
}

impl Default for Bar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Bar {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Animate
        let delta = (self.target - self.value) * 0.15;
        if delta.abs() > 0.0001 {
            self.value += delta;
        }

        // Background
        cx.scene.draw_quad(Quad::new(bounds).with_background(self.bg_color));

        // Fill
        let t = self.normalized_value();
        let fill_bounds = if self.vertical {
            let h = bounds.size.height * t;
            Bounds {
                origin: Point {
                    x: bounds.origin.x,
                    y: bounds.origin.y + bounds.size.height - h,
                },
                size: Size {
                    width: bounds.size.width,
                    height: h,
                },
            }
        } else {
            Bounds {
                origin: bounds.origin,
                size: Size {
                    width: bounds.size.width * t,
                    height: bounds.size.height,
                },
            }
        };

        if t > 0.001 {
            cx.scene.draw_quad(Quad::new(fill_bounds).with_background(self.current_color()));
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        if self.vertical {
            (Some(16.0), Some(64.0))
        } else {
            (Some(100.0), Some(16.0))
        }
    }
}

impl VizPrimitive for Bar {
    fn update(&mut self, value: f32) {
        self.target = value;
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.target = value;
    }
}

impl Fill for Bar {
    fn set_range(&mut self, min: f32, max: f32) {
        self.min = min;
        self.max = max;
    }

    fn set_thresholds(&mut self, warning: f32, critical: f32) {
        self.warning_threshold = warning;
        self.critical_threshold = critical;
    }
}
