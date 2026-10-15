//! A job being played: building from the tray, Go, the run, and the
//! verdict.

use sha2::{Digest, Sha256};

use crate::build::Build;
use crate::kinds::PieceKind;
use crate::level::{Job, Level};
use crate::sim::{Happened, SETTLE, World};
use crate::{TURNS, V, metres, ticks};

/// What the player does. Positions are whole millimetres, so a run
/// records and replays exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Drop a piece from the tray held at `(x, y)`, turned `turn` steps:
    /// a tile snaps onto the build, a strut braces a square, a bolt locks
    /// a seam.
    Place {
        kind: PieceKind,
        x: i32,
        y: i32,
        turn: u8,
    },
    /// Take the piece at `(x, y)` back to the tray.
    Remove { x: i32, y: i32 },
    /// Run the build.
    Go,
    /// Back to building with the same build: after a run, or to stop one.
    Fix,
}

/// Why a run didn't do the job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    /// The two pieces the job joins came apart.
    CameApart,
    /// The crate didn't reach the beacon.
    Missed,
    /// The crate was spinning.
    Spun,
    /// A pod came off.
    PodLost,
    /// The pods felt too little gravity.
    TooLight,
    /// The pods felt too much.
    TooHeavy,
    /// The ship ran into the build: no room for it.
    NoRoom,
    /// The ship found nothing to tie up to.
    NoMooring,
    /// Something in the dock let go or popped.
    Broke,
}

/// Where the game is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Building,
    Running,
    Won,
    Failed(Fail),
}

/// What happened in a tick, for sounds and effects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A tile snapped on: its index.
    Snapped(usize),
    /// A strut braced this square.
    Braced(usize),
    /// A bolt locked this seam.
    Bolted(usize),
    /// A piece was dropped where it couldn't go.
    NoSnap,
    /// A piece went back to the tray.
    Removed(PieceKind),
    Go,
    /// The storm came by itself.
    StormCame,
    Broke(V),
    Popped(usize),
    Bumped(V),
    Moored(bool),
    Won,
    Failed(Fail),
}

/// A job in play.
#[derive(Clone, Debug)]
pub struct Game {
    pub level: Level,
    pub build: Build,
    /// What is left in the tray.
    pub tray: Vec<(PieceKind, u32)>,
    pub status: Status,
    /// Ticks since the job began.
    pub tick: u32,
    /// Building ticks toward the storm.
    pub clock: u32,
    pub world: Option<World>,
    /// This tick's events.
    pub events: Vec<Event>,
    /// Runs so far.
    pub runs: u32,
    /// Ticks the pods have been in the band, in a row.
    pub in_band: u32,
    /// Stars for the win, 1 to 3.
    pub stars: u8,
}

impl Game {
    #[must_use]
    pub fn new(level: Level) -> Self {
        let build = level.start();
        let tray = level.tray.clone();
        Self {
            level,
            build,
            tray,
            status: Status::Building,
            tick: 0,
            clock: 0,
            world: None,
            events: Vec::new(),
            runs: 0,
            in_band: 0,
            stars: 0,
        }
    }

    /// How many of `kind` are left in the tray.
    #[must_use]
    pub fn left(&self, kind: PieceKind) -> u32 {
        self.tray
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(0, |(_, n)| *n)
    }

    fn take(&mut self, kind: PieceKind) -> bool {
        match self.tray.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        }
    }

    fn give_back(&mut self, kind: PieceKind, count: u32) {
        if count == 0 {
            return;
        }
        match self.tray.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += count,
            None => self.tray.push((kind, count)),
        }
    }

    /// The pieces the job tagged, by tag.
    #[must_use]
    pub fn tags(&self) -> Vec<(char, usize)> {
        self.build
            .pieces
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.tag.map(|t| (t, i)))
            .collect()
    }

    /// Seconds left before the storm comes, if the job has one and the
    /// player is building.
    #[must_use]
    pub fn storm_left(&self) -> Option<u32> {
        let storm = self.level.storm?;
        (self.status == Status::Building).then(|| storm.clock.saturating_sub(self.clock))
    }

    /// Advances one tick with the player's inputs.
    pub fn step(&mut self, inputs: &[Input]) {
        self.events.clear();
        self.tick += 1;
        for input in inputs {
            self.apply(*input);
        }
        match self.status {
            Status::Building => {
                if let Some(storm) = self.level.storm {
                    self.clock += 1;
                    if self.clock >= storm.clock {
                        self.events.push(Event::StormCame);
                        self.go();
                    }
                }
            }
            Status::Running => self.run(),
            Status::Won | Status::Failed(_) => {}
        }
    }

    fn apply(&mut self, input: Input) {
        match (input, self.status) {
            (Input::Place { kind, x, y, turn }, Status::Building) => {
                self.place(kind, V::new(metres(x), metres(y)), turn % TURNS);
            }
            (Input::Remove { x, y }, Status::Building) => {
                if let Some(index) = self.build.piece_at(V::new(metres(x), metres(y))) {
                    let back = self.build.remove(index);
                    self.give_back(back.kind, 1);
                    self.give_back(PieceKind::Strut, back.struts);
                    self.give_back(PieceKind::Bolt, back.bolts);
                    self.events.push(Event::Removed(back.kind));
                }
            }
            (Input::Go, Status::Building) => self.go(),
            (Input::Fix, Status::Running | Status::Won | Status::Failed(_)) => {
                self.status = Status::Building;
                self.world = None;
                self.clock = 0;
                self.in_band = 0;
            }
            _ => {}
        }
    }

    fn place(&mut self, kind: PieceKind, at: V, turn: u8) {
        if self.left(kind) == 0 {
            self.events.push(Event::NoSnap);
            return;
        }
        match kind {
            PieceKind::Strut => match self.build.strut_target(at, turn) {
                Some((square, diagonal)) => {
                    self.take(kind);
                    self.build.pieces[square].strut = Some(diagonal);
                    self.events.push(Event::Braced(square));
                }
                None => self.events.push(Event::NoSnap),
            },
            PieceKind::Bolt => match self.build.bolt_target(at) {
                Some(seam) => {
                    self.take(kind);
                    self.build.seams[seam].bolted = true;
                    self.events.push(Event::Bolted(seam));
                }
                None => self.events.push(Event::NoSnap),
            },
            _ => match self.build.candidate(kind, at, turn) {
                Some(corners) => {
                    self.take(kind);
                    let index = self.build.place(kind, corners, false, None);
                    self.events.push(Event::Snapped(index));
                }
                None => self.events.push(Event::NoSnap),
            },
        }
    }

    fn go(&mut self) {
        self.world = Some(World::new(&self.build, &self.level));
        self.status = Status::Running;
        self.in_band = 0;
        self.runs += 1;
        self.events.push(Event::Go);
    }

    fn run(&mut self) {
        let tags = self.tags();
        let Some(world) = self.world.as_mut() else {
            return;
        };
        for happened in world.step(&self.level, &tags) {
            self.events.push(match happened {
                Happened::Broke(at) => Event::Broke(at),
                Happened::Popped(piece) => Event::Popped(piece),
                Happened::Bumped(at) => Event::Bumped(at),
                Happened::Moored(ok) => Event::Moored(ok),
            });
        }
        if let Some(verdict) = self.judge() {
            self.status = verdict;
            match verdict {
                Status::Won => {
                    self.stars = self.stars_for(self.build.used());
                    self.events.push(Event::Won);
                }
                Status::Failed(why) => self.events.push(Event::Failed(why)),
                _ => {}
            }
        }
    }

    /// Stars for a win that used `used` pieces.
    #[must_use]
    pub fn stars_for(&self, used: u32) -> u8 {
        if used <= self.level.par {
            3
        } else if used <= self.level.par + 3 {
            2
        } else {
            1
        }
    }

    fn tagged(&self, tag: char) -> usize {
        self.build.tagged(tag).expect("the level checks its tags")
    }

    /// The verdict, once the run has one.
    fn judge(&mut self) -> Option<Status> {
        let world = self.world.as_ref()?;
        let over = world.tick >= self.level.run;
        match self.level.job {
            Job::Connect(a, b) => {
                let (a, b) = (self.tagged(a), self.tagged(b));
                if !over {
                    return None;
                }
                let groups = world.groups();
                Some(if groups[a] == groups[b] {
                    Status::Won
                } else {
                    Status::Failed(Fail::CameApart)
                })
            }
            Job::Push(tag) => {
                let crate_ = self.tagged(tag);
                let group = world.group_of(crate_);
                let (_, _, omega) = world.motion(&group);
                let (beacon, radius) = self.level.beacon.expect("the level checks it");
                if (world.centre(crate_) - beacon).len() <= radius {
                    return Some(if omega.abs() <= self.level.spin_max {
                        Status::Won
                    } else {
                        Status::Failed(Fail::Spun)
                    });
                }
                over.then_some(Status::Failed(if omega.abs() > self.level.spin_max {
                    Fail::Spun
                } else {
                    Fail::Missed
                }))
            }
            Job::Spin(tag) => {
                let hub = self.tagged(tag);
                let group = world.group_of(hub);
                let pods: Vec<usize> = (0..self.build.pieces.len())
                    .filter(|i| self.build.pieces[*i].kind == PieceKind::Pod)
                    .collect();
                let lost = pods.iter().any(|p| !group.contains(p));
                let (low, high) = self.level.gravity;
                let felt: Vec<f64> = pods
                    .iter()
                    .map(|p| world.spin_gravity(&group, *p))
                    .collect();
                let in_band = !lost && felt.iter().all(|g| *g >= low && *g <= high);
                if world.tick > self.level.burn && in_band {
                    self.in_band += 1;
                } else {
                    self.in_band = 0;
                }
                if self.in_band >= self.level.hold {
                    return Some(Status::Won);
                }
                if lost && world.tick > self.level.burn {
                    return Some(Status::Failed(Fail::PodLost));
                }
                over.then(|| {
                    let mean = felt.iter().sum::<f64>() / felt.len().max(1) as f64;
                    Status::Failed(if lost {
                        Fail::PodLost
                    } else if mean < low {
                        Fail::TooLight
                    } else {
                        Fail::TooHeavy
                    })
                })
            }
            Job::Dock(tag) => {
                // A beat after the bump, so the bump is seen.
                if let Some(at) = world.blocked_at {
                    return (world.tick >= at + ticks(1.0) || over)
                        .then_some(Status::Failed(Fail::NoRoom));
                }
                let parked = world.parked_at?;
                let ship = world.ship?;
                if ship.moorings.is_none() {
                    return Some(Status::Failed(Fail::NoMooring));
                }
                let done = parked + self.level.moor.1 + ticks(SETTLE);
                if world.tick < done && !over {
                    return None;
                }
                let base = self.tagged(tag);
                let original: Vec<usize> = {
                    let groups = self.build.groups();
                    (0..groups.len())
                        .filter(|i| groups[*i] == groups[base])
                        .collect()
                };
                let group = world.group_of(base);
                let moored = ship.moorings.is_some_and(|nodes| {
                    nodes
                        .iter()
                        .all(|node| group.iter().any(|piece| world.nodes(*piece).contains(node)))
                });
                Some(if world.damaged(&original) || !moored {
                    Status::Failed(Fail::Broke)
                } else {
                    Status::Won
                })
            }
        }
    }

    /// SHA-256 of the whole state, for receipts and replays.
    #[must_use]
    pub fn state_digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.tick.to_le_bytes());
        hash.update(self.clock.to_le_bytes());
        hash.update(format!("{:?} {}", self.status, self.runs));
        for piece in &self.build.pieces {
            hash.update(piece.kind.word());
            for p in &piece.at {
                hash.update(p.x.to_bits().to_le_bytes());
                hash.update(p.y.to_bits().to_le_bytes());
            }
            hash.update([piece.strut.map_or(9, |s| s)]);
        }
        for seam in &self.build.seams {
            hash.update(format!(
                "{} {} {} {} {}",
                seam.a, seam.a_side, seam.b, seam.b_side, seam.bolted
            ));
        }
        for (kind, n) in &self.tray {
            hash.update(format!("{} {n}", kind.word()));
        }
        if let Some(world) = &self.world {
            hash.update(world.digest());
        }
        hash.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level;
    use crate::mm;

    fn place(game: &mut Game, kind: PieceKind, x: f64, y: f64, turn: u8) {
        game.step(&[Input::Place {
            kind,
            x: mm(x),
            y: mm(y),
            turn,
        }]);
    }

    fn finish(game: &mut Game) {
        game.step(&[Input::Go]);
        for _ in 0..game.level.run + 10 {
            if game.status != Status::Running {
                break;
            }
            game.step(&[]);
        }
    }

    #[test]
    fn job_one_is_won_by_a_row_of_squares_and_lost_without_one() {
        let mut game = Game::new(level::job(1));
        finish(&mut game);
        assert_eq!(game.status, Status::Failed(Fail::CameApart));
        game.step(&[Input::Fix]);
        assert_eq!(game.status, Status::Building);
        for x in [1.5, 2.5, 3.5] {
            place(&mut game, PieceKind::Square, x, 0.5, 0);
        }
        assert_eq!(
            game.left(PieceKind::Square),
            game.level.tray_count(PieceKind::Square) - 3
        );
        finish(&mut game);
        assert_eq!(game.status, Status::Won, "{:?}", game.events);
        assert!(game.stars >= 1);
    }

    #[test]
    fn the_tray_runs_out_and_taking_a_piece_off_gives_it_back() {
        let mut game = Game::new(level::job(1));
        let squares = game.left(PieceKind::Square);
        for i in 0..=squares {
            place(&mut game, PieceKind::Square, 1.5 + f64::from(i), 0.5, 0);
        }
        assert_eq!(game.left(PieceKind::Square), 0);
        assert!(game.events.contains(&Event::NoSnap) || squares >= 4);
        game.step(&[Input::Remove { x: 1500, y: 500 }]);
        assert_eq!(game.left(PieceKind::Square), 1);
        assert_eq!(game.events, vec![Event::Removed(PieceKind::Square)]);
        // Given pieces stay.
        game.step(&[Input::Remove { x: 500, y: 500 }]);
        assert!(game.events.is_empty());
    }

    #[test]
    fn the_storm_comes_by_itself_when_its_clock_runs_out() {
        let mut game = Game::new(level::job(3));
        let clock = game.level.storm.expect("job 3 has a storm").clock;
        for _ in 0..clock - 1 {
            game.step(&[]);
        }
        assert_eq!(game.status, Status::Building);
        game.step(&[]);
        assert_eq!(game.status, Status::Running);
        assert!(game.events.contains(&Event::StormCame));
    }
}
