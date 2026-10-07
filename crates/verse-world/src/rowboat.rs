//! Rowboats (`docs/verse/water.md`, Rowboats; phase W6): buoyant boats a
//! player boards, rows, capsizes, rights, and breaks.
//!
//! A [`Fleet`] holds the boats on one [`physics::World`] with the zone's
//! beds and banks as static boxes, and steps them at 120 Hz with
//! `physics::water`'s buoyancy and drag. Each boat is one body: a compound
//! of cuboids (the bottom, the two sides, the bow, the stern, and two
//! thwarts) for contact, and the hull's box for buoyancy, so the water the
//! hull keeps out lifts it (Archimedes). Its masses give a draft of about
//! 0.12 m empty and 0.2 m with two aboard.
//!
//! - **Seats.** A rower and one passenger. Boarding needs the boat upright
//!   and within [`BOARD_REACH`]: at once from land, or after a
//!   [`CLIMB_SECONDS`] climb from the water. An occupant's weight presses
//!   on its seat.
//! - **Rowing.** The rower's [`Intent`] pulls each oar once a stroke, at
//!   [`STROKE_RATE`] strokes a second: an impulse at each oarlock, ahead or
//!   astern, one oar alone to turn. A keel force resists sliding sideways.
//!   On still water the boat tops out near 1.5 m/s.
//! - **Capsizing.** Rolled past [`CAPSIZE_ROLL`] with a gunwale under, a
//!   boat capsizes, its occupants fall in swimming, and it floats upside
//!   down until a swimmer beside it rights it.
//! - **Breaking.** At zero hit points a boat breaks: its occupants fall in
//!   and each plank floats away as a body of its own.
//! - **Authority.** The rower's client is the boat's host: its inputs are
//!   intents to its own simulation, and its reports ([`Report`], NIP-MV's
//!   shared-body entries) carry the pose others snap to. A passenger rides
//!   its seat and publishes no pose of its own.
//!
//! Nothing here draws or reads a key; the zone does.

use glam::{DQuat, DVec2, DVec3, Vec3};
use physics::water::{Settings, Water, apply_scaled};
use physics::{Body, BodyId, BodyKind, Collider, Filter, Shape, Uniform, World};

/// The physics step, s.
pub const STEP: f64 = 1.0 / 120.0;
/// How near a boat a player must be to board it, or a swimmer to right it,
/// m from the hull.
pub const BOARD_REACH: f32 = 2.0;
/// How long climbing in from the water takes, s.
pub const CLIMB_SECONDS: f32 = 1.5;
/// Strokes a second.
pub const STROKE_RATE: f32 = 0.8;
/// Roll past which a boat with a gunwale under capsizes, rad (60°).
pub const CAPSIZE_ROLL: f64 = std::f64::consts::FRAC_PI_3;
/// An occupant's mass, kg.
pub const OCCUPANT: f64 = 80.0;
/// The hull's mass, kg, with its center of mass this far over the keel, m.
pub const HULL_MASS: f64 = 300.0;
const KEEL_TO_MASS: f64 = 0.12;
/// The hull's outside: half its beam, depth, and length, m.
pub const HALF: DVec3 = DVec3::new(0.46, 0.21, 1.35);
/// The impulse each oar gives a full stroke, N s.
pub const OAR_IMPULSE: f64 = 80.0;
/// Half a plank's thickness, m.
const PLANK: f64 = 0.025;
/// How far out each oarlock sits from the centerline, m.
const OARLOCK: f64 = 0.55;
/// How hard the keel resists sliding sideways: a rate, 1/s.
const KEEL: f64 = 1.6;
/// The hit points of a boat.
pub const HIT_POINTS: i32 = 18;
/// Wood's density, kg/m³, for the planks of a broken boat.
pub const WOOD: f64 = 600.0;
/// How many steps a frame runs at most.
const MOST_STEPS: usize = 12;
/// A seated occupant sits this far over the thwart, m.
const SIT: f64 = 0.05;

/// The hull's drag: far less than the generic body's, since a hull is
/// shaped to slip through the water ahead.
pub const DRAG: Settings = Settings {
    gravity: DVec3::new(0.0, -9.81, 0.0),
    linear: 4.0,
    quadratic: 0.05,
    angular: 1.2,
};

/// A seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Seat {
    Rower,
    Passenger,
}

impl Seat {
    const ALL: [Self; 2] = [Self::Rower, Self::Passenger];

    fn index(self) -> usize {
        match self {
            Self::Rower => 0,
            Self::Passenger => 1,
        }
    }

    /// The seat's point in the boat's frame over its keel, m: on its
    /// thwart.
    #[must_use]
    pub fn local(self) -> DVec3 {
        match self {
            Self::Rower => DVec3::new(0.0, 0.32 + SIT, 0.1),
            Self::Passenger => DVec3::new(0.0, 0.32 + SIT, -0.85),
        }
    }
}

/// What the rower wants: ahead (+1) or astern (−1), and turning left (−1)
/// or right (+1) by pulling one oar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intent {
    pub ahead: f32,
    pub turn: f32,
}

impl Intent {
    fn idle(&self) -> bool {
        self.ahead.abs() < 0.01 && self.turn.abs() < 0.01
    }
}

/// Where a boat is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Upright,
    Capsized,
    Broken,
}

impl State {
    /// The state's name in a report.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Upright => "upright",
            Self::Capsized => "capsized",
            Self::Broken => "broken",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Upright, Self::Capsized, Self::Broken]
            .into_iter()
            .find(|s| s.name() == name)
    }
}

/// Someone on a seat, or climbing in toward it.
#[derive(Clone, Debug, PartialEq)]
pub struct Occupant {
    pub who: String,
    /// Seconds of the climb from the water left; 0 once seated.
    pub climbing: f32,
}

/// One rowboat.
#[derive(Clone, Debug)]
pub struct Boat {
    /// Its shared-body id, such as `rowboat-0`.
    pub id: String,
    pub body: BodyId,
    pub state: State,
    pub seats: [Option<Occupant>; 2],
    pub intent: Intent,
    /// Seconds until the next stroke's pull.
    stroke: f32,
    pub strokes: u32,
    pub hit_points: i32,
    /// The planks of a broken boat.
    pub planks: Vec<BodyId>,
    /// Where it was moored, over its keel, and its heading.
    pub home: (DVec3, f64),
    /// Whose simulation the others follow, and the stamp of its motion.
    pub owner: Option<String>,
    pub stamp: [u64; 2],
}

/// What happened, for the zone's effects and log.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Boarded {
        boat: usize,
        who: String,
        seat: Seat,
    },
    Left {
        boat: usize,
        who: String,
    },
    Stroke {
        boat: usize,
        at: Vec3,
    },
    /// The occupants fell in.
    Capsized {
        boat: usize,
        dumped: Vec<String>,
    },
    Righted {
        boat: usize,
    },
    Broken {
        boat: usize,
        dumped: Vec<String>,
    },
}

/// Why a boarding or righting was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    TooFar,
    NotUpright,
    Full,
    Upright,
    Aboard,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooFar => "The boat is out of reach",
            Self::NotUpright => "The boat must be upright to board",
            Self::Full => "Both seats are taken",
            Self::Upright => "The boat is already upright",
            Self::Aboard => "You are already aboard",
        })
    }
}

/// A boat's pose as its host reports it, NIP-MV's shared-body entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub id: String,
    pub pos: DVec3,
    pub rot: DQuat,
    pub vel: DVec3,
    pub omega: DVec3,
    /// `[epoch, rev]`.
    pub stamp: [u64; 2],
    pub state: State,
    pub rest: bool,
    /// Whether the host sits at the oars.
    pub rowing: bool,
}

/// The zone's rowboats and the world they float in.
pub struct Fleet {
    world: World,
    water: &'static (dyn Water + Sync),
    pub boats: Vec<Boat>,
    pending: f64,
    events: Vec<Event>,
}

/// The collider filter of the beds and banks.
const BED: Filter = Filter {
    group: 1,
    mask: u32::MAX,
};

impl Fleet {
    /// Boats moored at `moorings` (each over the water, a heading as the
    /// controller's yaw: 0 is +z), on `water`, over `bed`: static boxes,
    /// each a center and half extents, for the beds and banks.
    #[must_use]
    pub fn new(
        water: &'static (dyn Water + Sync),
        bed: &[(DVec3, DVec3)],
        moorings: &[(DVec2, f64)],
    ) -> Self {
        let mut world = World::new(STEP);
        let ground = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
        for &(center, half) in bed {
            world.add_collider(
                Collider::new(ground, Shape::Cuboid { half })
                    .at(center, DQuat::IDENTITY)
                    .with_filter(BED),
            );
        }
        let mut fleet = Self {
            world,
            water,
            boats: Vec::new(),
            pending: 0.0,
            events: Vec::new(),
        };
        for (k, &(at, yaw)) in moorings.iter().enumerate() {
            let level = water.sample(at.x, at.y, 0).map_or(0.0, |s| s.height);
            let keel = DVec3::new(at.x, level - 0.12, at.y);
            let body = fleet.add_hull(keel, yaw);
            fleet.boats.push(Boat {
                id: format!("rowboat-{k}"),
                body,
                state: State::Upright,
                seats: [None, None],
                intent: Intent::default(),
                stroke: 0.0,
                strokes: 0,
                hit_points: HIT_POINTS,
                planks: Vec::new(),
                home: (keel, yaw),
                owner: None,
                stamp: [0, 0],
            });
        }
        fleet
    }

    /// The hull's contact boxes in its frame over the keel: the bottom,
    /// the sides, the bow, the stern, and the two thwarts.
    #[must_use]
    pub fn planks() -> [(DVec3, DVec3); 7] {
        let h = HALF;
        let t = PLANK;
        [
            (DVec3::new(0.0, t, 0.0), DVec3::new(h.x - t, t, h.z - t)),
            (DVec3::new(h.x - t, h.y, 0.0), DVec3::new(t, h.y, h.z)),
            (DVec3::new(-h.x + t, h.y, 0.0), DVec3::new(t, h.y, h.z)),
            (
                DVec3::new(0.0, h.y, h.z - t),
                DVec3::new(h.x - 2.0 * t, h.y, t),
            ),
            (
                DVec3::new(0.0, h.y, -h.z + t),
                DVec3::new(h.x - 2.0 * t, h.y, t),
            ),
            (
                Seat::Rower.local() - DVec3::new(0.0, SIT + 0.02, 0.0),
                DVec3::new(h.x - 2.0 * t, 0.02, 0.12),
            ),
            (
                Seat::Passenger.local() - DVec3::new(0.0, SIT + 0.02, 0.0),
                DVec3::new(h.x - 2.0 * t, 0.02, 0.12),
            ),
        ]
    }

    fn add_hull(&mut self, keel: DVec3, yaw: f64) -> BodyId {
        let rot = DQuat::from_rotation_y(yaw);
        let com = DVec3::new(0.0, KEEL_TO_MASS, 0.0);
        let mut body = Body::new(
            HULL_MASS,
            Body::box_inertia(HULL_MASS, HALF * 2.0),
            keel + rot * com,
        );
        body.orientation = rot;
        body.prev_orientation = rot;
        let id = self.world.add(body);
        for (center, half) in Self::planks() {
            self.world.add_collider(
                Collider::new(id, Shape::Cuboid { half }).at(center - com, DQuat::IDENTITY),
            );
        }
        // The space inside the planks, for buoyancy alone: the water the
        // hull keeps out lifts it as much as its walls do.
        let t = 2.0 * PLANK;
        let inside = DVec3::new(HALF.x - t, HALF.y - PLANK, HALF.z - t);
        self.world.add_collider(
            Collider::new(id, Shape::Cuboid { half: inside })
                .at(DVec3::new(0.0, t + inside.y, 0.0) - com, DQuat::IDENTITY)
                .with_filter(Filter::NONE),
        );
        id
    }

    /// The boats' world.
    #[must_use]
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Takes what happened since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Boat `k`'s frame over its keel: the keel's point and the rotation.
    #[must_use]
    pub fn frame(&self, k: usize) -> (DVec3, DQuat) {
        let b = &self.world[self.boats[k].body];
        let com = DVec3::new(0.0, KEEL_TO_MASS, 0.0);
        (b.pos - b.orientation * com, b.orientation)
    }

    /// Boat `k`'s heading as the controller's yaw.
    #[must_use]
    pub fn yaw(&self, k: usize) -> f64 {
        let f = self.frame(k).1 * DVec3::Z;
        f.x.atan2(f.z)
    }

    /// How far boat `k` rolls or pitches from upright, rad.
    #[must_use]
    pub fn tilt(&self, k: usize) -> f64 {
        let up = self.frame(k).1 * DVec3::Y;
        up.dot(DVec3::Y).clamp(-1.0, 1.0).acos()
    }

    /// How deep boat `k`'s keel lies under the water at its middle, m.
    #[must_use]
    pub fn draft(&self, k: usize) -> f64 {
        let (keel, _) = self.frame(k);
        self.water
            .sample(keel.x, keel.z, self.world.tick)
            .map_or(0.0, |s| s.height - keel.y)
    }

    /// Boat `k`'s speed over the water, m/s.
    #[must_use]
    pub fn speed(&self, k: usize) -> f64 {
        let v = self.world[self.boats[k].body].vel;
        DVec2::new(v.x, v.z).length()
    }

    /// Where `seat` of boat `k` is in the world, and its rotation.
    #[must_use]
    pub fn seat_pose(&self, k: usize, seat: Seat) -> (DVec3, DQuat) {
        let (keel, rot) = self.frame(k);
        (keel + rot * seat.local(), rot)
    }

    /// The seat `who` holds, and in which boat, seated or climbing.
    #[must_use]
    pub fn seat_of(&self, who: &str) -> Option<(usize, Seat, bool)> {
        self.boats.iter().enumerate().find_map(|(k, b)| {
            Seat::ALL.into_iter().find_map(|seat| {
                b.seats[seat.index()]
                    .as_ref()
                    .filter(|o| o.who == who)
                    .map(|o| (k, seat, o.climbing <= 0.0))
            })
        })
    }

    /// How far `at` lies from boat `k`'s hull in the horizontal plane, m.
    #[must_use]
    pub fn reach(&self, k: usize, at: Vec3) -> f32 {
        if self.boats[k].state == State::Broken {
            return f32::INFINITY;
        }
        let (keel, rot) = self.frame(k);
        let local = rot.inverse() * (at.as_dvec3() - keel);
        let dx = (local.x.abs() - HALF.x).max(0.0);
        let dz = (local.z.abs() - HALF.z).max(0.0);
        dx.hypot(dz) as f32
    }

    /// The boat nearest `at` within [`BOARD_REACH`], if any.
    #[must_use]
    pub fn near(&self, at: Vec3) -> Option<usize> {
        (0..self.boats.len())
            .map(|k| (k, self.reach(k, at)))
            .filter(|(_, d)| *d <= BOARD_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    /// `who` at `at` boards boat `k`: into the rower's seat if free, else
    /// the passenger's; at once from land, or climbing in for
    /// [`CLIMB_SECONDS`] when `swimming`.
    ///
    /// # Errors
    ///
    /// Refuses a boat out of reach, not upright, or full, and a player
    /// already aboard one.
    pub fn board(
        &mut self,
        k: usize,
        who: &str,
        at: Vec3,
        swimming: bool,
    ) -> Result<Seat, Refusal> {
        if self.seat_of(who).is_some() {
            return Err(Refusal::Aboard);
        }
        if self.boats[k].state != State::Upright {
            return Err(Refusal::NotUpright);
        }
        if self.reach(k, at) > BOARD_REACH {
            return Err(Refusal::TooFar);
        }
        let boat = &mut self.boats[k];
        let seat = Seat::ALL
            .into_iter()
            .find(|s| boat.seats[s.index()].is_none())
            .ok_or(Refusal::Full)?;
        boat.seats[seat.index()] = Some(Occupant {
            who: who.to_owned(),
            climbing: if swimming { CLIMB_SECONDS } else { 0.0 },
        });
        if seat == Seat::Rower {
            boat.owner = Some(who.to_owned());
            boat.stamp[1] += 1;
        }
        let body = boat.body;
        self.world.wake(body);
        if !swimming {
            self.events.push(Event::Boarded {
                boat: k,
                who: who.to_owned(),
                seat,
            });
        }
        Ok(seat)
    }

    /// `who` leaves its seat. Returns the boat and seat it left.
    pub fn leave(&mut self, who: &str) -> Option<(usize, Seat)> {
        let (k, seat, _) = self.seat_of(who)?;
        let boat = &mut self.boats[k];
        boat.seats[seat.index()] = None;
        if seat == Seat::Rower {
            boat.intent = Intent::default();
        }
        self.events.push(Event::Left {
            boat: k,
            who: who.to_owned(),
        });
        Some((k, seat))
    }

    /// Sets the rower's intent; anyone else's is ignored.
    pub fn row(&mut self, who: &str, intent: Intent) {
        if let Some((k, Seat::Rower, true)) = self.seat_of(who) {
            let intent = Intent {
                ahead: intent.ahead.clamp(-1.0, 1.0),
                turn: intent.turn.clamp(-1.0, 1.0),
            };
            let boat = &mut self.boats[k];
            if boat.intent.idle() && !intent.idle() {
                // The first pull comes at once.
                boat.stroke = 0.0;
            }
            boat.intent = intent;
        }
    }

    /// A swimmer at `at` rights capsized boat `k`.
    ///
    /// # Errors
    ///
    /// Refuses a boat out of reach or not capsized.
    pub fn right(&mut self, k: usize, at: Vec3) -> Result<(), Refusal> {
        if self.boats[k].state != State::Capsized {
            return Err(Refusal::Upright);
        }
        if self.reach(k, at) > BOARD_REACH {
            return Err(Refusal::TooFar);
        }
        let yaw = self.yaw(k);
        let (keel, _) = self.frame(k);
        let level = self
            .water
            .sample(keel.x, keel.z, self.world.tick)
            .map_or(keel.y, |s| s.height);
        let rot = DQuat::from_rotation_y(yaw);
        let body = &mut self.world[self.boats[k].body];
        body.orientation = rot;
        body.prev_orientation = rot;
        body.pos =
            DVec3::new(keel.x, level - 0.1, keel.z) + rot * DVec3::new(0.0, KEEL_TO_MASS, 0.0);
        body.vel = DVec3::ZERO;
        body.omega = DVec3::ZERO;
        body.wake();
        let boat = &mut self.boats[k];
        boat.state = State::Upright;
        boat.stamp[1] += 1;
        self.events.push(Event::Righted { boat: k });
        Ok(())
    }

    /// Strikes boat `k` for `damage` hit points; at zero it breaks.
    /// Returns whether it broke.
    pub fn damage(&mut self, k: usize, damage: i32) -> bool {
        let boat = &mut self.boats[k];
        if boat.state == State::Broken || damage <= 0 {
            return false;
        }
        boat.hit_points -= damage;
        if boat.hit_points > 0 {
            return false;
        }
        self.break_up(k);
        true
    }

    /// Damages every boat whose hull lies within `radius` of `at`, as a
    /// hammer's blow or a meteor's blast does. Returns the boats broken.
    pub fn strike(&mut self, at: Vec3, radius: f32, damage: i32) -> Vec<usize> {
        (0..self.boats.len())
            .filter(|&k| self.reach(k, at) <= radius && self.damage(k, damage))
            .collect()
    }

    fn dump(&mut self, k: usize) -> Vec<String> {
        let boat = &mut self.boats[k];
        boat.intent = Intent::default();
        boat.seats
            .iter_mut()
            .filter_map(Option::take)
            .map(|o| o.who)
            .collect()
    }

    fn break_up(&mut self, k: usize) {
        let dumped = self.dump(k);
        let (keel, rot) = self.frame(k);
        let hull = self.boats[k].body;
        let (vel, omega) = (self.world[hull].vel, self.world[hull].omega);
        self.world.remove_body(hull);
        let mut planks = Vec::new();
        for (i, (center, half)) in Self::planks().into_iter().enumerate() {
            let mass = WOOD * 8.0 * half.x * half.y * half.z;
            let pos = keel + rot * center;
            let mut body = Body::new(mass, Body::box_inertia(mass, half * 2.0), pos);
            body.orientation = rot;
            body.prev_orientation = rot;
            // A little spread, so the planks part.
            let out = rot * DVec3::new(center.x.signum() * 0.4, 0.6, center.z * 0.3);
            body.vel = vel + out * (0.6 + 0.1 * i as f64);
            body.omega = omega + DVec3::new(0.3, 0.7, -0.4) * (i as f64 - 3.0) * 0.2;
            let id = self.world.add(body);
            self.world
                .add_collider(Collider::new(id, Shape::Cuboid { half }));
            planks.push(id);
        }
        let boat = &mut self.boats[k];
        boat.state = State::Broken;
        boat.planks = planks;
        boat.stamp[1] += 1;
        self.events.push(Event::Broken { boat: k, dumped });
    }

    /// Advances the boats by `dt` seconds of wall time.
    pub fn tick(&mut self, dt: f32) {
        // Climbs finish on the wall clock.
        for k in 0..self.boats.len() {
            for seat in Seat::ALL {
                let done = match &mut self.boats[k].seats[seat.index()] {
                    Some(o) if o.climbing > 0.0 => {
                        o.climbing -= dt;
                        (o.climbing <= 0.0).then(|| o.who.clone())
                    }
                    _ => None,
                };
                if let Some(who) = done {
                    self.events.push(Event::Boarded { boat: k, who, seat });
                }
            }
        }
        self.pending += f64::from(dt.clamp(0.0, 0.25));
        let mut steps = 0;
        while self.pending >= STEP && steps < MOST_STEPS {
            self.pending -= STEP;
            steps += 1;
            self.step();
        }
        if steps == MOST_STEPS {
            self.pending = 0.0;
        }
    }

    /// One physics step: weights on the seats, strokes, the keel, water,
    /// and capsizing.
    fn step(&mut self) {
        let dt = STEP;
        for k in 0..self.boats.len() {
            let state = self.boats[k].state;
            if state == State::Broken {
                continue;
            }
            let id = self.boats[k].body;
            let (keel, rot) = self.frame(k);
            if state == State::Upright {
                // Each seated occupant's weight presses on its seat.
                for seat in Seat::ALL {
                    if self.boats[k].seats[seat.index()].is_some() {
                        let at = keel + rot * (seat.local() + DVec3::new(0.0, 0.25, 0.0));
                        self.world[id].apply_force_at(DVec3::new(0.0, -9.81 * OCCUPANT, 0.0), at);
                    }
                }
                self.pull(k, keel, rot);
            } else {
                // Capsized: the trapped air holds it keel up.
                let up = rot * DVec3::Y;
                let error = up.dot(-DVec3::Y).clamp(-1.0, 1.0).acos();
                let axis = up
                    .cross(-DVec3::Y)
                    .try_normalize()
                    .unwrap_or(rot * DVec3::Z);
                let body = &mut self.world[id];
                let torque = axis * error * 2500.0 - body.omega_world() * 400.0;
                body.apply_torque(torque);
                body.wake();
            }
            // The keel: the water resists the hull sliding sideways.
            let wet = self
                .water
                .sample(keel.x, keel.z, self.world.tick)
                .is_some_and(|s| s.height > keel.y);
            let body = &mut self.world[id];
            let side = rot * DVec3::X;
            let slide = body.vel.dot(side);
            if wet {
                body.apply_force(-side * slide * KEEL * body.mass);
            }
        }
        let water = self.water;
        let tick = self.world.tick;
        let boats: Vec<BodyId> = self
            .boats
            .iter()
            .filter(|b| b.state != State::Broken)
            .map(|b| b.body)
            .collect();
        let planks: Vec<BodyId> = self.boats.iter().flat_map(|b| b.planks.clone()).collect();
        // The hull's drag on the boats, the generic body's on the planks.
        apply_scaled(&mut self.world, water, tick, dt, &DRAG, |id| {
            boats.contains(&id).then_some(1.0)
        });
        apply_scaled(
            &mut self.world,
            water,
            tick,
            dt,
            &Settings::default(),
            |id| planks.contains(&id).then_some(1.0),
        );
        self.world.step(&Uniform(DVec3::new(0.0, -9.81, 0.0)));
        for k in 0..self.boats.len() {
            if self.boats[k].state == State::Upright && self.swamped(k) {
                let dumped = self.dump(k);
                let boat = &mut self.boats[k];
                boat.state = State::Capsized;
                boat.stamp[1] += 1;
                self.events.push(Event::Capsized { boat: k, dumped });
            }
        }
    }

    /// The rower's strokes: once a stroke, an impulse at each oarlock.
    fn pull(&mut self, k: usize, keel: DVec3, rot: DQuat) {
        let rowing = self.boats[k].seats[0]
            .as_ref()
            .is_some_and(|o| o.climbing <= 0.0);
        let intent = self.boats[k].intent;
        if !rowing || intent.idle() {
            self.boats[k].stroke = 0.0;
            return;
        }
        let boat = &mut self.boats[k];
        boat.stroke -= STEP as f32;
        if boat.stroke > 0.0 {
            return;
        }
        boat.stroke += 1.0 / STROKE_RATE;
        boat.strokes += 1;
        let id = boat.body;
        let ahead = rot * DVec3::Z;
        let lock = Seat::Rower.local() + DVec3::new(0.0, 0.1, 0.0);
        // Turning right pulls the left oar harder, and the other way.
        let left = f64::from((intent.ahead + intent.turn).clamp(-1.0, 1.0));
        let right = f64::from((intent.ahead - intent.turn).clamp(-1.0, 1.0));
        for (side, pull) in [(OARLOCK, left), (-OARLOCK, right)] {
            let at = keel + rot * (lock + DVec3::new(side, 0.0, 0.0));
            self.world[id].apply_impulse_at(ahead * OAR_IMPULSE * pull, at);
        }
        self.world.wake(id);
        let at = (keel + rot * lock).as_vec3();
        self.events.push(Event::Stroke { boat: k, at });
    }

    /// Whether boat `k` rolls past [`CAPSIZE_ROLL`] with a gunwale under.
    fn swamped(&self, k: usize) -> bool {
        if self.tilt(k) < CAPSIZE_ROLL {
            return false;
        }
        let (keel, rot) = self.frame(k);
        [-1.0, 1.0].into_iter().any(|side| {
            let gunwale = keel + rot * DVec3::new(side * HALF.x, 2.0 * HALF.y, 0.0);
            self.water
                .sample(gunwale.x, gunwale.z, self.world.tick)
                .is_some_and(|s| s.height > gunwale.y)
        })
    }

    /// Spins boat `k` about its long axis by `omega` rad/s, as a wave or a
    /// shove does.
    pub fn roll(&mut self, k: usize, omega: f64) {
        let id = self.boats[k].body;
        let body = &mut self.world[id];
        body.omega += DVec3::Z * omega;
        body.wake();
    }

    /// Boat `k`'s pose as its host reports it.
    #[must_use]
    pub fn report(&self, k: usize) -> Report {
        let boat = &self.boats[k];
        let (pos, rot) = self.frame(k);
        let body = &self.world[boat.body];
        Report {
            id: boat.id.clone(),
            pos,
            rot,
            vel: if boat.state == State::Broken {
                DVec3::ZERO
            } else {
                body.vel
            },
            omega: if boat.state == State::Broken {
                DVec3::ZERO
            } else {
                body.omega_world()
            },
            stamp: boat.stamp,
            state: boat.state,
            rest: body.sleeping,
            rowing: boat.seats[0]
                .as_ref()
                .is_some_and(|o| Some(&o.who) == boat.owner.as_ref()),
        }
    }

    /// The reports `me` hosts: the boats it rows, or rowed last.
    #[must_use]
    pub fn reports(&self, me: &str) -> Vec<Report> {
        (0..self.boats.len())
            .filter(|&k| self.boats[k].owner.as_deref() == Some(me))
            .map(|k| self.report(k))
            .collect()
    }

    /// Takes a report from `from`, the host of its boat. Stamps order by
    /// epoch, then revision, then host key, as NIP-MV's shared bodies do:
    /// a report applies when it outranks the stamp held, or comes from the
    /// host already followed. The boat then snaps to it and takes its
    /// state, and a host at the oars takes the rower's seat, moving a
    /// local occupant there to the passenger's. Returns whether it applied.
    pub fn receive(&mut self, from: &str, report: &Report) -> bool {
        let Some(k) = self.boats.iter().position(|b| b.id == report.id) else {
            return false;
        };
        let plausible = report.pos.is_finite()
            && report.vel.length() < 20.0
            && report.omega.length() < 30.0
            && report.rot.is_finite();
        let boat = &self.boats[k];
        let held = (boat.stamp, boat.owner.clone().unwrap_or_default());
        let offered = (report.stamp, from.to_owned());
        let follows = boat.owner.as_deref() == Some(from) && report.stamp >= boat.stamp;
        if !plausible || !(follows || offered > held) {
            return false;
        }
        if report.state == State::Broken && boat.state != State::Broken {
            self.break_up(k);
        }
        if report.state == State::Capsized && self.boats[k].state == State::Upright {
            let dumped = self.dump(k);
            self.events.push(Event::Capsized { boat: k, dumped });
        }
        let boat = &mut self.boats[k];
        boat.stamp = report.stamp;
        boat.owner = Some(from.to_owned());
        boat.state = report.state;
        if report.rowing && boat.seats[0].as_ref().is_none_or(|o| o.who != from) {
            let moved = boat.seats[0].replace(Occupant {
                who: from.to_owned(),
                climbing: 0.0,
            });
            if let Some(o) = moved
                && boat.seats[1].is_none()
            {
                boat.seats[1] = Some(o);
            }
        }
        if boat.state == State::Broken {
            return true;
        }
        let id = boat.body;
        let body = &mut self.world[id];
        let rot = report.rot.normalize();
        body.orientation = rot;
        body.prev_orientation = rot;
        body.pos = report.pos + rot * DVec3::new(0.0, KEEL_TO_MASS, 0.0);
        body.vel = report.vel;
        body.omega = rot.inverse() * report.omega;
        if report.rest {
            body.vel = DVec3::ZERO;
            body.omega = DVec3::ZERO;
        } else {
            body.wake();
        }
        true
    }

    /// Movers for the ripple field: each boat and floating plank, at its
    /// waterline, its speed, and its size.
    #[must_use]
    pub fn wakes(&self) -> Vec<(Vec3, Vec3, f32)> {
        let mut out = Vec::new();
        for (k, boat) in self.boats.iter().enumerate() {
            if boat.state != State::Broken {
                let (keel, _) = self.frame(k);
                out.push((keel.as_vec3(), self.world[boat.body].vel.as_vec3(), 0.9));
            }
            for &p in &boat.planks {
                let b = &self.world[p];
                out.push((b.pos.as_vec3(), b.vel.as_vec3(), 0.3));
            }
        }
        out
    }

    /// A plank's pose, for drawing.
    #[must_use]
    pub fn plank_pose(&self, id: BodyId) -> (DVec3, DQuat) {
        let b = &self.world[id];
        (b.pos, b.orientation)
    }

    /// Puts every boat back at its mooring, whole and empty.
    pub fn reset(&mut self) {
        for k in 0..self.boats.len() {
            let dumped = self.dump(k);
            if !dumped.is_empty() {
                self.events.push(Event::Capsized { boat: k, dumped });
            }
            for p in std::mem::take(&mut self.boats[k].planks) {
                self.world.remove_body(p);
            }
            if self.boats[k].state == State::Broken {
                let (keel, yaw) = self.boats[k].home;
                self.boats[k].body = self.add_hull(keel, yaw);
            }
            let (keel, yaw) = self.boats[k].home;
            let rot = DQuat::from_rotation_y(yaw);
            let id = self.boats[k].body;
            let body = &mut self.world[id];
            body.orientation = rot;
            body.prev_orientation = rot;
            body.pos = keel + rot * DVec3::new(0.0, KEEL_TO_MASS, 0.0);
            body.vel = DVec3::ZERO;
            body.omega = DVec3::ZERO;
            let boat = &mut self.boats[k];
            boat.state = State::Upright;
            boat.hit_points = HIT_POINTS;
            boat.stamp = [boat.stamp[0] + 1, 0];
            boat.owner = None;
        }
    }
}

#[cfg(test)]
#[path = "rowboat_tests.rs"]
mod tests;
