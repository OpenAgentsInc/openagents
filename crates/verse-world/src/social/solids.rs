//! What a character stands on and runs into in a social world, with
//! height: each blocker's footprint and top, and each gabled roof's
//! surface, over the zone's heightfield. A character whose feet are above a
//! blocker's top passes over it, and the highest surface under the feet
//! holds it up, so a levitating character comes down onto a roof rather
//! than through it. Verse's Everglade zone builds these from its pinned
//! pack, and a hosted instance walks avatars over the same values.

use super::controller::{Footprint, InputState, PlayerController};

/// How far the feet may be below a surface and still step onto it, m.
pub const STEP: f32 = 0.35;

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
}

impl Default for Solids {
    fn default() -> Self {
        Self::over(super::everglade::height)
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
        }
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
        self.roofs
            .iter()
            .filter_map(|roof| roof.surface(x, z))
            .filter(|&surface| surface >= head)
            .min_by(f32::total_cmp)
    }

    /// The footprints that block feet at `feet`: those whose top is more
    /// than a step above them.
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
        blocks.chain(roofs).fold(self.ground(x, z), f32::max)
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
        let feet = player.pos.y;
        let grounded = !player.airborne();
        let floor = self.floor(player.pos.x, player.pos.z, feet);
        let blockers = self.blocking(feet);
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
