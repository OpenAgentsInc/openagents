//! Geometry utilities for graph layout
//!
//! Ported from Unit framework's geometry package.
//! Provides vector math, shape calculations, and surface distance computations.

use std::f64::consts::PI;

const TWO_PI: f64 = 2.0 * PI;

/// 2D Point
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// Create a new point
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Create a zero point
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Calculate magnitude (length) of vector
    pub fn norm(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Normalize to unit vector
    pub fn normalize(&self) -> Self {
        let d = self.norm();
        if d == 0.0 {
            return random_unit_vector();
        }
        Self {
            x: self.x / d,
            y: self.y / d,
        }
    }

    /// Distance to another point
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Unit vector pointing to another point
    pub fn unit_vector_to(&self, other: &Point) -> Self {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let d = (dx * dx + dy * dy).sqrt();
        if d == 0.0 {
            return random_unit_vector();
        }
        Self { x: dx / d, y: dy / d }
    }

    /// Add another point
    pub fn add(&self, other: &Point) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    /// Subtract another point
    pub fn sub(&self, other: &Point) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    /// Scale by a factor
    pub fn scale(&self, factor: f64) -> Self {
        Self {
            x: self.x * factor,
            y: self.y * factor,
        }
    }
}

impl std::ops::Add for Point {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Point::add(&self, &other)
    }
}

impl std::ops::Sub for Point {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Point::sub(&self, &other)
    }
}

impl std::ops::Mul<f64> for Point {
    type Output = Self;
    fn mul(self, factor: f64) -> Self {
        self.scale(factor)
    }
}

/// Generate a random unit vector
pub fn random_unit_vector() -> Point {
    // Use a simple deterministic fallback for reproducibility
    // In practice, you'd use rand crate
    Point::new(0.707, 0.707).normalize()
}

/// Calculate unit vector from (x0, y0) to (x1, y1)
pub fn unit_vector(x0: f64, y0: f64, x1: f64, y1: f64) -> Point {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let d = (dx * dx + dy * dy).sqrt();
    if d == 0.0 {
        return random_unit_vector();
    }
    Point { x: dx / d, y: dy / d }
}

/// Calculate magnitude of vector (x, y)
pub fn norm(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

/// Calculate distance between two points
pub fn distance(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    norm(ax - bx, ay - by)
}

/// Shape type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    Circle { radius: f64 },
    Rect { width: f64, height: f64 },
}

impl Default for Shape {
    fn default() -> Self {
        Shape::Circle { radius: 20.0 }
    }
}

/// Thing: A positioned shape
#[derive(Debug, Clone, Copy)]
pub struct Thing {
    pub x: f64,
    pub y: f64,
    pub shape: Shape,
}

impl Thing {
    /// Create a new circle thing
    pub fn circle(x: f64, y: f64, radius: f64) -> Self {
        Self {
            x,
            y,
            shape: Shape::Circle { radius },
        }
    }

    /// Create a new rectangle thing
    pub fn rect(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            shape: Shape::Rect { width, height },
        }
    }

    /// Get position as Point
    pub fn position(&self) -> Point {
        Point { x: self.x, y: self.y }
    }
}

/// Surface distance result
#[derive(Debug, Clone, Copy)]
pub struct SurfaceDistanceResult {
    /// Gap distance (negative if overlapping)
    pub gap: f64,
    /// Center-to-center distance
    pub center_distance: f64,
    /// Unit vector from a to b
    pub direction: Point,
}

/// Calculate surface distance between two things
///
/// Returns the gap distance (how far apart their surfaces are),
/// the center distance, and the direction vector.
pub fn surface_distance(a: &Thing, b: &Thing) -> SurfaceDistanceResult {
    let d = distance(a.x, a.y, b.x, b.y);
    let u = unit_vector(a.x, a.y, b.x, b.y);

    let a_d = center_to_surface_distance(a, &u);
    let b_d = center_to_surface_distance(b, &u);

    let gap = d - (a_d + b_d);

    SurfaceDistanceResult {
        gap,
        center_distance: d,
        direction: u,
    }
}

/// Calculate distance from center to surface in direction u
pub fn center_to_surface_distance(thing: &Thing, u: &Point) -> f64 {
    match thing.shape {
        Shape::Circle { radius } => radius,
        Shape::Rect { width, height } => {
            let region = rectangle_region(thing.x, thing.y, width, height, u);
            match region {
                RectRegion::Left | RectRegion::Right => {
                    let tan = if u.y != 0.0 { u.x / u.y } else { f64::INFINITY };
                    norm(width / 2.0, width / 2.0 / tan.abs())
                }
                RectRegion::Top | RectRegion::Bottom => {
                    let tan = if u.y != 0.0 { u.x / u.y } else { 0.0 };
                    norm((height / 2.0) * tan.abs(), height / 2.0)
                }
            }
        }
    }
}

/// Find point on node surface in direction of unit vector
pub fn point_in_node(node: &Thing, u: &Point, padding: f64) -> Point {
    match node.shape {
        Shape::Circle { radius } => point_in_circle(node.x, node.y, radius, u, padding),
        Shape::Rect { width, height } => {
            point_in_rectangle(node.x, node.y, width, height, u, padding)
        }
    }
}

/// Find point on circle surface
pub fn point_in_circle(x: f64, y: f64, r: f64, u: &Point, padding: f64) -> Point {
    Point {
        x: x + u.x * (r - padding),
        y: y + u.y * (r - padding),
    }
}

/// Find point on rectangle surface
pub fn point_in_rectangle(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    u: &Point,
    padding: f64,
) -> Point {
    let region = rectangle_region(x, y, width, height, u);
    let a = u.y.atan2(u.x);

    match region {
        RectRegion::Left | RectRegion::Right => {
            let sx = u.x.signum();
            let tan = if u.y != 0.0 { u.x / u.y } else { f64::INFINITY };
            Point {
                x: x + sx * (width / 2.0) - padding * a.cos(),
                y: y + sx * (width / 2.0 / tan) - padding * a.sin(),
            }
        }
        RectRegion::Top | RectRegion::Bottom => {
            let sy = u.y.signum();
            let tan = if u.y != 0.0 { u.x / u.y } else { 0.0 };
            Point {
                x: x + sy * ((height / 2.0) * tan) - padding * a.cos(),
                y: y + sy * (height / 2.0) - padding * a.sin(),
            }
        }
    }
}

/// Rectangle region
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RectRegion {
    Left,
    Right,
    Top,
    Bottom,
}

/// Determine which region of rectangle the unit vector points to
pub fn rectangle_region(x: f64, y: f64, w: f64, h: f64, u: &Point) -> RectRegion {
    let a = unit_vector(x, y, x - w / 2.0, y - h / 2.0);
    let b = unit_vector(x, y, x + w / 2.0, y - h / 2.0);

    let alpha = rad_between(a.x, a.y, b.x, b.y);
    let beta = rad_between(u.x, u.y, b.x, b.y);

    if beta <= alpha {
        RectRegion::Top
    } else if beta <= PI {
        RectRegion::Left
    } else if beta <= PI + alpha {
        RectRegion::Bottom
    } else {
        RectRegion::Right
    }
}

/// Calculate radians between two vectors
pub fn rad_between(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let mut a = by.atan2(bx) - ay.atan2(ax);
    if a < 0.0 {
        a += TWO_PI;
    }
    a
}

// SVG Path utilities

/// Generate SVG path for a rectangle
pub fn describe_rect(x: f64, y: f64, width: f64, height: f64) -> String {
    format!(
        "M {} {} H {} V {} H {} Z",
        x,
        y,
        x + width,
        y + height,
        x
    )
}

/// Generate SVG path for a circle
pub fn describe_circle(x: f64, y: f64, r: f64) -> String {
    format!(
        "M {} {} A {} {} 0 1 0 {} {} z",
        x - r,
        y,
        r,
        r,
        x - r,
        y - 0.001
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_norm() {
        let p = Point::new(3.0, 4.0);
        assert!((p.norm() - 5.0).abs() < 0.0001);
    }

    #[test]
    fn test_point_normalize() {
        let p = Point::new(3.0, 4.0);
        let n = p.normalize();
        assert!((n.norm() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_unit_vector() {
        let u = unit_vector(0.0, 0.0, 3.0, 4.0);
        assert!((u.norm() - 1.0).abs() < 0.0001);
        assert!((u.x - 0.6).abs() < 0.0001);
        assert!((u.y - 0.8).abs() < 0.0001);
    }

    #[test]
    fn test_surface_distance_circles() {
        let a = Thing::circle(0.0, 0.0, 10.0);
        let b = Thing::circle(30.0, 0.0, 10.0);

        let result = surface_distance(&a, &b);

        assert!((result.center_distance - 30.0).abs() < 0.0001);
        assert!((result.gap - 10.0).abs() < 0.0001); // 30 - 10 - 10 = 10
    }

    #[test]
    fn test_point_in_circle() {
        let p = point_in_circle(0.0, 0.0, 10.0, &Point::new(1.0, 0.0), 0.0);
        assert!((p.x - 10.0).abs() < 0.0001);
        assert!((p.y - 0.0).abs() < 0.0001);
    }

    #[test]
    fn test_describe_rect() {
        let path = describe_rect(10.0, 20.0, 100.0, 50.0);
        assert!(path.contains("M 10 20"));
        assert!(path.contains("H 110"));
    }
}
