//! The fifth round: foliage (`scripts/blender/foliage.py`). Wild woods at
//! the town's edges, layered from canopy to forest floor, with clearings,
//! deadwood, rocks, a campfire, a stone circle, and a cascade on Glade Run;
//! overgrown verges along the roads; and greenery between the buildings:
//! trees and shrubs in yards, alleys, and courtyards, ivy on walls and
//! fences, climbing roses, window boxes, hedges with gateways, planters on
//! the paved streets, and willows by the ponds.
//!
//! Every piece stands on open ground, as [`super::streets::clear`] finds
//! it, extended past the clearing into the woods, and apart from what
//! already stands there ([`Ground`]). Trunks, rocks, logs, stumps, hedges,
//! and planters block walking; shrubs, ferns, grass, flowers, roots, and
//! ivy don't. Nothing here breaks (`demolition::carve`).

use super::super::{CLEARING_RADIUS, HALF_EXTENT, HALL, STRONGROOM};
use super::generated::Instance;
use super::{
    Collision, PAVED, PAVED_SQUARES, PONDS, Placement, RETURN_PORTAL, STATIONS, STREAM,
    STREAM_HALF, YARD, inside, noise, on_road, reserved, segment_distance, stream_distance,
};
use crate::controller::{Footprint, forward};
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The broadleaf trees, with the half width of each one's trunk and its
/// crown's radius, m, at scale one.
const BROADLEAF: [(&str, f32, f32); 4] = [
    ("foliage/oak_forked", 0.35, 3.6),
    ("foliage/beech_tall", 0.3, 2.6),
    ("foliage/linden_broad", 0.4, 4.2),
    ("foliage/oak_old", 0.75, 5.0),
];
const SNAG: (&str, f32, f32) = ("foliage/snag", 0.3, 1.6);
/// The fir, the woods' conifer, in the nature kit's pines' place.
pub const FIR: (&str, f32, f32) = ("foliage/fir", 0.3, 3.0);
/// The forest belt's cheap trees (`streets::woods`), for the stands past
/// the tree ring.
const BELT: [(&str, f32, f32); 3] = [
    ("generated/oak_low", 0.35, 2.6),
    ("generated/birch_low", 0.3, 2.0),
    ("generated/spruce_low", 0.35, 2.2),
];
/// The broadleaf trees the commons and the streets plant instead of the
/// nature kit's, a fifth of its triangles each (`greens`).
pub const PARK_TREES: [&str; 3] = [
    "foliage/oak_forked",
    "foliage/linden_broad",
    "foliage/beech_tall",
];
const WILLOW: (&str, f32, f32) = ("foliage/willow_weeping", 0.42, 3.6);
/// The shrubs, and how far each reaches, m.
const SHRUBS: [(&str, f32); 4] = [
    ("foliage/shrub_mound", 1.2),
    ("foliage/shrub_tall", 0.8),
    ("foliage/shrub_flowering", 1.0),
    ("foliage/bramble", 1.4),
];
/// Ground cover: ferns, tall grass, and wildflowers.
const COVER: [&str; 3] = [
    "foliage/fern_clump",
    "foliage/grass_tall",
    "foliage/wildflower_clump",
];
/// Where the woods' stands lie: the inner and outer radius of the band, m.
const BAND: (f32, f32) = (108.0, 164.0);
/// Stands of trees round the band.
const STANDS: u32 = 52;
/// Walden Woods and Fernhollow, where the woods are thickest: a center on
/// the ground and the reach of each, m.
const THICKETS: [([f32; 2], f32); 2] = [([-104.0, -88.0], 46.0), ([98.0, 84.0], 40.0)];

/// Every placement of the fifth round, after the earlier rounds'.
pub fn build(out: &mut Vec<Placement>, placed: &[([f32; 2], f32)]) {
    let mut ground = Ground::new(out, placed);
    let start = out.len();
    walls(out, start);
    windows(out, start);
    fences(out, start);
    ponds(out, &mut ground);
    cascade(out, &mut ground);
    campfire(out, &mut ground);
    stone_circle(out, &mut ground);
    woods(out, &mut ground);
    thicken(out, &mut ground);
    hedges(out, &mut ground);
    planters(out, &mut ground);
    yards(out, &mut ground);
    verges(out, &mut ground);
}

/// What stands where already, and the open ground: the checks of
/// [`super::streets::clear`] with the town's buildings computed once, and
/// a grid of the pieces placed so far.
struct Ground {
    rects: Vec<([f32; 2], [f32; 2])>,
    blocks: Vec<Footprint>,
    /// The buildings: kit footprints and each generated model's boxes.
    buildings: Vec<Footprint>,
    taken: HashMap<(i32, i32), Vec<([f32; 2], f32)>>,
}

/// The grid's cell, m.
const GRID: f32 = 6.0;

impl Ground {
    fn new(out: &[Placement], placed: &[([f32; 2], f32)]) -> Self {
        let rects = reserved();
        let blocks: Vec<Footprint> = super::generated()
            .iter()
            .flat_map(Instance::blocks)
            .map(|(f, _)| f)
            .collect();
        let buildings = rects
            .iter()
            .map(|&([x, z], [hx, hz])| Footprint {
                min: [x - hx, z - hz],
                max: [x + hx, z + hz],
            })
            .chain(blocks.iter().copied())
            .collect();
        let mut ground = Self {
            rects,
            blocks,
            buildings,
            taken: HashMap::new(),
        };
        for &(at, r) in placed {
            ground.take(at, r);
        }
        // Trees and anything else that blocks, from the earlier rounds.
        for p in out {
            let r = match p.collision {
                _ if p.model.starts_with("village/") => continue,
                Collision::None => continue,
                Collision::Core(half) => half * p.scale + 0.8,
                _ => 0.8,
            };
            ground.take(p.at, r);
        }
        ground
    }

    fn cell(at: [f32; 2]) -> (i32, i32) {
        ((at[0] / GRID).floor() as i32, (at[1] / GRID).floor() as i32)
    }

    fn take(&mut self, at: [f32; 2], r: f32) {
        self.taken.entry(Self::cell(at)).or_default().push((at, r));
    }

    /// Whether anything placed stands within `r` m of `at`, with its own
    /// reach.
    fn crowded(&self, at: [f32; 2], r: f32) -> bool {
        let (cx, cz) = Self::cell(at);
        let span = ((r + 6.0) / GRID).ceil() as i32;
        (-span..=span).any(|dx| {
            (-span..=span).any(|dz| {
                self.taken.get(&(cx + dx, cz + dz)).is_some_and(|items| {
                    items
                        .iter()
                        .any(|(p, pr)| (p[0] - at[0]).hypot(p[1] - at[1]) < r + pr)
                })
            })
        })
    }

    /// Whether a piece reaching `r` m from `(x, z)` stands on open ground:
    /// [`super::streets::clear`]'s rule, but also out in the woods past
    /// the clearing, to the zone's edge.
    fn open(&self, x: f32, z: f32, r: f32) -> bool {
        let near = |p: [f32; 2], d: f32| (p[0] - x).hypot(p[1] - z) < d;
        x.abs() < HALF_EXTENT - 8.0 - r
            && z.abs() < HALF_EXTENT - 8.0 - r
            && !on_road(x, z, r + 0.25)
            && !self.rects.iter().any(|rect| inside(*rect, x, z, r + 0.3))
            && !self.blocks.iter().any(|f| f.contains(x, z, r + 0.4))
            && !PONDS.iter().any(|(c, pr)| near(*c, pr + r + 1.6))
            && stream_distance(x, z) > STREAM_HALF + r + 1.4
            && !inside(YARD, x, z, r + 1.0)
            && !near([RETURN_PORTAL.x, RETURN_PORTAL.z], r + 3.5)
            && !STATIONS.iter().any(|s| near(s.at, r + 2.2))
    }

    /// Distance from `(x, z)` to the nearest building, m.
    fn to_building(&self, x: f32, z: f32) -> f32 {
        self.buildings
            .iter()
            .map(|f| {
                let dx = (f.min[0] - x).max(x - f.max[0]).max(0.0);
                let dz = (f.min[1] - z).max(z - f.max[1]).max(0.0);
                dx.hypot(dz)
            })
            .fold(f32::INFINITY, f32::min)
    }

    /// Places `p` if the ground within `r` m is open and nothing placed
    /// stands within `r`; returns whether it did.
    fn put(&mut self, out: &mut Vec<Placement>, p: Placement, r: f32) -> bool {
        let [x, z] = p.at;
        if self.crowded(p.at, r) || !self.open(x, z, r) {
            return false;
        }
        self.take(p.at, r);
        out.push(p);
        true
    }
}

/// A tree of `kind` at `at`: its trunk blocks, and, under some, roots
/// spread over the ground.
fn tree(
    out: &mut Vec<Placement>,
    ground: &mut Ground,
    (model, trunk, crown): (&'static str, f32, f32),
    at: [f32; 2],
    n: u32,
    scale: f32,
    roots: bool,
) -> bool {
    let yaw = noise(n, 610) * TAU;
    let placement = Placement::new(model, at, yaw, Collision::Core(trunk))
        .scale(scale)
        .lift(-0.1);
    // The crown's reach keeps trees apart; the trunk needs open ground.
    let [x, z] = at;
    let reach = crown * scale * 0.55;
    if ground.crowded(at, reach) || !ground.open(x, z, trunk * scale + 0.4) {
        return false;
    }
    ground.take(at, reach);
    out.push(placement);
    if roots {
        out.push(
            Placement::new("foliage/roots_spread", at, yaw + 0.7, Collision::None)
                .scale(scale * (trunk / 0.4).clamp(0.8, 1.8))
                .lift(-0.05),
        );
    }
    true
}

/// How near Fernhollow `(x, z)` is, 0 to 1: its woods are darker, with
/// more firs.
fn fernhollow(at: [f32; 2]) -> f32 {
    let ([cx, cz], reach) = THICKETS[1];
    (1.0 - (at[0] - cx).hypot(at[1] - cz) / reach).clamp(0.0, 1.0)
}

/// How thick the woods grow at `(x, z)`, 0 to 1: thickest in Walden Woods
/// and Fernhollow.
fn thickness(x: f32, z: f32) -> f32 {
    THICKETS
        .iter()
        .map(|&([cx, cz], reach)| (1.0 - (x - cx).hypot(z - cz) / reach).clamp(0.0, 1.0))
        .fold(0.0, f32::max)
}

/// The woods round the town: stands of broadleaf trees over shrubs and
/// brambles, ferns and grass, fallen logs, stumps, snags, and rocks, with
/// glades among them.
fn woods(out: &mut Vec<Placement>, ground: &mut Ground) {
    for s in 0..STANDS {
        let angle = (s as f32 + noise(s, 620)) / STANDS as f32 * TAU;
        // More stands at the inner edge, where the town sees them.
        let deep = noise(s, 621).powf(1.6);
        let r = BAND.0 + (BAND.1 - BAND.0) * deep;
        let center = [angle.cos() * r, angle.sin() * r];
        let thick = thickness(center[0], center[1]);
        stand(out, ground, s, center, thick);
    }
    // Walden Woods and Fernhollow get more stands among the first.
    for (k, &([cx, cz], reach)) in THICKETS.iter().enumerate() {
        for s in 0..10_u32 {
            let n = 1000 + k as u32 * 100 + s;
            let a = noise(n, 622) * TAU;
            let d = reach * 0.9 * noise(n, 623).sqrt();
            stand(out, ground, n, [cx + a.cos() * d, cz + a.sin() * d], 1.0);
        }
    }
}

/// The forest belt past the tree ring, filled in with more of its cheap
/// trees wherever it is open, so the woods read as one canopy from the
/// town.
fn thicken(out: &mut Vec<Placement>, ground: &mut Ground) {
    const TRIES: u32 = 160;
    for n in 0..TRIES {
        let a = noise(n, 800) * TAU;
        let r = CLEARING_RADIUS + 8.0 + 92.0 * noise(n, 801);
        let at = [a.cos() * r, a.sin() * r];
        let kind = BELT[(noise(n, 802) * 3.0) as usize % 3];
        let scale = 1.1 + 0.7 * noise(n, 803);
        tree(out, ground, kind, at, n + 5000, scale, false);
    }
}

/// One stand of the woods at `center`, `thick` from 0 to 1.
fn stand(out: &mut Vec<Placement>, ground: &mut Ground, s: u32, center: [f32; 2], thick: f32) {
    let glade = s % 7 == 3;
    let spread = 7.0 + 6.0 * noise(s, 630) + 4.0 * thick;
    let at = |n: u32, salt: u32, near: f32, far: f32| {
        let a = noise(n, salt) * TAU;
        let d = near + (far - near) * noise(n, salt + 1).sqrt();
        [center[0] + a.cos() * d, center[1] + a.sin() * d]
    };
    // The canopy, leaving a glade's middle open.
    let trees = 3 + (2.5 * noise(s, 631) + 3.0 * thick) as u32;
    let inner = if glade { 7.0 } else { 0.0 };
    let mut planted = Vec::new();
    for t in 0..trees * 3 {
        if planted.len() as u32 == trees {
            break;
        }
        let n = s * 64 + t;
        let p = at(n, 632, inner, spread + inner);
        let pick = noise(n, 634);
        // Past the tree ring, half are the forest belt's cheap trees.
        let far = p[0].hypot(p[1]) > CLEARING_RADIUS + 6.0;
        let kind = if pick < 0.07 {
            SNAG
        } else if pick < 0.16 {
            BROADLEAF[3]
        } else if pick < 0.24 + 0.2 * fernhollow(p) {
            FIR
        } else if far && pick < 0.62 {
            BELT[(noise(n, 635) * 3.0) as usize % 3]
        } else {
            BROADLEAF[(noise(n, 635) * 3.0) as usize % 3]
        };
        let scale = 0.9 + 0.45 * noise(n, 636);
        let roots =
            kind.0 == BROADLEAF[3].0 || (kind.0.starts_with("foliage/") && noise(n, 637) < 0.22);
        if tree(out, ground, kind, p, n, scale, roots) {
            planted.push(p);
        }
    }
    // Shrubs and brambles under and between the trees.
    let shrubs = 2 + (2.5 * noise(s, 640) + 3.5 * thick) as u32;
    for k in 0..shrubs * 2 {
        let n = s * 64 + k;
        let p = at(n, 641, inner * 0.6, spread + inner + 3.0);
        let pick = noise(n, 643);
        let (model, reach) = SHRUBS[if pick < 0.12 {
            3
        } else {
            (pick * 3.0) as usize % 3
        }];
        let scale = 0.8 + 0.5 * noise(n, 644);
        let piece = Placement::new(model, p, noise(n, 645) * TAU, Collision::None).scale(scale);
        ground.put(out, piece, reach * scale * 0.7);
    }
    // Deadwood and rocks.
    for k in 0..2 + (2.0 * thick) as u32 {
        let n = s * 64 + 40 + k;
        let p = at(n, 650, inner * 0.5, spread + inner);
        let (model, collision, reach) = match (noise(n, 652) * 7.0) as u32 {
            0 => ("foliage/log_hollow", Collision::Bounds, 2.2),
            1 => ("foliage/log_broken", Collision::Bounds, 2.6),
            2 => ("foliage/stump_mossy", Collision::Core(0.5), 1.0),
            3 => ("foliage/stump_broken", Collision::Core(0.4), 0.9),
            4 => ("foliage/boulder_cluster", Collision::Core(0.8), 1.6),
            5 => ("foliage/root_arch", Collision::Core(0.9), 2.0),
            _ => ("foliage/stump_mossy", Collision::Core(0.5), 1.0),
        };
        let piece = Placement::new(model, p, noise(n, 653) * TAU, collision);
        ground.put(out, piece, reach);
    }
    // Cliff rocks where the ground rises past the tree ring.
    let r = center[0].hypot(center[1]);
    if r > 146.0 && noise(s, 655) < 0.35 {
        let p = at(s, 656, 2.0, spread);
        let piece = Placement::new(
            "foliage/cliff_rock",
            p,
            noise(s, 657) * TAU,
            Collision::Core(1.6),
        )
        .scale(0.8 + 0.6 * noise(s, 658))
        .lift(-0.4);
        ground.put(out, piece, 2.8);
    }
    // The forest floor: ferns thick in the shade, grass and flowers at the
    // edges and in the glades.
    let cover = 4 + (3.0 * thick) as u32 + if glade { 6 } else { 0 };
    for k in 0..cover * 2 {
        let n = s * 64 + k;
        let p = at(n, 660, 0.0, spread + inner + 4.0);
        let shade = planted
            .iter()
            .any(|t: &[f32; 2]| (t[0] - p[0]).hypot(t[1] - p[1]) < 4.0);
        let model = if shade || noise(n, 662) < 0.4 {
            COVER[0]
        } else if noise(n, 663) < 0.55 {
            COVER[1]
        } else {
            COVER[2]
        };
        let piece = Placement::new(model, p, noise(n, 664) * TAU, Collision::None)
            .scale(0.8 + 0.5 * noise(n, 665));
        ground.put(out, piece, 0.5);
    }
    // A glade's fairy ring.
    if glade && noise(s, 670) < 0.5 {
        let piece = Placement::new(
            "foliage/mushroom_ring",
            center,
            noise(s, 671) * TAU,
            Collision::None,
        );
        ground.put(out, piece, 2.0);
    }
}

/// Weeping willows on the ponds' banks, two to a pond, leaning over the
/// water.
fn ponds(out: &mut Vec<Placement>, ground: &mut Ground) {
    for (k, &([px, pz], pr)) in PONDS.iter().enumerate() {
        let mut set = 0;
        for i in 0..24_u32 {
            if set == 2 {
                break;
            }
            let n = k as u32 * 32 + i;
            let a = noise(n, 680) * TAU;
            let r = pr + 3.4;
            let at = [px + a.cos() * r, pz + a.sin() * r];
            let scale = 0.9 + 0.25 * noise(n, 681);
            if tree(out, ground, WILLOW, at, n, scale, false) {
                set += 1;
            }
        }
    }
}

/// A low cascade where Glade Run spills over a weir of stones in Walden
/// Woods, with the stream's course through the model's length.
fn cascade(out: &mut Vec<Placement>, ground: &mut Ground) {
    let (a, b) = (STREAM[5], STREAM[6]);
    let at = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    // The model's water falls along its +z, downstream.
    let yaw = (b[0] - a[0]).atan2(b[1] - a[1]);
    out.push(Placement::new("foliage/cascade", at, yaw, Collision::None).scale(0.95));
    ground.take(at, 3.0);
    // Trees and ferns along the run's banks in the woods.
    for (k, w) in STREAM.windows(2).enumerate().skip(4) {
        let (a, b) = (w[0], w[1]);
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let (tx, tz) = ((b[0] - a[0]) / length, (b[1] - a[1]) / length);
        for i in 0..(length / 5.0) as u32 {
            let n = k as u32 * 32 + i;
            let along = (i as f32 + noise(n, 690)) * 5.0;
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let off = STREAM_HALF + 2.6 + 2.5 * noise(n, 691);
            let at = [
                a[0] + tx * along - tz * off * side,
                a[1] + tz * along + tx * off * side,
            ];
            if noise(n, 692) < 0.3 {
                let kind = if noise(n, 693) < 0.5 {
                    WILLOW
                } else {
                    BROADLEAF[0]
                };
                tree(out, ground, kind, at, n, 0.8 + 0.3 * noise(n, 694), false);
            } else {
                let model = if noise(n, 695) < 0.6 {
                    COVER[0]
                } else {
                    COVER[1]
                };
                let piece = Placement::new(model, at, noise(n, 696) * TAU, Collision::None);
                ground.put(out, piece, 0.5);
            }
        }
    }
}

/// A campfire in a clearing of Walden Woods, its log seats round it and
/// the woods close about.
fn campfire(out: &mut Vec<Placement>, ground: &mut Ground) {
    for k in 0..40_u32 {
        let a = noise(k, 700) * TAU;
        let d = 18.0 * noise(k, 701).sqrt();
        let at = [-96.0 + a.cos() * d, -104.0 + a.sin() * d];
        if !ground.open(at[0], at[1], 3.4) || ground.crowded(at, 3.4) {
            continue;
        }
        let fire = Placement::new(
            "foliage/campfire",
            at,
            noise(k, 702) * TAU,
            Collision::Core(0.75),
        );
        ground.put(out, fire, 3.4);
        // A ring of stumps for more seats, and a woodpile log.
        for i in 0..2_u32 {
            let b = a + 1.2 + i as f32 * 2.2;
            let p = [at[0] + b.cos() * 4.4, at[1] + b.sin() * 4.4];
            let stump =
                Placement::new("foliage/stump_mossy", p, b, Collision::Core(0.5)).scale(0.8);
            ground.put(out, stump, 0.9);
        }
        return;
    }
}

/// A small circle of standing stones in a clearing in the woods, with one
/// fallen.
fn stone_circle(out: &mut Vec<Placement>, ground: &mut Ground) {
    const STONES: u32 = 9;
    const RADIUS: f32 = 5.5;
    // The first clearing round the band, from the north, wide enough.
    let Some(center) = (0..96_u32)
        .flat_map(|k| {
            let a = FRAC_PI_2 + k as f32 / 96.0 * TAU;
            [138.0_f32, 146.0, 154.0].map(|r| [a.cos() * r, a.sin() * r])
        })
        .find(|&c| ground.open(c[0], c[1], RADIUS + 1.0) && !ground.crowded(c, RADIUS + 1.0))
    else {
        return;
    };
    for i in 0..STONES {
        let a = i as f32 / STONES as f32 * TAU;
        let at = [center[0] + a.cos() * RADIUS, center[1] + a.sin() * RADIUS];
        // Each faces the middle; one is squat and leans.
        let facing = (center[0] - at[0]).atan2(center[1] - at[1]);
        let (model, lift) = if i == 4 {
            ("foliage/standing_stone_squat", -0.2)
        } else {
            ("foliage/standing_stone", -0.15)
        };
        out.push(
            Placement::new(
                model,
                at,
                facing + 0.2 * noise(i, 710),
                Collision::Core(0.45),
            )
            .scale(0.85 + 0.35 * noise(i, 711))
            .lift(lift),
        );
    }
    ground.take(center, RADIUS + 1.0);
    out.push(Placement::new("foliage/mushroom_ring", center, 0.0, Collision::None).scale(1.4));
}

/// Ivy climbing the town's ground-floor walls, and climbing roses on a few,
/// among the placements before `start`. Each stands where its wall piece
/// does, the model's own offset putting it on the wall's face, so the town's
/// demolition breaks it with its wall (`demolition::town`).
fn walls(out: &mut Vec<Placement>, start: usize) {
    const WALLS: [&str; 3] = [
        "village/Wall_Plaster_Straight",
        "village/Wall_Plaster_WoodGrid",
        "village/Wall_Plaster_Straight_Base",
    ];
    let mut added = Vec::new();
    for (k, p) in out[..start].iter().enumerate() {
        if !WALLS.contains(&p.model) || p.lift > 0.1 {
            continue;
        }
        let [x, z] = p.at;
        if inside(HALL, x, z, 2.0) || inside(STRONGROOM, x, z, 2.0) {
            continue;
        }
        let n = k as u32;
        let f = forward(p.yaw);
        let pick = noise(n, 720);
        if pick < 0.16 {
            added.push(Placement::new(
                "foliage/ivy_wall",
                p.at,
                p.yaw,
                Collision::None,
            ));
        } else if pick < 0.2 && !on_road(x + f.x * 0.8, z + f.z * 0.8, 0.3) {
            added.push(Placement::new(
                "foliage/rose_trellis",
                p.at,
                p.yaw,
                Collision::None,
            ));
        }
    }
    out.extend(added);
}

/// Window boxes overflowing with flowers under about a third of the kit
/// houses' windows, each where its window piece stands, as [`walls`]'
/// ivy does.
fn windows(out: &mut Vec<Placement>, start: usize) {
    let mut added = Vec::new();
    for (k, p) in out[..start].iter().enumerate() {
        if !matches!(
            p.model,
            "village/Window_Wide_Flat1" | "village/Window_Wide_Round1"
        ) || noise(k as u32, 730) > 0.34
        {
            continue;
        }
        added.push(Placement::new("foliage/window_box", p.at, p.yaw, Collision::None).lift(p.lift));
    }
    out.extend(added);
}

/// Ivy over the dry-stone walls and the picket fences.
fn fences(out: &mut Vec<Placement>, start: usize) {
    let mut added = Vec::new();
    for (k, p) in out[..start].iter().enumerate() {
        // Each fence's half depth and the ivy's scale for its height.
        let (half, scale, share) = match p.model {
            "generated/stone_wall" => (0.29, 0.55, 0.45),
            "generated/picket_fence" => (0.07, 0.85, 0.3),
            _ => continue,
        };
        let n = k as u32;
        if noise(n, 740) > share {
            continue;
        }
        // On the side away from the nearest road, or either side.
        let side = if noise(n, 741) < 0.5 { 0.0 } else { PI };
        let f = forward(p.yaw + side);
        let [x, z] = p.at;
        added.push(
            Placement::new(
                "foliage/ivy_low",
                [x + f.x * half, z + f.z * half],
                p.yaw + side,
                Collision::None,
            )
            .scale(scale * (0.9 + 0.2 * noise(n, 742))),
        );
    }
    out.extend(added);
}

/// Whether `(x, z)` lies on the cobbles of a paved street or square,
/// within `margin` m.
fn paved(x: f32, z: f32, margin: f32) -> bool {
    PAVED
        .iter()
        .any(|&(a, b, half)| segment_distance(a, b, x, z) <= half + margin)
        || PAVED_SQUARES.iter().any(|r| inside(*r, x, z, margin))
}

/// Clipped hedges along some lanes' verges, in runs with gaps and a
/// gateway under an arch.
fn hedges(out: &mut Vec<Placement>, ground: &mut Ground) {
    for (k, &(a, b, half)) in super::roads().iter().enumerate() {
        let n = k as u32;
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        if length < 24.0 || noise(n, 750) > 0.4 {
            continue;
        }
        let (tx, tz) = ((b[0] - a[0]) / length, (b[1] - a[1]) / length);
        let side = if noise(n, 751) < 0.5 { 1.0 } else { -1.0 };
        let off = half + 1.7;
        let yaw = tx.atan2(tz) + FRAC_PI_2;
        let mut along = 3.0;
        let mut i = 0_u32;
        while along + 4.0 < length {
            let gate = i % 4 == 2;
            let (model, size, collision) = if gate {
                ("foliage/hedge_gate", 5.8, Collision::Opening(1.3))
            } else {
                ("foliage/hedge_long", 4.0, Collision::Bounds)
            };
            let mid = along + size / 2.0;
            let at = [
                a[0] + tx * mid - tz * off * side,
                a[1] + tz * mid + tx * off * side,
            ];
            // Gaps: a missing piece now and then.
            // Points along the piece, each with the hedge's half depth.
            let points: Vec<[f32; 2]> = [-0.4_f32, -0.2, 0.0, 0.2, 0.4]
                .iter()
                .map(|t| [at[0] + tx * size * t, at[1] + tz * size * t])
                .collect();
            let fits = points
                .iter()
                .all(|&p| ground.open(p[0], p[1], 0.75) && !ground.crowded(p, 0.75));
            // Gaps: a missing piece now and then.
            if fits && noise(n * 64 + i, 752) > 0.15 && !paved(at[0], at[1], 1.0) {
                // The front faces the lane.
                let facing = if side > 0.0 { yaw } else { yaw + PI };
                out.push(Placement::new(model, at, facing, collision));
                for p in points {
                    ground.take(p, 0.75);
                }
            }
            along += size + 0.1;
            i += 1;
        }
    }
}

/// Planters overflowing with flowers along the paved streets' edges.
fn planters(out: &mut Vec<Placement>, ground: &mut Ground) {
    for (k, &(a, b, half)) in PAVED.iter().enumerate() {
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let (tx, tz) = ((b[0] - a[0]) / length, (b[1] - a[1]) / length);
        let yaw = tx.atan2(tz) + FRAC_PI_2;
        let mut along = 6.0;
        let mut i = 0_u32;
        while along < length {
            let n = k as u32 * 64 + i;
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let off = half + 0.9;
            let at = [
                a[0] + tx * along - tz * off * side,
                a[1] + tz * along + tx * off * side,
            ];
            let piece = Placement::new("foliage/planter_overflow", at, yaw, Collision::Bounds)
                .scale(0.9 + 0.2 * noise(n, 760));
            ground.put(out, piece, 0.9);
            along += 11.0 + 4.0 * noise(n, 761);
            i += 1;
        }
    }
}

/// Trees, shrubs, and flowers in the yards, alleys, and courtyards
/// between the town's buildings.
fn yards(out: &mut Vec<Placement>, ground: &mut Ground) {
    const STEP: f32 = 3.2;
    let span = (CLEARING_RADIUS / STEP) as i32;
    for i in -span..=span {
        for j in -span..=span {
            let n = ((i + 100) * 1000 + j + 100) as u32;
            let x = i as f32 * STEP + STEP * (noise(n, 770) - 0.5);
            let z = j as f32 * STEP + STEP * (noise(n, 771) - 0.5);
            if x.hypot(z) > CLEARING_RADIUS - 10.0 || thickness(x, z) > 0.0 {
                continue;
            }
            let d = ground.to_building(x, z);
            if !(1.4..7.0).contains(&d) || paved(x, z, 0.6) {
                continue;
            }
            let pick = noise(n, 772);
            let yaw = noise(n, 773) * TAU;
            if pick < 0.12 && d > 2.6 {
                let kind = BROADLEAF[(noise(n, 774) * 3.0) as usize % 3];
                tree(
                    out,
                    ground,
                    kind,
                    [x, z],
                    n,
                    0.6 + 0.25 * noise(n, 775),
                    false,
                );
            } else if pick < 0.5 {
                let (model, reach) = SHRUBS[(noise(n, 776) * 3.0) as usize % 3];
                let scale = 0.7 + 0.4 * noise(n, 777);
                let piece = Placement::new(model, [x, z], yaw, Collision::None).scale(scale);
                ground.put(out, piece, reach * scale * 0.7);
            } else if pick < 0.57 {
                let model = COVER[1 + (noise(n, 778) * 2.0) as usize % 2];
                let piece = Placement::new(model, [x, z], yaw, Collision::None);
                ground.put(out, piece, 0.5);
            }
        }
    }
}

/// Tall grass, wildflowers, ferns, and the odd shrub along every road's
/// verges, off the cobbles.
fn verges(out: &mut Vec<Placement>, ground: &mut Ground) {
    for (k, &(a, b, half)) in super::roads().iter().enumerate() {
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        if length < 4.0 {
            continue;
        }
        let (tx, tz) = ((b[0] - a[0]) / length, (b[1] - a[1]) / length);
        let steps = (length / 4.5) as u32;
        for i in 0..steps {
            for side in [1.0_f32, -1.0] {
                let n = k as u32 * 512 + i * 2 + u32::from(side > 0.0);
                let along = (i as f32 + noise(n, 780)) * 4.5;
                let off = half + 1.2 + 1.2 * noise(n, 781);
                let at = [
                    a[0] + tx * along - tz * off * side,
                    a[1] + tz * along + tx * off * side,
                ];
                if paved(at[0], at[1], 0.4) {
                    continue;
                }
                let pick = noise(n, 782);
                let yaw = noise(n, 783) * TAU;
                let wild = thickness(at[0], at[1]) > 0.0 || at[0].hypot(at[1]) > 110.0;
                if pick < 0.12 {
                    let (model, reach) = SHRUBS[(noise(n, 784) * 4.0) as usize % 4];
                    let piece = Placement::new(model, at, yaw, Collision::None).scale(0.8);
                    ground.put(out, piece, reach * 0.6);
                } else if pick < 0.29 {
                    let model = if wild && noise(n, 785) < 0.5 {
                        COVER[0]
                    } else {
                        COVER[1 + (noise(n, 786) * 2.0) as usize % 2]
                    };
                    let piece = Placement::new(model, at, yaw, Collision::None)
                        .scale(0.8 + 0.4 * noise(n, 787));
                    ground.put(out, piece, 0.45);
                }
            }
        }
    }
}
