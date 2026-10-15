//! A player that solves each job: it builds with the same inputs a person
//! uses (a piece held at a point, turned, and let go), braces and bolts
//! what it built, adds thrusters where a job needs them, and tries its
//! build by running it, as a player would, before it keeps it. Tests use
//! it to show every job can be done with the tray it gives.

use crate::game::{Game, Input, Status};
use crate::kinds::PieceKind;
use crate::level::{Job, Level};
use crate::{V, mm, nearest_turn};

/// A bot playing one job: the plan it found, given out one input a tick.
#[derive(Clone, Debug)]
pub struct Bot {
    plan: Vec<Input>,
    next: usize,
}

impl Bot {
    /// The bot for `level`, if it found a way to do the job.
    #[must_use]
    pub fn new(level: &Level) -> Option<Self> {
        solve(level).map(|plan| Self { plan, next: 0 })
    }

    /// The bot's input for this tick, if any: one step of the plan while
    /// building, then nothing while the run plays.
    pub fn input(&mut self, game: &Game) -> Option<Input> {
        if game.status != Status::Building {
            return None;
        }
        let input = self.plan.get(self.next).copied();
        self.next += 1;
        input.or_else(|| (game.phase > 0).then_some(Input::Go))
    }

    /// The whole plan, ending with Go.
    #[must_use]
    pub fn plan(&self) -> &[Input] {
        &self.plan
    }
}

/// A build in progress: the game and the inputs that made it.
#[derive(Clone)]
struct Plan {
    game: Game,
    inputs: Vec<Input>,
}

impl Plan {
    fn new(level: &Level) -> Self {
        Self {
            game: Game::new(level.clone()),
            inputs: Vec::new(),
        }
    }

    fn input(&mut self, input: Input) {
        self.inputs.push(input);
        self.game.step(&[input]);
    }

    fn left(&self, kind: PieceKind) -> u32 {
        self.game.left(kind)
    }

    /// Holds `kind` where it would sit at `corners` and lets go. Returns
    /// the new piece if it snapped there.
    fn put(&mut self, kind: PieceKind, corners: &[V]) -> Option<usize> {
        if self.left(kind) == 0 {
            return None;
        }
        let shape = kind.shape();
        let mut centre = V::ZERO;
        for p in corners {
            centre += *p;
        }
        let centre = centre * (1.0 / corners.len() as f64);
        let local = (shape[1] - shape[0]).unit();
        let world = (corners[1] - corners[0]).unit();
        let turn = nearest_turn(local.dot(world), local.cross(world));
        let before = self.game.build.pieces.len();
        self.input(Input::Place {
            kind,
            x: mm(centre.x),
            y: mm(centre.y),
            turn,
        });
        let after = self.game.build.pieces.len();
        (after > before).then(|| after - 1)
    }

    /// A square cell onto a free side: a square with a strut, or two half
    /// squares when the struts have run out.
    fn braced_cell(&mut self, free: (usize, usize)) -> bool {
        let build = &self.game.build;
        let cell = build.onto(PieceKind::Square, 0, free);
        if !build.fits(&cell) {
            return false;
        }
        if self.left(PieceKind::Square) > 0 && self.left(PieceKind::Strut) > 0 {
            let Some(square) = self.put(PieceKind::Square, &cell) else {
                return false;
            };
            let centre = self.game.build.pieces[square].centre();
            let d = (self.game.build.pieces[square].at[2] - self.game.build.pieces[square].at[0])
                .unit();
            self.input(Input::Place {
                kind: PieceKind::Strut,
                x: mm(centre.x),
                y: mm(centre.y),
                turn: nearest_turn(d.x, d.y),
            });
            return true;
        }
        if self.left(PieceKind::Wedge) >= 2 {
            let first = self.game.build.onto(PieceKind::Wedge, 0, free);
            let Some(wedge) = self.put(PieceKind::Wedge, &first) else {
                return false;
            };
            let piece = &self.game.build.pieces[wedge];
            let mut middle = V::ZERO;
            for p in &cell {
                middle += *p;
            }
            let middle = middle * 0.25;
            let other = middle * 2.0 - piece.centre();
            // The second half square, turned half way round, onto the
            // first one's long side.
            let local = (PieceKind::Wedge.shape()[1] - PieceKind::Wedge.shape()[0]).unit();
            let world = (piece.at[1] - piece.at[0]).unit();
            let turn = (nearest_turn(local.dot(world), local.cross(world)) + 6) % crate::TURNS;
            let before = self.game.build.pieces.len();
            self.input(Input::Place {
                kind: PieceKind::Wedge,
                x: mm(other.x),
                y: mm(other.y),
                turn,
            });
            return self.game.build.pieces.len() > before;
        }
        if self.left(PieceKind::Square) > 0 {
            return self.put(PieceKind::Square, &cell).is_some();
        }
        false
    }

    /// The free sides of `piece`'s group, now.
    fn group_sides(&self, piece: usize) -> Vec<(usize, usize)> {
        let groups = self.game.build.groups();
        self.game
            .build
            .free_sides()
            .into_iter()
            .filter(|(p, _)| groups[*p] == groups[piece])
            .collect()
    }

    /// Grows braced cells from `piece`'s group toward `target`, keeping
    /// out of `avoid`, until `done` says so. Returns whether it got there.
    fn grow(
        &mut self,
        piece: usize,
        target: V,
        avoid: Option<(V, V)>,
        done: &dyn Fn(&Game) -> bool,
    ) -> bool {
        for _ in 0..16 {
            if done(&self.game) {
                return true;
            }
            let mut best: Option<(f64, (usize, usize))> = None;
            for free in self.group_sides(piece) {
                let build = &self.game.build;
                if build.pieces[free.0].kind == PieceKind::Thruster {
                    continue;
                }
                let cell = build.onto(PieceKind::Square, 0, free);
                if !build.fits(&cell) || avoid.is_some_and(|rect| touches(&cell, rect)) {
                    continue;
                }
                let mut centre = V::ZERO;
                for p in &cell {
                    centre += *p;
                }
                let score = (centre * 0.25 - target).len2();
                if best.is_none_or(|(old, _)| score < old - 1e-9) {
                    best = Some((score, free));
                }
            }
            let Some((_, free)) = best else {
                return false;
            };
            if !self.braced_cell(free) {
                return false;
            }
        }
        done(&self.game)
    }

    /// Bolts seams, first made first, while bolts last.
    fn bolt_all(&mut self) {
        let mut index = 0;
        while self.left(PieceKind::Bolt) > 0 && index < self.game.build.seams.len() {
            let seam = self.game.build.seams[index];
            if !seam.bolted {
                let at = seam.middle(&self.game.build);
                self.input(Input::Place {
                    kind: PieceKind::Bolt,
                    x: mm(at.x),
                    y: mm(at.y),
                    turn: 0,
                });
            }
            index += 1;
        }
    }

    /// Free sides of `piece`'s group a thruster could go on, with the
    /// direction it would push and where.
    fn thruster_spots(&self, piece: usize) -> Vec<((usize, usize), V, V)> {
        let build = &self.game.build;
        self.group_sides(piece)
            .into_iter()
            .filter(|free| build.pieces[free.0].kind != PieceKind::Thruster)
            .filter_map(|free| {
                let corners = build.onto(PieceKind::Thruster, 0, free);
                if !build.fits(&corners) {
                    return None;
                }
                let base = corners[0].mid(corners[1]);
                Some((free, (base - corners[2]).unit(), base))
            })
            .collect()
    }

    fn thruster(&mut self, free: (usize, usize)) -> bool {
        let corners = self.game.build.onto(PieceKind::Thruster, 0, free);
        self.put(PieceKind::Thruster, &corners).is_some()
    }

    /// Runs a copy of the build and says whether it does the job.
    fn works(&self) -> bool {
        let mut game = self.game.clone();
        game.step(&[Input::Go]);
        for _ in 0..game.level.run + 2 {
            if game.status != Status::Running {
                break;
            }
            game.step(&[]);
        }
        game.status == Status::Won
    }

    fn finish(mut self) -> Vec<Input> {
        self.inputs.push(Input::Go);
        self.inputs
    }
}

/// Whether a piece at `corners` reaches into the box `rect`.
fn touches(corners: &[V], rect: (V, V)) -> bool {
    let (low, high) = rect;
    let mut min = V::new(f64::MAX, f64::MAX);
    let mut max = V::new(f64::MIN, f64::MIN);
    for p in corners {
        min = V::new(min.x.min(p.x), min.y.min(p.y));
        max = V::new(max.x.max(p.x), max.y.max(p.y));
    }
    min.x < high.x - 1e-6 && max.x > low.x + 1e-6 && min.y < high.y - 1e-6 && max.y > low.y + 1e-6
}

fn tagged(level: &Level, tag: char) -> usize {
    level
        .given
        .iter()
        .position(|g| g.tag == Some(tag))
        .expect("the level checks its tags")
}

/// A plan that does `level`'s job, ending with Go, if the bot finds one.
#[must_use]
pub fn solve(level: &Level) -> Option<Vec<Input>> {
    if !level.solution.is_empty() {
        let game = play_plan(level, &level.solution);
        return (game.status == Status::Won).then(|| level.solution.clone());
    }
    match level.job {
        Job::Connect(a, b) => connect(level, tagged(level, a), tagged(level, b)),
        Job::Push(c) => push(level, tagged(level, c)),
        Job::Spin(h) => spin(level, tagged(level, h)),
        Job::Dock(b) => dock(level, tagged(level, b)),
        Job::Stop(_) | Job::Swing(_) | Job::Tow(_) => None,
    }
}

/// Plays construction inputs and each test through the same public rules.
pub fn play_plan(level: &Level, plan: &[Input]) -> Game {
    record_plan(level, plan).0
}

/// The bot's run, including each Go between tests, as a checkable receipt.
pub fn record_plan(level: &Level, plan: &[Input]) -> (Game, crate::receipt::Receipt) {
    let mut game = Game::new(level.clone());
    let mut recorder = crate::receipt::Recorder::new();
    for input in plan {
        recorder.step(&mut game, &[*input]);
    }
    let budget = (level.run + 2) * level.tests.len() as u32 + 8;
    for _ in 0..budget {
        match game.status {
            Status::Running => recorder.step(&mut game, &[]),
            Status::Building if game.phase > 0 => recorder.step(&mut game, &[Input::Go]),
            _ => break,
        }
    }
    let receipt = recorder.finish(&game);
    (game, receipt)
}

fn connect(level: &Level, a: usize, b: usize) -> Option<Vec<Input>> {
    let mut plan = Plan::new(level);
    let target = plan.game.build.pieces[b].centre();
    let joined = move |game: &Game| {
        let groups = game.build.groups();
        groups[a] == groups[b]
    };
    if !plan.grow(a, target, None, &joined) {
        return None;
    }
    plan.bolt_all();
    plan.works().then(|| plan.finish())
}

fn push(level: &Level, c: usize) -> Option<Vec<Input>> {
    let (beacon, _) = level.beacon?;
    for count in 1..=level.tray_count(PieceKind::Thruster) {
        let mut plan = Plan::new(level);
        for _ in 0..count {
            let centre = plan.game.build.pieces[c].centre();
            let toward = (beacon - centre).unit();
            // Through the middle and toward the beacon: the push's line
            // passes nearest the centre.
            let spot = plan
                .thruster_spots(c)
                .into_iter()
                .filter(|(_, dir, _)| dir.dot(toward) > 0.9)
                .min_by(|x, y| {
                    let off = |s: &(_, V, V)| (s.2 - centre).cross(s.1).abs();
                    off(x).total_cmp(&off(y))
                });
            let Some((free, _, _)) = spot else {
                break;
            };
            if !plan.thruster(free) {
                break;
            }
        }
        if plan.works() {
            return Some(plan.finish());
        }
    }
    None
}

fn spin(level: &Level, hub: usize) -> Option<Vec<Input>> {
    let mut plan = Plan::new(level);
    let pods: Vec<usize> = (0..plan.game.build.pieces.len())
        .filter(|i| plan.game.build.pieces[*i].kind == PieceKind::Pod)
        .collect();
    for pod in &pods {
        let target = plan.game.build.pieces[*pod].centre();
        let pod = *pod;
        let joined = move |game: &Game| {
            let groups = game.build.groups();
            groups[hub] == groups[pod]
        };
        if !plan.grow(hub, target, None, &joined) {
            return None;
        }
    }
    plan.bolt_all();
    // Thrusters where they turn the build hardest, anticlockwise, added
    // until a run does the job.
    let centre = {
        let pieces: Vec<usize> = (0..plan.game.build.pieces.len()).collect();
        let world = crate::sim::World::new(&plan.game.build, level);
        world.motion(&pieces).0
    };
    let mut spots = plan.thruster_spots(hub);
    spots.retain(|(_, dir, at)| (*at - centre).cross(*dir) > 0.5);
    spots.sort_by(|x, y| {
        let turn = |s: &(_, V, V)| (s.2 - centre).cross(s.1);
        turn(y).total_cmp(&turn(x))
    });
    for (free, _, _) in spots {
        if plan.left(PieceKind::Thruster) == 0 {
            break;
        }
        if !plan.thruster(free) {
            continue;
        }
        if plan.works() {
            return Some(plan.finish());
        }
    }
    None
}

fn dock(level: &Level, base: usize) -> Option<Vec<Input>> {
    let ship = level.ship?;
    let mut plan = Plan::new(level);
    let lane = (
        V::new(-100.0, ship.y - ship.height / 2.0 - 0.05),
        V::new(100.0, ship.y + ship.height / 2.0 + 0.05),
    );
    let back = ship.berth + ship.length;
    let above = ship.y + ship.height / 2.0 + 0.6;
    let below = ship.y - ship.height / 2.0 - 0.6;
    let groups = plan.game.build.groups();
    let given: Vec<usize> = (0..plan.game.build.pieces.len())
        .filter(|i| groups[*i] == groups[base])
        .collect();
    // An arm above the ship and one below, reaching past its back.
    for y in [above, below] {
        let start = *given
            .iter()
            .min_by(|a, b| {
                let gap = |p: &usize| (plan.game.build.pieces[*p].centre().y - y).abs();
                gap(a).total_cmp(&gap(b))
            })
            .expect("a base");
        let tip = V::new(back - 0.25, y);
        let reached = move |game: &Game| {
            game.build
                .pieces
                .iter()
                .any(|p| !p.given && (p.centre() - tip).len() < 0.5)
        };
        if !plan.grow(start, tip, Some(lane), &reached) {
            return None;
        }
    }
    plan.bolt_all();
    plan.works().then(|| plan.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level;

    /// Plays `plan` the way the page does, one input a tick, to the end.
    fn play(level: &Level, plan: &[Input]) -> Game {
        play_plan(level, plan)
    }

    #[test]
    fn the_bot_does_every_job() {
        for number in 1..=level::COUNT {
            let level = level::job(number);
            let plan = solve(&level).unwrap_or_else(|| panic!("job {number}: no plan"));
            let game = play(&level, &plan);
            assert_eq!(game.status, Status::Won, "job {number}: {:?}", game.events);
            // Each job's par is the bot's own build, so it earns three stars.
            assert_eq!(game.stars, 3, "job {number}: {} pieces", game.build.used());
            eprintln!(
                "job {number} {}: {} inputs, {} pieces used, {} stars",
                level.name,
                plan.len(),
                game.build.used(),
                game.stars
            );
        }
    }

    #[test]
    fn the_bot_gives_its_plan_one_input_a_tick() {
        let level = level::job(1);
        let mut bot = Bot::new(&level).expect("a plan");
        let mut game = Game::new(level.clone());
        for _ in 0..level.run + bot.plan().len() as u32 + 5 {
            let input = bot.input(&game);
            game.step(&input.into_iter().collect::<Vec<_>>());
        }
        assert_eq!(game.status, Status::Won);
    }
}
