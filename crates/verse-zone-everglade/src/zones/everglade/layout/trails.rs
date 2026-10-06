//! The eighth round's trails: footpaths from the town's last streets out
//! through the wild ground, the tree ring, and the forest belt toward the
//! zone's edges, where the illustrated map (`docs/verse/everglade-map.svg`)
//! draws its dashed paths: north past Market Way, north-west past the
//! beekeeper's hut, west from Hearth Road, south-west through Walden Woods,
//! south from the farm lane, south-east past Observatory Hill, east from
//! Foundry Road, and north-east out of Fernhollow.
//!
//! Each trail is worn into the turf (`draw::ground` browns the grass along
//! it), so it follows the rising ground past the clearing; inside the flat
//! clearing it is laid with footpath pieces too
//! (`foliage::verdure`). Nothing grows on a trail: the foliage rounds keep
//! off it, and [`clear`] lifts the plants earlier rounds set there.

use super::{Placement, segment_distance};

/// Half a trail's worn width, m.
pub const HALF: f32 = 0.9;

/// The trails, each a polyline from a street's end outward, m.
pub const TRAILS: [&[[f32; 2]]; 8] = [
    // North, past Market Way and the chapel, to the zone's north edge.
    &[
        [16.0, 97.0],
        [13.0, 112.0],
        [8.0, 128.0],
        [11.0, 150.0],
        [4.0, 182.0],
        [9.0, 214.0],
        [3.0, 244.0],
    ],
    // North-west, past the beekeeper's hut and the orchard.
    &[
        [-82.0, 80.0],
        [-92.0, 92.0],
        [-108.0, 104.0],
        [-126.0, 124.0],
        [-150.0, 152.0],
        [-176.0, 186.0],
        [-200.0, 222.0],
    ],
    // West, from Hearth Road's end through the Lantern Quarter's woods.
    &[
        [-118.0, -8.0],
        [-130.0, -6.0],
        [-150.0, -10.0],
        [-178.0, -6.0],
        [-210.0, -12.0],
        [-244.0, -8.0],
    ],
    // South-west, from Lantern Road's end past the Thinking Pond into
    // Walden Woods.
    &[
        [-100.0, -79.0],
        [-104.0, -92.0],
        [-118.0, -110.0],
        [-138.0, -128.0],
        [-162.0, -152.0],
        [-188.0, -182.0],
        [-212.0, -214.0],
    ],
    // South, from the farm lane past the farmhouse.
    &[
        [8.0, -97.0],
        [2.0, -110.0],
        [-3.0, -126.0],
        [2.0, -146.0],
        [-4.0, -176.0],
        [3.0, -208.0],
        [-2.0, -244.0],
    ],
    // South-east, from Library Way's end past Observatory Hill and the long
    // meadow.
    &[
        [105.0, -30.0],
        [118.0, -44.0],
        [128.0, -60.0],
        [146.0, -86.0],
        [168.0, -118.0],
        [190.0, -152.0],
        [214.0, -190.0],
    ],
    // East, from Foundry Road's end.
    &[
        [113.0, 0.0],
        [126.0, 3.0],
        [146.0, -1.0],
        [174.0, 4.0],
        [208.0, -2.0],
        [244.0, 3.0],
    ],
    // North-east, from the fern cabin's path out of Fernhollow.
    &[
        [87.0, 86.0],
        [93.0, 85.0],
        [104.0, 96.0],
        [116.0, 112.0],
        [134.0, 132.0],
        [158.0, 160.0],
        [184.0, 192.0],
        [208.0, 226.0],
    ],
];

/// Distance from `(x, z)` to the nearest trail's middle, m.
#[must_use]
pub fn distance(x: f32, z: f32) -> f32 {
    TRAILS
        .iter()
        .flat_map(|t| t.windows(2))
        .map(|w| segment_distance(w[0], w[1], x, z))
        .fold(f32::INFINITY, f32::min)
}

/// Whether a piece reaching `r` m from `(x, z)` stands on a trail.
#[must_use]
pub fn on_trail(x: f32, z: f32, r: f32) -> bool {
    distance(x, z) < HALF + r
}

/// The plants of earlier rounds that grow on a trail: trees by their
/// trunks and crowns' feet, and the undergrowth.
fn plant_reach(model: &str) -> Option<f32> {
    const PLANTS: [(&str, f32); 14] = [
        ("generated/oak_low", 1.0),
        ("generated/birch_low", 0.9),
        ("generated/spruce_low", 1.1),
        ("generated/poplar_low", 0.8),
        ("generated/bush_round", 1.0),
        ("generated/fruit_tree", 1.0),
        ("generated/fallen_log", 1.6),
        ("generated/stump", 0.6),
        ("generated/mossy_rock", 0.8),
        ("generated/mushrooms", 0.5),
        ("generated/wildflowers", 0.8),
        ("generated/flower_patch_", 1.0),
        ("foliage/", 1.0),
        ("nature/", 1.0),
    ];
    PLANTS
        .iter()
        .find(|(prefix, _)| model.starts_with(prefix))
        .map(|&(_, r)| r)
}

/// Drops every plant from `out` that stands on a trail, so the trails run
/// clear through the woods.
pub fn clear(out: &mut Vec<Placement>) {
    out.retain(|p| plant_reach(p.model).is_none_or(|r| !on_trail(p.at[0], p.at[1], r * p.scale)));
}
