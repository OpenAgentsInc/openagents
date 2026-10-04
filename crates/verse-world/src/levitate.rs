//! Levitate (SRD 5.2.1) as a per-body gravity override with altitude hold.
//!
//! Level 2 Transmutation. Casting time: Action. Range: 60 feet. Duration:
//! Concentration, up to 10 minutes. One creature or loose object of up to
//! 500 pounds rises vertically up to 20 feet and stays suspended. An
//! unwilling creature that succeeds on a Constitution saving throw is
//! unaffected. The target moves only by pushing or pulling against a fixed
//! object or surface within reach, as if climbing. Its altitude changes by
//! up to 20 feet in either direction on the caster's turn. When the spell
//! ends, the target floats gently to the ground.
//!
//! The SRD outcome decides whether the spell takes hold; physics decides
//! what happens next. Gravity on the target is cancelled by a hold force
//! that also tracks the commanded altitude, horizontal motion has only very
//! light damping so momentum persists, and the only self-propulsion is a
//! push-off against a surface in reach. Every force and impulse this module
//! applies is recorded in a [`Ledger`] under a named term, so a test can
//! show that nothing else moves the target.

use glam::DVec3;
use physics::{BodyId, BodyKind, Collider, ColliderId, Filter, Ledger, World};
use serde::{Deserialize, Serialize};

pub use crate::spells::{FEET, GRAVITY, SPELL_SAVE_DC};

/// One pound, kg.
pub const POUNDS: f64 = 0.453_592_37;
/// SRD range: 60 feet. The target must stay this close to the caster.
pub const RANGE: f64 = 60.0 * FEET;
/// SRD rise: up to 20 feet above the ground under the target at cast time.
pub const MAX_RISE: f64 = 20.0 * FEET;
/// SRD altitude change per turn: up to 20 feet in either direction.
pub const ALTITUDE_STEP: f64 = 20.0 * FEET;
/// One round, s. Altitude commands for another target come once per turn.
pub const TURN: f64 = 6.0;
/// SRD duration: concentration, up to 10 minutes, s.
pub const DURATION: f64 = 600.0;
/// SRD weight limit for an object: 500 pounds, kg.
pub const WEIGHT_LIMIT: f64 = 500.0 * POUNDS;
/// Fastest the hold moves the target vertically, m/s.
pub const RISE_SPEED: f64 = 1.5;
/// Horizontal damping while levitated, 1/s. Light, so momentum persists.
pub const DRIFT_DAMPING: f64 = 0.05;
/// SRD reach for a push-off: 5 feet from the target's surface.
pub const REACH: f64 = 5.0 * FEET;
/// The chamber's forward walking speed (`play.rs`), m/s.
pub const WALK_SPEED: f64 = 6.4008;
/// SRD climbing speed: half the walking speed, m/s.
pub const CLIMB_SPEED: f64 = WALK_SPEED / 2.0;
/// Feather Fall's descent rate, 60 feet per round, m/s. The end of
/// Levitate lowers its target no faster than this.
pub const FEATHER_FALL_SPEED: f64 = 10.0 * FEET;

/// Altitude error to commanded vertical speed, 1/s.
const HOLD_GAIN: f64 = 1.5;
/// Vertical speed error to control acceleration, 1/s.
const HOLD_RESPONSE: f64 = 6.0;
/// Largest control acceleration beyond cancelling gravity, m/s^2.
const HOLD_ACCEL: f64 = 4.0;

/// Ledger terms this module writes.
pub mod terms {
    /// The hold force: cancelled gravity plus the altitude controller.
    pub const HOLD: &str = "levitate hold";
    /// Horizontal drift damping.
    pub const DAMPING: &str = "levitate damping";
    /// A push-off against a fixed surface (the surface's reaction).
    pub const PUSH_OFF: &str = "levitate push-off";
    /// The capped descent after the spell ends.
    pub const DESCENT: &str = "levitate descent";
    /// Uniform gravity on the target.
    pub const GRAVITY: &str = "gravity";
}

/// What the spell is cast on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Subject {
    /// A creature. Creatures are not weight-limited; an unwilling one saves.
    Creature {
        willing: bool,
        /// Constitution modifier from its SRD stat block.
        constitution: i32,
    },
    /// An object. It must be loose (not secured) and at most 500 pounds.
    Object { mass: f64, secured: bool },
}

/// One deterministic Constitution save.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    pub roll: i32,
    pub modifier: i32,
    pub dc: i32,
    pub success: bool,
}

/// Why a cast does not take hold.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Refusal {
    /// The target is farther than 60 feet, m.
    OutOfRange { distance: f64 },
    /// The object weighs more than 500 pounds, kg.
    TooHeavy { mass: f64 },
    /// The object is secured, not loose.
    Secured,
    /// The unwilling creature made its save.
    Saved(Save),
}

impl Refusal {
    /// Overlay text for the refusal.
    #[must_use]
    pub fn reason(&self) -> String {
        match self {
            Self::OutOfRange { distance } => {
                format!("out of range: {:.0} ft > 60 ft", distance / FEET)
            }
            Self::TooHeavy { mass } => {
                format!("too heavy: {:.0} lb ({mass:.0} kg) > 500 lb", mass / POUNDS)
            }
            Self::Secured => "not a loose object".into(),
            Self::Saved(save) => format!(
                "CON save {} + {} = {} vs DC {}: unaffected",
                save.roll,
                save.modifier,
                save.roll + save.modifier,
                save.dc
            ),
        }
    }
}

/// Admit a cast. `roll` is called for an unwilling creature only, and must
/// return a d20 from the simulation's seeded dice. Returns the save made,
/// if any, when the spell takes hold.
///
/// # Errors
///
/// Returns the [`Refusal`] when the target is out of range, too heavy,
/// secured, or saves.
pub fn admit(
    subject: Subject,
    distance: f64,
    roll: impl FnOnce() -> i32,
) -> Result<Option<Save>, Refusal> {
    if !distance.is_finite() || distance > RANGE {
        return Err(Refusal::OutOfRange { distance });
    }
    match subject {
        Subject::Object { secured: true, .. } => Err(Refusal::Secured),
        Subject::Object { mass, .. } if !(mass <= WEIGHT_LIMIT) => Err(Refusal::TooHeavy { mass }),
        Subject::Object { .. } | Subject::Creature { willing: true, .. } => Ok(None),
        Subject::Creature {
            willing: false,
            constitution,
        } => {
            let roll = roll();
            let save = Save {
                roll,
                modifier: constitution,
                dc: SPELL_SAVE_DC,
                success: roll + constitution >= SPELL_SAVE_DC,
            };
            if save.success {
                Err(Refusal::Saved(save))
            } else {
                Ok(Some(save))
            }
        }
    }
}

/// Why a levitation ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum End {
    /// The caster's concentration ended (another concentration spell,
    /// death, or dismissal).
    Concentration,
    /// Ten minutes passed.
    Duration,
    /// The target left the 60-foot range.
    OutOfRange,
    /// The target died or left the world.
    TargetGone,
}

/// Where a levitation is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Suspended with gravity off and the altitude held.
    Holding,
    /// Gravity is back, the descent is capped at Feather Fall speed, and
    /// landing deals no falling damage.
    Descending(End),
    /// Landed; the spell no longer touches the target.
    Done(End),
}

/// Why an altitude command was refused.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AltitudeRefusal {
    /// More than 20 feet in one command.
    TooFar,
    /// Another command this turn (another target only), s until the next.
    ThisTurn { wait: f64 },
    /// The spell no longer holds the target.
    NotHolding,
}

/// One levitated target's spell state. Plain data, so it checkpoints with
/// the world and replays exactly.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Levitation {
    /// The target's reference height when resting on the ground under it at
    /// cast time, m: a body's center of mass, or a character's feet.
    pub base: f64,
    /// Commanded height above `base`, 0 to [`MAX_RISE`], m.
    pub rise: f64,
    /// The caster is the target, so altitude changes are free.
    pub self_target: bool,
    /// When the spell was cast, s.
    pub cast_at: f64,
    /// When another target's altitude last changed, s.
    pub commanded_at: Option<f64>,
    pub phase: Phase,
}

impl Levitation {
    /// A new levitation lifting its target `rise` (clamped to 20 feet)
    /// above `base`.
    #[must_use]
    pub fn new(base: f64, rise: f64, self_target: bool, now: f64) -> Self {
        Self {
            base,
            rise: rise.clamp(0.0, MAX_RISE),
            self_target,
            cast_at: now,
            commanded_at: None,
            phase: Phase::Holding,
        }
    }

    /// The commanded reference height, m.
    #[must_use]
    pub fn target_height(&self) -> f64 {
        self.base + self.rise
    }

    #[must_use]
    pub fn holding(&self) -> bool {
        self.phase == Phase::Holding
    }

    /// Whether landing from this descent is exempt from falling damage.
    #[must_use]
    pub fn gentle(&self) -> bool {
        matches!(self.phase, Phase::Descending(_) | Phase::Done(_))
    }

    /// Change the commanded altitude by `delta` meters, within 20 feet per
    /// command and the 0 to 20 feet band above the cast-time ground. For
    /// another target, once per 6-second turn.
    ///
    /// # Errors
    ///
    /// Returns an [`AltitudeRefusal`] and changes nothing when refused.
    pub fn command(&mut self, delta: f64, now: f64) -> Result<f64, AltitudeRefusal> {
        if !self.holding() {
            return Err(AltitudeRefusal::NotHolding);
        }
        if !delta.is_finite() || delta.abs() > ALTITUDE_STEP + 1e-9 {
            return Err(AltitudeRefusal::TooFar);
        }
        if !self.self_target
            && let Some(at) = self.commanded_at
            && now - at < TURN
        {
            return Err(AltitudeRefusal::ThisTurn {
                wait: TURN - (now - at),
            });
        }
        self.rise = (self.rise + delta).clamp(0.0, MAX_RISE);
        if !self.self_target {
            self.commanded_at = Some(now);
        }
        Ok(self.rise)
    }

    /// End the spell: gravity returns and the target floats down.
    pub fn end(&mut self, why: End) {
        if self.holding() {
            self.phase = Phase::Descending(why);
        }
    }

    /// Apply the duration and range limits. `caster_distance` is from the
    /// caster to the target, m.
    pub fn update(&mut self, now: f64, caster_distance: f64) {
        if !self.holding() {
            return;
        }
        if now - self.cast_at >= DURATION {
            self.end(End::Duration);
        } else if !(caster_distance <= RANGE) {
            self.end(End::OutOfRange);
        }
    }

    /// Mark a descending target as landed.
    pub fn land(&mut self) {
        if let Phase::Descending(why) = self.phase {
            self.phase = Phase::Done(why);
        }
    }

    /// The vertical speed the hold commands at height `y`, m/s: toward the
    /// commanded height, no faster than [`RISE_SPEED`].
    #[must_use]
    pub fn hold_speed(&self, y: f64) -> f64 {
        (HOLD_GAIN * (self.target_height() - y)).clamp(-RISE_SPEED, RISE_SPEED)
    }

    /// Vertical acceleration the spell adds to a target at height `y` with
    /// vertical speed `vy` under gravity `g` (positive, m/s^2), m/s^2.
    /// Holding, it cancels gravity and tracks the commanded height at no
    /// more than [`RISE_SPEED`]. Descending, it only stops the next step
    /// from falling faster than [`FEATHER_FALL_SPEED`].
    #[must_use]
    pub fn vertical_accel(&self, y: f64, vy: f64, g: f64, dt: f64) -> f64 {
        match self.phase {
            Phase::Holding => {
                g + (HOLD_RESPONSE * (self.hold_speed(y) - vy)).clamp(-HOLD_ACCEL, HOLD_ACCEL)
            }
            Phase::Descending(_) => {
                let next = vy - g * dt;
                if next < -FEATHER_FALL_SPEED {
                    (-FEATHER_FALL_SPEED - next) / dt
                } else {
                    0.0
                }
            }
            Phase::Done(_) => 0.0,
        }
    }

    /// Horizontal acceleration the spell adds: light damping while held.
    #[must_use]
    pub fn horizontal_accel(&self, vel: DVec3) -> DVec3 {
        if self.holding() {
            -DVec3::new(vel.x, 0.0, vel.z) * DRIFT_DAMPING
        } else {
            DVec3::ZERO
        }
    }
}

/// A surface within reach of a levitated target.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    /// Nearest point on the surface, m.
    pub point: DVec3,
    /// Unit normal from the surface toward the target.
    pub normal: DVec3,
    /// Gap between the target's surface and this one, m.
    pub gap: f64,
    /// The body the surface belongs to.
    pub body: BodyId,
    pub collider: ColliderId,
}

/// The velocity change a push-off gives a levitated target with velocity
/// `vel` for movement input `input` (horizontal, length at most 1). Without
/// a surface in reach, input does nothing. With one, the target pushes
/// away from it or pulls along it (input into the surface keeps only its
/// component along the surface), up to [`CLIMB_SPEED`] in the input
/// direction. The push never slows the target.
#[must_use]
pub fn push_off(input: DVec3, vel: DVec3, surface: Option<&Surface>) -> DVec3 {
    let Some(surface) = surface else {
        return DVec3::ZERO;
    };
    let input = DVec3::new(input.x, 0.0, input.z);
    let strength = input.length().min(1.0);
    if strength < 1e-6 || surface.gap > REACH {
        return DVec3::ZERO;
    }
    let normal = surface.normal;
    let mut direction = input / input.length();
    let into = direction.dot(normal);
    if into < 0.0 {
        direction -= normal * into;
    }
    let Some(direction) = direction.try_normalize() else {
        return DVec3::ZERO;
    };
    let along = vel.dot(direction);
    direction * (CLIMB_SPEED * strength - along).max(0.0)
}

/// The nearest surface within `reach` of body `id`'s colliders, among
/// colliders on other bodies that `accept` allows. Distances come from two
/// rounds of alternating closest points, which is exact for the convex
/// pairs a wall, pillar, or floor makes with a capsule or box.
#[must_use]
pub fn surface_in_reach(
    world: &World,
    id: BodyId,
    reach: f64,
    accept: &dyn Fn(BodyId) -> bool,
) -> Option<Surface> {
    let center = world[id].pos;
    let own: Vec<&Collider> = world.colliders().iter().filter(|c| c.body == id).collect();
    let mut best: Option<Surface> = None;
    for (index, other) in world.colliders().iter().enumerate() {
        if other.body == id || other.filter == Filter::NONE || !accept(other.body) {
            continue;
        }
        for mine in &own {
            let mut p = other.closest_point(world, center);
            let mut q = mine.closest_point(world, p);
            for _ in 0..2 {
                p = other.closest_point(world, q);
                q = mine.closest_point(world, p);
            }
            let gap = p.distance(q);
            let normal = (q - p)
                .try_normalize()
                .or_else(|| (center - p).try_normalize())
                .unwrap_or(DVec3::Y);
            let surface = Surface {
                point: p,
                normal,
                gap,
                body: other.body,
                collider: ColliderId(index as u32),
            };
            if gap <= reach && best.is_none_or(|b| gap < b.gap) {
                best = Some(surface);
            }
        }
    }
    best
}

/// The surface in reach that movement input pushes or pulls against: a
/// wall, a pillar, a ceiling, a ledge top, or the floor under a lowered
/// target, which it can only crawl along at climbing speed.
#[must_use]
pub fn push_surface(world: &World, id: BodyId, accept: &dyn Fn(BodyId) -> bool) -> Option<Surface> {
    surface_in_reach(world, id, REACH, &|b| accept(b) && !world[b].removed)
}

/// The nearest surface within `reach` of an upright capsule from `a` to `b`
/// with `radius` (a character), among colliders on bodies `accept` allows.
#[must_use]
pub fn capsule_surface(
    world: &World,
    a: DVec3,
    b: DVec3,
    radius: f64,
    reach: f64,
    accept: &dyn Fn(BodyId) -> bool,
) -> Option<Surface> {
    let axis = b - a;
    let segment = |q: DVec3| {
        let t = if axis.length_squared() > 0.0 {
            ((q - a).dot(axis) / axis.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        a + axis * t
    };
    let middle = (a + b) * 0.5;
    let extent = axis.length() * 0.5 + radius + reach;
    let mut best: Option<Surface> = None;
    for (index, collider) in world.colliders().iter().enumerate() {
        if collider.filter == Filter::NONE
            || world[collider.body].removed
            || !accept(collider.body)
            || collider.pose(world).0.distance(middle) > collider.shape.bound() + extent
        {
            continue;
        }
        let mut p = collider.closest_point(world, middle);
        let mut q = segment(p);
        for _ in 0..3 {
            p = collider.closest_point(world, q);
            q = segment(p);
        }
        let gap = (p.distance(q) - radius).max(0.0);
        if gap > reach || best.is_some_and(|s| s.gap <= gap) {
            continue;
        }
        let normal = (q - p)
            .try_normalize()
            .or_else(|| (middle - collider.pose(world).0).try_normalize())
            .unwrap_or(DVec3::Y);
        best = Some(Surface {
            point: p,
            normal,
            gap,
            body: collider.body,
            collider: ColliderId(index as u32),
        });
    }
    best
}

/// The height of the ground under body `id`, m, by a ray straight down
/// from its lowest point.
#[must_use]
pub fn ground_under(world: &World, id: BodyId) -> Option<f64> {
    let bottom = lowest_point(world, id)?;
    world
        .raycast(bottom + DVec3::Y * 1e-3, -DVec3::Y, 1000.0, &|c| {
            c.body != id
        })
        .map(|hit| hit.point.y)
}

/// The resting reference height for body `id`: its center when its lowest
/// point sits on the ground under it, m.
#[must_use]
pub fn resting_height(world: &World, id: BodyId) -> Option<f64> {
    let bottom = lowest_point(world, id)?;
    Some(ground_under(world, id)? + (world[id].pos.y - bottom.y))
}

fn lowest_point(world: &World, id: BodyId) -> Option<DVec3> {
    let far = world[id].pos - DVec3::Y * 1e4;
    world
        .colliders()
        .iter()
        .filter(|c| c.body == id)
        .map(|c| c.closest_point(world, far))
        .min_by(|a, b| a.y.total_cmp(&b.y))
}

impl Levitation {
    /// Apply this step's spell forces to body `id` before the world steps
    /// under uniform gravity `g`, recording each in `ledger`.
    pub fn drive(&self, world: &mut World, id: BodyId, g: f64, ledger: &mut Ledger) {
        let dt = world.dt;
        let body = &world[id];
        let (mass, pos, vel) = (body.mass, body.pos, body.vel);
        let vertical = DVec3::Y * (mass * self.vertical_accel(pos.y, vel.y, g, dt));
        let damping = self.horizontal_accel(vel) * mass;
        if vertical != DVec3::ZERO {
            world[id].apply_force(vertical);
            let term = if self.holding() {
                terms::HOLD
            } else {
                terms::DESCENT
            };
            ledger.add_impulse(term, vertical * dt, pos);
        }
        if damping != DVec3::ZERO {
            world[id].apply_force(damping);
            ledger.add_impulse(terms::DAMPING, damping * dt, pos);
        }
    }

    /// Apply movement `input` to a held body `id` through a push-off
    /// against a surface in reach, if any. The surface takes the reaction when
    /// it is dynamic; a fixed surface's reaction is recorded in `ledger`.
    /// Returns the velocity change.
    pub fn steer(
        &self,
        world: &mut World,
        id: BodyId,
        input: DVec3,
        accept: &dyn Fn(BodyId) -> bool,
        ledger: &mut Ledger,
    ) -> DVec3 {
        if !self.holding() {
            return DVec3::ZERO;
        }
        let surface = push_surface(world, id, accept);
        let dv = push_off(input, world[id].vel, surface.as_ref());
        if dv == DVec3::ZERO {
            return dv;
        }
        let Some(surface) = surface else {
            return DVec3::ZERO;
        };
        let impulse = dv * world[id].mass;
        let at = world[id].pos;
        world[id].apply_impulse_at(impulse, at);
        if world[surface.body].kind == BodyKind::Dynamic && !world[surface.body].removed {
            world[surface.body].apply_impulse_at(-impulse, surface.point);
        } else {
            ledger.add_impulse(terms::PUSH_OFF, impulse, at);
        }
        dv
    }
}

#[cfg(test)]
mod tests;
