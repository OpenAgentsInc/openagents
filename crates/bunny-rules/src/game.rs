//! One garden run: the bunny, the farmer, and the fixed-step rules.

use sha2::{Digest, Sha256};

use crate::garden::{Exit, Garden, lane_offset};
use crate::kinds::{Contact, DANDELION_JUMPS, EdibleKind, PowerKind, bonus_points};
use crate::{
    CORRIDOR_HALF, HZ, LANE_WIDTH, TIER_QUARTERS, TIER_SIGHT, TIER_SPEED, UNIT, isqrt, ticks,
    tier_for,
};

/// Growth lost to a tumble (3 GP).
pub const TUMBLE_QUARTERS: u32 = 12;
/// Growth a Giant loses bursting out of the net (8 GP).
pub const ESCAPE_QUARTERS: u32 = 32;

/// A lane change takes 0.18 s.
pub const LANE_RATE: i32 = LANE_WIDTH * 1000 / (180 * HZ as i32);
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
/// A jump's air time at every size, and a dandelion float's.
pub const JUMP: u32 = ticks(550);
pub const FLOAT: u32 = ticks(1500);
/// How long a duck lasts.
pub const DUCK: u32 = ticks(600);
/// How long running through a puddle slows the bunny.
pub const SPLASH: u32 = ticks(600);
/// Edibles eaten within this of each other make a munch chain.
pub const CHAIN: u32 = ticks(1500);
/// How long the bonus vegetable stays.
pub const BONUS_TIME: u32 = ticks(10_000);
/// How long a spooked farmer, bumped, lies dazed in the compost.
pub const DAZED: u32 = ticks(5000);
/// The clear bonus, its reward per second under par, and the no-tumble
/// bonus.
pub const CLEAR_BONUS: u32 = 5_000;
pub const PAR_BONUS: u32 = 50;
pub const CLEAN_BONUS: u32 = 2_000;
/// The first bump of a spooked farmer, doubling after.
pub const BUMP_POINTS: u32 = 200;
pub const BUMP_MAX: u32 = 1_600;
/// The share of food eaten when the bonus vegetable appears, in percent.
pub const BONUS_AT: [usize; 2] = [35, 70];

/// The bunny's half length, from its centre to its nose.
const BODY: i32 = UNIT / 4;
/// How close an edible must be, along the corridor, to be eaten.
const EAT_REACH: i32 = UNIT * 4 / 10;
/// How close to a lane's centre counts as being in it.
const LANE_HIT: i32 = UNIT * 7 / 10;

const SHED_WAIT: u32 = ticks(3000);
const LOSE_SIGHT: u32 = ticks(4000);
const SEARCH: u32 = ticks(3000);
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
/// The Gentle mode's farmer speed and shortest wind-up.
const GENTLE_PERCENT: i32 = 85;
const GENTLE_WINDUP: u32 = ticks(700);
/// A spooked farmer runs at this share of his speed, in percent.
const SPOOKED_PERCENT: i32 = 70;

/// A player's command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Input {
    /// Lane left, or turn left at the next junction.
    Left,
    /// Lane right, or turn right at the next junction.
    Right,
    Jump,
    Duck,
    /// Turn back.
    Back,
    /// Leave the garden: neither a win nor a loss.
    Leave,
}

impl Input {
    pub const ALL: [Self; 6] = [
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::Duck,
        Self::Back,
        Self::Leave,
    ];

    /// A one-letter code, for receipts.
    #[must_use]
    pub fn code(self) -> char {
        match self {
            Self::Left => 'L',
            Self::Right => 'R',
            Self::Jump => 'J',
            Self::Duck => 'D',
            Self::Back => 'B',
            Self::Leave => 'X',
        }
    }

    #[must_use]
    pub fn from_code(code: char) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.code() == code)
    }
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
    /// The player left; neither a win nor a loss.
    Left,
}

impl Status {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Playing => "playing",
            Self::Won => "won",
            Self::Caught => "caught",
            Self::Left => "left",
        }
    }
}

/// Something that happened in the last step, for sounds and effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Ate edible `index`, scoring this many points.
    Ate(usize, u32),
    /// A munch chain reached this many edibles.
    Chain(u32),
    AteBonus(u32),
    BonusShown,
    BonusGone,
    PowerUp(usize),
    PowerOver,
    Grew(u8),
    Shrank(u8),
    Tumbled,
    Smashed(usize, u32),
    Splashed,
    Jumped,
    Ducked,
    Skidded,
    TurnedBack,
    Whistle,
    WindUp,
    Missed,
    Escaped,
    Spooked,
    /// Bumped the spooked farmer for this many points.
    Bumped(u32),
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
    /// Ticks left running slowly, and how slowly, in percent of its speed.
    pub slow: u32,
    pub slow_percent: u8,
    pub grace: u32,
    pub quarters: u32,
    pub tier: u8,
    /// Ticks left in the air, and the jump's whole length.
    pub air: u32,
    pub air_span: u32,
    /// Ticks left ducking.
    pub duck: u32,
    /// Floating dandelion jumps left.
    pub floats: u8,
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

    /// Whether it is in a floating dandelion jump.
    #[must_use]
    pub fn floating(&self) -> bool {
        self.air > 0 && self.air_span == FLOAT
    }

    /// How far through its jump it is, 0 to 1, for drawing.
    #[must_use]
    pub fn jump_progress(&self) -> f32 {
        if self.air == 0 || self.air_span == 0 {
            0.0
        } else {
            1.0 - self.air as f32 / self.air_span as f32
        }
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
    /// Tending the garden: walking his rounds.
    Patrol { next: usize },
    /// Running after the bunny, or the junction ahead of it.
    Chase { spell: u32 },
    /// Looking around where he last saw it.
    Search { left: u32, node: usize },
    /// Walking back to his shed for a breather.
    Scatter { left: u32 },
    /// Running away from the bunny while a golden carrot lasts.
    Spooked { left: u32 },
    /// In the compost after a spooked bump.
    Dazed { left: u32 },
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
    Away,
}

/// One garden run.
#[derive(Clone, Debug)]
pub struct Game {
    pub garden: Garden,
    pub seed: u64,
    /// Gentle mode: a slower farmer with a longer wind-up, and a stop with
    /// no way chosen turns toward more food.
    pub gentle: bool,
    pub bunny: Bunny,
    pub farmer: Farmer,
    /// Whether the farmer plays; off only in tests and practice.
    pub farmer_on: bool,
    pub eaten: Vec<bool>,
    /// Which obstacles still stand.
    pub alive: Vec<bool>,
    /// Which power-ups have been picked up.
    pub taken: Vec<bool>,
    pub food_left: usize,
    pub status: Status,
    pub tick: u32,
    pub score: u32,
    /// The munch chain's length and when its last edible was eaten.
    pub chain: u32,
    last_eat: Option<u32>,
    pub tumbles: u32,
    /// Spooked-farmer bumps so far.
    pub bumps: u32,
    /// The running power-up and its ticks left.
    pub power: Option<(PowerKind, u32)>,
    /// Ticks left of the golden carrot.
    pub golden: u32,
    /// Ticks left showing the bonus vegetable, and how many times it has
    /// appeared.
    pub bonus_left: u32,
    pub bonus_shown: u8,
    /// The tick the last edible was eaten.
    pub clear_tick: Option<u32>,
    /// What happened in the last step.
    pub events: Vec<Event>,
    rng: u64,
    /// Shortest walking distances between junctions for the farmer.
    dist: Vec<Vec<i64>>,
}

const FAR: i64 = i64::MAX / 4;

impl Game {
    /// A fresh run of `garden` with seed 0.
    #[must_use]
    pub fn new(garden: Garden) -> Self {
        Self::with_seed(garden, 0, false)
    }

    /// A fresh run of `garden`. The seed only picks where the farmer starts
    /// his rounds and how he breaks ties between equally good ways.
    #[must_use]
    pub fn with_seed(garden: Garden, seed: u64, gentle: bool) -> Self {
        let mut rng = seed ^ 0x9E37_79B9_7F4A_7C15;
        if rng == 0 {
            rng = 1;
        }
        let farmer = Self::farmer_at_shed(&garden, FarmerState::Shed { left: SHED_WAIT });
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
            slow_percent: 100,
            grace: 0,
            quarters: 0,
            tier: 0,
            air: 0,
            air_span: 0,
            duck: 0,
            floats: 0,
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
            eaten: vec![false; garden.edibles.len()],
            alive: vec![true; garden.obstacles.len()],
            taken: vec![false; garden.powers.len()],
            food_left: garden.food(),
            garden,
            seed,
            gentle,
            bunny,
            farmer,
            farmer_on: true,
            status: Status::Playing,
            tick: 0,
            score: 0,
            chain: 0,
            last_eat: None,
            tumbles: 0,
            bumps: 0,
            power: None,
            golden: 0,
            bonus_left: 0,
            bonus_shown: 0,
            clear_tick: None,
            events: Vec::new(),
            rng,
            dist,
        }
    }

    /// The farmer in his shed's doorway, as if he had just walked in, so his
    /// first step picks a way out.
    fn farmer_at_shed(garden: &Garden, state: FarmerState) -> Farmer {
        let exit = garden.exits(garden.shed)[0];
        Farmer {
            edge: exit.edge,
            fwd: !exit.fwd,
            s: if exit.fwd {
                0
            } else {
                garden.edges[exit.edge].len
            },
            lane_pos: 0,
            state,
            facing: (exit.dx, exit.dz),
            windup: 0,
            since_swing: u32::MAX / 2,
            recover: 0,
            stagger: 0,
            pause: 0,
            unseen: 0,
            last_seen: garden.shed,
        }
    }

    fn random(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
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

    /// Whether the bonus vegetable is out.
    #[must_use]
    pub fn bonus_out(&self) -> bool {
        self.bonus_left > 0
    }

    /// The bunny's run speed now, in units per tick.
    #[must_use]
    pub fn bunny_speed(&self) -> i32 {
        let mut speed = TIER_SPEED[usize::from(self.bunny.tier)];
        if matches!(self.power, Some((PowerKind::Clover, _))) {
            speed = speed * 125 / 100;
        }
        if self.bunny.slow > 0 {
            speed = speed * i32::from(self.bunny.slow_percent) / 100;
        }
        speed
    }

    /// The farmer's net wind-up in this run.
    #[must_use]
    pub fn windup(&self) -> u32 {
        if self.gentle {
            self.garden.farmer.windup.max(GENTLE_WINDUP)
        } else {
            self.garden.farmer.windup
        }
    }

    fn farmer_speed(&self, speed: i32) -> i32 {
        if self.gentle {
            speed * GENTLE_PERCENT / 100
        } else {
            speed
        }
    }

    /// Advances one tick with the inputs pressed since the last.
    pub fn step(&mut self, inputs: &[Input]) {
        self.events.clear();
        if self.status != Status::Playing {
            return;
        }
        self.tick += 1;
        for input in inputs {
            if *input == Input::Leave {
                self.status = Status::Left;
                return;
            }
            self.press(*input);
        }
        self.step_timers();
        self.step_bunny();
        if self.food_left == 0 {
            self.win();
            return;
        }
        if self.farmer_on && self.status == Status::Playing {
            self.step_farmer();
        }
    }

    fn win(&mut self) {
        self.status = Status::Won;
        self.clear_tick = Some(self.tick);
        let seconds = self.tick.div_ceil(HZ);
        self.score += CLEAR_BONUS + PAR_BONUS * self.garden.par.saturating_sub(seconds);
        if self.tumbles == 0 {
            self.score += CLEAN_BONUS;
        }
        self.events.push(Event::Won);
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
            _ => None,
        };
        match self.bunny.mv {
            Move::Tumble { .. } | Move::UTurn { .. } => {}
            Move::Skid { .. } | Move::Wait => match (input, side) {
                (_, Some(side)) => {
                    if let Some(exit) = self.exit_on(side) {
                        self.take(exit);
                        self.bunny.mv = Move::Run;
                    }
                }
                (Input::Back, _) => self.turn_back(),
                (Input::Jump, _) => self.jump(),
                (Input::Duck, _) => self.duck(),
                _ => {}
            },
            Move::Run => match input {
                Input::Left | Input::Right => {
                    let side = side.unwrap_or(Side::Left);
                    let near =
                        self.bunny.to_end(&self.garden) <= self.bunny_speed() * BUFFER as i32;
                    if near && self.exit_on(side).is_some() {
                        self.bunny.pending = Some(side);
                    } else {
                        let step = if side == Side::Left { -1 } else { 1 };
                        self.bunny.lane = (self.bunny.lane + step).clamp(-1, 1);
                    }
                }
                Input::Jump => self.jump(),
                Input::Duck => self.duck(),
                Input::Back => {
                    if self.bunny.cooldown == 0 {
                        self.turn_back();
                    }
                }
                Input::Leave => {}
            },
        }
    }

    fn jump(&mut self) {
        if self.bunny.air == 0 && self.bunny.duck == 0 {
            let span = if self.bunny.floats > 0 {
                self.bunny.floats -= 1;
                FLOAT
            } else {
                JUMP
            };
            self.bunny.air = span;
            self.bunny.air_span = span;
            self.events.push(Event::Jumped);
        }
    }

    fn duck(&mut self) {
        if self.bunny.air == 0 && self.bunny.duck == 0 {
            self.bunny.duck = DUCK;
            self.events.push(Event::Ducked);
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
        if !matches!(self.power, Some((PowerKind::Clover, _))) {
            let quarters = self.bunny.quarters.saturating_sub(TUMBLE_QUARTERS);
            self.set_quarters(quarters);
        }
        self.tumbles += 1;
        self.bunny.mv = Move::Tumble { left: TUMBLE };
        self.bunny.pending = None;
        self.bunny.air = 0;
        self.bunny.duck = 0;
        self.events.push(Event::Tumbled);
    }

    fn step_timers(&mut self) {
        if let Some((kind, left)) = self.power {
            if left > 1 {
                self.power = Some((kind, left - 1));
            } else {
                self.power = None;
                if kind == PowerKind::Dandelion {
                    self.bunny.floats = 0;
                }
                self.events.push(Event::PowerOver);
            }
        }
        self.golden = self.golden.saturating_sub(1);
        if self.bonus_left > 0 {
            self.bonus_left -= 1;
            if self.bonus_left == 0 {
                self.events.push(Event::BonusGone);
            }
        }
    }

    fn step_bunny(&mut self) {
        let b = &mut self.bunny;
        b.cooldown = b.cooldown.saturating_sub(1);
        b.slow = b.slow.saturating_sub(1);
        b.grace = b.grace.saturating_sub(1);
        b.air = b.air.saturating_sub(1);
        b.duck = b.duck.saturating_sub(1);
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
                    b.slow_percent = 60;
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
                } else if let Some(exit) = then.or_else(|| self.gentle_way()) {
                    self.bunny.mv = Move::Run;
                    self.take(exit);
                } else {
                    self.bunny.mv = Move::Wait;
                }
            }
            Move::Wait => {}
            Move::Run => {
                self.run();
                if self.bunny.mv == Move::Run && self.farmer_on {
                    self.meet_farmer();
                }
            }
        }
    }

    /// In Gentle mode, the way with more food at a stop.
    fn gentle_way(&self) -> Option<Exit> {
        if !self.gentle {
            return None;
        }
        let food = |exit: &Exit| {
            self.garden
                .edibles
                .iter()
                .enumerate()
                .filter(|(index, e)| !self.eaten[*index] && e.edge == exit.edge)
                .count()
        };
        let left = self.exit_on(Side::Left);
        let right = self.exit_on(Side::Right);
        match (left, right) {
            (Some(l), Some(r)) => Some(if food(&r) > food(&l) { r } else { l }),
            (one, other) => one.or(other),
        }
    }

    fn meet_farmer(&mut self) {
        if matches!(
            self.farmer.state,
            FarmerState::Shed { .. } | FarmerState::Dazed { .. }
        ) {
            return;
        }
        let (bx, bz) = self.bunny_point();
        let (fx, fz) = self.farmer_point();
        let (hx, hz) = self.bunny.heading(&self.garden);
        let (dx, dz) = (fx - bx, fz - bz);
        if matches!(self.farmer.state, FarmerState::Spooked { .. }) {
            if dx.abs() < UNIT * 9 / 10 && dz.abs() < UNIT * 9 / 10 {
                self.daze();
            }
            return;
        }
        let forward = dx * hx + dz * hz;
        let lateral = -dx * hz + dz * hx;
        if forward > 0 && forward < UNIT * 7 / 10 && lateral.abs() < UNIT * 6 / 10 {
            self.tumble();
        }
    }

    fn daze(&mut self) {
        let points = (BUMP_POINTS << self.bumps.min(3)).min(BUMP_MAX);
        self.bumps += 1;
        self.score += points;
        self.farmer = Self::farmer_at_shed(&self.garden, FarmerState::Dazed { left: DAZED });
        self.events.push(Event::Bumped(points));
    }

    fn run(&mut self) {
        let mut remaining = self.bunny_speed();
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

    /// Runs into what stands between `s0` and `s1`; true when it stopped.
    fn contact(&mut self, s0: i32, s1: i32, dir: i32) -> bool {
        let (f0, f1) = (s0 + dir * BODY, s1 + dir * BODY);
        let lateral = self.bunny.lateral();
        let edge = self.bunny.edge;
        let crosses = |s: i32| {
            if dir > 0 {
                f0 < s && s <= f1
            } else {
                f1 <= s && s < f0
            }
        };
        let in_lane = |lane: i8| (lateral - lane_offset(lane)).abs() < LANE_HIT;
        let mut hits: Vec<(i32, Option<usize>, Option<usize>)> = Vec::new();
        for (index, o) in self.garden.obstacles.iter().enumerate() {
            if self.alive[index] && o.edge == edge && in_lane(o.lane) && crosses(o.s) {
                hits.push((o.s, Some(index), None));
            }
        }
        let tier = self.bunny.tier;
        for (index, e) in self.garden.edibles.iter().enumerate() {
            if !self.eaten[index]
                && !e.air
                && e.kind.min_tier() > tier
                && e.edge == edge
                && in_lane(e.lane)
                && crosses(e.s)
            {
                hits.push((e.s, None, Some(index)));
            }
        }
        hits.sort_by_key(|(s, _, _)| (s - f0).abs());
        for (s, obstacle, _) in hits {
            if self.bunny.floating() {
                continue;
            }
            let contact = match obstacle {
                Some(index) => self.garden.obstacles[index].kind.contact(tier),
                None => Contact::Block,
            };
            let passed = match contact {
                Contact::Pass => true,
                Contact::Slow(percent) => {
                    self.bunny.slow = self.bunny.slow.max(SPLASH);
                    self.bunny.slow_percent = percent;
                    self.events.push(Event::Splashed);
                    true
                }
                Contact::Jump => self.bunny.air > 0,
                Contact::Duck => self.bunny.duck > 0,
                Contact::Smash => {
                    if let Some(index) = obstacle {
                        self.alive[index] = false;
                        let points = self.garden.obstacles[index].kind.smash_points();
                        self.score += points;
                        self.events.push(Event::Smashed(index, points));
                    }
                    true
                }
                Contact::Block => false,
            };
            if !passed {
                self.bunny.s = s - dir * (BODY + 1);
                self.tumble();
                return true;
            }
        }
        false
    }

    fn eat(&mut self, s0: i32, s1: i32) {
        let (lo, hi) = (s0.min(s1) - EAT_REACH, s0.max(s1) + EAT_REACH);
        let lateral = self.bunny.lateral();
        let reach = if matches!(self.power, Some((PowerKind::Magnet, _))) {
            LANE_WIDTH + LANE_HIT
        } else {
            LANE_HIT
        };
        let edge = self.bunny.edge;
        let near = |e_edge: usize, s: i32, lane: i8| {
            e_edge == edge && s >= lo && s <= hi && (lateral - lane_offset(lane)).abs() < reach
        };
        for index in 0..self.garden.edibles.len() {
            let e = self.garden.edibles[index];
            if !self.eaten[index]
                && near(e.edge, e.s, e.lane)
                && e.kind.min_tier() <= self.bunny.tier
                && (!e.air || self.bunny.air > 0)
            {
                self.eaten[index] = true;
                self.food_left -= 1;
                let points = self.chained(e.kind.points());
                self.events.push(Event::Ate(index, points));
                self.set_quarters(self.bunny.quarters + e.kind.quarters());
                if e.kind == EdibleKind::Golden {
                    self.spook();
                }
                self.maybe_bonus();
            }
        }
        let spot = self.garden.bonus;
        if self.bonus_left > 0 && near(spot.edge, spot.s, spot.lane) {
            self.bonus_left = 0;
            let points = self.chained(bonus_points(self.garden.set));
            self.events.push(Event::AteBonus(points));
        }
        for index in 0..self.garden.powers.len() {
            let p = self.garden.powers[index];
            if !self.taken[index] && near(p.edge, p.s, p.lane) {
                self.taken[index] = true;
                if let Some((PowerKind::Dandelion, _)) = self.power {
                    self.bunny.floats = 0;
                }
                self.power = Some((p.kind, p.kind.duration()));
                if p.kind == PowerKind::Dandelion {
                    self.bunny.floats = DANDELION_JUMPS;
                }
                self.events.push(Event::PowerUp(index));
            }
        }
    }

    /// Scores `points` in the munch chain.
    fn chained(&mut self, points: u32) -> u32 {
        let in_chain = self
            .last_eat
            .is_some_and(|at| self.tick.saturating_sub(at) <= CHAIN);
        self.chain = if in_chain { self.chain + 1 } else { 1 };
        self.last_eat = Some(self.tick);
        if self.chain >= 3 {
            self.events.push(Event::Chain(self.chain));
        }
        let points = if self.chain >= 8 {
            points * 2
        } else if self.chain >= 4 {
            points * 3 / 2
        } else {
            points
        };
        self.score += points;
        points
    }

    fn maybe_bonus(&mut self) {
        let food = self.garden.food();
        let eaten = food - self.food_left;
        let due = BONUS_AT
            .iter()
            .filter(|percent| eaten * 100 >= food * **percent)
            .count() as u8;
        if due > self.bonus_shown && self.food_left > 0 {
            self.bonus_shown = due;
            self.bonus_left = BONUS_TIME;
            self.events.push(Event::BonusShown);
        }
    }

    fn spook(&mut self) {
        let spook = self.garden.farmer.spook;
        self.golden = spook;
        if !matches!(
            self.farmer.state,
            FarmerState::Shed { .. } | FarmerState::Dazed { .. }
        ) && self.farmer_on
        {
            self.farmer.state = FarmerState::Spooked { left: spook };
            self.farmer.windup = 0;
            self.events.push(Event::Spooked);
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

    /// How far the farmer sees the bunny now.
    #[must_use]
    pub fn sight(&self) -> i32 {
        let sight = TIER_SIGHT[usize::from(self.bunny.tier)];
        if matches!(self.power, Some((PowerKind::SunHat, _))) {
            sight / 2
        } else {
            sight
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
        let sight = i64::from(self.sight());
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

    fn patrol_start(&mut self) -> usize {
        (self.random() % self.garden.patrol.len() as u64) as usize
    }

    fn step_farmer(&mut self) {
        self.farmer.since_swing = self.farmer.since_swing.saturating_add(1);
        if let FarmerState::Dazed { left } = self.farmer.state {
            if left > 1 {
                self.farmer.state = FarmerState::Dazed { left: left - 1 };
            } else {
                let next = self.patrol_start();
                self.farmer.state = FarmerState::Patrol { next };
            }
            return;
        }
        let f = &mut self.farmer;
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
        let plan = self.garden.farmer;
        match self.farmer.state {
            FarmerState::Shed { left } => {
                if left > 1 {
                    self.farmer.state = FarmerState::Shed { left: left - 1 };
                    return;
                }
                let next = self.patrol_start();
                self.farmer.state = FarmerState::Patrol { next };
            }
            FarmerState::Patrol { .. } | FarmerState::Search { .. } => {
                if visible {
                    self.farmer.state = FarmerState::Chase { spell: 0 };
                    self.farmer.unseen = 0;
                    self.farmer.last_seen = ahead;
                    self.events.push(Event::Whistle);
                }
            }
            FarmerState::Chase { spell } => {
                let f = &mut self.farmer;
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
                } else if plan.scatter > 0 && spell + 1 >= plan.chase {
                    FarmerState::Scatter { left: plan.scatter }
                } else {
                    FarmerState::Chase { spell: spell + 1 }
                };
            }
            FarmerState::Scatter { left } => {
                self.farmer.state = if left > 1 {
                    FarmerState::Scatter { left: left - 1 }
                } else {
                    self.farmer.unseen = 0;
                    FarmerState::Chase { spell: 0 }
                };
            }
            FarmerState::Spooked { left } => {
                if left > 1 {
                    self.farmer.state = FarmerState::Spooked { left: left - 1 };
                } else if visible {
                    self.farmer.state = FarmerState::Chase { spell: 0 };
                    self.events.push(Event::Whistle);
                } else {
                    let next = self.patrol_start();
                    self.farmer.state = FarmerState::Patrol { next };
                }
            }
            FarmerState::Dazed { .. } => {}
        }
        let spooked = matches!(self.farmer.state, FarmerState::Spooked { .. });
        if visible && !spooked && self.bunny.grace == 0 && self.in_trigger() {
            let (bx, bz) = self.bunny_point();
            let (fx, fz) = self.farmer_point();
            let (dx, dz) = (bx - fx, bz - fz);
            self.farmer.facing = if dx.abs() >= dz.abs() {
                (dx.signum(), 0)
            } else {
                (0, dz.signum())
            };
            self.farmer.windup = self.windup();
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
        let lands = (-UNIT * 3 / 10..=NET_REACH).contains(&forward)
            && lateral.abs() <= NET_HALF
            && self.bunny.grace == 0
            && self.bunny.air == 0
            && self.bunny.duck == 0;
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

    /// The bunny's walking distance from junction `node` for the farmer.
    fn to_bunny(&self, node: usize) -> i64 {
        let e = &self.garden.edges[self.bunny.edge];
        if self.garden.farmer_may_walk(self.bunny.edge) {
            (self.dist[node][e.a] + i64::from(self.bunny.s))
                .min(self.dist[node][e.b] + i64::from(e.len - self.bunny.s))
        } else {
            self.dist[node][e.a].min(self.dist[node][e.b])
        }
    }

    /// The farmer's walking distance from junction `node` to the target;
    /// for running away, the less the farther from the bunny.
    fn distance(&self, node: usize, target: &Target) -> i64 {
        match target {
            Target::Node(goal) => self.dist[node][*goal],
            Target::Bunny => self.to_bunny(node),
            Target::Away => -self.to_bunny(node).min(FAR / 2),
        }
    }

    fn farmer_target(&self) -> Option<(Target, i32)> {
        let g = &self.garden;
        let plan = g.farmer;
        let speed = |s: i32| self.farmer_speed(s);
        match self.farmer.state {
            FarmerState::Shed { .. } | FarmerState::Dazed { .. } => None,
            FarmerState::Patrol { next } => Some((
                Target::Node(g.patrol[next % g.patrol.len()]),
                speed(plan.walk),
            )),
            FarmerState::Chase { .. } => {
                let ahead = g.edges[self.bunny.edge].end(self.bunny.fwd);
                let target = if plan.ambush && self.farmer.edge != self.bunny.edge {
                    Target::Node(ahead)
                } else {
                    Target::Bunny
                };
                Some((target, speed(plan.run)))
            }
            FarmerState::Search { node, .. } => Some((Target::Node(node), speed(plan.run))),
            FarmerState::Scatter { .. } => Some((Target::Node(g.shed), speed(plan.walk))),
            FarmerState::Spooked { .. } => {
                Some((Target::Away, speed(plan.run) * SPOOKED_PERCENT / 100))
            }
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
        let same_edge = self.farmer.edge == self.bunny.edge;
        let lane_goal = match target {
            Target::Bunny if same_edge => self.bunny.lateral(),
            Target::Node(_) if same_edge && self.garden.farmer.ambush => self.bunny.lateral(),
            _ => 0,
        };
        self.farmer.lane_pos +=
            (lane_goal - self.farmer.lane_pos).clamp(-FARMER_LANE_RATE, FARMER_LANE_RATE);
        if same_edge && !matches!(target, Target::Away) {
            // In the bunny's corridor he goes straight for it.
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
        if same_edge {
            // Spooked in the bunny's corridor: away from it.
            let want = self.bunny.s < self.farmer.s;
            if want != self.farmer.fwd {
                self.turn_farmer();
                return;
            }
        } else {
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
                        let next = self.patrol_start();
                        self.farmer.state = FarmerState::Patrol { next };
                    }
                }
                _ => {}
            }
            return false;
        }
        let stay = self.distance(node, target);
        let mut best: Vec<(i64, Exit)> = Vec::new();
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
            let cost = if matches!(target, Target::Away) {
                // Never toward the bunny's corridor, whatever lies beyond.
                if exit.edge == self.bunny.edge {
                    FAR
                } else {
                    self.distance(e.end(exit.fwd), target)
                }
            } else {
                cost
            };
            match best.first() {
                Some((old, _)) if cost > *old => {}
                Some((old, _)) if cost == *old => best.push((cost, *exit)),
                _ => best = vec![(cost, *exit)],
            }
        }
        if best.is_empty() {
            return false;
        }
        let pick = if best.len() > 1 {
            (self.random() % best.len() as u64) as usize
        } else {
            0
        };
        let (cost, exit) = best[pick];
        if !matches!(target, Target::Away) && stay <= 0 && cost > 0 {
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

    /// SHA-256 of the whole state, for checking that two runs match.
    #[must_use]
    pub fn state_digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        let mut put = |value: i64| hash.update(value.to_le_bytes());
        let b = &self.bunny;
        let f = &self.farmer;
        let mv = |m: Move| match m {
            Move::Run => (0, 0),
            Move::Skid { left, then } => (1, i64::from(left) * 2 + i64::from(then.is_some())),
            Move::Wait => (2, 0),
            Move::UTurn { left } => (3, i64::from(left)),
            Move::Tumble { left } => (4, i64::from(left)),
        };
        let (m, ml) = mv(b.mv);
        let fs = match f.state {
            FarmerState::Shed { left } => (0, i64::from(left)),
            FarmerState::Patrol { next } => (1, next as i64),
            FarmerState::Chase { spell } => (2, i64::from(spell)),
            FarmerState::Search { left, node } => (3, i64::from(left) * 1000 + node as i64),
            FarmerState::Scatter { left } => (4, i64::from(left)),
            FarmerState::Spooked { left } => (5, i64::from(left)),
            FarmerState::Dazed { left } => (6, i64::from(left)),
        };
        for value in [
            b.edge as i64,
            i64::from(b.fwd),
            i64::from(b.s),
            i64::from(b.lane),
            i64::from(b.lane_pos),
            m,
            ml,
            i64::from(b.quarters),
            i64::from(b.air),
            i64::from(b.duck),
            i64::from(b.slow),
            i64::from(b.cooldown),
            f.edge as i64,
            i64::from(f.fwd),
            i64::from(f.s),
            i64::from(f.lane_pos),
            fs.0,
            fs.1,
            i64::from(f.windup),
            self.food_left as i64,
            i64::from(self.tick),
            i64::from(self.score),
            i64::from(self.chain),
            i64::from(self.tumbles),
            i64::from(self.bumps),
            i64::from(self.golden),
            i64::from(self.bonus_left),
            self.rng as i64,
        ] {
            put(value);
        }
        for flags in [&self.eaten, &self.alive, &self.taken] {
            for chunk in flags.chunks(8) {
                let byte = chunk
                    .iter()
                    .enumerate()
                    .fold(0_u8, |acc, (i, on)| acc | (u8::from(*on) << i));
                hash.update([byte]);
            }
        }
        hash.update([self.status as u8]);
        hash.finalize().into()
    }

    /// The first eight bytes of [`Self::state_digest`].
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let digest = self.state_digest();
        u64::from_le_bytes(digest[..8].try_into().expect("eight bytes"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::garden::Obstacle;
    use crate::kinds::ObstacleKind;
    use crate::level;

    fn quiet(number: usize) -> Game {
        let mut game = Game::new(level::garden(number));
        game.farmer_on = false;
        game
    }

    /// Garden 1 with the bunny in its spawn corridor's row of seedlings.
    fn in_the_seedlings() -> Game {
        let mut game = quiet(1);
        let (edge, s, fwd) = (
            game.garden.spawn_edge,
            game.garden.spawn_s,
            game.garden.spawn_fwd,
        );
        let lane = game.garden.edibles[0].lane;
        place(&mut game, edge, s, fwd, if fwd { lane } else { -lane });
        game
    }

    fn run(game: &mut Game, ticks: u32) {
        for _ in 0..ticks {
            game.step(&[]);
        }
    }

    fn place(game: &mut Game, edge: usize, s: i32, fwd: bool, lane: i8) {
        game.bunny.edge = edge;
        game.bunny.s = s;
        game.bunny.fwd = fwd;
        game.bunny.lane = lane;
        game.bunny.lane_pos = lane_offset(lane);
    }

    fn grow(game: &mut Game, tier: u8) {
        game.bunny.quarters = TIER_QUARTERS[usize::from(tier)];
        game.bunny.tier = tier;
    }

    /// An obstacle of `kind` 6 m ahead of the bunny in its lane, on a
    /// corridor of garden 1 with nothing else on it.
    fn ahead(kind: ObstacleKind, tier: u8) -> (Game, usize) {
        let mut game = quiet(1);
        let edge = game.garden.spawn_edge;
        game.garden.edibles.retain(|e| e.edge != edge);
        game.eaten = vec![false; game.garden.edibles.len()];
        game.food_left = game.garden.food();
        game.garden.obstacles.clear();
        game.garden.obstacles.push(Obstacle {
            edge,
            s: 10 * UNIT,
            lane: 0,
            kind,
        });
        game.alive = vec![true];
        place(&mut game, edge, 4 * UNIT, true, 0);
        grow(&mut game, tier);
        (game, edge)
    }

    /// Runs until the bunny is `metres` short of the obstacle at 10 m.
    fn approach(game: &mut Game, tenths: i32) {
        while game.bunny.s < 10 * UNIT - tenths * UNIT / 10 {
            game.step(&[]);
        }
    }

    #[test]
    fn the_bunny_runs_forward_and_eats_what_is_in_its_lane() {
        let mut game = in_the_seedlings();
        let start = game.bunny.s;
        run(&mut game, 90);
        assert!(game.bunny.s > start);
        let eaten = game.eaten.iter().filter(|e| **e).count();
        assert!(eaten >= 2, "{eaten}");
        assert!(game.score >= 20);
    }

    #[test]
    fn left_and_right_change_lanes_away_from_junctions() {
        let mut game = quiet(1);
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
        let mut game = quiet(1);
        let heading = game.bunny.heading(&game.garden);
        while game.bunny.to_end(&game.garden) > 15_000 {
            game.step(&[]);
        }
        let side = if game.exit_on(Side::Right).is_some() {
            Side::Right
        } else {
            Side::Left
        };
        game.step(&[if side == Side::Right {
            Input::Right
        } else {
            Input::Left
        }]);
        assert_eq!(game.bunny.pending, Some(side));
        assert_eq!(game.bunny.lane, 0);
        run(&mut game, 20);
        assert_ne!(game.bunny.heading(&game.garden), heading, "it turned");
    }

    #[test]
    fn turning_back_reverses_and_then_cools_down() {
        let mut game = quiet(1);
        run(&mut game, 5);
        let fwd = game.bunny.fwd;
        game.step(&[Input::Back]);
        assert_ne!(game.bunny.fwd, fwd);
        run(&mut game, UTURN + 5);
        assert_eq!(game.bunny.mv, Move::Run);
        game.step(&[Input::Back]);
        assert_ne!(game.bunny.fwd, fwd, "still cooling down");
    }

    #[test]
    fn a_small_bunny_tumbles_into_a_pot_loses_growth_and_turns_back() {
        let (mut game, _) = ahead(ObstacleKind::Pot, 0);
        game.bunny.quarters = 20;
        run(&mut game, 60);
        assert_eq!(game.tumbles, 1);
        assert_eq!(game.bunny.quarters, 8);
        assert!(game.alive[0]);
        run(&mut game, TUMBLE);
        assert!(!game.bunny.fwd, "bounced back the way it came");
        assert!(game.bunny.slow > 0);
    }

    #[test]
    fn a_jack_smashes_the_pot_for_points() {
        let (mut game, _) = ahead(ObstacleKind::Pot, 2);
        run(&mut game, 60);
        assert!(!game.alive[0]);
        assert_eq!(game.bunny.mv, Move::Run);
        assert_eq!(game.score, 50);
        assert_eq!(game.tumbles, 0);
    }

    #[test]
    fn every_obstacle_does_what_the_table_says_at_every_size() {
        for kind in ObstacleKind::ALL {
            for tier in 0..5_u8 {
                let contact = kind.contact(tier);
                // Running into it with no action.
                let (mut game, _) = ahead(kind, tier);
                run(&mut game, 60);
                let tumbled = game.tumbles > 0;
                match contact {
                    Contact::Pass | Contact::Slow(_) | Contact::Smash => {
                        assert!(!tumbled, "{kind:?} at {tier}");
                        assert_eq!(!game.alive[0], contact == Contact::Smash, "{kind:?}");
                    }
                    Contact::Jump | Contact::Duck | Contact::Block => {
                        assert!(tumbled, "{kind:?} at {tier}");
                    }
                }
                // With the right action just before it.
                if matches!(contact, Contact::Jump | Contact::Duck) {
                    let (mut game, _) = ahead(kind, tier);
                    approach(&mut game, 15);
                    game.step(&[if contact == Contact::Jump {
                        Input::Jump
                    } else {
                        Input::Duck
                    }]);
                    run(&mut game, 60);
                    assert_eq!(game.tumbles, 0, "{kind:?} at {tier} with {contact:?}");
                    assert!(game.bunny.s > 10 * UNIT);
                }
            }
        }
    }

    #[test]
    fn a_puddle_slows_a_kit_but_not_a_jack() {
        let (mut game, _) = ahead(ObstacleKind::Puddle, 0);
        approach(&mut game, 1);
        run(&mut game, 2);
        assert_eq!(game.bunny_speed(), TIER_SPEED[0] * 60 / 100);
        let (mut game, _) = ahead(ObstacleKind::Puddle, 2);
        approach(&mut game, 1);
        run(&mut game, 2);
        assert_eq!(game.bunny_speed(), TIER_SPEED[2]);
    }

    #[test]
    fn a_dandelion_float_clears_anything_and_a_clover_saves_growth() {
        let (mut game, _) = ahead(ObstacleKind::Scarecrow, 0);
        game.bunny.floats = 1;
        approach(&mut game, 20);
        game.step(&[Input::Jump]);
        assert!(game.bunny.floating());
        run(&mut game, 60);
        assert_eq!(game.tumbles, 0);
        assert!(game.alive[0], "floated over it");
        let (mut game, _) = ahead(ObstacleKind::Pot, 1);
        game.bunny.quarters = 40;
        game.power = Some((PowerKind::Clover, 1_000));
        run(&mut game, 60);
        assert_eq!((game.tumbles, game.bunny.quarters), (1, 40));
    }

    #[test]
    fn too_big_food_is_solid_and_lettuce_feeds_a_jack() {
        let mut game = quiet(3);
        let (index, lettuce) = game
            .garden
            .edibles
            .iter()
            .copied()
            .enumerate()
            .find(|(_, e)| e.kind == EdibleKind::Lettuce)
            .unwrap();
        place(
            &mut game,
            lettuce.edge,
            lettuce.s - 3 * UNIT,
            true,
            lettuce.lane,
        );
        game.garden.edibles.retain(|e| {
            e.edge != lettuce.edge || e.kind == EdibleKind::Lettuce || e.lane != lettuce.lane
        });
        let index = game
            .garden
            .edibles
            .iter()
            .position(|e| *e == lettuce)
            .unwrap_or(index);
        game.eaten = vec![false; game.garden.edibles.len()];
        game.food_left = game.garden.food();
        game.garden.obstacles.retain(|o| o.edge != lettuce.edge);
        game.alive = vec![true; game.garden.obstacles.len()];
        let mut big = game.clone();
        run(&mut game, 40);
        assert_eq!(game.tumbles, 1);
        assert!(!game.eaten[index]);
        grow(&mut big, 2);
        run(&mut big, 40);
        assert!(big.eaten[index]);
        assert_eq!(big.bunny.quarters, TIER_QUARTERS[2] + 12);
    }

    #[test]
    fn munch_chains_multiply_and_a_win_adds_the_clear_bonuses() {
        let mut game = in_the_seedlings();
        let mut points = Vec::new();
        for _ in 0..200 {
            game.step(&[]);
            for event in &game.events {
                if let Event::Ate(_, p) = event {
                    points.push(*p);
                }
            }
        }
        // The spawn corridor's row of seedlings: 10 each, then 15 from the
        // fourth in a chain.
        assert_eq!(&points[..3], &[10, 10, 10]);
        assert_eq!(points[3], 15);
        if points.len() >= 8 {
            assert_eq!(points[7], 20);
        }
        let mut game = quiet(1);
        let before = game.score;
        let last = game.eaten.len() - 1;
        for e in &mut game.eaten[..last] {
            *e = true;
        }
        game.food_left = 1;
        let edible = game.garden.edibles[last];
        place(&mut game, edible.edge, edible.s - UNIT, true, edible.lane);
        if game.garden.edges[edible.edge].len - edible.s < 0 {
            unreachable!();
        }
        game.garden.obstacles.retain(|o| o.edge != edible.edge);
        game.alive = vec![true; game.garden.obstacles.len()];
        run(&mut game, 30);
        assert_eq!(game.status, Status::Won);
        assert!(game.clear_tick.is_some());
        assert!(
            game.score - before >= CLEAR_BONUS + CLEAN_BONUS + PAR_BONUS * (game.garden.par - 1),
            "{}",
            game.score
        );
    }

    #[test]
    fn the_bonus_vegetable_comes_out_at_35_and_70_percent_and_goes() {
        let mut game = in_the_seedlings();
        let food = game.garden.food();
        let target = food * 35 / 100 + 1;
        let mut count = 0;
        for (index, e) in game.eaten.iter_mut().enumerate() {
            if count + 1 < target && index > 4 {
                *e = true;
                count += 1;
            }
        }
        game.food_left = food - count;
        run(&mut game, 90);
        assert!(game.bonus_out(), "out after the next bite");
        assert_eq!(game.bonus_shown, 1);
        run(&mut game, BONUS_TIME);
        assert!(!game.bonus_out());
    }

    /// Puts the farmer `tenths` of a metre ahead of the bunny in the same
    /// corridor, facing it, out of his shed.
    fn face_off(tenths: i32, standing: bool) -> Game {
        let mut game = Game::new(level::garden(1));
        let edge = game.garden.spawn_edge;
        game.garden.edibles.retain(|e| e.edge != edge);
        game.eaten = vec![false; game.garden.edibles.len()];
        game.food_left = game.garden.food();
        let fwd = game.garden.spawn_fwd;
        let start = if fwd { 4 * UNIT } else { 16 * UNIT };
        place(&mut game, edge, start, fwd, 0);
        if standing {
            game.bunny.mv = Move::Wait;
        }
        game.farmer.edge = edge;
        game.farmer.fwd = !fwd;
        game.farmer.s = start + if fwd { 1 } else { -1 } * tenths * UNIT / 10;
        game.face_farmer();
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

    fn dodge_with(input: Input) {
        let mut game = face_off(15, true);
        for _ in 0..120 {
            let dodge = if game.farmer.windup == 1 {
                vec![input]
            } else {
                vec![]
            };
            game.step(&dodge);
            if game.events.contains(&Event::Missed) {
                assert_eq!(game.status, Status::Playing);
                return;
            }
            assert_eq!(game.status, Status::Playing, "caught despite {input:?}");
        }
        panic!("the swing never missed: {:?}", game.status);
    }

    #[test]
    fn a_jump_or_a_duck_during_the_wind_up_dodges_the_net() {
        dodge_with(Input::Jump);
        dodge_with(Input::Duck);
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
    fn a_giant_bursts_out_of_the_first_net_but_not_a_second_soon_after() {
        let mut game = face_off(15, true);
        grow(&mut game, 4);
        game.bunny.quarters += 8;
        let mut escaped = false;
        for _ in 0..600 {
            game.step(&[]);
            if game.events.contains(&Event::Escaped) {
                assert_eq!(game.bunny.tier, 3);
                escaped = true;
                grow(&mut game, 4);
                game.bunny.mv = Move::Wait;
            }
            if game.status == Status::Caught {
                assert!(escaped);
                return;
            }
        }
        panic!("no escape and catch: {:?} {escaped}", game.status);
    }

    #[test]
    fn no_swing_lands_in_the_grace_after_a_tumble() {
        let mut game = face_off(15, true);
        game.bunny.grace = GRACE;
        for _ in 0..GRACE - 1 {
            game.step(&[]);
            assert_eq!(game.farmer.windup, 0, "no wind-up during the grace");
        }
    }

    #[test]
    fn a_golden_carrot_spooks_the_farmer_and_bumps_daze_him_for_rising_points() {
        let mut game = face_off(40, false);
        game.farmer.state = FarmerState::Patrol { next: 0 };
        game.spook();
        assert!(matches!(game.farmer.state, FarmerState::Spooked { .. }));
        game.farmer.pause = 1_000;
        let mut points = Vec::new();
        for _ in 0..300 {
            game.step(&[]);
            for e in &game.events {
                if let Event::Bumped(p) = e {
                    points.push(*p);
                }
            }
            if !points.is_empty() {
                break;
            }
        }
        assert_eq!(points, [200]);
        assert!(matches!(game.farmer.state, FarmerState::Dazed { .. }));
        // He can't catch anyone while dazed, then gets up and tends again.
        for _ in 0..DAZED {
            game.step(&[]);
            assert_ne!(game.status, Status::Caught);
        }
        assert!(matches!(game.farmer.state, FarmerState::Patrol { .. }));
        game.bumps = 3;
        let before = game.score;
        game.daze();
        game.daze();
        assert_eq!(game.score - before, 1_600 + 1_600);
    }

    #[test]
    fn the_farmer_waits_in_his_shed_then_comes_out() {
        let mut game = Game::new(level::garden(1));
        let start = game.farmer_point();
        run(&mut game, SHED_WAIT - 1);
        assert_eq!(game.farmer_point(), start);
        run(&mut game, 120);
        assert_ne!(game.farmer_point(), start);
        assert_eq!(game.status, Status::Playing);
    }

    #[test]
    fn chase_and_scatter_alternate_and_a_zero_scatter_never_scatters() {
        let mut game = Game::new(level::garden(1));
        game.farmer.state = FarmerState::Chase { spell: 0 };
        game.farmer.pause = u32::MAX / 2;
        let chase = game.garden.farmer.chase;
        let (edge, s) = (game.farmer.edge, game.farmer.s);
        place(&mut game, edge, s, true, 0);
        game.bunny.mv = Move::Wait;
        game.bunny.grace = u32::MAX / 2;
        for _ in 0..chase {
            game.step(&[]);
        }
        assert!(matches!(game.farmer.state, FarmerState::Scatter { .. }));
        let mut game2 = game.clone();
        game2.garden.farmer.scatter = 0;
        game2.farmer.state = FarmerState::Chase { spell: 0 };
        for _ in 0..chase * 2 {
            game2.step(&[]);
        }
        assert!(matches!(game2.farmer.state, FarmerState::Chase { .. }));
    }

    #[test]
    fn hedges_block_the_farmers_sight_and_a_sun_hat_halves_it() {
        let mut game = Game::new(level::garden(1));
        game.farmer.state = FarmerState::Patrol { next: 0 };
        let (fx, fz) = game.farmer_point();
        let spacing = game.garden.edges[0].len;
        let edge = game
            .garden
            .edges
            .iter()
            .position(|e| {
                let a = game.garden.nodes[e.a];
                (a.x - fx).abs() == spacing && (a.z - fz).abs() == spacing
            })
            .unwrap();
        place(&mut game, edge, spacing / 2, true, 0);
        let (bx, bz) = game.bunny_point();
        assert!(bx != fx && bz != fz);
        grow(&mut game, 4);
        assert!(!game.farmer_sees());
        // Down a straight corridor 15 m away a Jack is seen, but not under a
        // sun hat.
        let mut game = face_off(150, false);
        grow(&mut game, 2);
        assert!(game.farmer_sees());
        game.power = Some((PowerKind::SunHat, 100));
        assert!(!game.farmer_sees());
    }

    #[test]
    fn leaving_ends_the_run_as_neither_win_nor_loss() {
        let mut game = Game::new(level::garden(1));
        run(&mut game, 30);
        game.step(&[Input::Leave]);
        assert_eq!(game.status, Status::Left);
        let tick = game.tick;
        game.step(&[]);
        assert_eq!(game.tick, tick);
    }

    #[test]
    fn gentle_mode_slows_the_farmer_and_lengthens_the_wind_up() {
        let game = Game::with_seed(level::garden(1), 0, true);
        assert_eq!(game.windup(), GENTLE_WINDUP);
        assert!(game.farmer_speed(100) < 100);
    }

    #[test]
    fn the_same_inputs_give_the_same_run() {
        let script = |tick: u32| match tick % 97 {
            10 => vec![Input::Left],
            30 => vec![Input::Jump],
            40 => vec![Input::Right],
            55 => vec![Input::Duck],
            70 if tick.is_multiple_of(3) => vec![Input::Back],
            _ => vec![],
        };
        for seed in [0, 7] {
            let mut one = Game::with_seed(level::garden(2), seed, false);
            let mut two = Game::with_seed(level::garden(2), seed, false);
            for tick in 0..3_000 {
                one.step(&script(tick));
                two.step(&script(tick));
                assert_eq!(one.state_digest(), two.state_digest(), "tick {tick}");
            }
        }
    }
}
