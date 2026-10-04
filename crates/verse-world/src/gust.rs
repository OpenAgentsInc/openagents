//! Gust of Wind (SRD 5.2.1) as a steerable drag field.
//!
//! Level 2 Evocation, casting time Action, range Self, concentration up to
//! one minute. A Line of strong wind 60 feet long and 10 feet wide blasts
//! from the caster. Each creature in the Line makes a Strength save or is
//! pushed 15 feet along it, again whenever it ends a turn there; moving
//! closer to the caster costs double movement; unprotected flames go out
//! and protected ones have a 50 percent chance to. A Bonus Action re-aims
//! the Line.
//!
//! Creatures follow the rule exactly: saves, a calibrated push, and halved
//! approach speed. Objects instead feel a quadratic drag field toward the
//! wind velocity, so what moves depends on mass, frontal area, and
//! friction. Ordinary projectiles feel the same drag; the SRD does not say
//! so, and [`DRAG_ORDINARY_PROJECTILES`] switches it off.
//!
//! This module holds the rule and field math against [`physics::World`]
//! and owns no actor, light, or projectile state of its own.
use glam::{DQuat, DVec3};
use physics::{Body, BodyId, Collider, Material, Shape, World};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One foot, m.
pub const FEET: f64 = 0.3048;
/// Length of the Line, m (60 ft).
pub const LENGTH: f64 = 60.0 * FEET;
/// Width of the Line, m (10 ft).
pub const WIDTH: f64 = 10.0 * FEET;
/// Height of the Line's volume, m (10 ft, matching its width).
pub const HEIGHT: f64 = 10.0 * FEET;
/// Wind speed along the Line, m/s.
pub const WIND_SPEED: f64 = 20.0;
/// Sea-level air density, kg/m³.
pub const AIR_DENSITY: f64 = 1.225;
/// Distance a failed save pushes a creature, m (15 ft).
pub const PUSH_DISTANCE: f64 = 15.0 * FEET;
/// One round, s. A creature that stays in the Line saves again this often.
pub const TURN: f64 = 6.0;
/// Bonus Action re-aim cooldown, s.
pub const REAIM_COOLDOWN: f64 = 6.0;
/// Concentration, up to one minute, s.
pub const DURATION: f64 = 60.0;
/// Player wizard spell save DC.
pub const SPELL_SAVE_DC: i32 = 15;
/// Feet of movement spent for each foot moved closer to the caster.
pub const APPROACH_COST: f64 = 2.0;
/// Chance in percent that a protected flame goes out.
pub const PROTECTED_FLAME_PERCENT: u32 = 50;
/// Ordinary flights (arrows and bolts) crossing the Line feel its drag.
/// A physical consequence the SRD does not state; on by default.
pub const DRAG_ORDINARY_PROJECTILES: bool = true;
/// Drag coefficient times frontal area of an arrow or bolt, fletching
/// included, m². An arrow weathervanes into the relative wind, so it always
/// presents this face.
pub const ARROW_DRAG_AREA: f64 = 1.5e-4;
/// Arrow mass, kg.
pub const ARROW_MASS: f64 = 0.025;

/// The Line's volume: a box from the caster's feet along a horizontal
/// direction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// The caster's feet, m.
    pub origin: DVec3,
    /// Unit horizontal direction the wind blows.
    pub direction: DVec3,
}

impl Line {
    /// The Line from `origin` along the horizontal part of `aim`, or `None`
    /// when `aim` has no usable horizontal part.
    #[must_use]
    pub fn new(origin: DVec3, aim: DVec3) -> Option<Self> {
        let flat = DVec3::new(aim.x, 0.0, aim.z);
        (origin.is_finite() && flat.is_finite() && flat.length() > 1e-3).then(|| Self {
            origin,
            direction: flat.normalize(),
        })
    }

    /// Unit horizontal direction to the right of the wind.
    #[must_use]
    pub fn side(&self) -> DVec3 {
        DVec3::new(-self.direction.z, 0.0, self.direction.x)
    }

    /// `point` as distance along the Line, to its side, and above the
    /// caster's feet, m.
    #[must_use]
    pub fn local(&self, point: DVec3) -> DVec3 {
        let d = point - self.origin;
        DVec3::new(d.dot(self.direction), d.dot(self.side()), d.y)
    }

    #[must_use]
    pub fn contains(&self, point: DVec3) -> bool {
        let l = self.local(point);
        (0.0..=LENGTH).contains(&l.x) && l.y.abs() <= WIDTH / 2.0 && (0.0..=HEIGHT).contains(&l.z)
    }

    /// Whether a sphere touches the volume, for gas and vapor clouds.
    #[must_use]
    pub fn touches_sphere(&self, center: DVec3, radius: f64) -> bool {
        let l = self.local(center);
        let nearest = DVec3::new(
            l.x.clamp(0.0, LENGTH),
            l.y.clamp(-WIDTH / 2.0, WIDTH / 2.0),
            l.z.clamp(0.0, HEIGHT),
        );
        nearest.distance(l) <= radius
    }

    /// Wind velocity inside the Line, m/s.
    #[must_use]
    pub fn wind(&self) -> DVec3 {
        self.direction * WIND_SPEED
    }

    /// The eight corners of the volume, bottom four first, for drawing.
    #[must_use]
    pub fn corners(&self) -> [DVec3; 8] {
        let side = self.side() * (WIDTH / 2.0);
        let ahead = self.direction * LENGTH;
        let up = DVec3::Y * HEIGHT;
        let o = self.origin;
        [
            o - side,
            o + side,
            o + ahead + side,
            o + ahead - side,
            o - side + up,
            o + side + up,
            o + ahead + side + up,
            o + ahead - side + up,
        ]
    }
}

/// Quadratic drag acceleration toward the wind,
/// `a = (ρ·C_d·A / 2m)·|w − v|·(w − v)`, m/s².
#[must_use]
pub fn drag_acceleration(drag_area: f64, mass: f64, wind: DVec3, velocity: DVec3) -> DVec3 {
    if mass <= 0.0 {
        return DVec3::ZERO;
    }
    let relative = wind - velocity;
    AIR_DENSITY * drag_area / (2.0 * mass) * relative.length() * relative
}

/// Velocity change over `dt` under [`drag_acceleration`], integrated
/// exactly along the relative wind, `r' = r / (1 + k·|r|·dt)`. Explicit
/// steps overshoot the wind for light objects such as paper; this cannot.
#[must_use]
pub fn drag_velocity_change(
    drag_area: f64,
    mass: f64,
    wind: DVec3,
    velocity: DVec3,
    dt: f64,
) -> DVec3 {
    if mass <= 0.0 {
        return DVec3::ZERO;
    }
    let relative = wind - velocity;
    let k = AIR_DENSITY * drag_area / (2.0 * mass);
    relative - relative / (1.0 + k * relative.length() * dt)
}

/// Area of a box with half extents `half`, oriented by `rotation`,
/// projected onto the plane across `direction`, m².
#[must_use]
pub fn projected_area(half: DVec3, rotation: DQuat, direction: DVec3) -> f64 {
    let d = (rotation.inverse() * direction.normalize_or_zero()).abs();
    4.0 * (half.y * half.z * d.x + half.x * half.z * d.y + half.x * half.y * d.z)
}

/// The objects the playground and chamber blow around.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Prop {
    Paper,
    Cloth,
    Basket,
    Crate,
    Barrel,
    Anvil,
}

/// Mass and shape of a [`Prop`], as the wind sees it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// kg.
    pub mass: f64,
    /// Half extents of the bounding box in the body frame, m. A barrel's
    /// axis is the body z axis.
    pub half: DVec3,
    pub drag_coefficient: f64,
    pub friction: f64,
}

/// Barrel staves' radius and the radius of the ring it stands on, m.
const BARREL_RADIUS: f64 = 0.3;
const BARREL_RING: f64 = 0.2;
/// Half the barrel's cylinder length between its rounded ends, m.
const BARREL_HALF_LENGTH: f64 = 0.15;

impl Prop {
    #[must_use]
    pub fn profile(self) -> Profile {
        let p = |mass, half: DVec3, drag_coefficient, friction| Profile {
            mass,
            half,
            drag_coefficient,
            friction,
        };
        match self {
            // One sheet, lying flat; it presents only its edge until it lifts.
            Self::Paper => p(0.005, DVec3::new(0.15, 0.002, 0.105), 1.2, 0.4),
            // A bundled cloth.
            Self::Cloth => p(0.3, DVec3::splat(0.15), 1.3, 0.5),
            Self::Basket => p(1.0, DVec3::splat(0.2), 1.2, 0.5),
            Self::Crate => p(20.0, DVec3::splat(0.35), 1.05, 0.5),
            // An empty barrel standing on end; a full one (60 kg) holds.
            Self::Barrel => p(
                15.0,
                DVec3::new(
                    BARREL_RADIUS,
                    BARREL_RADIUS,
                    BARREL_HALF_LENGTH + BARREL_RADIUS,
                ),
                1.0,
                0.6,
            ),
            Self::Anvil => p(250.0, DVec3::new(0.25, 0.2, 0.15), 1.0, 0.5),
        }
    }

    /// Add this prop resting on a floor at height `floor.y`, centered over
    /// `floor`, and return its body.
    pub fn spawn(self, world: &mut World, floor: DVec3) -> BodyId {
        let profile = self.profile();
        let material = Material {
            friction: profile.friction,
            torsional: 0.0,
            restitution: 0.1,
        };
        if self == Self::Barrel {
            let (r, h) = (BARREL_RADIUS, BARREL_HALF_LENGTH);
            let length = 2.0 * (h + r);
            let axial = profile.mass * r * r / 2.0;
            let across = profile.mass * (3.0 * r * r + length * length) / 12.0;
            let mut body = Body::new(
                profile.mass,
                DVec3::new(across, across, axial),
                floor + DVec3::Y * (h + r),
            );
            // Standing on end: the body z axis points up.
            body.orientation = DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2);
            body.prev_orientation = body.orientation;
            let id = world.add(body);
            world.add_collider(
                Collider::new(
                    id,
                    Shape::Capsule {
                        radius: r,
                        half_length: h,
                    },
                )
                .with_material(material),
            );
            // Rims: a ring of small spheres at each end that the barrel
            // stands on, inside the staves' radius so it rolls on its side.
            let foot = 0.05;
            for end in [-1.0, 1.0] {
                for i in 0..6 {
                    let angle = f64::from(i) * std::f64::consts::TAU / 6.0;
                    let offset = DVec3::new(
                        BARREL_RING * angle.cos(),
                        BARREL_RING * angle.sin(),
                        end * (h + r - foot),
                    );
                    world.add_collider(
                        Collider::new(id, Shape::Sphere { radius: foot })
                            .at(offset, DQuat::IDENTITY)
                            .with_material(material),
                    );
                }
            }
            return id;
        }
        let id = world.add(Body::new(
            profile.mass,
            Body::box_inertia(profile.mass, profile.half * 2.0),
            floor + DVec3::Y * profile.half.y,
        ));
        world.add_collider(
            Collider::new(id, Shape::Cuboid { half: profile.half }).with_material(material),
        );
        id
    }
}

/// The wind on one body in one step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    pub body: BodyId,
    /// Impulse delivered, N s. A secured body feels it without moving.
    pub impulse: DVec3,
    /// Where it acts, m.
    pub at: DVec3,
}

/// Why a creature rolled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveReason {
    /// Inside the Line when it was cast or re-aimed onto it.
    Cast,
    /// Entered the Line.
    Entry,
    /// Ended a turn in the Line.
    Turn,
}

/// A creature the Line can hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Creature {
    pub id: u32,
    pub feet: DVec3,
    /// Strength save modifier from the creature's SRD stat block.
    pub strength: i32,
}

/// A lit scene light.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flame {
    pub id: u32,
    pub position: DVec3,
    /// A lantern rather than a candle or torch.
    pub protected: bool,
    pub lit: bool,
}

/// What the Line did this step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Save {
        creature: u32,
        reason: SaveReason,
        roll: i32,
        modifier: i32,
        dc: i32,
        passed: bool,
    },
    /// Push the creature `distance` along `direction` (a failed save).
    Push {
        creature: u32,
        direction: DVec3,
        distance: f64,
    },
    /// An unprotected flame went out.
    Extinguished { flame: u32 },
    /// A protected flame rolled percentile dice; under 50 it went out.
    Gutter { flame: u32, roll: u32, out: bool },
}

/// An active Gust of Wind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gust {
    pub caster: u32,
    pub line: Line,
    pub cast_at: f64,
    /// Concentration ends here at the latest, s.
    pub until: f64,
    /// When the Bonus Action re-aim is next ready, s.
    pub reaim_ready: f64,
    /// When each creature inside next saves, s. A creature that leaves is
    /// forgotten, so coming back is an entry.
    clocks: BTreeMap<u32, f64>,
    /// Creatures that must save at their next update (inside at the cast).
    pending: BTreeSet<u32>,
    /// Protected flames that already rolled.
    rolled: BTreeSet<u32>,
    ended: bool,
}

impl Gust {
    /// Cast from the caster's feet along `aim`.
    ///
    /// # Errors
    ///
    /// Returns a message when `aim` has no horizontal part or the inputs
    /// are not finite.
    pub fn cast(caster: u32, origin: DVec3, aim: DVec3, time: f64) -> Result<Self, String> {
        if !time.is_finite() || time < 0.0 {
            return Err("Invalid Gust of Wind cast time".into());
        }
        let line = Line::new(origin, aim).ok_or("Gust of Wind needs a horizontal direction")?;
        Ok(Self {
            caster,
            line,
            cast_at: time,
            until: time + DURATION,
            reaim_ready: time + REAIM_COOLDOWN,
            clocks: BTreeMap::new(),
            pending: BTreeSet::new(),
            rolled: BTreeSet::new(),
            ended: false,
        })
    }

    #[must_use]
    pub fn active(&self, time: f64) -> bool {
        !self.ended && time < self.until
    }

    /// Concentration ends; the field is gone.
    pub fn end(&mut self) {
        self.ended = true;
        self.clocks.clear();
        self.pending.clear();
    }

    /// The Line follows the caster; its direction does not change.
    pub fn follow(&mut self, origin: DVec3) {
        if origin.is_finite() {
            self.line.origin = origin;
        }
    }

    /// Bonus Action: point the Line along `aim`.
    ///
    /// # Errors
    ///
    /// Returns a message while the re-aim is cooling down, after the spell
    /// ended, or for an unusable direction.
    pub fn reaim(&mut self, aim: DVec3, time: f64) -> Result<(), String> {
        if !self.active(time) {
            return Err("Gust of Wind has ended".into());
        }
        if time < self.reaim_ready {
            return Err("Gust of Wind re-aim is cooling down".into());
        }
        let line =
            Line::new(self.line.origin, aim).ok_or("Gust of Wind needs a horizontal direction")?;
        self.line = line;
        self.reaim_ready = time + REAIM_COOLDOWN;
        Ok(())
    }

    /// Saves and pushes for creatures at `time`. Everyone inside at the
    /// cast saves; a creature saves when it enters, and again every
    /// [`TURN`] while it stays. `d20` rolls one die.
    pub fn creatures(
        &mut self,
        time: f64,
        creatures: &[Creature],
        d20: &mut impl FnMut() -> i32,
    ) -> Vec<Event> {
        self.creatures_with(time, creatures, &mut |_| d20())
    }

    /// Resolve saves using a die source bound to each creature's identity.
    pub fn creatures_with(
        &mut self,
        time: f64,
        creatures: &[Creature],
        d20: &mut impl FnMut(u32) -> i32,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        if !self.active(time) {
            return events;
        }
        let first = time <= self.cast_at;
        let inside: BTreeSet<u32> = creatures
            .iter()
            .filter(|c| c.id != self.caster && self.line.contains(c.feet))
            .map(|c| c.id)
            .collect();
        self.clocks.retain(|id, _| inside.contains(id));
        for creature in creatures.iter().filter(|c| inside.contains(&c.id)) {
            let reason = match self.clocks.get(&creature.id) {
                None if first || self.pending.remove(&creature.id) => SaveReason::Cast,
                None => SaveReason::Entry,
                Some(&due) if time >= due => SaveReason::Turn,
                Some(_) => continue,
            };
            let due = self.clocks.get(&creature.id).copied();
            self.clocks
                .insert(creature.id, due.map_or(time, |d| d) + TURN);
            let roll = d20(creature.id);
            let passed = roll + creature.strength >= SPELL_SAVE_DC;
            events.push(Event::Save {
                creature: creature.id,
                reason,
                roll,
                modifier: creature.strength,
                dc: SPELL_SAVE_DC,
                passed,
            });
            if !passed {
                events.push(Event::Push {
                    creature: creature.id,
                    direction: self.line.direction,
                    distance: PUSH_DISTANCE,
                });
            }
        }
        events
    }

    /// A creature's velocity with its part toward the caster halved while
    /// it stands in the Line: two feet of movement per foot closer.
    #[must_use]
    pub fn approach(&self, time: f64, feet: DVec3, velocity: DVec3) -> DVec3 {
        if !self.active(time) || !self.line.contains(feet) {
            return velocity;
        }
        let toward = DVec3::new(
            self.line.origin.x - feet.x,
            0.0,
            self.line.origin.z - feet.z,
        )
        .normalize_or_zero();
        let closing = velocity.dot(toward);
        if closing > 0.0 {
            velocity - toward * closing * (1.0 - 1.0 / APPROACH_COST)
        } else {
            velocity
        }
    }

    /// Put out flames in the Line. A candle or torch goes out; a lantern
    /// rolls once, on `percentile` (1 to 100), and goes out on 50 or less.
    pub fn flames(
        &mut self,
        time: f64,
        flames: &mut [Flame],
        percentile: &mut impl FnMut() -> u32,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        if !self.active(time) {
            return events;
        }
        for flame in flames
            .iter_mut()
            .filter(|f| f.lit && self.line.contains(f.position))
        {
            if !flame.protected {
                flame.lit = false;
                events.push(Event::Extinguished { flame: flame.id });
            } else if self.rolled.insert(flame.id) {
                let roll = percentile();
                let out = roll <= PROTECTED_FLAME_PERCENT;
                flame.lit = !out;
                events.push(Event::Gutter {
                    flame: flame.id,
                    roll,
                    out,
                });
            }
        }
        events
    }

    /// Whether a lit lantern at `position` dances wildly in the wind.
    #[must_use]
    pub fn flickers(&self, time: f64, position: DVec3) -> bool {
        self.active(time) && self.line.contains(position)
    }

    /// Whether a gas or vapor cloud is dispersed.
    #[must_use]
    pub fn disperses(&self, time: f64, center: DVec3, radius: f64) -> bool {
        self.active(time) && self.line.touches_sphere(center, radius)
    }

    /// Apply this step's wind to `props` as forces for the next
    /// [`World::step`]. Each prop's frontal area comes from its current
    /// orientation; the force acts at the middle of the part inside the
    /// Line's height, so a tall object caught low or high tips. Secured
    /// (static or kinematic) bodies feel it but do not move.
    pub fn blow(&self, time: f64, world: &mut World, props: &[(BodyId, Profile)]) -> Vec<Blow> {
        let mut blows = Vec::new();
        if !self.active(time) {
            return blows;
        }
        let dt = world.dt;
        let wind = self.line.wind();
        for &(id, profile) in props {
            let body = world[id];
            if body.removed || !self.line.contains(body.pos.with_y(self.line.origin.y)) {
                continue;
            }
            let extent = (body.orientation * DVec3::X).y.abs() * profile.half.x
                + (body.orientation * DVec3::Y).y.abs() * profile.half.y
                + (body.orientation * DVec3::Z).y.abs() * profile.half.z;
            let floor = self.line.origin.y;
            let (bottom, top) = (body.pos.y - extent, body.pos.y + extent);
            let (low, high) = (bottom.max(floor), top.min(floor + HEIGHT));
            if high <= low {
                continue;
            }
            let fraction = (high - low) / (top - bottom).max(1e-9);
            let relative = wind - body.vel;
            if relative.length() < 1e-9 {
                continue;
            }
            let area = projected_area(profile.half, body.orientation, relative) * fraction;
            let dv = drag_velocity_change(
                profile.drag_coefficient * area,
                profile.mass,
                wind,
                body.vel,
                dt,
            );
            let impulse = dv * profile.mass;
            let at = body.pos.with_y((low + high) / 2.0);
            if body.kind == physics::BodyKind::Dynamic {
                world[id].apply_force_at(impulse / dt, at);
            }
            blows.push(Blow {
                body: id,
                impulse,
                at,
            });
        }
        blows
    }

    /// Velocity change for one step of an ordinary flight (an arrow or
    /// bolt) at `position`. Spell flights and the switch's off position
    /// return zero.
    #[must_use]
    pub fn arrow_drag(&self, time: f64, position: DVec3, velocity: DVec3, dt: f64) -> DVec3 {
        if !DRAG_ORDINARY_PROJECTILES || !self.active(time) || !self.line.contains(position) {
            return DVec3::ZERO;
        }
        drag_velocity_change(ARROW_DRAG_AREA, ARROW_MASS, self.line.wind(), velocity, dt)
    }

    /// Mark everyone inside at a re-aim as saving on their next update,
    /// as at the cast.
    pub fn sweep(&mut self, creatures: &[Creature]) {
        for c in creatures {
            if c.id != self.caster && self.line.contains(c.feet) && !self.clocks.contains_key(&c.id)
            {
                self.pending.insert(c.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::{BodyKind, Uniform};

    const DT: f64 = 1.0 / 120.0;

    /// Deterministic dice for tests (SplitMix64).
    struct Dice(u64);
    impl Dice {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }
        fn die(&mut self, sides: u64) -> u32 {
            (self.next() % sides) as u32 + 1
        }
    }

    fn floor(world: &mut World) {
        let id = world.add(
            Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static),
        );
        world.add_collider(
            Collider::new(
                id,
                Shape::Cuboid {
                    half: DVec3::new(60.0, 0.5, 60.0),
                },
            )
            .with_material(Material {
                friction: 0.5,
                torsional: 0.0,
                restitution: 0.1,
            }),
        );
    }

    fn gust() -> Gust {
        Gust::cast(0, DVec3::ZERO, DVec3::X, 0.0).unwrap()
    }

    #[test]
    fn the_line_is_sixty_by_ten_feet() {
        let line = gust().line;
        assert!((LENGTH / FEET - 60.0).abs() < 1e-9 && (WIDTH / FEET - 10.0).abs() < 1e-9);
        assert!(line.contains(DVec3::new(0.01, 0.0, 0.0)));
        assert!(line.contains(DVec3::new(18.28, 1.0, 1.52)));
        assert!(!line.contains(DVec3::new(18.30, 1.0, 0.0)));
        assert!(!line.contains(DVec3::new(5.0, 0.0, 1.53)));
        assert!(!line.contains(DVec3::new(-0.1, 0.0, 0.0)));
        let c = line.corners();
        assert!((c[0].distance(c[1]) - WIDTH).abs() < 1e-9);
        assert!((c[1].distance(c[2]) - LENGTH).abs() < 1e-9);
        assert!(line.touches_sphere(DVec3::new(10.0, 0.0, 2.5), 1.0));
        assert!(!line.touches_sphere(DVec3::new(10.0, 0.0, 3.0), 1.0));
    }

    #[test]
    fn a_failed_save_pushes_fifteen_feet_and_a_pass_does_not() {
        let mut g = gust();
        let creatures = [
            Creature {
                id: 1,
                feet: DVec3::new(5.0, 0.0, 0.0),
                strength: 0,
            },
            Creature {
                id: 2,
                feet: DVec3::new(6.0, 0.0, 1.0),
                strength: 3,
            },
            Creature {
                id: 3,
                feet: DVec3::new(6.0, 0.0, 4.0),
                strength: 0,
            },
        ];
        let mut rolls = [4, 12].into_iter();
        let events = g.creatures(0.0, &creatures, &mut || rolls.next().unwrap());
        assert_eq!(
            events,
            vec![
                Event::Save {
                    creature: 1,
                    reason: SaveReason::Cast,
                    roll: 4,
                    modifier: 0,
                    dc: 15,
                    passed: false
                },
                Event::Push {
                    creature: 1,
                    direction: DVec3::X,
                    distance: PUSH_DISTANCE
                },
                Event::Save {
                    creature: 2,
                    reason: SaveReason::Cast,
                    roll: 12,
                    modifier: 3,
                    dc: 15,
                    passed: true
                },
            ]
        );
        assert!((PUSH_DISTANCE / FEET - 15.0).abs() < 1e-9);
    }

    #[test]
    fn saves_repeat_every_turn_inside_and_restart_on_entry() {
        let mut g = gust();
        let inside = [Creature {
            id: 1,
            feet: DVec3::new(5.0, 0.0, 0.0),
            strength: 0,
        }];
        let mut saves = Vec::new();
        let mut step = 0;
        while f64::from(step) * DT <= 13.0 {
            let t = f64::from(step) * DT;
            for e in g.creatures(t, &inside, &mut || 20) {
                if let Event::Save { reason, .. } = e {
                    saves.push((t, reason));
                }
            }
            step += 1;
        }
        let times: Vec<f64> = saves.iter().map(|s| s.0).collect();
        assert_eq!(saves.len(), 3, "{saves:?}");
        assert!(times[0] == 0.0 && (times[1] - 6.0).abs() < DT && (times[2] - 12.0).abs() < DT);
        assert_eq!(saves[1].1, SaveReason::Turn);
        // Leave, then come back: an entry save, then a new six-second clock.
        let outside = [Creature {
            feet: DVec3::new(5.0, 0.0, 9.0),
            ..inside[0]
        }];
        assert!(g.creatures(13.5, &outside, &mut || 20).is_empty());
        let back = g.creatures(14.0, &inside, &mut || 20);
        assert!(matches!(
            back[0],
            Event::Save {
                reason: SaveReason::Entry,
                ..
            }
        ));
        assert!(g.creatures(19.9, &inside, &mut || 20).is_empty());
        assert_eq!(g.creatures(20.0, &inside, &mut || 20).len(), 1);
    }

    #[test]
    fn movement_toward_the_caster_is_halved() {
        let g = gust();
        let feet = DVec3::new(10.0, 0.0, 0.0);
        let toward = g.approach(0.0, feet, DVec3::new(-4.0, 0.0, 0.0));
        assert!((toward.x + 2.0).abs() < 1e-9);
        assert_eq!(
            g.approach(0.0, feet, DVec3::new(4.0, 0.0, 0.0)).x,
            4.0,
            "away is free"
        );
        let across = g.approach(0.0, feet, DVec3::new(0.0, 0.0, 3.0));
        assert_eq!(across, DVec3::new(0.0, 0.0, 3.0));
        assert_eq!(
            g.approach(0.0, DVec3::new(10.0, 0.0, 5.0), DVec3::NEG_X),
            DVec3::NEG_X,
            "outside the Line"
        );
    }

    #[test]
    fn drag_scales_with_area_over_mass_and_never_overshoots() {
        let wind = DVec3::X * WIND_SPEED;
        let a = drag_acceleration(0.5, 10.0, wind, DVec3::ZERO);
        let b = drag_acceleration(1.0, 10.0, wind, DVec3::ZERO);
        let c = drag_acceleration(1.0, 20.0, wind, DVec3::ZERO);
        assert!((b.x / a.x - 2.0).abs() < 1e-12 && (b.x / c.x - 2.0).abs() < 1e-12);
        assert!((b.x - 1.225 * 1.0 / 20.0 * 400.0).abs() < 1e-9);
        // A paper sheet catches the full wind in one step without passing it.
        let paper = Prop::Paper.profile();
        let mut v = DVec3::ZERO;
        for _ in 0..240 {
            v += drag_velocity_change(1.2 * 0.0315, paper.mass, wind, v, DT);
            assert!(v.x <= WIND_SPEED);
        }
        assert!(v.x > 19.0);
    }

    struct Scene {
        world: World,
        props: Vec<(BodyId, Profile)>,
        ids: Vec<(Prop, BodyId)>,
    }

    fn row(kinds: &[Prop]) -> Scene {
        let mut world = World::new(DT);
        floor(&mut world);
        let mut props = Vec::new();
        let mut ids = Vec::new();
        for (i, &kind) in kinds.iter().enumerate() {
            // Spread across the Line so the row does not shelter itself.
            let at = DVec3::new(3.0, 0.0, -1.2 + 0.8 * i as f64);
            let id = kind.spawn(&mut world, at);
            props.push((id, kind.profile()));
            ids.push((kind, id));
        }
        Scene { world, props, ids }
    }

    fn up(world: &World, id: BodyId, kind: Prop) -> f64 {
        let axis = if kind == Prop::Barrel {
            DVec3::Z
        } else {
            DVec3::Y
        };
        (world[id].orientation * axis).y
    }

    #[test]
    fn light_props_fly_the_crate_slides_the_barrel_tips_and_the_anvil_stays() {
        let kinds = [
            Prop::Paper,
            Prop::Basket,
            Prop::Crate,
            Prop::Barrel,
            Prop::Anvil,
        ];
        let mut scene = row(&kinds);
        let g = Uniform(DVec3::new(0.0, -9.81, 0.0));
        for _ in 0..60 {
            scene.world.step(&g);
        }
        let start: Vec<DVec3> = scene
            .ids
            .iter()
            .map(|(_, id)| scene.world[*id].pos)
            .collect();
        let gust = gust();
        let mut tipped_barrel = false;
        for step in 0..240 {
            let t = f64::from(step) * DT;
            gust.blow(t, &mut scene.world, &scene.props);
            scene.world.step(&g);
            let (_, barrel) = scene.ids[3];
            tipped_barrel |= up(&scene.world, barrel, Prop::Barrel).abs() < 0.3;
        }
        let moved = |i: usize| {
            let (_, id) = scene.ids[i];
            scene.world[id].pos - start[i]
        };
        assert!(moved(0).x > 8.0, "paper {}", moved(0));
        assert!(moved(1).x > 4.0, "basket {}", moved(1));
        let crate_ = moved(2);
        assert!(crate_.x > 0.3, "crate {crate_}");
        assert!(
            up(&scene.world, scene.ids[2].1, Prop::Crate) > 0.99,
            "the crate slides upright"
        );
        assert!(tipped_barrel, "the barrel tips");
        assert!(moved(3).x > 0.5, "the barrel rolls {}", moved(3));
        assert!(moved(4).length() < 1e-3, "anvil {}", moved(4));
    }

    #[test]
    fn secured_props_feel_the_wind_without_moving() {
        let mut scene = row(&[Prop::Crate]);
        let (_, id) = scene.ids[0];
        scene.world[id].kind = BodyKind::Static;
        let before = scene.world[id].pos;
        let g = Uniform(DVec3::new(0.0, -9.81, 0.0));
        let blows = gust().blow(0.0, &mut scene.world, &scene.props);
        assert!(blows[0].impulse.x > 0.0);
        for _ in 0..120 {
            gust().blow(0.0, &mut scene.world, &scene.props);
            scene.world.step(&g);
        }
        assert_eq!(scene.world[id].pos, before);
    }

    #[test]
    fn a_tall_object_caught_low_tips_from_the_off_center_push() {
        let mut world = World::new(DT);
        floor(&mut world);
        // A 6 m post standing on a raised plinth; only its lower half is in
        // the Line, so the wind acts below its center of mass.
        let half = DVec3::new(0.15, 3.0, 0.15);
        let profile = Profile {
            mass: 4.0,
            half,
            drag_coefficient: 1.2,
            friction: 0.9,
        };
        let id = world.add(Body::new(
            4.0,
            Body::box_inertia(4.0, half * 2.0),
            DVec3::new(3.0, 3.0, 0.0),
        ));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
        let gust = gust();
        let blows = gust.blow(0.0, &mut world, &[(id, profile)]);
        assert!((blows[0].at.y - HEIGHT / 2.0).abs() < 1e-9);
        assert!(world[id].torque.length() > 0.0);
    }

    #[test]
    fn candles_go_out_and_lanterns_gutter_half_the_time() {
        let mut g = gust();
        let mut flames: Vec<Flame> = (0..400)
            .map(|i| Flame {
                id: i,
                position: DVec3::new(2.0 + f64::from(i % 40) * 0.4, 1.0, 0.0),
                protected: i >= 200,
                lit: true,
            })
            .collect();
        flames.push(Flame {
            id: 999,
            position: DVec3::new(2.0, 1.0, 3.0),
            protected: false,
            lit: true,
        });
        let mut dice = Dice(7);
        let events = g.flames(0.0, &mut flames, &mut || dice.die(100));
        assert!(flames[..200].iter().all(|f| !f.lit));
        assert!(flames[400].lit, "outside the Line");
        let out = flames[200..400].iter().filter(|f| !f.lit).count();
        assert!((80..=120).contains(&out), "{out} of 200 lanterns out");
        // Each lantern rolls only once.
        let again = g.flames(1.0, &mut flames, &mut || 1);
        assert!(again.is_empty());
        assert_eq!(events.len(), 400);
        // Fixed seed, fixed result.
        let mut g = gust();
        let mut lantern = [Flame {
            id: 1,
            position: DVec3::new(4.0, 1.0, 0.0),
            protected: true,
            lit: true,
        }];
        let mut dice = Dice(10_456);
        let roll = Dice(10_456).die(100);
        g.flames(0.0, &mut lantern, &mut || dice.die(100));
        assert_eq!(lantern[0].lit, roll > PROTECTED_FLAME_PERCENT);
    }

    #[test]
    fn re_aim_waits_for_its_cooldown() {
        let mut g = gust();
        assert!(g.reaim(DVec3::Z, 3.0).is_err());
        assert_eq!(g.line.direction, DVec3::X);
        g.reaim(DVec3::Z, 6.0).unwrap();
        assert_eq!(g.line.direction, DVec3::Z);
        assert!(g.reaim(DVec3::X, 11.9).is_err());
        g.reaim(DVec3::X, 12.0).unwrap();
        g.follow(DVec3::new(1.0, 0.0, 1.0));
        assert_eq!(g.line.direction, DVec3::X, "following keeps the aim");
        assert!(g.line.contains(DVec3::new(19.0, 0.0, 1.0)));
    }

    #[test]
    fn concentration_end_removes_the_field() {
        let mut scene = row(&[Prop::Basket]);
        let mut g = gust();
        g.end();
        assert!(!g.active(1.0));
        assert!(g.blow(1.0, &mut scene.world, &scene.props).is_empty());
        let c = [Creature {
            id: 1,
            feet: DVec3::new(5.0, 0.0, 0.0),
            strength: -5,
        }];
        assert!(g.creatures(1.0, &c, &mut || 1).is_empty());
        assert!(g.reaim(DVec3::Z, 30.0).is_err());
        let timed = gust();
        assert!(timed.active(59.9) && !timed.active(DURATION));
        assert_eq!(timed.approach(61.0, c[0].feet, DVec3::NEG_X), DVec3::NEG_X);
    }

    /// Fly an arrow from 1.5 m up at `velocity` until it lands; return the
    /// landing point.
    fn shoot(gust: Option<&Gust>, velocity: DVec3) -> DVec3 {
        let (mut p, mut v) = (DVec3::new(0.0, 1.5, 0.0), velocity);
        while p.y > 0.0 {
            if let Some(g) = gust {
                v += g.arrow_drag(0.0, p, v, DT);
            }
            v.y -= 9.81 * DT;
            p += v * DT;
        }
        p
    }

    #[test]
    fn arrows_into_the_wind_fall_short_and_across_it_curve() {
        // The archer stands at the far end of the Line, shooting back at the
        // caster.
        let g = Gust::cast(0, DVec3::new(-18.0, 0.0, 0.0), DVec3::X, 0.0).unwrap();
        let aim = DVec3::new(-24.0 * 0.98, 24.0 * 0.2, 0.0);
        let still = shoot(None, aim);
        let windy = shoot(Some(&g), aim);
        assert!(
            windy.x - still.x > 2.0,
            "into the wind: {} vs {}",
            windy.x,
            still.x
        );
        let across = Gust::cast(0, DVec3::new(10.0, 0.0, -1.0), DVec3::Z, 0.0).unwrap();
        let curved = shoot(Some(&across), DVec3::new(24.0 * 0.98, 24.0 * 0.2, 0.0));
        assert!(curved.z > 0.2, "across: {curved}");
    }

    #[test]
    fn re_aiming_onto_a_creature_makes_it_save() {
        let mut g = gust();
        let c = [Creature {
            id: 4,
            feet: DVec3::new(0.0, 0.0, 6.0),
            strength: 0,
        }];
        assert!(g.creatures(0.0, &c, &mut || 1).is_empty());
        g.reaim(DVec3::Z, 6.0).unwrap();
        g.sweep(&c);
        let events = g.creatures(6.0, &c, &mut || 1);
        assert!(matches!(
            events[0],
            Event::Save {
                reason: SaveReason::Cast,
                passed: false,
                ..
            }
        ));
    }
}
