//! Signal meter - multi-segment level indicator

use wgpui::components::{Component, PaintContext};
use wgpui::{Bounds, Hsla, Point, Quad, Size};

use crate::grammar::{Fill, VizPrimitive};

/// A segmented level meter (like VU meters)
pub struct Meter {
    value: f32,
    target: f32,
    min: f32,
    max: f32,
    warning_threshold: f32,
    critical_threshold: f32,
    segments: u32,
    gap: f32,
    vertical: bool,

    off_color: Hsla,
    normal_color: Hsla,
    warning_color: Hsla,
    critical_color: Hsla,
}

impl Meter {
    pub fn new(segments: u32) -> Self {
        Self {
            value: 0.0,
            target: 0.0,
            min: 0.0,
            max: 1.0,
            warning_threshold: 0.7,
            critical_threshold: 0.9,
            segments,
            gap: 2.0,
            vertical: true,
            off_color: Hsla::new(0.0, 0.0, 0.1, 1.0),
            normal_color: Hsla::new(145.0 / 360.0, 0.8, 0.4, 1.0),
            warning_color: Hsla::new(45.0 / 360.0, 1.0, 0.5, 1.0),
            critical_color: Hsla::new(0.0, 0.9, 0.5, 1.0),
        }
    }

    pub fn horizontal(mut self) -> Self {
        self.vertical = false;
        self
    }

    pub fn with_gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    fn normalized_value(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    fn color_for_segment(&self, seg_normalized: f32, is_lit: bool) -> Hsla {
        if !is_lit {
            return self.off_color;
        }
        if seg_normalized >= self.critical_threshold {
            self.critical_color
        } else if seg_normalized >= self.warning_threshold {
            self.warning_color
        } else {
            self.normal_color
        }
    }
}

impl Component for Meter {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Animate
        let delta = (self.target - self.value) * 0.2;
        if delta.abs() > 0.0001 {
            self.value += delta;
        }

        let v = self.normalized_value();
        let lit_segments = (v * self.segments as f32).ceil() as u32;

        if self.vertical {
            let seg_height = (bounds.size.height - self.gap * (self.segments - 1) as f32)
                / self.segments as f32;

            for i in 0..self.segments {
                let seg_norm = (i as f32 + 0.5) / self.segments as f32;
                let is_lit = i < lit_segments;
                let color = self.color_for_segment(seg_norm, is_lit);

                let seg_bounds = Bounds {
                    origin: Point {
                        x: bounds.origin.x,
                        y: bounds.origin.y
                            + bounds.size.height
                            - (i + 1) as f32 * (seg_height + self.gap)
                            + self.gap,
                    },
                    size: Size {
                        width: bounds.size.width,
                        height: seg_height,
                    },
                };
                cx.scene.draw_quad(Quad::new(seg_bounds).with_background(color));
            }
        } else {
            let seg_width = (bounds.size.width - self.gap * (self.segments - 1) as f32)
                / self.segments as f32;

            for i in 0..self.segments {
                let seg_norm = (i as f32 + 0.5) / self.segments as f32;
                let is_lit = i < lit_segments;
                let color = self.color_for_segment(seg_norm, is_lit);

                let seg_bounds = Bounds {
                    origin: Point {
                        x: bounds.origin.x + i as f32 * (seg_width + self.gap),
                        y: bounds.origin.y,
                    },
                    size: Size {
                        width: seg_width,
                        height: bounds.size.height,
                    },
                };
                cx.scene.draw_quad(Quad::new(seg_bounds).with_background(color));
            }
        }
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        if self.vertical {
            (Some(24.0), Some(80.0))
        } else {
            (Some(80.0), Some(24.0))
        }
    }
}

impl VizPrimitive for Meter {
    fn update(&mut self, value: f32) {
        self.target = value;
    }

    fn animate_to(&mut self, value: f32, _duration_ms: u32) {
        self.target = value;
    }
}

impl Fill for Meter {
    fn set_range(&mut self, min: f32, max: f32) {
        self.min = min;
        self.max = max;
    }

    fn set_thresholds(&mut self, warning: f32, critical: f32) {
        self.warning_threshold = warning;
        self.critical_threshold = critical;
    }
}
