//! Drawing a [`physics::Cloth`] (#11658): its grid as triangles with
//! smooth normals, both sides, for flat-shaded meshes and the physically
//! lit pass.

use glam::{DVec3, Vec3};

use crate::pbr::LitVertex;

/// The cloth's particles between its last two steps (`alpha` in [0, 1]),
/// in scene units, shifted by `origin`.
#[must_use]
pub fn positions(cloth: &physics::Cloth, alpha: f64, origin: Vec3) -> Vec<Vec3> {
    cloth
        .interpolated(alpha)
        .map(|p: DVec3| origin + p.as_vec3())
        .collect()
}

/// Unit normals per particle: the area-weighted sum of its triangles'.
#[must_use]
pub fn normals(cloth: &physics::Cloth, points: &[Vec3]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; points.len()];
    for [a, b, c] in cloth.triangles() {
        let (a, b, c) = (a as usize, b as usize, c as usize);
        let n = (points[b] - points[a]).cross(points[c] - points[a]);
        for k in [a, b, c] {
            normals[k] += n;
        }
    }
    normals
        .into_iter()
        .map(|n| n.normalize_or(Vec3::Y))
        .collect()
}

/// Every triangle of the cloth twice, once per side, each corner with its
/// smooth normal (flipped for the back): `(corners, normals)`.
#[must_use]
pub fn two_sided(cloth: &physics::Cloth, points: &[Vec3]) -> Vec<([Vec3; 3], [Vec3; 3])> {
    let normals = normals(cloth, points);
    let mut out = Vec::with_capacity(cloth.triangles().len() * 2);
    for [a, b, c] in cloth.triangles() {
        let (a, b, c) = (a as usize, b as usize, c as usize);
        out.push((
            [points[a], points[b], points[c]],
            [normals[a], normals[b], normals[c]],
        ));
        out.push((
            [points[a], points[c], points[b]],
            [-normals[a], -normals[c], -normals[b]],
        ));
    }
    out
}

/// The cloth for the physically lit pass, in linear `color`, rough.
#[must_use]
pub fn lit(cloth: &physics::Cloth, alpha: f64, origin: Vec3, color: [f32; 3]) -> Vec<LitVertex> {
    let points = positions(cloth, alpha, origin);
    let mut out = Vec::new();
    for (corners, normals) in two_sided(cloth, &points) {
        let tangent = (corners[1] - corners[0]).normalize_or(Vec3::X);
        for (p, n) in corners.into_iter().zip(normals) {
            out.push(LitVertex {
                pos: p.to_array(),
                normal: n.to_array(),
                tangent: tangent.to_array(),
                local: (p - origin).to_array(),
                color,
                params: [0.0, 0.85, 0.0, 1.0],
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_cloth_draws_both_sides_with_opposite_normals() {
        let cloth = physics::Cloth::grid(
            DVec3::ZERO,
            DVec3::X,
            DVec3::Z,
            4,
            3,
            0.1,
            physics::ClothSettings::default(),
        );
        let points = positions(&cloth, 1.0, Vec3::new(0.0, 1.0, 0.0));
        let tris = two_sided(&cloth, &points);
        assert_eq!(tris.len(), 2 * 2 * 3 * 2);
        for (corners, normals) in &tris {
            let face = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            // Each side's normals agree with its winding.
            assert!(normals.iter().all(|n| n.dot(face) > 0.0));
            assert!(corners.iter().all(|p| (p.y - 1.0).abs() < 1e-6));
        }
        assert_eq!(lit(&cloth, 1.0, Vec3::ZERO, [0.5; 3]).len(), tris.len() * 3);
    }
}
