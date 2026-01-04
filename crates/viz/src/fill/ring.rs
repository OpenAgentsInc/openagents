//! Ring gauge - circular progress indicator

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad};

use crate::grammar::{Fill, VizPrimitive};

/// A ring/donut gauge showing progress around a circle
pub struct Ring {
    value: f32,
    target: f32,
    min: f32,
    max: f32,
    warning_threshold: f32,
    critical_threshold: f32,
    thickness: f32,
    segments: u32,
    animation_speed: f32,

    // Colors
    bg_color: Hsla,
    normal_color: Hsla,
    warning_color: Hsla,
    critical_color: Hsla,
}

impl Ring {
    pub fn new() -> Self {
        Self {
            value: 0.0,
            target: 0.0,
            min: 0.0,
            max: 1.0,
            warning_threshold: 0.7,
            critical_threshold: 0.9,
            thickness: 8.0,
            segments: 64,
            animation_speed: 5.0,
            bg_color: Hsla::new(0.0, 0.0, 0.15, 1.0),
            normal_color: Hsla::new(145.0 / 360.0, 0.8, 0.4, 1.0),
            warning_color: Hsla::new(45.0 / 360.0, 1.0, 0.5, 1.0),
            critical_color: Hsla::new(0.0, 0.9, 0.5, 1.0),
        }
    }

    pub fn with_thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    pub fn with_segments(mut self, segments: u32) -> Self {
        self.segments = segments;
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

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Ring {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Animate towards target
        let delta = (self.target - self.value) * 0.1 * self.animation_speed;
        if delta.abs() > 0.0001 {
            self.value += delta;
        }

        let center = Point {
            x: bounds.origin.x + bounds.size.width / 2.0,
            y: bounds.origin.y + bounds.size.height / 2.0,
        };
        let radius = (bounds.size.width.min(bounds.size.height) / 2.0) - self.thickness;

        // Draw background ring
        draw_arc(
            cx,
            center,
            radius,
            self.thickness,
            0.0,
            std::f32::consts::TAU,
            self.bg_color,
            self.segments,
        );

        // Draw filled portion
        let fill_angle = self.normalized_value() * std::f32::consts::TAU;
        if fill_angle > 0.01 {
            draw_arc(
                cx,
                center,
                radius,
                self.thickness,
                -std::f32::consts::FRAC_PI_2, // Start from top
                fill_angle,
                self.current_color(),
                self.segments,
            );
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(64.0), Some(64.0))
    }
}

impl VizPrimitive for Ring {
    fn update(&mut self, value: f32) {
        self.target = value;
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.target = value;
    }
}

impl Fill for Ring {
    fn set_range(&mut self, min: f32, max: f32) {
        self.min = min;
        self.max = max;
    }

    fn set_thresholds(&mut self, warning: f32, critical: f32) {
        self.warning_threshold = warning;
        self.critical_threshold = critical;
    }
}

/// Draw an arc using quads (approximated as small segments)
fn draw_arc(
    cx: &mut PaintContext,
    center: Point,
    radius: f32,
    thickness: f32,
    start_angle: f32,
    sweep: f32,
    color: Hsla,
    segments: u32,
) {
    let segments = (segments as f32 * (sweep / std::f32::consts::TAU)).max(4.0) as u32;
    let step = sweep / segments as f32;

    for i in 0..segments {
        let a0 = start_angle + step * i as f32;
        let a1 = start_angle + step * (i + 1) as f32;
        let mid_angle = (a0 + a1) / 2.0;

        // Position a small quad at the arc segment
        let seg_x = center.x + radius * mid_angle.cos() - thickness / 2.0;
        let seg_y = center.y + radius * mid_angle.sin() - thickness / 2.0;

        let seg_bounds = Bounds {
            origin: Point { x: seg_x, y: seg_y },
            size: wgpui::Size {
                width: thickness,
                height: thickness,
            },
        };

        cx.scene.draw_quad(Quad::new(seg_bounds).with_background(color));
    }
}
