//! Black Tentacles (SRD 5.2.1) as articulated grabbing tentacles.
//!
//! Level 4 Conjuration. Casting time: Action. Range: 90 feet. Components:
//! V, S, M (a tentacle). Duration: Concentration, up to 1 minute. Squirming
//! tentacles fill a 20-foot square on ground within range, and the area is
//! Difficult Terrain. Each creature in the area makes a Strength saving
//! throw; on a failure it takes 3d6 Bludgeoning damage and is Restrained
//! until the spell ends. A creature also makes that save when it enters
//! the area or ends its turn there, only once per turn. A Restrained
//! creature can take an action to make a Strength (Athletics) check
//! against the spell save DC, ending the condition on a success.
//!
//! Nine tentacles stand on a 3 x 3 grid. Each is a chain of eight capsule
//! segments: a ball joint to a static root, then a ball joint between
//! neighbors, each paired with a soft angular spring and an angular limit.
//! The spell carries the segments' weight and damps them, so a tip force
//! toward a goal and seeded writhing move the whole chain. A capture is a
//! soft, force-limited joint between a tip and the nearest point of its
//! target. Creatures are captured only after a failed save; loose props
//! are grabbed on reach, and a pull stronger than the grip steals them
//! back.

use glam::{DQuat, DVec3};
use physics::{
    Body, BodyId, BodyKind, Collider, Filter, Joint, JointId, JointKind, Ledger, Momentum, Shape,
    World,
};
use serde::{Deserialize, Serialize};

/// One foot, m.
pub const FOOT: f64 = 0.3048;
/// Spell range: 90 feet.
pub const RANGE: f64 = 90.0 * FOOT;
/// Side of the square area: 20 feet.
pub const SIDE: f64 = 20.0 * FOOT;
/// One round, s. "Ends its turn there" is once a round, and "only once
/// per turn" allows at most one save a round.
pub const ROUND: f64 = 6.0;
/// Concentration, up to 1 minute, s.
pub const CONCENTRATION: f64 = 60.0;
/// Spell level.
pub const LEVEL: u8 = 4;
/// Difficult Terrain: every foot costs two, so speed is halved.
pub const DIFFICULT_TERRAIN: f64 = 0.5;
/// Bludgeoning damage on a failed save: 3d6.
pub const DAMAGE_DICE: u32 = 3;
pub const DAMAGE_SIDES: u32 = 6;
/// Tentacles along each side of the square.
pub const GRID: usize = 3;
/// Segments in one tentacle.
pub const SEGMENTS: usize = 8;
/// Length of one tentacle, m.
pub const LENGTH: f64 = 2.4;
/// Length of one segment, m.
pub const SEGMENT_LENGTH: f64 = LENGTH / SEGMENTS as f64;
/// Mass of one segment, kg.
pub const SEGMENT_MASS: f64 = 4.0;
/// Segment radius at the root and at the tip, m.
pub const BASE_RADIUS: f64 = 0.1;
pub const TIP_RADIUS: f64 = 0.045;
/// Height of a root's ball joint above the ground, m: the base segment
/// clears the floor.
pub const ROOT_HEIGHT: f64 = BASE_RADIUS + 0.02;
/// How far a tip reaches from its root, m, with a hand's width of slack.
pub const REACH: f64 = LENGTH + 0.2;
/// Force limit of a ball joint in the chain, N: far above any load, and
/// finite so a checkpoint is valid JSON.
pub const CHAIN_FORCE: f64 = 1.0e7;
/// Natural frequency of the angular spring between neighbors, Hz.
pub const BEND_HZ: f64 = 1.5;
/// Natural frequency of the angular spring that stands the base up, Hz.
pub const BASE_HZ: f64 = 3.0;
/// Damping ratio of the angular springs.
pub const BEND_DAMPING: f64 = 0.6;
/// Largest torque an angular spring transmits, N m.
pub const BEND_TORQUE: f64 = 300.0;
/// Largest bend between neighbors, rad (40 degrees).
pub const BEND_LIMIT: f64 = 40.0 * std::f64::consts::PI / 180.0;
/// Fraction of a bend beyond the limit removed per step.
pub const LIMIT_ERP: f64 = 0.2;
/// Tip controller stiffness, N/m.
pub const TIP_STIFFNESS: f64 = 700.0;
/// Tip controller damping, N s/m.
pub const TIP_DAMPING: f64 = 70.0;
/// Largest tip force, N.
pub const TIP_FORCE: f64 = 700.0;
/// Drag on every segment, 1/s: the force is this times its momentum.
pub const DRAG: f64 = 3.0;
/// Height above the root an idle tip seeks, m.
pub const IDLE_HEIGHT: f64 = 1.8;
/// Amplitude of the idle writhing, m.
pub const WRITHE: f64 = 0.6;
/// Slowest and fastest writhing frequency, Hz.
pub const WRITHE_HZ: [f64; 2] = [0.25, 0.6];
/// A seeking tip grabs within this distance of its target, m.
pub const GRAB_DISTANCE: f64 = 0.3;
/// A seizing tip attaches after this long even if it has not reached, s.
pub const SEIZE_TIMEOUT: f64 = 0.75;
/// Tentacles that wrap a creature that failed its save.
pub const WRAPS: usize = 3;
/// Tentacles that may hold one prop.
pub const PROP_HOLDERS: usize = 2;
/// Natural frequency of a capture joint, Hz.
pub const CAPTURE_HZ: f64 = 4.0;
/// Damping ratio of a capture joint.
pub const CAPTURE_DAMPING: f64 = 1.0;
/// Force limit of a capture on a creature, N. Restrained ends only when the
/// spell ends or the creature escapes, so this capture never breaks; the
/// limit only lets a push stretch it.
pub const CAPTURE_FORCE: f64 = 4_000.0;
/// Force limit of a grip on a prop, N: below Telekinesis's grip on a
/// Small object (300 kg at twice gravity), so Telekinesis steals it back.
pub const PROP_GRIP_FORCE: f64 = 1_500.0;
/// A prop grip at its limit this long is stolen, s.
pub const STEAL_TIME: f64 = 0.25;
/// A stolen prop is not grabbed again for this long, s.
pub const REGRAB_DELAY: f64 = 3.0;
/// A held creature's pull back toward where it was seized: stiffness per
/// holding tentacle, N/m.
pub const HOLD_STIFFNESS: f64 = 1_500.0;
/// Its damping per holding tentacle, N s/m.
pub const HOLD_DAMPING: f64 = 300.0;
/// Height above its root a held prop is lifted to, m.
pub const PROP_LIFT: f64 = 1.0;
/// Distance a seeker treats each other seeker on a target as, m, so the
/// tentacles spread over several targets.
pub const SPREAD: f64 = 1.5;
/// Largest step in ground height under the square, m, for placement.
pub const GROUND_TOLERANCE: f64 = 0.4;
/// Collision group of every tentacle segment. Segments do not collide
/// with each other.
pub const TENTACLE_GROUP: u32 = 1 << 29;
/// Ledger term for every impulse the spell puts into the world.
pub const LEDGER_TERM: &str = "black_tentacles";
/// The overlay's SRD line.
pub const SRD_LINE: &str = "Black Tentacles - level 4 Conjuration - range 90 ft - \
    20-ft square - Concentration, up to 1 minute - Strength save";

/// Whole steps in `seconds` at the world's step length.
#[must_use]
pub fn ticks(world: &World, seconds: f64) -> u64 {
    (seconds / world.dt).round() as u64
}

/// What the spell can act on this step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub body: BodyId,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A creature with its Strength modifier and Athletics bonus.
    Creature { strength: i32, athletics: i32 },
    /// An object; a secured one is welded to static geometry.
    Prop { secured: bool },
}

/// A d20 roll plus a modifier against a DC; meeting the DC succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    pub d20: i32,
    pub modifier: i32,
    pub dc: i32,
    pub success: bool,
}

impl Save {
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

/// What made a creature save.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    /// It was in the area when the spell was cast.
    Cast,
    /// It entered the area.
    Enter,
    /// It ended its turn in the area: a round on its own clock.
    EndOfTurn,
}

/// Something the spell did this step, for the caller to apply or show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// A Strength save. On a failure, `damage` is the 3d6 Bludgeoning the
    /// caller applies and the creature is Restrained.
    Save {
        body: BodyId,
        trigger: Trigger,
        save: Save,
        damage: Option<i32>,
    },
    /// A tentacle tip attached to a Restrained creature.
    Wrapped { body: BodyId, tentacle: usize },
    /// A tentacle tip grabbed a loose prop.
    Grabbed { body: BodyId, tentacle: usize },
    /// A stronger pull took a prop from a tentacle.
    Stolen { body: BodyId, tentacle: usize },
    /// A Strength (Athletics) check to escape.
    Escape { body: BodyId, check: Save },
    /// The spell ended and released everything.
    Ended,
}

/// Why a cast was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// The point is beyond 90 feet.
    OutOfRange,
    /// There is no ground under the point.
    NoGround,
    /// The ground under the square is not level enough to fill.
    Uneven,
}

/// What a tentacle is doing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    /// Writhing in place.
    Idle,
    /// Reaching for a target. Creatures are only reached for, not caught.
    Seek { body: BodyId },
    /// Reaching to wrap a creature that failed its save.
    Seize { body: BodyId, since: u64 },
    /// Holding a target through a capture joint. `strained` counts steps
    /// the joint spent at its force limit in a row.
    Hold {
        body: BodyId,
        joint: JointId,
        strained: u64,
    },
}

impl Mode {
    /// The body this tentacle is after or holds.
    #[must_use]
    pub fn body(self) -> Option<BodyId> {
        match self {
            Self::Idle => None,
            Self::Seek { body } | Self::Seize { body, .. } | Self::Hold { body, .. } => Some(body),
        }
    }
}

/// One articulated tentacle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tentacle {
    /// The root's ball joint, m.
    pub root: DVec3,
    /// The static body the root joint holds.
    pub anchor: BodyId,
    /// Segments from root to tip.
    pub segments: Vec<BodyId>,
    /// Ball joints and angular springs, root first.
    pub joints: Vec<JointId>,
    /// Writhing phases, rad, and frequencies, Hz.
    pub phase: [f64; 3],
    pub frequency: [f64; 3],
    pub mode: Mode,
}

impl Tentacle {
    #[must_use]
    pub fn tip(&self) -> BodyId {
        self.segments[SEGMENTS - 1]
    }

    /// World position of the tip's free end, m.
    #[must_use]
    pub fn tip_end(&self, world: &World) -> DVec3 {
        world[self.tip()].to_world(DVec3::Z * (SEGMENT_LENGTH / 2.0))
    }
}

/// What the spell remembers about one creature.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub body: BodyId,
    pub inside: bool,
    /// Tick its end-of-turn clock last started: on entry or at the cast.
    pub clock: u64,
    pub last_save: Option<u64>,
    pub restrained: bool,
    /// Where it was when it was seized, m: the tentacles pull it back here.
    pub hold: DVec3,
}

/// One cast of Black Tentacles. Serializes with the world, so a checkpoint
/// taken mid-grab continues exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlackTentacles {
    /// Center of the square on the ground, m.
    pub center: DVec3,
    /// Gravity the tentacles do not feel, m/s^2.
    pub gravity: DVec3,
    /// The caster's spell save DC.
    pub dc: i32,
    /// Tick the spell was cast.
    pub cast: u64,
    /// Tick concentration runs out.
    pub ends: u64,
    pub ended: bool,
    pub tentacles: Vec<Tentacle>,
    pub creatures: Vec<Creature>,
    /// Stolen props and the tick each may be grabbed again.
    pub regrab: Vec<(BodyId, u64)>,
}

/// Check a cast at `at` by a caster at `caster`. `ground(x, z)` is the
/// walkable ground height there, if any. Returns the square's center on
/// the ground.
///
/// # Errors
///
/// Refuses a point beyond 90 feet, without ground, or over ground that
/// steps more than [`GROUND_TOLERANCE`] anywhere on the square's edge.
pub fn place(
    caster: DVec3,
    at: DVec3,
    ground: impl Fn(f64, f64) -> Option<f64>,
) -> Result<DVec3, Refusal> {
    if caster.distance(at) > RANGE {
        return Err(Refusal::OutOfRange);
    }
    let height = ground(at.x, at.z).ok_or(Refusal::NoGround)?;
    let half = SIDE / 2.0;
    for (dx, dz) in [
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ] {
        let edge = ground(at.x + dx * half, at.z + dz * half).ok_or(Refusal::Uneven)?;
        if (edge - height).abs() > GROUND_TOLERANCE {
            return Err(Refusal::Uneven);
        }
    }
    Ok(DVec3::new(at.x, height, at.z))
}

/// A uniform value in [0, 1) from `seed` and `n`.
fn unit(seed: u64, n: u64) -> f64 {
    let mut z = seed ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Roll `count` dice of `sides` with `roll`.
fn dice(count: u32, sides: u32, roll: &mut dyn FnMut(u32) -> u32) -> i32 {
    (0..count).map(|_| roll(sides) as i32).sum()
}

fn segment_radius(i: usize) -> f64 {
    let t = i as f64 / (SEGMENTS - 1) as f64;
    BASE_RADIUS + (TIP_RADIUS - BASE_RADIUS) * t
}

/// Rotation taking a segment's axis, body z, onto `dir`.
fn along(dir: DVec3) -> DQuat {
    DQuat::from_rotation_arc(DVec3::Z, dir.normalize())
}

/// The point of `body`'s colliders nearest `x`.
fn nearest(world: &World, body: BodyId, x: DVec3) -> DVec3 {
    world
        .colliders()
        .iter()
        .filter(|c| c.body == body && c.filter != Filter::NONE)
        .map(|c| c.closest_point(world, x))
        .min_by(|a, b| a.distance(x).total_cmp(&b.distance(x)))
        .unwrap_or(world[body].pos)
}

impl BlackTentacles {
    /// Fill the square centered on `center` (on the ground, from
    /// [`place`]) with tentacles lying on the ground, then make every
    /// creature in `targets` inside the square save. `seed` sets how the
    /// tentacles lie and writhe; `roll(sides)` rolls one die.
    pub fn cast(
        world: &mut World,
        center: DVec3,
        gravity: DVec3,
        dc: i32,
        seed: u64,
        targets: &[Target],
        roll: &mut dyn FnMut(u32) -> u32,
    ) -> (Self, Vec<Event>) {
        Self::cast_with(
            world,
            center,
            gravity,
            dc,
            seed,
            targets,
            &mut |_, sides| roll(sides),
        )
    }

    /// Cast with identity-bound saves and a shared damage-die stream.
    pub fn cast_with(
        world: &mut World,
        center: DVec3,
        gravity: DVec3,
        dc: i32,
        seed: u64,
        targets: &[Target],
        roll: &mut dyn FnMut(Option<BodyId>, u32) -> u32,
    ) -> (Self, Vec<Event>) {
        let spacing = SIDE / GRID as f64;
        let mut tentacles = Vec::with_capacity(GRID * GRID);
        for i in 0..GRID * GRID {
            let (gx, gz) = ((i % GRID) as f64 - 1.0, (i / GRID) as f64 - 1.0);
            let root = center + DVec3::new(gx * spacing, ROOT_HEIGHT, gz * spacing);
            tentacles.push(Self::grow(world, root, seed, i as u64));
        }
        let mut spell = Self {
            center,
            gravity,
            dc,
            cast: world.tick,
            ends: world.tick + ticks(world, CONCENTRATION),
            ended: false,
            tentacles,
            creatures: Vec::new(),
            regrab: Vec::new(),
        };
        let mut events = Vec::new();
        for target in targets {
            if let Kind::Creature { strength, .. } = target.kind {
                let inside = spell.contains(world[target.body].pos);
                spell.creatures.push(Creature {
                    body: target.body,
                    inside,
                    clock: world.tick,
                    last_save: None,
                    restrained: false,
                    hold: world[target.body].pos,
                });
                if inside {
                    events.extend(spell.save(world, target.body, strength, Trigger::Cast, roll));
                }
            }
        }
        (spell, events)
    }

    /// Lay one tentacle on the ground from `root` in a seeded direction.
    fn grow(world: &mut World, root: DVec3, seed: u64, n: u64) -> Tentacle {
        let anchor = world.add(Body::new(0.0, DVec3::ONE, root).with_kind(BodyKind::Static));
        let heading = unit(seed, n * 16) * std::f64::consts::TAU;
        let dir = DVec3::new(heading.cos(), 0.0, heading.sin());
        let rest = along(dir);
        let filter = Filter {
            group: TENTACLE_GROUP,
            mask: !TENTACLE_GROUP,
        };
        let tau = std::f64::consts::TAU;
        let mut segments = Vec::with_capacity(SEGMENTS);
        let mut joints = Vec::with_capacity(SEGMENTS * 2);
        let mut previous = anchor;
        for i in 0..SEGMENTS {
            let radius = segment_radius(i);
            let at = root + dir * (SEGMENT_LENGTH * (i as f64 + 0.5));
            let mut body = Body::new(
                SEGMENT_MASS,
                Body::box_inertia(
                    SEGMENT_MASS,
                    DVec3::new(2.0 * radius, 2.0 * radius, SEGMENT_LENGTH),
                ),
                at,
            );
            body.orientation = rest;
            body.prev_orientation = rest;
            let id = world.add(body);
            world.add_collider(
                Collider::new(
                    id,
                    Shape::Capsule {
                        radius,
                        half_length: SEGMENT_LENGTH / 2.0,
                    },
                )
                .with_filter(filter),
            );
            let half = DVec3::Z * (SEGMENT_LENGTH / 2.0);
            let (anchor_a, relative, hz) = if i == 0 {
                (DVec3::ZERO, along(DVec3::Y), BASE_HZ)
            } else {
                (half, DQuat::IDENTITY, BEND_HZ)
            };
            joints.push(
                world.add_joint(
                    Joint::new(previous, anchor_a, id, -half, JointKind::Point)
                        // Finite limits keep the world's checkpoint valid JSON.
                        .limited(CHAIN_FORCE, CHAIN_FORCE),
                ),
            );
            joints.push(
                world.add_joint(
                    Joint::new(previous, anchor_a, id, -half, JointKind::Weld { relative })
                        .soft(tau * hz, BEND_DAMPING)
                        // Angular only: the ball joint holds the anchors.
                        .limited(0.0, BEND_TORQUE),
                ),
            );
            segments.push(id);
            previous = id;
        }
        let mut phase = [0.0; 3];
        let mut frequency = [0.0; 3];
        for k in 0..3 {
            phase[k] = unit(seed, n * 16 + 1 + k as u64) * tau;
            let t = unit(seed, n * 16 + 4 + k as u64);
            frequency[k] = WRITHE_HZ[0] + (WRITHE_HZ[1] - WRITHE_HZ[0]) * t;
        }
        Tentacle {
            root,
            anchor,
            segments,
            joints,
            phase,
            frequency,
            mode: Mode::Idle,
        }
    }

    /// Whether `pos` is over the square.
    #[must_use]
    pub fn contains(&self, pos: DVec3) -> bool {
        let d = pos - self.center;
        let half = SIDE / 2.0;
        !self.ended && d.x.abs() <= half && d.z.abs() <= half
    }

    /// Movement speed scale at `pos`: halved over the square.
    #[must_use]
    pub fn speed_scale(&self, pos: DVec3) -> f64 {
        if self.contains(pos) {
            DIFFICULT_TERRAIN
        } else {
            1.0
        }
    }

    /// Whether `body` is Restrained by this spell; its controller takes no
    /// input while it is.
    #[must_use]
    pub fn restrained(&self, body: BodyId) -> bool {
        self.creatures
            .iter()
            .any(|c| c.body == body && c.restrained)
    }

    /// Capture joints holding `body`.
    #[must_use]
    pub fn captures(&self, body: BodyId) -> Vec<JointId> {
        self.tentacles
            .iter()
            .filter_map(|t| match t.mode {
                Mode::Hold { body: b, joint, .. } if b == body => Some(joint),
                _ => None,
            })
            .collect()
    }

    /// Bodies and joints the spell owns.
    #[must_use]
    pub fn owned(&self) -> (Vec<BodyId>, Vec<JointId>) {
        let mut bodies = Vec::new();
        let mut joints = Vec::new();
        for t in &self.tentacles {
            bodies.push(t.anchor);
            bodies.extend(&t.segments);
            joints.extend(&t.joints);
            if let Mode::Hold { joint, .. } = t.mode {
                joints.push(joint);
            }
        }
        (bodies, joints)
    }

    fn creature_mut(&mut self, body: BodyId) -> Option<&mut Creature> {
        self.creatures.iter_mut().find(|c| c.body == body)
    }

    /// One Strength save by `body`, unless it already saved this round.
    fn save(
        &mut self,
        world: &World,
        body: BodyId,
        strength: i32,
        trigger: Trigger,
        roll: &mut dyn FnMut(Option<BodyId>, u32) -> u32,
    ) -> Vec<Event> {
        let round = ticks(world, ROUND);
        let tick = world.tick;
        let dc = self.dc;
        let Some(creature) = self.creature_mut(body) else {
            return Vec::new();
        };
        if creature.restrained || creature.last_save.is_some_and(|t| tick < t + round) {
            return Vec::new();
        }
        creature.last_save = Some(tick);
        let save = Save::new(roll(Some(body), 20) as i32, strength, dc);
        let damage =
            (!save.success).then(|| dice(DAMAGE_DICE, DAMAGE_SIDES, &mut |s| roll(None, s)));
        if !save.success {
            creature.restrained = true;
            creature.hold = world[body].pos;
            self.seize(world, body);
        }
        vec![Event::Save {
            body,
            trigger,
            save,
            damage,
        }]
    }

    /// Send the nearest free tentacles in reach to wrap `body`.
    fn seize(&mut self, world: &World, body: BodyId) {
        let mut free: Vec<(f64, usize)> = self
            .tentacles
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(t.mode, Mode::Hold { .. } | Mode::Seize { .. }))
            .map(|(i, t)| (nearest(world, body, t.root).distance(t.root), i))
            .filter(|(d, _)| *d <= REACH)
            .collect();
        free.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for &(_, i) in free.iter().take(WRAPS) {
            self.tentacles[i].mode = Mode::Seize {
                body,
                since: world.tick,
            };
        }
    }

    /// A Restrained creature's Strength (Athletics) check with `d20`. On a
    /// success its captures are removed and it is free until its next save.
    pub fn escape(
        &mut self,
        world: &mut World,
        body: BodyId,
        d20: i32,
        athletics: i32,
    ) -> Option<Event> {
        let dc = self.dc;
        let creature = self.creature_mut(body)?;
        if !creature.restrained {
            return None;
        }
        let check = Save::new(d20, athletics, dc);
        if check.success {
            creature.restrained = false;
            for t in &mut self.tentacles {
                if t.mode.body() == Some(body) {
                    if let Mode::Hold { joint, .. } = t.mode {
                        world.remove_joint(joint);
                    }
                    t.mode = Mode::Idle;
                }
            }
        }
        Some(Event::Escape { body, check })
    }

    /// Before a world step: track creatures in and out of the area and make
    /// the saves they owe, choose what each tentacle reaches for, and apply
    /// the spell's forces, counting them in `ledger` under
    /// [`LEDGER_TERM`].
    pub fn before_step(
        &mut self,
        world: &mut World,
        targets: &[Target],
        roll: &mut dyn FnMut(u32) -> u32,
        ledger: Option<&mut Ledger>,
    ) -> Vec<Event> {
        self.before_step_with(world, targets, &mut |_, sides| roll(sides), ledger)
    }

    /// Step with identity-bound saves, preserving the native six-second clock.
    pub fn before_step_with(
        &mut self,
        world: &mut World,
        targets: &[Target],
        roll: &mut dyn FnMut(Option<BodyId>, u32) -> u32,
        mut ledger: Option<&mut Ledger>,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        if self.ended {
            return events;
        }
        let round = ticks(world, ROUND);
        let tick = world.tick;
        for target in targets {
            let Kind::Creature { strength, .. } = target.kind else {
                continue;
            };
            let pos = world[target.body].pos;
            let inside = self.contains(pos);
            if self.creature_mut(target.body).is_none() {
                self.creatures.push(Creature {
                    body: target.body,
                    inside: false,
                    clock: tick,
                    last_save: None,
                    restrained: false,
                    hold: pos,
                });
            }
            let creature = self.creature_mut(target.body).expect("tracked");
            let entered = inside && !creature.inside;
            creature.inside = inside;
            let trigger = if entered {
                creature.clock = tick;
                Some(Trigger::Enter)
            } else if inside && tick >= creature.clock + round {
                creature.clock += round;
                Some(Trigger::EndOfTurn)
            } else {
                None
            };
            if let Some(trigger) = trigger {
                events.extend(self.save(world, target.body, strength, trigger, roll));
            }
        }
        self.regrab.retain(|&(_, until)| until > tick);
        self.choose(world, targets);
        events.extend(self.attach(world));
        self.drive(world, ledger.as_deref_mut());
        self.limit_bends(world);
        events
    }

    /// Give idle and seeking tentacles their targets: the nearest creature
    /// or loose prop in the area within reach, counting each seeker already
    /// on a target as [`SPREAD`] meters farther.
    fn choose(&mut self, world: &World, targets: &[Target]) {
        let wanted: Vec<(BodyId, bool)> = targets
            .iter()
            .filter(|t| self.contains(world[t.body].pos))
            .filter_map(|t| match t.kind {
                Kind::Creature { .. } => {
                    let restrained = self.restrained(t.body);
                    Some((t.body, restrained))
                }
                Kind::Prop { secured: false } => {
                    (!self.regrab.iter().any(|&(b, _)| b == t.body)).then_some((t.body, false))
                }
                Kind::Prop { secured: true } => None,
            })
            .collect();
        let mut count: Vec<usize> = wanted
            .iter()
            .map(|(body, _)| {
                self.tentacles
                    .iter()
                    .filter(|t| {
                        matches!(t.mode, Mode::Hold { .. } | Mode::Seize { .. })
                            && t.mode.body() == Some(*body)
                    })
                    .count()
            })
            .collect();
        let props: Vec<bool> = wanted
            .iter()
            .map(|(body, _)| {
                targets
                    .iter()
                    .any(|t| t.body == *body && matches!(t.kind, Kind::Prop { .. }))
            })
            .collect();
        for t in &mut self.tentacles {
            if matches!(t.mode, Mode::Hold { .. } | Mode::Seize { .. }) {
                continue;
            }
            let mut best: Option<(f64, usize)> = None;
            for (k, (body, restrained)) in wanted.iter().enumerate() {
                // A wrapped creature needs no more seekers; a held prop
                // takes at most two hands.
                if (*restrained && count[k] >= WRAPS) || (props[k] && count[k] >= PROP_HOLDERS) {
                    continue;
                }
                let distance = nearest(world, *body, t.root).distance(t.root);
                if distance > REACH {
                    continue;
                }
                let cost = distance + SPREAD * count[k] as f64;
                if best.is_none_or(|(c, _)| cost < c) {
                    best = Some((cost, k));
                }
            }
            t.mode = match best {
                Some((_, k)) => {
                    count[k] += 1;
                    Mode::Seek { body: wanted[k].0 }
                }
                None => Mode::Idle,
            };
        }
    }

    /// Attach seizing tips that reached or ran out of time, and seeking
    /// tips that reached a loose prop.
    fn attach(&mut self, world: &mut World) -> Vec<Event> {
        let timeout = ticks(world, SEIZE_TIMEOUT);
        let mut events = Vec::new();
        for (i, t) in self.tentacles.iter_mut().enumerate() {
            let (body, creature) = match t.mode {
                Mode::Seize { body, since } => {
                    let tip = t.tip_end(world);
                    let near = nearest(world, body, tip).distance(tip) <= GRAB_DISTANCE;
                    if !near && world.tick < since + timeout {
                        continue;
                    }
                    (body, true)
                }
                Mode::Seek { body } => {
                    let tip = t.tip_end(world);
                    let prop = !self.creatures.iter().any(|c| c.body == body);
                    if !prop || nearest(world, body, tip).distance(tip) > GRAB_DISTANCE {
                        continue;
                    }
                    (body, false)
                }
                _ => continue,
            };
            let tip = t.tip_end(world);
            let point = nearest(world, body, tip);
            let target = &world[body];
            let anchor_b = target.orientation.inverse() * (point - target.pos);
            let limit = if creature {
                CAPTURE_FORCE
            } else {
                PROP_GRIP_FORCE
            };
            let joint = world.add_joint(
                Joint::new(
                    t.tip(),
                    DVec3::Z * (SEGMENT_LENGTH / 2.0),
                    body,
                    anchor_b,
                    JointKind::Point,
                )
                .soft(std::f64::consts::TAU * CAPTURE_HZ, CAPTURE_DAMPING)
                .limited(limit, 0.0),
            );
            t.mode = Mode::Hold {
                body,
                joint,
                strained: 0,
            };
            events.push(if creature {
                Event::Wrapped { body, tentacle: i }
            } else {
                Event::Grabbed { body, tentacle: i }
            });
        }
        events
    }

    /// Where each tentacle's tip is driven, and the forces that carry the
    /// segments' weight, damp them, and drive the tips.
    fn drive(&self, world: &mut World, mut ledger: Option<&mut Ledger>) {
        let dt = world.dt;
        let time = (world.tick - self.cast) as f64 * dt;
        let tau = std::f64::consts::TAU;
        let mut push = |world: &mut World, body: BodyId, force: DVec3| {
            let b = &mut world[body];
            if let Some(ledger) = ledger.as_deref_mut() {
                ledger.add_impulse(LEDGER_TERM, force * dt, b.pos);
            }
            b.apply_force(force);
        };
        for t in &self.tentacles {
            let w = |k: usize| (tau * t.frequency[k] * time + t.phase[k]).sin();
            let writhe = DVec3::new(w(0), 0.4 * w(2), w(1)) * WRITHE;
            let tip = t.tip_end(world);
            let tip_vel = world[t.tip()].vel;
            let pd = |goal: DVec3| {
                ((goal - tip) * TIP_STIFFNESS - tip_vel * TIP_DAMPING).clamp_length_max(TIP_FORCE)
            };
            let force = match t.mode {
                Mode::Idle => pd(t.root + DVec3::Y * IDLE_HEIGHT + writhe),
                Mode::Seek { body } | Mode::Seize { body, .. } => {
                    pd(nearest(world, body, tip) + writhe * 0.15)
                }
                Mode::Hold { body, .. } => match self.creatures.iter().find(|c| c.body == body) {
                    Some(c) => {
                        let target = &world[body];
                        ((c.hold - target.pos) * HOLD_STIFFNESS - target.vel * HOLD_DAMPING)
                            .clamp_length_max(TIP_FORCE)
                    }
                    None => pd(t.root + DVec3::Y * PROP_LIFT + writhe * 0.5),
                },
            };
            push(world, t.tip(), force);
            for &s in &t.segments {
                let body = &world[s];
                let carry = -self.gravity * body.mass - body.vel * body.mass * DRAG;
                push(world, s, carry);
            }
        }
    }

    /// Hold every bend at most [`BEND_LIMIT`]: remove relative angular
    /// velocity that opens a bend beyond it, with a bias that closes it. The
    /// impulses are equal and opposite, so they add no angular momentum.
    fn limit_bends(&self, world: &mut World) {
        let dt = world.dt;
        for t in &self.tentacles {
            for pair in t.segments.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let error = world[b].orientation * world[a].orientation.inverse();
                let error = if error.w < 0.0 { -error } else { error };
                let (axis, angle) = error.to_axis_angle();
                if angle <= BEND_LIMIT || !axis.is_finite() {
                    continue;
                }
                let opening = (world[b].omega_world() - world[a].omega_world()).dot(axis);
                let wanted = -LIMIT_ERP * (angle - BEND_LIMIT) / dt;
                if opening <= wanted {
                    continue;
                }
                let inverse = axis.dot(world[a].inverse_inertia_world(axis))
                    + axis.dot(world[b].inverse_inertia_world(axis));
                if inverse <= 0.0 {
                    continue;
                }
                let impulse = axis * ((opening - wanted) / inverse);
                world[a].apply_angular_impulse(impulse);
                world[b].apply_angular_impulse(-impulse);
            }
        }
    }

    /// After a world step: count the impulses the roots took in `ledger`,
    /// let a prop go from a grip held at its limit for [`STEAL_TIME`], and
    /// end the spell when concentration runs out.
    pub fn after_step(&mut self, world: &mut World, ledger: Option<&mut Ledger>) -> Vec<Event> {
        let mut events = Vec::new();
        if self.ended {
            return events;
        }
        if let Some(ledger) = ledger {
            for t in &self.tentacles {
                for &id in &t.joints[..2] {
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
            }
        }
        let steal = ticks(world, STEAL_TIME);
        let regrab = world.tick + ticks(world, REGRAB_DELAY);
        for (i, t) in self.tentacles.iter_mut().enumerate() {
            let Mode::Hold {
                body,
                joint,
                strained,
            } = &mut t.mode
            else {
                continue;
            };
            if self.creatures.iter().any(|c| c.body == *body) {
                continue;
            }
            let saturated = world.joint(*joint).is_some_and(|j| j.saturated);
            *strained = if saturated { *strained + 1 } else { 0 };
            if *strained >= steal {
                let body = *body;
                world.remove_joint(*joint);
                t.mode = Mode::Idle;
                self.regrab.push((body, regrab));
                events.push(Event::Stolen { body, tentacle: i });
            }
        }
        if world.tick >= self.ends {
            events.extend(self.end(world));
        }
        events
    }

    /// End the spell: remove every capture, joint, and tentacle body, and
    /// free every creature.
    pub fn end(&mut self, world: &mut World) -> Vec<Event> {
        if self.ended {
            return Vec::new();
        }
        let (bodies, joints) = self.owned();
        for joint in joints {
            world.remove_joint(joint);
        }
        for body in bodies {
            world.remove_body(body);
        }
        for t in &mut self.tentacles {
            t.mode = Mode::Idle;
        }
        for c in &mut self.creatures {
            c.restrained = false;
        }
        self.ended = true;
        self.ends = self.ends.min(world.tick);
        vec![Event::Ended]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::trace::{Tolerance, Trace};
    use physics::{NoField, Uniform};

    const DT: f64 = 1.0 / 120.0;
    const DC: i32 = 15;
    const DOWN: DVec3 = DVec3::new(0.0, -9.81, 0.0);
    /// A Thunderwave push gives a grounded target this initial speed, m/s:
    /// `sqrt(2 a d)` for 10 feet under a 6 m/s^2 friction deceleration.
    const PUSH_SPEED: f64 = 6.05;
    /// Walking speed, m/s.
    const WALK: f64 = 1.5;

    fn floor(world: &mut World) {
        let id = world.add(
            Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static),
        );
        world.add_collider(Collider::new(
            id,
            Shape::Cuboid {
                half: DVec3::new(60.0, 0.5, 60.0),
            },
        ));
    }

    /// A 75 kg upright capsule creature standing at `(x, z)`.
    fn creature(world: &mut World, x: f64, z: f64) -> BodyId {
        let id = world.add(Body::new(75.0, DVec3::splat(1e6), DVec3::new(x, 0.9, z)));
        world.add_collider(
            Collider::new(
                id,
                Shape::Capsule {
                    radius: 0.35,
                    half_length: 0.55,
                },
            )
            .at(DVec3::ZERO, along(DVec3::Y)),
        );
        id
    }

    fn barrel(world: &mut World, x: f64, z: f64) -> BodyId {
        let half = DVec3::new(0.3, 0.45, 0.3);
        let id = world.add(Body::new(
            60.0,
            Body::box_inertia(60.0, half * 2.0),
            DVec3::new(x, 0.45, z),
        ));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
        id
    }

    fn creature_target(body: BodyId) -> Target {
        Target {
            body,
            kind: Kind::Creature {
                strength: 0,
                athletics: 0,
            },
        }
    }

    /// Dice that return `script` in order, then always 10.
    fn scripted(script: &[u32]) -> impl FnMut(u32) -> u32 + use<> {
        let mut script = script.to_vec();
        script.reverse();
        move |sides| script.pop().unwrap_or(10).min(sides)
    }

    /// A walker heading for `goal` at walking speed, slowed by the area,
    /// and still while Restrained.
    fn walk(world: &mut World, spell: &BlackTentacles, body: BodyId, goal: DVec3) {
        if spell.restrained(body) {
            return;
        }
        let b = &mut world[body];
        let mut to = goal - b.pos;
        to.y = 0.0;
        let speed = WALK * spell.speed_scale(b.pos);
        let v = to.clamp_length_max(speed.min(to.length() / DT));
        b.vel.x = v.x;
        b.vel.z = v.z;
        b.wake();
    }

    struct Scene {
        world: World,
        spell: BlackTentacles,
        targets: Vec<Target>,
        events: Vec<Event>,
        /// The tick of each event.
        at: Vec<u64>,
    }

    impl Scene {
        fn record(&mut self, events: Vec<Event>) {
            let tick = self.world.tick;
            self.at.extend(events.iter().map(|_| tick));
            self.events.extend(events);
        }

        fn last_save(&self, body: BodyId) -> u64 {
            let c = self.spell.creatures.iter().find(|c| c.body == body);
            c.and_then(|c| c.last_save).expect("saved")
        }

        fn step(&mut self, roll: &mut dyn FnMut(u32) -> u32, walkers: &[(BodyId, DVec3)]) {
            for &(body, goal) in walkers {
                walk(&mut self.world, &self.spell, body, goal);
            }
            let events = self
                .spell
                .before_step(&mut self.world, &self.targets, roll, None);
            self.record(events);
            self.world.step(&Uniform(DOWN));
            let events = self.spell.after_step(&mut self.world, None);
            self.record(events);
        }

        fn run(
            &mut self,
            seconds: f64,
            roll: &mut dyn FnMut(u32) -> u32,
            walkers: &[(BodyId, DVec3)],
        ) {
            for _ in 0..ticks(&self.world, seconds) {
                self.step(roll, walkers);
            }
        }

        fn saves(&self, body: BodyId) -> Vec<(Trigger, Save, Option<i32>)> {
            self.events
                .iter()
                .filter_map(|e| match *e {
                    Event::Save {
                        body: b,
                        trigger,
                        save,
                        damage,
                    } if b == body => Some((trigger, save, damage)),
                    _ => None,
                })
                .collect()
        }
    }

    fn scene(targets: Vec<Target>, world: World, roll: &mut dyn FnMut(u32) -> u32) -> Scene {
        let mut world = world;
        let (spell, events) =
            BlackTentacles::cast(&mut world, DVec3::ZERO, DOWN, DC, 7, &targets, roll);
        let at = vec![world.tick; events.len()];
        Scene {
            world,
            spell,
            targets,
            at,
            events,
        }
    }

    /// Assert two worlds carry the same saved state, naming the first
    /// body or joint that differs.
    fn same_world(a: &World, b: &World) {
        assert_eq!(a.tick, b.tick, "tick");
        assert_eq!(a.bodies().len(), b.bodies().len(), "body count");
        for (i, (x, y)) in a.bodies().iter().zip(b.bodies()).enumerate() {
            assert_eq!(x, y, "body {i}");
        }
        assert_eq!(a.colliders(), b.colliders(), "colliders");
        let (ja, jb): (Vec<_>, Vec<_>) = (a.joints().collect(), b.joints().collect());
        assert_eq!(ja.len(), jb.len(), "joint count");
        for (x, y) in ja.iter().zip(&jb) {
            assert_eq!(x, y, "joint {:?}", x.0);
        }
        let text = |w: &World| serde_json::to_string(w).unwrap();
        assert!(text(a) == text(b), "serialized worlds differ");
    }

    fn floored() -> World {
        let mut world = World::new(DT);
        floor(&mut world);
        world
    }

    #[test]
    fn the_area_is_a_twenty_foot_square_and_halves_speed_inside() {
        let mut world = floored();
        let (spell, _) = BlackTentacles::cast(
            &mut world,
            DVec3::ZERO,
            DOWN,
            DC,
            1,
            &[],
            &mut scripted(&[]),
        );
        assert!((SIDE - 6.096).abs() < 1e-12);
        let edge = SIDE / 2.0;
        assert!(spell.contains(DVec3::new(edge - 1e-6, 0.0, edge - 1e-6)));
        assert!(!spell.contains(DVec3::new(edge + 0.01, 0.0, 0.0)));
        assert!(!spell.contains(DVec3::new(0.0, 0.0, -edge - 0.01)));
        assert_eq!(spell.speed_scale(DVec3::ZERO), 0.5);
        assert_eq!(spell.speed_scale(DVec3::new(5.0, 0.0, 0.0)), 1.0);
        assert_eq!(spell.tentacles.len(), 9);
        assert!(spell.tentacles.iter().all(|t| t.segments.len() == SEGMENTS));
        let length: f64 = SEGMENT_LENGTH * SEGMENTS as f64;
        assert!((length - 2.4).abs() < 1e-12);
        let mass: f64 = spell.tentacles[0]
            .segments
            .iter()
            .map(|&s| world[s].mass)
            .sum();
        assert!((mass - 32.0).abs() < 1e-12);

        // A walker who passes its saves crosses at half speed inside.
        let mut world = floored();
        let walker = creature(&mut world, -5.0, 0.0);
        let mut roll = scripted(&[20, 20, 20, 20]);
        let mut s = scene(vec![creature_target(walker)], world, &mut roll);
        let goal = DVec3::new(8.0, 0.9, 0.0);
        s.run(1.0, &mut roll, &[(walker, goal)]);
        let outside = s.world[walker].pos.x;
        assert!((outside - (-5.0 + WALK)).abs() < 0.05, "{outside}");
        // Walk until well inside, then time one second there.
        while s.world[walker].pos.x < -1.0 {
            s.step(&mut roll, &[(walker, goal)]);
        }
        let x0 = s.world[walker].pos.x;
        s.run(1.0, &mut roll, &[(walker, goal)]);
        let inside = s.world[walker].pos.x - x0;
        // Commanded at half speed; brushing past the seeking tentacles may
        // slow it a little more, never speed it up.
        assert!(inside <= WALK * DIFFICULT_TERRAIN + 0.02, "{inside} inside");
        assert!(inside > WALK * DIFFICULT_TERRAIN * 0.7, "{inside} inside");
        assert!(!s.spell.restrained(walker));
    }

    #[test]
    fn placement_needs_range_and_level_ground() {
        let flat = |_: f64, _: f64| Some(0.0);
        let near = DVec3::new(10.0, 0.0, 0.0);
        assert_eq!(place(DVec3::ZERO, near, flat), Ok(near));
        let far = DVec3::new(RANGE + 0.1, 0.0, 0.0);
        assert_eq!(place(DVec3::ZERO, far, flat), Err(Refusal::OutOfRange));
        let ledge = |x: f64, _: f64| Some(if x > 11.0 { 6.0 } else { 0.0 });
        assert_eq!(place(DVec3::ZERO, near, ledge), Err(Refusal::Uneven));
        let chasm = |x: f64, _: f64| (x < 9.0).then_some(0.0);
        assert_eq!(
            place(DVec3::ZERO, DVec3::new(8.0, 0.0, 0.0), chasm),
            Err(Refusal::Uneven)
        );
        assert_eq!(
            place(DVec3::ZERO, near, |_, _| None),
            Err(Refusal::NoGround)
        );
    }

    #[test]
    fn saves_come_on_cast_entry_and_each_round_at_most_once_per_round() {
        let mut world = floored();
        let inside = creature(&mut world, 1.0, 1.0);
        let walker = creature(&mut world, -6.0, -2.0);
        // Every d20 is a 20, so every save passes; every d6 is a 1.
        let mut pass = |sides: u32| if sides == 20 { 20 } else { 1 };
        let mut s = scene(
            vec![creature_target(inside), creature_target(walker)],
            world,
            &mut pass,
        );
        let round = ticks(&s.world, ROUND);
        let cast = s.spell.cast;
        // The creature inside saved on the cast; the walker did not.
        assert_eq!(s.saves(inside).len(), 1);
        assert_eq!(s.saves(inside)[0].0, Trigger::Cast);
        assert!(s.saves(walker).is_empty());
        // The walker enters about 2 s in and saves.
        let into = DVec3::new(-2.0, 0.9, -2.0);
        s.run(3.0, &mut pass, &[(walker, into)]);
        let entry = s.saves(walker);
        assert_eq!(entry.len(), 1, "{entry:?}");
        assert_eq!(entry[0].0, Trigger::Enter);
        let first = s.last_save(walker);
        // It steps out and back in within the round: no second save.
        let out = DVec3::new(-6.0, 0.9, -2.0);
        s.run(2.0, &mut pass, &[(walker, out)]);
        assert!(
            !s.spell
                .creatures
                .iter()
                .any(|c| c.body == walker && c.inside)
        );
        s.run(1.5, &mut pass, &[(walker, into)]);
        assert!(
            s.spell
                .creatures
                .iter()
                .any(|c| c.body == walker && c.inside)
        );
        assert!(s.world.tick < first + round);
        assert_eq!(s.saves(walker).len(), 1, "{:?}", s.saves(walker));
        // The creature that stayed saved again exactly a round after the
        // cast.
        let saves = s.saves(inside);
        assert_eq!(saves.len(), 2, "{saves:?}");
        assert_eq!(saves[1].0, Trigger::EndOfTurn);
        assert_eq!(s.last_save(inside), cast + round);
        // A round later both have saved again on their own clocks, and no
        // two saves by one creature are less than a round apart.
        s.run(6.0, &mut pass, &[]);
        assert_eq!(s.saves(inside).len(), 3);
        assert_eq!(s.last_save(inside), cast + 2 * round);
        let saves = s.saves(walker);
        assert_eq!(saves.len(), 2, "{saves:?}");
        assert_eq!(saves[1].0, Trigger::EndOfTurn);
        for body in [inside, walker] {
            let at: Vec<u64> = s
                .events
                .iter()
                .zip(&s.at)
                .filter(|(e, _)| matches!(e, Event::Save { body: b, .. } if *b == body))
                .map(|(_, &t)| t)
                .collect();
            assert!(at.windows(2).all(|w| w[1] >= w[0] + round), "{at:?}");
        }
    }

    #[test]
    fn a_failure_deals_three_d6_and_restrains_with_two_or_three_tentacles() {
        let mut world = floored();
        let dummy = creature(&mut world, 0.5, 0.3);
        // d20 = 3 fails; the 3d6 are 4, 5, and 6.
        let mut roll = scripted(&[3, 4, 5, 6]);
        let mut s = scene(vec![creature_target(dummy)], world, &mut roll);
        let saves = s.saves(dummy);
        assert_eq!(saves.len(), 1);
        let (trigger, save, damage) = saves[0];
        assert_eq!(trigger, Trigger::Cast);
        assert!(!save.success);
        assert_eq!(damage, Some(15));
        assert!(s.spell.restrained(dummy));
        s.run(2.0, &mut roll, &[(dummy, DVec3::new(9.0, 0.9, 0.3))]);
        let wraps = s.spell.captures(dummy);
        assert!((2..=3).contains(&wraps.len()), "{wraps:?}");
        // Restrained, it did not walk.
        assert!(s.world[dummy].pos.distance(DVec3::new(0.5, 0.9, 0.3)) < 0.3);
        // 3d6 is 3 to 18 for any dice.
        let mut low = scripted(&[1, 1, 1, 1]);
        let mut high = scripted(&[6, 6, 6]);
        assert_eq!(dice(3, 6, &mut low), 3);
        assert_eq!(dice(3, 6, &mut high), 18);
    }

    #[test]
    fn a_thunderwave_push_stretches_the_tentacles_but_does_not_free() {
        let mut world = floored();
        let dummy = creature(&mut world, 0.6, 0.4);
        let mut roll = scripted(&[2, 3, 3, 3]);
        let mut s = scene(vec![creature_target(dummy)], world, &mut roll);
        s.run(2.0, &mut roll, &[]);
        assert_eq!(s.spell.captures(dummy).len(), 3);
        let hold = s.world[dummy].pos;
        // Push 10 feet's worth of speed away from a caster on -x.
        let b = &mut s.world[dummy];
        b.apply_impulse_at(DVec3::X * PUSH_SPEED * 75.0, b.pos);
        let mut farthest: f64 = 0.0;
        let mut strained = false;
        for _ in 0..ticks(&s.world, 3.0) {
            s.step(&mut roll, &[]);
            farthest = farthest.max(s.world[dummy].pos.distance(hold));
            strained |= s
                .spell
                .captures(dummy)
                .iter()
                .any(|&j| s.world.joint(j).unwrap().impulse.length() > 1.0);
        }
        // It moved visibly, far less than the free 3 m, and came back.
        assert!(farthest > 0.1 && farthest < 1.5, "{farthest}");
        assert!(strained);
        assert!(
            s.world[dummy].pos.distance(hold) < 0.25,
            "{}",
            s.world[dummy].pos
        );
        assert!(s.spell.restrained(dummy));
        assert_eq!(s.spell.captures(dummy).len(), 3);
    }

    #[test]
    fn a_successful_escape_frees_and_the_creature_walks_out() {
        let mut world = floored();
        let dummy = creature(&mut world, 0.5, 0.0);
        let mut roll = scripted(&[1, 2, 2, 2]);
        let mut s = scene(vec![creature_target(dummy)], world, &mut roll);
        s.run(2.0, &mut roll, &[]);
        assert!(s.spell.restrained(dummy));
        // A failed check changes nothing.
        let fail = s.spell.escape(&mut s.world, dummy, 5, 2).unwrap();
        assert!(matches!(fail, Event::Escape { check, .. } if !check.success));
        assert!(s.spell.restrained(dummy));
        assert_eq!(s.spell.captures(dummy).len(), 3);
        let joints = s.world.joints().count();
        let success = s.spell.escape(&mut s.world, dummy, 13, 2).unwrap();
        assert!(matches!(success, Event::Escape { check, .. } if check.success));
        assert!(!s.spell.restrained(dummy));
        assert!(s.spell.captures(dummy).is_empty());
        assert_eq!(s.world.joints().count(), joints - 3);
        // It walks out at half speed and is not grabbed on the way.
        let goal = DVec3::new(8.0, 0.9, 0.0);
        s.run(5.0, &mut roll, &[(dummy, goal)]);
        assert!(s.world[dummy].pos.x > SIDE / 2.0, "{}", s.world[dummy].pos);
        assert!(s.spell.captures(dummy).is_empty());
        assert!(!s.spell.restrained(dummy));
    }

    #[test]
    fn props_are_grabbed_without_a_save_and_telekinesis_steals_them() {
        let mut world = floored();
        let prop = barrel(&mut world, 1.2, -0.8);
        let secured = barrel(&mut world, -1.5, 1.5);
        let mut roll = scripted(&[]);
        let mut s = scene(
            vec![
                Target {
                    body: prop,
                    kind: Kind::Prop { secured: false },
                },
                Target {
                    body: secured,
                    kind: Kind::Prop { secured: true },
                },
            ],
            world,
            &mut roll,
        );
        s.run(4.0, &mut roll, &[]);
        assert!(s.events.iter().all(|e| !matches!(e, Event::Save { .. })));
        let holders = s.spell.captures(prop);
        assert!(!holders.is_empty() && holders.len() <= PROP_HOLDERS);
        assert!(s.spell.captures(secured).is_empty());
        // Still held two seconds later.
        s.run(2.0, &mut roll, &[]);
        assert!(!s.spell.captures(prop).is_empty());
        // A Telekinesis hand on a Small object pulls with up to 300 kg at
        // twice gravity, more than the grip: it steals the barrel.
        let hand = s
            .world
            .add(Body::new(0.0, DVec3::ONE, s.world[prop].pos).with_kind(BodyKind::Kinematic));
        let limit = 300.0 * 2.0 * 9.81;
        s.world.add_joint(
            Joint::new(hand, DVec3::ZERO, prop, DVec3::ZERO, JointKind::Point)
                .soft(std::f64::consts::TAU * 4.0, 1.0)
                .limited(limit, limit),
        );
        let mut stolen = false;
        for _ in 0..ticks(&s.world, 4.0) {
            let h = &mut s.world[hand];
            h.vel = if h.pos.x < 8.0 {
                DVec3::X * 3.0
            } else {
                DVec3::ZERO
            };
            s.step(&mut roll, &[]);
            stolen |= s.events.iter().any(|e| matches!(e, Event::Stolen { .. }));
        }
        assert!(stolen);
        assert!(s.spell.captures(prop).is_empty());
        assert!(s.world[prop].pos.x > SIDE / 2.0, "{}", s.world[prop].pos);
    }

    #[test]
    fn the_end_removes_every_tentacle_body_and_joint() {
        let mut world = floored();
        let dummy = creature(&mut world, 0.4, 0.4);
        let prop = barrel(&mut world, -1.5, -1.2);
        let bodies_before = world.bodies().len();
        let mut roll = scripted(&[1, 1, 1, 1]);
        let mut s = scene(
            vec![
                creature_target(dummy),
                Target {
                    body: prop,
                    kind: Kind::Prop { secured: false },
                },
            ],
            world,
            &mut roll,
        );
        s.run(3.0, &mut roll, &[]);
        assert!(!s.spell.captures(dummy).is_empty());
        assert!(!s.spell.captures(prop).is_empty());
        assert!(s.world.joints().count() > 9 * SEGMENTS * 2);
        // Concentration runs out after a minute.
        s.run(CONCENTRATION - 3.0, &mut roll, &[]);
        assert!(s.spell.ended);
        assert!(s.events.contains(&Event::Ended));
        assert_eq!(s.world.joints().count(), 0);
        let (bodies, _) = s.spell.owned();
        assert_eq!(bodies.len(), 9 * (SEGMENTS + 1));
        assert!(bodies.iter().all(|&b| s.world[b].removed));
        assert!(s.world.bodies()[..bodies_before].iter().all(|b| !b.removed));
        assert!(!s.spell.restrained(dummy));
        assert!(!s.spell.contains(DVec3::ZERO));
        // Everything drops free.
        s.run(2.0, &mut roll, &[]);
        assert!(s.world[prop].pos.y < 0.5, "{}", s.world[prop].pos);
    }

    #[test]
    fn the_tentacles_rise_from_the_ground_and_writhe() {
        let mut world = floored();
        let mut roll = scripted(&[]);
        let (mut spell, _) =
            BlackTentacles::cast(&mut world, DVec3::ZERO, DOWN, DC, 3, &[], &mut roll);
        let tips = |w: &World, s: &BlackTentacles| -> Vec<DVec3> {
            s.tentacles.iter().map(|t| t.tip_end(w)).collect()
        };
        assert!(tips(&world, &spell).iter().all(|p| p.y < 0.3));
        let mut last = tips(&world, &spell);
        let mut moved = 0.0;
        for i in 0..ticks(&world, 4.0) {
            spell.before_step(&mut world, &[], &mut roll, None);
            world.step(&Uniform(DOWN));
            spell.after_step(&mut world, None);
            if i > 240 {
                let now = tips(&world, &spell);
                moved += now
                    .iter()
                    .zip(&last)
                    .map(|(a, b)| a.distance(*b))
                    .sum::<f64>();
            }
            last = tips(&world, &spell);
        }
        for (t, tip) in spell.tentacles.iter().zip(tips(&world, &spell)) {
            assert!(tip.y > 1.2, "{tip}");
            assert!(tip.distance(t.root) <= LENGTH + 0.05, "{tip}");
            // Every ball joint holds.
            for &j in t.joints.iter().step_by(2) {
                let (a, b) = world.joint(j).unwrap().anchors(&world);
                assert!(a.distance(b) < 0.02, "{}", a.distance(b));
            }
        }
        assert!(moved > 2.0, "{moved}");
    }

    #[test]
    fn the_ledger_balances() {
        let mut world = World::new(DT);
        let mut roll = scripted(&[]);
        let (mut spell, _) =
            BlackTentacles::cast(&mut world, DVec3::ZERO, DOWN, DC, 5, &[], &mut roll);
        let origin = DVec3::new(0.5, 1.0, -0.25);
        let mut ledger = Ledger::new(origin, world.momentum(origin));
        for _ in 0..ticks(&world, 2.0) {
            spell.before_step(&mut world, &[], &mut roll, Some(&mut ledger));
            world.step(&NoField);
            spell.after_step(&mut world, Some(&mut ledger));
        }
        let error = ledger.error(world.momentum(origin));
        assert!(error.linear < 1e-6 && error.angular < 1e-6, "{error:?}");
        assert!(ledger.external[LEDGER_TERM].linear.length() > 1.0);
    }

    #[test]
    fn a_checkpoint_mid_grab_replays_identically() {
        let mut world = floored();
        let dummy = creature(&mut world, 0.4, -0.6);
        let walker = creature(&mut world, -5.0, 1.0);
        let prop = barrel(&mut world, 1.5, 1.5);
        let targets = vec![
            creature_target(dummy),
            creature_target(walker),
            Target {
                body: prop,
                kind: Kind::Prop { secured: false },
            },
        ];
        let mut roll = scripted(&[2, 3, 4, 5]);
        let mut s = scene(targets.clone(), world, &mut roll);
        let goal = DVec3::new(4.0, 0.9, 1.0);
        s.run(2.0, &mut roll, &[(walker, goal)]);
        assert!(
            s.spell
                .tentacles
                .iter()
                .any(|t| matches!(t.mode, Mode::Hold { .. }))
        );
        let saved = serde_json::to_string(&(&s.world, &s.spell)).unwrap();
        let (world2, spell2): (World, BlackTentacles) = serde_json::from_str(&saved).unwrap();
        // The last step's contact reports are output only and not saved, so
        // compare what a checkpoint carries.
        same_world(&world2, &s.world);
        assert_eq!(spell2, s.spell);
        let mut s2 = Scene {
            world: world2,
            spell: spell2,
            targets,
            events: Vec::new(),
            at: Vec::new(),
        };
        let (mut a, mut b) = (Trace::default(), Trace::default());
        for (scene, trace) in [(&mut s, &mut a), (&mut s2, &mut b)] {
            let mut n = 0u32;
            let mut dice = |sides: u32| {
                n += 7;
                n % sides + 1
            };
            for i in 0..600 {
                scene.step(&mut dice, &[(walker, goal)]);
                if i == 300 {
                    scene.spell.escape(&mut scene.world, dummy, 20, 0);
                }
                trace.record(&scene.world);
            }
        }
        a.compare(&b, Tolerance::EXACT).unwrap();
        assert_eq!(s.spell, s2.spell);
    }
}
