//! A model's collision taken from its triangles: the ground under it cut
//! into small square columns, each holding the heights where the model's
//! surfaces are solid. A wall is a run of tall spans, a stage or a step is a
//! span whose top a character stands on, and a roof or a lintel is a span
//! high enough to walk under. Every span belongs to a part (a destructible
//! building's piece), so a building that loses a piece loses its spans.

use glam::Vec3;

use super::controller::Footprint;

/// Spans of one part closer than this merge into one, m: the two faces of a
/// wall, or a floor's top and underside.
const MERGE: f32 = 0.2;

/// One solid height range of one column, m, and the part it belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub lo: f32,
    pub hi: f32,
    pub part: u32,
}

/// The columns of one model on a square grid aligned with the world's axes.
#[derive(Clone, Debug, PartialEq)]
pub struct Columns {
    /// The grid's lowest x and z corner, m.
    origin: [f32; 2],
    /// A column's side, m.
    cell: f32,
    /// Columns along x and z.
    dims: [usize; 2],
    /// Where each column's spans start in `spans`, with one more entry for
    /// the end.
    starts: Vec<u32>,
    spans: Vec<Span>,
}

impl Columns {
    /// The columns `cell` meters wide under `triangles`, each a world-space
    /// triangle and its part. `None` when there are no triangles.
    #[must_use]
    pub fn rasterize(triangles: &[([Vec3; 3], u32)], cell: f32) -> Option<Self> {
        let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for (corners, _) in triangles {
            for c in corners {
                if !c.is_finite() {
                    continue;
                }
                min = [min[0].min(c.x), min[1].min(c.z)];
                max = [max[0].max(c.x), max[1].max(c.z)];
            }
        }
        if !min[0].is_finite() || cell <= 0.0 {
            return None;
        }
        let dims = [
            (((max[0] - min[0]) / cell).floor() as usize + 1).min(4096),
            (((max[1] - min[1]) / cell).floor() as usize + 1).min(4096),
        ];
        let mut cells: Vec<Vec<Span>> = vec![Vec::new(); dims[0] * dims[1]];
        for (corners, part) in triangles {
            if corners.iter().any(|c| !c.is_finite()) {
                continue;
            }
            let lo = [
                corners.iter().map(|c| c.x).fold(f32::INFINITY, f32::min),
                corners.iter().map(|c| c.z).fold(f32::INFINITY, f32::min),
            ];
            let hi = [
                corners
                    .iter()
                    .map(|c| c.x)
                    .fold(f32::NEG_INFINITY, f32::max),
                corners
                    .iter()
                    .map(|c| c.z)
                    .fold(f32::NEG_INFINITY, f32::max),
            ];
            let first = |v: f32, axis: usize| {
                (((v - min[axis]) / cell).floor().max(0.0) as usize).min(dims[axis] - 1)
            };
            for i in first(lo[0], 0)..=first(hi[0], 0) {
                for j in first(lo[1], 1)..=first(hi[1], 1) {
                    let x0 = min[0] + cell * i as f32;
                    let z0 = min[1] + cell * j as f32;
                    if let Some((a, b)) = clip_heights(corners, [x0, z0], [x0 + cell, z0 + cell]) {
                        cells[i * dims[1] + j].push(Span {
                            lo: a,
                            hi: b,
                            part: *part,
                        });
                    }
                }
            }
        }
        let mut starts = Vec::with_capacity(cells.len() + 1);
        let mut spans = Vec::new();
        for mut column in cells {
            starts.push(spans.len() as u32);
            column.sort_by(|a, b| a.part.cmp(&b.part).then(a.lo.total_cmp(&b.lo)));
            let mut merged: Vec<Span> = Vec::with_capacity(column.len());
            for span in column {
                match merged.last_mut() {
                    Some(last) if last.part == span.part && span.lo <= last.hi + MERGE => {
                        last.hi = last.hi.max(span.hi);
                    }
                    _ => merged.push(span),
                }
            }
            spans.extend(merged);
        }
        starts.push(spans.len() as u32);
        Some(Self {
            origin: min,
            cell,
            dims,
            starts,
            spans,
        })
    }

    /// The ground the grid covers.
    #[must_use]
    pub fn bounds(&self) -> Footprint {
        Footprint {
            min: self.origin,
            max: [
                self.origin[0] + self.cell * self.dims[0] as f32,
                self.origin[1] + self.cell * self.dims[1] as f32,
            ],
        }
    }

    /// A column's side, m.
    #[must_use]
    pub fn cell(&self) -> f32 {
        self.cell
    }

    /// Every span, in column order.
    #[must_use]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// The column at `(x, z)`, if the grid covers it.
    #[must_use]
    pub fn column(&self, x: f32, z: f32) -> Option<[usize; 2]> {
        let i = ((x - self.origin[0]) / self.cell).floor();
        let j = ((z - self.origin[1]) / self.cell).floor();
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.dims[0] && (j as usize) < self.dims[1])
            .then(|| [i as usize, j as usize])
    }

    /// The spans of column `at`.
    #[must_use]
    pub fn at(&self, at: [usize; 2]) -> &[Span] {
        let k = at[0] * self.dims[1] + at[1];
        &self.spans[self.starts[k] as usize..self.starts[k + 1] as usize]
    }

    /// Every column's ground square and spans.
    pub fn iter(&self) -> impl Iterator<Item = (Footprint, &[Span])> {
        (0..self.dims[0])
            .flat_map(move |i| (0..self.dims[1]).map(move |j| [i, j]))
            .map(|at| (self.square(at), self.at(at)))
    }

    /// The spans under `(x, z)`, none off the grid.
    #[must_use]
    pub fn spans_at(&self, x: f32, z: f32) -> &[Span] {
        self.column(x, z).map_or(&[], |at| self.at(at))
    }

    /// The columns within `reach` of `(x, z)`, as an inclusive range of
    /// column indices along x and z, or `None` when none are.
    #[must_use]
    pub fn window(&self, x: f32, z: f32, reach: f32) -> Option<([usize; 2], [usize; 2])> {
        let b = self.bounds();
        if x + reach < b.min[0]
            || x - reach > b.max[0]
            || z + reach < b.min[1]
            || z - reach > b.max[1]
        {
            return None;
        }
        let index = |v: f32, axis: usize| {
            (((v - self.origin[axis]) / self.cell).floor().max(0.0) as usize)
                .min(self.dims[axis] - 1)
        };
        Some((
            [index(x - reach, 0), index(z - reach, 1)],
            [index(x + reach, 0), index(z + reach, 1)],
        ))
    }

    /// The ground square of column `at`.
    #[must_use]
    pub fn square(&self, at: [usize; 2]) -> Footprint {
        let x = self.origin[0] + self.cell * at[0] as f32;
        let z = self.origin[1] + self.cell * at[1] as f32;
        Footprint {
            min: [x, z],
            max: [x + self.cell, z + self.cell],
        }
    }
}

/// The lowest and highest height of the part of triangle `corners` over the
/// square from `min` to `max` (x and z), or `None` when it misses it.
fn clip_heights(corners: &[Vec3; 3], min: [f32; 2], max: [f32; 2]) -> Option<(f32, f32)> {
    let mut polygon: Vec<Vec3> = corners.to_vec();
    for (axis, bound, upper) in [
        (0, min[0], true),
        (0, max[0], false),
        (2, min[1], true),
        (2, max[1], false),
    ] {
        polygon = clip(&polygon, axis, bound, upper);
        if polygon.is_empty() {
            return None;
        }
    }
    let lo = polygon.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let hi = polygon
        .iter()
        .map(|p| p.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Some((lo, hi))
}

/// `polygon` cut by the plane where `axis` is `bound`, keeping the side at
/// or above it when `upper`.
fn clip(polygon: &[Vec3], axis: usize, bound: f32, upper: bool) -> Vec<Vec3> {
    let inside = |p: &Vec3| {
        if upper {
            p[axis] >= bound - 1e-5
        } else {
            p[axis] <= bound + 1e-5
        }
    };
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for k in 0..polygon.len() {
        let a = polygon[k];
        let b = polygon[(k + 1) % polygon.len()];
        let (ia, ib) = (inside(&a), inside(&b));
        if ia {
            out.push(a);
        }
        if ia != ib {
            let t = (bound - a[axis]) / (b[axis] - a[axis]);
            out.push(a + (b - a) * t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wall_quad_fills_the_columns_along_it() {
        // A 2 m long, 3 m tall wall along x at z = 0.
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(2.0, 0.0, 0.0);
        let c = Vec3::new(2.0, 3.0, 0.0);
        let d = Vec3::new(0.0, 3.0, 0.0);
        let grid = Columns::rasterize(&[([a, b, c], 0), ([a, c, d], 0)], 0.25).unwrap();
        let spans = grid.spans_at(1.1, 0.0);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].lo <= 0.01 && spans[0].hi >= 2.99, "{spans:?}");
    }

    #[test]
    fn a_floor_is_a_thin_span_at_its_height() {
        let y = 0.9;
        let a = Vec3::new(0.0, y, 0.0);
        let b = Vec3::new(4.0, y, 0.0);
        let c = Vec3::new(4.0, y, 4.0);
        let d = Vec3::new(0.0, y, 4.0);
        let grid = Columns::rasterize(&[([a, b, c], 3), ([a, c, d], 3)], 0.25).unwrap();
        let spans = grid.spans_at(2.0, 2.0);
        assert_eq!(
            spans,
            &[Span {
                lo: y,
                hi: y,
                part: 3
            }]
        );
        assert!(grid.spans_at(9.0, 2.0).is_empty());
    }

    fn quad(out: &mut Vec<([Vec3; 3], u32)>, corners: [[f32; 3]; 4], part: u32) {
        let [a, b, c, d] = corners.map(Vec3::from);
        out.push(([a, b, c], part));
        out.push(([a, c, d], part));
    }

    /// A box's six faces, from `min` to `max`.
    fn cuboid(out: &mut Vec<([Vec3; 3], u32)>, min: [f32; 3], max: [f32; 3], part: u32) {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        quad(
            out,
            [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
            part,
        );
        quad(
            out,
            [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
            part,
        );
        quad(
            out,
            [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]],
            part,
        );
        quad(
            out,
            [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            part,
        );
        quad(
            out,
            [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
            part,
        );
        quad(
            out,
            [[x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]],
            part,
        );
    }

    fn walk(solids: &Solids, player: &mut PlayerController, seconds: f32) {
        let input = InputState {
            forward: true,
            ..InputState::default()
        };
        for _ in 0..(seconds * 60.0) as usize {
            solids.step(player, &input, 1.0 / 60.0, 500.0, true);
        }
    }

    use super::super::controller::{InputState, PlayerController};
    use super::super::solids::Solids;
    use std::sync::Arc;

    /// A stage 0.9 m high from z = 4 to 8 with three steps up its front,
    /// a shell wall at its back, and a lintel at 2.4 m over the stage.
    fn stage() -> Solids {
        let mut t = Vec::new();
        for (k, z) in [(1.0_f32, 2.5_f32), (2.0, 3.0), (3.0, 3.5)] {
            cuboid(&mut t, [-2.0, 0.0, z], [2.0, 0.3 * k, z + 0.5], 0);
        }
        cuboid(&mut t, [-4.0, 0.0, 4.0], [4.0, 0.9, 8.0], 0);
        cuboid(&mut t, [-4.0, 0.9, 8.0], [4.0, 5.0, 8.3], 1);
        cuboid(&mut t, [-4.0, 3.3, 5.5], [4.0, 3.5, 6.0], 2);
        let grid = Columns::rasterize(&t, 0.25).unwrap();
        let mut solids = Solids::over(|_, _| 0.0);
        solids.add_columns(Arc::new(grid), None);
        solids
    }

    #[test]
    fn a_walker_climbs_the_steps_onto_the_stage_and_stops_at_its_back_wall() {
        let solids = stage();
        let mut player = PlayerController::new(Vec3::new(0.0, 0.0, 0.0), 0.0);
        walk(&solids, &mut player, 4.0);
        assert!(
            (player.pos.y - 0.9).abs() < 0.05,
            "on the stage at {}",
            player.pos.y
        );
        // Under the lintel and up to the shell, never into it.
        assert!(
            player.pos.z > 7.0 && player.pos.z < 8.0,
            "z = {}",
            player.pos.z
        );
    }

    #[test]
    fn the_stage_side_blocks_a_walker_on_the_ground() {
        let solids = stage();
        // West of the steps, the stage's side is 0.9 m: too tall to step.
        let mut player = PlayerController::new(Vec3::new(-3.0, 0.0, 0.0), 0.0);
        walk(&solids, &mut player, 3.0);
        assert!(player.pos.y < 0.05);
        assert!(player.pos.z < 4.0, "z = {}", player.pos.z);
    }

    #[test]
    fn a_fallen_part_stops_blocking() {
        let mut t = Vec::new();
        cuboid(&mut t, [-4.0, 0.0, 4.0], [4.0, 3.0, 4.3], 0);
        let grid = Arc::new(Columns::rasterize(&t, 0.25).unwrap());
        let mut solids = Solids::over(|_, _| 0.0);
        solids.add_columns(grid.clone(), Some(Arc::new(vec![false])));
        let mut player = PlayerController::new(Vec3::new(0.0, 0.0, 0.0), 0.0);
        walk(&solids, &mut player, 2.0);
        assert!(
            player.pos.z > 6.0,
            "walked through the gap: {}",
            player.pos.z
        );
        let mut standing = Solids::over(|_, _| 0.0);
        standing.add_columns(grid, None);
        let mut player = PlayerController::new(Vec3::new(0.0, 0.0, 0.0), 0.0);
        walk(&standing, &mut player, 2.0);
        assert!(player.pos.z < 4.0, "z = {}", player.pos.z);
        // A levitating character comes down onto the wall's top.
        assert!((standing.floor(0.0, 4.1, 5.0) - 3.0).abs() < 0.01);
    }
}
