//! The contract a Verse community game builds to
//! (`docs/verse/games/grow-little-bunny.md`, Engine APIs the game needs).
//!
//! A community game is a fixed-step simulation a host drives: the host
//! reads the player's keys, swipes or gamepad, turns them into the generic
//! [`GameInput`] channel, steps the game, draws what the game's HUD
//! elements ([`HudElement`]) say, and keeps a [`Replay`] of the run. The
//! game never sees a key code, a touch, or a frame clock, so the same game
//! runs under any host: the browser, the desktop app, or a phone, and a host
//! needs no edits for a new game. Nothing here draws or depends on a
//! renderer; the crate builds for `wasm32-unknown-unknown` unchanged.

use sha2::{Digest, Sha256};

/// The generic game-input channel: what a host sends a game, whatever the
/// device. Keyboard, swipes and gamepads all map onto it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameInput {
    Left,
    Right,
    Up,
    Down,
    /// Turn back, undo, or go back: a game decides.
    Back,
    /// The game's main action.
    Use,
    /// Pause; a host also handles it itself.
    Pause,
}

impl GameInput {
    pub const ALL: [Self; 7] = [
        Self::Left,
        Self::Right,
        Self::Up,
        Self::Down,
        Self::Back,
        Self::Use,
        Self::Pause,
    ];

    /// A one-letter code, for replays.
    #[must_use]
    pub fn code(self) -> char {
        match self {
            Self::Left => 'L',
            Self::Right => 'R',
            Self::Up => 'U',
            Self::Down => 'D',
            Self::Back => 'B',
            Self::Use => 'E',
            Self::Pause => 'P',
        }
    }

    #[must_use]
    pub fn from_code(code: char) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.code() == code)
    }

    /// The input a desktop key gives, by the DOM's `KeyboardEvent.key`
    /// names: arrows and WASD, Space for Up, X or Backspace for Back, E or
    /// Enter for Use, Escape or P for Pause.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "ArrowLeft" | "a" | "A" => Some(Self::Left),
            "ArrowRight" | "d" | "D" => Some(Self::Right),
            "ArrowUp" | "w" | "W" | " " => Some(Self::Up),
            "ArrowDown" | "s" | "S" => Some(Self::Down),
            "x" | "X" | "Backspace" => Some(Self::Back),
            "e" | "E" | "Enter" => Some(Self::Use),
            "Escape" | "p" | "P" => Some(Self::Pause),
            _ => None,
        }
    }
}

/// A swipe recognizer for touch screens: a swipe needs `min_travel` pixels
/// within `max_millis`, and is taken by its dominant axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swipe {
    pub min_travel: f32,
    pub max_millis: f64,
}

impl Default for Swipe {
    /// The spec's 24 px within 250 ms.
    fn default() -> Self {
        Self {
            min_travel: 24.0,
            max_millis: 250.0,
        }
    }
}

impl Swipe {
    /// The input for a stroke of `dx`, `dy` pixels (y down) over `millis`.
    #[must_use]
    pub fn read(&self, dx: f32, dy: f32, millis: f64) -> Option<GameInput> {
        if millis > self.max_millis || dx.abs().max(dy.abs()) < self.min_travel {
            return None;
        }
        Some(if dx.abs() > dy.abs() {
            if dx < 0.0 {
                GameInput::Left
            } else {
                GameInput::Right
            }
        } else if dy < 0.0 {
            GameInput::Up
        } else {
            GameInput::Down
        })
    }
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Won,
    Lost,
    /// The player left: neither.
    Left,
}

/// A small pictogram a HUD element shows; hosts draw their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Food,
    Size,
    Score,
    Clock,
    Danger,
    Search,
    Power(u8),
}

/// A dot on a HUD map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapDot {
    /// Position in the map's square, 0 to 1 on each axis.
    pub x: f32,
    pub y: f32,
    /// sRGB colour.
    pub colour: u32,
    /// Radius as a share of the map's side.
    pub size: f32,
}

/// The HUD elements a game can ask a host to show.
#[derive(Clone, Debug, PartialEq)]
pub enum HudElement {
    /// A number with an icon, and optionally a ring filling from 0 to 1.
    Counter {
        icon: Icon,
        value: u32,
        ring: Option<f32>,
    },
    /// `filled` of `count` pips, and a bar toward the next from 0 to 1.
    Pips {
        icon: Icon,
        filled: u8,
        count: u8,
        bar: f32,
        label: &'static str,
    },
    /// The score, with a multiplier when one is running.
    Score { value: u32, multiplier: Option<f32> },
    /// A ring timer running down from 1 to 0, with its icon.
    Ring { icon: Icon, left: f32 },
    /// An arrow at the screen's edge toward something off screen, at
    /// `angle` radians (0 straight ahead, positive to the right), heavier
    /// as `weight` goes from 0 to 1, with a mark over it.
    EdgeArrow {
        angle: f32,
        weight: f32,
        mark: Option<Icon>,
    },
    /// A small map: the game's lines (segments in map space) and dots.
    Map {
        lines: Vec<[f32; 4]>,
        dots: Vec<MapDot>,
    },
}

/// What a game shows on its HUD this frame.
pub type Hud = Vec<HudElement>;

/// A game a host can drive.
pub trait CommunityGame {
    /// The game's id, as replays name it.
    fn id(&self) -> &'static str;
    /// Steps per second.
    fn hz(&self) -> u32;
    /// Advances one step with the inputs since the last.
    fn step(&mut self, inputs: &[GameInput]);
    /// The step count so far.
    fn tick(&self) -> u32;
    /// How the run ended, once it has.
    fn outcome(&self) -> Option<Outcome>;
    /// The HUD to show now.
    fn hud(&self) -> Hud;
    /// SHA-256 of the whole state, for replays.
    fn state_digest(&self) -> [u8; 32];
    /// SHA-256 of the rules and of the level being played.
    fn rules_digest(&self) -> [u8; 32];
    fn level_digest(&self) -> [u8; 32];
    fn seed(&self) -> u64;
}

/// The game-neutral replay envelope, `verse.game-replay.v1`: the game, the
/// digests of its rules and level, the seed, every input by tick, and the
/// digest of the final state. Verifying one is running it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replay {
    pub game: String,
    pub rules: [u8; 32],
    pub level: [u8; 32],
    pub seed: u64,
    pub inputs: Vec<(u32, GameInput)>,
    pub ticks: u32,
    pub result: [u8; 32],
}

pub const REPLAY_FORMAT: &str = "verse.game-replay.v1";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("not a digest: {text}"));
    }
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

impl Replay {
    /// Records `game` as `inputs` step it; call [`Replay::step`] instead of
    /// the game's own step.
    #[must_use]
    pub fn start(game: &dyn CommunityGame) -> Self {
        Self {
            game: game.id().to_owned(),
            rules: game.rules_digest(),
            level: game.level_digest(),
            seed: game.seed(),
            inputs: Vec::new(),
            ticks: 0,
            result: [0; 32],
        }
    }

    /// Steps `game`, keeping the inputs.
    pub fn step(&mut self, game: &mut dyn CommunityGame, inputs: &[GameInput]) {
        if game.outcome().is_none() {
            let tick = game.tick() + 1;
            self.inputs.extend(inputs.iter().map(|i| (tick, *i)));
        }
        game.step(inputs);
        self.ticks = game.tick();
        self.result = game.state_digest();
    }

    #[must_use]
    pub fn encode(&self) -> String {
        let inputs: Vec<String> = self
            .inputs
            .iter()
            .map(|(t, i)| format!("{t}:{}", i.code()))
            .collect();
        format!(
            "{REPLAY_FORMAT}\ngame {}\nrules {}\nlevel {}\nseed {}\ninputs {}\nticks {}\nresult {}\n",
            self.game,
            hex(&self.rules),
            hex(&self.level),
            self.seed,
            inputs.join(" "),
            self.ticks,
            hex(&self.result)
        )
    }

    pub fn decode(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some(REPLAY_FORMAT) {
            return Err(format!("not a {REPLAY_FORMAT}"));
        }
        let mut field = |name: &str| -> Result<String, String> {
            let line = lines.next().ok_or(format!("no {name}"))?;
            line.strip_prefix(name)
                .map(|rest| rest.trim().to_owned())
                .ok_or(format!("expected {name}"))
        };
        let game = field("game")?;
        let rules = unhex(&field("rules")?)?;
        let level = unhex(&field("level")?)?;
        let seed = field("seed")?.parse().map_err(|_| "bad seed")?;
        let mut inputs = Vec::new();
        for word in field("inputs")?.split_whitespace() {
            let (tick, code) = word.split_once(':').ok_or("bad input")?;
            let tick: u32 = tick.parse().map_err(|_| "bad input")?;
            let input = code
                .chars()
                .next()
                .and_then(GameInput::from_code)
                .filter(|_| code.len() == 1)
                .ok_or("bad input")?;
            if inputs.last().is_some_and(|(last, _)| *last > tick) {
                return Err("inputs out of order".into());
            }
            inputs.push((tick, input));
        }
        let ticks = field("ticks")?.parse().map_err(|_| "bad ticks")?;
        let result = unhex(&field("result")?)?;
        Ok(Self {
            game,
            rules,
            level,
            seed,
            inputs,
            ticks,
            result,
        })
    }

    /// Runs the replay on a fresh `game` and checks it ends the same.
    pub fn verify(&self, game: &mut dyn CommunityGame) -> Result<(), String> {
        if game.id() != self.game {
            return Err("another game's replay".into());
        }
        if game.rules_digest() != self.rules || game.level_digest() != self.level {
            return Err("made under other rules or on another level".into());
        }
        if game.seed() != self.seed {
            return Err("another seed".into());
        }
        let mut next = 0;
        while game.tick() < self.ticks && game.outcome().is_none() {
            let tick = game.tick() + 1;
            let start = next;
            while next < self.inputs.len() && self.inputs[next].0 == tick {
                next += 1;
            }
            let inputs: Vec<GameInput> = self.inputs[start..next].iter().map(|(_, i)| *i).collect();
            game.step(&inputs);
        }
        if next != self.inputs.len() || game.tick() != self.ticks {
            return Err("the replay runs differently".into());
        }
        if game.state_digest() != self.result {
            return Err("the replay ends differently".into());
        }
        Ok(())
    }

    /// SHA-256 of the encoded replay, its identity.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.encode().as_bytes()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A counter that ends at 10, for the contract's own tests.
    struct Count {
        n: i64,
        tick: u32,
    }

    impl CommunityGame for Count {
        fn id(&self) -> &'static str {
            "count"
        }
        fn hz(&self) -> u32 {
            60
        }
        fn step(&mut self, inputs: &[GameInput]) {
            self.tick += 1;
            for i in inputs {
                match i {
                    GameInput::Up => self.n += 1,
                    GameInput::Down => self.n -= 1,
                    _ => {}
                }
            }
        }
        fn tick(&self) -> u32 {
            self.tick
        }
        fn outcome(&self) -> Option<Outcome> {
            (self.n >= 10).then_some(Outcome::Won)
        }
        fn hud(&self) -> Hud {
            vec![HudElement::Counter {
                icon: Icon::Score,
                value: self.n.max(0) as u32,
                ring: Some(self.n as f32 / 10.0),
            }]
        }
        fn state_digest(&self) -> [u8; 32] {
            Sha256::digest(format!("{} {}", self.n, self.tick)).into()
        }
        fn rules_digest(&self) -> [u8; 32] {
            [1; 32]
        }
        fn level_digest(&self) -> [u8; 32] {
            [2; 32]
        }
        fn seed(&self) -> u64 {
            7
        }
    }

    #[test]
    fn a_replay_round_trips_and_verifies_and_refuses_tampering() {
        let mut game = Count { n: 0, tick: 0 };
        let mut replay = Replay::start(&game);
        for t in 0..40 {
            let inputs = if t % 3 == 0 {
                vec![GameInput::Up]
            } else {
                vec![]
            };
            replay.step(&mut game, &inputs);
            if game.outcome().is_some() {
                break;
            }
        }
        assert_eq!(game.outcome(), Some(Outcome::Won));
        let read = Replay::decode(&replay.encode()).unwrap();
        assert_eq!(read, replay);
        read.verify(&mut Count { n: 0, tick: 0 }).unwrap();
        let mut bad = read.clone();
        bad.inputs[0].1 = GameInput::Down;
        assert!(bad.verify(&mut Count { n: 0, tick: 0 }).is_err());
        assert!(Replay::decode("verse.game-replay.v2\n").is_err());
    }

    #[test]
    fn keys_and_swipes_map_onto_the_channel() {
        assert_eq!(GameInput::from_key("ArrowUp"), Some(GameInput::Up));
        assert_eq!(GameInput::from_key(" "), Some(GameInput::Up));
        assert_eq!(GameInput::from_key("Backspace"), Some(GameInput::Back));
        assert_eq!(GameInput::from_key("Escape"), Some(GameInput::Pause));
        assert_eq!(GameInput::from_key("q"), None);
        let swipe = Swipe::default();
        assert_eq!(swipe.read(-30.0, 5.0, 100.0), Some(GameInput::Left));
        assert_eq!(swipe.read(3.0, -40.0, 100.0), Some(GameInput::Up));
        assert_eq!(swipe.read(3.0, 40.0, 100.0), Some(GameInput::Down));
        assert_eq!(swipe.read(10.0, 10.0, 100.0), None, "too short");
        assert_eq!(swipe.read(60.0, 0.0, 400.0), None, "too slow");
        for input in GameInput::ALL {
            assert_eq!(GameInput::from_code(input.code()), Some(input));
        }
    }
}
