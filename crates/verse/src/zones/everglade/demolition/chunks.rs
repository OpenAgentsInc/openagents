//! Cuts a piece's kit meshes into chunks at load, as the destructible
//! buildings specification allows for phase D1: each cut region is split
//! by axis-aligned planes into a grid, every triangle is clipped to the
//! cells it crosses, and each cell's triangles become one chunk whose box
//! is the cell's host geometry bounds. The cuts leave open edges; the
//! chunk's box, drawn just inside the faces, shows as the broken interior.

use super::cottage::{Cut, Draft};
use super::site::Cuboid;
use crate::pbr::textured::{TexturedVertex, UNBAKED};
use crate::zones::everglade_pack::ZonePack;
use glam::{Mat4, Vec3};

/// Smallest chunk half extent, m, so a thin sliver still has a body.
const MIN_HALF: f32 = 0.04;

/// One chunk: its box in the piece's body frame, and its triangles by pack
/// material, in the box's frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkMesh {
    pub cuboid: Option<Cuboid>,
    pub parts: Vec<(u16, Vec<TexturedVertex>)>,
}

/// A triangle in the host's model space, whether it is the host model's,
/// and its pack material.
struct Triangle {
    corners: [TexturedVertex; 3],
    host: bool,
    material: u16,
}

/// `draft`'s models from `pack`, cut into chunks.
///
/// # Errors
///
/// Returns a message when the pack lacks one of the piece's models.
pub fn cut(pack: &ZonePack, draft: &Draft) -> Result<Vec<ChunkMesh>, String> {
    let mut triangles = Vec::new();
    for (index, (name, transform)) in draft.models.iter().enumerate() {
        let model = pack
            .model(name)
            .ok_or_else(|| format!("The Everglade pack has no {name}"))?;
        for primitive in &model.primitives {
            let vertex = |i: u32| {
                let v = primitive.vertices[i as usize];
                TexturedVertex {
                    pos: transform
                        .transform_point3(Vec3::from(v.position))
                        .to_array(),
                    normal: transform
                        .transform_vector3(Vec3::from(v.normal))
                        .normalize_or_zero()
                        .to_array(),
                    uv: v.uv,
                    color: v.color,
                    light: UNBAKED,
                }
            };
            for tri in primitive.indices.chunks_exact(3) {
                triangles.push(Triangle {
                    corners: [vertex(tri[0]), vertex(tri[1]), vertex(tri[2])],
                    host: index == 0,
                    material: primitive.material,
                });
            }
        }
    }
    let origin = draft.origin;
    let mut chunks = Vec::new();
    for region in draft.cuts() {
        chunks.extend(cut_region(&triangles, &region, origin));
    }
    Ok(chunks)
}

fn cut_region(triangles: &[Triangle], region: &Cut, origin: Vec3) -> Vec<ChunkMesh> {
    let frame = Mat4::from_quat(region.frame);
    let inside = |t: &&Triangle| match region.side {
        None => true,
        Some(east) => {
            let x: f32 = t.corners.iter().map(|c| c.pos[0]).sum();
            (x > 0.0) == east
        }
    };
    let selected: Vec<&Triangle> = triangles.iter().filter(inside).collect();
    // Triangles in cut space.
    let to_cut = |v: &TexturedVertex| TexturedVertex {
        pos: frame.transform_point3(Vec3::from(v.pos)).to_array(),
        normal: frame.transform_vector3(Vec3::from(v.normal)).to_array(),
        ..*v
    };
    let cut: Vec<([TexturedVertex; 3], bool, u16)> = selected
        .iter()
        .map(|t| (t.corners.map(|c| to_cut(&c)), t.host, t.material))
        .collect();
    let host = cut.iter().any(|(_, h, _)| *h);
    let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for (corners, h, _) in &cut {
        if *h || !host {
            for c in corners {
                min = min.min(Vec3::from(c.pos));
                max = max.max(Vec3::from(c.pos));
            }
        }
    }
    if !min.is_finite() {
        return Vec::new();
    }
    let grid = region.grid;
    let cell = (max - min) / Vec3::new(grid[0] as f32, grid[1] as f32, grid[2] as f32);
    let cells = grid[0] * grid[1] * grid[2];
    let mut chunks: Vec<(ChunkMesh, Vec3, Vec3)> = (0..cells)
        .map(|_| {
            (
                ChunkMesh::default(),
                Vec3::splat(f32::INFINITY),
                Vec3::splat(f32::NEG_INFINITY),
            )
        })
        .collect();
    for (corners, is_host, material) in &cut {
        for i in 0..grid[0] {
            for j in 0..grid[1] {
                for k in 0..grid[2] {
                    let index = [i, j, k];
                    let mut polygon = corners.to_vec();
                    for axis in 0..3 {
                        let n = grid[axis];
                        if n < 2 {
                            continue;
                        }
                        let at = index[axis];
                        // Outer cells reach past the bounds, so dressing that
                        // overhangs the host lands in the edge cells.
                        if at > 0 {
                            let plane = min[axis] + cell[axis] * at as f32;
                            polygon = clip(&polygon, axis, plane, true);
                        }
                        if at + 1 < n {
                            let plane = min[axis] + cell[axis] * (at + 1) as f32;
                            polygon = clip(&polygon, axis, plane, false);
                        }
                        if polygon.len() < 3 {
                            break;
                        }
                    }
                    if polygon.len() < 3 {
                        continue;
                    }
                    let (chunk, lo, hi) = &mut chunks[(i * grid[1] + j) * grid[2] + k];
                    if *is_host || !host {
                        for v in &polygon {
                            *lo = lo.min(Vec3::from(v.pos));
                            *hi = hi.max(Vec3::from(v.pos));
                        }
                    }
                    let part = match chunk.parts.iter().position(|(m, _)| m == material) {
                        Some(p) => p,
                        None => {
                            chunk.parts.push((*material, Vec::new()));
                            chunk.parts.len() - 1
                        }
                    };
                    let out = &mut chunk.parts[part].1;
                    for f in 1..polygon.len() - 1 {
                        out.extend([polygon[0], polygon[f], polygon[f + 1]]);
                    }
                }
            }
        }
    }
    let back = region.frame.inverse();
    chunks
        .into_iter()
        .filter(|(chunk, ..)| !chunk.parts.is_empty())
        .map(|(mut chunk, mut lo, mut hi)| {
            if !lo.is_finite() {
                // Dressing alone: its own bounds.
                for v in chunk.parts.iter().flat_map(|(_, p)| p) {
                    lo = lo.min(Vec3::from(v.pos));
                    hi = hi.max(Vec3::from(v.pos));
                }
            }
            let mut center = (lo + hi) * 0.5;
            let mut half = ((hi - lo) * 0.5).max(Vec3::splat(MIN_HALF));
            if let Some(thickness) = region.thickness {
                let vertices: Vec<Vec3> = chunk
                    .parts
                    .iter()
                    .flat_map(|(_, p)| p.iter().map(|v| Vec3::from(v.pos)))
                    .collect();
                let mean = vertices.iter().copied().sum::<Vec3>() / vertices.len().max(1) as f32;
                for axis in (0..3).filter(|&a| grid[a] == 1) {
                    center[axis] = mean[axis];
                    half[axis] = half[axis].min(thickness);
                }
            }
            for v in chunk.parts.iter_mut().flat_map(|(_, p)| p.iter_mut()) {
                v.pos = (Vec3::from(v.pos) - center).to_array();
            }
            chunk.cuboid = Some(Cuboid {
                center: (back * center - origin).as_dvec3(),
                rotation: back.as_dquat(),
                half: half.as_dvec3(),
            });
            chunk
        })
        .collect()
}

/// Clips `polygon` to the side of the plane `pos[axis] = plane` that is
/// above it (`keep_above`) or below it, interpolating every attribute.
fn clip(
    polygon: &[TexturedVertex],
    axis: usize,
    plane: f32,
    keep_above: bool,
) -> Vec<TexturedVertex> {
    let side = |v: &TexturedVertex| {
        let d = v.pos[axis] - plane;
        if keep_above { d } else { -d }
    };
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for (i, a) in polygon.iter().enumerate() {
        let b = &polygon[(i + 1) % polygon.len()];
        let (da, db) = (side(a), side(b));
        if da >= 0.0 {
            out.push(*a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            let t = da / (da - db);
            out.push(lerp(a, b, t));
        }
    }
    out
}

fn lerp(a: &TexturedVertex, b: &TexturedVertex, t: f32) -> TexturedVertex {
    let mix3 = |x: [f32; 3], y: [f32; 3]| (Vec3::from(x).lerp(Vec3::from(y), t)).to_array();
    let color = std::array::from_fn(|i| {
        (f32::from(a.color[i]) + (f32::from(b.color[i]) - f32::from(a.color[i])) * t).round() as u8
    });
    TexturedVertex {
        pos: mix3(a.pos, b.pos),
        normal: Vec3::from(mix3(a.normal, b.normal))
            .normalize_or_zero()
            .to_array(),
        uv: [
            a.uv[0] + (b.uv[0] - a.uv[0]) * t,
            a.uv[1] + (b.uv[1] - a.uv[1]) * t,
        ],
        color,
        light: UNBAKED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(x: f32, y: f32) -> TexturedVertex {
        TexturedVertex::new(Vec3::new(x, y, 0.0), Vec3::Z, [x, y])
    }

    #[test]
    fn clipping_a_triangle_keeps_the_part_on_one_side_with_its_attributes() {
        let triangle = [vertex(0.0, 0.0), vertex(2.0, 0.0), vertex(0.0, 2.0)];
        let left = clip(&triangle, 0, 1.0, false);
        let right = clip(&triangle, 0, 1.0, true);
        assert!(left.iter().all(|v| v.pos[0] <= 1.0 + 1e-6));
        assert!(right.iter().all(|v| v.pos[0] >= 1.0 - 1e-6));
        assert_eq!(left.len(), 4);
        assert_eq!(right.len(), 3);
        // Coordinates interpolate with position.
        for v in left.iter().chain(&right) {
            assert!((v.uv[0] - v.pos[0]).abs() < 1e-6);
        }
    }
}
