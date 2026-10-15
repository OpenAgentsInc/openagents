//! Snap Station on the community-game contract (`verse-game`): the generic
//! inputs it takes, the HUD it asks for, and its replays.
//!
//! The generic channel has no pointer, so a host without one (a gamepad, a
//! keyboard, a Verse host) plays with a cursor: while building, Left and
//! Right pick a slot in the tray (the last one is Go), Use picks the piece
//! up, the four directions move it half a metre at a time, Use again snaps
//! it on (turned whichever way fits best there), and Back puts it down
//! again, or with nothing held takes the last piece off. After a run, Use
//! or Back goes back to building. The page uses the same cursor for its
//! keyboard, and its pointer sends the rules' own inputs.

use snap_rules::game::{Event, Game, Input, Status};
use snap_rules::level::{Job, Level};
use snap_rules::{PieceKind, TURNS, V, mm, receipt};
use verse_game::{CommunityGame, GameInput, Hud, HudElement, Icon, MapDot, Outcome};

/// The game's id.
pub const ID: &str = "snap-station";

/// How far the cursor moves a press, in metres.
pub const STEP: f64 = 0.5;

/// A slot in the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Piece(PieceKind),
    Go,
}

/// The colours the HUD's map uses.
pub const MAP_BUILD: u32 = 0x3BA7F5;
pub const MAP_BEACON: u32 = 0x7CF2C8;
pub const MAP_SHIP: u32 = 0xDDE3EA;
pub const MAP_CURSOR: u32 = 0xFFFFFF;

/// A job in play, as a host sees it.
#[derive(Clone, Debug)]
pub struct SnapGame {
    pub game: Game,
    pub cursor: V,
    /// The slot picked in the tray, an index into [`SnapGame::slots`].
    pub slot: usize,
    pub carrying: Option<PieceKind>,
}

impl SnapGame {
    #[must_use]
    pub fn new(level: Level) -> Self {
        let game = Game::new(level);
        let cursor = start_cursor(&game);
        Self {
            game,
            cursor,
            slot: 0,
            carrying: None,
        }
    }

    /// The tray's slots: every piece with some left, then Go.
    #[must_use]
    pub fn slots(&self) -> Vec<Slot> {
        let mut slots: Vec<Slot> = self
            .game
            .tray
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(k, _)| Slot::Piece(*k))
            .collect();
        slots.push(Slot::Go);
        slots
    }

    /// The turn that snaps `kind` nearest the cursor, if any does.
    #[must_use]
    pub fn best_turn(&self, kind: PieceKind) -> u8 {
        let build = &self.game.build;
        let mut best: Option<(f64, u8)> = None;
        for turn in 0..TURNS {
            let Some(corners) = build.candidate(kind, self.cursor, turn) else {
                continue;
            };
            let mut centre = V::ZERO;
            for p in &corners {
                centre += *p;
            }
            let gap = (centre * (1.0 / corners.len() as f64) - self.cursor).len2();
            if best.is_none_or(|(old, _)| gap < old) {
                best = Some((gap, turn));
            }
        }
        best.map_or(0, |(_, turn)| turn)
    }

    /// The rules' inputs for one generic input, moving the cursor and the
    /// tray slot as it goes.
    pub fn convert(&mut self, input: GameInput) -> Vec<Input> {
        match self.game.status {
            Status::Building => {}
            Status::Running => return Vec::new(),
            Status::Won | Status::Failed(_) => {
                return match input {
                    GameInput::Use | GameInput::Back => vec![Input::Fix],
                    _ => Vec::new(),
                };
            }
        }
        if let Some(kind) = self.carrying {
            match input {
                GameInput::Left => self.cursor.x -= STEP,
                GameInput::Right => self.cursor.x += STEP,
                GameInput::Up => self.cursor.y += STEP,
                GameInput::Down => self.cursor.y -= STEP,
                GameInput::Back => self.carrying = None,
                GameInput::Use => {
                    return vec![Input::Place {
                        kind,
                        x: mm(self.cursor.x),
                        y: mm(self.cursor.y),
                        turn: self.best_turn(kind),
                    }];
                }
                GameInput::Pause | GameInput::UpReleased => {}
            }
            let (low, high) = self.game.level.area;
            self.cursor.x = self.cursor.x.clamp(low.x, high.x);
            self.cursor.y = self.cursor.y.clamp(low.y, high.y);
            return Vec::new();
        }
        let slots = self.slots();
        match input {
            GameInput::Left => self.slot = (self.slot + slots.len() - 1) % slots.len(),
            GameInput::Right => self.slot = (self.slot + 1) % slots.len(),
            GameInput::Use => match slots[self.slot.min(slots.len() - 1)] {
                Slot::Go => return vec![Input::Go],
                Slot::Piece(kind) => self.carrying = Some(kind),
            },
            GameInput::Back => {
                let build = &self.game.build;
                if let Some(last) = (0..build.pieces.len())
                    .rev()
                    .find(|i| !build.pieces[*i].given)
                {
                    let at = build.pieces[last].centre();
                    return vec![Input::Remove {
                        x: mm(at.x),
                        y: mm(at.y),
                    }];
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// Steps the rules with inputs already in their own terms (from a
    /// pointer), keeping the cursor state in step.
    pub fn step_rules(&mut self, inputs: &[Input]) {
        self.game.step(inputs);
        self.after_step();
    }

    fn after_step(&mut self) {
        let placed = self
            .game
            .events
            .iter()
            .any(|e| matches!(e, Event::Snapped(_) | Event::Braced(_) | Event::Bolted(_)));
        if placed {
            self.carrying = None;
        }
        if let Some(kind) = self.carrying
            && self.game.left(kind) == 0
        {
            self.carrying = None;
        }
        let slots = self.slots().len();
        if self.slot >= slots {
            self.slot = slots - 1;
        }
    }

    /// The pieces' sides and the job's places on a square map.
    fn map(&self) -> HudElement {
        let (low, high) = self.game.level.area;
        let side = (high.x - low.x).max(high.y - low.y).max(1.0);
        let at = |p: V| {
            (
                ((p.x - low.x) / side) as f32,
                (1.0 - (p.y - low.y) / side) as f32,
            )
        };
        let mut lines = Vec::new();
        for piece in &self.game.build.pieces {
            let n = piece.at.len();
            for i in 0..n {
                let (ax, ay) = at(piece.at[i]);
                let (bx, by) = at(piece.at[(i + 1) % n]);
                lines.push([ax, ay, bx, by]);
            }
        }
        let mut dots = Vec::new();
        if let Some((beacon, _)) = self.game.level.beacon {
            let (x, y) = at(beacon);
            dots.push(MapDot {
                x,
                y,
                colour: MAP_BEACON,
                size: 0.04,
            });
        }
        if let Some(ship) = self.game.level.ship {
            let (x, y) = at(V::new(ship.berth + ship.length / 2.0, ship.y));
            dots.push(MapDot {
                x,
                y,
                colour: MAP_SHIP,
                size: 0.05,
            });
        }
        if self.carrying.is_some() {
            let (x, y) = at(self.cursor);
            dots.push(MapDot {
                x,
                y,
                colour: MAP_CURSOR,
                size: 0.02,
            });
        }
        for dot in &mut dots {
            dot.x = dot.x.clamp(0.0, 1.0);
            dot.y = dot.y.clamp(0.0, 1.0);
        }
        HudElement::Map { lines, dots }
    }

    /// How strong the plant pods' gravity is now, from 0 to 5 pips (the
    /// band's top is four), and how far through the hold they are.
    #[must_use]
    pub fn gravity(&self) -> Option<(f64, f32)> {
        let Job::Spin(tag) = self.game.level.job else {
            return None;
        };
        let world = self.game.world.as_ref()?;
        let hub = self.game.build.tagged(tag)?;
        let group = world.group_of(hub);
        let pods: Vec<usize> = (0..self.game.build.pieces.len())
            .filter(|i| self.game.build.pieces[*i].kind == PieceKind::Pod)
            .collect();
        let felt = pods
            .iter()
            .map(|p| world.spin_gravity(&group, *p))
            .fold(f64::MAX, f64::min);
        let hold = self.game.in_band as f32 / self.game.level.hold.max(1) as f32;
        Some((if pods.is_empty() { 0.0 } else { felt }, hold.min(1.0)))
    }
}

/// Where the cursor starts: on the job's first piece, a step to its right.
fn start_cursor(game: &Game) -> V {
    game.build
        .pieces
        .first()
        .map_or(V::ZERO, |p| p.centre() + V::new(1.0, 0.0))
}

impl CommunityGame for SnapGame {
    fn id(&self) -> &'static str {
        ID
    }

    fn hz(&self) -> u32 {
        snap_rules::HZ
    }

    fn step(&mut self, inputs: &[GameInput]) {
        let mut rules = Vec::new();
        for input in inputs {
            rules.extend(self.convert(*input));
        }
        self.step_rules(&rules);
    }

    fn tick(&self) -> u32 {
        self.game.tick
    }

    fn outcome(&self) -> Option<Outcome> {
        match self.game.status {
            Status::Building | Status::Running => None,
            Status::Won => Some(Outcome::Won),
            Status::Failed(_) => Some(Outcome::Lost),
        }
    }

    fn hud(&self) -> Hud {
        let mut hud = vec![HudElement::Counter {
            icon: Icon::Score,
            value: self.game.build.used(),
            ring: None,
        }];
        if let (Some(storm), Some(left)) = (self.game.level.storm, self.game.storm_left()) {
            hud.push(HudElement::Ring {
                icon: Icon::Clock,
                left: left as f32 / storm.clock.max(1) as f32,
            });
        }
        if let Some((felt, hold)) = self.gravity() {
            let top = self.game.level.gravity.1;
            let pips = (felt / top * 4.0).clamp(0.0, 5.0);
            hud.push(HudElement::Pips {
                icon: Icon::Power(1),
                filled: pips as u8,
                count: 5,
                bar: hold,
                label: "Gravity",
            });
        }
        hud.push(self.map());
        hud
    }

    fn state_digest(&self) -> [u8; 32] {
        self.game.state_digest()
    }

    fn rules_digest(&self) -> [u8; 32] {
        receipt::rules_digest()
    }

    fn level_digest(&self) -> [u8; 32] {
        self.game.level.digest
    }

    fn seed(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snap_rules::level;
    use verse_game::Replay;

    fn press(game: &mut SnapGame, replay: &mut Replay, inputs: &[GameInput]) {
        for input in inputs {
            replay.step(game, &[*input]);
        }
    }

    #[test]
    fn job_one_is_done_with_the_cursor_and_its_replay_verifies() {
        use GameInput::{Left, Right, Use};
        let mut game = SnapGame::new(level::job(1));
        let mut replay = Replay::start(&game);
        assert_eq!(game.slots()[0], Slot::Piece(PieceKind::Square));
        // Three squares: pick one up, snap it on, step right, again.
        press(&mut game, &mut replay, &[Use, Use]);
        press(&mut game, &mut replay, &[Use, Right, Right, Use]);
        press(&mut game, &mut replay, &[Use, Right, Right, Use]);
        assert_eq!(
            game.game.build.pieces.len(),
            5,
            "{:?}",
            game.game.build.pieces.len()
        );
        // Go is the last slot: one to the left of the first.
        press(&mut game, &mut replay, &[Left, Use]);
        assert_eq!(game.game.status, Status::Running);
        while game.outcome().is_none() {
            replay.step(&mut game, &[]);
        }
        assert_eq!(game.outcome(), Some(Outcome::Won));
        let text = replay.encode();
        let read = Replay::decode(&text).unwrap();
        read.verify(&mut SnapGame::new(level::job(1))).unwrap();
        assert!(read.verify(&mut SnapGame::new(level::job(2))).is_err());
    }

    #[test]
    fn back_takes_the_last_piece_off_and_use_after_a_run_fixes_it() {
        let mut game = SnapGame::new(level::job(1));
        CommunityGame::step(&mut game, &[GameInput::Use]);
        CommunityGame::step(&mut game, &[GameInput::Use]);
        assert_eq!(game.game.build.pieces.len(), 3);
        CommunityGame::step(&mut game, &[GameInput::Back]);
        assert_eq!(game.game.build.pieces.len(), 2);
        let go = game.slots().len() - 1;
        game.slot = go;
        CommunityGame::step(&mut game, &[GameInput::Use]);
        while game.outcome().is_none() {
            CommunityGame::step(&mut game, &[]);
        }
        assert_eq!(game.outcome(), Some(Outcome::Lost));
        CommunityGame::step(&mut game, &[GameInput::Use]);
        assert_eq!(game.game.status, Status::Building);
    }

    #[test]
    fn the_hud_counts_pieces_and_shows_the_storm_and_the_map() {
        let game = SnapGame::new(level::job(3));
        let hud = game.hud();
        assert!(matches!(
            hud[0],
            HudElement::Counter {
                icon: Icon::Score,
                value: 0,
                ..
            }
        ));
        assert!(matches!(hud[1], HudElement::Ring { icon: Icon::Clock, left } if left > 0.99));
        let HudElement::Map { lines, dots } = hud.last().unwrap() else {
            panic!("a map last");
        };
        assert_eq!(lines.len(), 4 + 4 + 6 + 6);
        assert!(dots.is_empty());
        for line in lines {
            assert!(line.iter().all(|v| (-0.01..=1.01).contains(v)), "{line:?}");
        }
    }
}
