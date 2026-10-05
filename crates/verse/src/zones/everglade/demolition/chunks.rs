//! Cuts a piece's kit meshes into chunks at load, as the destructible
//! buildings specification allows for phase D1: each cut region is split
//! by axis-aligned planes into a grid, every triangle is clipped to the
//! cells it crosses, and each cell's triangles become one chunk whose box
//! is the cell's host geometry bounds. Where a cut crosses the host model,
//! the chunk gets a cap over the cut in the material and texture of the
//! face it continues, darkened as a broken interior, so no chunk shows as a
//! bare box.

use super::kit::{Cut, Draft};
use super::site::Cuboid;
use crate::pbr::textured::{TexturedVertex, UNBAKED};
use crate::zones::everglade_pack::ZonePack;
use glam::{Mat4, Vec3};

/// Smallest chunk half extent, m, so a thin sliver still has a body.
const MIN_HALF: f32 = 0.04;
/// How close a vertex lies to a cut plane to be on it, m.
const ON_PLANE: f32 = 1e-4;
/// Gaps along a cut narrower than this are bridged by its cap, m.
const BRIDGE: f32 = 0.03;
/// A cap thinner than this across the section takes every material's
/// section instead of its main material's, m.
const THIN: f32 = 0.02;
/// How much darker a cap is than the face it continues.
const INTERIOR: f32 = 0.72;

/// One edge of a host triangle clipped onto a cut plane of its cell: the
/// plane's axis, whether it is the cell's upper face, and the material.
#[derive(Clone, Copy)]
struct Section {
    axis: usize,
    upper: bool,
    material: u16,
    ends: [TexturedVertex; 2],
}

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
    let mut sections: Vec<Vec<Section>> = vec![Vec::new(); cells];
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
                    let cell_index = (i * grid[1] + j) * grid[2] + k;
                    if *is_host || !host {
                        sections[cell_index]
                            .extend(on_planes(&polygon, index, grid, min, cell, *material));
                    }
                    let (chunk, lo, hi) = &mut chunks[cell_index];
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
    for ((chunk, ..), sections) in chunks.iter_mut().zip(&sections) {
        if !chunk.parts.is_empty() {
            cap(chunk, sections);
        }
    }
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

/// The edges of `polygon`, clipped into cell `index` of a `grid` of `cell`
/// sized cells from `min`, that lie on one of the cell's cut planes.
fn on_planes(
    polygon: &[TexturedVertex],
    index: [usize; 3],
    grid: [usize; 3],
    min: Vec3,
    cell: Vec3,
    material: u16,
) -> Vec<Section> {
    let mut out = Vec::new();
    for axis in (0..3).filter(|&a| grid[a] > 1) {
        let at = index[axis];
        let planes = [
            (at > 0).then(|| (false, min[axis] + cell[axis] * at as f32)),
            (at + 1 < grid[axis]).then(|| (true, min[axis] + cell[axis] * (at + 1) as f32)),
        ];
        for (upper, plane) in planes.into_iter().flatten() {
            let on = |v: &TexturedVertex| (v.pos[axis] - plane).abs() < ON_PLANE;
            for (e, a) in polygon.iter().enumerate() {
                let b = polygon[(e + 1) % polygon.len()];
                if on(a) && on(&b) {
                    out.push(Section {
                        axis,
                        upper,
                        material,
                        ends: [*a, b],
                    });
                }
            }
        }
    }
    out
}

/// Caps each cut face of `chunk` from the host's `sections` on it. Along
/// the cut's long direction the cap covers the sections' spans, bridging
/// narrow gaps but leaving a window's opening open; across it, the section
/// of the span's main material, whose face coordinates and color it takes,
/// darkened.
fn cap(chunk: &mut ChunkMesh, sections: &[Section]) {
    let mut faces: Vec<(usize, bool)> = sections.iter().map(|s| (s.axis, s.upper)).collect();
    faces.sort_unstable();
    faces.dedup();
    for (axis, upper) in faces {
        let on: Vec<&Section> = sections
            .iter()
            .filter(|s| s.axis == axis && s.upper == upper)
            .collect();
        let [b, c] = [(axis + 1) % 3, (axis + 2) % 3];
        let extent = |k: usize| {
            let (lo, hi) = bounds(on.iter().flat_map(|s| s.ends.iter().map(move |v| v.pos[k])));
            hi - lo
        };
        let (long, thin) = if extent(b) >= extent(c) {
            (b, c)
        } else {
            (c, b)
        };
        let span = |s: &Section| bounds(s.ends.iter().map(|v| v.pos[long]));
        let mut order = on.clone();
        order.sort_by(|x, y| span(x).0.total_cmp(&span(y).0));
        let mut runs: Vec<(f32, f32, Vec<&Section>)> = Vec::new();
        for s in order {
            let (lo, hi) = span(s);
            match runs.last_mut() {
                Some(run) if lo <= run.1 + BRIDGE => {
                    run.1 = run.1.max(hi);
                    run.2.push(s);
                }
                _ => runs.push((lo, hi, vec![s])),
            }
        }
        let mut normal = Vec3::ZERO;
        normal[axis] = if upper { 1.0 } else { -1.0 };
        for (lo, hi, members) in runs {
            if hi - lo < THIN {
                continue;
            }
            // The run's main material, by section length.
            let mut lengths: Vec<(u16, f32)> = Vec::new();
            for s in &members {
                let length = Vec3::from(s.ends[0].pos).distance(Vec3::from(s.ends[1].pos));
                match lengths.iter_mut().find(|(m, _)| *m == s.material) {
                    Some(entry) => entry.1 += length,
                    None => lengths.push((s.material, length)),
                }
            }
            let Some(&(material, _)) = lengths.iter().max_by(|x, y| x.1.total_cmp(&y.1)) else {
                continue;
            };
            let across = |only: Option<u16>| {
                bounds(
                    members
                        .iter()
                        .filter(|s| only.is_none_or(|m| s.material == m))
                        .flat_map(|s| s.ends.iter().map(|v| v.pos[thin])),
                )
            };
            let (mut t0, mut t1) = across(Some(material));
            if t1 - t0 < THIN {
                (t0, t1) = across(None);
            }
            if t1 - t0 < THIN * 0.5 {
                continue;
            }
            // The face's coordinates and color where the cap meets it at
            // each end of the run.
            let nearest = |target: f32| {
                members
                    .iter()
                    .filter(|s| s.material == material)
                    .flat_map(|s| s.ends.iter())
                    .min_by(|x, y| {
                        (x.pos[long] - target)
                            .abs()
                            .total_cmp(&(y.pos[long] - target).abs())
                    })
                    .copied()
            };
            let (Some(start), Some(end)) = (nearest(lo), nearest(hi)) else {
                continue;
            };
            let plane = members[0].ends[0].pos[axis];
            // Across the section the coordinates run at right angles to
            // their run along it, at the same scale: a planar mapping.
            let along = [end.uv[0] - start.uv[0], end.uv[1] - start.uv[1]];
            let across_uv = [-along[1], along[0]];
            let corner = |l: f32, t: f32, from: &TexturedVertex| {
                let mut pos = Vec3::ZERO;
                pos[axis] = plane;
                pos[long] = l;
                pos[thin] = t;
                let dark = |c: u8| (f32::from(c) * INTERIOR) as u8;
                let k = (t - t0) / (hi - lo);
                TexturedVertex {
                    pos: pos.to_array(),
                    normal: normal.to_array(),
                    uv: [from.uv[0] + across_uv[0] * k, from.uv[1] + across_uv[1] * k],
                    color: [
                        dark(from.color[0]),
                        dark(from.color[1]),
                        dark(from.color[2]),
                        from.color[3],
                    ],
                    light: UNBAKED,
                }
            };
            let quad = [
                corner(lo, t0, &start),
                corner(hi, t0, &end),
                corner(hi, t1, &end),
                corner(lo, t1, &start),
            ];
            let [p0, p1, p2] = [0, 1, 2].map(|i| Vec3::from(quad[i].pos));
            let triangles = if (p1 - p0).cross(p2 - p0).dot(normal) >= 0.0 {
                [quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]
            } else {
                [quad[0], quad[2], quad[1], quad[0], quad[3], quad[2]]
            };
            let part = match chunk.parts.iter().position(|(m, _)| *m == material) {
                Some(p) => p,
                None => {
                    chunk.parts.push((material, Vec::new()));
                    chunk.parts.len() - 1
                }
            };
            chunk.parts[part].1.extend(triangles);
        }
    }
}

/// The least and greatest of `values`.
fn bounds(values: impl Iterator<Item = f32>) -> (f32, f32) {
    values.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), x| {
        (lo.min(x), hi.max(x))
    })
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
    fn a_cut_through_a_wall_is_capped_in_its_material_around_a_window() {
        // A slab 0.2 m thick cut at x = 1, whose faces run from y = 0 to 1
        // and from 1.5 to 2.5 around a window.
        let section = |y0: f32, y1: f32, z: f32, material: u16| Section {
            axis: 0,
            upper: true,
            material,
            ends: [
                TexturedVertex::new(Vec3::new(1.0, y0, z), Vec3::Z, [0.0, y0]),
                TexturedVertex::new(Vec3::new(1.0, y1, z), Vec3::Z, [0.0, y1]),
            ],
        };
        let sections = [
            section(0.0, 1.0, 0.1, 3),
            section(0.0, 1.0, -0.1, 3),
            section(1.5, 2.5, 0.1, 3),
            section(1.5, 2.5, -0.1, 3),
            // A thin trim on the front face of the lower run.
            section(0.2, 0.4, 0.14, 5),
        ];
        let mut chunk = ChunkMesh::default();
        cap(&mut chunk, &sections);
        assert_eq!(chunk.parts.len(), 1, "the caps take the plaster");
        let (material, vertices) = &chunk.parts[0];
        assert_eq!(*material, 3);
        // Two quads: one each side of the window, none across it.
        assert_eq!(vertices.len(), 12);
        for v in vertices {
            assert!((v.pos[0] - 1.0).abs() < 1e-6);
            assert!(v.pos[1] <= 1.0 + 1e-6 || v.pos[1] >= 1.5 - 1e-6);
            assert!(v.pos[2].abs() <= 0.1 + 1e-6);
            assert_eq!(v.normal, [1.0, 0.0, 0.0]);
            assert!(v.color[0] < 255, "the broken face is darker");
        }
        // Each triangle faces out of the cut.
        for t in vertices.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|i| Vec3::from(t[i].pos));
            assert!((b - a).cross(c - a).x > 0.0);
        }
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
