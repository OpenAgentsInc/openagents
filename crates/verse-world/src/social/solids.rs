//! What a character stands on and runs into in a social world, with
//! height: each blocker's footprint and top, and each gabled roof's
//! surface, over the zone's heightfield. A character whose feet are above a
//! blocker's top passes over it, and the highest surface under the feet
//! holds it up, so a levitating character comes down onto a roof rather
//! than through it. A model whose collision follows its own triangles adds
//! [`Columns`]: its spans block a character whose body they cross and hold
//! up one whose feet are on them, so a character walks up a stage's steps,
//! under a lintel, and around a curved shell. Verse's Everglade zone builds
//! these from its pinned pack, and a hosted instance walks avatars over the
//! same values.

use std::sync::Arc;

use glam::Vec3;

use super::columns::Columns;
use super::controller::{Footprint, InputState, PlayerController, RADIUS, RUN_SPEED, SPRINT_MULT};
use super::sight::{self, Reach, Sight};

/// How far the feet may be below a surface and still step onto it, m.
pub const STEP: f32 = 0.35;
/// How high above the feet a column's span still blocks, m: a span whose
/// underside is higher passes over the head.
pub const HEAD: f32 = 1.8;
/// Farthest one controller step moves over columns, m, so a run never
/// passes through a thin wall in one long frame: under half of a 0.25 m
/// column and the character's 0.45 m radius either side of it.
const STRIDE: f32 = 0.35;
/// How far around the character the columns are gathered, m.
const NEAR: f32 = RADIUS + 0.6;

/// One model's columns, and which of its parts still stand (`None`: all).
#[derive(Clone, Debug)]
struct Placed {
    grid: Arc<Columns>,
    standing: Option<Arc<Vec<bool>>>,
}

impl Placed {
    fn spans(&self, x: f32, z: f32) -> impl Iterator<Item = &super::columns::Span> {
        self.grid
            .spans_at(x, z)
            .iter()
            .filter(move |s| self.stands(s.part))
    }

    /// Where the segment first meets a standing span, before `before`:
    /// the columns near points stepped along it, each tested as a box.
    fn sweep(&self, from: Vec3, to: Vec3, radius: f32, before: f32) -> Option<f32> {
        let cell = self.grid.cell();
        let length = from.distance(to);
        let step = cell * 0.5;
        let steps = ((length * before / step).ceil() as usize).clamp(1, 512);
        let reach = radius + step;
        let mut best: Option<f32> = None;
        for k in 0..=steps {
            let t = before * k as f32 / steps as f32;
            if best.is_some_and(|b| b + step / length.max(1e-6) < t) {
                break;
            }
            let p = from.lerp(to, t);
            let Some((lo, hi)) = self.grid.window(p.x, p.z, reach) else {
                continue;
            };
            for i in lo[0]..=hi[0] {
                for j in lo[1]..=hi[1] {
                    let square = self.grid.square([i, j]);
                    for span in self.grid.at([i, j]) {
                        if !self.stands(span.part) {
                            continue;
                        }
                        if let Some(hit) =
                            sight::column_hit(from, to, radius, &square, span.lo, span.hi)
                        {
                            best = Some(best.map_or(hit, |b: f32| b.min(hit)));
                        }
                    }
                }
            }
        }
        best
    }

    fn stands(&self, part: u32) -> bool {
        self.standing
            .as_ref()
            .is_none_or(|s| s.get(part as usize).copied().unwrap_or(true))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Block {
    footprint: Footprint,
    top: f32,
}

/// A gabled roof: its ridge runs along the model's z axis and the slopes
/// fall along x, from `ridge` at the center to `eave` at `half` either side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Roof {
    /// The roof's center on the ground, x and z, m.
    pub center: [f32; 2],
    /// The model's +x axis on the ground.
    pub across: [f32; 2],
    /// Half extents across and along the ridge, m.
    pub half: [f32; 2],
    /// Height of the eaves, m.
    pub eave: f32,
    /// Height of the ridge, m.
    pub ridge: f32,
}

impl Roof {
    fn surface(&self, x: f32, z: f32) -> Option<f32> {
        let (dx, dz) = (x - self.center[0], z - self.center[1]);
        let u = dx * self.across[0] + dz * self.across[1];
        let v = -dx * self.across[1] + dz * self.across[0];
        (u.abs() <= self.half[0] && v.abs() <= self.half[1])
            .then(|| self.ridge - (self.ridge - self.eave) * u.abs() / self.half[0])
    }

    /// Where, from 0 to 1, the segment from `from` to `to` first comes
    /// within `radius` of the roof's surface from the side it starts on, or
    /// `None` when it does not.
    fn sweep(&self, from: Vec3, to: Vec3, radius: f32) -> Option<f32> {
        let local = |p: Vec3| {
            let (dx, dz) = (p.x - self.center[0], p.z - self.center[1]);
            (
                dx * self.across[0] + dz * self.across[1],
                -dx * self.across[1] + dz * self.across[0],
            )
        };
        let (u0, v0) = local(from);
        let (u1, v1) = local(to);
        let (du, dv) = (u1 - u0, v1 - v0);
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for (o, d, limit) in [
            (u0, du, self.half[0] + radius),
            (v0, dv, self.half[1] + radius),
        ] {
            if d.abs() < 1e-9 {
                if o.abs() > limit {
                    return None;
                }
                continue;
            }
            let (a, b) = ((-limit - o) / d, (limit - o) / d);
            lo = lo.max(a.min(b));
            hi = hi.min(a.max(b));
        }
        if lo > hi {
            return None;
        }
        let slope = (self.ridge - self.eave) / self.half[0].max(1e-6);
        let gap = |t: f32| {
            let u = u0 + du * t;
            from.y + (to.y - from.y) * t - (self.ridge - slope * u.abs())
        };
        let start = gap(lo);
        if start.abs() < radius {
            // Entering the roof's skin through its edge, unless the segment
            // starts there.
            return (lo > 0.0).then_some(lo);
        }
        let above = start > 0.0;
        let target = if above { radius } else { -radius };
        let ridge = if du.abs() > 1e-9 { -u0 / du } else { -1.0 };
        let mut pieces = [(lo, hi), (hi, hi)];
        if ridge > lo && ridge < hi {
            pieces = [(lo, ridge), (ridge, hi)];
        }
        for (a, b) in pieces {
            if b <= a {
                continue;
            }
            let (fa, fb) = (gap(a), gap(b));
            let crossed = if above { fb < target } else { fb > target };
            if crossed {
                return Some(a + (fa - target) / (fa - fb) * (b - a));
            }
        }
        None
    }
}

/// A zone's solids over its ground.
#[derive(Clone, Debug)]
pub struct Solids {
    ground: fn(f32, f32) -> f32,
    blocks: Vec<Block>,
    roofs: Vec<Roof>,
    /// Blocks a spell raised, such as Wall of Stone's panels; replaced
    /// whole by [`Solids::set_spell_blocks`].
    spell: Vec<Block>,
    columns: Vec<Placed>,
}

impl Default for Solids {
    fn default() -> Self {
        Self::over(super::everglade::height)
    }
}

impl Sight for Solids {
    /// Sweeps over the ground, every block and spell-raised block up to its
    /// top, every roof, and the standing parts of every model's columns.
    fn sweep(&self, from: Vec3, to: Vec3, radius: f32) -> f32 {
        let reach = Reach::of(from, to, radius);
        let mut t = sight::ground_hit(from, to, radius, &self.ground).unwrap_or(1.0);
        for block in self.blocks.iter().chain(&self.spell) {
            if reach.meets(&block.footprint)
                && let Some(hit) = sight::column_hit(
                    from,
                    to,
                    radius,
                    &block.footprint,
                    f32::NEG_INFINITY,
                    block.top,
                )
            {
                t = t.min(hit);
            }
        }
        for roof in &self.roofs {
            if let Some(hit) = roof.sweep(from, to, radius) {
                t = t.min(hit);
            }
        }
        for placed in &self.columns {
            if reach.meets(&placed.grid.bounds())
                && let Some(hit) = placed.sweep(from, to, radius, t)
            {
                t = t.min(hit);
            }
        }
        t
    }

    fn ground(&self, x: f32, z: f32) -> Option<f32> {
        Some((self.ground)(x, z))
    }
}

impl Solids {
    /// No solids over the heightfield `ground`.
    #[must_use]
    pub fn over(ground: fn(f32, f32) -> f32) -> Self {
        Self {
            ground,
            blocks: Vec::new(),
            roofs: Vec::new(),
            spell: Vec::new(),
            columns: Vec::new(),
        }
    }

    /// Adds a model's columns; with `standing`, only the parts it marks
    /// `true` are solid.
    pub fn add_columns(&mut self, grid: Arc<Columns>, standing: Option<Arc<Vec<bool>>>) {
        self.columns.push(Placed { grid, standing });
    }

    /// How many models' columns there are.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Adds a blocker: `footprint` up to `top`, m.
    pub fn add_block(&mut self, footprint: Footprint, top: f32) {
        self.blocks.push(Block { footprint, top });
    }

    /// Keeps only the blockers `keep` accepts, by footprint and top, m.
    pub fn retain_blocks(&mut self, mut keep: impl FnMut(&Footprint, f32) -> bool) {
        self.blocks
            .retain(|block| keep(&block.footprint, block.top));
    }

    /// Adds a roof a character can stand on and strikes from below.
    pub fn add_roof(&mut self, roof: Roof) {
        self.roofs.push(roof);
    }

    /// The ground's height at `(x, z)`, m.
    #[must_use]
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        (self.ground)(x, z)
    }

    /// Replaces the spell-raised blocks with `blocks`, each a footprint and
    /// its top, m.
    pub fn set_spell_blocks(&mut self, blocks: impl IntoIterator<Item = (Footprint, f32)>) {
        self.spell = blocks
            .into_iter()
            .map(|(footprint, top)| Block { footprint, top })
            .collect();
    }

    /// The lowest roof at `(x, z)` at or above `head`, m: what a character
    /// rising there strikes.
    #[must_use]
    pub fn ceiling(&self, x: f32, z: f32, head: f32) -> Option<f32> {
        let columns = self
            .columns
            .iter()
            .flat_map(|p| p.spans(x, z))
            .map(|s| s.lo)
            .filter(|&lo| lo >= head);
        self.roofs
            .iter()
            .filter_map(|roof| roof.surface(x, z))
            .filter(|&surface| surface >= head)
            .chain(columns)
            .min_by(f32::total_cmp)
    }

    /// The footprints near `(x, z)` that block feet at `feet`: every block
    /// within `reach` whose top is more than a step above them, and each
    /// run of columns there whose spans cross the body, merged into
    /// rectangles.
    #[must_use]
    pub fn blocking_near(&self, x: f32, z: f32, reach: f32, feet: f32) -> Vec<Footprint> {
        let mut out: Vec<Footprint> = self
            .blocks
            .iter()
            .chain(&self.spell)
            .filter(|block| block.top > feet + STEP && block.footprint.contains(x, z, reach))
            .map(|block| block.footprint)
            .collect();
        // A stair's next step is within reach of the body before the feet
        // are on the first: under the body, a surface a step up counts as
        // where the feet are.
        let under = |f: &Footprint| {
            let dx = (f.min[0] - x).max(x - f.max[0]).max(0.0);
            let dz = (f.min[1] - z).max(z - f.max[1]).max(0.0);
            dx.hypot(dz) <= RADIUS
        };
        let mut feet_on = feet;
        for placed in &self.columns {
            let Some((lo, hi)) = placed.grid.window(x, z, RADIUS) else {
                continue;
            };
            for i in lo[0]..=hi[0] {
                for j in lo[1]..=hi[1] {
                    if !under(&placed.grid.square([i, j])) {
                        continue;
                    }
                    for s in placed.grid.at([i, j]) {
                        if s.hi > feet_on && s.hi <= feet + STEP && placed.stands(s.part) {
                            feet_on = s.hi;
                        }
                    }
                }
            }
        }
        let feet = feet_on;
        for placed in &self.columns {
            let Some((lo, hi)) = placed.grid.window(x, z, reach) else {
                continue;
            };
            let width = hi[1] - lo[1] + 1;
            let rows = hi[0] - lo[0] + 1;
            let mut mask = vec![false; rows * width];
            for i in lo[0]..=hi[0] {
                for j in lo[1]..=hi[1] {
                    mask[(i - lo[0]) * width + (j - lo[1])] =
                        placed.grid.at([i, j]).iter().any(|s| {
                            s.hi > feet + STEP && s.lo < feet + HEAD && placed.stands(s.part)
                        });
                }
            }
            // Greedy rectangles: a run along z, then as many rows along x
            // as are blocked over the same run.
            for i in 0..rows {
                let mut j = 0;
                while j < width {
                    if !mask[i * width + j] {
                        j += 1;
                        continue;
                    }
                    let mut end = j;
                    while end + 1 < width && mask[i * width + end + 1] {
                        end += 1;
                    }
                    let mut last = i;
                    while last + 1 < rows && (j..=end).all(|k| mask[(last + 1) * width + k]) {
                        last += 1;
                    }
                    for r in i..=last {
                        for k in j..=end {
                            mask[r * width + k] = false;
                        }
                    }
                    let a = placed.grid.square([lo[0] + i, lo[1] + j]);
                    let b = placed.grid.square([lo[0] + last, lo[1] + end]);
                    out.push(Footprint {
                        min: a.min,
                        max: b.max,
                    });
                    j = end + 1;
                }
            }
        }
        out
    }

    /// The footprints that block feet at `feet`: those whose top is more
    /// than a step above them. Columns are left out; they block only near
    /// a point ([`Self::blocking_near`]).
    #[must_use]
    pub fn blocking(&self, feet: f32) -> Vec<Footprint> {
        self.blocks
            .iter()
            .chain(&self.spell)
            .filter(|block| block.top > feet + STEP)
            .map(|block| block.footprint)
            .collect()
    }

    /// The highest surface at `(x, z)` that feet at `feet` stand on: the
    /// ground, or a block's top or a roof within a step above the feet or
    /// anywhere below them.
    #[must_use]
    pub fn floor(&self, x: f32, z: f32, feet: f32) -> f32 {
        let reach = feet + STEP;
        let inside =
            |f: &Footprint| x >= f.min[0] && x <= f.max[0] && z >= f.min[1] && z <= f.max[1];
        let blocks = self
            .blocks
            .iter()
            .chain(&self.spell)
            .filter(|block| block.top <= reach && inside(&block.footprint))
            .map(|block| block.top);
        let roofs = self
            .roofs
            .iter()
            .filter_map(|roof| roof.surface(x, z))
            .filter(|&surface| surface <= reach);
        let columns = self
            .columns
            .iter()
            .flat_map(|p| p.spans(x, z))
            .map(|s| s.hi)
            .filter(|&hi| hi <= reach);
        blocks
            .chain(roofs)
            .chain(columns)
            .fold(self.ground(x, z), f32::max)
    }

    /// The highest surface at `(x, z)`: the ground, or anything solid
    /// standing on it.
    #[must_use]
    pub fn top(&self, x: f32, z: f32) -> f32 {
        self.floor(x, z, f32::MAX / 2.0)
    }

    /// Walks `player` one controller step over these solids: the
    /// controller sees flat ground at the surface under the feet, the
    /// blockers taller than a step stop it, and a grounded walker that
    /// `settles` keeps to a surface a small step down instead of falling.
    /// `bound` is the walkable square's half extent, m.
    pub fn step(
        &self,
        player: &mut PlayerController,
        input: &InputState,
        dt: f32,
        bound: f32,
        settles: bool,
    ) {
        // Over columns, a long frame walks in short strides.
        let strides = if self.columns.is_empty() {
            1
        } else {
            ((RUN_SPEED * SPRINT_MULT * dt / STRIDE).ceil() as usize).clamp(1, 8)
        };
        let part = dt / strides as f32;
        let mut input = *input;
        for _ in 0..strides {
            self.stride(player, &input, part, bound, settles);
            input.jump = false;
        }
    }

    fn stride(
        &self,
        player: &mut PlayerController,
        input: &InputState,
        dt: f32,
        bound: f32,
        settles: bool,
    ) {
        let feet = player.pos.y;
        let grounded = !player.airborne();
        let floor = self.floor(player.pos.x, player.pos.z, feet);
        let blockers = self.blocking_near(player.pos.x, player.pos.z, NEAR + STRIDE, feet);
        player.pos.y -= floor;
        player.set_surface_height(0.0);
        player.update(input, dt, &blockers, bound);
        player.pos.y += floor;
        let landed = self.floor(player.pos.x, player.pos.z, player.pos.y);
        player.pos.y = player.pos.y.max(landed);
        if grounded && settles {
            // A small step down keeps walking instead of falling.
            player.settle_onto(landed, STEP);
        } else {
            player.set_surface_height(landed);
        }
    }
}
