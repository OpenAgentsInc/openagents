//! A textured scene as the baker reads it: vertices to light and triangles
//! to trace, with a digest of everything a bake depends on.

use glam::Vec3;
use sha2::{Digest, Sha256};
use verse_pbr::pbr::textured::{TexturedScene, TexturedVertex};
use verse_pbr::pbr::textured_bake::{BakeGeometry, Emitter};

/// A triangle that occludes and reflects light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    /// World-space corners, m.
    pub corners: [Vec3; 3],
    /// The unit face normal, turned to the side the authored normals face.
    pub normal: Vec3,
    /// Diffuse albedo, linear.
    pub albedo: Vec3,
    /// Fraction of light the triangle stops, 0 to 1.
    pub opacity: f32,
    /// The scene vertices at its corners.
    pub vertices: [u32; 3],
}

impl Triangle {
    /// Barycentric weights of `p`, projected onto the triangle's plane and
    /// clamped inside it.
    #[must_use]
    pub fn weights(&self, p: Vec3) -> Vec3 {
        let [a, b, c] = self.corners;
        let (v0, v1, v2) = (b - a, c - a, p - a);
        let (d00, d01, d11) = (v0.dot(v0), v0.dot(v1), v1.dot(v1));
        let (d20, d21) = (v2.dot(v0), v2.dot(v1));
        let denom = d00 * d11 - d01 * d01;
        if denom.abs() <= f32::EPSILON {
            return Vec3::splat(1.0 / 3.0);
        }
        let v = (d11 * d20 - d01 * d21) / denom;
        let w = (d00 * d21 - d01 * d20) / denom;
        let weights = Vec3::new(1.0 - v - w, v, w).max(Vec3::ZERO);
        let sum = weights.x + weights.y + weights.z;
        if sum > 0.0 {
            weights / sum
        } else {
            Vec3::splat(1.0 / 3.0)
        }
    }
}

/// A scene ready to bake.
#[derive(Clone, Debug)]
pub struct Scene {
    /// The merged vertices, in [`TexturedScene::merge`]'s order.
    pub vertices: Vec<TexturedVertex>,
    /// Whether each vertex belongs to an alpha-tested material.
    pub foliage: Vec<bool>,
    /// Whether each vertex belongs to a far level of detail, which is lit
    /// but does not occlude.
    pub far: Vec<bool>,
    /// The triangles that occlude. Degenerate ones are dropped here, so an
    /// index means the same triangle to every backend.
    pub triangles: Vec<Triangle>,
    /// The triangles that give light, which the lamp layer gathers from.
    pub emitters: Vec<Emitter>,
    /// SHA-256 of the vertices, flags, and triangles: everything about the
    /// scene that a bake reads.
    pub digest: [u8; 32],
}

impl Scene {
    /// Prepares `scene`: merges it and samples each triangle's material as
    /// the load-time bake does.
    ///
    /// # Errors
    ///
    /// Returns the scene's validation error.
    pub fn new(scene: &TexturedScene) -> Result<Self, String> {
        let geometry = BakeGeometry::new(scene)?;
        let triangles = geometry
            .occluders
            .iter()
            .zip(&geometry.corners)
            .filter_map(|(o, &vertices)| {
                if !o.corners.iter().all(|c| c.is_finite()) {
                    return None;
                }
                let [a, b, c] = o.corners;
                // The same test, in the same arithmetic, as the hierarchy's.
                let face = (b - a).cross(c - a).try_normalize()?;
                let authored = o.normal.try_normalize().unwrap_or(face);
                let normal = if face.dot(authored) < 0.0 {
                    -face
                } else {
                    face
                };
                Some(Triangle {
                    corners: o.corners,
                    normal,
                    albedo: o.albedo,
                    opacity: o.opacity.clamp(0.0, 1.0),
                    vertices,
                })
            })
            .collect();
        let mut scene = Self {
            vertices: geometry.vertices,
            foliage: geometry.foliage,
            far: geometry.far,
            triangles,
            emitters: geometry.emitters,
            digest: [0; 32],
        };
        scene.digest = scene.digest();
        Ok(scene)
    }

    fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"openagents.verse-bake.scene.v1\0");
        hash.update((self.vertices.len() as u64).to_le_bytes());
        for (i, v) in self.vertices.iter().enumerate() {
            for x in v.pos.iter().chain(&v.normal) {
                hash.update(x.to_le_bytes());
            }
            hash.update([u8::from(self.foliage[i]), u8::from(self.far[i])]);
        }
        hash.update((self.triangles.len() as u64).to_le_bytes());
        for t in &self.triangles {
            for corner in t.corners {
                for x in corner.to_array() {
                    hash.update(x.to_le_bytes());
                }
            }
            for x in t.normal.to_array().iter().chain(&t.albedo.to_array()) {
                hash.update(x.to_le_bytes());
            }
            hash.update(t.opacity.to_le_bytes());
            for v in t.vertices {
                hash.update(v.to_le_bytes());
            }
        }
        hash.finalize().into()
    }

    /// The box around the vertices, or a unit box around the origin when
    /// there are none.
    #[must_use]
    pub fn bounds(&self) -> (Vec3, Vec3) {
        if self.vertices.is_empty() {
            return (Vec3::splat(-1.0), Vec3::ONE);
        }
        self.vertices.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), v| (min.min(v.pos.into()), max.max(v.pos.into())),
        )
    }
}

/// Lowercase hexadecimal of `bytes`.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
