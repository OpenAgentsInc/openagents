//! One garden run: the bunny, the farmer, and the fixed-step rules.

use crate::garden::{Contact, Exit, Garden, lane_offset};
use crate::{
    CORRIDOR_HALF, LANE_WIDTH, TIER_QUARTERS, TIER_SIGHT, TIER_SPEED, UNIT, isqrt, ticks, tier_for,
};

/// Growth from one carrot, in quarter growth points.
pub const CARROT_QUARTERS: u32 = 4;
/// Growth lost to a tumble (3 GP).
pub const TUMBLE_QUARTERS: u32 = 12;
/// Growth a Giant loses bursting out of the net (8 GP).
pub const ESCAPE_QUARTERS: u32 = 32;

/// A lane change takes 0.18 s.
pub const LANE_RATE: i32 = LANE_WIDTH * 1000 / (180 * crate::HZ as i32);
/// A turn pressed up to 0.35 s before a junction waits for it.
pub const BUFFER: u32 = ticks(350);
/// Skidding to a stop at a junction with no straight way on.
pub const SKID: u32 = ticks(600);
/// Skidding around a corner with only one way on.
pub const CORNER_SKID: u32 = ticks(300);
/// Turning back.
pub const UTURN: u32 = ticks(400);
pub const UTURN_COOLDOWN: u32 = ticks(2000);
/// Lying in a tumble, then running slowly.
pub const TUMBLE: u32 = ticks(500);
pub const SLOW: u32 = ticks(1200);
/// After a tumble the net can't land.
pub const GRACE: u32 = ticks(300);

/// The bunny's half length, from its centre to its nose.
const BODY: i32 = UNIT / 4;
/// How close a carrot must be, along the corridor, to be eaten.
const EAT_REACH: i32 = UNIT * 4 / 10;
/// How close to a lane's centre counts as being in it.
const LANE_HIT: i32 = UNIT * 7 / 10;

const SHED_WAIT: u32 = ticks(3000);
const LOSE_SIGHT: u32 = ticks(4000);
const SEARCH: u32 = ticks(3000);
const CHASE_SPELL: u32 = ticks(20_000);
const SCATTER_SPELL: u32 = ticks(7000);
const FARMER_TURN: u32 = ticks(300);
const MISS_RECOVER: u32 = ticks(400);
const STAGGER: u32 = ticks(1000);
const ESCAPE_WINDOW: u32 = ticks(10_000);
const HEAR: i32 = 4 * UNIT;
/// He starts a swing when the bunny is this close in front of him.
pub const NET_TRIGGER: i32 = UNIT * 22 / 10;
/// How far the net reaches when it lands.
const NET_REACH: i32 = UNIT * 26 / 10;
/// The net covers his lane and half of each neighbour.
const NET_HALF: i32 = LANE_WIDTH * 3 / 4;
/// How close he walks up to the bunny.
const FARMER_STOP: i32 = UNIT * 8 / 10;
const FARMER_LANE_RATE: i32 = LANE_WIDTH / 36;

/// A player's command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Lane left, or turn left at the next junction.
    Left,
    /// Lane right, or turn right at the next junction.
    Right,
    /// Turn back.
    Back,
}

/// A side to turn to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// What the bunny is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Run,
    /// Stopping at a junction; then turns the one way on, or waits.
    Skid {
        left: u32,
        then: Option<Exit>,
    },
    /// Stopped at a junction until the player picks a way.
    Wait,
    UTurn {
        left: u32,
    },
    Tumble {
        left: u32,
    },
}

/// Whether the run goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Playing,
    Won,
    Caught,
}

/// Something that happened in the last step, for sounds and effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Ate(usize),
    Grew(u8),
    Shrank(u8),
    Tumbled,
    Smashed(usize),
    Skidded,
    TurnedBack,
    Whistle,
    WindUp,
    Missed,
    Escaped,
    Caught,
    Won,
}

/// The player's bunny.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bunny {
    pub edge: usize,
    /// Whether it runs from the edge's `a` to its `b`.
    pub fwd: bool,
    pub s: i32,
    /// The lane it is in or moving to, in its own frame (positive right).
    pub lane: i8,
    /// Its sideways offset from the corridor's centre, in its own frame.
    pub lane_pos: i32,
    pub mv: Move,
    pub pending: Option<Side>,
    pub cooldown: u32,
    pub slow: u32,
    pub grace: u32,
    pub quarters: u32,
    pub tier: u8,
    last_escape: Option<u32>,
}

impl Bunny {
    /// The unit direction it faces.
    #[must_use]
    pub fn heading(&self, garden: &Garden) -> (i32, i32) {
        let e = &garden.edges[self.edge];
        if self.fwd {
            (e.dx, e.dz)
        } else {
            (-e.dx, -e.dz)
        }
    }

    /// Its sideways offset in the edge's frame.
    #[must_use]
    pub fn lateral(&self) -> i32 {
        if self.fwd {
            self.lane_pos
        } else {
            -self.lane_pos
        }
    }

    /// The distance left to the junction ahead.
    #[must_use]
    pub fn to_end(&self, garden: &Garden) -> i32 {
        let len = garden.edges[self.edge].len;
        if self.fwd { len - self.s } else { self.s }
    }

    /// Its current run speed, in units per tick.
    #[must_use]
    pub fn speed(&self) -> i32 {
        let speed = TIER_SPEED[usize::from(self.tier)];
        if self.slow > 0 { speed * 6 / 10 } else { speed }
    }

    fn reverse(&mut self) {
        self.fwd = !self.fwd;
        self.lane = -self.lane;
        self.lane_pos = -self.lane_pos;
        self.pending = None;
    }
}

/// What the farmer is up to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FarmerState {
    /// In his shed at the start.
    Shed { left: u32 },
    /// Walking his rounds.
    Patrol { next: usize },
    /// Running after the bunny.
    Chase { spell: u32 },
    /// Looking around where he last saw it.
    Search { left: u32, node: usize },
    /// Walking back to his shed for a breather.
    Scatter { left: u32 },
}

/// The farmer and his net.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Farmer {
    pub edge: usize,
    pub fwd: bool,
    pub s: i32,
    /// Sideways offset in the edge's frame.
    pub lane_pos: i32,
    pub state: FarmerState,
    /// The unit direction he faces.
    pub facing: (i32, i32),
    /// Ticks left before a wound-up swing lands; 0 when not winding up.
    pub windup: u32,
    /// Ticks since his last swing, for drawing it.
    pub since_swing: u32,
    pub recover: u32,
    pub stagger: u32,
    pause: u32,
    unseen: u32,
    last_seen: usize,
}

enum Target {
    Node(usize),
    Bunny,
}

/// One garden run.
#[derive(Clone, Debug)]
pub struct Game {
    pub garden: Garden,
    pub bunny: Bunny,
    pub farmer: Farmer,
    /// Whether the farmer plays; off only in tests and practice.
    pub farmer_on: bool,
    pub eaten: Vec<bool>,
    /// Which obstacles still stand.
    pub alive: Vec<bool>,
    pub carrots_left: usize,
    pub status: Status,
    pub tick: u32,
    /// What happened in the last step.
    pub events: Vec<Event>,
    /// Shortest walking distances between junctions for the farmer.
    dist: Vec<Vec<i64>>,
}

const FAR: i64 = i64::MAX / 4;

impl Game {
    /// A fresh run of `garden`.
    #[must_use]
    pub fn new(garden: Garden) -> Self {
        let exit = garden.exits(garden.shed)[0];
        // He stands in the shed's doorway, as if he had just walked in, so
        // his first step picks a way out.
        let farmer = Farmer {
            edge: exit.edge,
            fwd: !exit.fwd,
            s: if exit.fwd {
                0
            } else {
                garden.edges[exit.edge].len
            },
            lane_pos: 0,
            state: FarmerState::Shed { left: SHED_WAIT },
            facing: (exit.dx, exit.dz),
            windup: 0,
            since_swing: u32::MAX / 2,
            recover: 0,
            stagger: 0,
            pause: 0,
            unseen: 0,
            last_seen: garden.shed,
        };
        let bunny = Bunny {
            edge: garden.spawn_edge,
            fwd: garden.spawn_fwd,
            s: garden.spawn_s,
            lane: 0,
            lane_pos: 0,
            mv: Move::Run,
            pending: None,
            cooldown: 0,
            slow: 0,
            grace: 0,
            quarters: 0,
            tier: 0,
            last_escape: None,
        };
        let n = garden.nodes.len();
        let mut dist = vec![vec![FAR; n]; n];
        for (i, row) in dist.iter_mut().enumerate() {
            row[i] = 0;
        }
        for (index, e) in garden.edges.iter().enumerate() {
            if garden.farmer_may_walk(index) {
                dist[e.a][e.b] = i64::from(e.len);
                dist[e.b][e.a] = i64::from(e.len);
            }
        }
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    let through = dist[i][k] + dist[k][j];
                    if through < dist[i][j] {
                        dist[i][j] = through;
                    }
                }
            }
        }
        Self {
            eaten: vec![false; garden.carrots.len()],
            alive: vec![true; garden.cells.len()],
            carrots_left: garden.carrots.len(),
            garden,
            bunny,
            farmer,
            farmer_on: true,
            status: Status::Playing,
            tick: 0,
            events: Vec::new(),
            dist,
        }
    }

    /// The bunny's position in the world.
    #[must_use]
    pub fn bunny_point(&self) -> (i32, i32) {
        self.garden
            .point(self.bunny.edge, self.bunny.s, self.bunny.lateral())
    }

    /// The farmer's position in the world.
    #[must_use]
    pub fn farmer_point(&self) -> (i32, i32) {
        self.garden
            .point(self.farmer.edge, self.farmer.s, self.farmer.lane_pos)
    }

    /// Advances one tick with the inputs pressed since the last.
    pub fn step(&mut self, inputs: &[Input]) {
        self.events.clear();
        if self.status != Status::Playing {
            return;
        }
        self.tick += 1;
        for input in inputs {
            self.press(*input);
        }
        self.step_bunny();
        if self.carrots_left == 0 {
            self.status = Status::Won;
            self.events.push(Event::Won);
            return;
        }
        if self.farmer_on && self.status == Status::Playing {
            self.step_farmer();
        }
    }

    /// The way out of the junction ahead on `side`, if there is one.
    #[must_use]
    pub fn exit_on(&self, side: Side) -> Option<Exit> {
        let e = &self.garden.edges[self.bunny.edge];
        let node = e.end(self.bunny.fwd);
        let (hx, hz) = self.bunny.heading(&self.garden);
        let want = match side {
            Side::Left => (hz, -hx),
            Side::Right => (-hz, hx),
        };
        self.garden
            .exits(node)
            .iter()
            .copied()
            .find(|exit| (exit.dx, exit.dz) == want)
    }

    fn press(&mut self, input: Input) {
        let side = match input {
            Input::Left => Some(Side::Left),
            Input::Right => Some(Side::Right),
            Input::Back => None,
        };
        match self.bunny.mv {
            Move::Tumble { .. } | Move::UTurn { .. } => {}
            Move::Skid { .. } | Move::Wait => match side {
                Some(side) => {
                    if let Some(exit) = self.exit_on(side) {
                        self.take(exit);
                        self.bunny.mv = Move::Run;
                    }
                }
                None => self.turn_back(),
            },
            Move::Run => match side {
                Some(side) => {
                    let near =
                        self.bunny.to_end(&self.garden) <= self.bunny.speed() * BUFFER as i32;
                    if near && self.exit_on(side).is_some() {
                        self.bunny.pending = Some(side);
                    } else {
                        let step = if side == Side::Left { -1 } else { 1 };
                        self.bunny.lane = (self.bunny.lane + step).clamp(-1, 1);
                    }
                }
                None => {
                    if self.bunny.cooldown == 0 {
                        self.turn_back();
                    }
                }
            },
        }
    }

    fn turn_back(&mut self) {
        self.bunny.reverse();
        self.bunny.cooldown = UTURN_COOLDOWN;
        self.bunny.mv = Move::UTurn { left: UTURN };
        self.events.push(Event::TurnedBack);
    }

    fn take(&mut self, exit: Exit) {
        self.bunny.edge = exit.edge;
        self.bunny.fwd = exit.fwd;
        self.bunny.s = if exit.fwd {
            0
        } else {
            self.garden.edges[exit.edge].len
        };
    }

    fn set_quarters(&mut self, quarters: u32) {
        let before = self.bunny.tier;
        self.bunny.quarters = quarters;
        self.bunny.tier = tier_for(quarters);
        if self.bunny.tier > before {
            self.events.push(Event::Grew(self.bunny.tier));
        } else if self.bunny.tier < before {
            self.events.push(Event::Shrank(self.bunny.tier));
        }
    }

    fn tumble(&mut self) {
        let quarters = self.bunny.quarters.saturating_sub(TUMBLE_QUARTERS);
        self.set_quarters(quarters);
        self.bunny.mv = Move::Tumble { left: TUMBLE };
        self.bunny.pending = None;
        self.events.push(Event::Tumbled);
    }

    fn step_bunny(&mut self) {
        let b = &mut self.bunny;
        b.cooldown = b.cooldown.saturating_sub(1);
        b.slow = b.slow.saturating_sub(1);
        b.grace = b.grace.saturating_sub(1);
        let target = lane_offset(b.lane);
        b.lane_pos += (target - b.lane_pos).clamp(-LANE_RATE, LANE_RATE);
        match b.mv {
            Move::Tumble { left } => {
                if left > 1 {
                    b.mv = Move::Tumble { left: left - 1 };
                } else {
                    b.reverse();
                    b.mv = Move::Run;
                    b.slow = SLOW;
                    b.grace = GRACE;
                }
            }
            Move::UTurn { left } => {
                b.mv = if left > 1 {
                    Move::UTurn { left: left - 1 }
                } else {
                    Move::Run
                };
            }
            Move::Skid { left, then } => {
                if left > 1 {
                    b.mv = Move::Skid {
                        left: left - 1,
                        then,
                    };
                } else if let Some(exit) = then {
                    b.mv = Move::Run;
                    self.take(exit);
                } else {
                    b.mv = Move::Wait;
                }
            }
            Move::Wait => {}
            Move::Run => {
                self.run();
                if self.bunny.mv == Move::Run && self.farmer_on && self.bumps_farmer() {
                    self.tumble();
                }
            }
        }
    }

    fn bumps_farmer(&self) -> bool {
        if matches!(self.farmer.state, FarmerState::Shed { .. }) {
            return false;
        }
        let (bx, bz) = self.bunny_point();
        let (fx, fz) = self.farmer_point();
        let (hx, hz) = self.bunny.heading(&self.garden);
        let (dx, dz) = (fx - bx, fz - bz);
        let forward = dx * hx + dz * hz;
        let lateral = -dx * hz + dz * hx;
        forward > 0 && forward < UNIT * 7 / 10 && lateral.abs() < UNIT * 6 / 10
    }

    fn run(&mut self) {
        let mut remaining = self.bunny.speed();
        while remaining > 0 {
            let to_end = self.bunny.to_end(&self.garden);
            let step = remaining.min(to_end);
            let dir = if self.bunny.fwd { 1 } else { -1 };
            let s0 = self.bunny.s;
            let s1 = s0 + dir * step;
            if self.contact(s0, s1, dir) {
                return;
            }
            self.bunny.s = s1;
            self.eat(s0, s1);
            remaining -= step;
            if step == to_end {
                self.arrive();
                if self.bunny.mv != Move::Run {
                    return;
                }
            }
        }
    }

    /// Runs into obstacles between `s0` and `s1`; true when one stopped it.
    fn contact(&mut self, s0: i32, s1: i32, dir: i32) -> bool {
        let (f0, f1) = (s0 + dir * BODY, s1 + dir * BODY);
        let lateral = self.bunny.lateral();
        let mut hits: Vec<usize> = (0..self.garden.cells.len())
            .filter(|index| {
                let cell = &self.garden.cells[*index];
                self.alive[*index]
                    && cell.edge == self.bunny.edge
                    && (lateral - lane_offset(cell.lane)).abs() < LANE_HIT
                    && if dir > 0 {
                        f0 < cell.s && cell.s <= f1
                    } else {
                        f1 <= cell.s && cell.s < f0
                    }
            })
            .collect();
        hits.sort_by_key(|index| (self.garden.cells[*index].s - f0).abs());
        for index in hits {
            let cell = self.garden.cells[index];
            match cell.kind.contact(self.bunny.tier) {
                Contact::Pass => {}
                Contact::Smash => {
                    self.alive[index] = false;
                    self.events.push(Event::Smashed(index));
                }
                Contact::Block => {
                    self.bunny.s = cell.s - dir * (BODY + 1);
                    self.tumble();
                    return true;
                }
            }
        }
        false
    }

    fn eat(&mut self, s0: i32, s1: i32) {
        let (lo, hi) = (s0.min(s1) - EAT_REACH, s0.max(s1) + EAT_REACH);
        let lateral = self.bunny.lateral();
        for index in 0..self.garden.carrots.len() {
            let carrot = self.garden.carrots[index];
            if !self.eaten[index]
                && carrot.edge == self.bunny.edge
                && carrot.s >= lo
                && carrot.s <= hi
                && (lateral - lane_offset(carrot.lane)).abs() < LANE_HIT
            {
                self.eaten[index] = true;
                self.carrots_left -= 1;
                self.events.push(Event::Ate(index));
                self.set_quarters(self.bunny.quarters + CARROT_QUARTERS);
            }
        }
    }

    fn arrive(&mut self) {
        if let Some(side) = self.bunny.pending.take()
            && let Some(exit) = self.exit_on(side)
        {
            self.take(exit);
            return;
        }
        let e = &self.garden.edges[self.bunny.edge];
        let node = e.end(self.bunny.fwd);
        let heading = self.bunny.heading(&self.garden);
        let ways: Vec<Exit> = self
            .garden
            .exits(node)
            .iter()
            .copied()
            .filter(|exit| (exit.dx, exit.dz) != (-heading.0, -heading.1))
            .collect();
        if let Some(straight) = ways.iter().find(|exit| (exit.dx, exit.dz) == heading) {
            self.take(*straight);
            return;
        }
        match ways.as_slice() {
            [] => {
                self.bunny.reverse();
                self.bunny.mv = Move::UTurn { left: UTURN };
            }
            [only] => {
                self.bunny.mv = Move::Skid {
                    left: CORNER_SKID,
                    then: Some(*only),
                };
            }
            _ => {
                self.bunny.mv = Move::Skid {
                    left: SKID,
                    then: None,
                };
                self.events.push(Event::Skidded);
            }
        }
    }

    /// Whether the farmer can see or hear the bunny.
    #[must_use]
    pub fn farmer_sees(&self) -> bool {
        let (bx, bz) = self.bunny_point();
        let (fx, fz) = self.farmer_point();
        let (dx, dz) = (i64::from(bx - fx), i64::from(bz - fz));
        let d2 = dx * dx + dz * dz;
        if d2 <= i64::from(HEAR).pow(2) {
            return true;
        }
        let sight = i64::from(TIER_SIGHT[usize::from(self.bunny.tier)]);
        if d2 > sight * sight {
            return false;
        }
        let steps = isqrt(d2) / i64::from(UNIT / 2) + 1;
        (1..steps).all(|i| {
            let x = i64::from(fx) + dx * i / steps;
            let z = i64::from(fz) + dz * i / steps;
            self.garden.in_corridor(x as i32, z as i32)
        })
    }

    fn step_farmer(&mut self) {
        let f = &mut self.farmer;
        f.since_swing = f.since_swing.saturating_add(1);
        if f.stagger > 0 {
            f.stagger -= 1;
            return;
        }
        if f.recover > 0 {
            f.recover -= 1;
            return;
        }
        if f.windup > 0 {
            f.windup -= 1;
            if f.windup == 0 {
                self.swing();
            }
            return;
        }
        let visible = self.farmer_sees();
        let ahead = self.garden.edges[self.bunny.edge].end(self.bunny.fwd);
        let f = &mut self.farmer;
        match f.state {
            FarmerState::Shed { left } => {
                if left > 1 {
                    f.state = FarmerState::Shed { left: left - 1 };
                    return;
                }
                f.state = FarmerState::Patrol { next: 0 };
            }
            FarmerState::Patrol { .. } | FarmerState::Search { .. } => {
                if visible {
                    f.state = FarmerState::Chase { spell: 0 };
                    f.unseen = 0;
                    f.last_seen = ahead;
                    self.events.push(Event::Whistle);
                }
            }
            FarmerState::Chase { spell } => {
                if visible {
                    f.unseen = 0;
                    f.last_seen = ahead;
                } else {
                    f.unseen += 1;
                }
                f.state = if f.unseen >= LOSE_SIGHT {
                    FarmerState::Search {
                        left: SEARCH,
                        node: f.last_seen,
                    }
                } else if spell + 1 >= CHASE_SPELL {
                    FarmerState::Scatter {
                        left: SCATTER_SPELL,
                    }
                } else {
                    FarmerState::Chase { spell: spell + 1 }
                };
            }
            FarmerState::Scatter { left } => {
                f.state = if left > 1 {
                    FarmerState::Scatter { left: left - 1 }
                } else {
                    f.unseen = 0;
                    FarmerState::Chase { spell: 0 }
                };
            }
        }
        if visible && self.bunny.grace == 0 && self.in_trigger() {
            let (bx, bz) = self.bunny_point();
            let (fx, fz) = self.farmer_point();
            let (dx, dz) = (bx - fx, bz - fz);
            self.farmer.facing = if dx.abs() >= dz.abs() {
                (dx.signum(), 0)
            } else {
                (0, dz.signum())
            };
            self.farmer.windup = self.garden.windup;
            self.events.push(Event::WindUp);
            return;
        }
        self.move_farmer();
    }

    fn in_trigger(&self) -> bool {
        let (bx, bz) = self.bunny_point();
        let (fx, fz) = self.farmer_point();
        let (dx, dz) = ((bx - fx).abs(), (bz - fz).abs());
        let (major, minor) = if dx >= dz { (dx, dz) } else { (dz, dx) };
        major <= NET_TRIGGER && minor <= CORRIDOR_HALF
    }

    fn swing(&mut self) {
        self.farmer.since_swing = 0;
        let (bx, bz) = self.bunny_point();
        let (fx, fz) = self.farmer_point();
        let (dx, dz) = (bx - fx, bz - fz);
        let (hx, hz) = self.farmer.facing;
        let forward = dx * hx + dz * hz;
        let lateral = -dx * hz + dz * hx;
        let lands = forward >= -UNIT * 3 / 10
            && forward <= NET_REACH
            && lateral.abs() <= NET_HALF
            && self.bunny.grace == 0;
        if !lands {
            self.farmer.recover = MISS_RECOVER;
            self.events.push(Event::Missed);
            return;
        }
        let recent = self
            .bunny
            .last_escape
            .is_some_and(|at| self.tick - at <= ESCAPE_WINDOW);
        if self.bunny.tier == 4 && !recent {
            let quarters = self
                .bunny
                .quarters
                .saturating_sub(ESCAPE_QUARTERS)
                .min(TIER_QUARTERS[4] - 1);
            self.set_quarters(quarters);
            self.bunny.last_escape = Some(self.tick);
            self.farmer.stagger = STAGGER;
            self.events.push(Event::Escaped);
        } else {
            self.status = Status::Caught;
            self.events.push(Event::Caught);
        }
    }

    /// The farmer's walking distance from junction `node` to the target.
    fn distance(&self, node: usize, target: &Target) -> i64 {
        match target {
            Target::Node(goal) => self.dist[node][*goal],
            Target::Bunny => {
                let e = &self.garden.edges[self.bunny.edge];
                if self.garden.farmer_may_walk(self.bunny.edge) {
                    (self.dist[node][e.a] + i64::from(self.bunny.s))
                        .min(self.dist[node][e.b] + i64::from(e.len - self.bunny.s))
                } else {
                    self.dist[node][e.a].min(self.dist[node][e.b])
                }
            }
        }
    }

    fn farmer_target(&self) -> Option<(Target, i32)> {
        let g = &self.garden;
        match self.farmer.state {
            FarmerState::Shed { .. } => None,
            FarmerState::Patrol { next } => Some((Target::Node(g.patrol[next]), g.farmer_walk)),
            FarmerState::Chase { .. } => Some((Target::Bunny, g.farmer_run)),
            FarmerState::Search { node, .. } => Some((Target::Node(node), g.farmer_run)),
            FarmerState::Scatter { .. } => Some((Target::Node(g.shed), g.farmer_walk)),
        }
    }

    fn move_farmer(&mut self) {
        if self.farmer.pause > 0 {
            self.farmer.pause -= 1;
            return;
        }
        let Some((target, speed)) = self.farmer_target() else {
            return;
        };
        let chasing = matches!(target, Target::Bunny);
        let same_edge = self.farmer.edge == self.bunny.edge;
        let lane_goal = if chasing && same_edge {
            self.bunny.lateral()
        } else {
            0
        };
        self.farmer.lane_pos +=
            (lane_goal - self.farmer.lane_pos).clamp(-FARMER_LANE_RATE, FARMER_LANE_RATE);
        if chasing && same_edge {
            let ds = self.bunny.s - self.farmer.s;
            if ds.abs() <= FARMER_STOP {
                return;
            }
            let want = ds > 0;
            if want != self.farmer.fwd {
                self.turn_farmer();
                return;
            }
            let step = speed.min(ds.abs() - FARMER_STOP);
            self.farmer.s += if want { step } else { -step };
            self.face_farmer();
            return;
        }
        let e = self.garden.edges[self.farmer.edge];
        let to_end = if self.farmer.fwd {
            e.len - self.farmer.s
        } else {
            self.farmer.s
        };
        let back = e.len - to_end;
        if back > 0 && to_end > 0 {
            let ahead = i64::from(to_end) + self.distance(e.end(self.farmer.fwd), &target);
            let behind = i64::from(back) + self.distance(e.end(!self.farmer.fwd), &target);
            if behind + i64::from(2 * UNIT) < ahead {
                self.turn_farmer();
                return;
            }
        }
        let mut remaining = speed;
        loop {
            let e = self.garden.edges[self.farmer.edge];
            let to_end = if self.farmer.fwd {
                e.len - self.farmer.s
            } else {
                self.farmer.s
            };
            if to_end == 0 {
                if !self.farmer_at(e.end(self.farmer.fwd), &target) {
                    return;
                }
                continue;
            }
            if remaining == 0 {
                break;
            }
            if remaining < to_end {
                self.farmer.s += if self.farmer.fwd {
                    remaining
                } else {
                    -remaining
                };
                break;
            }
            self.farmer.s = if self.farmer.fwd { e.len } else { 0 };
            remaining -= to_end;
        }
        self.face_farmer();
    }

    /// The farmer has reached junction `node`: picks the way on, or stays.
    /// True when he walks straight on without stopping.
    fn farmer_at(&mut self, node: usize, target: &Target) -> bool {
        if let Target::Node(goal) = target
            && *goal == node
        {
            match &mut self.farmer.state {
                FarmerState::Patrol { next } => {
                    *next = (*next + 1) % self.garden.patrol.len();
                }
                FarmerState::Search { left, .. } => {
                    if *left > 1 {
                        *left -= 1;
                    } else {
                        self.farmer.state = FarmerState::Patrol { next: 0 };
                    }
                }
                _ => {}
            }
            return false;
        }
        let stay = self.distance(node, target);
        let mut best: Option<(i64, Exit)> = None;
        for exit in self.garden.exits(node) {
            if !self.garden.farmer_may_walk(exit.edge) {
                continue;
            }
            let e = &self.garden.edges[exit.edge];
            let cost = if matches!(target, Target::Bunny) && exit.edge == self.bunny.edge {
                i64::from(if exit.fwd {
                    self.bunny.s
                } else {
                    e.len - self.bunny.s
                })
            } else {
                i64::from(e.len) + self.distance(e.end(exit.fwd), target)
            };
            if best.is_none_or(|(old, _)| cost < old) {
                best = Some((cost, *exit));
            }
        }
        let Some((cost, exit)) = best else {
            return false;
        };
        if stay <= 0 && cost > 0 {
            return false;
        }
        let turning = (exit.dx, exit.dz) != self.farmer.facing;
        self.farmer.edge = exit.edge;
        self.farmer.fwd = exit.fwd;
        self.farmer.s = if exit.fwd {
            0
        } else {
            self.garden.edges[exit.edge].len
        };
        self.farmer.lane_pos = 0;
        if turning {
            self.farmer.facing = (exit.dx, exit.dz);
            self.farmer.pause = FARMER_TURN;
            return false;
        }
        true
    }

    fn turn_farmer(&mut self) {
        self.farmer.fwd = !self.farmer.fwd;
        self.farmer.pause = FARMER_TURN;
        self.face_farmer();
    }

    fn face_farmer(&mut self) {
        let e = &self.garden.edges[self.farmer.edge];
        self.farmer.facing = if self.farmer.fwd {
            (e.dx, e.dz)
        } else {
            (-e.dx, -e.dz)
        };
    }

    /// A short summary of the whole state, for checking that two runs match.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut mix = |value: i64| {
            for byte in value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        };
        let b = &self.bunny;
        let f = &self.farmer;
        for value in [
            b.edge as i64,
            i64::from(b.fwd),
            i64::from(b.s),
            i64::from(b.lane_pos),
            i64::from(b.quarters),
            f.edge as i64,
            i64::from(f.fwd),
            i64::from(f.s),
            i64::from(f.lane_pos),
            i64::from(f.windup),
            self.carrots_left as i64,
            i64::from(self.tick),
        ] {
            mix(value);
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::garden::{Cell, CellKind};

    fn quiet() -> Game {
        let mut game = Game::new(Garden::first());
        game.farmer_on = false;
        game
    }

    fn run(game: &mut Game, ticks: u32) {
        for _ in 0..ticks {
            game.step(&[]);
        }
    }

    #[test]
    fn the_bunny_runs_forward_and_eats_the_carrots_in_its_lane() {
        let mut game = quiet();
        let start = game.bunny.s;
        run(&mut game, 55);
        assert!(game.bunny.s > start);
        // The spawn corridor's middle lane holds carrots at 5 m and 7 m.
        assert!(game.eaten[0] && game.eaten[1], "{:?}", &game.eaten[..4]);
        assert_eq!(game.bunny.quarters, 2 * CARROT_QUARTERS);
    }

    #[test]
    fn left_and_right_change_lanes_away_from_junctions() {
        let mut game = quiet();
        game.step(&[Input::Left]);
        assert_eq!(game.bunny.lane, -1);
        run(&mut game, 12);
        assert_eq!(game.bunny.lane_pos, -LANE_WIDTH);
        game.step(&[Input::Left]);
        assert_eq!(game.bunny.lane, -1, "the lanes end at the hedge");
        game.step(&[Input::Right, Input::Right]);
        assert_eq!(game.bunny.lane, 1);
    }

    #[test]
    fn a_turn_pressed_before_a_junction_is_taken_there() {
        let mut game = quiet();
        // Run to just before the first junction, then ask for a right turn.
        while game.bunny.to_end(&game.garden) > 15_000 {
            game.step(&[]);
        }
        game.step(&[Input::Right]);
        assert_eq!(game.bunny.pending, Some(Side::Right));
        assert_eq!(game.bunny.lane, 0);
        run(&mut game, 20);
        assert_eq!(
            game.bunny.heading(&game.garden),
            (0, 1),
            "now heading south"
        );
    }

    #[test]
    fn turning_back_reverses_and_then_cools_down() {
        let mut game = quiet();
        run(&mut game, 5);
        game.step(&[Input::Back]);
        assert!(!game.bunny.fwd);
        run(&mut game, UTURN + 5);
        assert_eq!(game.bunny.mv, Move::Run);
        game.step(&[Input::Back]);
        assert!(!game.bunny.fwd, "still cooling down");
    }

    fn place(game: &mut Game, edge: usize, s: i32, fwd: bool, lane: i8) {
        game.bunny.edge = edge;
        game.bunny.s = s;
        game.bunny.fwd = fwd;
        game.bunny.lane = lane;
        game.bunny.lane_pos = lane_offset(lane);
    }

    fn cell_of(game: &Game, kind: CellKind) -> (usize, Cell) {
        game.garden
            .cells
            .iter()
            .copied()
            .enumerate()
            .find(|(_, c)| c.kind == kind)
            .unwrap()
    }

    #[test]
    fn a_small_bunny_tumbles_into_a_pot_loses_growth_and_turns_back() {
        let mut game = quiet();
        let (index, pot) = cell_of(&game, CellKind::Pot);
        place(&mut game, pot.edge, pot.s - 2 * UNIT, true, pot.lane);
        game.bunny.quarters = 20;
        run(&mut game, 30);
        assert!(
            matches!(game.bunny.mv, Move::Tumble { .. }),
            "{:?}",
            game.bunny.mv
        );
        assert_eq!(game.bunny.quarters, 8);
        assert!(game.alive[index]);
        run(&mut game, TUMBLE);
        assert!(!game.bunny.fwd, "bounced back the way it came");
        assert!(game.bunny.slow > 0);
    }

    #[test]
    fn a_jack_smashes_the_pot_and_runs_on() {
        let mut game = quiet();
        let (index, pot) = cell_of(&game, CellKind::Pot);
        place(&mut game, pot.edge, pot.s - 2 * UNIT, true, pot.lane);
        game.bunny.quarters = TIER_QUARTERS[2];
        game.bunny.tier = 2;
        run(&mut game, 30);
        assert!(!game.alive[index]);
        assert_eq!(game.bunny.mv, Move::Run);
        assert!(game.bunny.s > pot.s);
    }

    #[test]
    fn the_fence_gap_fits_a_kit_but_bounces_a_jack() {
        let mut game = quiet();
        let (_, gap) = cell_of(&game, CellKind::Gap);
        place(&mut game, gap.edge, gap.s - 2 * UNIT, true, gap.lane);
        run(&mut game, 30);
        assert!(game.bunny.s > gap.s, "a Kit slips through");
        let mut game = quiet();
        place(&mut game, gap.edge, gap.s - 2 * UNIT, true, gap.lane);
        game.bunny.quarters = TIER_QUARTERS[2];
        game.bunny.tier = 2;
        run(&mut game, 30);
        assert!(matches!(game.bunny.mv, Move::Tumble { .. }));
        assert_eq!(game.bunny.tier, 1, "the bump costs growth and a size");
    }

    #[test]
    fn eating_grows_the_bunny_through_the_tiers() {
        let mut game = quiet();
        game.set_quarters(TIER_QUARTERS[1] - CARROT_QUARTERS);
        let quarters = game.bunny.quarters + CARROT_QUARTERS;
        game.set_quarters(quarters);
        assert_eq!(game.bunny.tier, 1);
        assert!(game.events.contains(&Event::Grew(1)));
        assert!(TIER_SPEED[1] > TIER_SPEED[0]);
    }

    /// Puts the farmer `tenths` of a metre ahead of the bunny in the same corridor,
    /// facing it, out of his shed.
    fn face_off(tenths: i32, standing: bool) -> Game {
        let mut game = Game::new(Garden::first());
        let edge = game.garden.spawn_edge;
        assert_eq!(game.garden.edges[edge].dx, 1);
        place(&mut game, edge, 2 * UNIT, true, 0);
        if standing {
            game.bunny.mv = Move::Wait;
        }
        game.farmer.edge = edge;
        game.farmer.fwd = false;
        game.farmer.s = 2 * UNIT + tenths * UNIT / 10;
        game.farmer.facing = (-1, 0);
        game.farmer.state = FarmerState::Chase { spell: 0 };
        game
    }

    #[test]
    fn the_net_winds_up_for_at_least_0_35_s_before_it_lands() {
        let mut game = face_off(15, true);
        let mut wound = None;
        for tick in 0..120 {
            game.step(&[]);
            if wound.is_none() && game.events.contains(&Event::WindUp) {
                wound = Some(tick);
            }
            if game.status == Status::Caught {
                let wound = wound.expect("a wind-up first");
                assert!(tick - wound >= ticks(350), "{tick} {wound}");
                return;
            }
        }
        panic!("never caught a bunny that stood still");
    }

    #[test]
    fn a_lane_change_during_the_wind_up_dodges_the_net() {
        let mut game = face_off(15, false);
        for _ in 0..120 {
            let dodge = if game.farmer.windup > 0 && game.bunny.lane == 0 {
                vec![Input::Right]
            } else {
                vec![]
            };
            game.step(&dodge);
            if game.events.contains(&Event::Missed) {
                assert_eq!(game.status, Status::Playing);
                return;
            }
        }
        panic!("the swing never missed: {:?}", game.status);
    }

    #[test]
    fn a_giant_bursts_out_of_the_first_net() {
        let mut game = face_off(15, true);
        game.bunny.quarters = TIER_QUARTERS[4] + 8;
        game.bunny.tier = 4;
        for _ in 0..120 {
            game.step(&[]);
            if game.events.contains(&Event::Escaped) {
                assert_eq!(game.bunny.tier, 3);
                assert_eq!(game.status, Status::Playing);
                return;
            }
        }
        panic!("no escape: {:?}", game.status);
    }

    #[test]
    fn the_farmer_waits_in_his_shed_then_comes_out() {
        let mut game = Game::new(Garden::first());
        let start = game.farmer_point();
        run(&mut game, SHED_WAIT - 1);
        assert_eq!(game.farmer_point(), start);
        run(&mut game, 120);
        assert_ne!(game.farmer_point(), start);
        assert_eq!(game.status, Status::Playing);
    }

    #[test]
    fn hedges_block_the_farmers_sight() {
        let mut game = Game::new(Garden::first());
        // The bunny across a hedge block, 8 m away diagonally.
        let (fx, fz) = game.farmer_point();
        let edge = game
            .garden
            .edges
            .iter()
            .position(|e| {
                let a = game.garden.nodes[e.a];
                (a.x - fx).abs() == 14 * UNIT && (a.z - fz).abs() == 14 * UNIT
            })
            .unwrap();
        place(&mut game, edge, 7 * UNIT, true, 0);
        let (bx, bz) = game.bunny_point();
        assert!(bx != fx && bz != fz);
        game.bunny.tier = 4;
        assert!(!game.farmer_sees());
    }

    #[test]
    fn the_same_inputs_give_the_same_run() {
        let script = |tick: u32| match tick % 97 {
            10 => vec![Input::Left],
            40 => vec![Input::Right],
            70 if tick % 3 == 0 => vec![Input::Back],
            _ => vec![],
        };
        let mut one = Game::new(Garden::first());
        let mut two = Game::new(Garden::first());
        for tick in 0..3_000 {
            one.step(&script(tick));
            two.step(&script(tick));
            assert_eq!(one.fingerprint(), two.fingerprint(), "tick {tick}");
        }
    }
}
