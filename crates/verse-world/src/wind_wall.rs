//! Wind Wall (SRD 5.2.1) as an updraft wall.
//!
//! Level 3 Evocation. Casting time: Action. Range: 120 feet. Components:
//! V, S, M (a fan and a feather). Duration: Concentration, up to 1 minute.
//! A wall of strong wind up to 50 feet long, 15 feet high, and 1 foot
//! thick rises along one continuous path on the ground. Each creature in
//! its area when it appears makes a Strength saving throw, taking 4d8
//! Bludgeoning damage on a failure and half on a success. Loose,
//! lightweight materials brought into the wall fly upward. Small or smaller
//! flying creatures or objects can't pass through. Arrows, bolts, and other
//! ordinary projectiles launched at targets behind the wall are deflected
//! upward and miss automatically; boulders hurled by giants or siege
//! engines are unaffected. The wall keeps fog, smoke, and other gases away.
//!
//! The wall is a ground polyline extruded into a vertical slab. Its volume
//! is the set of points whose horizontal distance to the polyline is at
//! most half the thickness, between the ground and the top, so it is
//! convex along each segment and the swept-segment entry of a projectile is
//! exact at any speed. Inside the volume an updraft lifts lightweight
//! bodies in a seeded spiral and gives heavier ones a modest lift that
//! cannot raise them. A static box per segment, in its own filter bit,
//! stops only airborne Small-or-smaller bodies that arrive from outside;
//! grounded bodies and creatures pass.

use std::collections::BTreeSet;

use glam::{DQuat, DVec2, DVec3};
use physics::{Body, BodyId, BodyKind, Collider, Filter, Ledger, Material, Shape, World};
use serde::{Deserialize, Serialize};

/// One foot, m.
pub const FOOT: f64 = 0.3048;
/// Spell level.
pub const LEVEL: u8 = 3;
/// Spell range: 120 feet.
pub const RANGE: f64 = 120.0 * FOOT;
/// Longest wall path: 50 feet.
pub const MAX_LENGTH: f64 = 50.0 * FOOT;
/// Wall height: 15 feet.
pub const HEIGHT: f64 = 15.0 * FOOT;
/// Wall thickness: 1 foot.
pub const THICKNESS: f64 = FOOT;
/// Concentration, up to 1 minute, s.
pub const DURATION: f64 = 60.0;
/// The player wizard's spell save DC.
pub const SAVE_DC: i32 = 15;
/// Appearance damage: 4d8 Bludgeoning (the PDF; the Ruins markdown says 3d8).
pub const DAMAGE_DICE: usize = 4;
/// Sides of each damage die.
pub const DAMAGE_SIDES: i32 = 8;
/// Heaviest body the wall treats as loose, lightweight material, kg.
pub const LIGHTWEIGHT_MASS: f64 = 2.0;
/// Updraft on lightweight bodies inside the wall, m/s^2 upward.
pub const UPDRAFT: f64 = 25.0;
/// Updraft on heavier bodies inside the wall, m/s^2 upward. Well below
/// gravity, so it lightens them without lifting them.
pub const HEAVY_UPDRAFT: f64 = 3.0;
/// Amplitude of the rotating lateral gust on lightweight bodies, m/s^2.
pub const TURBULENCE: f64 = 6.0;
/// Slowest and fastest spiral rate of that gust, Hz. Each body draws its
/// rate and phase from the seeded dice.
pub const SPIRAL_HZ: [f64; 2] = [1.0, 2.0];
/// Natural frequency of the critically damped pull that holds lightweight
/// material in the wall's mid-plane, Hz, so material brought into a 1-foot
/// wall stays in it and rises instead of crossing it.
pub const CAPTURE_HZ: f64 = 2.0;
/// Least elevation of a deflected ordinary projectile, rad (60 degrees).
pub const DEFLECT_ELEVATION: f64 = std::f64::consts::FRAC_PI_3;
/// Upward speed a deflected ordinary projectile gains, m/s.
pub const DEFLECT_BOOST: f64 = 10.0;
/// Restitution of the wall against the airborne bodies it stops.
pub const RESTITUTION: f64 = 0.6;
/// Filter bit that the wall and the bodies it stops share. A body collides
/// with the wall only while its filter carries this bit in both its group
/// and its mask.
pub const FILTER_BIT: u32 = 1 << 20;
/// Furthest the ground may rise or fall from the anchor's along the path
/// before the wall counts as not grounded, m.
pub const GROUND_TOLERANCE: f64 = 0.3;
/// Spacing of the ground samples along the path, m.
pub const GROUND_SAMPLE: f64 = 0.25;
/// Points along an authored arc.
pub const ARC_SEGMENTS: usize = 12;
/// Ledger term for every impulse the updraft puts into a body.
pub const LEDGER_TERM: &str = "wind_wall";
/// The overlay's SRD line.
pub const SRD_LINE: &str = "Wind Wall - level 3 Evocation - range 120 ft - \
    wall 50 x 15 x 1 ft - Concentration, up to 1 minute - Strength save, 4d8 or half";

/// SRD size categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Size {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gargantuan,
}

/// How the wall treats a projectile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    /// An arrow, bolt, or other ordinary projectile: deflected.
    Ordinary,
    /// A boulder from a giant or a siege engine: unaffected.
    Siege,
    /// A spell's projectile: unaffected.
    Spell,
}

/// Why a wall shape was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// Fewer than two points, a repeated point, or a non-finite point.
    Malformed,
    /// The path is longer than 50 feet.
    TooLong,
    /// The anchor is beyond 120 feet of the caster.
    OutOfRange,
    /// Part of the path has no ground under it, or ground that departs from
    /// the anchor's height.
    NotGrounded,
}

/// A validated wall: a continuous ground path, its base height, and its
/// extrusion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    /// Path vertices on the ground plane, (x, z), m.
    pub path: Vec<DVec2>,
    /// Height of the ground at the anchor, m. The wall rises from here.
    pub base: f64,
}

impl Wall {
    /// Validate a path: two or more distinct finite points, at most 50 feet
    /// long, anchored within range of `caster`, and on ground everywhere.
    /// `ground` returns the floor height under a point, or `None` over a
    /// chasm or outside the map.
    ///
    /// # Errors
    ///
    /// Returns the first rule the path breaks.
    pub fn new(
        path: Vec<DVec2>,
        caster: DVec3,
        ground: impl Fn(DVec2) -> Option<f64>,
    ) -> Result<Self, Refusal> {
        if path.len() < 2
            || path.iter().any(|p| !p.is_finite())
            || path.windows(2).any(|w| w[0].distance(w[1]) < 1e-6)
        {
            return Err(Refusal::Malformed);
        }
        let length: f64 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
        if length > MAX_LENGTH + 1e-9 {
            return Err(Refusal::TooLong);
        }
        if path[0].distance(DVec2::new(caster.x, caster.z)) > RANGE {
            return Err(Refusal::OutOfRange);
        }
        let base = ground(path[0]).ok_or(Refusal::NotGrounded)?;
        for w in path.windows(2) {
            let samples = (w[0].distance(w[1]) / GROUND_SAMPLE).ceil().max(1.0) as usize;
            for i in 0..=samples {
                let p = w[0].lerp(w[1], i as f64 / samples as f64);
                match ground(p) {
                    Some(y) if (y - base).abs() <= GROUND_TOLERANCE => {}
                    _ => return Err(Refusal::NotGrounded),
                }
            }
        }
        Ok(Self { path, base })
    }

    /// A straight path of `length` centered on `center` along `direction`.
    #[must_use]
    pub fn straight(center: DVec2, direction: DVec2, length: f64) -> Vec<DVec2> {
        let half = direction.normalize_or_zero() * (length / 2.0);
        vec![center - half, center + half]
    }

    /// An arc of `radius` about `center` from angle `from` through `sweep`,
    /// rad, measured from +x toward +z.
    #[must_use]
    pub fn arc(center: DVec2, radius: f64, from: f64, sweep: f64) -> Vec<DVec2> {
        (0..=ARC_SEGMENTS)
            .map(|i| {
                let a = from + sweep * i as f64 / ARC_SEGMENTS as f64;
                center + DVec2::new(a.cos(), a.sin()) * radius
            })
            .collect()
    }

    /// An L: `first` along `a` from `corner`'s far end into `corner`, then
    /// `second` along `b` out of it.
    #[must_use]
    pub fn l_shape(corner: DVec2, a: DVec2, first: f64, b: DVec2, second: f64) -> Vec<DVec2> {
        vec![
            corner - a.normalize_or_zero() * first,
            corner,
            corner + b.normalize_or_zero() * second,
        ]
    }

    /// Total path length, m.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    /// Height of the top of the wall, m.
    #[must_use]
    pub fn top(&self) -> f64 {
        self.base + HEIGHT
    }

    /// The nearest point of the path to `p` on the ground plane, with the
    /// path's unit tangent there.
    #[must_use]
    pub fn nearest(&self, p: DVec2) -> (DVec2, DVec2) {
        let mut best = (f64::INFINITY, self.path[0], DVec2::X);
        for w in self.path.windows(2) {
            let d = w[1] - w[0];
            let t = ((p - w[0]).dot(d) / d.length_squared()).clamp(0.0, 1.0);
            let q = w[0] + d * t;
            let distance = q.distance_squared(p);
            if distance < best.0 {
                best = (distance, q, d.normalize());
            }
        }
        (best.1, best.2)
    }

    /// Horizontal distance from `p` to the path, m.
    #[must_use]
    pub fn distance(&self, p: DVec3) -> f64 {
        let flat = DVec2::new(p.x, p.z);
        self.nearest(flat).0.distance(flat)
    }

    /// Whether `p` lies in the wall's volume grown by `margin`.
    #[must_use]
    pub fn contains(&self, p: DVec3, margin: f64) -> bool {
        p.y >= self.base - margin
            && p.y <= self.top() + margin
            && self.distance(p) <= THICKNESS / 2.0 + margin
    }

    /// Fraction of the segment from `a` to `b` at which it first enters the
    /// volume grown by `margin`, or `None` when it stays outside. A segment
    /// that starts inside enters at zero.
    #[must_use]
    pub fn entry(&self, a: DVec3, b: DVec3, margin: f64) -> Option<f64> {
        let r = THICKNESS / 2.0 + margin;
        let (lo, hi) = interval(a.y, b.y - a.y, self.base - margin, self.top() + margin)?;
        let (from, along) = (DVec2::new(a.x, a.z), DVec2::new(b.x - a.x, b.z - a.z));
        let mut first: Option<f64> = None;
        for w in self.path.windows(2) {
            let Some((s0, s1)) = capsule_interval(from, along, w[0], w[1], r) else {
                continue;
            };
            let (t0, t1) = (s0.max(lo).max(0.0), s1.min(hi).min(1.0));
            if t0 <= t1 && first.is_none_or(|f| t0 < f) {
                first = Some(t0);
            }
        }
        first
    }

    /// Whether a gas cloud of `radius` about `center` overlaps the wall, so
    /// the wall removes it.
    #[must_use]
    pub fn clears_gas(&self, center: DVec3, radius: f64) -> bool {
        self.contains(center, radius)
    }

    /// Whether a creature standing at `feet`, a capsule of `radius` and
    /// `height`, is in the wall's area when it appears.
    #[must_use]
    pub fn in_area(&self, feet: DVec3, radius: f64, height: f64) -> bool {
        feet.y <= self.top()
            && feet.y + height >= self.base
            && self.distance(feet) <= THICKNESS / 2.0 + radius
    }

    /// Whether a creature of `size` that is `flying` cannot pass through.
    #[must_use]
    pub fn blocks_creature(size: Size, flying: bool) -> bool {
        flying && size <= Size::Small
    }

    /// Ground footprints of the wall for navigation: for each segment, its
    /// center, half extents along and across it, and its unit direction.
    #[must_use]
    pub fn footprints(&self) -> Vec<(DVec2, DVec2, DVec2)> {
        self.path
            .windows(2)
            .map(|w| {
                let d = w[1] - w[0];
                (
                    (w[0] + w[1]) / 2.0,
                    DVec2::new(d.length() / 2.0 + THICKNESS / 2.0, THICKNESS / 2.0),
                    d.normalize(),
                )
            })
            .collect()
    }
}

/// Line parameters where `x0 + t dx` lies in `[lo, hi]`.
fn interval(x0: f64, dx: f64, lo: f64, hi: f64) -> Option<(f64, f64)> {
    if dx.abs() < 1e-12 {
        return (lo..=hi)
            .contains(&x0)
            .then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let (a, b) = ((lo - x0) / dx, (hi - x0) / dx);
    Some((a.min(b), a.max(b)))
}

/// Line parameters where `p + t d` lies within `r` of the segment from `a`
/// to `b`. The region is convex, so the union of its rectangle and end
/// discs is one interval.
fn capsule_interval(p: DVec2, d: DVec2, a: DVec2, b: DVec2, r: f64) -> Option<(f64, f64)> {
    let u = (b - a).normalize();
    let n = u.perp();
    let length = a.distance(b);
    let mut pieces = Vec::with_capacity(3);
    if let (Some(along), Some(across)) = (
        interval((p - a).dot(u), d.dot(u), 0.0, length),
        interval((p - a).dot(n), d.dot(n), -r, r),
    ) {
        let (lo, hi) = (along.0.max(across.0), along.1.min(across.1));
        if lo <= hi {
            pieces.push((lo, hi));
        }
    }
    for c in [a, b] {
        let f = p - c;
        let (qa, qb, qc) = (d.length_squared(), f.dot(d), f.length_squared() - r * r);
        if qa < 1e-24 {
            if qc <= 0.0 {
                pieces.push((f64::NEG_INFINITY, f64::INFINITY));
            }
            continue;
        }
        let disc = qb * qb - qa * qc;
        if disc >= 0.0 {
            let root = disc.sqrt();
            pieces.push(((-qb - root) / qa, (-qb + root) / qa));
        }
    }
    let lo = pieces.iter().map(|p| p.0).reduce(f64::min)?;
    let hi = pieces.iter().map(|p| p.1).reduce(f64::max)?;
    Some((lo, hi))
}

/// The velocity of an ordinary projectile after the wall deflects it: its
/// direction rises to at least 60 degrees above horizontal at the same
/// speed, then it gains 10 m/s upward.
#[must_use]
pub fn deflect(vel: DVec3) -> DVec3 {
    let flat = DVec2::new(vel.x, vel.z);
    let speed = vel.length();
    let elevation = vel.y.atan2(flat.length()).max(DEFLECT_ELEVATION);
    let heading = flat.normalize_or_zero();
    let horizontal = speed * elevation.cos();
    DVec3::new(
        heading.x * horizontal,
        speed * elevation.sin() + DEFLECT_BOOST,
        heading.y * horizontal,
    )
}

/// A projectile in flight, as far as the wall is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    pub pos: DVec3,
    pub vel: DVec3,
    pub tag: Tag,
    /// Deflected by a wind wall: it can no longer damage its original
    /// target.
    pub deflected: bool,
}

impl Flight {
    #[must_use]
    pub fn new(pos: DVec3, vel: DVec3, tag: Tag) -> Self {
        Self {
            pos,
            vel,
            tag,
            deflected: false,
        }
    }

    /// Whether the flight may still damage the target it was launched at.
    #[must_use]
    pub fn can_damage_target(&self) -> bool {
        !self.deflected
    }

    /// Advance one step of `dt` under `gravity` (velocity first, then
    /// position, matching the world's semi-implicit Euler). An ordinary
    /// flight whose swept segment enters `wall` stops at the entry point,
    /// is deflected, and spends the rest of the step on its new velocity.
    /// Returns the entry point when this step deflected it.
    pub fn advance(&mut self, dt: f64, gravity: DVec3, wall: Option<&Wall>) -> Option<DVec3> {
        self.vel += gravity * dt;
        let end = self.pos + self.vel * dt;
        let entry = wall
            .filter(|_| self.tag == Tag::Ordinary && !self.deflected)
            .and_then(|w| w.entry(self.pos, end, 0.0));
        match entry {
            Some(t) => {
                let point = self.pos.lerp(end, t);
                self.vel = deflect(self.vel);
                self.deflected = true;
                self.pos = point + self.vel * dt * (1.0 - t);
                Some(point)
            }
            None => {
                self.pos = end;
                None
            }
        }
    }
}

/// Appearance damage for one creature: 4d8 Bludgeoning, or half (rounded
/// down) on a successful Strength save. `d8` rolls one die.
pub fn appearance_damage(saved: bool, mut d8: impl FnMut() -> i32) -> (Vec<i32>, i32) {
    let rolls: Vec<i32> = (0..DAMAGE_DICE).map(|_| d8()).collect();
    let total: i32 = rolls.iter().sum();
    (rolls, if saved { total / 2 } else { total })
}

/// A dynamic body the wall acts on, with what the wall needs to know.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    pub body: BodyId,
    pub size: Size,
    /// Horizontal half extent, m: the body counts as inside the wall once
    /// it overlaps the slab by this much.
    pub radius: f64,
}

/// The active spell: the wall, its static colliders, and the bodies that
/// entered it. Everything serializes, so a checkpoint replays exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindWall {
    pub wall: Wall,
    /// Static bodies that stop airborne Small-or-smaller props.
    pub bodies: Vec<BodyId>,
    /// Concentration ends at this time, s.
    pub until: f64,
    /// Seed of the updraft's spiral rates and phases.
    pub seed: u64,
    /// Bodies inside the wall now. A body inside passes; only one that
    /// arrives from outside while airborne is stopped.
    pub inside: BTreeSet<BodyId>,
    /// Whether concentration has ended.
    pub ended: bool,
}

impl WindWall {
    /// Raise the wall in `world` at time `now`.
    pub fn raise(world: &mut World, wall: Wall, now: f64, seed: u64) -> Self {
        let mut bodies = Vec::new();
        let center_y = wall.base + HEIGHT / 2.0;
        let filter = Filter {
            group: FILTER_BIT,
            mask: FILTER_BIT,
        };
        let material = Material {
            restitution: RESTITUTION,
            ..Material::default()
        };
        let mut add = |world: &mut World, at: DVec2, rotation: DQuat, shape: Shape| {
            let body = world.add(
                Body::new(1.0, DVec3::ONE, DVec3::new(at.x, center_y, at.y))
                    .with_kind(BodyKind::Static),
            );
            let mut collider = Collider::new(body, shape)
                .with_filter(filter)
                .with_material(material);
            collider.rotation = rotation;
            world.add_collider(collider);
            bodies.push(body);
        };
        for w in wall.path.windows(2) {
            let d = w[1] - w[0];
            add(
                world,
                (w[0] + w[1]) / 2.0,
                DQuat::from_rotation_y(-d.y.atan2(d.x)),
                Shape::Cuboid {
                    half: DVec3::new(d.length() / 2.0, HEIGHT / 2.0, THICKNESS / 2.0),
                },
            );
        }
        // Round the joins so a bent wall has no gap on its outer side.
        for &corner in &wall.path[1..wall.path.len() - 1] {
            add(
                world,
                corner,
                DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2),
                Shape::Capsule {
                    radius: THICKNESS / 2.0,
                    half_length: HEIGHT / 2.0 - THICKNESS / 2.0,
                },
            );
        }
        Self {
            wall,
            bodies,
            until: now + DURATION,
            seed,
            inside: BTreeSet::new(),
            ended: false,
        }
    }

    /// Whether the wall still stands at `now`.
    #[must_use]
    pub fn active(&self, now: f64) -> bool {
        !self.ended && now < self.until
    }

    /// End concentration: the wall's bodies leave the world and its field
    /// stops.
    pub fn end(&mut self, world: &mut World, props: &[Prop]) {
        if self.ended {
            return;
        }
        for &body in &self.bodies {
            world.remove_body(body);
        }
        self.ended = true;
        self.inside.clear();
        for prop in props {
            set_blocked(world, prop.body, false);
        }
    }

    /// Run before each world step: end the wall when its time is up,
    /// decide which props the wall stops, and apply the updraft. Records
    /// every updraft impulse in `ledger`.
    pub fn before_step(&mut self, world: &mut World, props: &[Prop], ledger: &mut Ledger) {
        if world.time() >= self.until {
            self.end(world, props);
        }
        if self.ended {
            return;
        }
        let grounded = supported(world, &self.bodies);
        let dt = world.dt;
        let time = world.time();
        for prop in props {
            let body = world[prop.body];
            if body.kind != BodyKind::Dynamic || body.removed {
                continue;
            }
            let airborne = !body.sleeping && !grounded.contains(&prop.body);
            let stopped = airborne && prop.size <= Size::Small && body.mass > LIGHTWEIGHT_MASS;
            // A body admitted into the wall passes until it leaves; one the
            // wall stops stays stopped while it touches it.
            let blocked = if !self.wall.contains(body.pos, prop.radius) {
                self.inside.remove(&prop.body);
                stopped
            } else if self.inside.contains(&prop.body) || !stopped {
                self.inside.insert(prop.body);
                false
            } else {
                true
            };
            set_blocked(world, prop.body, blocked);
            if !self.inside.contains(&prop.body) || body.pos.y > self.wall.top() {
                continue;
            }
            let accel = self.updraft(&body, prop.body, time);
            let force = accel * body.mass;
            world[prop.body].apply_force(force);
            ledger.add_impulse(LEDGER_TERM, force * dt, body.pos);
        }
    }

    /// Acceleration the wind gives `body` inside the wall at `time`, m/s^2.
    #[must_use]
    pub fn updraft(&self, body: &Body, id: BodyId, time: f64) -> DVec3 {
        if body.mass > LIGHTWEIGHT_MASS {
            return DVec3::Y * HEAVY_UPDRAFT;
        }
        let flat = DVec2::new(body.pos.x, body.pos.z);
        let (nearest, tangent) = self.wall.nearest(flat);
        let along = DVec3::new(tangent.x, 0.0, tangent.y);
        let across = DVec3::new(-tangent.y, 0.0, tangent.x);
        let draw = mix(self.seed ^ mix(u64::from(id.0)));
        let unit = |bits: u64| (bits >> 11) as f64 / (1u64 << 53) as f64;
        let hz = SPIRAL_HZ[0] + (SPIRAL_HZ[1] - SPIRAL_HZ[0]) * unit(draw);
        let phase = std::f64::consts::TAU * unit(mix(draw));
        let angle = std::f64::consts::TAU * hz * time + phase;
        let omega = std::f64::consts::TAU * CAPTURE_HZ;
        let offset = (flat - nearest).dot(tangent.perp());
        DVec3::Y * UPDRAFT + (along * angle.cos() + across * angle.sin()) * TURBULENCE
            - across * (omega * omega * offset + 2.0 * omega * body.vel.dot(across))
    }
}

/// Add or remove the wall's filter bit on every collider of `body`.
pub fn set_blocked(world: &mut World, body: BodyId, blocked: bool) {
    let ids: Vec<_> = world
        .colliders()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.body == body)
        .map(|(i, _)| physics::ColliderId(i as u32))
        .collect();
    for id in ids {
        let collider = world.collider_mut(id);
        if collider.filter == Filter::NONE {
            continue;
        }
        if blocked {
            collider.filter.group |= FILTER_BIT;
            collider.filter.mask |= FILTER_BIT;
        } else {
            collider.filter.group &= !FILTER_BIT;
            collider.filter.mask &= !FILTER_BIT;
        }
    }
}

/// Bodies the last step's contacts held up from below, not counting the
/// wall's own bodies.
fn supported(world: &World, wall: &[BodyId]) -> BTreeSet<BodyId> {
    let mut out = BTreeSet::new();
    for c in &world.contacts {
        if wall.contains(&c.body_a) || wall.contains(&c.body_b) {
            continue;
        }
        // The normal points from a to b.
        if c.normal.y > 0.5 {
            out.insert(c.body_b);
        } else if c.normal.y < -0.5 {
            out.insert(c.body_a);
        }
    }
    out
}

/// SplitMix64's finalizer: a seeded draw with no state to checkpoint.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests;
