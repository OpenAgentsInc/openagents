//! Carving: every model in Everglade's town that is not a kit building's
//! piece, the ground, or a plant (the generated buildings and landmarks,
//! the stalls, the furniture, the fences, and the kit pieces outside a kit
//! building) is cut on a lattice of blocks about [`CELL`] m across and
//! [`LEVEL`] m tall, in its model's frame. Each block is one piece of a
//! destructible building ([`super::site::Role::Block`]).
//!
//! The lattice decides three things, all from the pinned pack's triangles
//! at load, so every device cuts the same way:
//!
//! - The static scene draws each carved model with its triangles split on
//!   the lattice's planes ([`split`]), so a block's triangles can leave the
//!   merged cells on their own when it breaks.
//! - The model's collision is its triangles' [`Columns`], each span tagged
//!   with its block, so a character walks up a stage's steps and around a
//!   curved shell, and a broken block stops blocking.
//! - A block that breaks falls apart into the chunks [`cut_cell`] cuts from
//!   its triangles.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use glam::{Mat4, Vec3};
use verse_world::social::columns::Columns;

use super::chunks::{self, ChunkMesh, Triangle};
use super::kit::Cut;
use crate::pbr::textured::{TexturedVertex, UNBAKED};
use crate::zones::everglade::layout::{Placement, generated::patches};
use crate::zones::everglade_pack::{ZonePack, kit};

/// A block's width and depth at most, m.
pub const CELL: f32 = 3.5;
/// A block's height at most, m: about a story.
pub const LEVEL: f32 = 3.2;
/// A collision column's side, m: narrow enough that a 0.45 m character
/// can't stand inside a curved wall or clip a corner.
pub const COLUMN: f32 = 0.25;
/// Models cut finer, so a strike takes a crater out of their side and a
/// cut through one side leaves a hinge: the concrete tower, three blocks
/// across and a level every 2.5 m.
pub const FINE: [(&str, f32, f32); 1] = [("generated/concrete_tower", 2.1, 2.5)];

/// Models that stay as placed: the ground and its paving, plants, rocks,
/// and water. The footbridge, the jetties, and the rowboats break like the
/// other props.
const KEEP: [&str; 19] = [
    "nature/",
    "village/Floor_",
    "village/Prop_ExteriorBorder",
    "village/Prop_Vine",
    "generated/birch_low",
    "generated/oak_low",
    "generated/pine_low",
    "generated/spruce_low",
    "generated/poplar_low",
    "generated/fruit_tree",
    "generated/bush_round",
    "generated/flower_",
    "generated/wildflowers",
    "generated/mushrooms",
    "generated/lily_pads",
    "generated/reeds",
    "generated/mossy_rock",
    "generated/stump",
    "generated/fallen_log",
];

/// Whether a placement of `model` outside a kit building is carved.
#[must_use]
pub fn carvable(model: &str) -> bool {
    (model.starts_with("generated/")
        || model.starts_with("props/")
        || model.starts_with("village/")
        || model.starts_with("kit/"))
        && !KEEP.iter().any(|prefix| model.starts_with(prefix))
}

/// Which of `placements` are carved: every carvable model that is not a
/// piece of a kit building the town's rules map ([`super::town`]).
#[must_use]
pub fn carved(placements: &[Placement]) -> Vec<bool> {
    let members = super::town::kit_members(placements);
    placements
        .iter()
        .enumerate()
        .map(|(i, p)| !members.contains(&i) && carvable(p.model))
        .collect()
}

/// A model's blocks: `n` cells along x, y, and z from `min`, each `size`,
/// in the model's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lattice {
    pub min: Vec3,
    pub size: Vec3,
    pub n: [usize; 3],
}

impl Lattice {
    /// The lattice of a model with `bounds` placed at `scale`.
    #[must_use]
    pub fn of(bounds: ([f32; 3], [f32; 3]), scale: f32) -> Self {
        Self::sized(bounds, scale, CELL, LEVEL)
    }

    /// The lattice of model `name` with `bounds` placed at `scale`: finer
    /// for the models in [`FINE`].
    #[must_use]
    pub fn of_model(name: &str, bounds: ([f32; 3], [f32; 3]), scale: f32) -> Self {
        // A medieval kit piece is already one piece of its house: one block.
        if name.starts_with("kit/") {
            return Self::sized(bounds, scale, f32::MAX, f32::MAX);
        }
        match FINE.iter().find(|(model, ..)| *model == name) {
            Some(&(_, cell, level)) => Self::sized(bounds, scale, cell, level),
            None => Self::of(bounds, scale),
        }
    }

    fn sized(bounds: ([f32; 3], [f32; 3]), scale: f32, cell: f32, level: f32) -> Self {
        let min = Vec3::from(bounds.0);
        let extent = (Vec3::from(bounds.1) - min).max(Vec3::splat(1e-3));
        let count = |e: f32, side: f32| ((e * scale.abs() / side).ceil().max(1.0) as usize).min(64);
        let n = [
            count(extent.x, cell),
            count(extent.y, level),
            count(extent.z, cell),
        ];
        Self {
            min,
            size: extent / Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32),
            n,
        }
    }

    /// How many cells there are.
    #[must_use]
    pub fn count(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    /// The cell `p` lies in, clamped to the lattice.
    #[must_use]
    pub fn cell(&self, p: Vec3) -> [usize; 3] {
        std::array::from_fn(|a| {
            (((p[a] - self.min[a]) / self.size[a]).floor().max(0.0) as usize).min(self.n[a] - 1)
        })
    }

    /// The cell's number.
    #[must_use]
    pub fn id(&self, c: [usize; 3]) -> u32 {
        ((c[0] * self.n[1] + c[1]) * self.n[2] + c[2]) as u32
    }

    /// The cell numbered `id`.
    #[must_use]
    pub fn index(&self, id: u32) -> [usize; 3] {
        let id = id as usize;
        [
            id / (self.n[1] * self.n[2]),
            (id / self.n[2]) % self.n[1],
            id % self.n[2],
        ]
    }

    /// The cell's box in the model's frame.
    #[must_use]
    pub fn bounds(&self, c: [usize; 3]) -> (Vec3, Vec3) {
        let lo = self.min + self.size * Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32);
        (lo, lo + self.size)
    }

    /// The cell a triangle belongs to: the cell of its centroid.
    #[must_use]
    pub fn cell_of(&self, corners: &[TexturedVertex; 3]) -> u32 {
        let c = corners.iter().map(|v| Vec3::from(v.pos)).sum::<Vec3>() / 3.0;
        self.id(self.cell(c))
    }
}

/// `indices` over `vertices` with every triangle that crosses a plane of
/// `lattice` cut into one piece per cell it crosses; the new corners are
/// added to `vertices`. A triangle within one cell keeps its indices.
#[must_use]
pub fn split(vertices: &mut Vec<TexturedVertex>, indices: &[u32], lattice: &Lattice) -> Vec<u32> {
    if lattice.count() == 1 {
        return indices.to_vec();
    }
    let mut out = Vec::with_capacity(indices.len());
    for tri in indices.chunks_exact(3) {
        let corners = [0, 1, 2].map(|k| vertices[tri[k] as usize]);
        let cells = corners.map(|v| lattice.cell(Vec3::from(v.pos)));
        if cells[0] == cells[1] && cells[1] == cells[2] {
            out.extend_from_slice(tri);
            continue;
        }
        for polygon in pieces(&corners, lattice) {
            // A corner the cut kept keeps its index; only new corners are
            // added.
            let at: Vec<u32> = polygon
                .iter()
                .map(|v| match corners.iter().position(|c| c.pos == v.pos) {
                    Some(k) => tri[k],
                    None => {
                        vertices.push(*v);
                        vertices.len() as u32 - 1
                    }
                })
                .collect();
            for f in 1..at.len() - 1 {
                out.extend([at[0], at[f], at[f + 1]]);
            }
        }
    }
    out
}

/// The parts of a triangle in each cell of `lattice` it crosses. Outer
/// cells reach past the lattice, so nothing outside its bounds is lost.
fn pieces(corners: &[TexturedVertex; 3], lattice: &Lattice) -> Vec<Vec<TexturedVertex>> {
    let cells = corners.map(|v| lattice.cell(Vec3::from(v.pos)));
    let lo: [usize; 3] = std::array::from_fn(|a| cells.iter().map(|c| c[a]).min().unwrap_or(0));
    let hi: [usize; 3] = std::array::from_fn(|a| cells.iter().map(|c| c[a]).max().unwrap_or(0));
    let mut out = Vec::new();
    for i in lo[0]..=hi[0] {
        for j in lo[1]..=hi[1] {
            for k in lo[2]..=hi[2] {
                let at = [i, j, k];
                let mut polygon = corners.to_vec();
                for axis in 0..3 {
                    let (min, max) = lattice.bounds(at);
                    if at[axis] > 0 {
                        polygon = chunks::clip(&polygon, axis, min[axis], true);
                    }
                    if at[axis] + 1 < lattice.n[axis] && polygon.len() >= 3 {
                        polygon = chunks::clip(&polygon, axis, max[axis], false);
                    }
                    if polygon.len() < 3 {
                        break;
                    }
                }
                if polygon.len() >= 3 {
                    out.push(polygon);
                }
            }
        }
    }
    out
}

/// One carved model's triangles split on its lattice, in its frame, each
/// with its pack material and its cell.
pub struct Split {
    pub lattice: Lattice,
    pub triangles: Vec<([TexturedVertex; 3], u16, u32)>,
}

/// Pack model `name` at `scale`, split on its lattice, cut once and shared.
///
/// # Errors
///
/// Returns a message when the pack lacks the model.
pub fn split_model(pack: &ZonePack, name: &str, scale: f32) -> Result<Arc<Split>, String> {
    type Cache = Mutex<BTreeMap<(String, u32), Arc<Split>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    // A kit piece's split depends on whether the licensed kit or its proxy
    // draws it.
    let drawn = if name.starts_with("kit/") && kit::installed(pack) {
        format!("{name}@kit")
    } else {
        name.to_owned()
    };
    let key = (drawn, scale.to_bits());
    let cache = CACHE.get_or_init(Cache::default);
    if let Some(hit) = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
    {
        return Ok(hit.clone());
    }
    let model = pack
        .model(name)
        .ok_or_else(|| format!("The Everglade pack has no {name}"))?;
    let lattice = Lattice::of_model(name, model.bounds(), scale);

    let mut triangles = Vec::new();
    for primitive in &model.primitives {
        let mut vertices: Vec<TexturedVertex> = primitive
            .vertices
            .iter()
            .map(|v| TexturedVertex {
                pos: v.position,
                normal: v.normal,
                uv: v.uv,
                color: v.color,
                light: UNBAKED,
            })
            .collect();
        let indices = split(&mut vertices, &primitive.indices, &lattice);
        for tri in indices.chunks_exact(3) {
            let corners = [0, 1, 2].map(|k| vertices[tri[k] as usize]);
            let cell = lattice.cell_of(&corners);
            triangles.push((corners, primitive.material, cell));
        }
    }
    let split = Arc::new(Split { lattice, triangles });
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key, split.clone());
    Ok(split)
}

/// A carved placement's collision columns, each span's part its cell,
/// built once for each placement and shared.
///
/// # Errors
///
/// Returns a message when the pack lacks the model.
pub fn columns(pack: &ZonePack, placement: &Placement) -> Result<Option<Arc<Columns>>, String> {
    type Cache = Mutex<BTreeMap<(String, [u32; 16]), Option<Arc<Columns>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let transform = placement.transform();
    let key = (
        placement.model.to_owned(),
        transform.to_cols_array().map(f32::to_bits),
    );
    let cache = CACHE.get_or_init(Cache::default);
    if let Some(hit) = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
    {
        return Ok(hit.clone());
    }
    // A medieval kit piece collides as its committed proxy, with the
    // licensed kit or without it.
    let pack = if placement.model.starts_with("kit/") {
        kit::proxies()
    } else {
        pack
    };
    let split = split_model(pack, placement.model, placement.scale)?;
    let mut triangles: Vec<([Vec3; 3], u32)> = split
        .triangles
        .iter()
        .map(|(corners, _, cell)| {
            (
                corners.map(|v| transform.transform_point3(Vec3::from(v.pos))),
                *cell,
            )
        })
        .collect();
    // The solid boxes its triangles leave out, cut on the same lattice.
    for &[x0, x1, z0, z1, y0, y1] in patches(placement.model) {
        let mut vertices: Vec<TexturedVertex> = (0..8)
            .map(|k| TexturedVertex {
                pos: [
                    if k & 1 == 0 { x0 } else { x1 },
                    if k & 2 == 0 { y0 } else { y1 },
                    if k & 4 == 0 { z0 } else { z1 },
                ],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color: [255; 4],
                light: UNBAKED,
            })
            .collect();
        // Two triangles a face: bottom, top, and the four sides.
        let faces: [[u32; 4]; 6] = [
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        let indices: Vec<u32> = faces
            .iter()
            .flat_map(|&[a, b, c, d]| [a, b, c, a, c, d])
            .collect();
        let indices = self::split(&mut vertices, &indices, &split.lattice);
        for tri in indices.chunks_exact(3) {
            let corners = [0, 1, 2].map(|k| vertices[tri[k] as usize]);
            triangles.push((
                corners.map(|v| transform.transform_point3(Vec3::from(v.pos))),
                split.lattice.cell_of(&corners),
            ));
        }
    }
    let grid = Columns::rasterize(&triangles, COLUMN).map(Arc::new);
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key, grid.clone());
    Ok(grid)
}

/// The chunks of cell `cell` of a carved `placement` whose model `split`
/// holds, in the body frame `frame` (a pose without scale, the block's body
/// in the world): its triangles cut into up to eight chunks with a box
/// each, capped where a cut crosses them. A medieval kit piece may break
/// finer, into up to `shards` chunks along each side longer than 2.6 m.
#[must_use]
pub fn cut_cell(
    split: &Split,
    placement: &Placement,
    cell: u32,
    frame: Mat4,
    shards: usize,
) -> Vec<ChunkMesh> {
    cut_body(
        &CellBody::of(split, placement, cell, frame),
        placement,
        shards,
    )
}

/// A carved cell's triangles in its body's frame, snapped to [`SNAP`], so
/// pieces of one model placed alike (every wall section of a house) give
/// the same triangles bit for bit whatever their place and turn, and so
/// share one cut and its meshes ([`CellBody::key`], issue #10937).
pub struct CellBody {
    triangles: Vec<Triangle>,
}

/// The grid a cell's body-frame positions snap to, m: a tenth of a
/// millimeter, far under a pixel, and far over the rounding a placement's
/// turn and its body's opposite turn leave behind.
pub const SNAP: f32 = 1.0e-4;

impl CellBody {
    /// Cell `cell` of `placement`'s model `split`, in the body frame
    /// `frame`.
    #[must_use]
    pub fn of(split: &Split, placement: &Placement, cell: u32, frame: Mat4) -> Self {
        let to_body = frame.inverse() * placement.transform();
        let rotate = Mat4::from_quat(to_body.to_scale_rotation_translation().1);
        let snap = |v: f32, step: f32| (v / step).round() * step;
        let triangles = split
            .triangles
            .iter()
            .filter(|(_, _, c)| *c == cell)
            .map(|(corners, material, _)| Triangle {
                corners: corners.map(|v| {
                    let pos = to_body.transform_point3(Vec3::from(v.pos));
                    let normal = rotate
                        .transform_vector3(Vec3::from(v.normal))
                        .normalize_or_zero();
                    TexturedVertex {
                        pos: pos.to_array().map(|x| snap(x, SNAP)),
                        normal: normal.to_array().map(|x| snap(x, 1.0e-4)),
                        ..v
                    }
                }),
                host: true,
                material: *material,
            })
            .collect();
        Self { triangles }
    }

    /// What the cut depends on, bit for bit: two cells with the same key
    /// cut into the same chunks.
    #[must_use]
    pub fn key(&self) -> Vec<u32> {
        let mut key = Vec::with_capacity(self.triangles.len() * 40);
        for t in &self.triangles {
            key.push(u32::from(t.material));
            for v in &t.corners {
                key.extend(v.pos.iter().map(|x| x.to_bits()));
                key.extend(v.normal.iter().map(|x| x.to_bits()));
                key.extend(v.uv.iter().map(|x| x.to_bits()));
                key.push(u32::from_le_bytes(v.color));
                key.push(u32::from_le_bytes(v.light));
            }
        }
        key
    }
}

/// [`cut_cell`] of a cell already in its body's frame.
#[must_use]
pub fn cut_body(body: &CellBody, placement: &Placement, shards: usize) -> Vec<ChunkMesh> {
    let triangles = &body.triangles;
    if triangles.is_empty() {
        return Vec::new();
    }
    let (lo, hi) = triangles
        .iter()
        .flat_map(|t| t.corners.iter().map(|v| Vec3::from(v.pos)))
        .fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(p), hi.max(p)),
        );
    let extent = hi - lo;
    let fine = shards >= 3 && placement.model.starts_with("kit/");
    let grid = [0, 1, 2].map(|a| {
        if fine && extent[a] > 2.6 {
            3
        } else if extent[a] > 1.2 {
            2
        } else {
            1
        }
    });
    let region = Cut {
        side: None,
        frame: glam::Quat::IDENTITY,
        grid,
        thickness: None,
    };
    chunks::cut_region(triangles, &region, Vec3::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(p: [f32; 3]) -> TexturedVertex {
        TexturedVertex {
            pos: p,
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
            color: [255; 4],
            light: UNBAKED,
        }
    }

    #[test]
    fn a_lattice_cuts_a_building_into_story_high_blocks() {
        let lattice = Lattice::of(([-6.0, 0.0, -10.0], [6.0, 12.0, 0.0]), 1.0);
        assert_eq!(lattice.n, [4, 4, 3]);
        assert!(lattice.size.x <= CELL && lattice.size.y <= LEVEL);
        let c = lattice.cell(Vec3::new(5.9, 0.1, -0.1));
        assert_eq!(c, [3, 0, 2]);
        assert_eq!(lattice.index(lattice.id(c)), c);
        // A small prop is one block.
        assert_eq!(
            Lattice::of(([-1.0, 0.0, -1.0], [1.0, 2.0, 1.0]), 1.0).count(),
            1
        );
    }

    #[test]
    fn splitting_keeps_every_triangle_inside_one_cell_and_the_area_whole() {
        let lattice = Lattice::of(([0.0, 0.0, 0.0], [7.0, 6.4, 0.1]), 1.0);
        // One big wall triangle across all four cells.
        let mut vertices = vec![
            vertex([0.0, 0.0, 0.0]),
            vertex([7.0, 0.0, 0.0]),
            vertex([0.0, 6.4, 0.0]),
        ];
        let indices = split(&mut vertices, &[0, 1, 2], &lattice);
        assert!(indices.len() > 3);
        let area = |t: &[u32]| {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(vertices[t[k] as usize].pos));
            (b - a).cross(c - a).length() * 0.5
        };
        let total: f32 = indices.chunks_exact(3).map(area).sum();
        assert!((total - 7.0 * 6.4 * 0.5).abs() < 1e-3, "{total}");
        for t in indices.chunks_exact(3) {
            let cells: Vec<[usize; 3]> = t
                .iter()
                .map(|&i| {
                    // Nudged toward the centroid, so a corner on a plane
                    // falls in its own cell.
                    let c = t
                        .iter()
                        .map(|&k| Vec3::from(vertices[k as usize].pos))
                        .sum::<Vec3>()
                        / 3.0;
                    lattice.cell(Vec3::from(vertices[i as usize].pos).lerp(c, 0.01))
                })
                .collect();
            assert!(cells.windows(2).all(|w| w[0] == w[1]), "{cells:?}");
        }
    }

    #[test]
    fn the_ground_and_plants_are_never_carved_but_walkways_are() {
        assert!(carvable("generated/library"));
        assert!(carvable("generated/bandshell"));
        assert!(carvable("props/Workbench"));
        assert!(carvable("generated/market_stall"));
        assert!(!carvable("nature/CommonTree_1"));
        assert!(!carvable("village/Floor_Brick"));
        assert!(!carvable("generated/oak_low"));
        assert!(carvable("generated/footbridge"));
        assert!(carvable("generated/dock"));
        assert!(carvable("generated/rowboat"));
        assert!(!carvable("foliage/oak_forked"));
    }
}
