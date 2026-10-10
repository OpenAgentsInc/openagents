//! Low-poly meshes made in code, flat shaded.
//!
//! A [`Mesh`] is a triangle list: position, the face's normal, and a colour
//! per vertex. [`Lines`] is a line list: position and colour, drawn unlit
//! (the amber inlay on the floor, the edges of the vault).

use glam::{Mat3, Mat4, Quat, Vec3};
use std::f32::consts::TAU;

/// Floats per triangle vertex: position 3, normal 3, colour 3.
pub const STRIDE: usize = 9;
/// Floats per line vertex: position 3, colour 3.
pub const LINE_STRIDE: usize = 6;

/// A linear-light colour from `0xRRGGBB` (sRGB).
#[must_use]
pub fn rgb(hex: u32) -> [f32; 3] {
    let c = |v: u32| {
        let s = (v & 0xFF) as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    [c(hex >> 16), c(hex >> 8), c(hex)]
}

/// A shape in its own unit space.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// A cube from -0.5 to 0.5.
    Cube,
    /// A prism 1 tall (y from 0 to 1) with `sides`, bottom and top radii, and
    /// the first corner at angle `turn`; a top of 0 makes a pyramid.
    Prism {
        sides: u32,
        bottom: f32,
        top: f32,
        turn: f32,
    },
    /// A sphere of radius 0.5.
    Sphere { rings: u32, segments: u32 },
    /// A ring in the xy plane: `radius` to the tube's centre, `tube` thick.
    Torus {
        segments: u32,
        sides: u32,
        radius: f32,
        tube: f32,
    },
    /// An eight-faced shard: a squashed octahedron, 1 tall and 0.6 wide.
    Shard,
}

/// A triangle and a point it must face away from.
type Tri = ([Vec3; 3], Vec3);

fn triangles(shape: Shape) -> Vec<Tri> {
    let mut out = Vec::new();
    match shape {
        Shape::Cube => {
            for axis in 0..3 {
                for sign in [-0.5_f32, 0.5] {
                    let v = |u: f32, w: f32| {
                        let mut c = [0.0; 3];
                        c[axis] = sign;
                        c[(axis + 1) % 3] = u;
                        c[(axis + 2) % 3] = w;
                        Vec3::from_array(c)
                    };
                    let (a, b, c, d) = (v(-0.5, -0.5), v(0.5, -0.5), v(0.5, 0.5), v(-0.5, 0.5));
                    out.push(([a, b, c], Vec3::ZERO));
                    out.push(([a, c, d], Vec3::ZERO));
                }
            }
        }
        Shape::Prism {
            sides,
            bottom,
            top,
            turn,
        } => {
            let at = |r: f32, y: f32, i: u32| {
                let a = turn + i as f32 / sides as f32 * TAU;
                Vec3::new(r * a.cos(), y, r * a.sin())
            };
            let inside = Vec3::new(0.0, 0.3, 0.0);
            let apex = Vec3::Y;
            for i in 0..sides {
                let (b0, b1) = (at(bottom, 0.0, i), at(bottom, 0.0, i + 1));
                if top > 0.0 {
                    let (t0, t1) = (at(top, 1.0, i), at(top, 1.0, i + 1));
                    out.push(([b0, b1, t1], inside));
                    out.push(([b0, t1, t0], inside));
                    out.push(([apex, t0, t1], inside));
                } else {
                    out.push(([b0, b1, apex], inside));
                }
                out.push(([Vec3::ZERO, b1, b0], inside));
            }
        }
        Shape::Sphere { rings, segments } => {
            let point = |r: u32, s: u32| {
                let lat = r as f32 / rings as f32 * std::f32::consts::PI;
                let lon = s as f32 / segments as f32 * TAU;
                Vec3::new(lat.sin() * lon.cos(), lat.cos(), lat.sin() * lon.sin()) * 0.5
            };
            for r in 0..rings {
                for s in 0..segments {
                    let (a, b, c, d) = (
                        point(r, s),
                        point(r, s + 1),
                        point(r + 1, s + 1),
                        point(r + 1, s),
                    );
                    if r > 0 {
                        out.push(([a, b, c], Vec3::ZERO));
                    }
                    if r < rings - 1 {
                        out.push(([a, c, d], Vec3::ZERO));
                    }
                }
            }
        }
        Shape::Torus {
            segments,
            sides,
            radius,
            tube,
        } => {
            let centre = |i: u32| {
                let u = i as f32 / segments as f32 * TAU;
                Vec3::new(u.cos(), u.sin(), 0.0) * radius
            };
            let point = |i: u32, j: u32| {
                let u = i as f32 / segments as f32 * TAU;
                let v = j as f32 / sides as f32 * TAU;
                centre(i) + Vec3::new(u.cos() * v.cos(), u.sin() * v.cos(), v.sin()) * tube
            };
            for i in 0..segments {
                let inside = (centre(i) + centre(i + 1)) * 0.5;
                for j in 0..sides {
                    let (a, b, c, d) = (
                        point(i, j),
                        point(i + 1, j),
                        point(i + 1, j + 1),
                        point(i, j + 1),
                    );
                    out.push(([a, b, c], inside));
                    out.push(([a, c, d], inside));
                }
            }
        }
        Shape::Shard => {
            let top = Vec3::new(0.0, 0.5, 0.0);
            let bottom = Vec3::new(0.0, -0.5, 0.0);
            let ring: Vec<Vec3> = (0..4)
                .map(|i| {
                    let a = i as f32 / 4.0 * TAU;
                    Vec3::new(0.3 * a.cos(), 0.08, 0.3 * a.sin())
                })
                .collect();
            for i in 0..4 {
                let (a, b) = (ring[i], ring[(i + 1) % 4]);
                out.push(([a, b, top], Vec3::ZERO));
                out.push(([b, a, bottom], Vec3::ZERO));
            }
        }
    }
    out
}

/// A triangle list ready for the GPU.
#[derive(Clone, Debug, Default)]
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

    /// Adds `shape` placed by `transform`, in `colour` (linear).
    pub fn add(&mut self, shape: Shape, transform: Mat4, colour: [f32; 3]) {
        for (corners, inside) in triangles(shape) {
            let mut p = corners.map(|c| transform.transform_point3(c));
            let inside = transform.transform_point3(inside);
            let mut normal = (p[1] - p[0]).cross(p[2] - p[0]);
            if normal.length_squared() < 1e-12 {
                continue;
            }
            let centroid = (p[0] + p[1] + p[2]) / 3.0;
            if normal.dot(centroid - inside) < 0.0 {
                p.swap(1, 2);
                normal = -normal;
            }
            let n = normal.normalize();
            for v in p {
                self.data.extend_from_slice(&[
                    v.x, v.y, v.z, n.x, n.y, n.z, colour[0], colour[1], colour[2],
                ]);
            }
        }
    }

    /// Adds a box from `min` to `max`.
    pub fn block(&mut self, min: Vec3, max: Vec3, colour: [f32; 3]) {
        let transform =
            Mat4::from_scale_rotation_translation(max - min, Quat::IDENTITY, (min + max) * 0.5);
        self.add(Shape::Cube, transform, colour);
    }

    /// Adds a smooth-looking column: a square plinth, a round shaft and a
    /// square capital, standing on `base`, `height` tall.
    pub fn column(
        &mut self,
        base: Vec3,
        height: f32,
        radius: f32,
        stone: [f32; 3],
        band: [f32; 3],
    ) {
        let w = radius * 2.6;
        self.block(
            base + Vec3::new(-w / 2.0, 0.0, -w / 2.0),
            base + Vec3::new(w / 2.0, 0.22, w / 2.0),
            stone,
        );
        let shaft = height - 0.22 - 0.3;
        self.add(
            Shape::Prism {
                sides: 14,
                bottom: radius,
                top: radius * 0.92,
                turn: 0.0,
            },
            Mat4::from_translation(base + Vec3::Y * 0.22)
                * Mat4::from_scale(Vec3::new(1.0, shaft, 1.0)),
            stone,
        );
        // A bronze band below the capital.
        self.add(
            Shape::Prism {
                sides: 14,
                bottom: radius * 1.02,
                top: radius * 1.02,
                turn: 0.0,
            },
            Mat4::from_translation(base + Vec3::Y * (0.22 + shaft - 0.16))
                * Mat4::from_scale(Vec3::new(1.0, 0.12, 1.0)),
            band,
        );
        let cap = radius * 2.8;
        self.block(
            base + Vec3::new(-cap / 2.0, height - 0.3, -cap / 2.0),
            base + Vec3::new(cap / 2.0, height, cap / 2.0),
            stone,
        );
    }

    /// The mesh moved by `transform`, normals turned with it.
    #[must_use]
    pub fn transformed(&self, transform: Mat4) -> Self {
        let normals = Mat3::from_mat4(transform).inverse().transpose();
        let mut data = self.data.clone();
        for v in data.chunks_mut(STRIDE) {
            let p = transform.transform_point3(Vec3::new(v[0], v[1], v[2]));
            let n = (normals * Vec3::new(v[3], v[4], v[5])).normalize_or_zero();
            v[..6].copy_from_slice(&[p.x, p.y, p.z, n.x, n.y, n.z]);
        }
        Self { data }
    }
}

/// A line list ready for the GPU.
#[derive(Clone, Debug, Default)]
pub struct Lines {
    pub data: Vec<f32>,
}

impl Lines {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn vertices(&self) -> usize {
        self.data.len() / LINE_STRIDE
    }

    pub fn line(&mut self, a: Vec3, b: Vec3, colour: [f32; 3]) {
        for p in [a, b] {
            self.data
                .extend_from_slice(&[p.x, p.y, p.z, colour[0], colour[1], colour[2]]);
        }
    }

    /// A circle on the ground plane (y), `segments` straight pieces.
    pub fn circle(&mut self, centre: Vec3, radius: f32, segments: u32, colour: [f32; 3]) {
        let at = |i: u32| {
            let a = i as f32 / segments as f32 * TAU;
            centre + Vec3::new(radius * a.cos(), 0.0, radius * a.sin())
        };
        for i in 0..segments {
            self.line(at(i), at(i + 1), colour);
        }
    }

    /// The twelve edges of the box from `min` to `max`.
    pub fn box_edges(&mut self, min: Vec3, max: Vec3, colour: [f32; 3]) {
        let c = |x: bool, y: bool, z: bool| {
            Vec3::new(
                if x { max.x } else { min.x },
                if y { max.y } else { min.y },
                if z { max.z } else { min.z },
            )
        };
        for a in [false, true] {
            for b in [false, true] {
                self.line(c(false, a, b), c(true, a, b), colour);
                self.line(c(a, false, b), c(a, true, b), colour);
                self.line(c(a, b, false), c(a, b, true), colour);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(mesh: &Mesh) {
        assert!(mesh.vertices() > 0 && mesh.vertices().is_multiple_of(3));
        for tri in mesh.data.chunks(STRIDE * 3) {
            let p = |k: usize| Vec3::new(tri[k * STRIDE], tri[k * STRIDE + 1], tri[k * STRIDE + 2]);
            let n = Vec3::new(tri[3], tri[4], tri[5]);
            assert!((n.length() - 1.0).abs() < 1e-3, "{n}");
            // Counter-clockwise seen from the side the normal points to.
            assert!((p(1) - p(0)).cross(p(2) - p(0)).dot(n) > 0.0);
        }
    }

    #[test]
    fn shapes_face_outward() {
        for shape in [
            Shape::Cube,
            Shape::Prism {
                sides: 4,
                bottom: 0.5,
                top: 0.35,
                turn: 0.78,
            },
            Shape::Prism {
                sides: 4,
                bottom: 0.5,
                top: 0.0,
                turn: 0.0,
            },
            Shape::Sphere {
                rings: 6,
                segments: 10,
            },
            Shape::Torus {
                segments: 24,
                sides: 6,
                radius: 0.5,
                tube: 0.04,
            },
            Shape::Shard,
        ] {
            let mut mesh = Mesh::new();
            mesh.add(
                shape,
                Mat4::from_scale_rotation_translation(
                    Vec3::new(2.0, 0.5, 1.0),
                    Quat::from_rotation_y(0.4),
                    Vec3::new(3.0, 1.0, -2.0),
                ),
                [0.5; 3],
            );
            check(&mesh);
        }
    }

    #[test]
    fn a_box_faces_away_from_its_centre() {
        let mut mesh = Mesh::new();
        mesh.block(Vec3::new(1.0, 0.0, 1.0), Vec3::new(3.0, 2.0, 5.0), [0.2; 3]);
        assert_eq!(mesh.vertices(), 36);
        let centre = Vec3::new(2.0, 1.0, 3.0);
        for tri in mesh.data.chunks(STRIDE * 3) {
            let a = Vec3::new(tri[0], tri[1], tri[2]);
            let n = Vec3::new(tri[3], tri[4], tri[5]);
            assert!(n.dot(a - centre) > 0.0);
        }
        let moved = mesh.transformed(Mat4::from_rotation_y(1.0));
        check(&moved);
        let mut column = Mesh::new();
        column.column(Vec3::ZERO, 4.0, 0.3, [0.6; 3], [0.1; 3]);
        check(&column);
    }

    #[test]
    fn lines_come_in_pairs() {
        let mut lines = Lines::new();
        lines.box_edges(Vec3::ZERO, Vec3::ONE, [1.0; 3]);
        assert_eq!(lines.vertices(), 24);
        lines.circle(Vec3::ZERO, 2.0, 16, [1.0; 3]);
        assert_eq!(lines.vertices(), 56);
        assert_eq!(rgb(0xFFFFFF), [1.0; 3]);
        assert!(rgb(0x808080)[0] < 0.25);
    }
}
