//! Everglade's water (`docs/verse/water.md`, phase W3): four ponds and
//! Glade Run, as `physics::water` bodies, and the beds carved into the
//! heightfield ([`super::everglade::height`]) so the bed and the walking
//! ground are one surface.
//!
//! - Each pond is a bowl under a still surface [`SHORE_DROP`] below the land
//!   at its rim: Lantern Pond 3 m deep at its center for diving, the
//!   Thinking Pond 2.5 m, the Fern Pond 2 m, and Reed Pond 1.6 m. A muddy
//!   bank [`BANK`] wide slopes from the land down to the water's edge.
//! - Glade Run leaves Reed Pond and runs south as a wading stream 0.45 to
//!   0.85 m deep, its level falling gently to a weir of stones in Walden
//!   Woods, where it drops [`WEIR_DROP`] into a plunge pool [`POOL_DEPTH`]
//!   deep, then runs on lower. Its current is a divergence-free potential
//!   flow along the course from the stream function
//!   ([`physics::water::FlowGrid::river`]): it speeds up where the run
//!   narrows and slows across the pool.
//!
//! Nothing here renders; the zone draws these bodies with the shared water
//! shader and walks its characters through them with
//! [`crate::water`]'s rules.

use std::sync::OnceLock;

use glam::DVec2;
use physics::water::{
    Course, FlowGrid, Kind, Level, Outline, Water, WaterBody, WaterId, WaterSet, medium,
};

/// The ponds: center and water radius, m, in the order of
/// [`POND_NAMES`]. Lantern Pond lies on the commons; Reed Pond in the long
/// meadow by the Knowledge District; the Thinking Pond in Walden Woods; the
/// Fern Pond in Fernhollow.
pub const PONDS: [([f32; 2], f32); 4] = [
    ([-1.0, 29.0], 6.0),
    ([6.0, -42.0], 4.0),
    ([-114.0, -64.0], 5.0),
    ([90.0, 76.0], 4.5),
];
/// Each pond's name.
pub const POND_NAMES: [&str; 4] = [
    "Lantern Pond",
    "Reed Pond",
    "the Thinking Pond",
    "the Fern Pond",
];
/// Each pond's depth at its center below its surface, m (the owner's
/// decision of 2026-10-06).
pub const POND_DEPTHS: [f32; 4] = [3.0, 1.6, 2.5, 2.0];
/// How far a pond's surface lies below the land at its rim, m.
pub const SHORE_DROP: f32 = 0.1;
/// Width of the bank that slopes from the land to the water's edge, m.
pub const BANK: f32 = 1.2;

/// Glade Run: the stream that leaves Reed Pond and runs south through the
/// long meadow, under Brownstone Row's footbridge, into Walden Woods. Its
/// course as points on the ground, m.
pub const STREAM: [[f32; 2]; 8] = [
    [6.0, -45.5],
    [5.0, -56.0],
    [9.0, -66.0],
    [9.0, -78.0],
    [6.0, -90.0],
    [0.0, -102.0],
    [-6.0, -114.0],
    [-9.0, -128.0],
];
/// The stream's name.
pub const STREAM_NAME: &str = "Glade Run";
/// Half the stream's width of water, m.
pub const STREAM_HALF: f32 = 1.1;
/// The shallowest and deepest the stream runs at its middle, m.
pub const STREAM_DEPTH: (f32, f32) = (0.45, 0.85);
/// How far the stream's level falls from Reed Pond to the weir, m.
pub const UPPER_FALL: f32 = 0.08;
/// How far the water drops over the weir, m: the stone weir's lip stands a
/// hand's height over the water below it.
pub const WEIR_DROP: f32 = 0.25;
/// How far the level falls from the weir to the run's end, m.
pub const LOWER_FALL: f32 = 0.07;
/// Half the length of the weir's ramp along the course, m.
const WEIR_RAMP: f32 = 0.3;
/// The plunge pool under the weir: how far downstream its center lies, its
/// radius, and its depth at the center, m.
pub const POOL_BELOW: f32 = 2.0;
pub const POOL_RADIUS: f32 = 2.0;
pub const POOL_DEPTH: f32 = 1.6;
/// The current's mean speed across the stream's first width, m/s.
pub const FLOW_SPEED: f64 = 0.45;
/// The flow grid's spacing, m (the specification's 0.5 m for streams).
pub const FLOW_CELL: f64 = 0.5;
/// Spacing of the resampled course, m.
const SAMPLE: f32 = 1.0;
/// Glade Run's body: after the four ponds.
pub const RUN: WaterId = WaterId(4);
/// Sides of each pond's outline polygon.
const POND_SIDES: usize = 48;

/// Glade Run's course resampled about every meter, with the distance along
/// it to each point.
#[derive(Clone, Debug)]
pub struct Run {
    pub points: Vec<[f32; 2]>,
    pub along: Vec<f32>,
    /// The weir: its distance along the course.
    pub weir: f32,
    /// The plunge pool's center.
    pub pool: [f32; 2],
    /// The box the stream and its banks lie in.
    lo: [f32; 2],
    hi: [f32; 2],
}

/// Where a point lies relative to the stream's course.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    /// Distance along the course, m, clamped to it.
    pub along: f32,
    /// Distance from the centerline, m: across it, or past an end, from the
    /// end point.
    pub across: f32,
    /// Which side of the flow the point lies on: 1 to the left, -1 to the
    /// right.
    pub side: f32,
    /// The direction of flow there, unit.
    pub tangent: [f32; 2],
}

/// The stream's resampled course, built once.
#[must_use]
pub fn run() -> &'static Run {
    static RUN: OnceLock<Run> = OnceLock::new();
    RUN.get_or_init(|| {
        let mut points = vec![STREAM[0]];
        for w in STREAM.windows(2) {
            let (a, b) = (w[0], w[1]);
            let length = (b[0] - a[0]).hypot(b[1] - a[1]);
            let pieces = (length / SAMPLE).ceil().max(1.0) as usize;
            for k in 1..=pieces {
                let t = k as f32 / pieces as f32;
                points.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            }
        }
        let mut along = vec![0.0];
        for w in points.windows(2) {
            along.push(along.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
        }
        // The weir sits halfway between the sixth and seventh points.
        let to = |n: usize| -> f32 {
            STREAM
                .windows(2)
                .take(n)
                .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                .sum()
        };
        let weir = 0.5 * (to(5) + to(6));
        let mut run = Run {
            points,
            along,
            weir,
            pool: [0.0; 2],
            lo: [f32::INFINITY; 2],
            hi: [f32::NEG_INFINITY; 2],
        };
        run.pool = run.point_at(weir + POOL_BELOW);
        let reach = POOL_RADIUS.max(STREAM_HALF) + BANK + 0.5;
        for p in &run.points {
            run.lo = [run.lo[0].min(p[0] - reach), run.lo[1].min(p[1] - reach)];
            run.hi = [run.hi[0].max(p[0] + reach), run.hi[1].max(p[1] + reach)];
        }
        run
    })
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Run {
    /// The course's length, m.
    #[must_use]
    pub fn length(&self) -> f32 {
        *self.along.last().unwrap()
    }

    /// The point on the centerline `along` meters down the course.
    #[must_use]
    pub fn point_at(&self, along: f32) -> [f32; 2] {
        let along = along.clamp(0.0, self.length());
        let i = self
            .along
            .windows(2)
            .position(|w| along <= w[1])
            .unwrap_or(self.points.len() - 2);
        let span = (self.along[i + 1] - self.along[i]).max(1e-6);
        let t = (along - self.along[i]) / span;
        let (a, b) = (self.points[i], self.points[i + 1]);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }

    /// The direction of flow `along` meters down the course, unit.
    #[must_use]
    pub fn tangent_at(&self, along: f32) -> [f32; 2] {
        let a = self.point_at(along - 0.5);
        let b = self.point_at(along + 0.5);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dz).max(1e-6);
        [dx / l, dz / l]
    }

    /// Whether `(x, z)` may lie in the stream or on its banks.
    #[must_use]
    pub fn near(&self, x: f32, z: f32) -> bool {
        x >= self.lo[0] && x <= self.hi[0] && z >= self.lo[1] && z <= self.hi[1]
    }

    /// The station of `(x, z)` on the course.
    #[must_use]
    pub fn locate(&self, x: f32, z: f32) -> Station {
        let mut best = (f32::INFINITY, 0, 0.0);
        for i in 0..self.points.len() - 1 {
            let (a, b) = (self.points[i], self.points[i + 1]);
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let l2 = dx * dx + dz * dz;
            let t = if l2 > 0.0 {
                (((x - a[0]) * dx + (z - a[1]) * dz) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (fx, fz) = (a[0] + dx * t, a[1] + dz * t);
            let d = (x - fx) * (x - fx) + (z - fz) * (z - fz);
            if d < best.0 {
                best = (d, i, t);
            }
        }
        let (d2, i, t) = best;
        let (a, b) = (self.points[i], self.points[i + 1]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dz).max(1e-6);
        let tangent = [dx / l, dz / l];
        let (fx, fz) = (a[0] + dx * t, a[1] + dz * t);
        // Left of the flow: the tangent turned a quarter counterclockwise
        // in (x, z), as `physics::water::Course` measures its offset.
        let offset = (x - fx) * -tangent[1] + (z - fz) * tangent[0];
        Station {
            along: self.along[i] + l * t,
            across: d2.sqrt(),
            side: if offset >= 0.0 { 1.0 } else { -1.0 },
            tangent,
        }
    }

    /// The stream's level `along` meters down the course, m above the
    /// land at Reed Pond: Reed Pond's level, falling gently to the weir,
    /// down its drop, then gently on.
    #[must_use]
    pub fn level_at(&self, along: f32) -> f32 {
        level_profile(self)
            .windows(2)
            .find(|w| along <= w[1][0])
            .map_or_else(
                || level_profile(self).last().unwrap()[1],
                |w| {
                    let span = (w[1][0] - w[0][0]).max(1e-6);
                    let t = ((along - w[0][0]) / span).clamp(0.0, 1.0);
                    w[0][1] + (w[1][1] - w[0][1]) * t
                },
            )
    }

    /// Half the stream's width `along` meters down the course, m: wider
    /// across the plunge pool.
    #[must_use]
    pub fn half_at(&self, along: f32) -> f32 {
        let from = (along - (self.weir + POOL_BELOW)).abs();
        STREAM_HALF + (POOL_RADIUS - STREAM_HALF) * smoothstep(1.0 - from / (POOL_RADIUS + 1.0))
    }

    /// The stream's depth at its middle `along` meters down the course, m:
    /// between [`STREAM_DEPTH`]'s bounds, wandering slowly.
    #[must_use]
    pub fn depth_at(&self, along: f32) -> f32 {
        let wander = 0.5 + 0.5 * (along * 0.21 + 0.7 * (along * 0.077).sin()).sin();
        STREAM_DEPTH.0 + (STREAM_DEPTH.1 - STREAM_DEPTH.0) * wander
    }

    /// The weir's lip: its middle, the direction of flow there, and its
    /// half width, m.
    #[must_use]
    pub fn weir_lip(&self) -> ([f32; 2], [f32; 2], f32) {
        (
            self.point_at(self.weir),
            self.tangent_at(self.weir),
            self.half_at(self.weir),
        )
    }
}

/// The breakpoints of Glade Run's level, `[along, height]`, m.
fn level_profile(run: &Run) -> [[f32; 2]; 4] {
    let start = pond_level(1);
    let upper = start - UPPER_FALL;
    let lower = upper - WEIR_DROP;
    [
        [0.0, start],
        [run.weir - WEIR_RAMP, upper],
        [run.weir + WEIR_RAMP, lower],
        [run.length(), lower - LOWER_FALL],
    ]
}

/// Pond `k`'s level, m: [`SHORE_DROP`] under the land at its center.
#[must_use]
pub fn pond_level(k: usize) -> f32 {
    let ([x, z], _) = PONDS[k];
    super::everglade::land(x, z) - SHORE_DROP
}

/// The ground of pond `k`'s bowl and bank `d` meters from its center, over
/// land at `land`.
fn pond_ground(k: usize, d: f32, land: f32) -> f32 {
    let (_, r) = PONDS[k];
    let level = pond_level(k);
    if d >= r {
        level + (land - level) * smoothstep((d - r) / BANK)
    } else {
        level - POND_DEPTHS[k] * smoothstep(1.0 - d / r)
    }
}

/// The stream's bed and bank at `(x, z)` over land at `land`, if they
/// reach there.
fn run_ground(x: f32, z: f32, land: f32) -> Option<f32> {
    let run = run();
    if !run.near(x, z) {
        return None;
    }
    let s = run.locate(x, z);
    let half = run.half_at(s.along);
    if s.across >= half + BANK {
        return None;
    }
    let level = run.level_at(s.along);
    let mut ground = if s.across >= half {
        level + (land - level) * smoothstep((s.across - half) / BANK)
    } else {
        // A flat bed across the middle, sloping up over the outer 60%.
        let u = s.across / half;
        level - run.depth_at(s.along) * smoothstep((1.0 - u) / 0.6)
    };
    let d = (x - run.pool[0]).hypot(z - run.pool[1]);
    if d < POOL_RADIUS {
        // Flat across its middle meter, then up to the run's bed.
        let u = d / POOL_RADIUS;
        let pool = run.level_at(run.weir + POOL_BELOW) - POOL_DEPTH * smoothstep((1.0 - u) / 0.5);
        ground = ground.min(pool);
    }
    Some(ground)
}

/// The ground at `(x, z)` over the `land` there, with every pond's bowl and
/// the stream's bed carved into it.
#[must_use]
pub fn carve(land: f32, x: f32, z: f32) -> f32 {
    let mut ground = land;
    for (k, ([cx, cz], r)) in PONDS.iter().enumerate() {
        let (dx, dz) = (x - cx, z - cz);
        let reach = r + BANK;
        if dx.abs() < reach && dz.abs() < reach {
            let d = dx.hypot(dz);
            if d < reach {
                ground = ground.min(pond_ground(k, d, land));
            }
        }
    }
    if let Some(bed) = run_ground(x, z, land) {
        ground = ground.min(bed);
    }
    ground
}

/// Whether `(x, z)` lies within a pond's or the stream's carving, banks
/// included.
#[must_use]
pub fn carved(x: f32, z: f32) -> bool {
    PONDS
        .iter()
        .any(|([cx, cz], r)| (x - cx).hypot(z - cz) < r + BANK)
        || (run().near(x, z) && {
            let s = run().locate(x, z);
            s.across < run().half_at(s.along) + BANK
        })
}

/// Distance from `(x, z)` to the stream's course, m.
#[must_use]
pub fn stream_distance(x: f32, z: f32) -> f32 {
    run().locate(x, z).across
}

/// Pond `k` as a body: a polygon round its rim at its level.
#[must_use]
pub fn pond_body(k: usize) -> WaterBody {
    let ([cx, cz], r) = PONDS[k];
    let ring = (0..POND_SIDES)
        .map(|i| {
            let a = i as f64 / POND_SIDES as f64 * std::f64::consts::TAU;
            DVec2::new(f64::from(cx), f64::from(cz)) + DVec2::new(a.cos(), a.sin()) * f64::from(r)
        })
        .collect();
    WaterBody::pond(WaterId(k as u32), ring, f64::from(pond_level(k)))
}

/// Glade Run's course for `physics::water`: the resampled points with the
/// full width at each.
#[must_use]
pub fn course() -> Course {
    let run = run();
    Course::new(
        run.points
            .iter()
            .map(|p| DVec2::new(f64::from(p[0]), f64::from(p[1])))
            .collect(),
        run.along
            .iter()
            .map(|&a| 2.0 * f64::from(run.half_at(a)))
            .collect(),
    )
}

/// Glade Run as a body: its course, its level profile, and its current.
#[must_use]
pub fn run_body() -> WaterBody {
    let run = run();
    let course = course();
    let flow = FlowGrid::river(&course, &[], FLOW_CELL, FLOW_SPEED);
    let points = level_profile(run)
        .iter()
        .map(|[a, h]| [f64::from(*a), f64::from(*h)])
        .collect();
    WaterBody::new(
        RUN,
        Kind::River,
        Outline::River { course },
        Level::Profile { points },
    )
    .with_flow(flow)
}

/// Every body of Everglade's water: the four ponds, then Glade Run. Where
/// the run starts inside Reed Pond, the pond's surface wins.
#[must_use]
pub fn water() -> &'static WaterSet {
    static WATER: OnceLock<WaterSet> = OnceLock::new();
    WATER.get_or_init(|| {
        let mut bodies: Vec<WaterBody> = (0..PONDS.len()).map(pond_body).collect();
        bodies.push(run_body());
        WaterSet::new(bodies, 4.0)
    })
}

/// The beds and banks of the water as static boxes for a physics world,
/// each a center and half extents: one `cell` meters a side for every cell
/// within `margin` of the water, its top on the carved ground. Floating
/// debris and boats lodge against the banks and sink onto the beds.
#[must_use]
pub fn bed_boxes(cell: f32, margin: f32) -> Vec<(glam::DVec3, glam::DVec3)> {
    let mut cells = std::collections::BTreeSet::new();
    let mut cover = |lo: [f32; 2], hi: [f32; 2]| {
        let i0 = ((lo[0] - margin) / cell).floor() as i32;
        let i1 = ((hi[0] + margin) / cell).ceil() as i32;
        let j0 = ((lo[1] - margin) / cell).floor() as i32;
        let j1 = ((hi[1] + margin) / cell).ceil() as i32;
        for j in j0..j1 {
            for i in i0..i1 {
                cells.insert((i, j));
            }
        }
    };
    for ([x, z], r) in PONDS {
        cover([x - r, z - r], [x + r, z + r]);
    }
    let run = run();
    for w in run.points.windows(2) {
        let half = STREAM_HALF + POOL_RADIUS;
        cover(
            [w[0][0].min(w[1][0]) - half, w[0][1].min(w[1][1]) - half],
            [w[0][0].max(w[1][0]) + half, w[0][1].max(w[1][1]) + half],
        );
    }
    let near = |x: f32, z: f32| {
        let reach = margin + cell * 0.71;
        (0..8).any(|k| {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            surface(x + a.cos() * reach, z + a.sin() * reach).is_some()
        }) || surface(x, z).is_some()
    };
    let h = f64::from(cell) * 0.5;
    cells
        .into_iter()
        .filter_map(|(i, j)| {
            let (x, z) = ((i as f32 + 0.5) * cell, (j as f32 + 0.5) * cell);
            if !near(x, z) {
                return None;
            }
            let top = f64::from(super::everglade::height(x, z));
            Some((
                glam::DVec3::new(f64::from(x), top - 0.5, f64::from(z)),
                glam::DVec3::new(h, 0.5, h),
            ))
        })
        .collect()
}

/// The gameplay surface's height over `(x, z)`, m, if there is water. The
/// ponds and the run carry no waves, so the tick does not matter.
#[must_use]
pub fn surface(x: f32, z: f32) -> Option<f32> {
    water()
        .sample(f64::from(x), f64::from(z), 0)
        .map(|s| s.height as f32)
}

/// The current at `(x, z)`, m/s in x and z.
#[must_use]
pub fn current(x: f32, z: f32) -> [f32; 2] {
    water()
        .sample(f64::from(x), f64::from(z), 0)
        .map_or([0.0; 2], |s| [s.flow.x as f32, s.flow.z as f32])
}

/// Where a character's feet rest at `(x, z)` with nobody steering it, as
/// the town's walkers and creatures place theirs: on the bed in wading
/// water, at the float line where the water is too deep to stand.
#[must_use]
pub fn stand(x: f32, z: f32) -> f32 {
    let bed = super::everglade::height(x, z);
    match surface(x, z) {
        Some(top) if top - bed > medium::SWIM_DEPTH as f32 => top - medium::FLOAT_DEPTH as f32,
        _ => bed,
    }
}

/// The nearest dry bank to `(x, z)`, where a defeated swimmer comes out: a
/// little past the bank's top, square to the shore.
#[must_use]
pub fn nearest_bank(x: f32, z: f32) -> [f32; 2] {
    let past = BANK + 0.8;
    let mut best: Option<(f32, [f32; 2])> = None;
    let mut consider = |d: f32, p: [f32; 2]| {
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, p));
        }
    };
    for ([cx, cz], r) in PONDS {
        let (dx, dz) = (x - cx, z - cz);
        let d = dx.hypot(dz);
        let (ux, uz) = if d > 1e-3 {
            (dx / d, dz / d)
        } else {
            (1.0, 0.0)
        };
        consider((r - d).abs(), [cx + ux * (r + past), cz + uz * (r + past)]);
    }
    let run = run();
    let s = run.locate(x, z);
    let half = run.half_at(s.along);
    let c = run.point_at(s.along);
    let normal = [-s.tangent[1] * s.side, s.tangent[0] * s.side];
    consider(
        (half - s.across).abs(),
        [
            c[0] + normal[0] * (half + past),
            c[1] + normal[1] * (half + past),
        ],
    );
    best.map_or([x, z], |(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::super::everglade::{height, land};
    use super::*;

    #[test]
    fn each_pond_is_a_bowl_to_its_depth_under_a_level_surface() {
        for (k, &([x, z], r)) in PONDS.iter().enumerate() {
            let level = pond_level(k);
            assert_eq!(surface(x, z), Some(level));
            // The bed at the center lies the pond's depth down.
            assert!((level - height(x, z) - POND_DEPTHS[k]).abs() < 1e-4);
            // The water's edge meets the ground, and past the bank the
            // land is uncarved.
            assert!((height(x + r, z) - level).abs() < 1e-3, "{k}");
            assert_eq!(height(x + r + BANK + 0.01, z), land(x + r + BANK + 0.01, z));
            // Walking in from the bank the bed only falls, so a walker
            // wades before it swims.
            let mut last = f32::INFINITY;
            for i in 0..=100 {
                let d = r + BANK - (r + BANK) * i as f32 / 100.0;
                let bed = height(x + d, z);
                assert!(bed <= last + 1e-5, "{k} at {d}");
                last = bed;
            }
        }
    }

    #[test]
    fn glade_run_wades_its_length_and_drops_into_the_pool_at_the_weir() {
        let run = run();
        for i in 0..(run.length() as usize) {
            let along = i as f32 + 0.5;
            let [x, z] = run.point_at(along);
            let pool = (x - run.pool[0]).hypot(z - run.pool[1]) < POOL_RADIUS;
            let in_pond = PONDS
                .iter()
                .any(|([cx, cz], r)| (x - cx).hypot(z - cz) < r + BANK);
            if in_pond {
                continue;
            }
            let Some(top) = surface(x, z) else {
                panic!("no water in Glade Run at {along}");
            };
            let depth = top - height(x, z);
            if !pool && (top - run.level_at(along)).abs() < 1e-3 {
                assert!(
                    (STREAM_DEPTH.0 - 0.01..=STREAM_DEPTH.1 + 0.01).contains(&depth),
                    "{depth} at {along}"
                );
            }
        }
        let [px, pz] = run.pool;
        let top = surface(px, pz).unwrap();
        assert!((top - height(px, pz) - POOL_DEPTH).abs() < 0.01);
        let above = run.level_at(run.weir - 1.0);
        let below = run.level_at(run.weir + 1.0);
        assert!((above - below - WEIR_DROP).abs() < 0.02, "{above} {below}");
        // The current runs downstream along the course.
        let [ux, uz] = run.tangent_at(40.0);
        let [x, z] = run.point_at(40.0);
        let [fx, fz] = current(x, z);
        assert!(fx * ux + fz * uz > 0.2, "{fx} {fz}");
    }

    #[test]
    fn a_defeated_swimmer_comes_out_on_dry_land_and_walkers_float_in_deep_water() {
        let ([x, z], _) = PONDS[0];
        let [bx, bz] = nearest_bank(x + 1.0, z);
        assert!(surface(bx, bz).is_none());
        assert_eq!(height(bx, bz), land(bx, bz));
        let top = surface(x, z).unwrap();
        assert!((stand(x, z) - (top - medium::FLOAT_DEPTH as f32)).abs() < 1e-5);
        let [rx, rz] = run().point_at(20.0);
        assert_eq!(stand(rx, rz), height(rx, rz));
        let [sx, sz] = nearest_bank(rx, rz);
        assert!(surface(sx, sz).is_none(), "{sx} {sz}");
    }
}
