//! Distance levels of detail for Everglade's placements.
//!
//! The pack's `lod` set holds a lighter far level of the city's heaviest
//! models, made by `scripts/blender/everglade_lod.py`: the generated
//! buildings and landmarks, the kit pieces that kit-built houses repeat, and
//! the nature kit's trees and bushes. A placement of such a model draws its
//! own model while its cell is nearer than [`FAR`] and the far level beyond
//! it; small ground cover and furniture draw only nearer than [`COVER`], and
//! the woods' understory and the town's shrubs and ivy only nearer than
//! [`UNDERSTORY`]. The renderer holds each cell at its level until the eye
//! moves [`crate::pbr::textured::HYSTERESIS`] past the switch, so a cell at
//! the switch doesn't flicker. Every distance lies inside the fog, which
//! starts at 40 m and closes completely by 180 m.
//!
//! A far level is placed after every layout placement, in layout order
//! ([`far_placements`]), so a layout placement's index is still its index in
//! the scene, and the town's demolition hides a building's far level with
//! its pieces.

use super::layout::Placement;
use crate::pbr::textured::Detail;
use crate::zones::everglade_pack::ZonePack;

/// Where a model with a far level switches to it, m across the ground: far
/// enough that a house's far level, watertight but plainer, shows only in
/// the haze.
pub(crate) const FAR: f32 = 80.0;
/// Where small ground cover and furniture stop drawing, m across the
/// ground.
pub(crate) const COVER: f32 = 52.0;
/// Where the woods' understory, the town's shrubs and ivy, and other
/// mid-sized foliage stop drawing, m across the ground: far enough that
/// they don't pop in the open, inside the fog's thickest part.
pub(crate) const UNDERSTORY: f32 = 72.0;
/// The scene's switch distances
/// ([`crate::pbr::textured::TexturedScene::switches`]).
pub(crate) const SWITCHES: [f32; 3] = [FAR, COVER, UNDERSTORY];
const FAR_SWITCH: u8 = 0;
const COVER_SWITCH: u8 = 1;
const UNDERSTORY_SWITCH: u8 = 2;

/// Models drawn only nearer than [`UNDERSTORY`], by prefix: shrubs and
/// brambles, deadwood, roots, small rocks and stones, ivy, roses, and
/// hedges.
const UNDERSTORY_MODELS: [&str; 15] = [
    "foliage/shrub_",
    "foliage/bramble",
    "foliage/log_",
    "foliage/stump_",
    "foliage/root_arch",
    "foliage/snag",
    "foliage/boulder_cluster",
    "foliage/standing_stone",
    "foliage/ivy_",
    "foliage/rose_trellis",
    "foliage/hedge_",
    "foliage/campfire",
    "generated/fallen_log",
    "generated/stump",
    "generated/mossy_rock",
];

/// Models drawn only nearer than [`COVER`], by prefix: grass, ferns,
/// flowers, mushrooms, and stepping stones, the flower beds and boxes, and
/// the station furniture and small props, which are a few pixels tall
/// beyond it.
const COVER_MODELS: [&str; 31] = [
    "foliage/fern_clump",
    "foliage/grass_tall",
    "foliage/wildflower_clump",
    "foliage/mushroom_ring",
    "foliage/roots_spread",
    "foliage/window_box",
    "foliage/planter_overflow",
    "nature/Grass_",
    "nature/Fern_",
    "nature/Clover_",
    "nature/Flower_",
    "nature/Mushroom_",
    "nature/Plant_",
    "nature/RockPath_",
    "nature/Pebble",
    "props/",
    "generated/wildflowers",
    "generated/flower_patch_",
    "generated/flower_box",
    "generated/flower_bed",
    "generated/veg_bed",
    "generated/mushrooms",
    "generated/planter",
    "generated/cafe_table",
    "generated/park_bench",
    "generated/barrel",
    "generated/hand_cart",
    "generated/shop_sign",
    "generated/sundial",
    "generated/beehives",
    "generated/hay_bales",
];

/// The pack's name for the far level of `model`: `lod/<set>.<name>`.
#[must_use]
pub(crate) fn far_name(model: &str) -> String {
    format!("lod/{}", model.replacen('/', ".", 1))
}

/// How `model` draws: its own detail, and the pack's far level that
/// stands in for it beyond [`FAR`], if any.
pub(crate) fn plan<'p>(pack: &'p ZonePack, model: &str) -> (Detail, Option<&'p str>) {
    if let Some(far) = pack.model(&far_name(model)) {
        return (Detail::Near(FAR_SWITCH), Some(far.name.as_str()));
    }
    if COVER_MODELS.iter().any(|prefix| model.starts_with(prefix)) {
        return (Detail::Near(COVER_SWITCH), None);
    }
    if UNDERSTORY_MODELS
        .iter()
        .any(|prefix| model.starts_with(prefix))
    {
        return (Detail::Near(UNDERSTORY_SWITCH), None);
    }
    (Detail::Always, None)
}

/// The far level placed for each placement, if any: its model and its
/// index in the scene, which follows every layout placement in order.
pub(crate) fn far_placements<'p>(
    pack: &'p ZonePack,
    placements: &[Placement],
) -> Vec<Option<(&'p str, usize)>> {
    let mut next = placements.len();
    placements
        .iter()
        .map(|p| {
            plan(pack, p.model).1.map(|far| {
                next += 1;
                (far, next - 1)
            })
        })
        .collect()
}

/// The far level of detail a placement draws as, at [`FAR`].
pub(crate) const FAR_DETAIL: Detail = Detail::Far(FAR_SWITCH);
