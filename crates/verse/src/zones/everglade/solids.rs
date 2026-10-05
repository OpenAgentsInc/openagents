//! What a character stands on and runs into in Everglade, with height: each
//! blocker's footprint and top, and each round-tile roof's gabled surface.
//! The solids themselves live in `verse_world::social::solids`, so a hosted
//! instance walks avatars over the same values; this module builds them
//! from the pinned pack's models.

use super::height;
use super::layout::{self, Collision, Placement};
use crate::zones::everglade_pack::ZonePack;
use glam::Vec3;
pub use verse_world::social::solids::{Roof, STEP, Solids};

/// The solids of `placements` with the models in `pack`, and the boards
/// and the ponds, which are taller than anyone levitates past them.
///
/// # Errors
///
/// Returns a message when the pack lacks a placed model.
pub fn build(pack: &ZonePack, placements: &[Placement]) -> Result<Solids, String> {
    // The ponds are walls to walking, like the boards: nothing stands on
    // their water.
    let mut walls = layout::board_blockers();
    walls.extend(layout::pond_blockers());
    build_with(pack, placements, &walls)
}

/// The solids of `placements` with the models in `pack`, and `boards`,
/// footprints taller than anyone levitates past.
///
/// # Errors
///
/// Returns a message when the pack lacks a placed model.
pub fn build_with(
    pack: &ZonePack,
    placements: &[Placement],
    boards: &[crate::controller::Footprint],
) -> Result<Solids, String> {
    let mut solids = Solids::over(height);
    for placement in placements {
        let model = pack
            .model(placement.model)
            .ok_or_else(|| format!("The Everglade pack has no {}", placement.model))?;
        let (min, max) = model.bounds();
        let base = height(placement.at[0], placement.at[1]) + placement.lift;
        if placement.collision != Collision::None {
            let top = base + max[1] * placement.scale;
            for footprint in placement.footprints((min, max)) {
                solids.add_block(footprint, top);
            }
        } else if placement.model.starts_with("village/Roof_RoundTiles") {
            let across = placement.transform().transform_vector3(Vec3::X).normalize();
            solids.add_roof(Roof {
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
    for &footprint in boards {
        solids.add_block(footprint, f32::INFINITY);
    }
    Ok(solids)
}
