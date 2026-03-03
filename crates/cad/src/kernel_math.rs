use serde::{Deserialize, Serialize};

use crate::measurement::CadMeasurePoint3;

pub type Scalar = f64;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point3 {
    pub x: Scalar,
    pub y: Scalar,
    pub z: Scalar,
}

impl Point3 {
    pub fn new(x: Scalar, y: Scalar, z: Scalar) -> Self {
        Self { x, y, z }
    }

    pub fn origin() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Vec3 {
    pub x: Scalar,
    pub y: Scalar,
    pub z: Scalar,
}

impl Vec3 {
    pub fn new(x: Scalar, y: Scalar, z: Scalar) -> Self {
        Self { x, y, z }
    }

    pub fn x() -> Self {
        Self::new(1.0, 0.0, 0.0)
    }

    pub fn y() -> Self {
        Self::new(0.0, 1.0, 0.0)
    }

    pub fn z() -> Self {
        Self::new(0.0, 0.0, 1.0)
    }

    pub fn dot(self, rhs: Self) -> Scalar {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    pub fn norm(self) -> Scalar {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Option<Self> {
        let n = self.norm();
        if n <= 1e-12 || !n.is_finite() {
            return None;
        }
        Some(Self::new(self.x / n, self.y / n, self.z / n))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Dir3 {
    x: Scalar,
    y: Scalar,
    z: Scalar,
}

impl Dir3 {
    pub fn new_normalize(v: Vec3) -> Self {
        let normalized = v.normalized().unwrap_or_else(Vec3::z);
        Self {
            x: normalized.x,
            y: normalized.y,
            z: normalized.z,
        }
    }

    pub fn into_inner(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point2 {
    pub x: Scalar,
    pub y: Scalar,
}

impl Point2 {
    pub fn new(x: Scalar, y: Scalar) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Vec2 {
    pub x: Scalar,
    pub y: Scalar,
}

impl Vec2 {
    pub fn new(x: Scalar, y: Scalar) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub matrix_row_major: [Scalar; 16],
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            matrix_row_major: identity_matrix(),
        }
    }

    pub fn translation(dx: Scalar, dy: Scalar, dz: Scalar) -> Self {
        Self {
            matrix_row_major: [
                1.0, 0.0, 0.0, dx, //
                0.0, 1.0, 0.0, dy, //
                0.0, 0.0, 1.0, dz, //
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    pub fn scale(sx: Scalar, sy: Scalar, sz: Scalar) -> Self {
        Self {
            matrix_row_major: [
                sx, 0.0, 0.0, 0.0, //
                0.0, sy, 0.0, 0.0, //
                0.0, 0.0, sz, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    pub fn rotation_x(angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self {
            matrix_row_major: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, c, -s, 0.0, //
                0.0, s, c, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    pub fn rotation_y(angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self {
            matrix_row_major: [
                c, 0.0, s, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                -s, 0.0, c, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    pub fn rotation_z(angle: Scalar) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self {
            matrix_row_major: [
                c, -s, 0.0, 0.0, //
                s, c, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    pub fn rotation_about_axis(axis: &Dir3, angle: Scalar) -> Self {
        let a = axis.into_inner();
        let (x, y, z) = (a.x, a.y, a.z);
        let c = angle.cos();
        let s = angle.sin();
        let t = 1.0 - c;
        Self {
            matrix_row_major: [
                t * x * x + c,
                t * x * y - s * z,
                t * x * z + s * y,
                0.0,
                t * x * y + s * z,
                t * y * y + c,
                t * y * z - s * x,
                0.0,
                t * x * z - s * y,
                t * y * z + s * x,
                t * z * z + c,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ],
        }
    }

    /// Compose `self` then `other` (self * other).
    pub fn then(&self, other: &Transform) -> Self {
        Self {
            matrix_row_major: mat_mul(&self.matrix_row_major, &other.matrix_row_major),
        }
    }

    pub fn apply_point(&self, p: &Point3) -> Point3 {
        let m = &self.matrix_row_major;
        Point3::new(
            m[0] * p.x + m[1] * p.y + m[2] * p.z + m[3],
            m[4] * p.x + m[5] * p.y + m[6] * p.z + m[7],
            m[8] * p.x + m[9] * p.y + m[10] * p.z + m[11],
        )
    }

    pub fn apply_vec(&self, v: &Vec3) -> Vec3 {
        let m = &self.matrix_row_major;
        Vec3::new(
            m[0] * v.x + m[1] * v.y + m[2] * v.z,
            m[4] * v.x + m[5] * v.y + m[6] * v.z,
            m[8] * v.x + m[9] * v.y + m[10] * v.z,
        )
    }

    pub fn apply_normal(&self, n: &Vec3) -> Vec3 {
        let Some(inv) = self.inverse() else {
            return *n;
        };
        let m = &inv.matrix_row_major;
        // inverse-transpose of upper-left 3x3 in row-major.
        Vec3::new(
            m[0] * n.x + m[4] * n.y + m[8] * n.z,
            m[1] * n.x + m[5] * n.y + m[9] * n.z,
            m[2] * n.x + m[6] * n.y + m[10] * n.z,
        )
    }

    pub fn inverse(&self) -> Option<Self> {
        inverse_matrix4(&self.matrix_row_major).map(|matrix_row_major| Self { matrix_row_major })
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

impl From<[Scalar; 16]> for Transform {
    fn from(matrix_row_major: [Scalar; 16]) -> Self {
        Self { matrix_row_major }
    }
}

impl From<Transform> for [Scalar; 16] {
    fn from(value: Transform) -> Self {
        value.matrix_row_major
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Tolerance {
    pub linear: Scalar,
    pub angular: Scalar,
}

impl Tolerance {
    pub const DEFAULT: Self = Self {
        linear: 1e-6,
        angular: 1e-9,
    };

    pub fn points_equal(&self, a: &Point3, b: &Point3) -> bool {
        (*a - *b).norm() < self.linear
    }

    pub fn is_zero(&self, d: Scalar) -> bool {
        d.abs() < self.linear
    }

    pub fn angles_equal(&self, a: Scalar, b: Scalar) -> bool {
        (a - b).abs() < self.angular
    }
}

impl Default for Tolerance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::ops::Sub for Point3 {
    type Output = Vec3;

    fn sub(self, rhs: Self) -> Self::Output {
        Vec3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Add<Vec3> for Point3 {
    type Output = Point3;

    fn add(self, rhs: Vec3) -> Self::Output {
        Point3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl From<CadMeasurePoint3> for Point3 {
    fn from(value: CadMeasurePoint3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<Point3> for CadMeasurePoint3 {
    fn from(value: Point3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<CadMeasurePoint3> for Vec3 {
    fn from(value: CadMeasurePoint3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<Vec3> for CadMeasurePoint3 {
    fn from(value: Vec3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

fn identity_matrix() -> [Scalar; 16] {
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat_mul(lhs: &[Scalar; 16], rhs: &[Scalar; 16]) -> [Scalar; 16] {
    let mut out = [0.0_f64; 16];
    for row in 0..4 {
        for col in 0..4 {
            out[row * 4 + col] = lhs[row * 4] * rhs[col]
                + lhs[row * 4 + 1] * rhs[4 + col]
                + lhs[row * 4 + 2] * rhs[8 + col]
                + lhs[row * 4 + 3] * rhs[12 + col];
        }
    }
    out
}

fn inverse_matrix4(m: &[Scalar; 16]) -> Option<[Scalar; 16]> {
    let mut inv = [0.0_f64; 16];

    inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
        + m[9] * m[7] * m[14]
        + m[13] * m[6] * m[11]
        - m[13] * m[7] * m[10];
    inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
        - m[8] * m[7] * m[14]
        - m[12] * m[6] * m[11]
        + m[12] * m[7] * m[10];
    inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
        + m[8] * m[7] * m[13]
        + m[12] * m[5] * m[11]
        - m[12] * m[7] * m[9];
    inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
        - m[8] * m[6] * m[13]
        - m[12] * m[5] * m[10]
        + m[12] * m[6] * m[9];
    inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
        - m[9] * m[3] * m[14]
        - m[13] * m[2] * m[11]
        + m[13] * m[3] * m[10];
    inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
        + m[8] * m[3] * m[14]
        + m[12] * m[2] * m[11]
        - m[12] * m[3] * m[10];
    inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
        - m[8] * m[3] * m[13]
        - m[12] * m[1] * m[11]
        + m[12] * m[3] * m[9];
    inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
        + m[8] * m[2] * m[13]
        + m[12] * m[1] * m[10]
        - m[12] * m[2] * m[9];
    inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
        + m[5] * m[3] * m[14]
        + m[13] * m[2] * m[7]
        - m[13] * m[3] * m[6];
    inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
        - m[4] * m[3] * m[14]
        - m[12] * m[2] * m[7]
        + m[12] * m[3] * m[6];
    inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
        + m[4] * m[3] * m[13]
        + m[12] * m[1] * m[7]
        - m[12] * m[3] * m[5];
    inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
        - m[4] * m[2] * m[13]
        - m[12] * m[1] * m[6]
        + m[12] * m[2] * m[5];
    inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
        - m[5] * m[3] * m[10]
        - m[9] * m[2] * m[7]
        + m[9] * m[3] * m[6];
    inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
        + m[4] * m[3] * m[10]
        + m[8] * m[2] * m[7]
        - m[8] * m[3] * m[6];
    inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
        - m[4] * m[3] * m[9]
        - m[8] * m[1] * m[7]
        + m[8] * m[3] * m[5];
    inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
        + m[4] * m[2] * m[9]
        + m[8] * m[1] * m[6]
        - m[8] * m[2] * m[5];

    let det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
    if det.abs() <= 1e-15 {
        return None;
    }
    let det_inv = 1.0 / det;
    for value in &mut inv {
        *value *= det_inv;
    }
    Some(inv)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::{CadMeasurePoint3, Dir3, Point3, Tolerance, Transform, Vec3};

    #[test]
    fn identity_transform_keeps_point_stable() {
        let t = Transform::identity();
        let p = Point3::new(1.0, 2.0, 3.0);
        let result = t.apply_point(&p);
        assert!((result - p).norm() < 1e-12);
    }

    #[test]
    fn translation_transform_matches_vcad_behavior() {
        let t = Transform::translation(10.0, 20.0, 30.0);
        let p = Point3::new(1.0, 2.0, 3.0);
        let result = t.apply_point(&p);
        assert!((result.x - 11.0).abs() < 1e-12);
        assert!((result.y - 22.0).abs() < 1e-12);
        assert!((result.z - 33.0).abs() < 1e-12);
    }

    #[test]
    fn rotation_z_90_maps_x_to_y() {
        let t = Transform::rotation_z(PI / 2.0);
        let p = Point3::new(1.0, 0.0, 0.0);
        let result = t.apply_point(&p);
        assert!(result.x.abs() < 1e-12);
        assert!((result.y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn compose_transform_matches_expected_order() {
        let t1 = Transform::translation(1.0, 0.0, 0.0);
        let t2 = Transform::scale(2.0, 2.0, 2.0);
        let composed = t2.then(&t1);
        let p = Point3::origin();
        let result = composed.apply_point(&p);
        assert!((result.x - 2.0).abs() < 1e-12);
    }

    #[test]
    fn inverse_round_trip_restores_point() {
        let t = Transform::translation(1.0, 2.0, 3.0).then(&Transform::rotation_x(PI / 4.0));
        let inv = t.inverse().expect("transform should be invertible");
        let composed = t.then(&inv);
        let p = Point3::new(5.0, 6.0, 7.0);
        let result = composed.apply_point(&p);
        assert!((result - p).norm() < 1e-9);
    }

    #[test]
    fn rotation_about_axis_matches_reference_case() {
        let axis = Dir3::new_normalize(Vec3::z());
        let t = Transform::rotation_about_axis(&axis, PI / 2.0);
        let p = Point3::new(1.0, 0.0, 0.0);
        let result = t.apply_point(&p);
        assert!(result.x.abs() < 1e-12);
        assert!((result.y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn tolerance_defaults_match_vcad() {
        let tol = Tolerance::default();
        assert!((tol.linear - 1e-6).abs() < 1e-18);
        assert!((tol.angular - 1e-9).abs() < 1e-18);
    }

    #[test]
    fn tolerance_points_equal_uses_linear_threshold() {
        let tol = Tolerance::DEFAULT;
        let a = Point3::new(1.0, 2.0, 3.0);
        let b = Point3::new(1.0 + 1e-7, 2.0, 3.0);
        assert!(tol.points_equal(&a, &b));
        let c = Point3::new(1.001, 2.0, 3.0);
        assert!(!tol.points_equal(&a, &c));
    }

    #[test]
    fn measurement_adapters_round_trip() {
        let source = CadMeasurePoint3::new(1.0, 2.0, 3.0);
        let point = Point3::from(source);
        let round_trip = CadMeasurePoint3::from(point);
        assert_eq!(source, round_trip);
    }
}
