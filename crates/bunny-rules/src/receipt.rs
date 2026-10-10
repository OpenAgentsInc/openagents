//! Run receipts (`bunny.run-receipt.v1`) and their verification.
//!
//! A receipt names the rules and the level by digest, the seed and the
//! mode, and every input by tick, with the outcome, the score and the
//! digest of the final state. Verifying it runs the inputs again: a ghost
//! or a leaderboard time is only as good as a replay that ends the same.
//!
//! ```text
//! bunny.run-receipt.v1
//! game grow-little-bunny
//! rules 1 <hex SHA-256 of the rules>
//! level <hex SHA-256 of the level file>
//! seed 42
//! gentle 0
//! inputs 12:L 40:J 41:R
//! ticks 5400
//! outcome won
//! score 9120
//! state <hex SHA-256 of the final state>
//! ```

use sha2::{Digest, Sha256};

use crate::game::{Game, Input, Status};
use crate::garden::Garden;
use crate::kinds::{EdibleKind, ObstacleKind, PowerKind};

pub const FORMAT: &str = "bunny.run-receipt.v1";
pub const GAME: &str = "grow-little-bunny";
/// Bumped whenever a rule changes how a run plays out.
pub const RULES_VERSION: u32 = 1;

/// SHA-256 over the rules' version and their tables, so a receipt made
/// under other rules is refused rather than replayed wrongly.
#[must_use]
pub fn rules_digest() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(format!("{GAME} rules {RULES_VERSION}\n"));
    hash.update(format!(
        "{:?} {:?} {:?} {:?}\n",
        crate::TIER_QUARTERS,
        crate::TIER_SPEED,
        crate::TIER_SIGHT,
        crate::LANE_WIDTH
    ));
    for kind in EdibleKind::ALL {
        hash.update(format!(
            "{} {} {} {}\n",
            kind.word(),
            kind.points(),
            kind.quarters(),
            kind.min_tier()
        ));
    }
    for kind in ObstacleKind::ALL {
        let row: Vec<_> = (0..5).map(|t| kind.contact(t)).collect();
        hash.update(format!("{} {row:?} {}\n", kind.word(), kind.smash_points()));
    }
    for kind in PowerKind::ALL {
        hash.update(format!("{} {}\n", kind.word(), kind.duration()));
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

/// A finished run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub rules: [u8; 32],
    pub level: [u8; 32],
    pub seed: u64,
    pub gentle: bool,
    /// Each input with the tick it was applied on.
    pub inputs: Vec<(u32, Input)>,
    pub ticks: u32,
    pub outcome: Status,
    pub score: u32,
    pub state: [u8; 32],
}

/// Records a run's inputs as it plays.
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
        if game.status == crate::Status::Playing {
            let tick = game.tick + 1;
            self.inputs.extend(inputs.iter().map(|i| (tick, *i)));
        }
        game.step(inputs);
    }

    /// The receipt of a finished run.
    #[must_use]
    pub fn finish(&self, game: &Game) -> Receipt {
        Receipt {
            rules: rules_digest(),
            level: game.garden.digest,
            seed: game.seed,
            gentle: game.gentle,
            inputs: self.inputs.clone(),
            ticks: game.tick,
            outcome: game.status,
            score: game.score,
            state: game.state_digest(),
        }
    }
}

impl Receipt {
    /// The receipt as text.
    #[must_use]
    pub fn encode(&self) -> String {
        let inputs: Vec<String> = self
            .inputs
            .iter()
            .map(|(tick, input)| format!("{tick}:{}", input.code()))
            .collect();
        format!(
            "{FORMAT}\ngame {GAME}\nrules {RULES_VERSION} {}\nlevel {}\nseed {}\ngentle {}\n\
             inputs {}\nticks {}\noutcome {}\nscore {}\nstate {}\n",
            hex(&self.rules),
            hex(&self.level),
            self.seed,
            u8::from(self.gentle),
            inputs.join(" "),
            self.ticks,
            self.outcome.word(),
            self.score,
            hex(&self.state),
        )
    }

    /// Reads a receipt.
    pub fn decode(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some(FORMAT) {
            return Err(format!("not a {FORMAT}"));
        }
        let mut field = |name: &str| -> Result<String, String> {
            let line = lines.next().ok_or(format!("no {name}"))?;
            line.strip_prefix(name)
                .and_then(|rest| rest.strip_prefix(' ').or(Some(rest)).filter(|_| true))
                .map(|rest| rest.trim().to_owned())
                .ok_or(format!("expected {name}"))
        };
        if field("game")? != GAME {
            return Err("another game's receipt".into());
        }
        let rules = field("rules")?;
        let (version, digest) = rules.split_once(' ').ok_or("bad rules")?;
        if version != RULES_VERSION.to_string() {
            return Err(format!("made under rules {version}"));
        }
        let rules = unhex(digest)?;
        let level = unhex(&field("level")?)?;
        let seed = field("seed")?.parse().map_err(|_| "bad seed")?;
        let gentle = match field("gentle")?.as_str() {
            "0" => false,
            "1" => true,
            _ => return Err("bad mode".into()),
        };
        let mut inputs = Vec::new();
        let mut last = 0;
        for word in field("inputs")?.split_whitespace() {
            let (tick, code) = word.split_once(':').ok_or("bad input")?;
            let tick: u32 = tick.parse().map_err(|_| "bad input tick")?;
            let mut chars = code.chars();
            let input = chars
                .next()
                .and_then(Input::from_code)
                .filter(|_| chars.next().is_none())
                .ok_or("bad input")?;
            if tick < last {
                return Err("inputs out of order".into());
            }
            last = tick;
            inputs.push((tick, input));
        }
        let ticks = field("ticks")?.parse().map_err(|_| "bad ticks")?;
        let outcome = match field("outcome")?.as_str() {
            "won" => Status::Won,
            "caught" => Status::Caught,
            "left" => Status::Left,
            "playing" => Status::Playing,
            _ => return Err("bad outcome".into()),
        };
        let score = field("score")?.parse().map_err(|_| "bad score")?;
        let state = unhex(&field("state")?)?;
        Ok(Self {
            rules,
            level,
            seed,
            gentle,
            inputs,
            ticks,
            outcome,
            score,
            state,
        })
    }

    /// Plays the receipt's inputs again on `garden` and checks that it ends
    /// the way the receipt says.
    pub fn verify(&self, garden: &Garden) -> Result<(), String> {
        if self.rules != rules_digest() {
            return Err("made under other rules".into());
        }
        if self.level != garden.digest {
            return Err("made on another garden".into());
        }
        if self.ticks > 60 * 60 * crate::HZ {
            return Err("longer than an hour".into());
        }
        let mut game = Game::with_seed(garden.clone(), self.seed, self.gentle);
        let mut next = 0;
        while game.tick < self.ticks && game.status == Status::Playing {
            let tick = game.tick + 1;
            let start = next;
            while next < self.inputs.len() && self.inputs[next].0 == tick {
                next += 1;
            }
            let inputs: Vec<Input> = self.inputs[start..next].iter().map(|(_, i)| *i).collect();
            game.step(&inputs);
        }
        if next != self.inputs.len() {
            return Err("inputs after the run ended".into());
        }
        if game.tick != self.ticks
            || game.status != self.outcome
            || game.score != self.score
            || game.state_digest() != self.state
        {
            return Err("the replay ends differently".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bot, level};

    fn bot_run(number: usize, seed: u64) -> (Game, Receipt) {
        let mut game = Game::with_seed(level::garden(number), seed, false);
        let mut recorder = Recorder::new();
        let mut brain = bot::Bot::new();
        for _ in 0..crate::HZ * 400 {
            let inputs: Vec<Input> = brain.input(&game).into_iter().collect();
            recorder.step(&mut game, &inputs);
            if game.status != Status::Playing {
                break;
            }
        }
        let receipt = recorder.finish(&game);
        (game, receipt)
    }

    #[test]
    fn a_receipt_round_trips_and_verifies_by_replay() {
        let (game, receipt) = bot_run(1, 3);
        assert!(!receipt.inputs.is_empty());
        let text = receipt.encode();
        assert!(text.starts_with(FORMAT));
        let read = Receipt::decode(&text).unwrap();
        assert_eq!(read, receipt);
        read.verify(&game.garden).unwrap();
    }

    #[test]
    fn a_tampered_receipt_is_refused() {
        let (game, receipt) = bot_run(1, 5);
        let mut faster = receipt.clone();
        faster.ticks -= 1;
        assert!(faster.verify(&game.garden).is_err());
        let mut richer = receipt.clone();
        richer.score += 100;
        assert!(richer.verify(&game.garden).is_err());
        let mut edited = receipt.clone();
        if let Some(first) = edited.inputs.first_mut() {
            first.1 = if first.1 == Input::Left {
                Input::Right
            } else {
                Input::Left
            };
        }
        assert!(edited.verify(&game.garden).is_err());
        assert!(receipt.verify(&level::garden(2)).is_err(), "another garden");
        let text = receipt.encode().replace("rules 1 ", "rules 2 ");
        assert!(Receipt::decode(&text).is_err());
    }

    /// The digest of one fixed run, the same on every platform. The test
    /// runs natively and, with `scripts/bunny-wasm-test.sh`, as wasm32.
    #[test]
    fn a_fixed_run_ends_in_the_same_state_everywhere() {
        let (game, receipt) = bot_run(2, 11);
        let digest = hex(&receipt.state);
        assert_eq!(receipt.ticks, game.tick);
        assert_eq!(digest, GOLDEN_STATE, "rules changed? update GOLDEN_STATE");
    }

    const GOLDEN_STATE: &str = "1f7a4f54d6778f851e40785892910a38ed87b99fb9f5c0c9922ea2abe3af7ce9";
}
