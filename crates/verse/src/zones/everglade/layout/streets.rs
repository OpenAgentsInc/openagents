//! The town's street furniture and dressing, after the illustrated map
//! (`docs/verse/everglade-map.svg`): warm lamps along the paved streets and
//! the main lanes, flower boxes under the shop fronts, clipped hedges
//! between Main Street and the commons, bunting over the plaza and the
//! streets, signposts at the crossings, wells, lily pads on the ponds, a
//! low stone wall around the orchard, meadows of wildflowers, park trees
//! on the commons, and dense woods at the town's edges.
//!
//! Every piece is a cheap generated model (`scripts/blender/street_props.py`)
//! or a kit tree, placed by rule or at a hashed point and kept only where
//! [`clear`] finds open ground, so none stands on a road, a walk, a
//! building, a pond, or a station.

use super::super::{CLEARING_RADIUS, HALF_EXTENT};
use super::generated::Instance;
use super::{
    Collision, PAVED, PONDS, Placement, RETURN_PORTAL, STATIONS, YARD, city, dress, inside, noise,
    on_road, reserved, tree,
};
use std::f32::consts::{FRAC_PI_2, TAU};

/// Spacing of the lamps along a paved street and along a lane, m.
const LAMP_STEP: f32 = 15.0;
const LANE_LAMP_STEP: f32 = 22.0;
/// Stands of low-poly trees in the forest belt beyond the tree ring, and
/// the trees in each.
const BELT_STANDS: u32 = 64;
const STAND_TREES: u32 = 11;
/// The lanes that get lamps too, besides the paved streets: Stoop Lane,
/// Lantern Road, Foundry Road, Studio Road, and the Foundry's east road.
const LIT_LANES: [([f32; 2], [f32; 2], f32); 5] = [
    ([-34.0, -40.0], [-34.0, 46.0], 1.4),
    ([-100.0, -78.0], [-100.0, 46.0], 1.4),
    ([64.0, -78.0], [64.0, 46.0], 1.4),
    ([64.0, 29.0], [112.0, 29.0], 1.3),
    ([64.0, 0.0], [112.0, 0.0], 1.3),
];

/// Whether a piece reaching `r` meters from `(x, z)` stands on open
/// ground: off every road and walk, every building's reserved ground and
/// generated model, the ponds, the yard, the return portal, and the
/// stations, and inside the clearing.
pub(super) fn clear(x: f32, z: f32, r: f32) -> bool {
    let near = |p: [f32; 2], d: f32| (p[0] - x).hypot(p[1] - z) < d;
    x.hypot(z) < CLEARING_RADIUS - 2.0 - r
        && !on_road(x, z, r + 0.25)
        && !reserved().iter().any(|rect| inside(*rect, x, z, r + 0.3))
        && !super::generated()
            .iter()
            .flat_map(Instance::blocks)
            .any(|(f, _)| f.contains(x, z, r + 0.4))
        && !PONDS.iter().any(|(c, pr)| near(*c, pr + r + 1.6))
        && super::stream_distance(x, z) > super::STREAM_HALF + r + 1.4
        && !inside(YARD, x, z, r + 1.0)
        && !near([RETURN_PORTAL.x, RETURN_PORTAL.z], r + 3.5)
        && !STATIONS.iter().any(|s| near(s.at, r + 2.2))
}

/// Every placement of the street furniture and dressing.
pub fn build(out: &mut Vec<Placement>) {
    let mut placed: Vec<([f32; 2], f32)> = Vec::new();
    let ([bx, bz], yaw) = super::BRIDGE;
    out.push(Placement::new(
        "generated/footbridge",
        [bx, bz],
        yaw,
        Collision::None,
    ));
    placed.push(([bx, bz], 3.6));
    lamps(out, &mut placed);
    fronts(out, &mut placed);
    hedges(out, &mut placed);
    bunting(out);
    signposts(out, &mut placed);
    wells(out, &mut placed);
    ponds(out);
    orchard_wall(out, &mut placed);
    meadows(out, &mut placed);
    park(out, &mut placed);
    woods(out);
}

/// Places `model` at `at` when the ground is clear for `r` meters and no
/// earlier piece stands there; returns whether it did.
fn try_put(
    out: &mut Vec<Placement>,
    placed: &mut Vec<([f32; 2], f32)>,
    placement: Placement,
    r: f32,
) -> bool {
    let [x, z] = placement.at;
    let crowded = placed
        .iter()
        .any(|(p, pr)| (p[0] - x).hypot(p[1] - z) < pr + r + 0.3);
    if crowded || !clear(x, z, r) {
        return false;
    }
    placed.push((placement.at, r));
    out.push(placement);
    true
}

/// Unit direction of a segment and its left normal.
fn frame(a: [f32; 2], b: [f32; 2]) -> ([f32; 2], [f32; 2], f32) {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dz);
    let t = [dx / length, dz / length];
    (t, [-t[1], t[0]], length)
}

/// Lamps on both sides of every paved street, alternating, and on one
/// side of the lit lanes.
fn lamps(out: &mut Vec<Placement>, placed: &mut Vec<([f32; 2], f32)>) {
    let streets = PAVED
        .iter()
        .map(|s| (*s, LAMP_STEP, true))
        .chain(LIT_LANES.iter().map(|s| (*s, LANE_LAMP_STEP, false)));
    for ((a, b, half), step, both) in streets {
        let (t, n, length) = frame(a, b);
        let mut k = 0;
        let mut along = step / 2.0;
        while along < length {
            let side = if both && k % 2 == 1 { -1.0 } else { 1.0 };
            for s in [side, -side] {
                let offset = half + 0.9;
                let at = [
                    a[0] + t[0] * along + n[0] * offset * s,
                    a[1] + t[1] * along + n[1] * offset * s,
                ];
                let lamp = Placement::new("generated/lamp_post", at, 0.0, Collision::Core(0.15));
                // The other side when this one is taken.
                if try_put(out, placed, lamp, 0.25) {
                    break;
                }
            }
            k += 1;
            along += step;
        }
    }
}

/// Flower boxes against the front wall of every building on Main Street
/// and Stoop Lane, either side of its door.
fn fronts(out: &mut Vec<Placement>, placed: &mut Vec<([f32; 2], f32)>) {
    for (door, outward) in city::street_fronts() {
        let side = [outward[1], -outward[0]];
        for s in [-2.4_f32, 2.4] {
            let at = [
                door[0] + side[0] * s + outward[0] * 0.4,
                door[1] + side[1] * s + outward[1] * 0.4,
            ];
            // Against the wall, so only the walks and the other pieces
            // need to be clear.
            let crowded = placed
                .iter()
                .any(|(p, r)| (p[0] - at[0]).hypot(p[1] - at[1]) < r + 0.9);
            if crowded || on_road(at[0], at[1], 0.75) {
                continue;
            }
            let yaw = outward[0].atan2(outward[1]);
            placed.push((at, 0.6));
            out.push(Placement::new(
                "generated/flower_box",
                at,
                yaw,
                Collision::Bounds,
            ));
        }
    }
}

/// A clipped hedge along Main Street's south side, between the street and
/// the commons, open where the commons walk and the stalls meet it.
fn hedges(out: &mut Vec<Placement>, placed: &mut Vec<([f32; 2], f32)>) {
    let mut x: f32 = -32.0;
    while x < 0.0 {
        // Leave the bunting's pole room.
        if (x + 16.0).abs() > 1.6 {
            let hedge = Placement::new("generated/hedge", [x, 41.6], 0.0, Collision::Bounds);
            try_put(out, placed, hedge, 1.15);
        }
        x += 2.3;
    }
}

/// Bunting over Market Way at the plaza, over Main Street between the
/// shops, and over Hearth Road in the Lantern Quarter.
fn bunting(out: &mut Vec<Placement>) {
    for (at, yaw) in [
        ([0.0, 63.0], 0.0),
        ([-16.0, 46.0], FRAC_PI_2),
        ([-104.0, -8.0], FRAC_PI_2),
        ([-56.0, -8.0], FRAC_PI_2),
    ] {
        out.push(Placement::new(
            "generated/bunting",
            at,
            yaw,
            Collision::None,
        ));
    }
}

/// Signposts at the main crossings.
fn signposts(out: &mut Vec<Placement>, placed: &mut Vec<([f32; 2], f32)>) {
    for (k, at) in [
        [3.2, 49.0],
        [-14.0, 49.0],
        [-14.0, -6.0],
        [-37.0, -11.0],
        [-103.0, -5.0],
        [67.0, 3.2],
        [23.0, -26.0],
        [67.0, -26.0],
        [-31.0, -75.0],
        [61.0, 49.0],
    ]
    .into_iter()
    .enumerate()
    {
        let post = Placement::new(
            "generated/signpost",
            at,
            noise(k as u32, 110) * TAU,
            Collision::Core(0.1),
        );
        try_put(out, placed, post, 0.3);
    }
}

/// Wells on Stoop Lane's green, in the Lantern Quarter, by Brownstone
/// Row's garden, and in the Creative District.
fn wells(out: &mut Vec<Placement>, placed: &mut Vec<([f32; 2], f32)>) {
    for at in [
        [-47.0, -6.0],
        [-92.0, -40.0],
        [-80.0, -56.0],
        [86.0, 47.0],
        [-26.0, 8.0],
    ] {
        let well = Placement::new("generated/well", at, 0.3, Collision::Bounds);
        try_put(out, placed, well, 1.3);
    }
}

/// Lily pads floating on each pond.
fn ponds(out: &mut Vec<Placement>) {
    for (k, &([x, z], r)) in PONDS.iter().enumerate() {
        for i in 0..2 {
            let angle = noise(k as u32 * 2 + i, 111) * TAU;
            let at = [x + angle.cos() * r * 0.45, z + angle.sin() * r * 0.45];
            out.push(
                Placement::new("generated/lily_pads", at, angle, Collision::None)
                    .lift(super::super::draw::WATER_LIFT + 0.01),
            );
        }
    }
}

/// A low dry-stone wall along the orchard's south and east sides.
fn orchard_wall(out: &mut Vec<Placement>, placed: &mut Vec<([f32; 2], f32)>) {
    let ([ox, oz], [ohx, ohz]) = city::ORCHARD;
    let (west, east, south, north) = (ox - ohx, ox + ohx, oz - ohz - 1.0, oz + ohz);
    let mut x = west + 1.0;
    while x < east {
        let wall = Placement::new("generated/stone_wall", [x, south], 0.0, Collision::Bounds);
        try_put(out, placed, wall, 1.05);
        x += 2.0;
    }
    let mut z = south + 2.0;
    while z < north {
        let wall = Placement::new(
            "generated/stone_wall",
            [east + 1.0, z],
            FRAC_PI_2,
            Collision::Bounds,
        );
        try_put(out, placed, wall, 1.05);
        z += 2.0;
    }
}

/// Wildflowers in the long meadow south of the Knowledge District, on the
/// commons' lawns, and in Fernhollow's clearing.
fn meadows(out: &mut Vec<Placement>, placed: &mut Vec<([f32; 2], f32)>) {
    let fields: [([f32; 2], [f32; 2], usize); 4] = [
        ([8.0, -112.0], [62.0, -48.0], 30),
        ([-30.0, 4.0], [34.0, 40.0], 14),
        ([70.0, 56.0], [110.0, 96.0], 12),
        ([-128.0, 50.0], [-60.0, 100.0], 12),
    ];
    for (f, (lo, hi, count)) in fields.into_iter().enumerate() {
        let mut planted = 0;
        for n in 0..count as u32 * 8 {
            if planted == count {
                break;
            }
            let salt = 120 + 3 * f as u32;
            let x = lo[0] + (hi[0] - lo[0]) * noise(n, salt);
            let z = lo[1] + (hi[1] - lo[1]) * noise(n, salt + 1);
            let flowers = Placement::new(
                "generated/wildflowers",
                [x, z],
                noise(n, salt + 2) * TAU,
                Collision::None,
            )
            .scale(0.8 + 0.5 * noise(n, salt + 3));
            if try_put(out, placed, flowers, 1.0) {
                planted += 1;
            }
        }
    }
}

/// Park trees on the commons, around Lantern Pond and the bandshell's
/// lawn, as the map draws them.
fn park(out: &mut Vec<Placement>, placed: &mut Vec<([f32; 2], f32)>) {
    let models = [
        "nature/CommonTree_1",
        "nature/CommonTree_3",
        "nature/CommonTree_4",
    ];
    let mut planted = 0;
    for n in 0..200_u32 {
        if planted == 9 {
            break;
        }
        let x = -30.0 + 62.0 * noise(n, 130);
        let z = 6.0 + 34.0 * noise(n, 131);
        let [cx, cz] = [x, z];
        let probe = Placement::new(models[planted % 3], [cx, cz], 0.0, Collision::Core(0.4));
        // Room for the crown: nothing else within its spread.
        if placed
            .iter()
            .any(|(p, r)| (p[0] - cx).hypot(p[1] - cz) < r + 3.5)
            || !clear(cx, cz, 2.5)
        {
            continue;
        }
        placed.push((probe.at, 2.5));
        tree(
            out,
            models[planted % 3],
            [cx, cz],
            noise(n, 132) * TAU,
            0.85 + 0.3 * noise(n, 133),
        );
        planted += 1;
    }
}

/// Dense woods beyond the town: Walden Woods to the southwest, Fernhollow
/// to the northeast, and the forest belt behind the tree ring, in the
/// cheap low-poly trees past the kit's.
fn woods(out: &mut Vec<Placement>) {
    // Stands of trees around hashed centers, so the forest reads as groves
    // and glades rather than an even sprinkle.
    for n in 0..BELT_STANDS * STAND_TREES {
        let stand = n / STAND_TREES;
        let angle = (stand as f32 + 0.5 * noise(stand, 145)) / BELT_STANDS as f32 * TAU;
        let r = 172.0 + 58.0 * noise(stand, 146);
        let spread = 6.0 + 9.0 * noise(stand, 147);
        let around = noise(n, 140) * TAU;
        let d = spread * noise(n, 141).sqrt();
        let (x, z) = (
            angle.cos() * r + around.cos() * d,
            angle.sin() * r + around.sin() * d,
        );
        if x.abs() > HALF_EXTENT - 6.0 || z.abs() > HALF_EXTENT - 6.0 {
            continue;
        }
        let model = if noise(n, 142) < 0.8 {
            "generated/pine_low"
        } else {
            "generated/oak_low"
        };
        out.push(
            Placement::new(model, [x, z], noise(n, 143) * TAU, Collision::Core(0.35))
                .scale(0.9 + 0.6 * noise(n, 144))
                .lift(-0.1),
        );
    }
    // Thicker groves in Walden Woods and Fernhollow, on open ground.
    for (k, (center, spread, count)) in [
        ([-118.0, -96.0], 22.0, 22),
        ([-130.0, -40.0], 14.0, 10),
        ([104.0, 92.0], 18.0, 14),
        ([118.0, 60.0], 12.0, 8),
    ]
    .into_iter()
    .enumerate()
    {
        let mut planted = 0;
        for n in 0..count as u32 * 10 {
            if planted == count {
                break;
            }
            let salt = 150 + 4 * k as u32;
            let angle = noise(n, salt) * TAU;
            let r = spread * noise(n, salt + 1).sqrt();
            let [x, z] = [center[0] + angle.cos() * r, center[1] + angle.sin() * r];
            if x.hypot(z) > 200.0 || !super::open_ground(x, z) {
                continue;
            }
            let model = if (planted + k) % 3 == 0 {
                "generated/oak_low"
            } else {
                "generated/pine_low"
            };
            out.push(
                Placement::new(
                    model,
                    [x, z],
                    noise(n, salt + 2) * TAU,
                    Collision::Core(0.35),
                )
                .scale(0.8 + 0.5 * noise(n, salt + 3))
                .lift(-0.1),
            );
            if k % 2 == 1 || planted % 3 == 0 {
                dress(
                    out,
                    "nature/Fern_1",
                    [x + 1.6, z - 1.1],
                    noise(n, salt + 4) * TAU,
                    0.4,
                );
            }
            planted += 1;
        }
    }
}
