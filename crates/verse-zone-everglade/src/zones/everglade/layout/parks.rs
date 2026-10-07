//! The town's second round of dressing, after the illustrated map
//! (`docs/verse/everglade-map.svg`): life on the ponds (reeds, lily pads,
//! jetties, and rowboats), the community garden's beds behind its picket
//! fence, the farm at the long meadow's edge, café tables on the decks and
//! the plaza, planters, Walden Woods' and Fernhollow's undergrowth (ferns,
//! mossy rocks, fallen logs, stumps, and toadstools), and round bushes at
//! the clearing's edge.
//!
//! Every piece is a cheap generated model (`scripts/blender/town_props.py`)
//! or a kit fern. Loose pieces go through `streets::try_put`, so none stands
//! on a road, a walk, a building, a pond, or a station; pieces that belong
//! against a bank or a fence are placed by rule, off the roads.

use super::streets::try_put;
use super::{Collision, PONDS, Placement, afloat, city, noise, on_land, on_road, tree};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A piece already placed: its center and radius, m.
type Placed = Vec<([f32; 2], f32)>;

/// Every placement of the second round's dressing.
pub fn build(out: &mut Vec<Placement>, placed: &mut Placed) {
    ponds(out, placed);
    garden(out, placed);
    farm(out, placed);
    cafes(out, placed);
    planters(out, placed);
    undergrowth(out, placed);
}

/// A piece set by rule rather than at a hashed point: kept when it is off
/// the roads and clear of the pieces placed so far.
fn put(out: &mut Vec<Placement>, placed: &mut Placed, placement: Placement, r: f32) -> bool {
    let [x, z] = placement.at;
    let crowded = placed
        .iter()
        .any(|(p, pr)| (p[0] - x).hypot(p[1] - z) < pr + r);
    if crowded || on_road(x, z, r + 0.2) {
        return false;
    }
    placed.push((placement.at, r));
    out.push(placement);
    true
}

/// The point `d` meters from pond `k`'s center at `angle`, m.
fn bank(k: usize, angle: f32, d: f32) -> [f32; 2] {
    let ([x, z], _) = PONDS[k];
    [x + angle.cos() * d, z + angle.sin() * d]
}

/// The heading that points a model's front from `from` toward `to`.
fn toward(from: [f32; 2], to: [f32; 2]) -> f32 {
    (to[0] - from[0]).atan2(to[1] - from[1])
}

/// Reeds round each pond's bank, more lily pads, and a jetty with a
/// rowboat at Lantern Pond, Reed Pond, and the Thinking Pond; another
/// rowboat waits inside the boathouse.
fn ponds(out: &mut Vec<Placement>, placed: &mut Placed) {
    for (k, &(_, r)) in PONDS.iter().enumerate() {
        let clumps = 4 + k as u32 % 2;
        for n in 0..clumps {
            let angle = (n as f32 + 0.6 * noise(n, 160 + k as u32)) / clumps as f32 * TAU;
            // Lantern Pond's north bank is the boathouse's.
            if k == 0 && angle.sin() > 0.5 {
                continue;
            }
            let at = bank(k, angle, r + 0.2);
            if on_road(at[0], at[1], 0.8) {
                continue;
            }
            let reeds = Placement::new("generated/reeds", at, noise(n, 161) * TAU, Collision::None)
                .scale(0.9 + 0.4 * noise(n, 162 + k as u32));
            out.push(reeds);
        }
        for i in 0..2 {
            let angle = noise(k as u32 * 2 + i, 163) * TAU;
            let at = bank(k, angle, r * (0.25 + 0.35 * noise(i, 164 + k as u32)));
            // Floating on the pond's surface.
            out.push(afloat(
                Placement::new("generated/lily_pads", at, angle, Collision::None).scale(0.8),
                0.012,
            ));
        }
    }
    // Jetties reach from the bank over the water, a boat tied beside each.
    for (k, angle) in [(0_usize, -2.35_f32), (1, PI + 0.3), (2, 2.4)] {
        let (_, r) = PONDS[k];
        let center = bank(k, angle, r);
        let water = bank(k, angle, 0.0);
        let yaw = toward(center, water);
        // The jetty's deck stands at the land's height over the bank.
        out.push(on_land(Placement::new(
            "generated/dock",
            center,
            yaw,
            Collision::None,
        )));
        placed.push((center, 3.2));
        let side = angle + FRAC_PI_2;
        let boat = bank(k, angle, r - 2.6);
        let boat = [boat[0] + side.cos() * 1.7, boat[1] + side.sin() * 1.7];
        // Afloat at its draft on the sampled surface; boarding it is W6's.
        out.push(afloat(
            Placement::new("generated/rowboat", boat, yaw + 0.25, Collision::None),
            -0.18,
        ));
    }
    if let Some(house) = city::GROUNDS.iter().find(|i| i.name == "boathouse") {
        out.push(
            Placement::new(
                "generated/rowboat",
                house.world([0.0, -2.6]),
                house.yaw,
                Collision::None,
            )
            .scale(house.scale)
            .lift(0.4 * house.scale),
        );
    }
}

/// The community garden behind Brownstone Row: a picket fence with a rose
/// arch at its east gate toward the glasshouse, and raised beds of
/// vegetables and flowers inside.
fn garden(out: &mut Vec<Placement>, placed: &mut Placed) {
    let ([gx, gz], [ghx, ghz]) = city::GARDEN;
    let (west, east, south, north) = (gx - ghx, gx + ghx, gz - ghz, gz + ghz);
    let fence = |out: &mut Vec<Placement>, at: [f32; 2], yaw: f32| {
        out.push(Placement::new(
            "generated/picket_fence",
            at,
            yaw,
            Collision::Bounds,
        ));
    };
    let mut x = west + 1.0;
    while x < east {
        fence(out, [x, south], 0.0);
        fence(out, [x, north], PI);
        x += 2.0;
    }
    let mut z = south + 1.0;
    while z < north {
        fence(out, [west, z], -FRAC_PI_2);
        // The gate, in the middle of the east side.
        if (z - gz).abs() > 1.2 {
            fence(out, [east, z], FRAC_PI_2);
        }
        z += 2.0;
    }
    out.push(Placement::new(
        "generated/garden_arch",
        [east, gz],
        FRAC_PI_2,
        Collision::Opening(0.7),
    ));
    placed.push(([gx, gz], ghx.max(ghz) + 0.5));
    for (i, row) in [-3.0_f32, 0.0, 3.0].into_iter().enumerate() {
        for (j, col) in [-3.2_f32, 2.4].into_iter().enumerate() {
            let model = if (i + j) % 2 == 0 {
                "generated/veg_bed"
            } else {
                "generated/flower_bed"
            };
            out.push(Placement::new(
                model,
                [gx + col, gz + row],
                0.0,
                Collision::Bounds,
            ));
        }
    }
    // Planters either side of the glasshouse's door.
    if let Some(glass) = city::GROUNDS.iter().find(|i| i.name == "glasshouse") {
        for x in [-1.8, 1.8] {
            let p = Placement::new(
                "generated/planter",
                glass.world([x, 1.0]),
                0.0,
                Collision::Core(0.45),
            );
            put(out, placed, p, 0.6);
        }
    }
}

/// The farm at the long meadow's edge: a rail-fenced paddock with a
/// haystack and bales, hay by the windmill, and beds before the farmhouse.
fn farm(out: &mut Vec<Placement>, placed: &mut Placed) {
    let (west, east, south, north) = (38.0_f32, 52.0_f32, -114.0_f32, -104.0_f32);
    let mut x = west + 1.0;
    while x < east {
        out.push(Placement::new(
            "generated/rail_fence",
            [x, south],
            0.0,
            Collision::Bounds,
        ));
        // The paddock's gate, toward the lane.
        if (x - 45.0).abs() > 1.5 {
            out.push(Placement::new(
                "generated/rail_fence",
                [x, north],
                0.0,
                Collision::Bounds,
            ));
        }
        x += 2.0;
    }
    let mut z = south + 1.0;
    while z < north {
        for x in [west, east] {
            out.push(Placement::new(
                "generated/rail_fence",
                [x, z],
                FRAC_PI_2,
                Collision::Bounds,
            ));
        }
        z += 2.0;
    }
    placed.push(([45.0, -109.0], 8.0));
    out.push(Placement::new(
        "generated/haystack",
        [48.5, -110.5],
        0.4,
        Collision::Core(1.2),
    ));
    out.push(Placement::new(
        "generated/hay_bales",
        [42.0, -111.0],
        0.3,
        Collision::Bounds,
    ));
    for (at, yaw, model, r) in [
        ([22.0, -112.0], 1.1, "generated/haystack", 1.6),
        ([37.0, -99.0], 2.0, "generated/hay_bales", 1.5),
        ([10.5, -100.6], 0.0, "generated/veg_bed", 1.3),
        ([18.0, -100.6], 0.0, "generated/veg_bed", 1.3),
    ] {
        let collision = if model.ends_with("haystack") {
            Collision::Core(1.2)
        } else {
            Collision::Bounds
        };
        put(out, placed, Placement::new(model, at, yaw, collision), r);
    }
    // The woodcutter's stumps and a log by the Walden cottage.
    for (at, model, yaw) in [
        ([-103.5, -81.5], "generated/stump", 0.3),
        ([-108.5, -82.0], "generated/fallen_log", 0.6),
        ([-104.5, -66.5], "generated/stump", 2.0),
    ] {
        let collision = if model.ends_with("log") {
            Collision::Bounds
        } else {
            Collision::Core(0.4)
        };
        put(out, placed, Placement::new(model, at, yaw, collision), 1.2);
    }
}

/// Café tables on the Boardwalk Cafés' decks and before the plaza's two
/// cafés.
fn cafes(out: &mut Vec<Placement>, placed: &mut Placed) {
    for cafe in city::instances()
        .into_iter()
        .filter(|i| i.name.starts_with("boardwalk cafe"))
    {
        for local in [[-2.8, 1.5], [3.4, 1.5]] {
            let at = cafe.world(local);
            out.push(
                Placement::new(
                    "generated/cafe_table",
                    at,
                    cafe.yaw + 0.3,
                    Collision::Core(0.3),
                )
                .lift(0.15),
            );
            placed.push((at, 1.3));
        }
    }
    for (k, at) in [[-13.2, 67.5], [-13.2, 76.0], [13.2, 68.0], [13.2, 75.5]]
        .into_iter()
        .enumerate()
    {
        let table = Placement::new(
            "generated/cafe_table",
            at,
            noise(k as u32, 170) * TAU,
            Collision::Core(0.3),
        );
        try_put(out, placed, table, 1.3);
    }
}

/// Planters at the Fountain Plaza's corners and flanking the clock tower.
fn planters(out: &mut Vec<Placement>, placed: &mut Placed) {
    for at in [
        [-9.6, 62.6],
        [9.6, 62.6],
        [-10.2, 83.0],
        [10.2, 83.0],
        [18.6, -26.4],
        [29.4, -26.4],
    ] {
        let planter = Placement::new("generated/planter", at, 0.0, Collision::Core(0.45));
        put(out, placed, planter, 0.6);
    }
}

/// Walden Woods' and Fernhollow's undergrowth: ferns, mossy rocks, fallen
/// logs, stumps, and toadstools at hashed points of open ground; birches
/// and spruces among Fernhollow's pines; and round bushes at the
/// clearing's edge.
fn undergrowth(out: &mut Vec<Placement>, placed: &mut Placed) {
    // (lo, hi, pieces): Walden Woods southwest, Fernhollow northeast.
    let woods: [([f32; 2], [f32; 2]); 2] = [
        ([-132.0, -112.0], [-78.0, -34.0]),
        ([66.0, 54.0], [122.0, 110.0]),
    ];
    let pieces: [(&str, f32, f32, u32); 6] = [
        ("nature/Fern_1", 0.45, 0.8, 16),
        ("generated/mossy_rock", 0.8, 1.4, 7),
        ("generated/fallen_log", 1.0, 1.9, 4),
        ("generated/mushrooms", 1.0, 0.6, 8),
        ("generated/stump", 1.0, 0.8, 4),
        ("generated/bush_round", 1.0, 1.2, 6),
    ];
    for (w, (lo, hi)) in woods.into_iter().enumerate() {
        for (p, &(model, scale, r, count)) in pieces.iter().enumerate() {
            let salt = 200 + 10 * w as u32 + p as u32;
            let mut planted = 0;
            for n in 0..count * 12 {
                if planted == count {
                    break;
                }
                let x = lo[0] + (hi[0] - lo[0]) * noise(n, salt);
                let z = lo[1] + (hi[1] - lo[1]) * noise(n, salt + 50);
                if x.hypot(z) > 132.0 {
                    continue;
                }
                let collision = match model {
                    "generated/mossy_rock" | "generated/fallen_log" => Collision::Bounds,
                    "generated/stump" => Collision::Core(0.4),
                    _ => Collision::None,
                };
                let piece = Placement::new(model, [x, z], noise(n, salt + 100) * TAU, collision)
                    .scale(scale * (0.85 + 0.3 * noise(n, salt + 150)));
                if try_put(out, placed, piece, r) {
                    planted += 1;
                }
            }
        }
    }
    // Fernhollow's birches and spruces, round the Fern Pond.
    let mut planted = 0;
    for n in 0..120_u32 {
        if planted == 10 {
            break;
        }
        let x = 70.0 + 48.0 * noise(n, 230);
        let z = 58.0 + 48.0 * noise(n, 231);
        if x.hypot(z) > 132.0 || !super::streets::clear(x, z, 1.5) {
            continue;
        }
        if placed
            .iter()
            .any(|(p, r)| (p[0] - x).hypot(p[1] - z) < r + 2.0)
        {
            continue;
        }
        let model = if planted % 2 == 0 {
            "generated/birch_low"
        } else {
            "generated/spruce_low"
        };
        placed.push(([x, z], 1.5));
        tree(
            out,
            model,
            [x, z],
            noise(n, 232) * TAU,
            0.85 + 0.35 * noise(n, 233),
        );
        planted += 1;
    }
    // Round bushes and ferns along the clearing's edge, inside the ring.
    let mut planted = 0;
    for n in 0..400_u32 {
        if planted == 40 {
            break;
        }
        let angle = noise(n, 240) * TAU;
        let r = 118.0 + 14.0 * noise(n, 241);
        let at = [angle.cos() * r, angle.sin() * r];
        let (model, scale) = if n % 3 == 0 {
            ("nature/Fern_1", 0.5)
        } else {
            ("generated/bush_round", 0.8 + 0.5 * noise(n, 242))
        };
        let piece = Placement::new(model, at, noise(n, 243) * TAU, Collision::None).scale(scale);
        if try_put(out, placed, piece, 1.1) {
            planted += 1;
        }
    }
}
