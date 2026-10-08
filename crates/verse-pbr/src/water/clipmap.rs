//! The ocean's geometry clipmap (`docs/verse/water.md`, phase W10): nested
//! square grids centered on the eye, each level twice as coarse as the one
//! inside it, so an unbounded sea draws with about as many triangles near
//! the eye as far from it, out to the horizon.
//!
//! The levels follow Losasso and Hoppe, "Geometry Clipmaps: Terrain
//! Rendering Using Nested Regular Grids" (SIGGRAPH 2004): each level is a
//! grid of `2 × half` cells a side whose center snaps to twice its own
//! spacing, so its vertices keep fixed places in the world and the waves
//! evaluated at them never swim. Level `l + 1` leaves a hole where level
//! `l` lies; the finer level sits zero or one coarse cell off the coarser
//! one's center, so each ring is uploaded with its hole in each of the
//! four places ([`Mesh::levels`]) and the frame draws the one that fits.
//! Toward its rim each level morphs into the next as in Losasso and Hoppe's
//! transition regions, with the vertex rule of Strugar's "Continuous
//! Distance-Dependent Level of Detail for Rendering Heightmaps" (*Journal
//! of Graphics, GPU, and Game Tools*, 2009): an odd vertex slides onto its
//! even neighbor as the distance from the eye grows, so at the rim a level
//! is exactly the coarser grid and the rings meet without cracks or
//! T-junctions. Past the last level, an apron of [`APRON_RINGS`] rings
//! stretches the last rim out to [`Spec::far`].
//!
//! The mesh holds grid coordinates only. The vertex shader (`vs_water` in
//! `photo.wgsl`) places each vertex from the frame's [`rows`], reads the
//! water's depth, shore distance, and current there from the streamed field
//! ([`super::field`]), and then moves it as it moves any sea vertex.
//! [`position`] is its CPU mirror.

use std::ops::Range;

use glam::{DVec2, Mat4, Vec2, Vec3};
use verse_engine::quality::Tier;

use super::frame::{Kind, WaterVertex};

/// The most levels a clipmap has (High's five rings).
pub const MAX_LEVELS: usize = 5;
/// The uniform rows a clipmap fills: one a level, then its shape.
pub const ROWS: usize = MAX_LEVELS + 1;
/// The kind channel of a clipmap vertex ([`super::Kind::code`] stops below
/// 5); `vs_water` draws it as the sea's [`Kind::Sea`] or [`Kind::Body`]
/// at full swell, as [`Ocean::sea`] says.
///
/// [`Ocean::sea`]: super::frame::Ocean::sea
pub const CODE: f32 = 6.0;
/// The share of a level's half width, from the eye, over which it morphs
/// into the next level.
pub const MORPH: f32 = 0.25;
/// Rings of the apron, each farther than the last by the same factor, so
/// the field is read at points between the last level's rim and the
/// apron's reach rather than only at its two ends.
pub const APRON_RINGS: u32 = 8;

/// A tier's clipmap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    /// Levels, the finest first: 3 on Low, 4 on Medium, 5 on High.
    pub levels: u32,
    /// The finest level's spacing, m.
    pub spacing: f32,
    /// Cells from a level's center to its rim, even.
    pub half: u32,
    /// How far from the center the apron reaches, m: past the fog.
    pub far: f32,
}

impl Spec {
    /// The clipmap a tier draws (`docs/verse/water.md`, the shaders per
    /// tier): Low 3 rings from 2 m spacing out to 256 m, Medium 4 from 1 m
    /// out to 320 m, High 5 from 0.5 m out to 384 m, each with an apron to
    /// past its fog's end.
    #[must_use]
    pub fn of(tier: Tier) -> Self {
        match tier {
            Tier::Low => Self {
                levels: 3,
                spacing: 2.0,
                half: 32,
                far: 2_000.0,
            },
            Tier::Medium => Self {
                levels: 4,
                spacing: 1.0,
                half: 40,
                far: 3_000.0,
            },
            Tier::High => Self {
                levels: 5,
                spacing: 0.5,
                half: 48,
                far: 4_000.0,
            },
        }
    }

    /// Level `level`'s spacing, m.
    #[must_use]
    pub fn level_spacing(&self, level: u32) -> f32 {
        self.spacing * (1u32 << level) as f32
    }

    /// Level `level`'s half width, m.
    #[must_use]
    pub fn extent(&self, level: u32) -> f32 {
        self.half as f32 * self.level_spacing(level)
    }

    /// Checks the shape the mesh and the shader assume.
    ///
    /// # Errors
    /// Names what is out of range.
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_LEVELS as u32).contains(&self.levels) {
            return Err(format!("A clipmap has 1 to {MAX_LEVELS} levels"));
        }
        if self.half < 4 || self.half % 2 != 0 || self.half > 256 {
            return Err("A clipmap level's half width is an even 4 to 256 cells".into());
        }
        if !(self.spacing.is_finite() && self.spacing > 0.0) {
            return Err("A clipmap's spacing is positive".into());
        }
        if !(self.far.is_finite() && self.far >= self.extent(self.levels - 1)) {
            return Err("A clipmap's apron reaches past its last level".into());
        }
        Ok(())
    }

    /// The triangles one frame draws.
    #[must_use]
    pub fn triangles(&self) -> u32 {
        let side = 2 * self.half;
        let ring = side * side - (self.half) * (self.half);
        2 * (side * side + (self.levels - 1) * ring + 4 * side * APRON_RINGS)
    }
}

/// Where each level's center lies for an eye at `eye` (x, z): its
/// spacing's double, below the eye, m. Computed in `f64` so the snap is
/// exact far from the origin.
#[must_use]
pub fn centers(spec: &Spec, eye: Vec2) -> [DVec2; MAX_LEVELS] {
    let mut out = [DVec2::ZERO; MAX_LEVELS];
    for (level, c) in out.iter_mut().enumerate().take(spec.levels as usize) {
        let step = 2.0 * f64::from(spec.level_spacing(level as u32));
        *c = (eye.as_dvec2() / step).floor() * step;
    }
    out
}

/// Which of its four holes level `level` (one or more) draws for an eye at
/// `eye`: 1 when the finer level sits a cell toward +x, plus 2 when toward
/// +z.
#[must_use]
pub fn variant(spec: &Spec, eye: Vec2, level: u32) -> usize {
    if level == 0 || level >= spec.levels {
        return 0;
    }
    let c = centers(spec, eye);
    let s = f64::from(spec.level_spacing(level));
    let k = (c[level as usize - 1] - c[level as usize]) / s;
    usize::from(k.x > 0.5) + 2 * usize::from(k.y > 0.5)
}

/// The rows `water.wgsl`'s `clip` reads: per level its center (x, z), its
/// spacing, and the distance from the eye where its morph starts; then the
/// level count, the half width in cells, the apron's reach, and 1 when the
/// clipmap draws the sea ([`Kind::Sea`]) rather than a body
/// ([`Kind::Body`]). All zero without a clipmap.
#[must_use]
pub fn rows(spec: &Spec, eye: Vec2, sea: bool) -> [[f32; 4]; ROWS] {
    let mut out = [[0.0; 4]; ROWS];
    let c = centers(spec, eye);
    for level in 0..spec.levels {
        let s = spec.level_spacing(level);
        let e = spec.extent(level);
        let at = c[level as usize];
        out[level as usize] = [at.x as f32, at.y as f32, s, e - 2.0 * s - MORPH * e];
    }
    out[MAX_LEVELS] = [
        spec.levels as f32,
        spec.half as f32,
        spec.far,
        if sea { 1.0 } else { 0.0 },
    ];
    out
}

/// A contiguous block's rest bounds in grid cells. The morph moves an odd
/// vertex by at most one cell toward the negative axes.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub range: Range<u32>,
    pub min: Vec2,
    pub max: Vec2,
}

/// A clipmap's vertices and triangles: every level's whole grid, each
/// level's triangles with its hole in each of four places, and the apron.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<WaterVertex>,
    pub indices: Vec<u32>,
    /// Per level, its triangles for each [`variant`]; level 0 has no hole,
    /// so its four are the same.
    pub levels: Vec<[Range<u32>; 4]>,
    /// Per-level blocks for each hole variant, in index order.
    pub blocks: Vec<[Vec<Block>; 4]>,
    /// The apron's triangles, from the last level's rim out to
    /// [`Spec::far`].
    pub apron: Range<u32>,
}

impl Mesh {
    /// Culls only bounded displaced blocks. The apron remains intact; an
    /// unknown displacement envelope draws the original complete mesh.
    #[must_use]
    pub fn draw_culled(
        &self,
        spec: &Spec,
        eye: Vec2,
        view_proj: Mat4,
        level: f32,
        displacement: Option<Vec3>,
    ) -> Vec<Range<u32>> {
        let Some(pad) = displacement.filter(|p| p.is_finite() && p.min_element() >= 0.0) else {
            return self.draw(spec, eye);
        };
        let centers = centers(spec, eye);
        let mut out: Vec<Range<u32>> = Vec::new();
        for (l, variants) in self.blocks.iter().enumerate() {
            let center = centers[l].as_vec2();
            let spacing = spec.level_spacing(l as u32);
            for block in &variants[variant(spec, eye, l as u32)] {
                let min = center + (block.min - Vec2::ONE) * spacing;
                let max = center + block.max * spacing;
                if crate::pbr::textured::in_frustum(
                    Vec3::new(min.x, level, min.y) - pad,
                    Vec3::new(max.x, level, max.y) + pad,
                    view_proj,
                ) {
                    if let Some(last) = out.last_mut().filter(|last| last.end == block.range.start)
                    {
                        last.end = block.range.end;
                    } else {
                        out.push(block.range.clone());
                    }
                }
            }
        }
        out.push(self.apron.clone());
        out
    }

    /// The index ranges a frame with its eye at `eye` draws.
    #[must_use]
    pub fn draw(&self, spec: &Spec, eye: Vec2) -> Vec<Range<u32>> {
        let mut out: Vec<Range<u32>> = (0..self.levels.len())
            .map(|level| self.levels[level][variant(spec, eye, level as u32)].clone())
            .collect();
        out.push(self.apron.clone());
        out
    }
}

/// The mesh of `spec`'s clipmap for body `body`. A vertex carries its grid
/// coordinates (x, z) and its level (y); an apron vertex carries its ring,
/// 1 to [`APRON_RINGS`], in the depth channel.
#[must_use]
pub fn mesh(spec: &Spec, body: usize) -> Mesh {
    let h = spec.half as i32;
    let side = (2 * h + 1) as u32;
    let mut out = Mesh::default();
    let vertex = |i: i32, level: u32, j: i32, ring: u32| {
        let mut v = WaterVertex::new(
            glam::Vec3::new(i as f32, level as f32, j as f32),
            ring as f32,
            Kind::Sea(1.0),
        )
        .in_body(body);
        v.kind = CODE;
        v
    };
    let quad = |indices: &mut Vec<u32>, q: [u32; 4]| {
        indices.extend_from_slice(&[q[0], q[2], q[1], q[0], q[3], q[2]]);
    };
    for level in 0..spec.levels {
        let base = out.vertices.len() as u32;
        for j in -h..=h {
            for i in -h..=h {
                out.vertices.push(vertex(i, level, j, 0));
            }
        }
        let at = |i: i32, j: i32| base + (j + h) as u32 * side + (i + h) as u32;
        let variants = if level == 0 { 1 } else { 4 };
        let mut ranges: Vec<Range<u32>> = Vec::with_capacity(4);
        let mut block_variants = Vec::new();
        for k in 0..variants {
            let (kx, kz) = ((k & 1) as i32, (k >> 1) as i32);
            let start = out.indices.len() as u32;
            let mut blocks = Vec::new();
            // Coarse blocks bound CPU submission cost; adjacent visible
            // blocks merge back into one draw.
            for bz in (-h..h).step_by(16) {
                for bx in (-h..h).step_by(16) {
                    let first = out.indices.len() as u32;
                    let (end_x, end_z) = ((bx + 16).min(h), (bz + 16).min(h));
                    for j in bz..end_z {
                        for i in bx..end_x {
                            let hole = level > 0
                                && (-h / 2 + kx..h / 2 + kx).contains(&i)
                                && (-h / 2 + kz..h / 2 + kz).contains(&j);
                            if !hole {
                                quad(
                                    &mut out.indices,
                                    [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)],
                                );
                            }
                        }
                    }
                    if first < out.indices.len() as u32 {
                        blocks.push(Block {
                            range: first..out.indices.len() as u32,
                            min: Vec2::new(bx as f32, bz as f32),
                            max: Vec2::new(end_x as f32, end_z as f32),
                        });
                    }
                }
            }
            block_variants.push(blocks);
            ranges.push(start..out.indices.len() as u32);
        }
        while ranges.len() < 4 {
            ranges.push(ranges[0].clone());
            block_variants.push(block_variants[0].clone());
        }
        let ranges: [Range<u32>; 4] = std::array::from_fn(|k| ranges[k].clone());
        out.levels.push(ranges);
        out.blocks
            .push(std::array::from_fn(|k| block_variants[k].clone()));
    }
    // The apron: the last level's rim, counterclockwise from (-h, -h), and
    // the same points stretched out ring by ring to the far square.
    let last = spec.levels - 1;
    let mut rim = Vec::new();
    for i in -h..h {
        rim.push((i, -h));
    }
    for j in -h..h {
        rim.push((h, j));
    }
    for i in (-h + 1..=h).rev() {
        rim.push((i, h));
    }
    for j in (-h + 1..=h).rev() {
        rim.push((-h, j));
    }
    let base = out.vertices.len() as u32;
    let rings = APRON_RINGS + 1;
    for &(i, j) in &rim {
        for ring in 0..rings {
            out.vertices.push(vertex(i, last, j, ring));
        }
    }
    let start = out.indices.len() as u32;
    let n = rim.len() as u32;
    for k in 0..n {
        let next = (k + 1) % n;
        for ring in 0..APRON_RINGS {
            let (a, b) = (base + k * rings + ring, base + next * rings + ring);
            quad(&mut out.indices, [a, b, b + 1, a + 1]);
        }
    }
    out.apron = start..out.indices.len() as u32;
    out
}

/// Where `vertex` of a clipmap lies at rest for an eye at `eye`, from the
/// frame's `rows`: `vs_water`'s placement and morph, on the CPU.
#[must_use]
pub fn position(rows: &[[f32; 4]; ROWS], eye: Vec2, vertex: &WaterVertex) -> Vec2 {
    let level = (vertex.pos[1].max(0.0) as usize).min(MAX_LEVELS - 1);
    let row = rows[level];
    let half = rows[MAX_LEVELS][1];
    let s = row[2];
    let center = Vec2::new(row[0], row[1]);
    let g = Vec2::new(vertex.pos[0], vertex.pos[2]);
    let world = center + g * s;
    // The morph grows with the distance from the eye along either axis,
    // reaching one before the rim wherever the eye stands in its snap cell.
    let d = (world - eye).abs();
    let width = MORPH * half * s;
    let alpha = ((d.max_element() - row[3]) / width).clamp(0.0, 1.0);
    let odd = g - 2.0 * (g * 0.5).floor();
    let morphed = g - odd * alpha;
    let reach = if vertex.depth > 0.5 {
        (rows[MAX_LEVELS][2] / (half * s)).powf(vertex.depth / APRON_RINGS as f32)
    } else {
        1.0
    };
    center + morphed * s * reach
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn culling_keeps_unknown_bounds_and_rejects_only_offscreen_blocks() {
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let spec = Spec::of(tier);
            let mesh = mesh(&spec, 0);
            let eye = Vec2::new(3.7, -1.3);
            assert_eq!(
                mesh.draw(&spec, eye),
                mesh.draw_culled(&spec, eye, Mat4::IDENTITY, 0.0, None)
            );
            for (level, blocks) in mesh.blocks.iter().enumerate() {
                for (variant, blocks) in blocks.iter().enumerate() {
                    let range = &mesh.levels[level][variant];
                    assert_eq!(blocks.first().unwrap().range.start, range.start);
                    assert_eq!(blocks.last().unwrap().range.end, range.end);
                    assert!(
                        blocks
                            .windows(2)
                            .all(|b| b[0].range.end == b[1].range.start)
                    );
                }
            }
            let visible = mesh.draw_culled(&spec, eye, Mat4::IDENTITY, 0.0, Some(Vec3::ZERO));
            assert!(
                visible.iter().map(|r| r.end - r.start).sum::<u32>()
                    < mesh
                        .draw(&spec, eye)
                        .iter()
                        .map(|r| r.end - r.start)
                        .sum::<u32>()
            );
            assert_eq!(visible.last().unwrap(), &mesh.apron);
        }
    }

    fn key(p: Vec2) -> (i64, i64) {
        ((p.x * 256.0).round() as i64, (p.y * 256.0).round() as i64)
    }

    /// For eyes across a snap cell and far from the origin, the drawn
    /// clipmap covers its far square once, and where two levels meet every
    /// vertex of the finer level's rim lies on a vertex of the coarser
    /// level's hole and the other way round, so the rings meet without a
    /// crack or a T-junction however the waves move them. Within a level the
    /// triangles share their vertices, so it cannot crack.
    #[test]
    fn the_rings_meet_without_cracks() {
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let spec = Spec::of(tier);
            spec.validate().unwrap();
            let mesh = mesh(&spec, 0);
            let h = spec.half as i32;
            for eye in [
                Vec2::new(0.1, 0.2),
                Vec2::new(3.7, -1.3),
                Vec2::new(-130.4, 77.9),
                Vec2::new(511.0, -611.5),
            ] {
                let rows = rows(&spec, eye, true);
                let at: Vec<Vec2> = mesh
                    .vertices
                    .iter()
                    .map(|v| position(&rows, eye, v))
                    .collect();
                let mut area = 0.0f64;
                for range in mesh.draw(&spec, eye) {
                    for t in mesh.indices[range.start as usize..range.end as usize].chunks(3) {
                        let [a, b, c] = [at[t[0] as usize], at[t[1] as usize], at[t[2] as usize]];
                        area += f64::from((b - a).perp_dot(c - a).abs()) * 0.5;
                    }
                }
                let square = f64::from(2.0 * spec.far).powi(2);
                assert!(
                    (area - square).abs() / square < 1e-4,
                    "{tier:?} {area} vs {square}"
                );
                let grid = |k: usize| {
                    let v = &mesh.vertices[k];
                    (
                        v.pos[1] as u32,
                        v.pos[0] as i32,
                        v.pos[2] as i32,
                        v.depth > 0.5,
                    )
                };
                for level in 0..spec.levels - 1 {
                    let k = variant(&spec, eye, level + 1) as i32;
                    let (kx, kz) = (k & 1, k >> 1);
                    let (lo_x, hi_x) = (-h / 2 + kx, h / 2 + kx);
                    let (lo_z, hi_z) = (-h / 2 + kz, h / 2 + kz);
                    let mut rim = std::collections::BTreeSet::new();
                    let mut hole = std::collections::BTreeSet::new();
                    for (i, p) in at.iter().enumerate() {
                        let (l, x, z, far) = grid(i);
                        if far || i >= (spec.levels as usize) * ((2 * h + 1) as usize).pow(2) {
                            continue;
                        }
                        if l == level && (x.abs() == h || z.abs() == h) {
                            rim.insert(key(*p));
                        }
                        let on_x = (x == lo_x || x == hi_x) && (lo_z..=hi_z).contains(&z);
                        let on_z = (z == lo_z || z == hi_z) && (lo_x..=hi_x).contains(&x);
                        if l == level + 1 && (on_x || on_z) {
                            hole.insert(key(*p));
                        }
                    }
                    assert_eq!(rim, hole, "{tier:?} at {eye}, level {level}");
                }
            }
        }
    }

    /// A vertex's world position moves continuously with the eye, so the
    /// morph never pops: from just before a snap to just after it, every
    /// vertex that still exists moves by well under a cell.
    #[test]
    fn the_morph_is_continuous_across_a_snap() {
        let spec = Spec::of(Tier::Medium);
        let mesh = mesh(&spec, 0);
        let s = f64::from(spec.spacing);
        let before = Vec2::new((2.0 * s - 1e-3) as f32, 0.3);
        let after = Vec2::new((2.0 * s + 1e-3) as f32, 0.3);
        let place = |eye: Vec2| -> HashMap<(i64, i64, u32), Vec2> {
            let rows = rows(&spec, eye, true);
            mesh.vertices
                .iter()
                .filter(|v| v.depth < 0.5)
                .map(|v| {
                    let level = v.pos[1] as u32;
                    let s = spec.level_spacing(level);
                    let c = Vec2::new(rows[level as usize][0], rows[level as usize][1]);
                    let rest = c + Vec2::new(v.pos[0], v.pos[2]) * s;
                    ((key(rest).0, key(rest).1, level), position(&rows, eye, v))
                })
                .collect()
        };
        let (a, b) = (place(before), place(after));
        let mut shared = 0;
        for (k, p) in &a {
            if let Some(q) = b.get(k) {
                shared += 1;
                let cell = spec.level_spacing(k.2);
                assert!(p.distance(*q) < 0.01 * cell, "{k:?}: {p} vs {q}");
            }
        }
        assert!(shared > mesh.vertices.len() / 2);
    }

    /// The tiers' ring counts and reach follow the specification, and a
    /// frame stays inside the zone's triangle budgets.
    #[test]
    fn tiers_draw_three_four_and_five_rings() {
        let [low, medium, high] = [Tier::Low, Tier::Medium, Tier::High].map(Spec::of);
        assert_eq!([low.levels, medium.levels, high.levels], [3, 4, 5]);
        for spec in [low, medium, high] {
            let mesh = mesh(&spec, 2);
            let drawn: u32 = mesh
                .draw(&spec, Vec2::ZERO)
                .iter()
                .map(|r| r.end - r.start)
                .sum();
            assert_eq!(drawn / 3, spec.triangles());
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.kind == CODE && v.body == 2.0)
            );
        }
        assert!(low.triangles() < 30_000);
        assert!(high.triangles() < 100_000);
        assert!(high.extent(4) >= 380.0 && low.extent(2) >= 250.0);
    }
}
