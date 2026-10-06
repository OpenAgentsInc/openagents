//! The town's third round of dressing, after the illustrated map
//! (`docs/verse/everglade-map.svg`): Well Square, the second plaza, with
//! its well, benches, lamps, and flowers; paper lanterns strung across
//! Lantern Road and two-armed lamps through the Lantern Quarter; painted
//! signs before Main Street's shops; green and gold stalls on Main Street;
//! the barnyard; benches round Lantern Pond and in Brownstone Row's
//! courtyards; and drifts of spring, summer, and autumn flowers on the
//! lawns.
//!
//! The pieces are the cheap generated models of `scripts/blender/town_props.py`
//! and `market_stall.py`. Loose pieces go through `streets::try_put`, so none
//! stands on a road, a walk, a building, a pond, or a station; the square's
//! pieces are set by rule on its cobbles, which no other piece takes.

use super::streets::{clear, try_put};
use super::{Collision, PONDS, Placement, WELL_SQUARE, city, noise};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A piece already placed: its center and radius, m.
type Placed = Vec<([f32; 2], f32)>;

/// Every placement of the third round's dressing.
pub fn build(out: &mut Vec<Placement>, placed: &mut Placed) {
    well_square(out, placed);
    lanterns(out, placed);
    signs(out, placed);
    stalls(out, placed);
    barnyard(out, placed);
    benches(out, placed);
    flowers(out, placed);
}

/// The heading that points a model's front from `from` toward `to`.
fn toward(from: [f32; 2], to: [f32; 2]) -> f32 {
    (to[0] - from[0]).atan2(to[1] - from[1])
}

/// Sets a piece by rule and records it, so later loose pieces keep clear.
fn set(out: &mut Vec<Placement>, placed: &mut Placed, placement: Placement, r: f32) {
    placed.push((placement.at, r));
    out.push(placement);
}

/// Well Square: the well in the middle of its cobbles, a bench on each side
/// facing it, two-armed lamps at two corners, and a drift of flowers at the
/// others, between the two hipped houses and the lane up to Hearth Road.
fn well_square(out: &mut Vec<Placement>, placed: &mut Placed) {
    let ([cx, cz], [hx, hz]) = WELL_SQUARE;
    let well = Placement::new("generated/well", [cx, cz], 0.3, Collision::Bounds);
    set(out, placed, well, 1.3);
    for (dx, dz) in [(0.0, 3.6), (0.0, -3.6), (-3.6, 0.0), (3.6, 0.0)] {
        let at = [cx + dx, cz + dz];
        let bench = Placement::new(
            "generated/park_bench",
            at,
            toward(at, [cx, cz]),
            Collision::Bounds,
        );
        set(out, placed, bench, 1.0);
    }
    for (sx, sz) in [(-1.0, 1.0), (1.0, -1.0)] {
        let lamp = Placement::new(
            "generated/lamp_double",
            [cx + sx * (hx - 0.8), cz + sz * (hz - 0.8)],
            0.25 * PI,
            Collision::Core(0.2),
        );
        set(out, placed, lamp, 0.6);
    }
    for (k, (sx, sz)) in [(1.0, 1.0), (-1.0, -1.0)].into_iter().enumerate() {
        let patch = Placement::new(
            [
                "generated/flower_patch_spring",
                "generated/flower_patch_summer",
            ][k],
            [cx + sx * (hx - 1.6), cz + sz * (hz - 1.6)],
            noise(k as u32, 400) * TAU,
            Collision::None,
        )
        .scale(0.75);
        set(out, placed, patch, 1.2);
    }
}

/// Paper lanterns strung across Lantern Road between the Lantern Quarter's
/// houses, and two-armed lamps at its crossing with Hearth Road and along
/// both roads.
fn lanterns(out: &mut Vec<Placement>, placed: &mut Placed) {
    for z in [-20.0_f32, 14.0, 28.0] {
        // Both posts must stand on open ground and clear of the lamps.
        let posts = [[-104.0, z], [-96.0, z]];
        let free = posts.iter().all(|&[x, pz]| {
            clear(x, pz, 0.2)
                && !placed
                    .iter()
                    .any(|(p, r)| (p[0] - x).hypot(p[1] - pz) < r + 0.6)
        });
        if free {
            for post in posts {
                placed.push((post, 0.3));
            }
            out.push(Placement::new(
                "generated/lantern_string",
                [-100.0, z],
                0.0,
                Collision::None,
            ));
        }
    }
    let corners = [
        [-103.2, -4.8],
        [-96.8, -11.2],
        [-80.0, -11.6],
        [-70.0, -4.4],
        [-116.0, -11.6],
        [-96.6, 6.0],
        [-103.4, -40.0],
    ];
    for (k, at) in corners.into_iter().enumerate() {
        let lamp = Placement::new(
            "generated/lamp_double",
            at,
            noise(k as u32, 401) * PI,
            Collision::Core(0.2),
        );
        try_put(out, placed, lamp, 0.4);
    }
}

/// A painted sign on a post before each of Main Street's far shops, on the
/// side of its door away from its flower box.
fn signs(out: &mut Vec<Placement>, placed: &mut Placed) {
    for (door, outward) in city::street_fronts() {
        // Main Street's shops only; Stoop Lane's homes have no signs.
        if outward[1] > -0.5 {
            continue;
        }
        let side = [outward[1], -outward[0]];
        let at = [
            door[0] + side[0] * 3.4 + outward[0] * 1.5,
            door[1] + side[1] * 3.4 + outward[1] * 1.5,
        ];
        let yaw = outward[0].atan2(outward[1]) + FRAC_PI_2;
        let sign = Placement::new("generated/shop_sign", at, yaw, Collision::Core(0.1));
        try_put(out, placed, sign, 0.3);
    }
}

/// A gold stall on Main Street's south side, facing the shops across it,
/// east of the Fountain Plaza's corner. (A green stall beside it never
/// found clear ground and left the pack in the sixth round.)
fn stalls(out: &mut Vec<Placement>, placed: &mut Placed) {
    for (model, x) in [("generated/market_stall_gold", 29.0)] {
        let stall = Placement::new(model, [x, 42.4], 0.0, Collision::Bounds);
        if try_put(out, placed, stall, 1.4) {
            let crate_at = [x + 1.8, 42.0];
            let barrel = Placement::new("generated/barrel", crate_at, 0.4, Collision::Bounds);
            try_put(out, placed, barrel, 0.4);
        }
    }
}

/// The barn's yard: bales stacked by its doors, a hand cart, and a wagon.
fn barnyard(out: &mut Vec<Placement>, placed: &mut Placed) {
    let Some(barn) = city::instances().into_iter().find(|i| i.name == "barn") else {
        return;
    };
    for (model, local, yaw, r) in [
        ("generated/hay_bales", [-3.4, 2.6], 0.4, 1.4),
        ("generated/hand_cart", [5.6, 1.4], 2.2, 1.2),
        ("village/Prop_Wagon", [-7.2, -4.0], FRAC_PI_2, 2.2),
    ] {
        let at = barn.world(local);
        let piece = Placement::new(model, at, barn.yaw + yaw, Collision::Bounds);
        try_put(out, placed, piece, r);
    }
}

/// Benches round Lantern Pond facing the water, along the commons walk,
/// and in the courtyards behind Brownstone Row's brownstones, with a
/// planter and a fruit tree each.
fn benches(out: &mut Vec<Placement>, placed: &mut Placed) {
    let ([px, pz], pr) = PONDS[0];
    for angle in [-0.4_f32, -1.4, -2.9, 2.6] {
        let at = [px + angle.cos() * (pr + 3.2), pz + angle.sin() * (pr + 3.2)];
        let bench = Placement::new(
            "generated/park_bench",
            at,
            toward(at, [px, pz]),
            Collision::Bounds,
        );
        try_put(out, placed, bench, 1.0);
    }
    for z in [6.0_f32, 18.0, 30.0] {
        for (x, yaw) in [(-13.4_f32, FRAC_PI_2), (-8.6, -FRAC_PI_2)] {
            let bench = Placement::new("generated/park_bench", [x, z], yaw, Collision::Bounds);
            if try_put(out, placed, bench, 1.0) {
                break;
            }
        }
    }
    for x in [-19.0_f32, -49.0, -69.0] {
        let center = [x, -61.0];
        for (model, dx, dz, yaw, r, collision) in [
            (
                "generated/park_bench",
                -1.4,
                0.0,
                PI,
                1.0,
                Collision::Bounds,
            ),
            (
                "generated/planter",
                1.2,
                0.4,
                0.0,
                0.7,
                Collision::Core(0.45),
            ),
            (
                "generated/fruit_tree",
                0.2,
                2.4,
                0.0,
                1.4,
                Collision::Core(0.3),
            ),
        ] {
            let piece = Placement::new(model, [center[0] + dx, center[1] + dz], yaw, collision);
            try_put(out, placed, piece, r);
        }
    }
}

/// Drifts of one season's flowers each: spring's pastels on the commons,
/// high summer's reds round the town's lawns, and autumn's golds and plums
/// at the woods' edges.
fn flowers(out: &mut Vec<Placement>, placed: &mut Placed) {
    let fields: [(&str, [f32; 2], [f32; 2], usize); 5] = [
        (
            "generated/flower_patch_spring",
            [-30.0, 4.0],
            [34.0, 40.0],
            8,
        ),
        (
            "generated/flower_patch_summer",
            [-96.0, -50.0],
            [-36.0, 40.0],
            9,
        ),
        (
            "generated/flower_patch_summer",
            [36.0, -70.0],
            [110.0, 44.0],
            6,
        ),
        (
            "generated/flower_patch_autumn",
            [-132.0, -110.0],
            [-80.0, -36.0],
            5,
        ),
        (
            "generated/flower_patch_autumn",
            [70.0, 56.0],
            [116.0, 100.0],
            4,
        ),
    ];
    for (f, (model, lo, hi, count)) in fields.into_iter().enumerate() {
        let mut planted = 0;
        for n in 0..count as u32 * 12 {
            if planted == count {
                break;
            }
            let salt = 410 + 4 * f as u32;
            let x = lo[0] + (hi[0] - lo[0]) * noise(n, salt);
            let z = lo[1] + (hi[1] - lo[1]) * noise(n, salt + 1);
            let patch = Placement::new(model, [x, z], noise(n, salt + 2) * TAU, Collision::None)
                .scale(0.8 + 0.4 * noise(n, salt + 3));
            if try_put(out, placed, patch, 1.6) {
                planted += 1;
            }
        }
    }
}

/// The chimneys that smoke, in their generated model's frame: x, height,
/// and z of each chimney's top, m, read from the chimney pieces of each
/// glb that `scripts/blender/buildings.py` builds.
const CHIMNEYS: [(&str, [f32; 3]); 20] = [
    ("generated/bakery", [3.3, 12.69, -5.7]),
    ("generated/smithy", [4.85, 10.53, -4.0]),
    ("generated/log_cabin", [-3.55, 5.87, -2.5]),
    ("generated/farmhouse", [3.57, 6.98, -3.6]),
    ("generated/cottage_thatch", [1.57, 6.4, -3.1]),
    ("generated/cottage_tower", [1.67, 6.84, -4.6]),
    ("generated/tavern", [-4.23, 10.05, -6.2]),
    ("generated/guild_hall", [4.47, 9.96, -8.0]),
    ("generated/hip_house", [-2.63, 9.24, -5.2]),
    ("generated/townhouse_jettied", [-1.63, 13.16, -6.0]),
    ("generated/row_townhouse", [0.87, 12.62, -6.8]),
    ("generated/boardwalk_cafe", [2.47, 6.62, -4.8]),
    // The sixth round's (`scripts/blender/town_houses.py`).
    ("generated/shop_house", [1.9, 10.82, -7.4]),
    ("generated/gambrel_house", [1.4, 8.1, -6.5]),
    ("generated/stone_cottage", [4.35, 7.25, -3.2]),
    ("generated/brownstone", [-3.55, 12.5, -4.4]),
    ("generated/brownstone", [3.55, 12.3, -5.6]),
    ("generated/timber_house", [2.4, 10.02, -5.7]),
    ("generated/lantern_inn", [-3.0, 10.25, -5.1]),
    ("generated/lantern_inn", [3.4, 10.05, -5.9]),
];

/// Where smoke rises from the town's chimneys, a little above each top, m.
#[must_use]
pub fn chimneys() -> Vec<[f32; 3]> {
    super::generated()
        .iter()
        .flat_map(|i| {
            CHIMNEYS
                .iter()
                .filter(|(model, _)| *model == i.model.name)
                .map(|&(_, [x, y, z])| {
                    let [wx, wz] = i.world([x, z]);
                    [wx, super::height(i.at[0], i.at[1]) + y * i.scale + 0.2, wz]
                })
        })
        .collect()
}
