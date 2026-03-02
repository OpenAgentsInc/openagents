use crate::kernel_math::{Point3, Vec3};

use super::types::{Point2D, ViewDirection};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewMatrix {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

impl ViewMatrix {
    pub fn from_view_direction(view_direction: ViewDirection) -> Self {
        let forward = normalize_or(view_direction.view_vector(), Vec3::y());
        let world_up = normalize_or(view_direction.up_vector(), Vec3::z());
        let mut right = world_up.cross(forward);
        if right.norm() <= 1e-10 {
            right = Vec3::x().cross(forward);
        }
        right = normalize_or(right, Vec3::x());
        let up = normalize_or(forward.cross(right), Vec3::z());

        Self { right, up, forward }
    }

    pub fn project(self, point: Point3) -> (Point2D, f64) {
        let vector = Vec3::new(point.x, point.y, point.z);
        (
            Point2D::new(vector.dot(self.right), vector.dot(self.up)),
            vector.dot(self.forward),
        )
    }

    pub fn project_point(self, point: Point3) -> Point2D {
        self.project(point).0
    }

    pub fn depth(self, point: Point3) -> f64 {
        self.project(point).1
    }
}

pub fn project_point(point: Point3, view_direction: ViewDirection) -> Point2D {
    ViewMatrix::from_view_direction(view_direction).project_point(point)
}

pub fn project_point_with_depth(point: Point3, view_direction: ViewDirection) -> (Point2D, f64) {
    ViewMatrix::from_view_direction(view_direction).project(point)
}

fn normalize_or(vector: Vec3, fallback: Vec3) -> Vec3 {
    vector.normalized().unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_projection_maps_height_to_y() {
        let projected = project_point(Point3::new(2.0, 3.0, 5.0), ViewDirection::Front);
        assert!((projected.y - 5.0).abs() < 1e-9);
    }

    #[test]
    fn isometric_projection_keeps_origin_stable() {
        let projected = project_point(Point3::origin(), ViewDirection::ISOMETRIC_STANDARD);
        assert!(projected.x.abs() < 1e-9);
        assert!(projected.y.abs() < 1e-9);
    }
}
