//! The town's fourth round, after the illustrated map
//! (`docs/verse/everglade-map.svg`), which the far levels of detail made
//! room for: the commons wooded as the map draws it, trees along Main
//! Street, Library Way, and Brownstone Row, an orchard in spring bloom,
//! parasols over the Boardwalk Cafés' tables and a waterside café on Lantern
//! Pond, a stepping-stone path with lamps and benches up Observatory Hill,
//! and Fernhollow's ferny glen round the Fern Pond.
//!
//! Loose pieces go through `streets::try_put`, so none stands on a road, a
//! walk, a building, a pond, or a station.

use super::streets::{clear, try_put};
use super::{Collision, PONDS, Placement, city, noise, tree};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A piece already placed: its center and radius, m.
type Placed = Vec<([f32; 2], f32)>;

/// Every placement of the fourth round.
pub fn build(out: &mut Vec<Placement>, placed: &mut Placed) {
    commons_grove(out, placed);
    street_trees(out, placed);
    blossom(out, placed);
    cafes(out, placed);
    observatory_hill(out, placed);
    fern_glen(out, placed);
}

/// A tree on open ground with room for its crown: nothing else within
/// `crown` m, and clear of roads and buildings by `trunk` m.
fn crowned(
    out: &mut Vec<Placement>,
    placed: &mut Placed,
    model: &'static str,
    at: [f32; 2],
    (crown, trunk): (f32, f32),
    yaw: f32,
    scale: f32,
) -> bool {
    let [x, z] = at;
    if placed
        .iter()
        .any(|(p, r)| (p[0] - x).hypot(p[1] - z) < r + crown)
        || !clear(x, z, trunk)
    {
        return false;
    }
    placed.push((at, crown * 0.6));
    tree(out, model, at, yaw, scale);
    true
}

/// More park trees on the commons, in clumps, as the map's great lawn
/// draws them, leaving the lawn round the bandshell open.
fn commons_grove(out: &mut Vec<Placement>, placed: &mut Placed) {
    const MODELS: [&str; 4] = [
        "nature/CommonTree_1",
        "nature/CommonTree_3",
        "nature/CommonTree_4",
        "nature/CommonTree_5",
    ];
    let mut planted = 0;
    for n in 0..600_u32 {
        if planted == 16 {
            break;
        }
        let at = [-32.0 + 66.0 * noise(n, 500), 4.0 + 40.0 * noise(n, 501)];
        let model = MODELS[n as usize % MODELS.len()];
        let scale = 0.7 + 0.3 * noise(n, 502);
        if crowned(
            out,
            placed,
            model,
            at,
            (2.2, 1.4),
            noise(n, 503) * TAU,
            scale,
        ) {
            planted += 1;
        }
    }
}

/// Trees along Main Street, Library Way, and Brownstone Row, in the
/// verges beside their cobbles.
fn street_trees(out: &mut Vec<Placement>, placed: &mut Placed) {
    for (k, (z, from, to, side)) in [
        (46.0_f32, -96.0_f32, 100.0_f32, 4.8_f32),
        (-29.0, 44.0, 100.0, -4.4),
        (-78.0, -96.0, 16.0, 4.4),
    ]
    .into_iter()
    .enumerate()
    {
        let mut x = from;
        let mut n = 0_u32;
        while x <= to {
            let model = if (n + k as u32) % 2 == 0 {
                "nature/CommonTree_4"
            } else {
                "nature/CommonTree_3"
            };
            let salt = 510 + 4 * k as u32;
            let scale = 0.55 + 0.15 * noise(n, salt);
            // The verge on either side, wherever there is room.
            for s in [side, -side] {
                let at = [x + 2.0 * noise(n, salt + 1) - 1.0, z + s];
                let yaw = noise(n, salt + 2) * TAU;
                if crowned(out, placed, model, at, (1.8, 0.9), yaw, scale) {
                    break;
                }
            }
            x += 13.0;
            n += 1;
        }
    }
}

/// An orchard in spring bloom: rows of blossoming fruit trees on the
/// lawns west of the Lantern Quarter and by the beekeeper's hut, and a
/// blossoming tree among each courtyard's fruit trees where there is room.
fn blossom(out: &mut Vec<Placement>, placed: &mut Placed) {
    for (k, (corner, rows, cols)) in [
        ([-128.0_f32, 14.0_f32], 3, 4),
        ([-128.0, -30.0], 3, 3),
        ([-62.0, 56.0], 2, 5),
    ]
    .into_iter()
    .enumerate()
    {
        for row in 0..rows {
            for col in 0..cols {
                let n = (k * 32 + row * 8 + col) as u32;
                let at = [
                    corner[0] + col as f32 * 5.0 + 0.6 * noise(n, 520),
                    corner[1] + row as f32 * 5.0 + 0.6 * noise(n, 521),
                ];
                let tree = Placement::new(
                    "generated/fruit_tree_bloom",
                    at,
                    noise(n, 522) * TAU,
                    Collision::Core(0.3),
                )
                .scale(0.9 + 0.25 * noise(n, 523));
                try_put(out, placed, tree, 1.6);
            }
        }
    }
}

/// Parasols over tables on the Boardwalk Cafés' fronts and a waterside
/// café on Lantern Pond's south bank.
fn cafes(out: &mut Vec<Placement>, placed: &mut Placed) {
    for cafe in city::instances()
        .into_iter()
        .filter(|i| i.model.name == "generated/boardwalk_cafe")
    {
        let [fx, fz] = cafe.front();
        let [ox, oz] = cafe.outward();
        let side = [oz, -ox];
        let mut set = 0;
        'spots: for out_by in [2.4_f32, 3.6, 5.0, 6.4] {
            for across in [-4.4_f32, 4.4, -2.6, 2.6, 0.0] {
                if set == 2 {
                    break 'spots;
                }
                let at = [
                    fx + ox * out_by + side[0] * across,
                    fz + oz * out_by + side[1] * across,
                ];
                let parasol =
                    Placement::new("generated/cafe_umbrella", at, 0.3, Collision::Core(0.5));
                if try_put(out, placed, parasol, 1.2) {
                    set += 1;
                }
            }
        }
    }
    let ([px, pz], pr) = PONDS[0];
    let mut set = 0;
    for k in 0..24_u32 {
        if set == 3 {
            break;
        }
        let angle = k as f32 / 24.0 * TAU;
        let r = pr + 4.0;
        let at = [px + angle.cos() * r, pz + angle.sin() * r];
        let parasol = Placement::new(
            "generated/cafe_umbrella",
            at,
            noise(k, 530) * TAU,
            Collision::Core(0.5),
        );
        if try_put(out, placed, parasol, 1.2) {
            set += 1;
            for s in [-1.0_f32, 1.0] {
                let bench = Placement::new(
                    "generated/park_bench",
                    [at[0] + s * 1.5, at[1]],
                    s * FRAC_PI_2 - FRAC_PI_2,
                    Collision::Bounds,
                )
                .scale(0.6);
                try_put(out, placed, bench, 0.6);
            }
        }
    }
}

/// A stepping-stone path up Observatory Hill from Library Way, with lamps
/// beside it and benches at its turns.
fn observatory_hill(out: &mut Vec<Placement>, placed: &mut Placed) {
    let Some(observatory) = city::instances()
        .into_iter()
        .find(|i| i.name == "observatory")
    else {
        return;
    };
    let top = observatory.front();
    let start = [top[0] - 6.0, -33.0];
    let steps = 14;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        // A gentle S up the slope.
        let wiggle = 3.0 * (t * PI * 2.0).sin();
        let at = [
            start[0] + (top[0] - start[0]) * t + wiggle,
            start[1] + (top[1] - start[1]) * t,
        ];
        if !clear(at[0], at[1], 0.4) {
            continue;
        }
        out.push(
            Placement::new(
                "nature/RockPath_Round_Wide",
                at,
                noise(i, 540) * TAU,
                Collision::None,
            )
            .scale(0.7)
            .lift(-0.05),
        );
        let side = if i % 2 == 0 { 2.2 } else { -2.2 };
        let beside = [at[0] + side, at[1]];
        if i % 4 == 2 {
            let lamp = Placement::new("generated/lamp_post", beside, 0.0, Collision::Core(0.15));
            try_put(out, placed, lamp, 0.4);
        } else if i % 7 == 4 {
            let bench = Placement::new(
                "generated/park_bench",
                beside,
                if side > 0.0 { -FRAC_PI_2 } else { FRAC_PI_2 },
                Collision::Bounds,
            );
            try_put(out, placed, bench, 1.0);
        }
    }
}

/// Fernhollow's glen: ferns, broad-leaved plants, mossy rocks, and
/// toadstools thick round the Fern Pond.
fn fern_glen(out: &mut Vec<Placement>, placed: &mut Placed) {
    let ([px, pz], pr) = PONDS[3];
    let mut planted = 0;
    for n in 0..160_u32 {
        if planted == 34 {
            break;
        }
        let angle = noise(n, 550) * TAU;
        let r = pr + 2.2 + 11.0 * noise(n, 551);
        let at = [px + angle.cos() * r, pz + angle.sin() * r];
        let (model, radius, scale) = match n % 6 {
            0 | 1 | 2 => ("nature/Fern_1", 0.7, 0.9 + 0.5 * noise(n, 552)),
            3 => ("nature/Plant_1_Big", 0.8, 0.8 + 0.4 * noise(n, 552)),
            4 => ("generated/mossy_rock", 0.8, 0.7 + 0.5 * noise(n, 552)),
            _ => ("generated/mushrooms", 0.5, 0.8),
        };
        let collision = if model == "generated/mossy_rock" {
            Collision::Core(0.4)
        } else {
            Collision::None
        };
        let piece = Placement::new(model, at, noise(n, 553) * TAU, collision).scale(scale);
        if try_put(out, placed, piece, radius) {
            planted += 1;
        }
    }
}
