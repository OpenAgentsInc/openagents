//! Low-poly meshes made in code.
//!
//! A mesh is a plain triangle list. Each vertex carries its position, the
//! flat normal of its face (for the two-band fill), an outward "hull"
//! direction shared by every face at that corner (so the inverted-hull
//! outline has no gaps at corners), a colour, and an outline weight (0 for
//! things that never get a line, such as markings on the ground).

use glam::{Mat3, Mat4, Vec3};

/// Floats per vertex: position 3, normal 3, hull 3, colour 3, outline 1.
pub const STRIDE: usize = 13;

/// A colour from `0xRRGGBB`.
#[must_use]
pub fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

/// A shape in its own unit space, centred on the origin.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// A cube from -0.5 to 0.5.
    Cube,
    /// A frustum 1 tall along y, with bottom and top radii; a top radius of
    /// 0 makes a cone.
    Frustum { sides: u32, bottom: f32, top: f32 },
    /// A sphere of radius 0.5.
    Sphere { rings: u32, segments: u32 },
    /// A ring in the xy plane: `radius` to the tube's centre, `tube` thick.
    Torus {
        segments: u32,
        sides: u32,
        radius: f32,
        tube: f32,
    },
    /// A flat disc of radius 0.5 facing up.
    Disc { sides: u32 },
    /// A flat square from -0.5 to 0.5 facing up.
    Square,
}

/// One corner: its position and its hull direction.
type Corner = (Vec3, Vec3);

fn triangles(shape: Shape) -> Vec<[Corner; 3]> {
    fn quad(out: &mut Vec<[Corner; 3]>, a: Corner, b: Corner, c: Corner, d: Corner) {
        out.push([a, b, c]);
        out.push([a, c, d]);
    }
    let mut out = Vec::new();
    match shape {
        Shape::Cube => {
            let corner = |x: f32, y: f32, z: f32| {
                let p = Vec3::new(x, y, z) * 0.5;
                (p, p.normalize())
            };
            for axis in 0..3 {
                for sign in [-1.0_f32, 1.0] {
                    let v = |u: f32, w: f32| {
                        let mut c = [0.0; 3];
                        c[axis] = sign;
                        c[(axis + 1) % 3] = u;
                        c[(axis + 2) % 3] = w;
                        corner(c[0], c[1], c[2])
                    };
                    quad(
                        &mut out,
                        v(-1.0, -1.0),
                        v(1.0, -1.0),
                        v(1.0, 1.0),
                        v(-1.0, 1.0),
                    );
                }
            }
        }
        Shape::Frustum { sides, bottom, top } => {
            let ring = |r: f32, y: f32, i: u32| {
                let a = i as f32 / sides as f32 * std::f32::consts::TAU;
                let p = Vec3::new(r * a.cos(), y, r * a.sin());
                (p, Vec3::new(a.cos(), y * 1.2, a.sin()).normalize())
            };
            let up = (Vec3::new(0.0, 0.5, 0.0), Vec3::Y);
            let down = (Vec3::new(0.0, -0.5, 0.0), Vec3::NEG_Y);
            for i in 0..sides {
                let (b0, b1) = (ring(bottom, -0.5, i), ring(bottom, -0.5, i + 1));
                if top > 0.0 {
                    let (t0, t1) = (ring(top, 0.5, i), ring(top, 0.5, i + 1));
                    quad(&mut out, b0, b1, t1, t0);
                    out.push([up, t0, t1]);
                } else {
                    out.push([b0, b1, up]);
                }
                out.push([down, b1, b0]);
            }
        }
        Shape::Sphere { rings, segments } => {
            let point = |ring: u32, seg: u32| {
                let lat = ring as f32 / rings as f32 * std::f32::consts::PI;
                let lon = seg as f32 / segments as f32 * std::f32::consts::TAU;
                let n = Vec3::new(lat.sin() * lon.cos(), lat.cos(), lat.sin() * lon.sin());
                (n * 0.5, n)
            };
            for r in 0..rings {
                for s in 0..segments {
                    let (a, b, c, d) = (
                        point(r, s),
                        point(r, s + 1),
                        point(r + 1, s + 1),
                        point(r + 1, s),
                    );
                    if r == 0 {
                        out.push([a, c, d]);
                    } else if r == rings - 1 {
                        out.push([a, b, c]);
                    } else {
                        quad(&mut out, a, b, c, d);
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
            let point = |i: u32, j: u32| {
                let u = i as f32 / segments as f32 * std::f32::consts::TAU;
                let v = j as f32 / sides as f32 * std::f32::consts::TAU;
                let centre = Vec3::new(u.cos(), u.sin(), 0.0) * radius;
                let n = Vec3::new(u.cos() * v.cos(), u.sin() * v.cos(), v.sin());
                (centre + n * tube, n)
            };
            for i in 0..segments {
                for j in 0..sides {
                    quad(
                        &mut out,
                        point(i, j),
                        point(i + 1, j),
                        point(i + 1, j + 1),
                        point(i, j + 1),
                    );
                }
            }
        }
        Shape::Square => {
            let corner = |x: f32, z: f32| {
                let p = Vec3::new(x * 0.5, 0.0, z * 0.5);
                (p, (p.normalize() + Vec3::Y).normalize())
            };
            quad(
                &mut out,
                corner(-1.0, -1.0),
                corner(-1.0, 1.0),
                corner(1.0, 1.0),
                corner(1.0, -1.0),
            );
        }
        Shape::Disc { sides } => {
            let centre = (Vec3::ZERO, Vec3::Y);
            for i in 0..sides {
                let at = |i: u32| {
                    let a = i as f32 / sides as f32 * std::f32::consts::TAU;
                    let p = Vec3::new(0.5 * a.cos(), 0.0, 0.5 * a.sin());
                    (p, (p.normalize() + Vec3::Y).normalize())
                };
                out.push([centre, at(i), at(i + 1)]);
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

    /// Adds `shape` placed by `transform`, in `colour`, with an outline
    /// `weight` (1 is the normal line, 0 none).
    pub fn add(&mut self, shape: Shape, transform: Mat4, colour: u32, weight: f32) {
        let linear = Mat3::from_mat4(transform);
        let normals = linear.inverse().transpose();
        let colour = rgb(colour);
        for triangle in triangles(shape) {
            let mut corners = triangle.map(|(p, hull)| {
                (
                    transform.transform_point3(p),
                    (linear * hull).normalize_or_zero(),
                )
            });
            let mut normal = (corners[1].0 - corners[0].0).cross(corners[2].0 - corners[0].0);
            if normal.length_squared() < 1e-12 {
                continue;
            }
            // Face outward: the same side as the corners' hull directions.
            let outward: Vec3 = triangle.iter().map(|(_, hull)| normals * *hull).sum();
            if normal.dot(outward) < 0.0 {
                corners.swap(1, 2);
                normal = -normal;
            }
            let normal = normal.normalize();
            for (p, hull) in corners {
                self.data.extend_from_slice(&[
                    p.x, p.y, p.z, normal.x, normal.y, normal.z, hull.x, hull.y, hull.z, colour[0],
                    colour[1], colour[2], weight,
                ]);
            }
        }
    }

    /// Adds a box from `min` to `max`.
    pub fn block(&mut self, min: Vec3, max: Vec3, colour: u32, weight: f32) {
        let transform = Mat4::from_scale_rotation_translation(
            max - min,
            glam::Quat::IDENTITY,
            (min + max) * 0.5,
        );
        self.add(Shape::Cube, transform, colour, weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn faces_point_outward(mesh: &Mesh) {
        for v in mesh.data.chunks(STRIDE) {
            let p = Vec3::new(v[0], v[1], v[2]);
            let n = Vec3::new(v[3], v[4], v[5]);
            let hull = Vec3::new(v[6], v[7], v[8]);
            assert!((n.length() - 1.0).abs() < 1e-4, "{n}");
            assert!(hull.length() > 0.99, "{hull}");
            let _ = p;
        }
    }

    #[test]
    fn shapes_have_outward_unit_normals() {
        for shape in [
            Shape::Cube,
            Shape::Frustum {
                sides: 8,
                bottom: 0.5,
                top: 0.3,
            },
            Shape::Frustum {
                sides: 6,
                bottom: 0.5,
                top: 0.0,
            },
            Shape::Sphere {
                rings: 5,
                segments: 8,
            },
            Shape::Torus {
                segments: 10,
                sides: 4,
                radius: 0.4,
                tube: 0.05,
            },
            Shape::Disc { sides: 12 },
            Shape::Square,
        ] {
            let mut mesh = Mesh::new();
            mesh.add(
                shape,
                Mat4::from_scale(Vec3::new(2.0, 0.5, 1.0)),
                0xFF8800,
                1.0,
            );
            assert!(mesh.vertices() > 0 && mesh.vertices() % 3 == 0);
            faces_point_outward(&mesh);
        }
    }

    #[test]
    fn a_cube_faces_away_from_its_centre() {
        let mut mesh = Mesh::new();
        mesh.block(Vec3::new(1.0, 0.0, 1.0), Vec3::new(3.0, 2.0, 5.0), 0, 1.0);
        assert_eq!(mesh.vertices(), 36);
        let centre = Vec3::new(2.0, 1.0, 3.0);
        for tri in mesh.data.chunks(STRIDE * 3) {
            let a = Vec3::new(tri[0], tri[1], tri[2]);
            let n = Vec3::new(tri[3], tri[4], tri[5]);
            assert!(n.dot(a - centre) > 0.0);
            // Counter-clockwise seen from outside.
            let b = Vec3::new(tri[STRIDE], tri[STRIDE + 1], tri[STRIDE + 2]);
            let c = Vec3::new(tri[2 * STRIDE], tri[2 * STRIDE + 1], tri[2 * STRIDE + 2]);
            assert!((b - a).cross(c - a).dot(n) > 0.0);
        }
        assert_eq!(rgb(0xF28A1E), [242.0 / 255.0, 138.0 / 255.0, 30.0 / 255.0]);
    }
}
