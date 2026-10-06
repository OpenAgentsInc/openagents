//! The eighth round: a great deal more foliage, and trails to the zone's
//! edges (`scripts/blender/foliage.py`, `scripts/blender/town_houses.py`).
//!
//! - The trails ([`super::super::trails`]) are laid with footpath pieces
//!   across the flat clearing, edged with ferns and grass, and lined
//!   with undergrowth, waymark stones, and young trees out through the
//!   woods. A trail shelter with a bench stands where each meets the
//!   clearing's edge, and the north trail passes the wayside chapel
//!   (`city::GROUNDS`).
//! - Groves on the wild ground and up the rising ground to the tree ring:
//!   birch trios, rowans, hazels, elders in flower, and fern banks among
//!   copses, thickets, and young trees.
//! - The forest floor past the tree ring, thickest beside the trails.
//! - Elders, hazels, ferns, and small trees in the gaps between the
//!   town's buildings and in its back yards.
//!
//! Every piece goes through [`Ground`], so it stands on open ground, off
//! the roads and the trails, and apart from what stands there. Trees block
//! walking at their trunks; shrubs, ferns, and the footpaths don't. The
//! foliage set's pieces stay whole; the seventh round's copses, thickets,
//! and young trees, the footpaths, and the shelters break as before
//! (`demolition::carve`).

use super::super::super::height;
use super::super::trails::{self, TRAILS};
use super::super::{Collision, Placement, noise};
use super::wilds::{COPSE, FOOTPATH, THICKET, YOUNG_TREES};
use super::{COVER, Ground, SHRUBS, paved, thickness, tree};
use std::f32::consts::{FRAC_PI_2, TAU};

/// Three slender birches from one root, with the half width of their
/// trunks and their crowns' reach, m.
pub const BIRCH_TRIO: (&str, f32, f32) = ("foliage/birch_trio", 0.35, 2.4);
/// A rowan in berry.
pub const ROWAN: (&str, f32, f32) = ("foliage/rowan", 0.2, 2.0);
/// A hazel's fan of stems.
pub const HAZEL: &str = "foliage/shrub_hazel";
/// An elder in flower.
pub const ELDER: &str = "foliage/shrub_elder";
/// A drift of ferns among grass, 3 m across.
pub const FERN_BANK: &str = "foliage/fern_bank";
/// The open shelter with a bench where a trail meets the woods.
pub const SHELTER: &str = "generated/trail_shelter";
/// A waymark stone beside a trail.
const WAYMARK: &str = "foliage/standing_stone_squat";

/// The wild ground's groves: their inner and outer radius, m, from the
/// town's last streets up the rising ground to the tree ring.
const GROVES: (f32, f32) = (92.0, 178.0);
/// How many groves.
const GROVE_COUNT: u32 = 60;
/// The forest floor past the tree ring: inner and outer radius, m.
const FLOOR: (f32, f32) = (176.0, 236.0);

/// Every placement of the eighth round, after the seventh's.
pub(super) fn build(out: &mut Vec<Placement>, ground: &mut Ground) {
    trails(out, ground);
    groves(out, ground);
    floor(out, ground);
    between(out, ground);
}

/// The yaw that turns a model's front toward `(dx, dz)`.
fn facing(dx: f32, dz: f32) -> f32 {
    dx.atan2(dz)
}

/// One undergrowth piece by draw `pick` in 0..1, at `at`, as `n` seeds it:
/// mostly shrubs and ferns, with a young tree or a birch trio now and
/// then. Returns whether it stood.
fn undergrowth(
    out: &mut Vec<Placement>,
    ground: &mut Ground,
    at: [f32; 2],
    n: u32,
    pick: f32,
) -> bool {
    let yaw = noise(n, 1310) * TAU;
    let scale = 0.8 + 0.4 * noise(n, 1311);
    let piece =
        |model: &'static str, collision| Placement::new(model, at, yaw, collision).scale(scale);
    if pick < 0.08 {
        tree(out, ground, BIRCH_TRIO, at, n + 13_000, scale, false)
    } else if pick < 0.14 {
        tree(out, ground, ROWAN, at, n + 13_000, scale, false)
    } else if pick < 0.3 {
        ground.put(out, piece(HAZEL, Collision::None), 1.4 * scale)
    } else if pick < 0.42 {
        ground.put(out, piece(ELDER, Collision::None), 1.3 * scale)
    } else if pick < 0.62 {
        ground.put(out, piece(FERN_BANK, Collision::None), 1.5 * scale)
    } else if pick < 0.72 {
        ground.put(out, piece(YOUNG_TREES, Collision::Core(0.12)), 1.5 * scale)
    } else if pick < 0.8 {
        ground.put(out, piece(COPSE, Collision::None), 2.4 * scale)
    } else if pick < 0.86 {
        ground.put(out, piece(THICKET, Collision::None), 1.6 * scale)
    } else {
        let (model, reach) = SHRUBS[(noise(n, 1312) * 4.0) as usize % 4];
        ground.put(out, piece(model, Collision::None), reach * scale * 0.6)
    }
}

/// The trails: footpath pieces where the ground is flat, ferns and grass
/// at their edges, undergrowth and waymark stones along them through the
/// woods, and a shelter where each meets the clearing's edge.
fn trails(out: &mut Vec<Placement>, ground: &mut Ground) {
    const STEP: f32 = 2.9;
    for (t, trail) in TRAILS.iter().enumerate() {
        let t = t as u32;
        let mut laid = 0_u32;
        let mut sheltered = t == 0;
        let mut marked = 0.0_f32;
        let mut walked = 0.0_f32;
        for w in trail.windows(2) {
            let (a, b) = (w[0], w[1]);
            let length = (b[0] - a[0]).hypot(b[1] - a[1]);
            let dir = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
            let side = [-dir[1], dir[0]];
            let mut along = STEP / 2.0;
            while along < length {
                let n = t * 512 + laid;
                laid += 1;
                let at = [a[0] + dir[0] * along, a[1] + dir[1] * along];
                along += STEP;
                walked += STEP;
                let r = at[0].hypot(at[1]);
                let ends = [
                    [at[0] - dir[0] * 1.5, at[1] - dir[1] * 1.5],
                    [at[0] + dir[0] * 1.5, at[1] + dir[1] * 1.5],
                ];
                let flat = ends
                    .iter()
                    .chain(std::iter::once(&at))
                    .all(|e| height(e[0], e[1]).abs() < 0.02);
                if flat
                    && ends.iter().all(|e| ground.clear(e[0], e[1], 0.4))
                    && !paved(at[0], at[1], 0.8)
                {
                    out.push(
                        Placement::new(FOOTPATH, at, facing(dir[0], dir[1]), Collision::None)
                            .lift(0.025),
                    );
                }
                // The shelter, beside the trail where it meets the woods.
                if !sheltered && r > 108.0 && r < 132.0 {
                    for s in [1.0_f32, -1.0] {
                        let spot = [at[0] + side[0] * 4.0 * s, at[1] + side[1] * 4.0 * s];
                        let yaw = facing(-side[0] * s, -side[1] * s);
                        let shelter = Placement::new(SHELTER, spot, yaw, Collision::None);
                        let level = height(spot[0], spot[1]).abs() < 0.02;
                        if level && ground.put(out, shelter, 2.2) {
                            sheltered = true;
                            break;
                        }
                    }
                }
                // Undergrowth both sides: sparse in the clearing, thick in
                // the woods.
                let wild = ((r - 100.0) / 80.0).clamp(0.1, 0.6);
                for (k, s) in [1.0_f32, -1.0].into_iter().enumerate() {
                    let m = n * 2 + k as u32;
                    if noise(m, 1321) > wild {
                        continue;
                    }
                    let off = trails::HALF + 1.2 + 4.5 * noise(m, 1322);
                    let p = [
                        at[0] + side[0] * off * s + dir[0] * (noise(m, 1323) - 0.5) * 2.0,
                        at[1] + side[1] * off * s + dir[1] * (noise(m, 1323) - 0.5) * 2.0,
                    ];
                    if off < trails::HALF + 2.0 {
                        let model = COVER[(noise(m, 1324) * 3.0) as usize % 3];
                        let piece = Placement::new(model, p, noise(m, 1325) * TAU, Collision::None);
                        ground.put(out, piece, 0.45);
                    } else {
                        undergrowth(out, ground, p, m + 20_000, noise(m, 1326));
                    }
                }
                // A waymark stone every 30 m or so out in the woods.
                if r > 140.0 && walked - marked > 30.0 {
                    let s = if laid % 2 == 0 { 1.0 } else { -1.0 };
                    let p = [
                        at[0] + side[0] * (trails::HALF + 0.8) * s,
                        at[1] + side[1] * (trails::HALF + 0.8) * s,
                    ];
                    let stone =
                        Placement::new(WAYMARK, p, noise(n, 1327) * TAU, Collision::Core(0.3))
                            .scale(0.45)
                            .lift(-0.05);
                    if ground.put(out, stone, 0.6) {
                        marked = walked;
                    }
                }
            }
        }
    }
}

/// Groves on the wild ground and up the rising ground to the tree ring:
/// each a cluster of birch trios, rowans, hazels, elders, fern banks,
/// copses, thickets, and young trees, with meadow between.
fn groves(out: &mut Vec<Placement>, ground: &mut Ground) {
    for g in 0..GROVE_COUNT {
        let a = (g as f32 + noise(g, 1330)) / GROVE_COUNT as f32 * TAU;
        let r = GROVES.0 + (GROVES.1 - GROVES.0) * noise(g, 1331).powf(1.2);
        let center = [a.cos() * r, a.sin() * r];
        if trails::distance(center[0], center[1]) < 5.0 {
            continue;
        }
        // Walden Woods and Fernhollow take fewer, mostly ferns and hazels.
        let thick = thickness(center[0], center[1]);
        let reach = 5.0 + 6.0 * noise(g, 1332);
        let count = 5 + (noise(g, 1333) * 6.0) as u32;
        for i in 0..count {
            let n = g * 32 + i;
            let t = noise(n, 1334) * TAU;
            let d = reach * noise(n, 1335).sqrt();
            let at = [center[0] + t.cos() * d, center[1] + t.sin() * d];
            let pick = if thick > 0.4 {
                0.14 + 0.48 * noise(n, 1336)
            } else {
                noise(n, 1336)
            };
            undergrowth(out, ground, at, n + 30_000, pick);
        }
    }
}

/// The forest floor past the tree ring: undergrowth among the belt's
/// trees, so the woods read as layered from the trails and the ring.
fn floor(out: &mut Vec<Placement>, ground: &mut Ground) {
    const TRIES: u32 = 110;
    for n in 0..TRIES {
        let a = noise(n, 1340) * TAU;
        let r = FLOOR.0 + (FLOOR.1 - FLOOR.0) * noise(n, 1341);
        let at = [a.cos() * r, a.sin() * r];
        // Ferns, hazels, and elders mostly; the belt has its trees.
        let pick = 0.14 + 0.5 * noise(n, 1342);
        undergrowth(out, ground, at, n + 40_000, pick);
    }
}

/// Elders, hazels, fern banks, and a small tree now and then in the gaps
/// between the town's buildings and in its back yards, where the earlier
/// rounds left open grass.
fn between(out: &mut Vec<Placement>, ground: &mut Ground) {
    const STEP: f32 = 3.0;
    const REACH: f32 = 92.0;
    let span = (REACH / STEP) as i32;
    for i in -span..=span {
        for j in -span..=span {
            let n = ((i + 300) * 1000 + j + 300) as u32;
            let x = i as f32 * STEP + STEP * (noise(n, 1350) - 0.5);
            let z = j as f32 * STEP + STEP * (noise(n, 1351) - 0.5);
            if x.hypot(z) > REACH || thickness(x, z) > 0.0 || paved(x, z, 0.5) {
                continue;
            }
            let d = ground.to_building(x, z);
            if !(0.8..4.5).contains(&d) {
                continue;
            }
            let pick = noise(n, 1352);
            let yaw = noise(n, 1353) * TAU;
            let scale = 0.7 + 0.3 * noise(n, 1354);
            if pick < 0.04 {
                let piece = Placement::new(ELDER, [x, z], yaw, Collision::None).scale(scale);
                ground.put(out, piece, 1.2 * scale);
            } else if pick < 0.07 {
                let piece = Placement::new(HAZEL, [x, z], yaw, Collision::None).scale(scale);
                ground.put(out, piece, 1.3 * scale);
            } else if pick < 0.12 {
                let piece = Placement::new(FERN_BANK, [x, z], yaw, Collision::None).scale(scale);
                ground.put(out, piece, 1.3 * scale);
            } else if pick < 0.14 && d > 2.6 {
                let kind = if noise(n, 1355) < 0.5 {
                    ROWAN
                } else {
                    BIRCH_TRIO
                };
                tree(
                    out,
                    ground,
                    kind,
                    [x, z],
                    n + 50_000,
                    0.75 + 0.2 * noise(n, 1356),
                    false,
                );
            } else if pick < 0.16 {
                // A young tree against a gable, its back to the wall.
                let piece =
                    Placement::new(YOUNG_TREES, [x, z], yaw + FRAC_PI_2, Collision::Core(0.12))
                        .scale(0.8);
                ground.put(out, piece, 1.3);
            }
        }
    }
}
