//! Physically lit geometry builders for Lagrange 1.
//!
//! Every shape is built in object space and placed by a transform, so the
//! object-space position each vertex carries keeps procedural detail (the
//! crinkle of insulation foil) fixed to its part as the part moves.

use glam::{Mat4, Quat, Vec3};

use crate::pbr::{LitVertex, Material};

/// A material with optional overrides of its color and roughness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Surface {
    pub material: Material,
    pub color: Option<[f32; 3]>,
    pub roughness: Option<f32>,
}

impl From<Material> for Surface {
    fn from(material: Material) -> Self {
        Self {
            material,
            color: None,
            roughness: None,
        }
    }
}

impl Surface {
    pub fn tinted(material: Material, color: [f32; 3]) -> Self {
        Self {
            material,
            color: Some(color),
            roughness: None,
        }
    }

    pub(super) fn vertex(self, t: &Mat4, p: Vec3, n: Vec3, tangent: Vec3) -> LitVertex {
        let (color, metallic, roughness) = self.material.parameters();
        LitVertex {
            pos: t.transform_point3(p).to_array(),
            normal: t.transform_vector3(n).normalize_or_zero().to_array(),
            tangent: t.transform_vector3(tangent).normalize_or_zero().to_array(),
            local: p.to_array(),
            color: self.color.unwrap_or(color),
            params: [
                metallic,
                self.roughness.unwrap_or(roughness),
                self.material.code(),
                1.0,
            ],
        }
    }
}

/// One vertex of `s` placed by `t`.
pub(super) fn vertex(s: Surface, t: &Mat4, p: Vec3, n: Vec3, tangent: Vec3) -> LitVertex {
    s.vertex(t, p, n, tangent)
}

/// Appends a quad `q` (object space, wound either way) facing `normal`.
pub(super) fn quad(
    out: &mut Vec<LitVertex>,
    t: &Mat4,
    q: [Vec3; 4],
    normal: Vec3,
    tangent: Vec3,
    s: Surface,
) {
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(s.vertex(t, q[i], normal, tangent));
    }
}

/// A box of half extents `half` centered on the object origin. `faces` gives
/// the surface of the +X, −X, +Y, −Y, +Z, and −Z faces. Tangents follow the
/// box's long axis, so brushed metal streaks along a member.
pub(super) fn cuboid_faces(out: &mut Vec<LitVertex>, t: &Mat4, half: Vec3, faces: [Surface; 6]) {
    let c = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * half;
    let along = if half.z >= half.x && half.z >= half.y {
        Vec3::Z
    } else if half.x >= half.y {
        Vec3::X
    } else {
        Vec3::Y
    };
    let tangent_for = |n: Vec3| {
        let t = along - n * along.dot(n);
        if t.length_squared() > 1e-6 {
            t.normalize()
        } else {
            n.any_orthonormal_vector()
        }
    };
    let sides = [
        (
            Vec3::X,
            [
                c(1., -1., -1.),
                c(1., 1., -1.),
                c(1., 1., 1.),
                c(1., -1., 1.),
            ],
        ),
        (
            -Vec3::X,
            [
                c(-1., -1., -1.),
                c(-1., -1., 1.),
                c(-1., 1., 1.),
                c(-1., 1., -1.),
            ],
        ),
        (
            Vec3::Y,
            [
                c(-1., 1., -1.),
                c(-1., 1., 1.),
                c(1., 1., 1.),
                c(1., 1., -1.),
            ],
        ),
        (
            -Vec3::Y,
            [
                c(-1., -1., -1.),
                c(1., -1., -1.),
                c(1., -1., 1.),
                c(-1., -1., 1.),
            ],
        ),
        (
            Vec3::Z,
            [
                c(-1., -1., 1.),
                c(1., -1., 1.),
                c(1., 1., 1.),
                c(-1., 1., 1.),
            ],
        ),
        (
            -Vec3::Z,
            [
                c(-1., -1., -1.),
                c(-1., 1., -1.),
                c(1., 1., -1.),
                c(1., -1., -1.),
            ],
        ),
    ];
    for ((n, q), s) in sides.into_iter().zip(faces) {
        quad(out, t, q, n, tangent_for(n), s);
    }
}

pub(super) fn cuboid(out: &mut Vec<LitVertex>, t: &Mat4, half: Vec3, s: impl Into<Surface>) {
    let s = s.into();
    cuboid_faces(out, t, half, [s; 6]);
}

/// A box placed by center and rotation.
pub(super) fn block(
    out: &mut Vec<LitVertex>,
    center: Vec3,
    half: Vec3,
    rotation: Quat,
    s: impl Into<Surface>,
) {
    cuboid(
        out,
        &Mat4::from_rotation_translation(rotation, center),
        half,
        s,
    );
}

/// A square-section member from `a` to `b`.
pub(super) fn member(
    out: &mut Vec<LitVertex>,
    a: Vec3,
    b: Vec3,
    width: f32,
    s: impl Into<Surface>,
) {
    let d = b - a;
    let Some(dir) = d.try_normalize() else {
        return;
    };
    let rotation = Quat::from_rotation_arc(Vec3::Z, dir);
    block(
        out,
        (a + b) / 2.0,
        Vec3::new(width / 2.0, width / 2.0, d.length() / 2.0),
        rotation,
        s,
    );
}

/// A cone frustum along object +Z, centered, with smooth side normals.
#[allow(clippy::too_many_arguments)]
pub(super) fn frustum(
    out: &mut Vec<LitVertex>,
    t: &Mat4,
    r0: f32,
    r1: f32,
    length: f32,
    segments: usize,
    s: impl Into<Surface>,
    caps: bool,
) {
    let s = s.into();
    let (z0, z1) = (-length / 2.0, length / 2.0);
    // The side normal tilts by the cone's slope.
    let slope = (r0 - r1) / length;
    let ring = |i: usize| {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        Vec3::new(a.cos(), a.sin(), 0.0)
    };
    for i in 0..segments {
        let (d0, d1) = (ring(i), ring(i + 1));
        let n0 = (d0 + Vec3::Z * slope).normalize();
        let n1 = (d1 + Vec3::Z * slope).normalize();
        let p = [
            d0 * r0 + Vec3::Z * z0,
            d1 * r0 + Vec3::Z * z0,
            d1 * r1 + Vec3::Z * z1,
            d0 * r1 + Vec3::Z * z1,
        ];
        let n = [n0, n1, n1, n0];
        for k in [0, 1, 2, 0, 2, 3] {
            out.push(s.vertex(t, p[k], n[k], Vec3::Z));
        }
        if caps {
            for (z, r, normal, flip) in [(z0, r0, -Vec3::Z, true), (z1, r1, Vec3::Z, false)] {
                if r <= 0.0 {
                    continue;
                }
                let (a, b) = if flip { (d1, d0) } else { (d0, d1) };
                let center = Vec3::Z * z;
                for p in [center, a * r + center, b * r + center] {
                    out.push(s.vertex(t, p, normal, Vec3::X));
                }
            }
        }
    }
}

/// A lattice truss: four longerons, rungs, and diagonal braces.
pub(super) fn lattice(
    out: &mut Vec<LitVertex>,
    a: Vec3,
    b: Vec3,
    width: f32,
    s: impl Into<Surface>,
) {
    let s = s.into();
    let axis = (b - a).normalize();
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    let h = width / 2.0;
    let corners = [u * h + v * h, -u * h + v * h, -u * h - v * h, u * h - v * h];
    for c in corners {
        member(out, a + c, b + c, 0.1, s);
    }
    let bays = ((b - a).length() / width).max(1.0) as usize;
    for i in 0..bays {
        let p0 = a.lerp(b, i as f32 / bays as f32);
        let p1 = a.lerp(b, (i + 1) as f32 / bays as f32);
        for k in 0..4 {
            let (c0, c1) = (corners[k], corners[(k + 1) % 4]);
            member(out, p0 + c0, p1 + c1, 0.04, s);
            member(out, p0 + c0, p0 + c1, 0.05, s);
        }
    }
}

/// A band around a cylinder: a short, slightly larger cylinder.
pub(super) fn band(
    out: &mut Vec<LitVertex>,
    center: Vec3,
    axis: Vec3,
    radius: f32,
    width: f32,
    s: impl Into<Surface>,
) {
    let t =
        Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Z, axis.normalize()), center);
    frustum(out, &t, radius, radius, width, 32, s, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_have_outward_unit_normals_and_brushed_tangents() {
        let mut out = Vec::new();
        member(
            &mut out,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 4.0),
            0.2,
            Material::Aluminium,
        );
        assert_eq!(out.len(), 36);
        for v in &out {
            let n = Vec3::from(v.normal);
            let p = Vec3::from(v.pos) - Vec3::new(0.0, 0.0, 2.0);
            assert!((n.length() - 1.0).abs() < 1e-5);
            assert!(n.dot(p) > 0.0, "normal points inward at {p}");
            let t = Vec3::from(v.tangent);
            assert!(n.dot(t).abs() < 1e-5);
        }
        // Side faces brush along the member.
        let side = out.iter().find(|v| v.normal[0] > 0.9).unwrap();
        assert!(side.tangent[2].abs() > 0.99);
    }

    #[test]
    fn frustum_normals_face_out_and_caps_close_it() {
        let mut out = Vec::new();
        frustum(
            &mut out,
            &Mat4::IDENTITY,
            1.0,
            0.5,
            2.0,
            12,
            Material::WhitePaint,
            true,
        );
        assert_eq!(out.len(), 12 * 12);
        for v in &out {
            let n = Vec3::from(v.normal);
            let p = Vec3::from(v.pos);
            assert!(n.dot(p) > -1e-4, "{n} at {p}");
        }
    }
}
