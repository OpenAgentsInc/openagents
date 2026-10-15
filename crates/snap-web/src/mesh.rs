//! Low-poly meshes made in code: plain triangle lists with a flat normal
//! and a colour at each corner. The build plane is z = 0, x to the right
//! and y up, and the camera looks down the z axis at it, so a piece is a
//! flat outline pushed out a little toward and away from the camera.

use glam::{Vec2, Vec3};

/// Floats per vertex: position 3, normal 3, colour 3.
pub const STRIDE: usize = 9;

/// A colour from `0xRRGGBB`.
#[must_use]
pub fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

/// `a` mixed toward `b` by `t`.
#[must_use]
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// A triangle list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub data: Vec<f32>,
}

impl Mesh {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn vertices(&self) -> usize {
        self.data.len() / STRIDE
    }

    #[must_use]
    pub fn triangles(&self) -> usize {
        self.vertices() / 3
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Every colour in the mesh, one per vertex.
    #[must_use]
    pub fn colours(&self) -> Vec<[f32; 3]> {
        self.data
            .chunks(STRIDE)
            .map(|v| [v[6], v[7], v[8]])
            .collect()
    }

    /// A triangle, anticlockwise seen from its front, lit by its own
    /// normal.
    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, colour: [f32; 3]) {
        let normal = (b - a).cross(c - a).normalize_or_zero();
        for p in [a, b, c] {
            self.data.extend_from_slice(&[
                p.x, p.y, p.z, normal.x, normal.y, normal.z, colour[0], colour[1], colour[2],
            ]);
        }
    }

    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, colour: [f32; 3]) {
        self.tri(a, b, c, colour);
        self.tri(a, c, d, colour);
    }

    /// A convex outline (anticlockwise in x and y) pushed out from `z0`
    /// to `z1`: a front face in `front`, the sides in `side`.
    pub fn prism(&mut self, outline: &[Vec2], z0: f32, z1: f32, front: [f32; 3], side: [f32; 3]) {
        let n = outline.len();
        if n < 3 {
            return;
        }
        let at = |p: Vec2, z: f32| Vec3::new(p.x, p.y, z);
        for i in 1..n - 1 {
            self.tri(
                at(outline[0], z1),
                at(outline[i], z1),
                at(outline[i + 1], z1),
                front,
            );
            self.tri(
                at(outline[0], z0),
                at(outline[i + 1], z0),
                at(outline[i], z0),
                side,
            );
        }
        for i in 0..n {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            if (b - a).length_squared() < 1e-8 {
                continue;
            }
            self.quad(at(a, z0), at(b, z0), at(b, z1), at(a, z1), side);
        }
    }

    /// A flat rod from `a` to `b`, `width` wide, from `z0` to `z1`.
    pub fn rod(&mut self, a: Vec2, b: Vec2, width: f32, z0: f32, z1: f32, colour: [f32; 3]) {
        let along = b - a;
        if along.length_squared() < 1e-8 {
            return;
        }
        let side = along.normalize().perp() * (width / 2.0);
        let outline = [a - side, b - side, b + side, a + side];
        self.prism(&outline, z0, z1, colour, shade(colour, 0.8));
    }

    /// A regular polygon of `sides` around `centre`, radius `r`, pushed
    /// out from `z0` to `z1`, first corner at `start` radians.
    pub fn disc(
        &mut self,
        centre: Vec2,
        r: f32,
        sides: u32,
        start: f32,
        z0: f32,
        z1: f32,
        colour: [f32; 3],
    ) {
        let outline: Vec<Vec2> = (0..sides)
            .map(|i| {
                let a = start + i as f32 / sides as f32 * std::f32::consts::TAU;
                centre + Vec2::new(a.cos(), a.sin()) * r
            })
            .collect();
        self.prism(&outline, z0, z1, colour, shade(colour, 0.75));
    }

    /// A sphere of radius `r`.
    pub fn sphere(&mut self, centre: Vec3, r: f32, rings: u32, segments: u32, colour: [f32; 3]) {
        let point = |ring: u32, seg: u32| {
            let lat = ring as f32 / rings as f32 * std::f32::consts::PI;
            let lon = seg as f32 / segments as f32 * std::f32::consts::TAU;
            centre + Vec3::new(lat.sin() * lon.cos(), lat.cos(), lat.sin() * lon.sin()) * r
        };
        for ring in 0..rings {
            for seg in 0..segments {
                let (a, b, c, d) = (
                    point(ring, seg),
                    point(ring, seg + 1),
                    point(ring + 1, seg + 1),
                    point(ring + 1, seg),
                );
                if ring != 0 {
                    self.tri(a, b, c, colour);
                }
                if ring != rings - 1 {
                    self.tri(a, c, d, colour);
                }
            }
        }
    }

    /// A flat ring in the plane, from radius `inner` to `outer`.
    pub fn annulus(
        &mut self,
        centre: Vec2,
        inner: f32,
        outer: f32,
        segments: u32,
        z0: f32,
        z1: f32,
        colour: [f32; 3],
    ) {
        for i in 0..segments {
            let a0 = i as f32 / segments as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let (d0, d1) = (Vec2::new(a0.cos(), a0.sin()), Vec2::new(a1.cos(), a1.sin()));
            let outline = [
                centre + d0 * inner,
                centre + d0 * outer,
                centre + d1 * outer,
                centre + d1 * inner,
            ];
            self.prism(&outline, z0, z1, colour, shade(colour, 0.8));
        }
    }
}

/// A colour darkened (below 1) or lightened toward white (above 1).
#[must_use]
pub fn shade(colour: [f32; 3], k: f32) -> [f32; 3] {
    if k <= 1.0 {
        [colour[0] * k, colour[1] * k, colour[2] * k]
    } else {
        mix(colour, [1.0, 1.0, 1.0], (k - 1.0).min(1.0))
    }
}

/// The outline, turned to run anticlockwise if it ran clockwise.
#[must_use]
pub fn ccw(mut outline: Vec<Vec2>) -> Vec<Vec2> {
    let n = outline.len();
    let area: f32 = (0..n)
        .map(|i| outline[i].perp_dot(outline[(i + 1) % n]))
        .sum();
    if area < 0.0 {
        outline.reverse();
    }
    outline
}

/// An outline moved `inset` toward its middle.
#[must_use]
pub fn shrink(outline: &[Vec2], inset: f32) -> Vec<Vec2> {
    let n = outline.len().max(1) as f32;
    let centre = outline.iter().copied().sum::<Vec2>() / n;
    outline
        .iter()
        .map(|p| {
            let d = *p - centre;
            let len = d.length();
            if len <= inset {
                centre
            } else {
                centre + d * ((len - inset) / len)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prism_is_closed_and_faces_out() {
        let mut mesh = Mesh::new();
        let square = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        mesh.prism(&square, -0.1, 0.1, rgb(0xFF0000), rgb(0x800000));
        assert_eq!(mesh.triangles(), 2 + 2 + 8);
        // The front face points at the camera.
        assert!(mesh.data[5] > 0.99);
        assert_eq!(rgb(0x336699), [0.2, 0.4, 0.6]);
        let inner = shrink(&square, 0.1);
        assert!(inner[0].x > 0.0 && inner[2].x < 1.0);
    }

    #[test]
    fn every_closed_shape_faces_outward() {
        let mut mesh = Mesh::new();
        mesh.sphere(Vec3::ZERO, 1.0, 6, 8, rgb(0xFFFFFF));
        mesh.disc(Vec2::ZERO, 1.0, 6, 0.0, -0.5, 0.5, rgb(0xFFFFFF));
        mesh.rod(
            Vec2::new(-1.0, 0.2),
            Vec2::new(1.0, -0.3),
            0.2,
            -0.1,
            0.1,
            rgb(0xFFFFFF),
        );
        for tri in mesh.data.chunks(STRIDE * 3) {
            let centre = Vec3::new(
                (tri[0] + tri[9] + tri[18]) / 3.0,
                (tri[1] + tri[10] + tri[19]) / 3.0,
                (tri[2] + tri[11] + tri[20]) / 3.0,
            );
            let normal = Vec3::new(tri[3], tri[4], tri[5]);
            assert!(normal.dot(centre) > 0.0, "{centre:?} {normal:?}");
        }
    }
}
