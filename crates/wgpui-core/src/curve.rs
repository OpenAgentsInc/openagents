//! Bezier curve primitives for GPU rendering.
//!
//! This module provides cubic bezier curve support via CPU tessellation.
//! Curves are converted to line segments and rendered as thin rotated quads.

use crate::color::Hsla;
use crate::geometry::Point;

/// A cubic bezier curve primitive.
#[derive(Clone, Debug)]
pub struct CurvePrimitive {
    /// Control points: [start, control1, control2, end]
    pub points: [Point; 4],
    /// Stroke width in logical pixels
    pub stroke_width: f32,
    /// Stroke color
    pub color: Hsla,
}

/// A line segment (used after tessellation).
#[derive(Clone, Copy, Debug)]
pub struct LineSegment {
    pub start: Point,
    pub end: Point,
}

impl CurvePrimitive {
    /// Create a new cubic bezier curve.
    pub fn new(start: Point, control1: Point, control2: Point, end: Point) -> Self {
        Self {
            points: [start, control1, control2, end],
            stroke_width: 1.0,
            color: Hsla::white(),
        }
    }

    /// Create a quadratic bezier curve (converted to cubic).
    pub fn quadratic(start: Point, control: Point, end: Point) -> Self {
        // Convert quadratic to cubic: control points at 2/3 of the way
        let control1 = Point::new(
            start.x + (2.0 / 3.0) * (control.x - start.x),
            start.y + (2.0 / 3.0) * (control.y - start.y),
        );
        let control2 = Point::new(
            end.x + (2.0 / 3.0) * (control.x - end.x),
            end.y + (2.0 / 3.0) * (control.y - end.y),
        );
        Self::new(start, control1, control2, end)
    }

    /// Set stroke width.
    pub fn with_stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    /// Set stroke color.
    pub fn with_color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Evaluate the cubic bezier at parameter t (0.0 to 1.0).
    pub fn evaluate(&self, t: f32) -> Point {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        let [p0, p1, p2, p3] = self.points;

        Point::new(
            mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
            mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
        )
    }

    /// Tessellate the curve into line segments.
    ///
    /// # Arguments
    /// * `segments` - Number of line segments to generate (higher = smoother)
    pub fn tessellate(&self, segments: usize) -> Vec<LineSegment> {
        if segments == 0 {
            return vec![];
        }

        let mut result = Vec::with_capacity(segments);
        let mut prev = self.evaluate(0.0);

        for i in 1..=segments {
            let t = i as f32 / segments as f32;
            let curr = self.evaluate(t);
            result.push(LineSegment {
                start: prev,
                end: curr,
            });
            prev = curr;
        }

        result
    }

    /// Adaptive tessellation based on curve flatness.
    /// Produces fewer segments for straight-ish curves, more for curvy ones.
    pub fn tessellate_adaptive(&self, tolerance: f32) -> Vec<LineSegment> {
        let mut result = Vec::new();
        self.tessellate_recursive(&mut result, 0.0, 1.0, tolerance, 0);
        result
    }

    fn tessellate_recursive(
        &self,
        result: &mut Vec<LineSegment>,
        t0: f32,
        t1: f32,
        tolerance: f32,
        depth: usize,
    ) {
        const MAX_DEPTH: usize = 10;

        if depth >= MAX_DEPTH {
            result.push(LineSegment {
                start: self.evaluate(t0),
                end: self.evaluate(t1),
            });
            return;
        }

        let p0 = self.evaluate(t0);
        let p1 = self.evaluate(t1);
        let mid_t = (t0 + t1) / 2.0;
        let mid_curve = self.evaluate(mid_t);
        let mid_line = Point::new((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);

        // Check if the curve midpoint is close enough to the line midpoint
        let dx = mid_curve.x - mid_line.x;
        let dy = mid_curve.y - mid_line.y;
        let dist_sq = dx * dx + dy * dy;

        if dist_sq <= tolerance * tolerance {
            result.push(LineSegment { start: p0, end: p1 });
        } else {
            self.tessellate_recursive(result, t0, mid_t, tolerance, depth + 1);
            self.tessellate_recursive(result, mid_t, t1, tolerance, depth + 1);
        }
    }
}

impl LineSegment {
    /// Get the length of this segment.
    pub fn length(&self) -> f32 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Get the angle of this segment in radians.
    pub fn angle(&self) -> f32 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        dy.atan2(dx)
    }

    /// Get the midpoint of this segment.
    pub fn midpoint(&self) -> Point {
        Point::new(
            (self.start.x + self.end.x) / 2.0,
            (self.start.y + self.end.y) / 2.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_endpoints() {
        let curve = CurvePrimitive::new(
            Point::new(0.0, 0.0),
            Point::new(10.0, 20.0),
            Point::new(20.0, 20.0),
            Point::new(30.0, 0.0),
        );

        let start = curve.evaluate(0.0);
        let end = curve.evaluate(1.0);

        assert!((start.x - 0.0).abs() < 0.001);
        assert!((start.y - 0.0).abs() < 0.001);
        assert!((end.x - 30.0).abs() < 0.001);
        assert!((end.y - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_tessellate() {
        let curve = CurvePrimitive::new(
            Point::new(0.0, 0.0),
            Point::new(10.0, 20.0),
            Point::new(20.0, 20.0),
            Point::new(30.0, 0.0),
        );

        let segments = curve.tessellate(10);
        assert_eq!(segments.len(), 10);

        // First segment should start at curve start
        assert!((segments[0].start.x - 0.0).abs() < 0.001);
        assert!((segments[0].start.y - 0.0).abs() < 0.001);

        // Last segment should end at curve end
        let last = &segments[9];
        assert!((last.end.x - 30.0).abs() < 0.001);
        assert!((last.end.y - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_line_segment_length() {
        let seg = LineSegment {
            start: Point::new(0.0, 0.0),
            end: Point::new(3.0, 4.0),
        };
        assert!((seg.length() - 5.0).abs() < 0.001);
    }
}
