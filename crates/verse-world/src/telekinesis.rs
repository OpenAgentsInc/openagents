//! Telekinesis (SRD 5.2.1) as a physics grip.
//!
//! Level 5 Transmutation. Casting time: Action. Range: 60 feet. Components:
//! V, S. Duration: Concentration, up to 10 minutes. Each application moves
//! one creature or object you can see within range up to 30 feet in any
//! direction; a creature makes a Strength saving throw and, on a failure,
//! is Restrained and suspended until the end of your next turn.
//!
//! The grip is two soft joints between a kinematic hand and the target's
//! center of mass: a 4 Hz critically damped point spring that carries it,
//! and a 2 Hz angular spring that keeps its orientation at grab time. The
//! target keeps its own mass, inertia, and colliders, so it still strikes
//! and rests on everything else. Size, not mass, sets the force limit, as
//! the SRD sets no weight limit. Releasing removes the joints and leaves
//! the target's velocity alone, so a throw is the hand's motion at release.

use glam::{DQuat, DVec3};
use physics::{Body, BodyId, BodyKind, Joint, JointId, JointKind, Ledger, Momentum, World};
use serde::{Deserialize, Serialize};

/// One foot, m.
pub const FOOT: f64 = 0.3048;
/// Spell range: 60 feet.
pub const RANGE: f64 = 60.0 * FOOT;
/// Hand path allowed by one application: 30 feet.
pub const MOVE_BUDGET: f64 = 30.0 * FOOT;
/// Fastest the hand moves, m/s.
pub const HAND_SPEED: f64 = 6.0;
/// One round, s. "Until the end of your next turn" lasts one round.
pub const ROUND: f64 = 6.0;
/// Concentration, up to 10 minutes, s.
pub const CONCENTRATION: f64 = 600.0;
/// Spell level.
pub const LEVEL: u8 = 5;
/// Natural frequency of the linear grip spring, Hz.
pub const LINEAR_HZ: f64 = 4.0;
/// Natural frequency of the angular grip spring, Hz.
pub const ANGULAR_HZ: f64 = 2.0;
/// Both grip springs are critically damped.
pub const DAMPING_RATIO: f64 = 1.0;
/// Acceleration the grip's force limit allows for its size category's
/// reference mass, m/s^2: twice standard gravity.
pub const HOLD_ACCELERATION: f64 = 2.0 * 9.81;
/// Lever arm that turns the force limit into a torque limit, m.
pub const TORQUE_ARM: f64 = 1.0;
/// Ledger term for every impulse the grip puts into a body.
pub const LEDGER_TERM: &str = "telekinesis";
/// The overlay's SRD line.
pub const SRD_LINE: &str = "Telekinesis - level 5 Transmutation - range 60 ft - \
    Concentration, up to 10 minutes - Strength save (creatures)";

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

impl Size {
    /// Mass the grip is sized for, kg; `None` for a target too large to
    /// affect. A grip holds anything up to this mass against twice gravity.
    #[must_use]
    pub fn reference_mass(self) -> Option<f64> {
        match self {
            Self::Tiny => Some(10.0),
            Self::Small => Some(300.0),
            Self::Medium => Some(1_500.0),
            Self::Large => Some(6_000.0),
            Self::Huge => Some(24_000.0),
            Self::Gargantuan => None,
        }
    }

    /// The linear grip's force limit, N.
    #[must_use]
    pub fn force_limit(self) -> Option<f64> {
        self.reference_mass().map(|m| m * HOLD_ACCELERATION)
    }
}

/// What the spell affects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// A creature, with its Strength modifier.
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

/// One Strength saving throw.
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

    /// The save `target` makes, rolling only when it makes one.
    pub fn of(target: Target, dc: i32, roll: impl FnOnce() -> i32) -> Option<Self> {
        target
            .save_modifier()
            .map(|modifier| Self::new(roll(), modifier, dc))
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

/// A grip that ended. The body kept its velocity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub body: BodyId,
    pub reason: Reason,
    /// Velocity at release, m/s.
    pub velocity: DVec3,
    /// Linear momentum at release, kg m/s.
    pub momentum: DVec3,
    /// Whether it was a creature, which then takes falling damage when it
    /// lands.
    pub creature: bool,
}

/// The current grip.
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

    /// Hand path this application still allows, m.
    #[must_use]
    pub fn budget(&self) -> f64 {
        (MOVE_BUDGET - self.path).max(0.0)
    }
}

/// One caster's concentration on Telekinesis: its kinematic hand and at
/// most one grip. Serializes with the world, so a checkpoint taken mid-grip
/// continues exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Telekinesis {
    /// The hand body: kinematic, massless, without colliders.
    pub hand: BodyId,
    /// The world's gravity, which a gripped creature does not feel.
    pub gravity: DVec3,
    /// Tick when concentration ends.
    pub ends: u64,
    pub grip: Option<Grip>,
}

/// Whole steps in `seconds` at the world's step length.
#[must_use]
pub fn ticks(world: &World, seconds: f64) -> u64 {
    (seconds / world.dt).round() as u64
}

impl Telekinesis {
    /// Start concentrating: add the hand at `at`. Nothing is gripped until
    /// [`Telekinesis::apply`].
    pub fn cast(world: &mut World, at: DVec3, gravity: DVec3) -> Self {
        let hand = world.add(Body::new(0.0, DVec3::ONE, at).with_kind(BodyKind::Kinematic));
        Self {
            hand,
            gravity,
            ends: world.tick + ticks(world, CONCENTRATION),
            grip: None,
        }
    }

    /// Hand position, m.
    #[must_use]
    pub fn hand_position(&self, world: &World) -> DVec3 {
        world[self.hand].pos
    }

    /// Exert the spell on `body`, a target of `size` seen from `caster`.
    /// `save` is the save the target made, from [`Save::of`]; pass `None`
    /// for an object. Choosing another body releases the current one, even
    /// when the new target resists. Re-applying to a gripped body renews
    /// its budget and, on a failed save, its hold.
    ///
    /// # Errors
    ///
    /// Refuses a target beyond range, larger than Huge, or immovable, and
    /// any application after concentration ends. A refusal changes nothing.
    pub fn apply(
        &mut self,
        world: &mut World,
        caster: DVec3,
        body: BodyId,
        size: Size,
        target: Target,
        save: Option<Save>,
        releases: &mut Vec<Release>,
    ) -> Result<Applied, Refusal> {
        if world.tick >= self.ends {
            return Err(Refusal::Ended);
        }
        let Some(force_limit) = size.force_limit() else {
            return Err(Refusal::TooLarge);
        };
        if world[body].kind != BodyKind::Dynamic || world[body].removed {
            return Err(Refusal::Immovable);
        }
        if world[body].pos.distance(caster) > RANGE {
            return Err(Refusal::OutOfRange);
        }
        let same = self.grip.is_some_and(|g| g.body == body);
        if !same && let Some(release) = self.release(world, Reason::Switched) {
            releases.push(release);
        }
        if save.is_some_and(|s| s.success) {
            return Ok(Applied::Resisted);
        }
        let creature = matches!(target, Target::Creature { .. });
        let hold_until = creature.then(|| world.tick + ticks(world, ROUND));
        if same && let Some(grip) = self.grip.as_mut() {
            grip.path = 0.0;
            grip.hold_until = hold_until;
            return Ok(Applied::Gripped);
        }
        let at = world[body].pos;
        let hand = &mut world[self.hand];
        hand.pos = at;
        hand.prev_pos = at;
        hand.vel = DVec3::ZERO;
        hand.orientation = DQuat::IDENTITY;
        hand.prev_orientation = DQuat::IDENTITY;
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
            .limited(0.0, force_limit * TORQUE_ARM),
        );
        self.grip = Some(Grip {
            body,
            target,
            size,
            linear,
            angular,
            path: 0.0,
            hold_until,
        });
        Ok(Applied::Gripped)
    }

    /// Let go of the current target, keeping its velocity.
    pub fn release(&mut self, world: &mut World, reason: Reason) -> Option<Release> {
        let grip = self.grip.take()?;
        world.remove_joint(grip.linear);
        world.remove_joint(grip.angular);
        world[self.hand].vel = DVec3::ZERO;
        let body = &world[grip.body];
        Some(Release {
            body: grip.body,
            reason,
            velocity: body.vel,
            momentum: body.momentum(),
            creature: grip.restrained(),
        })
    }

    /// End concentration: release the target and remove the hand.
    pub fn end(&mut self, world: &mut World) -> Option<Release> {
        let release = self.release(world, Reason::Ended);
        world.remove_body(self.hand);
        self.ends = world.tick;
        release
    }

    /// Before a world step: move the hand toward `aim`, clamped to range of
    /// `caster`, at most [`HAND_SPEED`] and within the application's
    /// remaining budget, and suspend a restrained creature's gravity.
    pub fn steer(&mut self, world: &mut World, caster: DVec3, aim: DVec3) {
        let dt = world.dt;
        let Some(grip) = self.grip.as_mut() else {
            world[self.hand].vel = DVec3::ZERO;
            return;
        };
        let offset = aim - caster;
        let wanted = caster + offset.clamp_length_max(RANGE);
        let from = world[self.hand].pos;
        let step = (wanted - from).clamp_length_max((HAND_SPEED * dt).min(grip.budget()));
        grip.path += step.length();
        world[self.hand].vel = step / dt;
        if grip.restrained() {
            let body = &mut world[grip.body];
            let weight = self.gravity * body.mass;
            body.apply_force(-weight);
        }
    }

    /// After a world step: count the grip's impulses in `ledger` under
    /// [`LEDGER_TERM`], then release a target that left range or whose hold
    /// expired, and end the spell when concentration runs out.
    pub fn after_step(
        &mut self,
        world: &mut World,
        caster: DVec3,
        ledger: Option<&mut Ledger>,
    ) -> Vec<Release> {
        let mut releases = Vec::new();
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
}

/// Falling damage dice for a fall of `height` meters: 1d6 per full 10 feet,
/// at most 20d6.
#[must_use]
pub fn falling_dice(height: f64) -> u32 {
    ((height / (10.0 * FOOT)).floor().max(0.0) as u32).min(20)
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::trace::{Tolerance, Trace};
    use physics::{Collider, NoField, Shape, Uniform};

    const DT: f64 = 1.0 / 120.0;
    const DC: i32 = 15;
    const DOWN: DVec3 = DVec3::new(0.0, -9.81, 0.0);

    fn static_box(world: &mut World, center: DVec3, half: DVec3) {
        let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
    }

    fn floor(world: &mut World) {
        static_box(
            world,
            DVec3::new(0.0, -0.5, 0.0),
            DVec3::new(60.0, 0.5, 60.0),
        );
    }

    fn block(world: &mut World, mass: f64, half: DVec3, at: DVec3) -> BodyId {
        let id = world.add(Body::new(mass, Body::box_inertia(mass, half * 2.0), at));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
        id
    }

    fn crate_at(world: &mut World, at: DVec3) -> BodyId {
        block(world, 20.0, DVec3::splat(0.3), at)
    }

    fn dummy_at(world: &mut World, x: f64) -> BodyId {
        block(
            world,
            75.0,
            DVec3::new(0.25, 0.9, 0.15),
            DVec3::new(x, 0.9, 0.0),
        )
    }

    fn run(
        world: &mut World,
        tk: &mut Telekinesis,
        caster: DVec3,
        aim: DVec3,
        seconds: f64,
    ) -> Vec<Release> {
        let mut releases = Vec::new();
        for _ in 0..ticks(world, seconds) {
            tk.steer(world, caster, aim);
            world.step(&Uniform(DOWN));
            releases.extend(tk.after_step(world, caster, None));
        }
        releases
    }

    fn grab(world: &mut World, tk: &mut Telekinesis, caster: DVec3, body: BodyId) {
        let applied = tk
            .apply(
                world,
                caster,
                body,
                Size::Small,
                Target::Object,
                None,
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(applied, Applied::Gripped);
    }

    #[test]
    fn hand_path_per_application_is_at_most_thirty_feet() {
        let mut world = World::new(DT);
        floor(&mut world);
        let caster = DVec3::ZERO;
        let body = crate_at(&mut world, DVec3::new(2.0, 0.3, 0.0));
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        grab(&mut world, &mut tk, caster, body);
        let start = tk.hand_position(&world);
        // Ask for far more than the budget, high enough to stay airborne.
        let aim = DVec3::new(14.0, 4.0, 10.0);
        run(&mut world, &mut tk, caster, aim, 2.0);
        let mid = tk.hand_position(&world);
        assert!(tk.grip.unwrap().path > 9.0, "{:?}", tk.grip);
        run(&mut world, &mut tk, caster, aim, 4.0);
        let grip = tk.grip.unwrap();
        let end = tk.hand_position(&world);
        assert!(grip.path <= MOVE_BUDGET + 1e-9, "{}", grip.path);
        assert!((grip.path - MOVE_BUDGET).abs() < 1e-9, "{}", grip.path);
        assert!(end.distance(start) <= MOVE_BUDGET + 1e-9);
        // Spent: the hand froze and the crate hangs there.
        assert!(end.distance(mid) < 1e-9 || mid.distance(start) < MOVE_BUDGET);
        assert!(world[body].pos.distance(end) < 0.05, "{}", world[body].pos);
        assert!(world[body].pos.y > 2.0);
        // A re-application renews the budget.
        grab(&mut world, &mut tk, caster, body);
        assert_eq!(tk.grip.unwrap().path, 0.0);
        run(&mut world, &mut tk, caster, aim, 2.0);
        assert!(tk.hand_position(&world).distance(end) > 1.0);
        assert!(tk.grip.unwrap().path <= MOVE_BUDGET + 1e-9);
    }

    #[test]
    fn the_hand_never_leaves_range_and_moves_at_most_six_meters_a_second() {
        let mut world = World::new(DT);
        let caster = DVec3::ZERO;
        let body = crate_at(&mut world, DVec3::new(15.0, 1.0, 0.0));
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        grab(&mut world, &mut tk, caster, body);
        let mut last = tk.hand_position(&world);
        for _ in 0..240 {
            tk.steer(&mut world, caster, DVec3::new(100.0, 1.0, 0.0));
            world.step(&NoField);
            let now = tk.hand_position(&world);
            assert!(now.distance(last) <= HAND_SPEED * DT + 1e-12);
            assert!(now.length() <= RANGE + 1e-9, "{now}");
            last = now;
        }
        assert!((last.length() - RANGE).abs() < 1e-9);
    }

    #[test]
    fn the_grip_releases_beyond_sixty_feet() {
        let mut world = World::new(DT);
        floor(&mut world);
        let mut caster = DVec3::ZERO;
        let body = crate_at(&mut world, DVec3::new(10.0, 0.3, 0.0));
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        grab(&mut world, &mut tk, caster, body);
        let aim = DVec3::new(15.0, 2.0, 0.0);
        assert!(run(&mut world, &mut tk, caster, aim, 2.0).is_empty());
        // The crate hangs at the hand while the caster walks away.
        let mut released = None;
        for _ in 0..ticks(&world, 6.0) {
            caster.x -= 3.0 * DT;
            tk.steer(&mut world, caster, aim);
            world.step(&Uniform(DOWN));
            let before = world[body].pos.distance(caster);
            if let Some(r) = tk.after_step(&mut world, caster, None).pop() {
                released = Some((r, before));
                break;
            }
            assert!(before <= RANGE);
        }
        let (release, distance) = released.expect("released at the range limit");
        assert_eq!(release.reason, Reason::OutOfRange);
        assert!(distance > RANGE && distance < RANGE + 0.05, "{distance}");
        assert!(tk.grip.is_none());
        // Released, it falls.
        run(&mut world, &mut tk, caster, aim, 2.0);
        assert!(world[body].pos.y < 0.4, "{}", world[body].pos);
    }

    #[test]
    fn a_target_beyond_range_or_larger_than_huge_is_refused() {
        let mut world = World::new(DT);
        let caster = DVec3::ZERO;
        let far = crate_at(&mut world, DVec3::new(RANGE + 0.1, 0.3, 0.0));
        let near = crate_at(&mut world, DVec3::new(3.0, 0.3, 0.0));
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        let mut releases = Vec::new();
        let refusal = tk.apply(
            &mut world,
            caster,
            far,
            Size::Small,
            Target::Object,
            None,
            &mut releases,
        );
        assert_eq!(refusal, Err(Refusal::OutOfRange));
        let refusal = tk.apply(
            &mut world,
            caster,
            near,
            Size::Gargantuan,
            Target::Object,
            None,
            &mut releases,
        );
        assert_eq!(refusal, Err(Refusal::TooLarge));
        assert!(tk.grip.is_none() && releases.is_empty());
        assert_eq!(world.joints().count(), 0);
    }

    #[test]
    fn a_failed_save_suspends_a_creature_for_exactly_six_seconds_then_it_falls() {
        let mut world = World::new(DT);
        floor(&mut world);
        let caster = DVec3::ZERO;
        let dummy = dummy_at(&mut world, 4.0);
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        run(&mut world, &mut tk, caster, DVec3::ZERO, 1.0);
        let rest = world[dummy].pos.y;
        let target = Target::Creature { strength: 0 };
        let save = Save::of(target, DC, || 2).unwrap();
        assert!(!save.success);
        let applied = tk
            .apply(
                &mut world,
                caster,
                dummy,
                Size::Medium,
                target,
                Some(save),
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(applied, Applied::Gripped);
        let gripped = world.tick;
        let lift = 20.0 * FOOT + 0.02;
        let aim = world[dummy].pos + DVec3::Y * lift;
        let mut peak = rest;
        let mut released = None;
        while released.is_none() {
            assert!(world.tick - gripped <= ticks(&world, ROUND));
            tk.steer(&mut world, caster, aim);
            world.step(&Uniform(DOWN));
            peak = peak.max(world[dummy].pos.y);
            if world.tick - gripped > ticks(&world, 3.0) {
                // Suspended: gravity is off and the hand is still.
                assert!((world[dummy].pos.y - aim.y).abs() < 1e-3);
                assert!(tk.grip.unwrap().restrained());
            }
            released = tk.after_step(&mut world, caster, None).pop();
        }
        let release = released.unwrap();
        assert_eq!(release.reason, Reason::HoldExpired);
        assert!(release.creature);
        assert_eq!(world.tick - gripped, ticks(&world, ROUND));
        assert_eq!(world.tick - gripped, 720);
        // It falls and lands.
        run(&mut world, &mut tk, caster, aim, 3.0);
        let landed = world[dummy].pos.y;
        assert!((landed - rest).abs() < 0.01, "{landed} {rest}");
        let fall = peak - landed;
        assert!(fall >= 20.0 * FOOT && fall < 21.0 * FOOT, "{fall}");
        assert_eq!(falling_dice(fall), 2);
    }

    #[test]
    fn a_successful_save_means_no_movement() {
        let mut world = World::new(DT);
        floor(&mut world);
        let caster = DVec3::ZERO;
        let dummy = dummy_at(&mut world, 4.0);
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        run(&mut world, &mut tk, caster, DVec3::ZERO, 1.0);
        let before = world[dummy].pos;
        let target = Target::Creature { strength: 0 };
        let save = Save::of(target, DC, || 18).unwrap();
        assert!(save.success);
        let applied = tk
            .apply(
                &mut world,
                caster,
                dummy,
                Size::Medium,
                target,
                Some(save),
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(applied, Applied::Resisted);
        assert!(tk.grip.is_none());
        run(&mut world, &mut tk, caster, before + DVec3::Y * 6.0, 3.0);
        assert!(world[dummy].pos.distance(before) < 1e-3);
        assert_eq!(world.joints().count(), 0);
    }

    #[test]
    fn released_velocity_is_preserved_and_the_ledger_balances() {
        let mut world = World::new(DT);
        let caster = DVec3::ZERO;
        let body = crate_at(&mut world, DVec3::new(3.0, 1.0, 0.0));
        let mut tk = Telekinesis::cast(&mut world, caster, DVec3::ZERO);
        grab(&mut world, &mut tk, caster, body);
        let origin = DVec3::new(1.0, -2.0, 0.5);
        let mut ledger = Ledger::new(origin, world.momentum(origin));
        let aim = DVec3::new(3.0, 1.0, 9.0);
        for _ in 0..ticks(&world, 1.0) {
            tk.steer(&mut world, caster, aim);
            world.step(&NoField);
            tk.after_step(&mut world, caster, Some(&mut ledger));
        }
        let before = world[body].vel;
        assert!((before.z - HAND_SPEED).abs() < 0.05, "{before}");
        let release = tk.release(&mut world, Reason::Let).unwrap();
        assert_eq!(release.velocity, before);
        assert_eq!(release.momentum, before * 20.0);
        for _ in 0..ticks(&world, 1.0) {
            tk.steer(&mut world, caster, aim);
            world.step(&NoField);
            tk.after_step(&mut world, caster, Some(&mut ledger));
        }
        assert_eq!(world[body].vel, before);
        let thrown = ledger.external[LEDGER_TERM].linear;
        assert!(thrown.distance(release.momentum) < 1e-9, "{thrown}");
        let error = ledger.error(world.momentum(origin));
        assert!(error.linear < 1e-9 && error.angular < 1e-9, "{error:?}");
    }

    #[test]
    fn a_gripped_body_still_stops_at_a_wall() {
        let mut world = World::new(DT);
        floor(&mut world);
        static_box(
            &mut world,
            DVec3::new(3.25, 2.0, 0.0),
            DVec3::new(0.25, 2.0, 4.0),
        );
        let caster = DVec3::ZERO;
        let body = crate_at(&mut world, DVec3::new(0.5, 1.0, 0.0));
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        grab(&mut world, &mut tk, caster, body);
        // The hand passes through the wall; the crate does not.
        run(&mut world, &mut tk, caster, DVec3::new(7.0, 1.0, 0.0), 4.0);
        assert!(tk.hand_position(&world).x > 6.0);
        let face = world[body].pos.x + 0.3;
        assert!(face <= 3.0 + 0.01, "{face}");
        assert!(face > 2.9, "{face}");
        assert!(tk.grip.is_some());
    }

    #[test]
    fn a_gripped_crate_knocks_over_a_standing_plank() {
        let mut world = World::new(DT);
        floor(&mut world);
        let caster = DVec3::ZERO;
        let plank = block(
            &mut world,
            8.0,
            DVec3::new(0.05, 0.6, 0.3),
            DVec3::new(3.0, 0.6, 0.0),
        );
        let body = crate_at(&mut world, DVec3::new(1.0, 1.0, 0.0));
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        grab(&mut world, &mut tk, caster, body);
        run(&mut world, &mut tk, caster, DVec3::new(6.0, 1.0, 0.0), 3.0);
        let up = world[plank].orientation * DVec3::Y;
        assert!(up.y < 0.5, "{up}");
    }

    #[test]
    fn a_huge_limit_holds_a_thousand_kilogram_block_steadily() {
        let mut world = World::new(DT);
        floor(&mut world);
        let caster = DVec3::ZERO;
        let stone = block(
            &mut world,
            1_000.0,
            DVec3::splat(0.5),
            DVec3::new(4.0, 0.5, 0.0),
        );
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        tk.apply(
            &mut world,
            caster,
            stone,
            Size::Huge,
            Target::Object,
            None,
            &mut Vec::new(),
        )
        .unwrap();
        let aim = DVec3::new(4.0, 3.0, 0.0);
        run(&mut world, &mut tk, caster, aim, 4.0);
        let sag = 9.81 / (std::f64::consts::TAU * LINEAR_HZ).powi(2);
        let pos = world[stone].pos;
        assert!((pos.y - (aim.y - sag)).abs() < 0.01, "{pos}");
        assert!(world[stone].vel.length() < 1e-3);
        let joint = world.joint(tk.grip.unwrap().linear).unwrap();
        assert!(!joint.saturated);
    }

    #[test]
    fn switching_targets_releases_the_first() {
        let mut world = World::new(DT);
        floor(&mut world);
        let caster = DVec3::ZERO;
        let first = crate_at(&mut world, DVec3::new(2.0, 0.3, 0.0));
        let second = crate_at(&mut world, DVec3::new(-2.0, 0.3, 0.0));
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        grab(&mut world, &mut tk, caster, first);
        run(&mut world, &mut tk, caster, DVec3::new(2.0, 2.0, 0.0), 1.0);
        let mut releases = Vec::new();
        tk.apply(
            &mut world,
            caster,
            second,
            Size::Small,
            Target::Object,
            None,
            &mut releases,
        )
        .unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].body, first);
        assert_eq!(releases[0].reason, Reason::Switched);
        assert_eq!(tk.grip.unwrap().body, second);
        assert_eq!(world.joints().count(), 2);
    }

    #[test]
    fn a_checkpoint_mid_grip_replays_identically() {
        let mut world = World::new(DT);
        floor(&mut world);
        let caster = DVec3::ZERO;
        let body = crate_at(&mut world, DVec3::new(2.0, 0.3, 0.0));
        let dummy = dummy_at(&mut world, -3.0);
        let mut tk = Telekinesis::cast(&mut world, caster, DOWN);
        grab(&mut world, &mut tk, caster, body);
        let aim = |t: u64| DVec3::new(2.0 + (t as f64 * 0.01).sin() * 3.0, 2.5, -1.0);
        for _ in 0..90 {
            let at = aim(world.tick);
            tk.steer(&mut world, caster, at);
            world.step(&Uniform(DOWN));
            tk.after_step(&mut world, caster, None);
        }
        let saved = serde_json::to_string(&(&world, &tk)).unwrap();
        let (mut world2, mut tk2): (World, Telekinesis) = serde_json::from_str(&saved).unwrap();
        assert_eq!(world2, world);
        assert_eq!(tk2, tk);
        let (mut a, mut b) = (Trace::default(), Trace::default());
        for (w, t, trace) in [
            (&mut world, &mut tk, &mut a),
            (&mut world2, &mut tk2, &mut b),
        ] {
            for i in 0..600 {
                let at = aim(w.tick);
                t.steer(w, caster, at);
                w.step(&Uniform(DOWN));
                t.after_step(w, caster, None);
                if i == 200 {
                    t.release(w, Reason::Let);
                    let save = Save::new(3, 0, DC);
                    t.apply(
                        w,
                        caster,
                        dummy,
                        Size::Medium,
                        Target::Creature { strength: 0 },
                        Some(save),
                        &mut Vec::new(),
                    )
                    .unwrap();
                }
                trace.record(w);
            }
        }
        a.compare(&b, Tolerance::EXACT).unwrap();
        assert_eq!(tk, tk2);
    }

    #[test]
    fn falling_dice_follow_the_srd() {
        assert_eq!(falling_dice(9.9 * FOOT), 0);
        assert_eq!(falling_dice(10.0 * FOOT + 1e-9), 1);
        assert_eq!(falling_dice(25.0 * FOOT), 2);
        assert_eq!(falling_dice(250.0 * FOOT), 20);
    }
}
