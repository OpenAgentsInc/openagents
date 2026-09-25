//! Line and face geometry in one vertex format.
//!
//! The world is drawn twice: solid faces in the near-black field color,
//! which hide whatever stands behind them, and amber lines along the edges.
//! That hidden-line look is the whole style.

use bytemuck::{Pod, Zeroable};
use coder_terminal::Intensity;
use glam::{Mat4, Vec3};

use crate::palette;

/// One vertex: world position, linear color, and how much fog applies.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    /// World position in meters.
    pub pos: [f32; 3],
    /// Linear-light color.
    pub color: [f32; 3],
    /// 1.0 fades into the fog with distance; 0.0 ignores the fog.
    pub fog: f32,
}

/// A line list and a triangle list, built together.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    /// Pairs of vertices, one pair per line segment.
    pub lines: Vec<Vertex>,
    /// Triples of vertices, one triple per triangle.
    pub faces: Vec<Vertex>,
}

impl Mesh {
    /// Appends one amber segment.
    pub fn line(&mut self, a: Vec3, b: Vec3, step: Intensity) {
        self.line_with_fog(a, b, step, 1.0);
    }

    /// Appends one amber segment with an explicit fog weight.
    pub fn line_with_fog(&mut self, a: Vec3, b: Vec3, step: Intensity, fog: f32) {
        let color = palette::amber(step);
        self.lines.push(vertex(a, color, fog));
        self.lines.push(vertex(b, color, fog));
    }

    /// Appends a closed loop through `points`.
    pub fn polyline_loop(&mut self, points: &[Vec3], step: Intensity) {
        for (i, &a) in points.iter().enumerate() {
            let b = points[(i + 1) % points.len()];
            self.line(a, b, step);
        }
    }

    /// Appends one near-black quad, wound either way.
    pub fn quad(&mut self, corners: [Vec3; 4]) {
        let color = palette::field();
        let [a, b, c, d] = corners.map(|p| vertex(p, color, 1.0));
        self.faces.extend_from_slice(&[a, b, c, a, c, d]);
    }

    /// Appends a box: near-black faces with amber edges. `transform` maps
    /// the unit cube centered on the origin into the world.
    pub fn cube(&mut self, transform: Mat4, step: Intensity) {
        let corner = |i: usize| {
            let x = if i & 1 == 0 { -0.5 } else { 0.5 };
            let y = if i & 2 == 0 { -0.5 } else { 0.5 };
            let z = if i & 4 == 0 { -0.5 } else { 0.5 };
            transform.transform_point3(Vec3::new(x, y, z))
        };
        let c: [Vec3; 8] = std::array::from_fn(corner);
        for [a, b, d, e] in [
            [0, 1, 3, 2],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 3, 7, 5],
        ] {
            self.quad([c[a], c[b], c[d], c[e]]);
        }
        for (a, b) in [
            (0, 1),
            (2, 3),
            (4, 5),
            (6, 7),
            (0, 2),
            (1, 3),
            (4, 6),
            (5, 7),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ] {
            self.line(c[a], c[b], step);
        }
    }

    /// Appends a flat ring on the ground.
    pub fn ring(&mut self, center: Vec3, radius: f32, segments: usize, step: Intensity) {
        let points: Vec<Vec3> = (0..segments)
            .map(|i| {
                let a = i as f32 / segments as f32 * std::f32::consts::TAU;
                center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius)
            })
            .collect();
        self.polyline_loop(&points, step);
    }

    /// Appends everything in `other`.
    pub fn extend(&mut self, other: &Mesh) {
        self.lines.extend_from_slice(&other.lines);
        self.faces.extend_from_slice(&other.faces);
    }
}

fn vertex(p: Vec3, color: palette::Linear, fog: f32) -> Vertex {
    Vertex {
        pos: p.to_array(),
        color,
        fog,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cube_has_twelve_edges_and_twelve_triangles() {
        let mut mesh = Mesh::default();
        mesh.cube(Mat4::IDENTITY, Intensity::Full);
        assert_eq!(mesh.lines.len(), 24);
        assert_eq!(mesh.faces.len(), 36);
    }

    #[test]
    fn cube_edges_are_unit_length() {
        let mut mesh = Mesh::default();
        mesh.cube(Mat4::IDENTITY, Intensity::Full);
        for pair in mesh.lines.chunks(2) {
            let a = Vec3::from(pair[0].pos);
            let b = Vec3::from(pair[1].pos);
            assert!((a.distance(b) - 1.0).abs() < 1e-6);
        }
    }
}
