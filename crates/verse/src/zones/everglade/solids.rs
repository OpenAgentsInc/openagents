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

/// The solids of `placements` with the models in `pack`, the city's walls,
/// and the boards and the ponds, which are taller than anyone levitates
/// past them.
///
/// # Errors
///
/// Returns a message when the pack lacks a placed model.
pub fn build(pack: &ZonePack, placements: &[Placement]) -> Result<Solids, String> {
    // The ponds are walls to walking, like the boards: nothing stands on
    // their water.
    let mut walls = layout::board_blockers();
    walls.extend(layout::pond_blockers());
    let mut solids = build_with(pack, placements, &walls)?;
    // The city's walls, one block per run as tall as their stories, and the generated models' boxes and roofs.
    for (footprint, top) in layout::city::blocks() {
        solids.add_block(footprint, top);
    }
    for roof in layout::generated().iter().flat_map(|i| i.roofs()) {
        solids.add_roof(roof);
    }
    // The footbridge's deck, plank by plank, low enough to step onto.
    for (footprint, top) in layout::bridge_steps() {
        solids.add_block(footprint, top);
    }
    Ok(solids)
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
        let (blocks, roof) = of_placement(pack, placement)?;
        for (footprint, top) in blocks {
            solids.add_block(footprint, top);
        }
        if let Some(roof) = roof {
            solids.add_roof(roof);
        }
    }
    for &footprint in boards {
        solids.add_block(footprint, f32::INFINITY);
    }
    Ok(solids)
}

/// What one placement adds to the solids with the models in `pack`: its
/// blocks, each a footprint and its top, and a round-tile roof's surface.
///
/// # Errors
///
/// Returns a message when the pack lacks the placed model.
pub fn of_placement(
    pack: &ZonePack,
    placement: &Placement,
) -> Result<(Vec<(crate::controller::Footprint, f32)>, Option<Roof>), String> {
    let model = pack
        .model(placement.model)
        .ok_or_else(|| format!("The Everglade pack has no {}", placement.model))?;
    let (min, max) = model.bounds();
    let base = height(placement.at[0], placement.at[1]) + placement.lift;
    if placement.collision != Collision::None {
        let top = base + max[1] * placement.scale;
        let blocks = placement
            .footprints((min, max))
            .into_iter()
            .map(|footprint| (footprint, top))
            .collect();
        return Ok((blocks, None));
    }
    if placement.model.starts_with("village/Roof_RoundTiles")
        || placement.model == layout::HOUSE_ROOF
    {
        let across = placement.transform().transform_vector3(Vec3::X).normalize();
        return Ok((
            Vec::new(),
            Some(Roof {
                center: placement.at,
                across: [across.x, across.z],
                half: [
                    min[0].abs().max(max[0].abs()) * placement.scale,
                    min[2].abs().max(max[2].abs()) * placement.scale,
                ],
                eave: base + min[1] * placement.scale,
                ridge: base + max[1] * placement.scale,
            }),
        ));
    }
    Ok((Vec::new(), None))
}
