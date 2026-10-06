//! The seventh round: wild growth and trails (`scripts/blender/town_props.py`,
//! `scripts/blender/town_houses.py`). Wild ground between the town and the
//! woods, thick with thickets, young trees, shrubs, and tall grass in
//! patches; trees along the outer lanes toward the woods; foliage
//! interleaved in the narrow gaps between buildings; footpaths worn from
//! the town's edge toward the woods; garden sheds in back gardens; and
//! woodsheds by the cabins.
//!
//! Every piece goes through [`Ground`], as the fifth round's do, so it
//! stands on open ground and apart from what stands there. Thickets,
//! shrubs, cover, copses, and footpaths don't block walking; young trees
//! block at their middle trunk, and the sheds at their walls.

use super::super::super::height;
use super::super::city::BUILDINGS;
use super::super::{Collision, Placement, noise, segment_distance};
use super::{BELT, COVER, Ground, SHRUBS, paved, thickness, tree};
use crate::controller::forward;
use std::f32::consts::TAU;

/// The thicket: bushes round two saplings, about 3 m across.
pub const THICKET: &str = "generated/thicket";
/// A young birch, spruce, and oak a meter or so apart.
pub const YOUNG_TREES: &str = "generated/young_trees";
/// Six young trees, two each of birch, spruce, and oak, 5 m across.
pub const COPSE: &str = "generated/copse";
/// Three meters of worn footpath.
pub const FOOTPATH: &str = "generated/footpath";
/// A plastered garden shed; its origin is the middle of its front wall,
/// and it reaches [`SHED_DEPTH`] behind it.
pub const GARDEN_SHED: &str = "generated/garden_shed";
/// An open woodshed over stacked logs, with the same origin.
pub const WOODSHED: &str = "generated/woodshed";
const SHED_DEPTH: f32 = 2.6;

/// The wild ground's inner and outer radius, m: from the town's last
/// streets to the woods' edge.
const WILD: (f32, f32) = (92.0, 150.0);
/// Patches of wild growth round the town, and the reach of each, m.
const PATCHES: u32 = 90;

/// The sheds, placed before the fifth round's foliage fills the gardens.
pub(super) fn outbuildings(out: &mut Vec<Placement>, ground: &mut Ground) {
    woodsheds(out, ground);
    garden_sheds(out, ground);
}

/// The rest of the seventh round, after the fifth's foliage.
pub(super) fn build(out: &mut Vec<Placement>, ground: &mut Ground) {
    trails(out, ground);
    lane_trees(out, ground);
    patches(out, ground);
    interleave(out, ground);
    meadow(out, ground);
}

/// The yaw that turns a model's front toward `(dx, dz)`.
fn facing(dx: f32, dz: f32) -> f32 {
    dx.atan2(dz)
}

/// Places a shed `model` with its front at `at`, facing `yaw`, if the
/// ground under its whole footprint is open.
fn shed(
    out: &mut Vec<Placement>,
    ground: &mut Ground,
    model: &'static str,
    at: [f32; 2],
    yaw: f32,
) -> bool {
    let f = forward(yaw);
    let center = [
        at[0] - f.x * SHED_DEPTH / 2.0,
        at[1] - f.z * SHED_DEPTH / 2.0,
    ];
    // Before its door, too, so a walker can reach it.
    let door = [at[0] + f.x * 1.2, at[1] + f.z * 1.2];
    let r = 1.9;
    if ground.crowded(center, r)
        || !ground.open(center[0], center[1], r)
        || !ground.open(door[0], door[1], 0.6)
        || ground.crowded(door, 0.6)
    {
        return false;
    }
    ground.take(center, r);
    ground.take(door, 0.6);
    out.push(Placement::new(model, at, yaw, Collision::Bounds));
    true
}

/// A woodshed beside each cabin, hut, and cottage, at its side or back.
fn woodsheds(out: &mut Vec<Placement>, ground: &mut Ground) {
    for (k, b) in BUILDINGS.iter().enumerate() {
        let rural = ["cabin", "hut", "cottage", "shed", "farmhouse", "windmill"]
            .iter()
            .any(|w| b.name.contains(w));
        if !rural {
            continue;
        }
        let ([x, z], [hx, hz]) = b.rect;
        // Beside each side wall and behind, the shed's back to the wall.
        let tries = [
            ([x - hx - 1.6 - SHED_DEPTH, z], [-1.0, 0.0]),
            ([x + hx + 1.6 + SHED_DEPTH, z], [1.0, 0.0]),
            ([x, z + hz + 1.6 + SHED_DEPTH], [0.0, 1.0]),
            ([x, z - hz - 1.6 - SHED_DEPTH], [0.0, -1.0]),
        ];
        let start = (noise(k as u32, 900) * 4.0) as usize;
        for i in 0..4 {
            let (at, [dx, dz]) = tries[(start + i) % 4];
            if shed(out, ground, WOODSHED, at, facing(dx, dz)) {
                break;
            }
        }
    }
}

/// Garden sheds in the back gardens: on open ground a few meters behind
/// a house, their doors toward it, no two within 16 m.
fn garden_sheds(out: &mut Vec<Placement>, ground: &mut Ground) {
    const MOST: usize = 16;
    let mut placed: Vec<[f32; 2]> = Vec::new();
    for (k, b) in BUILDINGS.iter().enumerate() {
        if placed.len() >= MOST {
            break;
        }
        let ([x, z], [hx, hz]) = b.rect;
        if noise(k as u32, 902) < 0.25 {
            continue;
        }
        // The garden lies behind the wall opposite the door; the shed's
        // door faces the house.
        let ([dx, dz], reach) = match b.door {
            super::super::city::Side::South => ([0.0, -1.0], hz),
            super::super::city::Side::North => ([0.0, 1.0], hz),
            super::super::city::Side::West => ([-1.0, 0.0], hx),
            super::super::city::Side::East => ([1.0, 0.0], hx),
        };
        // To one side of the garden first, so the back door stays clear.
        let side = if noise(k as u32, 901) < 0.5 {
            1.0
        } else {
            -1.0
        };
        'tries: for depth in [4.0_f32, 5.5, 7.0] {
            for across in [2.4 * side, -2.4 * side, 0.0] {
                let back = reach + depth;
                let at = [x - dx * back + dz * across, z - dz * back + dx * across];
                if at[0].hypot(at[1]) > WILD.0
                    || placed
                        .iter()
                        .any(|p| (p[0] - at[0]).hypot(p[1] - at[1]) < 16.0)
                    || paved(at[0], at[1], 1.0)
                {
                    continue;
                }
                if shed(out, ground, GARDEN_SHED, at, facing(dx, dz)) {
                    placed.push(at);
                    break 'tries;
                }
            }
        }
    }
}

/// Footpaths worn from the town's edge across the wild ground to the
/// woods, each wandering a little, with ferns and grass along it. A piece
/// is laid only where the ground under both its ends is level with its
/// middle, inside the flat clearing.
fn trails(out: &mut Vec<Placement>, ground: &mut Ground) {
    const TRAILS: u32 = 9;
    const STEP: f32 = 2.9;
    for t in 0..TRAILS {
        let base = (t as f32 + 0.3 + 0.4 * noise(t, 910)) / TRAILS as f32 * TAU;
        let mut r = WILD.0 + 6.0;
        let mut laid = 0;
        while r < 134.0 {
            let n = t * 64 + laid;
            let a = base + 0.06 * (r * 0.09 + t as f32).sin();
            let at = [a.cos() * r, a.sin() * r];
            // The path runs outward, bending with the wander.
            let ahead = a + 0.06 * 0.09 * STEP * ((r + STEP) * 0.09 + t as f32).cos();
            let dir = [ahead.cos(), ahead.sin()];
            let yaw = facing(dir[0], dir[1]);
            let ends = [
                [at[0] - dir[0] * 1.5, at[1] - dir[1] * 1.5],
                [at[0] + dir[0] * 1.5, at[1] + dir[1] * 1.5],
            ];
            let h = height(at[0], at[1]);
            let level = ends.iter().all(|e| (height(e[0], e[1]) - h).abs() < 0.03);
            if level
                && ends
                    .iter()
                    .all(|e| ground.open(e[0], e[1], 0.5) && !ground.crowded(*e, 0.4))
                && !paved(at[0], at[1], 0.8)
            {
                out.push(Placement::new(FOOTPATH, at, yaw, Collision::None).lift(0.025));
                for e in ends {
                    ground.take(e, 0.6);
                }
                ground.take(at, 0.7);
                // Ferns and tall grass at its edges now and then.
                for side in [1.0_f32, -1.0] {
                    if noise(n, 911 + u32::from(side > 0.0)) < 0.25 {
                        let off = 1.3 + 0.4 * noise(n, 913);
                        let p = [at[0] - dir[1] * off * side, at[1] + dir[0] * off * side];
                        let model = COVER[(noise(n, 914) * 3.0) as usize % 3];
                        let piece = Placement::new(model, p, noise(n, 915) * TAU, Collision::None);
                        ground.put(out, piece, 0.45);
                    }
                }
            }
            laid += 1;
            r += STEP;
        }
    }
}

/// Trees along the lanes that leave the town for the woods: young trees
/// and the belt's cheap trees, alternating sides, every 9 m or so.
fn lane_trees(out: &mut Vec<Placement>, ground: &mut Ground) {
    for (k, &(a, b, half)) in super::super::roads().iter().enumerate() {
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        if length < 10.0 {
            continue;
        }
        let (tx, tz) = ((b[0] - a[0]) / length, (b[1] - a[1]) / length);
        let mut along = 4.0;
        let mut i = 0_u32;
        while along < length {
            let n = k as u32 * 256 + i;
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let off = half + 2.6 + 0.8 * noise(n, 920);
            let at = [
                a[0] + tx * along - tz * off * side,
                a[1] + tz * along + tx * off * side,
            ];
            let r = at[0].hypot(at[1]);
            if r > WILD.0 - 4.0 && !paved(at[0], at[1], 1.0) {
                if noise(n, 921) < 0.55 {
                    let piece =
                        Placement::new(YOUNG_TREES, at, noise(n, 922) * TAU, Collision::Core(0.12))
                            .scale(0.9 + 0.3 * noise(n, 923));
                    ground.put(out, piece, 1.6);
                } else {
                    let kind = BELT[(noise(n, 924) * 3.0) as usize % 3];
                    tree(
                        out,
                        ground,
                        kind,
                        at,
                        n + 9000,
                        0.9 + 0.4 * noise(n, 925),
                        false,
                    );
                }
            }
            along += 8.0 + 3.0 * noise(n, 926);
            i += 1;
        }
    }
}

/// Patches of wild growth on the ground between the town and the woods:
/// young trees among thickets, shrubs, and brambles, with open meadow
/// between the patches. Young trees, three to six to a model at 40 to 50
/// triangles a tree, are most of each patch: they show from across the
/// town, where the understory has stopped drawing.
fn patches(out: &mut Vec<Placement>, ground: &mut Ground) {
    for p in 0..PATCHES {
        let a = (p as f32 + noise(p, 930)) / PATCHES as f32 * TAU;
        let r = WILD.0 + (WILD.1 - WILD.0) * noise(p, 931).powf(1.3);
        let center = [a.cos() * r, a.sin() * r];
        // Walden Woods and Fernhollow are thick already.
        if thickness(center[0], center[1]) > 0.3 {
            continue;
        }
        let reach = 6.0 + 7.0 * noise(p, 932);
        let count = 8 + (noise(p, 933) * 9.0) as u32;
        for i in 0..count {
            let n = p * 32 + i;
            let t = noise(n, 934) * TAU;
            let d = reach * noise(n, 935).sqrt();
            let at = [center[0] + t.cos() * d, center[1] + t.sin() * d];
            let yaw = noise(n, 936) * TAU;
            let pick = noise(n, 937);
            if pick < 0.25 {
                let piece = Placement::new(YOUNG_TREES, at, yaw, Collision::Core(0.12))
                    .scale(0.85 + 0.4 * noise(n, 938));
                ground.put(out, piece, 1.5);
            } else if pick < 0.62 {
                let piece = Placement::new(COPSE, at, yaw, Collision::None)
                    .scale(0.85 + 0.4 * noise(n, 944));
                ground.put(out, piece, 2.4);
            } else if pick < 0.8 {
                let piece = Placement::new(THICKET, at, yaw, Collision::None)
                    .scale(0.8 + 0.5 * noise(n, 939));
                ground.put(out, piece, 1.6);
            } else {
                let (model, reach) = SHRUBS[(noise(n, 940) * 4.0) as usize % 4];
                let scale = 0.8 + 0.5 * noise(n, 941);
                let piece = Placement::new(model, at, yaw, Collision::None).scale(scale);
                ground.put(out, piece, reach * scale * 0.6);
            }
        }
    }
}

/// Foliage in the narrow gaps between buildings, which the fifth round's
/// yards leave bare: shrubs and small thickets against the walls.
fn interleave(out: &mut Vec<Placement>, ground: &mut Ground) {
    const STEP: f32 = 2.4;
    let span = (WILD.0 / STEP) as i32;
    for i in -span..=span {
        for j in -span..=span {
            let n = ((i + 200) * 1000 + j + 200) as u32;
            let x = i as f32 * STEP + STEP * (noise(n, 950) - 0.5);
            let z = j as f32 * STEP + STEP * (noise(n, 951) - 0.5);
            if x.hypot(z) > WILD.0 || thickness(x, z) > 0.0 {
                continue;
            }
            let d = ground.to_building(x, z);
            if !(0.6..2.4).contains(&d) || paved(x, z, 0.5) {
                continue;
            }
            let pick = noise(n, 952);
            let yaw = noise(n, 953) * TAU;
            if pick < 0.12 {
                let piece = Placement::new(THICKET, [x, z], yaw, Collision::None).scale(0.55);
                ground.put(out, piece, 0.9);
            } else if pick < 0.42 {
                let (model, reach) = SHRUBS[(noise(n, 954) * 3.0) as usize % 3];
                let scale = 0.6 + 0.3 * noise(n, 955);
                let piece = Placement::new(model, [x, z], yaw, Collision::None).scale(scale);
                ground.put(out, piece, reach * scale * 0.6);
            }
        }
    }
}

/// Tall grass and wildflowers scattered over the open meadow between the
/// patches, thinning toward the town.
fn meadow(out: &mut Vec<Placement>, ground: &mut Ground) {
    const TRIES: u32 = 40;
    for n in 0..TRIES {
        let a = noise(n, 960) * TAU;
        let r = WILD.0 + (WILD.1 - WILD.0) * noise(n, 961);
        let at = [a.cos() * r, a.sin() * r];
        if noise(n, 962) > (r - WILD.0) / 30.0 || paved(at[0], at[1], 0.5) {
            continue;
        }
        let near_road = super::super::roads()
            .iter()
            .any(|&(p, q, half)| segment_distance(p, q, at[0], at[1]) < half + 1.0);
        if near_road {
            continue;
        }
        let model = COVER[1 + (noise(n, 963) * 2.0) as usize % 2];
        let piece = Placement::new(model, at, noise(n, 964) * TAU, Collision::None)
            .scale(0.9 + 0.5 * noise(n, 965));
        ground.put(out, piece, 0.5);
    }
}
