//! What a character stands on and runs into in Everglade, with height: each
//! blocker's footprint and top, and each round-tile roof's gabled surface.
//! A character whose feet are above a blocker's top passes over it, and the
//! highest surface under the feet holds it up, so a levitating character
//! comes down onto a roof rather than through it.

use super::height;
use super::layout::{self, Collision, Placement};
use crate::controller::Footprint;
use crate::zones::everglade_pack::ZonePack;
use glam::Vec3;

/// How far the feet may be below a surface and still step onto it, m.
pub const STEP: f32 = 0.35;

#[derive(Clone, Copy, Debug)]
struct Block {
    footprint: Footprint,
    top: f32,
}

/// A gabled roof: its ridge runs along the model's z axis and the slopes
/// fall along x, from `ridge` at the center to `eave` at `half` either side.
#[derive(Clone, Copy, Debug)]
struct Roof {
    center: [f32; 2],
    /// The model's +x axis on the ground.
    across: [f32; 2],
    half: [f32; 2],
    eave: f32,
    ridge: f32,
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

/// Everglade's solids.
#[derive(Clone, Debug, Default)]
pub struct Solids {
    blocks: Vec<Block>,
    roofs: Vec<Roof>,
    /// Blocks a spell raised, such as Wall of Stone's panels; replaced
    /// whole by [`Solids::set_spell_blocks`].
    spell: Vec<Block>,
}

impl Solids {
    /// The solids of `placements` with the models in `pack`, and the boards,
    /// which are taller than anyone levitates past them.
    pub fn build(pack: &ZonePack, placements: &[Placement]) -> Result<Self, String> {
        let mut solids = Self::default();
        for placement in placements {
            let model = pack
                .model(placement.model)
                .ok_or_else(|| format!("The Everglade pack has no {}", placement.model))?;
            let (min, max) = model.bounds();
            let base = height(placement.at[0], placement.at[1]) + placement.lift;
            if placement.collision != Collision::None {
                let top = base + max[1] * placement.scale;
                solids.blocks.extend(
                    placement
                        .footprints((min, max))
                        .into_iter()
                        .map(|footprint| Block { footprint, top }),
                );
            } else if placement.model.starts_with("village/Roof_RoundTiles") {
                let across = placement.transform().transform_vector3(Vec3::X).normalize();
                solids.roofs.push(Roof {
                    center: placement.at,
                    across: [across.x, across.z],
                    half: [
                        min[0].abs().max(max[0].abs()) * placement.scale,
                        min[2].abs().max(max[2].abs()) * placement.scale,
                    ],
                    eave: base + min[1] * placement.scale,
                    ridge: base + max[1] * placement.scale,
                });
            }
        }
        solids
            .blocks
            .extend(layout::board_blockers().into_iter().map(|footprint| Block {
                footprint,
                top: f32::INFINITY,
            }));
        Ok(solids)
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
        blocks.chain(roofs).fold(height(x, z), f32::max)
    }
}
