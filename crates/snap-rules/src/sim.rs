//! The physics Go runs: the build as a truss of point masses.
//!
//! - Every corner of every piece is a point mass: the piece's mass shared
//!   among its corners (a strut adds its own to the two it joins).
//! - Every side of a piece, its inner stiffeners, and a strut are stiff
//!   damped springs ("bars").
//! - A square frame has only weak springs across its diagonals, standing in
//!   for the grip of its corner magnets, so a sideways load makes it lean.
//!   Leaning past [`LEAN_POP_DEGREES`] pops it apart: its seams let go.
//! - A seam is two stiff damped springs, corner to corner, of rest length
//!   zero. When either is stretched by more than a magnet can hold
//!   ([`MAGNET`] newtons), the seam lets go, unless it is bolted.
//! - Thrusters push along their axis while they burn; the solar storm
//!   pushes on each piece by its sail area; a nudge pushes a tagged piece;
//!   the cargo ship glides in on its own track, stops short if it runs
//!   into a piece, and once in its berth ties up and tugs on its moorings.
//!
//! Pieces don't collide with each other: in space, a piece that comes
//! loose just drifts. Each tick is [`SUBSTEPS`] steps of semi-implicit
//! Euler.

use sha2::{Digest, Sha256};

use crate::build::{Build, groups, inside};
use crate::kinds::PieceKind;
use crate::level::{Job, Level};
use crate::mechanism::{self, Coupling};
use crate::{HZ, SUBSTEPS, V, ticks};

/// A bar's stiffness, newtons a metre.
pub const K_BAR: f64 = 3000.0;
/// A bar's damping, newton seconds a metre.
pub const C_BAR: f64 = 6.0;
/// A magnet join's stiffness, newtons a metre.
pub const K_MAGNET: f64 = 3000.0;
/// A magnet join's damping.
pub const C_MAGNET: f64 = 6.0;
/// How hard one corner-to-corner magnet holds before it lets go, newtons.
pub const MAGNET: f64 = 3.0;
/// A square frame's grip against leaning, per diagonal, newtons a metre.
pub const K_LEAN: f64 = 3.0;
/// Its damping.
pub const C_LEAN: f64 = 0.6;
/// How far a square may lean before it pops, in degrees.
pub const LEAN_POP_DEGREES: u32 = 20;
/// The sine of [`LEAN_POP_DEGREES`].
pub const LEAN_POP: f64 = 0.342_020_143_325_668_7;
/// A thruster's push, newtons.
pub const THRUST: f64 = 4.0;
/// How long a thruster takes to reach full push, in seconds.
pub const RAMP: f64 = 0.25;
/// How far a mooring line reaches from the ship's corner to the build.
pub const MOOR_REACH: f64 = 0.6;
/// How long the dock settles after the ship's tug before it is judged.
pub const SETTLE: f64 = 1.5;
/// Cargo mass carried at the two mooring points while towing, kg.
pub const SHIP_MASS: f64 = 40.;

const DT: f64 = 1.0 / (HZ as f64 * SUBSTEPS as f64);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bar {
    a: usize,
    b: usize,
    rest: f64,
    k: f64,
    c: f64,
    /// A square's lean spring: dropped when the square pops.
    lean_of: Option<usize>,
}

/// A seam in the simulation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joint {
    /// The two pieces.
    pub pieces: (usize, usize),
    /// The two corner-to-corner joins, as node pairs.
    pub nodes: [(usize, usize); 2],
    pub bolted: bool,
    pub coupling: Coupling,
    pub broken: bool,
}

/// Something that happened in a tick of the run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Happened {
    /// A seam let go, here.
    Broke(V),
    /// A square leaned too far and popped.
    Popped(usize),
    /// The ship ran into a piece, here, and stopped.
    Bumped(V),
    /// The ship reached its berth and tied up (or found nothing to tie to).
    Moored(bool),
}

/// Where the cargo ship is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipState {
    /// Its nose (left end).
    pub nose: f64,
    pub parked: bool,
    /// It ran into a piece and stopped short.
    pub blocked: bool,
    /// The nodes its mooring lines hold, once parked.
    pub moorings: Option<[usize; 2]>,
    pub moored_at: Option<V>,
    pub rise: f64,
}

/// A running build.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub pos: Vec<V>,
    pub vel: Vec<V>,
    mass: Vec<f64>,
    inv: Vec<f64>,
    sail: Vec<f64>,
    /// The first node of each piece; a piece's nodes are its corners.
    pub first: Vec<usize>,
    pub kinds: Vec<PieceKind>,
    bars: Vec<Bar>,
    pub joints: Vec<Joint>,
    pub popped: Vec<bool>,
    /// How far each square leans: the sine of the angle, signed.
    pub lean: Vec<f64>,
    /// Thrusters: their two base nodes and their nozzle node.
    thrusters: Vec<[usize; 3]>,
    /// Ticks since Go.
    pub tick: u32,
    pub ship: Option<ShipState>,
    /// The tick the ship parked, if it has.
    pub parked_at: Option<u32>,
    /// The tick the ship ran into a piece, if it did.
    pub blocked_at: Option<u32>,
}

impl World {
    /// The run of `build`, at rest where it was built.
    #[must_use]
    pub fn new(build: &Build, level: &Level) -> Self {
        let mut world = Self {
            pos: Vec::new(),
            vel: Vec::new(),
            mass: Vec::new(),
            inv: Vec::new(),
            sail: Vec::new(),
            first: Vec::new(),
            kinds: Vec::new(),
            bars: Vec::new(),
            joints: Vec::new(),
            popped: vec![false; build.pieces.len()],
            lean: vec![0.0; build.pieces.len()],
            thrusters: Vec::new(),
            tick: 0,
            ship: level.ship.map(|ship| ShipState {
                nose: ship.start,
                parked: false,
                blocked: false,
                moorings: None,
                moored_at: None,
                rise: 0.,
            }),
            parked_at: None,
            blocked_at: None,
        };
        for (index, piece) in build.pieces.iter().enumerate() {
            let first = world.pos.len();
            world.first.push(first);
            world.kinds.push(piece.kind);
            let n = piece.at.len();
            let share = piece.kind.mass() / n as f64;
            let sail = piece.kind.sail() / n as f64;
            for p in &piece.at {
                world.pos.push(*p);
                world.vel.push(V::ZERO);
                world.mass.push(share);
                world.sail.push(sail);
            }
            let mut bar = |a: usize, b: usize, k: f64, c: f64, lean_of: Option<usize>| {
                let rest = (piece.at[b] - piece.at[a]).len();
                world.bars.push(Bar {
                    a: first + a,
                    b: first + b,
                    rest,
                    k,
                    c,
                    lean_of,
                });
            };
            for i in 0..n {
                bar(i, (i + 1) % n, K_BAR, C_BAR, None);
            }
            for (a, b) in piece.kind.inner_bars() {
                bar(*a, *b, K_BAR, C_BAR, None);
            }
            match (piece.kind.leans(), piece.strut) {
                (true, Some(diagonal)) => {
                    let (a, b) = if diagonal == 0 { (0, 2) } else { (1, 3) };
                    bar(a, b, K_BAR, C_BAR, None);
                    let half = PieceKind::Strut.mass() / 2.0;
                    world.mass[first + a] += half;
                    world.mass[first + b] += half;
                }
                (true, None) => {
                    bar(0, 2, K_LEAN, C_LEAN, Some(index));
                    bar(1, 3, K_LEAN, C_LEAN, Some(index));
                }
                _ => {}
            }
            if piece.kind == PieceKind::Thruster {
                world.thrusters.push([first, first + 1, first + 2]);
            }
        }
        world.inv = world.mass.iter().map(|m| 1.0 / *m).collect();
        for seam in &build.seams {
            let [(a0, b0), (a1, b1)] = seam.joints(build);
            world.joints.push(Joint {
                pieces: (seam.a, seam.b),
                nodes: [
                    (world.first[a0.0] + a0.1, world.first[b0.0] + b0.1),
                    (world.first[a1.0] + a1.1, world.first[b1.0] + b1.1),
                ],
                bolted: seam.bolted,
                coupling: seam.coupling,
                broken: false,
            });
        }
        for (tag, omega) in &level.initial_spin {
            if let Some(p) = build.tagged(*tag) {
                let group = world.group_of(p);
                let (centre, _, _) = world.motion(&group);
                for piece in group {
                    for n in world.nodes(piece) {
                        let r = world.pos[n] - centre;
                        world.vel[n] += V::new(-r.y, r.x) * *omega;
                    }
                }
            }
        }
        world
    }

    /// The nodes of piece `piece`.
    #[must_use]
    pub fn nodes(&self, piece: usize) -> std::ops::Range<usize> {
        let end = self.first.get(piece + 1).copied().unwrap_or(self.pos.len());
        self.first[piece]..end
    }

    /// The middle of piece `piece`'s corners now.
    #[must_use]
    pub fn centre(&self, piece: usize) -> V {
        let range = self.nodes(piece);
        let n = range.len() as f64;
        let mut sum = V::ZERO;
        for i in range {
            sum += self.pos[i];
        }
        sum * (1.0 / n)
    }

    /// Each piece's group now: joined by seams that hold. A popped square
    /// is on its own.
    #[must_use]
    pub fn groups(&self) -> Vec<usize> {
        groups(
            self.first.len(),
            self.joints
                .iter()
                .filter(|j| !j.broken)
                .map(|j| j.pieces)
                .filter(|(a, b)| !self.popped[*a] && !self.popped[*b]),
        )
    }

    /// The pieces in `piece`'s group.
    #[must_use]
    pub fn group_of(&self, piece: usize) -> Vec<usize> {
        let groups = self.groups();
        (0..groups.len())
            .filter(|i| groups[*i] == groups[piece])
            .collect()
    }

    /// The centre of mass, velocity and spin (radians a second,
    /// anticlockwise positive) of `pieces` together.
    #[must_use]
    pub fn motion(&self, pieces: &[usize]) -> (V, V, f64) {
        let mut mass = 0.0;
        let mut centre = V::ZERO;
        let mut momentum = V::ZERO;
        for piece in pieces {
            for i in self.nodes(*piece) {
                mass += self.mass[i];
                centre += self.pos[i] * self.mass[i];
                momentum += self.vel[i] * self.mass[i];
            }
        }
        if mass <= 0.0 {
            return (V::ZERO, V::ZERO, 0.0);
        }
        let centre = centre * (1.0 / mass);
        let velocity = momentum * (1.0 / mass);
        let (mut spin, mut inertia) = (0.0, 0.0);
        for piece in pieces {
            for i in self.nodes(*piece) {
                let r = self.pos[i] - centre;
                spin += self.mass[i] * r.cross(self.vel[i] - velocity);
                inertia += self.mass[i] * r.len2();
            }
        }
        let omega = if inertia > 1e-9 { spin / inertia } else { 0.0 };
        (centre, velocity, omega)
    }

    /// The gravity a piece in `pieces` feels from their spin: the pull
    /// toward their centre of mass, `omega^2 * r`, in metres a second
    /// squared.
    #[must_use]
    pub fn spin_gravity(&self, pieces: &[usize], piece: usize) -> f64 {
        let (centre, _, omega) = self.motion(pieces);
        omega * omega * (self.centre(piece) - centre).len()
    }

    /// Whether thrusters are burning, and how hard, from 0 to 1.
    #[must_use]
    pub fn throttle(&self, level: &Level) -> f64 {
        let t = f64::from(self.tick) / f64::from(HZ);
        let burn = f64::from(level.burn) / f64::from(HZ);
        if t >= burn {
            0.0
        } else if t < RAMP {
            t / RAMP
        } else {
            1.0
        }
    }

    /// The storm's push now, up or down, per square metre of sail.
    #[must_use]
    pub fn wind(&self, level: &Level) -> f64 {
        if level.tests.len() > 1 && !matches!(level.job, Job::Connect(_, _)) {
            return 0.;
        }
        let Some(storm) = level.storm else {
            return 0.0;
        };
        if self.tick >= storm.length {
            return 0.0;
        }
        let gust = self.tick / storm.gust;
        let within = f64::from(self.tick % storm.gust) / f64::from(storm.gust);
        // Each gust rises and falls, and the next blows the other way.
        let shape = if within < 0.5 {
            within * 2.0
        } else {
            (1.0 - within) * 2.0
        };
        let sign = if gust % 2 == 0 { 1.0 } else { -1.0 };
        storm.strength * shape * sign
    }

    /// The ship's box now: lowest and highest corners.
    #[must_use]
    pub fn ship_box(&self, level: &Level) -> Option<(V, V)> {
        let (state, ship) = (self.ship?, level.ship?);
        Some((
            V::new(state.nose, ship.y + state.rise - ship.height / 2.0),
            V::new(
                state.nose + ship.length,
                ship.y + state.rise + ship.height / 2.0,
            ),
        ))
    }

    /// Distributes a force at a point, including its moment about the piece.
    fn force_at(&self, force: &mut [V], piece: usize, point: V, value: V) {
        let centre = self.centre(piece);
        let range = self.nodes(piece);
        let n = range.len() as f64;
        let moment = (point - centre).cross(value);
        let inertia = range
            .clone()
            .map(|i| (self.pos[i] - centre).len2())
            .sum::<f64>()
            .max(1e-9);
        for i in range {
            let r = self.pos[i] - centre;
            force[i] += value * (1. / n) + V::new(-r.y, r.x) * (moment / inertia);
        }
    }
    fn torque_on(&self, force: &mut [V], piece: usize, torque: f64) {
        let centre = self.centre(piece);
        let range = self.nodes(piece);
        let inertia = range
            .clone()
            .map(|i| (self.pos[i] - centre).len2())
            .sum::<f64>()
            .max(1e-9);
        for i in range {
            let r = self.pos[i] - centre;
            force[i] += V::new(-r.y, r.x) * (torque / inertia);
        }
    }
    fn wheel_mount(&self, joint: Joint) -> (usize, usize, V, V) {
        let wheel_is_a = self.kinds[joint.pieces.0] == PieceKind::Wheel;
        let (wheel, base) = if wheel_is_a {
            joint.pieces
        } else {
            (joint.pieces.1, joint.pieces.0)
        };
        let [(a0, b0), (a1, b1)] = joint.nodes;
        let edge = if wheel_is_a {
            self.pos[b0].mid(self.pos[b1])
        } else {
            self.pos[a0].mid(self.pos[a1])
        };
        let anchor = edge + (edge - self.centre(base)).unit() * 0.5;
        (wheel, base, self.centre(wheel), anchor)
    }

    /// Advances one tick.
    pub fn step(&mut self, level: &Level, tags: &[(char, usize)]) -> Vec<Happened> {
        let mut happened = Vec::new();
        let n = self.pos.len();
        let throttle = self.throttle(level);
        let wind = self.wind(level);
        let mut outside = vec![V::ZERO; n];
        for [b0, b1, tip] in &self.thrusters {
            if throttle <= 0.0 {
                break;
            }
            let base = self.pos[*b0].mid(self.pos[*b1]);
            let push = (base - self.pos[*tip]).unit() * (THRUST * throttle / 2.0);
            outside[*b0] += push;
            outside[*b1] += push;
        }
        if wind != 0.0 {
            for (i, force) in outside.iter_mut().enumerate() {
                force.y += wind * self.sail[i];
            }
        }
        for nudge in &level.nudges {
            if self.tick < nudge.from || self.tick >= nudge.until {
                continue;
            }
            let pieces: Vec<usize> = if let Some(kind) = nudge.kind {
                self.kinds
                    .iter()
                    .enumerate()
                    .filter_map(|(i, k)| (*k == kind).then_some(i))
                    .collect()
            } else {
                tags.iter()
                    .find(|(t, _)| *t == nudge.tag)
                    .map(|(_, p)| vec![*p])
                    .unwrap_or_default()
            };
            for piece in pieces {
                let range = self.nodes(piece);
                let share = nudge.force * (1. / range.len() as f64);
                for i in range {
                    outside[i] += share;
                }
            }
        }
        let tug = self.tug(level);
        if let Some((nodes, force)) = tug {
            for node in nodes {
                outside[node].x += force;
            }
        }
        let ship_speed = level.ship.map_or(0.0, |s| s.speed);
        for _ in 0..SUBSTEPS {
            let mut force = outside.clone();
            for bar in &self.bars {
                if bar.lean_of.is_some_and(|p| self.popped[p]) {
                    continue;
                }
                let d = self.pos[bar.b] - self.pos[bar.a];
                let len = d.len();
                if len < 1e-9 {
                    continue;
                }
                let dir = d * (1.0 / len);
                let along = (self.vel[bar.b] - self.vel[bar.a]).dot(dir);
                let f = dir * (bar.k * (len - bar.rest) + bar.c * along);
                force[bar.a] += f;
                force[bar.b] -= f;
            }
            for joint in &self.joints {
                if joint.broken {
                    continue;
                }
                if joint.coupling == Coupling::Wheel {
                    let (wheel, base, at, anchor) = self.wheel_mount(*joint);
                    let (centre, velocity, spin) = self.motion(&[base]);
                    let r = anchor - centre;
                    let at_velocity = velocity + V::new(-r.y, r.x) * spin;
                    let wheel_velocity = self.motion(&[wheel]).1;
                    let f = (at - anchor) * mechanism::K_PIVOT
                        + (wheel_velocity - at_velocity) * mechanism::C_PIVOT;
                    self.force_at(&mut force, base, anchor, f);
                    self.force_at(&mut force, wheel, at, f * (-1.));
                    self.torque_on(&mut force, wheel, mechanism::WHEEL_TORQUE * throttle);
                    self.torque_on(&mut force, base, -mechanism::WHEEL_TORQUE * throttle);
                    continue;
                }
                if joint.coupling != Coupling::Magnet {
                    let [(a0, b0), (a1, b1)] = joint.nodes;
                    let drive = if self.kinds[joint.pieces.0] == PieceKind::Wheel {
                        throttle
                    } else {
                        -throttle
                    };
                    let f = mechanism::forces(
                        joint.coupling,
                        [self.pos[a0], self.pos[a1]],
                        [self.pos[b0], self.pos[b1]],
                        [self.vel[a0], self.vel[a1]],
                        [self.vel[b0], self.vel[b1]],
                        drive,
                    );
                    force[a0] += f[0][0];
                    force[a1] += f[0][1];
                    force[b0] += f[1][0];
                    force[b1] += f[1][1];
                    continue;
                }
                for (a, b) in joint.nodes {
                    let f = (self.pos[b] - self.pos[a]) * K_MAGNET
                        + (self.vel[b] - self.vel[a]) * C_MAGNET;
                    force[a] += f;
                    force[b] -= f;
                }
            }
            for i in 0..n {
                self.vel[i] += force[i] * (self.inv[i] * DT);
                self.pos[i] += self.vel[i] * DT;
            }
            if let Some(state) = &mut self.ship
                && !state.parked
                && !state.blocked
            {
                state.nose -= ship_speed * DT;
            }
        }
        self.tick += 1;
        self.bump(level, &mut happened);
        self.park(level, tags, &mut happened);
        self.check_breaks(&mut happened);
        if matches!(level.job, Job::Tow(_)) {
            if let (Some(state), Some(ship)) = (&mut self.ship, level.ship) {
                if let (Some(nodes), Some(at)) = (state.moorings, state.moored_at) {
                    let moved = self.pos[nodes[0]].mid(self.pos[nodes[1]]) - at;
                    state.nose = ship.berth + moved.x;
                    state.rise = moved.y;
                }
            }
        }
        happened
    }

    /// Stops the ship when it runs into a piece: the berth was too small,
    /// or something is in the way.
    fn bump(&mut self, level: &Level, happened: &mut Vec<Happened>) {
        let Some((low, high)) = self.ship_box(level) else {
            return;
        };
        if self.ship.is_some_and(|s| s.parked || s.blocked) {
            return;
        }
        let hull = [low, V::new(high.x, low.y), high, V::new(low.x, high.y)];
        let hit = (0..self.first.len()).find(|piece| {
            let outline: Vec<V> = self.nodes(*piece).map(|i| self.pos[i]).collect();
            !self.popped[*piece] && hits(&outline, &hull)
        });
        if let Some(piece) = hit {
            if let Some(state) = &mut self.ship {
                state.blocked = true;
            }
            self.blocked_at = Some(self.tick);
            happened.push(Happened::Bumped(self.centre(piece)));
        }
    }

    /// The ship's pull on its moorings now, if it is tugging.
    fn tug(&self, level: &Level) -> Option<([usize; 2], f64)> {
        if matches!(level.job, Job::Tow(_)) {
            return None;
        }
        let at = self.parked_at?;
        let nodes = self.ship?.moorings?;
        let (force, length) = level.moor;
        (self.tick >= at && self.tick < at + length).then_some((nodes, force))
    }

    fn park(&mut self, level: &Level, tags: &[(char, usize)], happened: &mut Vec<Happened>) {
        let (Some(ship), Some(state)) = (level.ship, self.ship) else {
            return;
        };
        if state.parked || state.blocked || state.nose > ship.berth {
            return;
        }
        let mut state = state;
        state.nose = ship.berth;
        state.parked = true;
        // The ship ties up to the base's group: the nearest node to each of
        // its back corners.
        let base = match level.job {
            Job::Dock(tag) | Job::Tow(tag) => tags.iter().find(|(t, _)| *t == tag).map(|(_, p)| *p),
            _ => None,
        };
        let group = base.map(|b| self.group_of(b)).unwrap_or_default();
        let back = ship.berth + ship.length;
        let corners = [
            V::new(back, ship.y + ship.height / 2.0),
            V::new(back, ship.y - ship.height / 2.0),
        ];
        let mut found = [None, None];
        for (slot, corner) in corners.iter().enumerate() {
            let mut best: Option<(f64, usize)> = None;
            for piece in &group {
                for i in self.nodes(*piece) {
                    let gap = (self.pos[i] - *corner).len2();
                    if gap <= MOOR_REACH * MOOR_REACH && best.is_none_or(|(old, _)| gap < old) {
                        best = Some((gap, i));
                    }
                }
            }
            found[slot] = best.map(|(_, i)| i);
        }
        state.moorings = match found {
            [Some(a), Some(b)] => Some([a, b]),
            _ => None,
        };
        happened.push(Happened::Moored(state.moorings.is_some()));
        if matches!(level.job, Job::Tow(_)) {
            if let Some(nodes) = state.moorings {
                state.moored_at = Some(self.pos[nodes[0]].mid(self.pos[nodes[1]]));
                for n in nodes {
                    self.mass[n] += SHIP_MASS / 2.;
                    self.inv[n] = 1. / self.mass[n];
                }
            }
        }
        self.ship = Some(state);
        self.parked_at = Some(self.tick);
    }

    fn check_breaks(&mut self, happened: &mut Vec<Happened>) {
        let limit = MAGNET / K_MAGNET;
        let limit2 = limit * limit;
        for index in 0..self.joints.len() {
            let joint = self.joints[index];
            if joint.broken || joint.bolted {
                continue;
            }
            let stretched = match joint.coupling {
                Coupling::Magnet => joint
                    .nodes
                    .iter()
                    .any(|(a, b)| (self.pos[*b] - self.pos[*a]).len2() > limit2),
                Coupling::Spring => joint
                    .nodes
                    .iter()
                    .any(|(a, b)| (self.pos[*b] - self.pos[*a]).len2() > 4.),
                Coupling::Wheel => {
                    let (_, _, at, anchor) = self.wheel_mount(joint);
                    (at - anchor).len2() > 0.05 * 0.05
                }
                Coupling::Tether => joint
                    .nodes
                    .iter()
                    .any(|(a, b)| (self.pos[*b] - self.pos[*a]).len2() > 0.75 * 0.75),
                _ => {
                    let [(a0, b0), (a1, b1)] = joint.nodes;
                    (self.pos[a0].mid(self.pos[a1]) - self.pos[b0].mid(self.pos[b1])).len2()
                        > 0.05 * 0.05
                }
            };
            if stretched {
                self.joints[index].broken = true;
                let (a, b) = joint.nodes[0];
                happened.push(Happened::Broke(self.pos[a].mid(self.pos[b])));
            }
        }
        for piece in 0..self.first.len() {
            if self.kinds[piece] != PieceKind::Square || self.popped[piece] {
                continue;
            }
            if !self.bars.iter().any(|b| b.lean_of == Some(piece)) {
                continue;
            }
            let f = self.first[piece];
            let d1 = (self.pos[f + 2] - self.pos[f]).len2();
            let d2 = (self.pos[f + 3] - self.pos[f + 1]).len2();
            let lean = (d1 - d2) / 4.0;
            self.lean[piece] = lean;
            if lean.abs() > LEAN_POP {
                self.popped[piece] = true;
                for joint in &mut self.joints {
                    if joint.pieces.0 == piece || joint.pieces.1 == piece {
                        joint.broken = true;
                    }
                }
                happened.push(Happened::Popped(piece));
            }
        }
    }

    /// Whether anything let go or popped in the group of `piece`, or in
    /// pieces that were in it when the run began.
    #[must_use]
    pub fn damaged(&self, pieces: &[usize]) -> bool {
        pieces.iter().any(|p| self.popped[*p])
            || self
                .joints
                .iter()
                .any(|j| j.broken && (pieces.contains(&j.pieces.0) || pieces.contains(&j.pieces.1)))
    }

    /// SHA-256 of the state, bit for bit.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.tick.to_le_bytes());
        for (p, v) in self.pos.iter().zip(&self.vel) {
            for x in [p.x, p.y, v.x, v.y] {
                hash.update(x.to_bits().to_le_bytes());
            }
        }
        for joint in &self.joints {
            hash.update([u8::from(joint.broken)]);
        }
        for popped in &self.popped {
            hash.update([u8::from(*popped)]);
        }
        if let Some(ship) = self.ship {
            hash.update(ship.nose.to_bits().to_le_bytes());
            hash.update(ship.rise.to_bits().to_le_bytes());
            if let Some(at) = ship.moored_at {
                hash.update(at.x.to_bits().to_le_bytes());
                hash.update(at.y.to_bits().to_le_bytes());
            }
            hash.update([u8::from(ship.parked), u8::from(ship.blocked)]);
        }
        hash.finalize().into()
    }
}

/// Whether a piece's outline and the ship's hull overlap: a corner or a
/// side's middle of the piece inside the hull, or a point along the hull's
/// sides or its middle line inside the piece.
fn hits(outline: &[V], hull: &[V; 4]) -> bool {
    const MARGIN: f64 = 0.01;
    let n = outline.len();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        if inside(a, hull, MARGIN) || inside(a.mid(b), hull, MARGIN) {
            return true;
        }
    }
    let (low, high) = (hull[0], hull[2]);
    let middle = (low.y + high.y) / 2.0;
    let steps = ((high.x - low.x) / 0.25) as usize + 1;
    for step in 0..=steps {
        let x = low.x + (high.x - low.x) * step as f64 / steps as f64;
        for y in [low.y, middle, high.y] {
            if inside(V::new(x, y), outline, MARGIN) {
                return true;
            }
        }
    }
    false
}

/// Seconds of run to ticks, for tests and the bot.
#[must_use]
pub fn run_ticks(seconds: f64) -> u32 {
    ticks(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level;

    fn level(text: &str) -> Level {
        level::parse(text).unwrap()
    }

    const OPEN: &str = "snap.level.v1\nname Test\nnumber 1\ncrew pip\njob push A\n\
        area -20 -20 20 20\ngive module 0.5 0.5 0 A\nbeacon 10 0.5 1\nrun 10\nburn 2\n";

    fn snap(build: &mut Build, kind: PieceKind, at: V, turn: u8) -> usize {
        let corners = build.candidate(kind, at, turn).expect("snaps");
        build.place(kind, corners, false, None)
    }

    fn run(world: &mut World, level: &Level, seconds: f64) -> Vec<Happened> {
        let mut all = Vec::new();
        for _ in 0..ticks(seconds) {
            all.extend(world.step(level, &[('A', 0)]));
        }
        all
    }

    #[test]
    fn wheel_torque_stays_inside_the_station_and_towing_carries_the_ship_mass() {
        let level = crate::level::job(11);
        let mut game = crate::Game::new(level.clone());
        for input in &level.solution {
            game.step(&[*input]);
        }
        let momentum = |w: &World| {
            let all: Vec<_> = (0..w.first.len()).collect();
            let centre = w.motion(&all).0;
            w.pos
                .iter()
                .zip(&w.vel)
                .zip(&w.mass)
                .map(|((p, v), m)| (*p - centre).cross(*v) * m)
                .sum::<f64>()
        };
        let before = momentum(game.world.as_ref().unwrap());
        while game.status == crate::Status::Running {
            game.step(&[]);
        }
        let after = momentum(game.world.as_ref().unwrap());
        assert!(
            (after - before).abs() < before.abs() * 0.001,
            "{before} -> {after}"
        );
        let level = crate::level::job(10);
        let game = crate::bot::play_plan(&level, &level.solution);
        assert_eq!(game.status, crate::Status::Won);
        let world = game.world.unwrap();
        let parts = game.build.pieces.iter().map(|p| p.kind.mass()).sum::<f64>();
        assert!((world.mass.iter().sum::<f64>() - parts - SHIP_MASS).abs() < 1e-8);
        assert!(world.ship.unwrap().nose > level.ships[0].berth + 0.5);
    }

    #[test]
    fn a_spring_catch_settles_after_the_bump_and_taking_it_off_returns_the_tool() {
        let level = crate::level::job(17);
        let game = crate::bot::play_plan(&level, &level.solution);
        let world = game.world.as_ref().unwrap();
        let a = game.build.tagged('A').unwrap();
        let b = game.build.tagged('B').unwrap();
        assert!((world.motion(&[a]).1 - world.motion(&[b]).1).len() < 0.02);
        let mut build = game.build.clone();
        let panel = build.pieces.iter().position(|p| !p.given).unwrap();
        let back = build.remove(panel);
        assert_eq!(back.links, [0, 1, 0]);
        assert_eq!(back.bolts, 1);
    }

    #[test]
    fn a_build_left_alone_stays_put() {
        let level = level(&OPEN.replace("burn 2", "burn 0"));
        let mut build = level.start();
        snap(&mut build, PieceKind::Square, V::new(1.5, 0.5), 0);
        snap(&mut build, PieceKind::Triangle, V::new(0.5, 1.3), 0);
        let mut world = World::new(&build, &level);
        let before = world.pos.clone();
        assert!(run(&mut world, &level, 3.0).is_empty());
        assert_eq!(world.pos, before, "nothing moves without a push");
    }

    #[test]
    fn a_thruster_through_the_middle_pushes_straight_and_one_off_centre_spins() {
        let level = level(OPEN);
        let mut straight = level.start();
        // On the module's left side, pushing right: a quarter turn puts
        // its base upright.
        snap(&mut straight, PieceKind::Thruster, V::new(-0.3, 0.5), 3);
        let mut world = World::new(&straight, &level);
        run(&mut world, &level, 3.0);
        let (centre, velocity, omega) = world.motion(&[0, 1]);
        assert!(velocity.x > 0.5 && velocity.y.abs() < 1e-6, "{velocity:?}");
        assert!(omega.abs() < 1e-6 && centre.x > 0.5, "{omega}");

        // The same thruster on a square stuck on top: off-centre, it spins.
        let mut crooked = level.start();
        snap(&mut crooked, PieceKind::Panel, V::new(0.5, 1.5), 0);
        snap(&mut crooked, PieceKind::Thruster, V::new(-0.3, 1.5), 3);
        let mut world = World::new(&crooked, &level);
        run(&mut world, &level, 3.0);
        let (_, _, omega) = world.motion(&[0, 1, 2]);
        assert!(
            omega < -0.1,
            "a push above the middle turns it clockwise: {omega}"
        );
    }

    #[test]
    fn a_shove_makes_a_square_lean_and_pop_but_a_braced_one_holds() {
        let nudge = "nudge B 0 8 0 2\n";
        let text = format!("{OPEN}give module 3.5 0.5 0 B\n{nudge}");
        let level = level(&text);
        let tags = [('A', 0), ('B', 1)];
        for (braced, should_pop) in [(false, true), (true, false)] {
            let mut build = level.start();
            let a = snap(&mut build, PieceKind::Square, V::new(1.5, 0.5), 0);
            let b = snap(&mut build, PieceKind::Square, V::new(2.5, 0.5), 0);
            if braced {
                build.pieces[a].strut = Some(0);
                build.pieces[b].strut = Some(0);
            }
            for seam in &mut build.seams {
                seam.bolted = true;
            }
            let mut world = World::new(&build, &level);
            let mut popped = false;
            for _ in 0..ticks(2.0) {
                popped |= world
                    .step(&level, &tags)
                    .iter()
                    .any(|h| matches!(h, Happened::Popped(_)));
            }
            assert_eq!(popped, should_pop, "braced {braced}");
        }
    }

    #[test]
    fn magnets_let_go_under_a_big_pull_but_bolts_hold() {
        let text = format!("{OPEN}give module 1.5 0.5 0 B\nnudge B 30 0 0 1\n");
        let level = level(&text);
        let tags = [('A', 0), ('B', 1)];
        for bolted in [false, true] {
            let mut build = level.start();
            assert_eq!(build.seams.len(), 1, "the given modules join");
            build.seams[0].bolted = bolted;
            let mut world = World::new(&build, &level);
            let mut broke = false;
            for _ in 0..ticks(1.0) {
                broke |= world
                    .step(&level, &tags)
                    .iter()
                    .any(|h| matches!(h, Happened::Broke(_)));
            }
            assert_eq!(broke, !bolted);
            assert_eq!(world.groups()[1] == world.groups()[0], bolted);
        }
    }

    #[test]
    fn spinning_makes_gravity_that_grows_with_distance() {
        let mut world = World::new(&level(OPEN).start(), &level(OPEN));
        // Spin the module by hand: one radian a second about its middle.
        let centre = V::new(0.5, 0.5);
        for i in 0..world.pos.len() {
            world.vel[i] = (world.pos[i] - centre).perp();
        }
        let (_, _, omega) = world.motion(&[0]);
        assert!((omega - 1.0).abs() < 1e-12);
        assert!(world.spin_gravity(&[0], 0) < 1e-12, "none at the middle");
    }
}
