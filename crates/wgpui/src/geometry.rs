//! Geometric primitives: Point, Size, Bounds, Edges.

use bytemuck::{Pod, Zeroable};
use std::ops::{Add, Mul, Sub};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    pub fn distance(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Point {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

impl Mul<f32> for Size {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.width * rhs, self.height * rhs)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Bounds {
    pub origin: Point,
    pub size: Size,
}

impl Bounds {
    pub const ZERO: Self = Self {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub fn from_corners(top_left: Point, bottom_right: Point) -> Self {
        Self {
            origin: top_left,
            size: Size::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y),
        }
    }

    pub fn x(&self) -> f32 {
        self.origin.x
    }

    pub fn y(&self) -> f32 {
        self.origin.y
    }

    pub fn width(&self) -> f32 {
        self.size.width
    }

    pub fn height(&self) -> f32 {
        self.size.height
    }

    pub fn min_x(&self) -> f32 {
        self.origin.x
    }

    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn min_y(&self) -> f32 {
        self.origin.y
    }

    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width / 2.0,
            self.origin.y + self.size.height / 2.0,
        )
    }

    pub fn top_left(&self) -> Point {
        self.origin
    }

    pub fn top_right(&self) -> Point {
        Point::new(self.max_x(), self.origin.y)
    }

    pub fn bottom_left(&self) -> Point {
        Point::new(self.origin.x, self.max_y())
    }

    pub fn bottom_right(&self) -> Point {
        Point::new(self.max_x(), self.max_y())
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.min_x()
            && point.x <= self.max_x()
            && point.y >= self.min_y()
            && point.y <= self.max_y()
    }

    pub fn intersects(&self, other: &Bounds) -> bool {
        self.min_x() < other.max_x()
            && self.max_x() > other.min_x()
            && self.min_y() < other.max_y()
            && self.max_y() > other.min_y()
    }

    pub fn intersection(&self, other: &Bounds) -> Option<Bounds> {
        let x1 = self.min_x().max(other.min_x());
        let y1 = self.min_y().max(other.min_y());
        let x2 = self.max_x().min(other.max_x());
        let y2 = self.max_y().min(other.max_y());

        if x1 < x2 && y1 < y2 {
            Some(Bounds::new(x1, y1, x2 - x1, y2 - y1))
        } else {
            None
        }
    }

    pub fn union(&self, other: &Bounds) -> Bounds {
        let x1 = self.min_x().min(other.min_x());
        let y1 = self.min_y().min(other.min_y());
        let x2 = self.max_x().max(other.max_x());
        let y2 = self.max_y().max(other.max_y());
        Bounds::new(x1, y1, x2 - x1, y2 - y1)
    }

    pub fn inset(&self, amount: f32) -> Bounds {
        Bounds::new(
            self.origin.x + amount,
            self.origin.y + amount,
            (self.size.width - 2.0 * amount).max(0.0),
            (self.size.height - 2.0 * amount).max(0.0),
        )
    }

    pub fn expand(&self, amount: f32) -> Bounds {
        self.inset(-amount)
    }

    pub fn offset(&self, dx: f32, dy: f32) -> Bounds {
        Bounds {
            origin: self.origin.offset(dx, dy),
            size: self.size,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.size.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub const fn uniform(value: f32) -> Self {
        Self::new(value, value, value, value)
    }

    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_operations() {
        let p1 = Point::new(1.0, 2.0);
        let p2 = Point::new(3.0, 4.0);

        let sum = p1 + p2;
        assert!((sum.x - 4.0).abs() < 0.001);
        assert!((sum.y - 6.0).abs() < 0.001);

        let diff = p2 - p1;
        assert!((diff.x - 2.0).abs() < 0.001);
        assert!((diff.y - 2.0).abs() < 0.001);

        let scaled = p1 * 2.0;
        assert!((scaled.x - 2.0).abs() < 0.001);
        assert!((scaled.y - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(3.0, 4.0);
        let dist = p1.distance(p2);
        assert!((dist - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_size_area() {
        let size = Size::new(10.0, 20.0);
        assert!((size.area() - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_size_is_empty() {
        assert!(Size::ZERO.is_empty());
        assert!(Size::new(0.0, 10.0).is_empty());
        assert!(Size::new(10.0, 0.0).is_empty());
        assert!(!Size::new(10.0, 10.0).is_empty());
    }

    #[test]
    fn test_bounds_accessors() {
        let bounds = Bounds::new(10.0, 20.0, 100.0, 50.0);

        assert!((bounds.x() - 10.0).abs() < 0.001);
        assert!((bounds.y() - 20.0).abs() < 0.001);
        assert!((bounds.width() - 100.0).abs() < 0.001);
        assert!((bounds.height() - 50.0).abs() < 0.001);
        assert!((bounds.min_x() - 10.0).abs() < 0.001);
        assert!((bounds.max_x() - 110.0).abs() < 0.001);
        assert!((bounds.min_y() - 20.0).abs() < 0.001);
        assert!((bounds.max_y() - 70.0).abs() < 0.001);
    }

    #[test]
    fn test_bounds_center() {
        let bounds = Bounds::new(0.0, 0.0, 100.0, 100.0);
        let center = bounds.center();
        assert!((center.x - 50.0).abs() < 0.001);
        assert!((center.y - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_bounds_contains() {
        let bounds = Bounds::new(0.0, 0.0, 100.0, 100.0);

        assert!(bounds.contains(Point::new(50.0, 50.0)));
        assert!(bounds.contains(Point::new(0.0, 0.0)));
        assert!(bounds.contains(Point::new(100.0, 100.0)));
        assert!(!bounds.contains(Point::new(-1.0, 50.0)));
        assert!(!bounds.contains(Point::new(101.0, 50.0)));
    }

    #[test]
    fn test_bounds_intersects() {
        let b1 = Bounds::new(0.0, 0.0, 100.0, 100.0);
        let b2 = Bounds::new(50.0, 50.0, 100.0, 100.0);
        let b3 = Bounds::new(200.0, 200.0, 100.0, 100.0);

        assert!(b1.intersects(&b2));
        assert!(!b1.intersects(&b3));
    }

    #[test]
    fn test_bounds_intersection() {
        let b1 = Bounds::new(0.0, 0.0, 100.0, 100.0);
        let b2 = Bounds::new(50.0, 50.0, 100.0, 100.0);

        let intersection = b1.intersection(&b2).unwrap();
        assert!((intersection.x() - 50.0).abs() < 0.001);
        assert!((intersection.y() - 50.0).abs() < 0.001);
        assert!((intersection.width() - 50.0).abs() < 0.001);
        assert!((intersection.height() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_bounds_union() {
        let b1 = Bounds::new(0.0, 0.0, 50.0, 50.0);
        let b2 = Bounds::new(50.0, 50.0, 50.0, 50.0);

        let union = b1.union(&b2);
        assert!((union.x() - 0.0).abs() < 0.001);
        assert!((union.y() - 0.0).abs() < 0.001);
        assert!((union.width() - 100.0).abs() < 0.001);
        assert!((union.height() - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_bounds_inset_expand() {
        let bounds = Bounds::new(0.0, 0.0, 100.0, 100.0);

        let inset = bounds.inset(10.0);
        assert!((inset.x() - 10.0).abs() < 0.001);
        assert!((inset.y() - 10.0).abs() < 0.001);
        assert!((inset.width() - 80.0).abs() < 0.001);
        assert!((inset.height() - 80.0).abs() < 0.001);

        let expanded = bounds.expand(10.0);
        assert!((expanded.x() - -10.0).abs() < 0.001);
        assert!((expanded.y() - -10.0).abs() < 0.001);
        assert!((expanded.width() - 120.0).abs() < 0.001);
        assert!((expanded.height() - 120.0).abs() < 0.001);
    }

    #[test]
    fn test_edges() {
        let uniform = Edges::uniform(10.0);
        assert!((uniform.horizontal() - 20.0).abs() < 0.001);
        assert!((uniform.vertical() - 20.0).abs() < 0.001);

        let symmetric = Edges::symmetric(5.0, 10.0);
        assert!((symmetric.top - 5.0).abs() < 0.001);
        assert!((symmetric.right - 10.0).abs() < 0.001);
        assert!((symmetric.bottom - 5.0).abs() < 0.001);
        assert!((symmetric.left - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_from_corners() {
        let bounds = Bounds::from_corners(Point::new(10.0, 20.0), Point::new(110.0, 70.0));
        assert!((bounds.x() - 10.0).abs() < 0.001);
        assert!((bounds.y() - 20.0).abs() < 0.001);
        assert!((bounds.width() - 100.0).abs() < 0.001);
        assert!((bounds.height() - 50.0).abs() < 0.001);
    }
}
