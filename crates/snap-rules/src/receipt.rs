//! Run receipts (`snap.run-receipt.v1`) and their verification.
//!
//! A receipt names the rules and the job by digest, and every input by
//! tick, with the outcome, the stars and the digest of the final state.
//! Verifying it plays the inputs again: a shared build or a best time is
//! only as good as a replay that ends the same.
//!
//! ```text
//! snap.run-receipt.v1
//! game snap-station
//! rules 1 <hex SHA-256 of the rules>
//! level <hex SHA-256 of the job file>
//! inputs 3:P/square/1500/500/0 9:B/bolt/1000/500/0 12:G
//! ticks 260
//! outcome won
//! stars 3
//! state <hex SHA-256 of the final state>
//! ```
//!
//! Input codes: `P/kind/x/y/turn` places (x and y in millimetres), `R/x/y`
//! takes a piece off, `G` is Go and `F` is Fix.

use sha2::{Digest, Sha256};

use crate::game::{Fail, Game, Input, Status};
use crate::kinds::PieceKind;
use crate::level::Level;
use crate::sim;

pub const FORMAT: &str = "snap.run-receipt.v1";
pub const GAME: &str = "snap-station";
/// Bumped whenever a rule changes how a run plays out.
pub const RULES_VERSION: u32 = 1;

/// SHA-256 over the rules' version and their numbers, so a receipt made
/// under other rules is refused rather than replayed wrongly.
#[must_use]
pub fn rules_digest() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(format!("{GAME} rules {RULES_VERSION}\n"));
    hash.update(format!(
        "{} {} {} {} {} {} {} {} {} {} {}\n",
        crate::HZ,
        crate::SUBSTEPS,
        sim::K_BAR,
        sim::C_BAR,
        sim::K_MAGNET,
        sim::C_MAGNET,
        sim::MAGNET,
        sim::K_LEAN,
        sim::C_LEAN,
        sim::LEAN_POP,
        sim::THRUST,
    ));
    hash.update(format!(
        "{} {} {} {} {} {}\n",
        sim::RAMP,
        sim::MOOR_REACH,
        sim::SETTLE,
        crate::build::SNAP_REACH,
        crate::build::SNAP_COS,
        crate::build::JOIN_GAP,
    ));
    for kind in PieceKind::ALL {
        hash.update(format!("{} {} {}\n", kind.word(), kind.mass(), kind.sail()));
    }
    hash.finalize().into()
}

/// Lower-case hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
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

/// An input's code.
#[must_use]
pub fn encode_input(input: Input) -> String {
    match input {
        Input::Place { kind, x, y, turn } => format!("P/{}/{x}/{y}/{turn}", kind.word()),
        Input::Remove { x, y } => format!("R/{x}/{y}"),
        Input::Go => "G".into(),
        Input::Fix => "F".into(),
    }
}

/// Reads an input's code.
///
/// # Errors
///
/// A code that isn't one of the four.
pub fn decode_input(text: &str) -> Result<Input, String> {
    let parts: Vec<&str> = text.split('/').collect();
    let int = |s: &str| {
        s.parse::<i32>()
            .map_err(|_| format!("not a number: {text}"))
    };
    match parts.as_slice() {
        ["G"] => Ok(Input::Go),
        ["F"] => Ok(Input::Fix),
        ["R", x, y] => Ok(Input::Remove {
            x: int(x)?,
            y: int(y)?,
        }),
        ["P", kind, x, y, turn] => Ok(Input::Place {
            kind: PieceKind::from_word(kind).ok_or_else(|| format!("no such piece: {text}"))?,
            x: int(x)?,
            y: int(y)?,
            turn: turn.parse().map_err(|_| format!("not a turn: {text}"))?,
        }),
        _ => Err(format!("not an input: {text}")),
    }
}

fn outcome_word(status: Status) -> String {
    match status {
        Status::Building => "building".into(),
        Status::Running => "running".into(),
        Status::Won => "won".into(),
        Status::Failed(why) => format!("failed-{why:?}").to_ascii_lowercase(),
    }
}

/// A finished run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub rules: [u8; 32],
    pub level: [u8; 32],
    /// Each input with the tick it was applied on.
    pub inputs: Vec<(u32, Input)>,
    pub ticks: u32,
    pub outcome: String,
    pub stars: u8,
    pub state: [u8; 32],
}

/// Records a game's inputs as it plays.
#[derive(Clone, Debug, Default)]
pub struct Recorder {
    inputs: Vec<(u32, Input)>,
}

impl Recorder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Steps `game` with `inputs`, keeping them.
    pub fn step(&mut self, game: &mut Game, inputs: &[Input]) {
        let tick = game.tick + 1;
        self.inputs.extend(inputs.iter().map(|i| (tick, *i)));
        game.step(inputs);
    }

    /// The receipt for `game` as it stands.
    #[must_use]
    pub fn finish(&self, game: &Game) -> Receipt {
        Receipt {
            rules: rules_digest(),
            level: game.level.digest,
            inputs: self.inputs.clone(),
            ticks: game.tick,
            outcome: outcome_word(game.status),
            stars: game.stars,
            state: game.state_digest(),
        }
    }
}

impl Receipt {
    #[must_use]
    pub fn encode(&self) -> String {
        let inputs: Vec<String> = self
            .inputs
            .iter()
            .map(|(tick, input)| format!("{tick}:{}", encode_input(*input)))
            .collect();
        format!(
            "{FORMAT}\ngame {GAME}\nrules {RULES_VERSION} {}\nlevel {}\ninputs {}\nticks {}\noutcome {}\nstars {}\nstate {}\n",
            hex(&self.rules),
            hex(&self.level),
            inputs.join(" "),
            self.ticks,
            self.outcome,
            self.stars,
            hex(&self.state)
        )
    }

    /// Reads a receipt.
    ///
    /// # Errors
    ///
    /// A malformed or missing line.
    pub fn decode(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some(FORMAT) {
            return Err(format!("not a {FORMAT}"));
        }
        let mut field = |name: &str| -> Result<String, String> {
            let line = lines.next().ok_or_else(|| format!("no {name}"))?;
            line.strip_prefix(name)
                .and_then(|rest| {
                    rest.strip_prefix(' ')
                        .or(Some(rest).filter(|r| r.is_empty()))
                })
                .map(str::to_owned)
                .ok_or_else(|| format!("expected {name}: {line}"))
        };
        if field("game")? != GAME {
            return Err("another game's receipt".into());
        }
        let rules = field("rules")?;
        let (version, digest) = rules.split_once(' ').ok_or("no rules digest")?;
        if version != RULES_VERSION.to_string() {
            return Err(format!("rules version {version}"));
        }
        let rules = unhex(digest)?;
        let level = unhex(&field("level")?)?;
        let mut inputs = Vec::new();
        for item in field("inputs")?.split_whitespace() {
            let (tick, code) = item.split_once(':').ok_or("an input without a tick")?;
            inputs.push((tick.parse().map_err(|_| "a bad tick")?, decode_input(code)?));
        }
        let ticks = field("ticks")?.parse().map_err(|_| "bad ticks")?;
        let outcome = field("outcome")?;
        let stars = field("stars")?.parse().map_err(|_| "bad stars")?;
        let state = unhex(&field("state")?)?;
        Ok(Self {
            rules,
            level,
            inputs,
            ticks,
            outcome,
            stars,
            state,
        })
    }

    /// Plays the receipt's inputs on `level` and checks that it ends as
    /// the receipt says.
    ///
    /// # Errors
    ///
    /// Other rules, another job, or a different ending.
    pub fn verify(&self, level: &Level) -> Result<(), String> {
        if self.rules != rules_digest() {
            return Err("made under other rules".into());
        }
        if self.level != level.digest {
            return Err("made on another job".into());
        }
        let mut game = Game::new(level.clone());
        let mut next = 0;
        while game.tick < self.ticks {
            let tick = game.tick + 1;
            let mut now = Vec::new();
            while next < self.inputs.len() && self.inputs[next].0 == tick {
                now.push(self.inputs[next].1);
                next += 1;
            }
            if next < self.inputs.len() && self.inputs[next].0 < tick {
                return Err("inputs out of order".into());
            }
            game.step(&now);
        }
        if outcome_word(game.status) != self.outcome || game.stars != self.stars {
            return Err(format!("it ends {} instead", outcome_word(game.status)));
        }
        if game.state_digest() != self.state {
            return Err("it ends in another state".into());
        }
        Ok(())
    }
}

/// The outcome word a failure gives, for tests.
#[must_use]
pub fn failed(why: Fail) -> String {
    outcome_word(Status::Failed(why))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level;

    /// A fixed run of job 1: three squares, a bolt, Go, to the end.
    fn fixed_run() -> (Game, Recorder) {
        let mut game = Game::new(level::job(1));
        let mut recorder = Recorder::new();
        for x in [1500, 2500, 3500] {
            recorder.step(
                &mut game,
                &[Input::Place {
                    kind: PieceKind::Square,
                    x,
                    y: 500,
                    turn: 0,
                }],
            );
        }
        recorder.step(&mut game, &[Input::Go]);
        while game.status == Status::Running {
            recorder.step(&mut game, &[]);
        }
        (game, recorder)
    }

    #[test]
    fn a_receipt_round_trips_and_verifies_by_replay() {
        let (game, recorder) = fixed_run();
        let receipt = recorder.finish(&game);
        assert_eq!(receipt.outcome, "won");
        let text = receipt.encode();
        let read = Receipt::decode(&text).unwrap();
        assert_eq!(read, receipt);
        read.verify(&level::job(1)).unwrap();
    }

    #[test]
    fn a_tampered_receipt_is_refused() {
        let (game, recorder) = fixed_run();
        let mut receipt = recorder.finish(&game);
        receipt.inputs.remove(1);
        assert!(receipt.verify(&level::job(1)).is_err(), "one square fewer");
        let receipt = recorder.finish(&game);
        assert!(receipt.verify(&level::job(2)).is_err(), "another job");
        let mut stars = receipt.clone();
        stars.stars = 3 - stars.stars.min(2);
        if stars.stars != receipt.stars {
            assert!(stars.verify(&level::job(1)).is_err(), "other stars");
        }
        assert_eq!(
            decode_input("P/strut/-12/40/3"),
            Ok(Input::Place {
                kind: PieceKind::Strut,
                x: -12,
                y: 40,
                turn: 3
            })
        );
        assert!(decode_input("P/glue/1/2/3").is_err());
        assert_eq!(failed(Fail::CameApart), "failed-cameapart");
    }

    /// The pinned final state of the fixed run. `scripts/snap-wasm-test.sh`
    /// runs this test as wasm32, where it must give the same digest: the
    /// simulation's arithmetic is the same bit for bit on every platform.
    #[test]
    fn a_fixed_run_ends_in_the_same_state_everywhere() {
        let (game, _) = fixed_run();
        assert_eq!(hex(&game.state_digest()), PINNED);
    }

    const PINNED: &str = "44761e7b1d2e6fe265583b3fe0403940f7f7e19f7d50510f3768f39976bd3954";
}
