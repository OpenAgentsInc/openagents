//! Telekinesis (SRD 5.2.1) as a physics grip.
//!
//! Level 5 Transmutation. Casting time: Action. Range: 60 feet. Components:
//! V, S. Duration: Concentration, up to 10 minutes. Each application moves
//! one creature or object you can see within range up to 30 feet in any
//! direction; a creature makes a Strength saving throw and, on a failure,
//! is Restrained and suspended until the end of your next turn.
//!
//! An object grip is two soft joints between a kinematic hand and the
//! target's center of mass: a 4 Hz critically damped point spring that
//! carries it, and a 2 Hz angular spring that keeps its orientation at grab
//! time. The target keeps its own mass, inertia, and colliders, so it still
//! strikes and rests on everything else. Size, not mass, sets the force
//! limit, as the SRD sets no weight limit. Releasing removes the joints and
//! leaves the target's velocity alone, so a throw is the hand's motion at
//! release. A creature is a kinematic character, so the same 4 Hz spring
//! drives its velocity toward the hand while its gravity is suspended.
//!
//! In play, the caster steers the hand while the application still has hand
//! path left: forward and back move it along the facing, strafing right and
//! left raises and lowers it, and turning swings it around the caster. Once
//! the 30-foot budget is spent the hand freezes and the caster walks again.
//! Jumping lets go. Re-casting on the held target renews the budget.
mod game;
mod scenario;
#[cfg(test)]
mod tests;

pub(crate) use game::cast;
pub(crate) use game::{after_step, before_step};
pub use game::{let_go, steer_input};
pub use scenario::scenario;

use crate::spells::FEET;
use glam::{DQuat, DVec3};
use physics::{Body, BodyId, BodyKind, Joint, JointId, JointKind, Ledger, Momentum, World};
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Telekinesis";
/// Spell range: 60 feet.
pub const RANGE: f64 = 60. * FEET;
/// Hand path allowed by one application: 30 feet.
pub const MOVE_BUDGET: f64 = 30. * FEET;
/// Fastest the hand moves, m/s.
pub const HAND_SPEED: f64 = 6.;
/// One round, s. "Until the end of your next turn" lasts one round.
pub const ROUND: f64 = 6.;
/// Concentration, up to 10 minutes, s.
pub const CONCENTRATION: f64 = 600.;
/// Spell level.
pub const LEVEL: u8 = 5;
/// Natural frequency of the linear grip spring, Hz.
pub const LINEAR_HZ: f64 = 4.;
/// Natural frequency of the angular grip spring, Hz.
pub const ANGULAR_HZ: f64 = 2.;
/// Both grip springs are critically damped.
pub const DAMPING_RATIO: f64 = 1.;
/// Acceleration the grip's force limit allows for its size category's
/// reference mass, m/s²: twice standard gravity.
pub const HOLD_ACCELERATION: f64 = 2. * crate::spells::GRAVITY;
/// Lever arm that turns the force limit into a torque limit, m.
pub const TORQUE_ARM: f64 = 1.;
/// Fastest a gripped creature moves, m/s.
pub const CREATURE_SPEED: f64 = 2. * HAND_SPEED;
/// Ledger term for every impulse the grip puts into a body.
pub const LEDGER_TERM: &str = "telekinesis";
/// The overlay's SRD line.
pub const SRD_LINE: &str = "Level 5 Transmutation | Range 60 ft | Concentration, up to 10 min | \
    STR save (creatures) | move up to 30 ft";

/// The row-two action-bar entry: slot 0, Shift+1.
pub const DEF: crate::spells::SpellDef = crate::spells::SpellDef {
    slot: 0,
    key: "telekinesis",
    label: NAME,
    icon: "telekinesis-icon",
    description: "Grip a creature or object within 60 ft; move keys steer, jump lets go",
    // MMO tuning: re-applying is the SRD's Magic action on each later turn;
    // the chamber allows it every half second.
    cost: 1,
    cooldown: 0.5,
    cast,
};

/// SRD size categories, including the one the spell refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Size {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gargantuan,
}

impl From<crate::spells::Size> for Size {
    fn from(size: crate::spells::Size) -> Self {
        match size {
            crate::spells::Size::Tiny => Self::Tiny,
            crate::spells::Size::Small => Self::Small,
            crate::spells::Size::Medium => Self::Medium,
            crate::spells::Size::Large => Self::Large,
            crate::spells::Size::Huge => Self::Huge,
        }
    }
}

impl Size {
    /// Mass the grip is sized for, kg; `None` for a target too large to
    /// affect. A grip holds anything up to this mass against twice gravity.
    #[must_use]
    pub fn reference_mass(self) -> Option<f64> {
        match self {
            Self::Tiny => Some(10.),
            Self::Small => Some(300.),
            Self::Medium => Some(1_500.),
            Self::Large => Some(6_000.),
            Self::Huge => Some(24_000.),
            Self::Gargantuan => None,
        }
    }

    /// The linear grip's force limit, N.
    #[must_use]
    pub fn force_limit(self) -> Option<f64> {
        self.reference_mass().map(|m| m * HOLD_ACCELERATION)
    }
}

/// What a body grip affects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// A creature that is a rigid body, with its Strength modifier.
    Creature { strength: i32 },
    /// An object nobody wears or carries: it moves automatically.
    Object,
    /// A worn or carried object, with its bearer's Strength modifier. On a
    /// failed save the caller detaches it into a dynamic body and applies
    /// the spell to that body as an [`Target::Object`].
    Carried { bearer_strength: i32 },
}

impl Target {
    /// The Strength modifier that saves, if this target saves at all.
    #[must_use]
    pub fn save_modifier(self) -> Option<i32> {
        match self {
            Self::Creature { strength } => Some(strength),
            Self::Object => None,
            Self::Carried { bearer_strength } => Some(bearer_strength),
        }
    }
}

/// Why an application was refused before any effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The target is beyond 60 feet.
    OutOfRange,
    /// The target is larger than Huge.
    TooLarge,
    /// Concentration has ended.
    Ended,
    /// The body cannot be moved.
    Immovable,
}

/// What an admitted application did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Applied {
    /// The target is gripped with a fresh 30-foot budget.
    Gripped,
    /// The target made its save; nothing moves and the action is spent.
    Resisted,
}

/// Why a grip ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    /// The caster chose another target.
    Switched,
    /// The target left the spell's range.
    OutOfRange,
    /// A creature's hold expired without a successful re-application.
    HoldExpired,
    /// Concentration ended.
    Ended,
    /// The caster let go.
    Let,
}

impl Reason {
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Switched => "switched targets",
            Self::OutOfRange => "beyond 60 ft",
            Self::HoldExpired => "hold expired after one round",
            Self::Ended => "concentration ended",
            Self::Let => "let go",
        }
    }
}

/// A grip that ended. A body kept its velocity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    /// The rigid body released, if it was one.
    pub body: Option<BodyId>,
    /// The scene actor released, if it was a character.
    pub actor: Option<u64>,
    pub reason: Reason,
    /// A body's velocity at release, m/s.
    pub velocity: DVec3,
    /// A body's linear momentum at release, kg m/s.
    pub momentum: DVec3,
    /// Whether it was a creature, which takes falling damage when it lands.
    pub creature: bool,
}

/// A rigid body held by the hand's joints.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grip {
    pub body: BodyId,
    pub target: Target,
    pub size: Size,
    pub linear: JointId,
    pub angular: JointId,
    /// Hand path used by this application, m.
    pub path: f64,
    /// Tick at which a creature's hold expires.
    pub hold_until: Option<u64>,
}

impl Grip {
    /// Whether the target is a Restrained creature with gravity suspended.
    #[must_use]
    pub fn restrained(&self) -> bool {
        self.hold_until.is_some()
    }
}

/// A character creature held by the hand: Restrained, its gravity
/// suspended, its velocity driven toward the hand.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreatureHold {
    pub actor: u64,
    /// Hand path used by this application, m.
    pub path: f64,
    /// Tick at which the hold expires.
    pub hold_until: u64,
}

/// What the hand holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Body(BodyId),
    Actor(u64),
}

/// One caster's concentration on Telekinesis: its kinematic hand and at
/// most one held target. Serializes with the spell world, so a checkpoint
/// taken mid-grip continues exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Telekinesis {
    /// The hand body: kinematic, massless, without colliders.
    pub hand: BodyId,
    /// The world's gravity, which a gripped creature does not feel.
    pub gravity: DVec3,
    /// Tick when concentration ends.
    pub ends: u64,
    pub grip: Option<Grip>,
    #[serde(default)]
    pub creature: Option<CreatureHold>,
    /// The cast that holds the caster's concentration.
    #[serde(default)]
    pub cast: u64,
    /// The caster's scene actor.
    #[serde(default)]
    pub caster: u64,
    /// The caster's feet and facing, refreshed every tick, m and rad.
    #[serde(default)]
    pub origin: DVec3,
    #[serde(default)]
    pub yaw: f64,
    /// Where the caster steers the hand: horizontal distance along the
    /// facing and height above the caster's feet, m.
    #[serde(default)]
    pub reach: f64,
    #[serde(default)]
    pub lift: f64,
    /// The actor whose gravity this spell suspended.
    #[serde(default)]
    pub suspended: Option<u64>,
    /// Releases since the game last reported them.
    #[serde(default)]
    pub released: Vec<Release>,
    /// Whether the overlay has reported the current budget as spent.
    #[serde(default)]
    pub spent_reported: bool,
}

/// Whole steps in `seconds` at the world's step length.
#[must_use]
pub fn ticks(world: &World, seconds: f64) -> u64 {
    (seconds / world.dt).round() as u64
}

/// Height of a character's center above its feet, m.
pub const CENTER: f64 = crate::spells::CHARACTER_HEIGHT * 0.5;

impl Telekinesis {
    /// Start concentrating: add the hand at `at`. Nothing is held until an
    /// application.
    pub fn cast(world: &mut World, at: DVec3, gravity: DVec3) -> Self {
        let hand = world.add(Body::new(0., DVec3::ONE, at).with_kind(BodyKind::Kinematic));
        Self {
            hand,
            gravity,
            ends: world.tick + ticks(world, CONCENTRATION),
            grip: None,
            creature: None,
            cast: 0,
            caster: 0,
            origin: DVec3::ZERO,
            yaw: 0.,
            reach: 0.,
            lift: 0.,
            suspended: None,
            released: vec![],
            spent_reported: false,
        }
    }

    /// Hand position, m.
    #[must_use]
    pub fn hand_position(&self, world: &World) -> DVec3 {
        world[self.hand].pos
    }

    /// What the hand holds.
    #[must_use]
    pub fn held(&self) -> Option<Held> {
        self.grip
            .map(|g| Held::Body(g.body))
            .or(self.creature.map(|c| Held::Actor(c.actor)))
    }

    /// Hand path the current application still allows, m.
    #[must_use]
    pub fn budget(&self) -> f64 {
        let path = self
            .grip
            .map(|g| g.path)
            .or(self.creature.map(|c| c.path))
            .unwrap_or(MOVE_BUDGET);
        (MOVE_BUDGET - path).max(0.)
    }

    /// Whether the hand holds something and can still move.
    #[must_use]
    pub fn steering(&self) -> bool {
        self.held().is_some() && self.budget() > 1e-9
    }

    /// Whether concentration has ended and the hand is gone.
    #[must_use]
    pub fn ended(&self, world: &World) -> bool {
        world[self.hand].removed
    }

    /// The caster's center, m: the point range is measured from.
    #[must_use]
    pub fn center(&self) -> DVec3 {
        self.origin + DVec3::Y * CENTER
    }

    /// Where the caster steers the hand.
    #[must_use]
    pub fn goal(&self) -> DVec3 {
        let forward = DVec3::new(-self.yaw.sin(), 0., -self.yaw.cos());
        self.origin + forward * self.reach + DVec3::Y * self.lift
    }

    /// Point the steering at the hand's present place, so a new
    /// application starts where the hand is.
    pub fn aim_at_hand(&mut self, world: &World) {
        let d = self.hand_position(world) - self.origin;
        self.reach = DVec3::new(d.x, 0., d.z).length();
        self.lift = d.y;
    }

    fn teleport_hand(&self, world: &mut World, at: DVec3) {
        let hand = &mut world[self.hand];
        hand.pos = at;
        hand.prev_pos = at;
        hand.vel = DVec3::ZERO;
        hand.orientation = DQuat::IDENTITY;
        hand.prev_orientation = DQuat::IDENTITY;
    }

    fn admit(&self, world: &World, size: Size, at: DVec3, caster: DVec3) -> Result<(), Refusal> {
        if world.tick >= self.ends || self.ended(world) {
            return Err(Refusal::Ended);
        }
        if size.force_limit().is_none() {
            return Err(Refusal::TooLarge);
        }
        if at.distance(caster) > RANGE {
            return Err(Refusal::OutOfRange);
        }
        Ok(())
    }

    /// Exert the spell on rigid `body`, a target of `size` seen from
    /// `caster`. `saved` says whether the target made its save; an object
    /// that makes none passes `false`. Choosing another target releases the
    /// current one, even when the new target resists. Re-applying to a
    /// gripped body renews its budget and, for a creature, its hold.
    ///
    /// # Errors
    ///
    /// Refuses a target beyond range, larger than Huge, or immovable, and
    /// any application after concentration ends. A refusal changes nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        world: &mut World,
        caster: DVec3,
        body: BodyId,
        size: Size,
        target: Target,
        saved: bool,
        releases: &mut Vec<Release>,
    ) -> Result<Applied, Refusal> {
        self.admit(world, size, world[body].pos, caster)?;
        let force_limit = size.force_limit().ok_or(Refusal::TooLarge)?;
        if world[body].kind != BodyKind::Dynamic || world[body].removed {
            return Err(Refusal::Immovable);
        }
        let same = self.grip.is_some_and(|g| g.body == body);
        if !same && let Some(release) = self.release(world, Reason::Switched) {
            releases.push(release);
        }
        if saved {
            return Ok(Applied::Resisted);
        }
        let creature = matches!(target, Target::Creature { .. });
        let hold_until = creature.then(|| world.tick + ticks(world, ROUND));
        self.spent_reported = false;
        if same && let Some(grip) = self.grip.as_mut() {
            grip.path = 0.;
            grip.hold_until = hold_until;
            return Ok(Applied::Gripped);
        }
        let at = world[body].pos;
        self.teleport_hand(world, at);
        let tau = std::f64::consts::TAU;
        let linear = world.add_joint(
            Joint::new(self.hand, DVec3::ZERO, body, DVec3::ZERO, JointKind::Point)
                .soft(tau * LINEAR_HZ, DAMPING_RATIO)
                // Finite limits keep the world's checkpoint valid JSON.
                .limited(force_limit, force_limit * TORQUE_ARM),
        );
        let relative = world[body].orientation;
        let angular = world.add_joint(
            Joint::new(
                self.hand,
                DVec3::ZERO,
                body,
                DVec3::ZERO,
                JointKind::Weld { relative },
            )
            .soft(tau * ANGULAR_HZ, DAMPING_RATIO)
            .limited(0., force_limit * TORQUE_ARM),
        );
        self.grip = Some(Grip {
            body,
            target,
            size,
            linear,
            angular,
            path: 0.,
            hold_until,
        });
        Ok(Applied::Gripped)
    }

    /// Exert the spell on a character creature `actor` whose center is at
    /// `center`. `saved` is the result of its Strength save. On a failure
    /// it is held for one round from now.
    ///
    /// # Errors
    ///
    /// The refusals of [`Telekinesis::apply`].
    #[allow(clippy::too_many_arguments)]
    pub fn apply_creature(
        &mut self,
        world: &mut World,
        caster: DVec3,
        actor: u64,
        center: DVec3,
        size: Size,
        saved: bool,
        releases: &mut Vec<Release>,
    ) -> Result<Applied, Refusal> {
        self.admit(world, size, center, caster)?;
        let same = self.creature.is_some_and(|c| c.actor == actor);
        if !same && let Some(release) = self.release(world, Reason::Switched) {
            releases.push(release);
        }
        if saved {
            return Ok(Applied::Resisted);
        }
        let hold_until = world.tick + ticks(world, ROUND);
        self.spent_reported = false;
        if same && let Some(hold) = self.creature.as_mut() {
            hold.path = 0.;
            hold.hold_until = hold_until;
            return Ok(Applied::Gripped);
        }
        self.teleport_hand(world, center);
        self.creature = Some(CreatureHold {
            actor,
            path: 0.,
            hold_until,
        });
        Ok(Applied::Gripped)
    }

    /// Let go of the current target. A body keeps its velocity.
    pub fn release(&mut self, world: &mut World, reason: Reason) -> Option<Release> {
        if let Some(grip) = self.grip.take() {
            world.remove_joint(grip.linear);
            world.remove_joint(grip.angular);
            world[self.hand].vel = DVec3::ZERO;
            let body = &world[grip.body];
            return Some(Release {
                body: Some(grip.body),
                actor: None,
                reason,
                velocity: body.vel,
                momentum: body.momentum(),
                creature: grip.restrained(),
            });
        }
        let hold = self.creature.take()?;
        world[self.hand].vel = DVec3::ZERO;
        Some(Release {
            body: None,
            actor: Some(hold.actor),
            reason,
            velocity: DVec3::ZERO,
            momentum: DVec3::ZERO,
            creature: true,
        })
    }

    /// End concentration: release the target and remove the hand.
    pub fn end(&mut self, world: &mut World) -> Option<Release> {
        let release = self.release(world, Reason::Ended);
        if !world[self.hand].removed {
            world.remove_body(self.hand);
        }
        self.ends = world.tick;
        release
    }

    /// Before a world step: move the hand toward `aim`, clamped to range of
    /// `caster`, at most [`HAND_SPEED`] and within the application's
    /// remaining budget, and suspend a restrained body's gravity.
    pub fn steer(&mut self, world: &mut World, caster: DVec3, aim: DVec3) {
        let dt = world.dt;
        let gravity = self.gravity;
        if world[self.hand].removed {
            return;
        }
        let (path, lifted) = match (self.grip.as_mut(), self.creature.as_mut()) {
            (Some(grip), _) => {
                let lifted = grip.restrained().then_some(grip.body);
                (&mut grip.path, lifted)
            }
            (None, Some(hold)) => (&mut hold.path, None),
            (None, None) => {
                world[self.hand].vel = DVec3::ZERO;
                return;
            }
        };
        let wanted = caster + (aim - caster).clamp_length_max(RANGE);
        let from = world[self.hand].pos;
        let budget = (MOVE_BUDGET - *path).max(0.);
        let step = (wanted - from).clamp_length_max((HAND_SPEED * dt).min(budget));
        *path += step.length();
        world[self.hand].vel = step / dt;
        if let Some(body) = lifted {
            let body = &mut world[body];
            let weight = gravity * body.mass;
            body.apply_force(-weight);
        }
    }

    /// After a world step: count the grip's impulses in `ledger` under
    /// [`LEDGER_TERM`], then release a body that left range or whose hold
    /// expired, and end the spell when concentration runs out.
    pub fn after_step(
        &mut self,
        world: &mut World,
        caster: DVec3,
        ledger: Option<&mut Ledger>,
    ) -> Vec<Release> {
        let mut releases = Vec::new();
        if world[self.hand].removed {
            return releases;
        }
        if let (Some(grip), Some(ledger)) = (self.grip, ledger) {
            for id in [grip.linear, grip.angular] {
                if let Some(joint) = world.joint(id) {
                    ledger.add(
                        LEDGER_TERM,
                        Momentum::impulse(joint.impulse, joint.point, ledger.origin)
                            + Momentum {
                                linear: DVec3::ZERO,
                                angular: joint.angular_impulse,
                            },
                    );
                }
            }
            if grip.restrained() {
                let body = &world[grip.body];
                let lift = -self.gravity * body.mass * world.dt;
                ledger.add_impulse(LEDGER_TERM, lift, body.prev_pos);
            }
        }
        if world.tick >= self.ends {
            releases.extend(self.end(world));
            return releases;
        }
        if let Some(grip) = self.grip {
            if world[grip.body].pos.distance(caster) > RANGE {
                releases.extend(self.release(world, Reason::OutOfRange));
            } else if grip.hold_until.is_some_and(|t| world.tick >= t) {
                releases.extend(self.release(world, Reason::HoldExpired));
            }
        }
        releases
    }

    /// One fixed step inside the spell world: steer toward the goal before
    /// it, and account and check after it.
    pub fn substep_before(&mut self, world: &mut World) {
        let (center, goal) = (self.center(), self.goal());
        self.steer(world, center, goal);
    }

    pub fn substep_after(&mut self, world: &mut World, ledger: &mut Ledger) {
        let center = self.center();
        let releases = self.after_step(world, center, Some(ledger));
        self.released.extend(releases);
    }
}

/// The velocity that carries a gripped character's center at `position`,
/// moving at `velocity`, toward the hand at `hand` moving at `hand_velocity`
/// over `dt`: the grip's critically damped spring, solved implicitly.
#[must_use]
pub fn follow(
    position: DVec3,
    velocity: DVec3,
    hand: DVec3,
    hand_velocity: DVec3,
    dt: f64,
) -> DVec3 {
    let w = std::f64::consts::TAU * LINEAR_HZ;
    let c = 2. * DAMPING_RATIO * w;
    let pull = w * w * (hand + hand_velocity * dt - position) + c * hand_velocity;
    ((velocity + pull * dt) / (1. + dt * dt * w * w + c * dt)).clamp_length_max(CREATURE_SPEED)
}

impl crate::spells::SpellWorld {
    /// Whether a caster's Telekinesis holds scene actor `actor`, which is
    /// then Restrained: its own movement input is ignored.
    #[must_use]
    pub fn holds_creature(&self, actor: u64) -> bool {
        self.telekinesis
            .values()
            .any(|t| t.creature.is_some_and(|c| c.actor == actor))
    }
}
