//! Reverse Gravity (SRD 5.2.1) as an inverted gravity cylinder.
//!
//! Level 7 Transmutation. Casting time: Action. Range: 100 feet.
//! Components: V, S, M (a lodestone and iron filings). Duration:
//! Concentration, up to 1 minute. Gravity reverses in a 50-foot-radius,
//! 100-foot-high Cylinder centered on a point within range. Creatures and
//! objects that aren't anchored to the ground fall upward and reach the top;
//! a creature can make a Dexterity saving throw to grab a fixed object it
//! can reach. A ceiling or anchored object met on the way up is struck as
//! in a downward fall, anything that reaches the top hovers there, and when
//! the spell ends everything falls.
//!
//! The cylinder stands on the ground at the chosen point. Inside it, the
//! [`Gravity`] field accelerates every dynamic body upward at standard
//! gravity; within the top [`HOVER_BAND`] a critically damped spring pulls
//! the body's reference point (a body's center, a creature's feet) onto the
//! top plane, so bodies rising from the ground settle there without passing
//! through. Anchored bodies feel the same field but stay put: static and
//! kinematic bodies ignore fields, and a secured prop is welded to static
//! geometry. A creature that makes its save with a fixed object in reach is
//! welded to the nearest point of that object until the spell ends.
//!
//! A [`Fall`] per body remembers the extremes of its current flight, so a
//! strike on a ceiling deals falling damage for the upward span and a
//! landing after the spell ends deals it for the downward span, by the same
//! table. Everything serializes, so a checkpoint taken mid-rise continues
//! exactly.

use glam::DVec3;
use physics::{BodyId, BodyKind, Joint, JointId, World};
use serde::{Deserialize, Serialize};

/// One foot, m.
pub const FOOT: f64 = 0.3048;
/// Spell range: 100 feet.
pub const RANGE: f64 = 100.0 * FOOT;
/// Cylinder radius: 50 feet.
pub const RADIUS: f64 = 50.0 * FOOT;
/// Cylinder height: 100 feet.
pub const HEIGHT: f64 = 100.0 * FOOT;
/// Concentration, up to 1 minute, s.
pub const DURATION: f64 = 60.0;
/// Spell level.
pub const LEVEL: u8 = 7;
/// Standard gravity, m/s^2. The field reverses it inside the cylinder.
pub const GRAVITY: f64 = 9.81;
/// Depth below the top plane where the hover spring takes over, m.
pub const HOVER_BAND: f64 = 1.5;
/// Natural frequency of the hover spring, rad/s. A body arriving from the
/// ground enters the band at about 24 m/s; critical damping stops it short
/// of the top plane whenever the frequency exceeds that speed divided by
/// the band depth (16 rad/s).
pub const HOVER_OMEGA: f64 = 20.0;
/// Light horizontal damping on hovering bodies, 1/s.
pub const HOVER_DRAG: f64 = 0.3;
/// How far a creature can reach for a fixed object: 5 feet.
pub const GRAB_REACH: f64 = 5.0 * FOOT;
/// Force and torque limit of a creature's hold, N and N m: effectively
/// unbreakable, and finite so a checkpoint stays valid JSON.
pub const HOLD_LIMIT: f64 = 1.0e12;
/// Vertical speed below which a stop is not a strike, m/s.
pub const STRIKE_SPEED: f64 = 1.5;
/// The overlay's SRD line.
pub const SRD_LINE: &str = "Reverse Gravity - level 7 Transmutation - range 100 ft - \
    50-ft-radius, 100-ft-high Cylinder - Concentration, up to 1 minute - \
    Dexterity save to grab a fixed object";

/// The spell's area: a vertical cylinder standing on `base`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cylinder {
    /// Center of the bottom face, on the ground, m.
    pub base: DVec3,
    pub radius: f64,
    pub height: f64,
}

impl Cylinder {
    /// The SRD cylinder standing on `base`.
    #[must_use]
    pub fn at(base: DVec3) -> Self {
        Self {
            base,
            radius: RADIUS,
            height: HEIGHT,
        }
    }

    /// Height of the top plane, m.
    #[must_use]
    pub fn top(&self) -> f64 {
        self.base.y + self.height
    }

    /// Horizontal distance from the axis, m.
    #[must_use]
    pub fn axis_distance(&self, p: DVec3) -> f64 {
        let d = p - self.base;
        (d.x * d.x + d.z * d.z).sqrt()
    }

    /// Whether `p` lies inside, boundary included.
    #[must_use]
    pub fn contains(&self, p: DVec3) -> bool {
        self.axis_distance(p) <= self.radius && (self.base.y..=self.top()).contains(&p.y)
    }
}

/// The acceleration field while the spell may be active: reversed inside
/// the cylinder, standard outside it or once `active` is false.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gravity {
    pub cylinder: Cylinder,
    pub active: bool,
}

impl Gravity {
    /// Acceleration of a reference point at `pos` moving at `vel`.
    #[must_use]
    pub fn at(&self, pos: DVec3, vel: DVec3) -> DVec3 {
        if !self.active || !self.cylinder.contains(pos) {
            return DVec3::NEG_Y * GRAVITY;
        }
        let below = self.cylinder.top() - pos.y;
        if below > HOVER_BAND {
            return DVec3::Y * GRAVITY;
        }
        let w = HOVER_OMEGA;
        DVec3::new(
            -HOVER_DRAG * vel.x,
            w * w * below - 2.0 * w * vel.y,
            -HOVER_DRAG * vel.z,
        )
    }
}

impl physics::Field for Gravity {
    fn accel(&self, pos: DVec3, vel: DVec3) -> DVec3 {
        self.at(pos, vel)
    }
}

/// Falling damage dice for a fall of `height` meters: 1d6 per full 10 feet,
/// at most 20d6. A small tolerance keeps an exact multiple of 10 feet from
/// rounding down.
#[must_use]
pub fn falling_dice(height: f64) -> u32 {
    ((height / (10.0 * FOOT) + 1e-9).floor().max(0.0) as u32).min(20)
}

/// One Dexterity saving throw to grab a fixed object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    pub d20: i32,
    pub modifier: i32,
    pub dc: i32,
    pub success: bool,
}

impl Save {
    /// A save of `d20` plus `modifier` against `dc`; meeting the DC succeeds.
    #[must_use]
    pub fn new(d20: i32, modifier: i32, dc: i32) -> Self {
        Self {
            d20,
            modifier,
            dc,
            success: d20 + modifier >= dc,
        }
    }
}

/// What a creature's grab came to.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Grab {
    /// It made its save and holds the fixed object at `anchor`.
    Held { anchor: DVec3 },
    /// It made its save but nothing fixed was within reach.
    NothingInReach,
    /// It failed its save.
    Failed,
}

impl Grab {
    /// Resolve a save against the nearest fixed point, if any.
    #[must_use]
    pub fn resolve(save: Save, from: DVec3, nearest: Option<DVec3>) -> Self {
        if !save.success {
            return Self::Failed;
        }
        match nearest {
            Some(anchor) if anchor.distance(from) <= GRAB_REACH => Self::Held { anchor },
            _ => Self::NothingInReach,
        }
    }

    #[must_use]
    pub fn held(self) -> bool {
        matches!(self, Self::Held { .. })
    }
}

/// The nearest point on any static collider in `world` to `from`, if one
/// lies within [`GRAB_REACH`], with the static body it belongs to. The
/// `ground` bodies are not objects to grab.
#[must_use]
pub fn nearest_fixed(world: &World, from: DVec3, ground: &[BodyId]) -> Option<(BodyId, DVec3)> {
    world
        .colliders()
        .iter()
        .filter(|c| {
            let body = &world[c.body];
            body.kind == BodyKind::Static && !body.removed && !ground.contains(&c.body)
        })
        .map(|c| (c.body, c.closest_point(world, from)))
        .filter(|(_, p)| p.distance(from) <= GRAB_REACH)
        .min_by(|a, b| a.1.distance(from).total_cmp(&b.1.distance(from)))
}

/// The extremes of a body's current flight, for falling damage.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fall {
    /// Highest reference height since the last strike, m.
    pub high: f64,
    /// Lowest reference height since the last strike, m.
    pub low: f64,
}

/// A strike that ended a fall, up into a ceiling or down onto the ground.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Strike {
    pub upward: bool,
    /// Distance fallen, m.
    pub span: f64,
    /// Falling damage dice (d6).
    pub dice: u32,
    /// Reference point at the strike, m.
    pub at: DVec3,
}

impl Fall {
    #[must_use]
    pub fn new(y: f64) -> Self {
        Self { high: y, low: y }
    }

    /// Observe one step: the reference point ended at `at`, its vertical
    /// speed went from `before` to `after`, and `contact` is the reference
    /// height at which a contact opposing its motion was met, if one was.
    /// The solver can stop or bounce a body just short of a surface within
    /// one step, so the strike is measured where the surface was met rather
    /// than where the step ended. A stop of at least half the speed against
    /// a contact is a strike, measured from the far extreme of the flight.
    pub fn observe(
        &mut self,
        at: DVec3,
        before: f64,
        after: f64,
        contact: Option<f64>,
    ) -> Option<Strike> {
        self.high = self.high.max(at.y);
        self.low = self.low.min(at.y);
        let upward = before > 0.0;
        let stopped = before.abs() >= STRIKE_SPEED && after * before.signum() < before.abs() * 0.5;
        let Some(met) = contact.filter(|_| stopped) else {
            return None;
        };
        let span = if upward {
            met.max(self.high) - self.low
        } else {
            self.high - met.min(self.low)
        };
        *self = Self::new(at.y);
        Some(Strike {
            upward,
            span,
            dice: falling_dice(span),
            at: DVec3::new(at.x, met, at.z),
        })
    }
}

/// A body's strike, for the caller to roll and apply.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Impact {
    pub body: BodyId,
    pub strike: Strike,
}

/// A creature's save at the cast or on entering, and its hold if any.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hold {
    pub body: BodyId,
    pub save: Save,
    pub grab: Grab,
    /// The weld to the fixed object, while held.
    pub joint: Option<JointId>,
}

/// Why a cast was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The point is beyond 100 feet.
    OutOfRange,
}

/// One caster's concentration on Reverse Gravity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReverseGravity {
    pub gravity: Gravity,
    /// Tick when concentration ends.
    pub ends: u64,
    /// Each dynamic body's current flight.
    pub falls: Vec<(BodyId, Fall)>,
    /// Creature saves, in the order they were made.
    pub holds: Vec<Hold>,
}

/// Whole steps in `seconds` at the world's step length.
#[must_use]
pub fn ticks(world: &World, seconds: f64) -> u64 {
    (seconds / world.dt).round() as u64
}

impl ReverseGravity {
    /// Reverse gravity in the cylinder standing on `point`, cast by a
    /// caster at `caster`. Every dynamic body wakes, since a sleeper would
    /// otherwise not notice the field.
    ///
    /// # Errors
    ///
    /// Refuses a point beyond 100 feet; a refusal changes nothing.
    pub fn cast(world: &mut World, caster: DVec3, point: DVec3) -> Result<Self, Refusal> {
        if caster.distance(point) > RANGE {
            return Err(Refusal::OutOfRange);
        }
        let falls = (0..world.bodies().len())
            .map(|i| BodyId(i as u32))
            .filter(|&id| world[id].kind == BodyKind::Dynamic && !world[id].removed)
            .map(|id| (id, Fall::new(world[id].pos.y)))
            .collect();
        let spell = Self {
            gravity: Gravity {
                cylinder: Cylinder::at(point),
                active: true,
            },
            ends: world.tick + ticks(world, DURATION),
            falls,
            holds: Vec::new(),
        };
        spell.wake(world);
        Ok(spell)
    }

    #[must_use]
    pub fn active(&self) -> bool {
        self.gravity.active
    }

    fn wake(&self, world: &mut World) {
        for (id, _) in &self.falls {
            world.wake(*id);
        }
    }

    /// Start tracking a body added after the cast.
    pub fn track(&mut self, world: &World, body: BodyId) {
        if !self.falls.iter().any(|(id, _)| *id == body) {
            self.falls.push((body, Fall::new(world[body].pos.y)));
        }
    }

    /// A creature's body makes its Dexterity save, at the cast or when it
    /// enters the cylinder. On a success with a fixed object in reach, it is
    /// welded to that object's nearest point and stays down; otherwise it
    /// falls upward with everything else. The `ground` bodies don't count
    /// as fixed objects.
    pub fn grab(&mut self, world: &mut World, body: BodyId, save: Save, ground: &[BodyId]) -> Grab {
        let from = world[body].pos;
        let nearest = nearest_fixed(world, from, ground);
        let grab = Grab::resolve(save, from, nearest.map(|(_, p)| p));
        let joint = match (grab, nearest) {
            (Grab::Held { anchor }, Some((fixed, _))) => Some(world.add_joint(
                Joint::weld_here(world, fixed, body, anchor).limited(HOLD_LIMIT, HOLD_LIMIT),
            )),
            _ => None,
        };
        self.holds.push(Hold {
            body,
            save,
            grab,
            joint,
        });
        grab
    }

    /// Whether `body` holds a fixed object.
    #[must_use]
    pub fn held(&self, body: BodyId) -> bool {
        self.holds
            .iter()
            .any(|h| h.body == body && h.joint.is_some())
    }

    /// End the spell: gravity returns, holds let go, and everything falls.
    /// Losing concentration ends it the same way.
    pub fn end(&mut self, world: &mut World) {
        if !self.gravity.active {
            return;
        }
        self.gravity.active = false;
        self.ends = self.ends.min(world.tick);
        for hold in &mut self.holds {
            if let Some(joint) = hold.joint.take() {
                world.remove_joint(joint);
            }
        }
        self.wake(world);
    }

    /// Advance the world one step under the field and report every strike.
    /// The spell ends by itself when its minute runs out. Keep stepping
    /// through this after the end, so the landings are reported.
    pub fn step(&mut self, world: &mut World) -> Vec<Impact> {
        let before: Vec<f64> = self.falls.iter().map(|(id, _)| world[*id].vel.y).collect();
        world.step(&self.gravity);
        let mut impacts = Vec::new();
        for ((id, fall), v0) in self.falls.iter_mut().zip(before) {
            let body = &world[*id];
            if body.removed {
                continue;
            }
            // The gap each opposing contact reported at the start of the
            // step, which the body closed before it stopped.
            let gap = world
                .contacts
                .iter()
                .filter_map(|c| {
                    let on_body = if c.body_b == *id {
                        c.impulse.y
                    } else if c.body_a == *id {
                        -c.impulse.y
                    } else {
                        return None;
                    };
                    (on_body * v0 < 0.0).then_some(c.separation.max(0.0))
                })
                .reduce(f64::min);
            let contact = gap.map(|gap| body.prev_pos.y + gap * v0.signum());
            if let Some(strike) = fall.observe(body.pos, v0, body.vel.y, contact) {
                impacts.push(Impact { body: *id, strike });
            }
        }
        if self.gravity.active && world.tick >= self.ends {
            self.end(world);
        }
        impacts
    }
}

/// A creature's reference point (its feet) moving vertically under the
/// field, for characters that are not rigid bodies: the vertical
/// acceleration at `feet` moving at `vertical_speed`.
#[must_use]
pub fn creature_accel(gravity: &Gravity, feet: DVec3, vertical_speed: f64) -> f64 {
    gravity.at(feet, DVec3::Y * vertical_speed).y
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::trace::{Tolerance, Trace};
    use physics::{Body, Collider, Shape};

    const DT: f64 = 1.0 / 120.0;
    const DC: i32 = 15;

    fn static_box(world: &mut World, center: DVec3, half: DVec3) -> BodyId {
        let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
        id
    }

    fn floor(world: &mut World) -> BodyId {
        static_box(
            world,
            DVec3::new(0.0, -0.5, 0.0),
            DVec3::new(80.0, 0.5, 80.0),
        )
    }

    fn block(world: &mut World, mass: f64, half: DVec3, at: DVec3) -> BodyId {
        let id = world.add(Body::new(mass, Body::box_inertia(mass, half * 2.0), at));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
        id
    }

    fn crate_at(world: &mut World, x: f64, z: f64) -> BodyId {
        block(world, 20.0, DVec3::splat(0.3), DVec3::new(x, 0.3, z))
    }

    fn dummy_at(world: &mut World, x: f64, z: f64) -> BodyId {
        block(
            world,
            75.0,
            DVec3::new(0.25, 0.9, 0.15),
            DVec3::new(x, 0.9, z),
        )
    }

    fn settled() -> World {
        let mut world = World::new(DT);
        floor(&mut world);
        world
    }

    fn rest(world: &mut World) {
        for _ in 0..120 {
            world.step(&physics::Uniform(DVec3::NEG_Y * GRAVITY));
        }
    }

    fn run(world: &mut World, spell: &mut ReverseGravity, seconds: f64) -> Vec<Impact> {
        let mut impacts = Vec::new();
        for _ in 0..ticks(world, seconds) {
            impacts.extend(spell.step(world));
        }
        impacts
    }

    fn cast(world: &mut World) -> ReverseGravity {
        ReverseGravity::cast(world, DVec3::new(-20.0, 0.0, 0.0), DVec3::ZERO).unwrap()
    }

    #[test]
    fn the_cylinder_is_fifty_feet_by_one_hundred_feet() {
        let c = Cylinder::at(DVec3::new(3.0, 1.0, -2.0));
        assert!((c.radius - 15.24).abs() < 1e-12);
        assert!((c.height - 30.48).abs() < 1e-12);
        assert!((c.top() - 31.48).abs() < 1e-12);
        assert!(c.contains(DVec3::new(3.0 + 15.23, 1.0, -2.0)));
        assert!(!c.contains(DVec3::new(3.0 + 15.25, 1.0, -2.0)));
        assert!(c.contains(DVec3::new(3.0, 31.47, -2.0)));
        assert!(!c.contains(DVec3::new(3.0, 31.49, -2.0)));
        assert!(!c.contains(DVec3::new(3.0, 0.99, -2.0)));
        let mut world = World::new(DT);
        assert_eq!(
            ReverseGravity::cast(&mut world, DVec3::ZERO, DVec3::new(30.49, 0.0, 0.0)),
            Err(Refusal::OutOfRange)
        );
        assert!(ReverseGravity::cast(&mut world, DVec3::ZERO, DVec3::new(30.47, 0.0, 0.0)).is_ok());
    }

    #[test]
    fn unanchored_bodies_rise_and_bodies_outside_stay() {
        let mut world = settled();
        let inside = crate_at(&mut world, 5.0, 0.0);
        let dummy = dummy_at(&mut world, -5.0, 3.0);
        let outside = crate_at(&mut world, 16.5, 0.0);
        rest(&mut world);
        let mut spell = cast(&mut world);
        run(&mut world, &mut spell, 1.0);
        assert!(world[inside].pos.y > 4.0, "{}", world[inside].pos.y);
        assert!(world[dummy].pos.y > 4.0, "{}", world[dummy].pos.y);
        assert!((world[outside].pos.y - 0.3).abs() < 0.01);
    }

    #[test]
    fn anchored_and_secured_bodies_stay_down() {
        let mut world = settled();
        let ground = BodyId(0);
        let secured = crate_at(&mut world, 2.0, 0.0);
        let pillar = static_box(
            &mut world,
            DVec3::new(-3.0, 3.0, 0.0),
            DVec3::new(0.5, 3.0, 0.5),
        );
        rest(&mut world);
        let at = world[secured].pos;
        world.add_joint(Joint::weld_here(&world, ground, secured, at));
        let mut spell = cast(&mut world);
        run(&mut world, &mut spell, 3.0);
        assert!(
            world[secured].pos.distance(at) < 0.01,
            "{}",
            world[secured].pos
        );
        assert_eq!(world[pillar].pos, DVec3::new(-3.0, 3.0, 0.0));
    }

    #[test]
    fn a_passed_save_with_a_fixed_object_in_reach_holds_the_creature() {
        let mut world = settled();
        static_box(
            &mut world,
            DVec3::new(0.0, 3.0, 0.0),
            DVec3::new(0.5, 3.0, 0.5),
        );
        let clinging = dummy_at(&mut world, 1.0, 0.0);
        let far = dummy_at(&mut world, 6.0, 0.0);
        let failed = dummy_at(&mut world, -1.0, 0.0);
        rest(&mut world);
        let mut spell = cast(&mut world);
        let held = spell.grab(&mut world, clinging, Save::new(14, 2, DC), &[BodyId(0)]);
        assert!(held.held(), "{held:?}");
        assert_eq!(
            spell.grab(&mut world, far, Save::new(20, 2, DC), &[BodyId(0)]),
            Grab::NothingInReach
        );
        assert_eq!(
            spell.grab(&mut world, failed, Save::new(3, 2, DC), &[BodyId(0)]),
            Grab::Failed
        );
        let start = world[clinging].pos;
        run(&mut world, &mut spell, 4.0);
        assert!(world[clinging].pos.distance(start) < 0.02);
        assert!(spell.held(clinging));
        assert!(world[far].pos.y > 25.0, "{}", world[far].pos.y);
        assert!(world[failed].pos.y > 25.0, "{}", world[failed].pos.y);
    }

    #[test]
    fn bodies_hover_at_the_top_without_overshooting() {
        let mut world = settled();
        let bodies = [
            crate_at(&mut world, 0.0, 0.0),
            dummy_at(&mut world, 3.0, 0.0),
            block(
                &mut world,
                60.0,
                DVec3::new(0.3, 0.45, 0.3),
                DVec3::new(-3.0, 0.45, 2.0),
            ),
        ];
        rest(&mut world);
        world[bodies[0]].vel.x = 1.0;
        let mut spell = cast(&mut world);
        let top = spell.gravity.cylinder.top();
        let mut highest = f64::MIN;
        for _ in 0..ticks(&world, 10.0) {
            spell.step(&mut world);
            for id in bodies {
                highest = highest.max(world[id].pos.y);
            }
        }
        assert!(highest <= top + 0.2, "overshot to {highest}");
        for id in bodies {
            let body = &world[id];
            assert!((body.pos.y - top).abs() < 0.2, "{}", body.pos.y);
            assert!(body.vel.y.abs() < 0.05);
        }
        // Hovering keeps horizontal motion, lightly damped.
        assert!(world[bodies[0]].vel.x > 0.0);
    }

    #[test]
    fn a_ceiling_strike_deals_falling_damage_for_the_upward_distance() {
        let mut world = settled();
        let crate_ = crate_at(&mut world, 0.0, 0.0);
        let dummy = dummy_at(&mut world, 2.0, 0.0);
        // The ceiling's underside sits 40 feet above the dummy's head.
        let underside = 1.8 + 40.0 * FOOT + 0.01;
        static_box(
            &mut world,
            DVec3::new(0.0, underside + 0.5, 0.0),
            DVec3::new(20.0, 0.5, 20.0),
        );
        rest(&mut world);
        let mut spell = cast(&mut world);
        let impacts = run(&mut world, &mut spell, 5.0);
        let first = |id| impacts.iter().find(|i| i.body == id).unwrap().strike;
        let hit = first(dummy);
        assert!(hit.upward);
        assert!((hit.span - (underside - 1.8)).abs() < 0.05, "{}", hit.span);
        assert_eq!(hit.dice, 4);
        let hit = first(crate_);
        assert!(hit.upward);
        assert_eq!(hit.dice, falling_dice(underside - 0.6));
        // Pressed against the ceiling afterward.
        assert!((world[dummy].pos.y - (underside - 0.9)).abs() < 0.05);
        assert!((world[crate_].pos.y - (underside - 0.3)).abs() < 0.05);
    }

    #[test]
    fn leaving_the_cylinder_restores_normal_gravity() {
        let mut world = settled();
        let crate_ = crate_at(&mut world, 13.0, 0.0);
        rest(&mut world);
        let mut spell = cast(&mut world);
        run(&mut world, &mut spell, 6.0);
        let top = spell.gravity.cylinder.top();
        assert!((world[crate_].pos.y - top).abs() < 0.2);
        // A lateral shove carries it over the edge.
        world[crate_].vel.x = 4.0;
        let impacts = run(&mut world, &mut spell, 5.0);
        let crate_body = &world[crate_];
        assert!(spell.gravity.cylinder.axis_distance(crate_body.pos) > RADIUS);
        assert!(
            (crate_body.pos.y - 0.3).abs() < 0.05,
            "{}",
            crate_body.pos.y
        );
        let landing = impacts.iter().find(|i| i.body == crate_).unwrap().strike;
        assert!(!landing.upward);
        assert_eq!(landing.dice, 9);
        assert!(spell.active());
    }

    #[test]
    fn the_end_drops_everything_with_falling_damage() {
        let mut world = settled();
        let crates = [
            crate_at(&mut world, 0.0, 0.0),
            crate_at(&mut world, 4.0, 4.0),
        ];
        let dummy = dummy_at(&mut world, -4.0, 0.0);
        rest(&mut world);
        let mut spell = cast(&mut world);
        assert!(run(&mut world, &mut spell, 8.0).is_empty());
        // Losing concentration ends it early.
        spell.end(&mut world);
        let impacts = run(&mut world, &mut spell, 4.0);
        let top = spell.gravity.cylinder.top();
        for id in crates {
            let strike = impacts.iter().find(|i| i.body == id).unwrap().strike;
            assert!(!strike.upward);
            assert!((strike.span - (top - 0.3)).abs() < 0.3, "{}", strike.span);
            assert_eq!(strike.dice, 9);
            assert!((world[id].pos.y - 0.3).abs() < 0.05);
        }
        let strike = impacts.iter().find(|i| i.body == dummy).unwrap().strike;
        assert_eq!(strike.dice, falling_dice(top - 0.9));
    }

    #[test]
    fn the_spell_ends_after_one_minute() {
        let mut world = settled();
        let crate_ = crate_at(&mut world, 0.0, 0.0);
        rest(&mut world);
        let mut spell = cast(&mut world);
        run(&mut world, &mut spell, DURATION - 0.5);
        assert!(spell.active());
        run(&mut world, &mut spell, 8.0);
        assert!(!spell.active());
        assert!((world[crate_].pos.y - 0.3).abs() < 0.05);
    }

    #[test]
    fn a_creature_hovering_at_the_top_falls_one_hundred_feet_for_ten_dice() {
        // A character is not a rigid body: integrate its feet directly.
        let gravity = Gravity {
            cylinder: Cylinder::at(DVec3::ZERO),
            active: true,
        };
        let (mut feet, mut speed) = (DVec3::ZERO, 0.0);
        let mut fall = Fall::new(0.0);
        let mut highest = 0.0_f64;
        for _ in 0..1200 {
            let before = speed;
            speed += creature_accel(&gravity, feet, speed) * DT;
            feet.y += speed * DT;
            highest = highest.max(feet.y);
            assert_eq!(fall.observe(feet, before, speed, None), None);
        }
        assert!(highest <= HEIGHT + 0.2);
        assert!((feet.y - HEIGHT).abs() < 0.2);
        let ended = Gravity {
            active: false,
            ..gravity
        };
        let mut strike = None;
        for _ in 0..600 {
            let before = speed;
            speed += creature_accel(&ended, feet, speed) * DT;
            feet.y += speed * DT;
            let landed = feet.y <= 0.0;
            if landed {
                feet.y = 0.0;
                speed = 0.0;
            }
            if let Some(s) = fall.observe(feet, before, speed, landed.then_some(0.0)) {
                strike = Some(s);
                break;
            }
        }
        let strike = strike.unwrap();
        assert!(!strike.upward);
        assert_eq!(strike.dice, 10);
        assert_eq!(falling_dice(HEIGHT), 10);
    }

    #[test]
    fn a_checkpoint_mid_rise_replays_identically() {
        let mut world = settled();
        for i in 0..4 {
            crate_at(&mut world, f64::from(i) * 1.5 - 2.0, 1.0);
        }
        let held = dummy_at(&mut world, 0.0, -3.0);
        static_box(
            &mut world,
            DVec3::new(0.0, 3.0, -4.0),
            DVec3::new(0.5, 3.0, 0.5),
        );
        static_box(
            &mut world,
            DVec3::new(6.0, 12.5, 0.0),
            DVec3::new(2.0, 0.5, 2.0),
        );
        let struck = crate_at(&mut world, 6.0, 0.0);
        rest(&mut world);
        let mut spell = cast(&mut world);
        spell.grab(&mut world, held, Save::new(18, 2, DC), &[BodyId(0)]);
        run(&mut world, &mut spell, 0.8);
        let json = serde_json::to_string(&(&world, &spell)).unwrap();
        let (mut world_b, mut spell_b): (World, ReverseGravity) =
            serde_json::from_str(&json).unwrap();
        assert_eq!(world_b, world);
        assert_eq!(spell_b, spell);
        let (mut a, mut b) = (Trace::default(), Trace::default());
        let (mut ia, mut ib) = (Vec::new(), Vec::new());
        for step in 0..ticks(&world, 6.0) {
            if step == 400 {
                spell.end(&mut world);
                spell_b.end(&mut world_b);
            }
            ia.extend(spell.step(&mut world));
            ib.extend(spell_b.step(&mut world_b));
            a.record(&world);
            b.record(&world_b);
        }
        a.compare(&b, Tolerance::EXACT).unwrap();
        assert_eq!(ia, ib);
        assert!(ia.iter().any(|i| i.body == struck && i.strike.upward));
    }
}
