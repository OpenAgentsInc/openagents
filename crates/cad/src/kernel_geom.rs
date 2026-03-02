use std::any::Any;
use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::kernel_math::{Dir3, Point2, Point3, Transform, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceKind {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    BSpline,
    Bilinear,
}

pub trait Surface: Send + Sync + std::fmt::Debug {
    fn evaluate(&self, uv: Point2) -> Point3;
    fn normal(&self, uv: Point2) -> Dir3;
    fn domain(&self) -> ((f64, f64), (f64, f64));
    fn surface_type(&self) -> SurfaceKind;
    fn clone_box(&self) -> Box<dyn Surface>;
    fn as_any(&self) -> &dyn Any;
    fn transform(&self, t: &Transform) -> Box<dyn Surface>;
}

impl Clone for Box<dyn Surface> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plane {
    pub origin: Point3,
    pub x_dir: Dir3,
    pub y_dir: Dir3,
    pub normal_dir: Dir3,
}

impl Plane {
    pub fn new(origin: Point3, x_dir: Vec3, y_dir: Vec3) -> Self {
        let x = Dir3::new_normalize(x_dir);
        let y = Dir3::new_normalize(y_dir);
        let n = Dir3::new_normalize(x_dir.cross(y_dir));
        Self {
            origin,
            x_dir: x,
            y_dir: y,
            normal_dir: n,
        }
    }

    pub fn from_normal(origin: Point3, normal: Vec3) -> Self {
        let n = Dir3::new_normalize(normal);
        let arbitrary = if n.into_inner().x.abs() < 0.9 {
            Vec3::x()
        } else {
            Vec3::y()
        };
        let x = Dir3::new_normalize(arbitrary.cross(n.into_inner()));
        let y = Dir3::new_normalize(n.into_inner().cross(x.into_inner()));
        Self {
            origin,
            x_dir: x,
            y_dir: y,
            normal_dir: n,
        }
    }

    pub fn xy() -> Self {
        Self::new(Point3::origin(), Vec3::x(), Vec3::y())
    }

    pub fn evaluate_plane(&self, uv: Point2) -> Point3 {
        self.origin
            + vec_add(
                vec_scale(self.x_dir.into_inner(), uv.x),
                vec_scale(self.y_dir.into_inner(), uv.y),
            )
    }

    pub fn project(&self, point: &Point3) -> Point2 {
        let delta = *point - self.origin;
        Point2::new(
            delta.dot(self.x_dir.into_inner()),
            delta.dot(self.y_dir.into_inner()),
        )
    }

    pub fn signed_distance(&self, point: &Point3) -> f64 {
        (*point - self.origin).dot(self.normal_dir.into_inner())
    }
}

impl Surface for Plane {
    fn evaluate(&self, uv: Point2) -> Point3 {
        self.evaluate_plane(uv)
    }

    fn normal(&self, _uv: Point2) -> Dir3 {
        self.normal_dir
    }

    fn domain(&self) -> ((f64, f64), (f64, f64)) {
        ((-1e10, 1e10), (-1e10, 1e10))
    }

    fn surface_type(&self) -> SurfaceKind {
        SurfaceKind::Plane
    }

    fn clone_box(&self) -> Box<dyn Surface> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn transform(&self, t: &Transform) -> Box<dyn Surface> {
        let origin = t.apply_point(&self.origin);
        let x_dir = t.apply_vec(&self.x_dir.into_inner());
        let y_dir = t.apply_vec(&self.y_dir.into_inner());
        Box::new(Plane::new(origin, x_dir, y_dir))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CylinderSurface {
    pub center: Point3,
    pub axis: Dir3,
    pub ref_dir: Dir3,
    pub radius: f64,
}

impl CylinderSurface {
    pub fn new(radius: f64) -> Self {
        Self {
            center: Point3::origin(),
            axis: Dir3::new_normalize(Vec3::z()),
            ref_dir: Dir3::new_normalize(Vec3::x()),
            radius,
        }
    }

    pub fn with_axis(center: Point3, axis: Vec3, radius: f64) -> Self {
        let axis_dir = Dir3::new_normalize(axis);
        let ref_dir = perpendicular_ref(axis_dir);
        Self {
            center,
            axis: axis_dir,
            ref_dir,
            radius,
        }
    }

    fn ortho_dir(&self) -> Vec3 {
        self.axis.into_inner().cross(self.ref_dir.into_inner())
    }

    fn radial_vec(&self, u: f64) -> Vec3 {
        let x = vec_scale(self.ref_dir.into_inner(), u.cos());
        let y = vec_scale(self.ortho_dir(), u.sin());
        vec_scale(vec_add(x, y), self.radius)
    }
}

impl Surface for CylinderSurface {
    fn evaluate(&self, uv: Point2) -> Point3 {
        self.center
            + vec_add(
                self.radial_vec(uv.x),
                vec_scale(self.axis.into_inner(), uv.y),
            )
    }

    fn normal(&self, uv: Point2) -> Dir3 {
        Dir3::new_normalize(self.radial_vec(uv.x))
    }

    fn domain(&self) -> ((f64, f64), (f64, f64)) {
        ((0.0, 2.0 * PI), (-1e10, 1e10))
    }

    fn surface_type(&self) -> SurfaceKind {
        SurfaceKind::Cylinder
    }

    fn clone_box(&self) -> Box<dyn Surface> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn transform(&self, t: &Transform) -> Box<dyn Surface> {
        Box::new(Self {
            center: t.apply_point(&self.center),
            axis: Dir3::new_normalize(t.apply_vec(&self.axis.into_inner())),
            ref_dir: Dir3::new_normalize(t.apply_vec(&self.ref_dir.into_inner())),
            radius: self.radius,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConeSurface {
    pub apex: Point3,
    pub axis: Dir3,
    pub ref_dir: Dir3,
    pub half_angle_rad: f64,
}

impl ConeSurface {
    pub fn new(apex: Point3, axis: Vec3, half_angle_rad: f64) -> Self {
        let axis = Dir3::new_normalize(axis);
        let ref_dir = perpendicular_ref(axis);
        Self {
            apex,
            axis,
            ref_dir,
            half_angle_rad,
        }
    }
}

impl Surface for ConeSurface {
    fn evaluate(&self, uv: Point2) -> Point3 {
        let v = uv.y.max(0.0);
        let radius = v * self.half_angle_rad.tan();
        let radial = vec_scale(
            vec_add(
                vec_scale(self.ref_dir.into_inner(), uv.x.cos()),
                vec_scale(
                    self.axis.into_inner().cross(self.ref_dir.into_inner()),
                    uv.x.sin(),
                ),
            ),
            radius,
        );
        self.apex + vec_add(vec_scale(self.axis.into_inner(), v), radial)
    }

    fn normal(&self, uv: Point2) -> Dir3 {
        let p = self.evaluate(uv);
        let axis_component = vec_scale(
            self.axis.into_inner(),
            (p - self.apex).dot(self.axis.into_inner()),
        );
        let radial = vec_add(p - self.apex, vec_scale(axis_component, -1.0));
        Dir3::new_normalize(radial)
    }

    fn domain(&self) -> ((f64, f64), (f64, f64)) {
        ((0.0, 2.0 * PI), (0.0, 1e10))
    }

    fn surface_type(&self) -> SurfaceKind {
        SurfaceKind::Cone
    }

    fn clone_box(&self) -> Box<dyn Surface> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn transform(&self, t: &Transform) -> Box<dyn Surface> {
        Box::new(Self {
            apex: t.apply_point(&self.apex),
            axis: Dir3::new_normalize(t.apply_vec(&self.axis.into_inner())),
            ref_dir: Dir3::new_normalize(t.apply_vec(&self.ref_dir.into_inner())),
            half_angle_rad: self.half_angle_rad,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SphereSurface {
    pub center: Point3,
    pub radius: f64,
}

impl SphereSurface {
    pub fn new(center: Point3, radius: f64) -> Self {
        Self { center, radius }
    }
}

impl Surface for SphereSurface {
    fn evaluate(&self, uv: Point2) -> Point3 {
        let u = uv.x;
        let v = uv.y;
        let cos_v = v.cos();
        let x = self.radius * cos_v * u.cos();
        let y = self.radius * cos_v * u.sin();
        let z = self.radius * v.sin();
        self.center + Vec3::new(x, y, z)
    }

    fn normal(&self, uv: Point2) -> Dir3 {
        Dir3::new_normalize(self.evaluate(uv) - self.center)
    }

    fn domain(&self) -> ((f64, f64), (f64, f64)) {
        ((0.0, 2.0 * PI), (-PI / 2.0, PI / 2.0))
    }

    fn surface_type(&self) -> SurfaceKind {
        SurfaceKind::Sphere
    }

    fn clone_box(&self) -> Box<dyn Surface> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn transform(&self, t: &Transform) -> Box<dyn Surface> {
        Box::new(Self {
            center: t.apply_point(&self.center),
            radius: self.radius,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorusSurface {
    pub center: Point3,
    pub axis: Dir3,
    pub ref_dir: Dir3,
    pub major_radius: f64,
    pub minor_radius: f64,
}

impl TorusSurface {
    pub fn new(center: Point3, axis: Vec3, major_radius: f64, minor_radius: f64) -> Self {
        let axis = Dir3::new_normalize(axis);
        let ref_dir = perpendicular_ref(axis);
        Self {
            center,
            axis,
            ref_dir,
            major_radius,
            minor_radius,
        }
    }
}

impl Surface for TorusSurface {
    fn evaluate(&self, uv: Point2) -> Point3 {
        let u = uv.x;
        let v = uv.y;
        let y_dir = self.axis.into_inner().cross(self.ref_dir.into_inner());
        let circle_center = self.center
            + vec_add(
                vec_scale(self.ref_dir.into_inner(), self.major_radius * u.cos()),
                vec_scale(y_dir, self.major_radius * u.sin()),
            );
        let radial_dir = vec_add(
            vec_scale(self.ref_dir.into_inner(), u.cos()),
            vec_scale(y_dir, u.sin()),
        );
        circle_center
            + vec_add(
                vec_scale(radial_dir, self.minor_radius * v.cos()),
                vec_scale(self.axis.into_inner(), self.minor_radius * v.sin()),
            )
    }

    fn normal(&self, uv: Point2) -> Dir3 {
        let u = uv.x;
        let y_dir = self.axis.into_inner().cross(self.ref_dir.into_inner());
        let circle_center = self.center
            + vec_add(
                vec_scale(self.ref_dir.into_inner(), self.major_radius * u.cos()),
                vec_scale(y_dir, self.major_radius * u.sin()),
            );
        Dir3::new_normalize(self.evaluate(uv) - circle_center)
    }

    fn domain(&self) -> ((f64, f64), (f64, f64)) {
        ((0.0, 2.0 * PI), (0.0, 2.0 * PI))
    }

    fn surface_type(&self) -> SurfaceKind {
        SurfaceKind::Torus
    }

    fn clone_box(&self) -> Box<dyn Surface> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn transform(&self, t: &Transform) -> Box<dyn Surface> {
        Box::new(Self {
            center: t.apply_point(&self.center),
            axis: Dir3::new_normalize(t.apply_vec(&self.axis.into_inner())),
            ref_dir: Dir3::new_normalize(t.apply_vec(&self.ref_dir.into_inner())),
            major_radius: self.major_radius,
            minor_radius: self.minor_radius,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BilinearSurface {
    pub p00: Point3,
    pub p10: Point3,
    pub p01: Point3,
    pub p11: Point3,
}

impl BilinearSurface {
    pub fn new(p00: Point3, p10: Point3, p01: Point3, p11: Point3) -> Self {
        Self { p00, p10, p01, p11 }
    }
}

impl Surface for BilinearSurface {
    fn evaluate(&self, uv: Point2) -> Point3 {
        let u = uv.x;
        let v = uv.y;
        let a = self.p00 + vec_scale(self.p10 - self.p00, u);
        let b = self.p01 + vec_scale(self.p11 - self.p01, u);
        a + vec_scale(b - a, v)
    }

    fn normal(&self, uv: Point2) -> Dir3 {
        let u = uv.x;
        let v = uv.y;
        let du = vec_add(
            vec_scale(self.p10 - self.p00, 1.0 - v),
            vec_scale(self.p11 - self.p01, v),
        );
        let dv = vec_add(
            vec_scale(self.p01 - self.p00, 1.0 - u),
            vec_scale(self.p11 - self.p10, u),
        );
        Dir3::new_normalize(du.cross(dv))
    }

    fn domain(&self) -> ((f64, f64), (f64, f64)) {
        ((0.0, 1.0), (0.0, 1.0))
    }

    fn surface_type(&self) -> SurfaceKind {
        SurfaceKind::Bilinear
    }

    fn clone_box(&self) -> Box<dyn Surface> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn transform(&self, t: &Transform) -> Box<dyn Surface> {
        Box::new(Self {
            p00: t.apply_point(&self.p00),
            p10: t.apply_point(&self.p10),
            p01: t.apply_point(&self.p01),
            p11: t.apply_point(&self.p11),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SurfaceRecord {
    Plane(Plane),
    Cylinder(CylinderSurface),
    Cone(ConeSurface),
    Sphere(SphereSurface),
    Torus(TorusSurface),
    Bilinear(BilinearSurface),
}

impl SurfaceRecord {
    pub fn kind(&self) -> SurfaceKind {
        match self {
            Self::Plane(_) => SurfaceKind::Plane,
            Self::Cylinder(_) => SurfaceKind::Cylinder,
            Self::Cone(_) => SurfaceKind::Cone,
            Self::Sphere(_) => SurfaceKind::Sphere,
            Self::Torus(_) => SurfaceKind::Torus,
            Self::Bilinear(_) => SurfaceKind::Bilinear,
        }
    }

    pub fn evaluate(&self, uv: Point2) -> Point3 {
        match self {
            Self::Plane(surface) => surface.evaluate(uv),
            Self::Cylinder(surface) => surface.evaluate(uv),
            Self::Cone(surface) => surface.evaluate(uv),
            Self::Sphere(surface) => surface.evaluate(uv),
            Self::Torus(surface) => surface.evaluate(uv),
            Self::Bilinear(surface) => surface.evaluate(uv),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GeometryStore {
    pub surfaces: Vec<SurfaceRecord>,
}

impl GeometryStore {
    pub fn add_surface(&mut self, surface: SurfaceRecord) -> usize {
        self.surfaces.push(surface);
        self.surfaces.len() - 1
    }

    pub fn get_surface(&self, index: usize) -> Option<&SurfaceRecord> {
        self.surfaces.get(index)
    }
}

fn vec_scale(v: Vec3, scalar: f64) -> Vec3 {
    Vec3::new(v.x * scalar, v.y * scalar, v.z * scalar)
}

fn vec_add(lhs: Vec3, rhs: Vec3) -> Vec3 {
    Vec3::new(lhs.x + rhs.x, lhs.y + rhs.y, lhs.z + rhs.z)
}

fn perpendicular_ref(axis: Dir3) -> Dir3 {
    let axis_vec = axis.into_inner();
    let arbitrary = if axis_vec.x.abs() < 0.9 {
        Vec3::x()
    } else {
        Vec3::y()
    };
    let projected = vec_add(arbitrary, vec_scale(axis_vec, -arbitrary.dot(axis_vec)));
    Dir3::new_normalize(projected)
}

#[cfg(test)]
mod tests {
    use super::{CylinderSurface, Plane, SphereSurface, Surface, SurfaceKind};
    use crate::kernel_math::{Point2, Point3, Vec3};

    #[test]
    fn plane_evaluate_and_project_are_consistent() {
        let plane = Plane::xy();
        let uv = Point2::new(2.0, 3.0);
        let point = plane.evaluate(uv);
        assert_eq!(point, Point3::new(2.0, 3.0, 0.0));
        let projected = plane.project(&point);
        assert!((projected.x - uv.x).abs() < 1e-12);
        assert!((projected.y - uv.y).abs() < 1e-12);
    }

    #[test]
    fn cylinder_surface_kind_and_normal_are_stable() {
        let cylinder = CylinderSurface::with_axis(Point3::origin(), Vec3::z(), 5.0);
        assert_eq!(cylinder.surface_type(), SurfaceKind::Cylinder);
        let normal = cylinder.normal(Point2::new(0.0, 1.0));
        let n = normal.into_inner();
        assert!((n.x - 1.0).abs() < 1e-12);
        assert!(n.y.abs() < 1e-12);
        assert!(n.z.abs() < 1e-12);
    }

    #[test]
    fn sphere_surface_evaluate_matches_radius() {
        let sphere = SphereSurface::new(Point3::origin(), 10.0);
        let p = sphere.evaluate(Point2::new(0.0, 0.0));
        assert!((p.x - 10.0).abs() < 1e-12);
        assert!(p.y.abs() < 1e-12);
        assert!(p.z.abs() < 1e-12);
    }
}
