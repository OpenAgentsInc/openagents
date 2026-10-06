//! The town's sixth round of dressing, after the illustrated map
//! (`docs/verse/everglade-map.svg`), round the lighter town houses of
//! `scripts/blender/town_houses.py` ([`city::STAND_INS`]): a busier
//! Fountain Plaza of produce stalls, crates, and a flower cart; plank
//! decks with café tables on Lantern Pond's banks; back gardens behind
//! Brownstone Row's houses, fenced, with beds and a bench; and street
//! furniture before the new houses and along the lanes: crates and flower
//! carts by the shops, benches and barrels by the inns, litter bins, hand
//! pumps, and a small fountain.
//!
//! The pieces are the cheap generated models of `scripts/blender/town_props.py`.
//! Loose pieces go through `streets::try_put`, so none stands on a road, a
//! walk, a building, a pond, or a station; the plaza's and the decks'
//! pieces are set by rule on ground no other piece takes.

use super::generated::Instance;
use super::streets::try_put;
use super::{Collision, PONDS, Placement, city, noise, on_road};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A piece already placed: its center and radius, m.
type Placed = Vec<([f32; 2], f32)>;

/// Every placement of the sixth round's dressing.
pub fn build(out: &mut Vec<Placement>, placed: &mut Placed) {
    plaza(out, placed);
    decks(out, placed);
    let houses: Vec<Instance> = city::instances()
        .into_iter()
        .filter(|i| LIGHT_HOUSES.contains(&i.model.name))
        .collect();
    gardens(out, placed, &houses);
    fronts(out, placed, &houses);
    lanes(out, placed);
}

/// The sixth round's houses, which this round dresses.
pub const LIGHT_HOUSES: [&str; 6] = [
    "generated/shop_house",
    "generated/gambrel_house",
    "generated/stone_cottage",
    "generated/brownstone",
    "generated/timber_house",
    "generated/lantern_inn",
];

/// The eighth round's townhouses, which the zone paints like the sixth
/// round's (`layout::paint`) but this round doesn't dress.
pub const TOWNHOUSES: [&str; 3] = [
    "generated/tall_house",
    "generated/narrow_house",
    "generated/dormer_house",
];

/// The heading that points a model's front from `from` toward `to`.
fn toward(from: [f32; 2], to: [f32; 2]) -> f32 {
    (to[0] - from[0]).atan2(to[1] - from[1])
}

/// Sets a piece by rule and records it, so later loose pieces keep clear.
fn set(out: &mut Vec<Placement>, placed: &mut Placed, placement: Placement, r: f32) {
    placed.push((placement.at, r));
    out.push(placement);
}

/// Whether no earlier piece stands within `r` of `at`.
fn free(placed: &Placed, at: [f32; 2], r: f32) -> bool {
    !placed
        .iter()
        .any(|(p, pr)| (p[0] - at[0]).hypot(p[1] - at[1]) < pr + r + 0.2)
}

/// The Fountain Plaza as a market: a produce stall between the two
/// awninged stalls on its east side and one on its west side facing
/// Market Way, with crates, a flower cart, and a hand pump.
fn plaza(out: &mut Vec<Placement>, placed: &mut Placed) {
    let ([px, pz], _) = city::PLAZA;
    // The cafés' walks cross the plaza a little south of its middle, so
    // the stalls stand north and south of them.
    let pieces = [
        // Facing the fountain, between the walk and the blue stall.
        ("generated/produce_stall", [6.6, 74.0], -FRAC_PI_2, 1.4),
        // Facing Market Way at the plaza's north-west corner.
        ("generated/produce_stall", [-9.0, 79.6], FRAC_PI_2, 1.4),
        ("generated/flower_cart", [-3.6, 66.2], 0.4, 1.3),
        ("generated/crate_stack", [-3.4, 69.5], -0.2, 1.0),
        ("generated/water_pump", [px + 3.6, pz - 5.5], PI, 0.9),
    ];
    for (model, at, yaw, r) in pieces {
        if fits(placed, at, r) {
            set(
                out,
                placed,
                Placement::new(model, at, yaw, Collision::Bounds),
                r,
            );
        }
    }
}

/// Plank decks on Lantern Pond's west and south banks, their rails over
/// the water and their open sides to the lawn, with café tables and a
/// parasol on each.
fn decks(out: &mut Vec<Placement>, placed: &mut Placed) {
    let ([cx, cz], r) = PONDS[0];
    // Each deck's center stands 0.6 m back from the water's edge, so its
    // railed side reaches 0.9 m over the water.
    // Round the bank from the west, the first two places that are free.
    let mut decks = 0;
    for k in 0..16 {
        if decks == 2 {
            break;
        }
        let angle = PI + k as f32 * TAU / 16.0;
        let (dx, dz) = (angle.cos(), angle.sin());
        let at = [cx + dx * (r + 0.6), cz + dz * (r + 0.6)];
        let land = [cx + dx * (r + 10.0), cz + dz * (r + 10.0)];
        let yaw = toward(at, land);
        let along = [dz, -dx];
        let span = [-2.2_f32, 0.0, 2.2].map(|s| [at[0] + along[0] * s, at[1] + along[1] * s]);
        if span
            .iter()
            .any(|p| !free(placed, *p, 1.4) || on_road(p[0], p[1], 1.5))
        {
            continue;
        }
        decks += 1;
        for p in span {
            placed.push((p, 1.4));
        }
        out.push(Placement::new(
            "generated/boardwalk",
            at,
            yaw,
            Collision::None,
        ));
        // Along the deck: two tables, and a parasol over the middle.
        for (k, s) in [-1.8_f32, 0.0, 1.8].into_iter().enumerate() {
            let p = [at[0] + along[0] * s, at[1] + along[1] * s];
            let model = if k == 1 {
                "generated/cafe_umbrella"
            } else {
                "generated/cafe_table"
            };
            out.push(
                Placement::new(
                    model,
                    p,
                    yaw + noise(k as u32, 600) * 0.6,
                    Collision::Core(0.4),
                )
                .lift(0.1),
            );
        }
    }
}

/// Back gardens behind the brownstones and the row houses across
/// Brownstone Row: a picket fence along the far side, a flower bed and a
/// vegetable bed, and a bench facing the house.
fn gardens(out: &mut Vec<Placement>, placed: &mut Placed, houses: &[Instance]) {
    for (k, house) in houses.iter().enumerate() {
        let row = house.name.starts_with("brownstone") || house.name.starts_with("row house");
        if !row {
            continue;
        }
        // The model's back wall stands 8 to 9 m behind its front.
        let back = -house.model.blocks[0][2].abs() - 0.9;
        let inward = house.yaw + PI;
        for x in [-3.0_f32, -1.0, 1.0, 3.0] {
            let fence = Placement::new(
                "generated/picket_fence",
                house.world([x, back - 4.4]),
                inward,
                Collision::Bounds,
            );
            try_put(out, placed, fence, 0.6);
        }
        let beds = if k % 2 == 0 {
            ["generated/flower_bed", "generated/veg_bed"]
        } else {
            ["generated/veg_bed", "generated/flower_bed"]
        };
        for (bed, x) in beds.into_iter().zip([-2.0_f32, 1.8]) {
            let p = Placement::new(
                bed,
                house.world([x, back - 1.6]),
                house.yaw,
                Collision::Bounds,
            );
            try_put(out, placed, p, 0.9);
        }
        let bench = Placement::new(
            "generated/park_bench",
            house.world([0.0, back - 3.3]),
            house.yaw + PI,
            Collision::Bounds,
        );
        try_put(out, placed, bench, 0.8);
    }
}

/// Street furniture before each of the new houses, beside its door on the
/// side away from its walk: crates and a flower cart by the shops, a bench
/// and barrels by the inns, and a litter bin or a planter by the homes.
fn fronts(out: &mut Vec<Placement>, placed: &mut Placed, houses: &[Instance]) {
    for (k, house) in houses.iter().enumerate() {
        // A brownstone's areaway rail and stoop fill its front.
        if house.model.name == "generated/brownstone" {
            continue;
        }
        let [fx, _] = house.model.front;
        // Beside the door, away from the walk's end, about a meter out.
        let side = if fx > 0.5 { -1.0 } else { 1.0 };
        let local = |x: f32, z: f32| house.world([fx + side * x, z]);
        let outward = house.outward();
        let facing = outward[0].atan2(outward[1]);
        let pieces: &[(&'static str, f32, f32, f32, Collision)] = match house.model.name {
            "generated/shop_house" => &[
                ("generated/crate_stack", 2.4, 0.9, 0.9, Collision::Bounds),
                ("generated/flower_cart", -2.6, 1.0, 1.0, Collision::Bounds),
            ],
            "generated/lantern_inn" => &[
                ("generated/park_bench", 2.6, 1.2, 0.9, Collision::Bounds),
                ("generated/barrel", -2.4, 1.0, 0.4, Collision::Bounds),
                ("generated/barrel", -3.1, 1.3, 0.4, Collision::Bounds),
            ],
            _ if k % 3 == 0 => &[("generated/street_bin", 1.6, 1.0, 0.4, Collision::Core(0.25))],
            _ if k % 3 == 1 => &[("generated/planter", 1.7, 1.0, 0.6, Collision::Bounds)],
            _ => &[],
        };
        for &(model, x, z, r, collision) in pieces {
            let yaw = if model == "generated/park_bench" {
                facing
            } else {
                facing + noise(k as u32, 610) * 0.5
            };
            let at = local(x, z);
            if fits(placed, at, r) {
                set(out, placed, Placement::new(model, at, yaw, collision), r);
            }
        }
    }
}

/// Whether a piece reaching `r` meters from `at` stands clear of the roads
/// and walks, earlier pieces, and every generated model's blocks: a
/// looser test than `streets::clear`, for pieces set against a house's
/// front or on the plaza's cobbles.
fn fits(placed: &Placed, at: [f32; 2], r: f32) -> bool {
    let [x, z] = at;
    free(placed, at, r)
        && !on_road(x, z, r * 0.7)
        && !super::generated()
            .iter()
            .flat_map(|i| i.blocks())
            .any(|(f, _)| f.contains(x, z, r * 0.6))
        && !super::city::kit_blocks()
            .iter()
            .any(|(f, _)| f.contains(x, z, r * 0.6))
}

/// Along the lanes: hand pumps at Market Row's ends and on Stoop Lane, a
/// small fountain on the lawn by Brownstone Row's footbridge and another
/// at Market Row's east end, and litter bins by the paved streets' lamps.
fn lanes(out: &mut Vec<Placement>, placed: &mut Placed) {
    let row = city::MARKET_ROW;
    for (k, at) in [
        [26.6, row + 2.8],
        [-26.6, row - 2.8],
        [-31.2, 8.0],
        [-31.2, -24.0],
        [61.2, 52.0],
    ]
    .into_iter()
    .enumerate()
    {
        let pump = Placement::new(
            "generated/water_pump",
            at,
            noise(k as u32, 620) * TAU,
            Collision::Bounds,
        );
        try_put(out, placed, pump, 0.9);
    }
    for at in [
        [24.0, row + 6.5],
        [-48.0, -50.0],
        [36.0, -58.0],
        [-14.0, -56.0],
    ] {
        let fountain = Placement::new("generated/fountain_small", at, 0.0, Collision::Bounds);
        try_put(out, placed, fountain, 1.8);
    }
    // A litter bin about every 30 m on each side of Main Street, Library
    // Way, and Brownstone Row.
    let mut bins = 0;
    for (k, (a, b, half)) in [
        ([-96.0_f32, 46.0_f32], [100.0_f32, 46.0_f32], 2.5_f32),
        ([6.0, -29.0], [100.0, -29.0], 2.0),
        ([-96.0, -78.0], [16.0, -78.0], 2.0),
    ]
    .into_iter()
    .enumerate()
    {
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let steps = (length / 30.0) as u32;
        for i in 0..=steps {
            let t = (i as f32 + 0.5 * (k % 2) as f32) / steps.max(1) as f32;
            let x = a[0] + (b[0] - a[0]) * t.min(1.0);
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let at = [x + 1.3, a[1] + side * (half + 0.7)];
            let bin = Placement::new(
                "generated/street_bin",
                at,
                if side > 0.0 { PI } else { 0.0 },
                Collision::Core(0.25),
            );
            if try_put(out, placed, bin, 0.4) {
                bins += 1;
            }
        }
    }
    debug_assert!(bins > 0);
}
