//! Point queries over a zone's water.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::body::{Sample, WaterBody, WaterId};

/// Water a body can float in.
pub trait Water {
    /// The water over `(x, z)` at `tick`, if any.
    fn sample(&self, x: f64, z: f64, tick: u64) -> Option<Sample>;

    /// The bodies whose outlines may overlap the box from `min` to `max` in
    /// the (x, z) plane, each once.
    fn bodies_overlapping(&self, min: DVec2, max: DVec2) -> Vec<WaterId>;
}

impl Water for WaterBody {
    fn sample(&self, x: f64, z: f64, tick: u64) -> Option<Sample> {
        self.surface().sample(x, z, tick)
    }

    fn bodies_overlapping(&self, min: DVec2, max: DVec2) -> Vec<WaterId> {
        overlaps(self, min, max)
            .then_some(self.id)
            .into_iter()
            .collect()
    }
}

fn overlaps(body: &WaterBody, min: DVec2, max: DVec2) -> bool {
    body.outline
        .bounds()
        .is_none_or(|(lo, hi)| lo.x <= max.x && lo.y <= max.y && hi.x >= min.x && hi.y >= min.y)
}

/// A zone's water bodies with a uniform grid over their outlines, so a
/// point lookup tests only the few bodies whose bounds cover its cell.
/// Where outlines overlap, the body listed first wins; unbounded bodies
/// (an ocean) come after every bounded one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterSet {
    bodies: Vec<WaterBody>,
    origin: DVec2,
    cell: f64,
    nx: usize,
    nz: usize,
    /// Indices of bounded bodies whose bounds cover each cell, row-major by z.
    cells: Vec<Vec<u32>>,
    /// Indices of unbounded bodies.
    everywhere: Vec<u32>,
}

/// The most grid cells a set builds; a coarser cell is chosen past it.
const MAX_CELLS: usize = 1 << 16;

impl WaterSet {
    /// The set of `bodies`, gridded at `cell` meters.
    #[must_use]
    pub fn new(bodies: Vec<WaterBody>, cell: f64) -> Self {
        let mut lo = DVec2::splat(f64::INFINITY);
        let mut hi = DVec2::splat(f64::NEG_INFINITY);
        let mut everywhere = Vec::new();
        for (i, body) in bodies.iter().enumerate() {
            match body.outline.bounds() {
                Some((a, b)) => {
                    lo = lo.min(a);
                    hi = hi.max(b);
                }
                None => everywhere.push(i as u32),
            }
        }
        if lo.x > hi.x {
            lo = DVec2::ZERO;
            hi = DVec2::ZERO;
        }
        let mut cell = cell.max(1e-3);
        let count = |cell: f64| {
            (
                ((hi.x - lo.x) / cell).floor() as usize + 1,
                ((hi.y - lo.y) / cell).floor() as usize + 1,
            )
        };
        while {
            let (x, z) = count(cell);
            x * z > MAX_CELLS
        } {
            cell *= 2.0;
        }
        let (nx, nz) = count(cell);
        let mut cells = vec![Vec::new(); nx * nz];
        for (index, body) in bodies.iter().enumerate() {
            let Some((a, b)) = body.outline.bounds() else {
                continue;
            };
            let (i0, j0) = cell_of(lo, cell, a);
            let (i1, j1) = cell_of(lo, cell, b);
            for j in j0..=j1.min(nz - 1) {
                for i in i0..=i1.min(nx - 1) {
                    cells[j * nx + i].push(index as u32);
                }
            }
        }
        Self {
            bodies,
            origin: lo,
            cell,
            nx,
            nz,
            cells,
            everywhere,
        }
    }

    #[must_use]
    pub fn bodies(&self) -> &[WaterBody] {
        &self.bodies
    }

    /// The body with id `id`, if the set has it.
    #[must_use]
    pub fn get(&self, id: WaterId) -> Option<&WaterBody> {
        self.bodies.iter().find(|b| b.id == id)
    }

    fn candidates(&self, p: DVec2) -> impl Iterator<Item = &WaterBody> {
        let g = (p - self.origin) / self.cell;
        let bounded: &[u32] =
            if g.x >= 0.0 && g.y >= 0.0 && (g.x as usize) < self.nx && (g.y as usize) < self.nz {
                &self.cells[g.y as usize * self.nx + g.x as usize]
            } else {
                &[]
            };
        bounded
            .iter()
            .chain(&self.everywhere)
            .map(|&i| &self.bodies[i as usize])
    }
}

fn cell_of(origin: DVec2, cell: f64, p: DVec2) -> (usize, usize) {
    let g = ((p - origin) / cell).max(DVec2::ZERO);
    (g.x as usize, g.y as usize)
}

impl Water for WaterSet {
    fn sample(&self, x: f64, z: f64, tick: u64) -> Option<Sample> {
        self.candidates(DVec2::new(x, z))
            .find_map(|body| body.surface().sample(x, z, tick))
    }

    fn bodies_overlapping(&self, min: DVec2, max: DVec2) -> Vec<WaterId> {
        let (i0, j0) = cell_of(self.origin, self.cell, min);
        let (i1, j1) = cell_of(self.origin, self.cell, max);
        let mut found: Vec<u32> = self.everywhere.clone();
        if i0 < self.nx && j0 < self.nz {
            for j in j0..=j1.min(self.nz - 1) {
                for i in i0..=i1.min(self.nx - 1) {
                    found.extend(&self.cells[j * self.nx + i]);
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found
            .into_iter()
            .map(|i| &self.bodies[i as usize])
            .filter(|b| overlaps(b, min, max))
            .map(|b| b.id)
            .collect()
    }
}
