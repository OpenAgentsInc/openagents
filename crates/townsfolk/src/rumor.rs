//! Rumors (`docs/verse/generative-agents.md`, item 5, phase E2): a fact
//! one villager starts at a place and a time, which passes between
//! villagers whose routines put them together.
//!
//! A rumor is one file, `openagents.verse-rumor.v1` ([`Rumor`]). It always
//! names a real quest step ([`crate::quest::STEPS`]), so it points the
//! player at something true and never at a made-up objective, and it earns
//! no XP. Its repeat score, how likely villagers are to pass it on, is set
//! once, when it is proposed ([`Scorer`]), and stored in the file, so the
//! digest the roster admits covers it and every device spreads it alike
//! ([`crate::diffusion`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use town_clock::{TOWN_DAY_SECONDS, TownTime};
use world_tree::Tree;

use crate::routine::{Placement, Roster, Villager};
use crate::validate::{NoScreen, Screen, place, text};
use crate::{Code, Npc, Problem, Town, digest_of, good_id, parse_time, pretty, quest};

/// A rumor's schema.
pub const RUMOR_SCHEMA: &str = "openagents.verse-rumor.v1";
/// The longest fact, characters.
pub const MAX_FACT: usize = 200;
/// The most town days a rumor travels.
pub const MAX_DAYS: u32 = 7;
/// The repeat probability when no judge scored a rumor.
pub const PRIOR_REPEAT: f64 = 0.5;
/// The lowest and highest repeat probability a score maps onto: no rumor
/// is certain to pass or certain to die.
pub const REPEAT_RANGE: [f64; 2] = [0.1, 0.9];
/// The longest repeat basis, characters.
const MAX_BASIS: usize = 80;

/// How likely villagers are to repeat a rumor, set once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repeat {
    /// The chance one villager passes it to another at one meeting, 0 to 1.
    pub probability: f64,
    /// What set it, such as `jev openagents.rumor-repeat.v1 MODEL` or
    /// `prior`.
    pub basis: String,
}

/// A rumor, `openagents.verse-rumor.v1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rumor {
    pub schema: String,
    /// Stable ID, `[a-z0-9-]`, which also names its file; never a
    /// villager's ID.
    pub id: String,
    /// What villagers say, in the third person.
    pub fact: String,
    /// The villager who starts it.
    pub source: String,
    /// Where the source stands when it starts: a world-tree node.
    pub node: String,
    /// The town day it starts.
    pub day: i64,
    /// When on that day, `HH:MM`.
    pub at: String,
    /// How many town days it travels.
    pub days: u32,
    /// The quest step it points at, from [`quest::STEPS`].
    pub step: String,
    /// Set once, when it is proposed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Repeat>,
}

impl Rumor {
    /// Reads a rumor.
    ///
    /// # Errors
    ///
    /// When the text isn't a rumor.
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    /// The digest the roster admits, as for a definition.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_of(self)
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        pretty(self)
    }

    /// When it starts.
    #[must_use]
    pub fn start(&self) -> Option<TownTime> {
        parse_time(&self.at).map(|s| TownTime {
            day: self.day,
            second: f64::from(s),
        })
    }

    /// Town seconds since day zero when it starts.
    #[must_use]
    pub fn start_seconds(&self) -> Option<i64> {
        parse_time(&self.at).map(|s| self.day * i64::from(TOWN_DAY_SECONDS) + i64::from(s))
    }

    /// Whether it still travels on `day`, or hasn't started yet: what
    /// counts against the roster's `rumors_in_flight`.
    #[must_use]
    pub fn in_flight(&self, day: i64) -> bool {
        day < self.day + i64::from(self.days)
    }

    /// The repeat probability, or [`PRIOR_REPEAT`] before it is scored.
    #[must_use]
    pub fn probability(&self) -> f64 {
        self.repeat
            .as_ref()
            .map_or(PRIOR_REPEAT, |r| r.probability)
            .clamp(0.0, 1.0)
    }

    /// The quest step it points at.
    #[must_use]
    pub fn quest_step(&self) -> Option<&'static quest::Step> {
        quest::step_of(&self.step)
    }
}

/// Scores a rumor's repeat probability once, when it is proposed: Jev in
/// the command line, a fake in tests.
pub trait Scorer {
    /// # Errors
    ///
    /// Why it couldn't score, such as no key.
    fn score(&mut self, rumor: &Rumor, source: &Npc) -> Result<Repeat, String>;
}

/// A scorer that gives every rumor one probability: for tests, and the
/// prior when no judge is set up.
pub struct Fixed(pub f64, pub &'static str);

impl Fixed {
    /// [`PRIOR_REPEAT`], labeled `prior`.
    pub const PRIOR: Self = Self(PRIOR_REPEAT, "prior");
}

impl Scorer for Fixed {
    fn score(&mut self, _: &Rumor, _: &Npc) -> Result<Repeat, String> {
        Ok(Repeat {
            probability: self.0,
            basis: self.1.to_owned(),
        })
    }
}

/// A score question's answer as a repeat probability: the
/// probability-weighted level (or `score` without probabilities) of
/// `levels`, mapped onto [`REPEAT_RANGE`].
#[must_use]
pub fn repeat_from(probabilities: &BTreeMap<u32, f64>, score: f64, levels: usize) -> f64 {
    let mass: f64 = probabilities.values().sum();
    let position = if mass > 0.0 {
        probabilities
            .iter()
            .map(|(level, p)| f64::from(*level) * p)
            .sum::<f64>()
            / mass
    } else {
        score
    };
    let top = levels.saturating_sub(1).max(1) as f64;
    let [low, high] = REPEAT_RANGE;
    (low + (high - low) * (position / top).clamp(0.0, 1.0)).clamp(low, high)
}

/// What a rumor is checked against.
pub struct Against<'a> {
    pub tree: &'a Tree,
    /// The town's villagers: the source must be one, at the node when the
    /// rumor starts.
    pub villagers: &'a [Villager],
    pub seed: u64,
    pub screen: &'a dyn Screen,
    /// Whether a missing repeat score is a problem: it is for admission
    /// and loading, not before propose scores it.
    pub scored: bool,
}

/// Every problem with `rumor`; empty when it is fit to admit.
#[must_use]
pub fn check(rumor: &Rumor, against: &Against) -> Vec<Problem> {
    let mut out = Vec::new();
    if rumor.schema != RUMOR_SCHEMA {
        out.push(Problem::new(
            "schema",
            Code::Schema,
            format!("is {:?}, not {RUMOR_SCHEMA}", rumor.schema),
        ));
    }
    if !good_id(&rumor.id) {
        out.push(Problem::new(
            "id",
            Code::Id,
            format!(
                "{:?} isn't 1 to {} characters of a-z, 0-9, and inner hyphens",
                rumor.id,
                crate::MAX_ID
            ),
        ));
    }
    if against.villagers.iter().any(|v| v.id() == rumor.id) {
        out.push(Problem::new(
            "id",
            Code::Duplicate,
            format!("{} is a villager's ID", rumor.id),
        ));
    }
    text(
        &mut out,
        against.screen,
        "fact".into(),
        &rumor.fact,
        MAX_FACT,
    );
    if quest::step_of(&rumor.step).is_none() {
        out.push(Problem::new(
            "step",
            Code::Step,
            format!(
                "{:?} isn't a quest step; a rumor points at a real one (townsfolk::quest::STEPS)",
                rumor.step
            ),
        ));
    }
    if !(1..=MAX_DAYS).contains(&rumor.days) {
        out.push(Problem::new(
            "days",
            Code::Time,
            format!("is {}, not 1 to {MAX_DAYS}", rumor.days),
        ));
    }
    match &rumor.repeat {
        None if against.scored => out.push(Problem::new(
            "repeat",
            Code::Unscored,
            "has no repeat score; propose sets it",
        )),
        None => {}
        Some(r) => {
            if !(r.probability.is_finite() && (0.0..=1.0).contains(&r.probability)) {
                out.push(Problem::new(
                    "repeat.probability",
                    Code::Unscored,
                    format!("is {}, not 0 to 1", r.probability),
                ));
            }
            text(
                &mut out,
                &NoScreen,
                "repeat.basis".into(),
                &r.basis,
                MAX_BASIS,
            );
        }
    }
    let placed = place(&mut out, against.tree, "node", &rumor.node);
    let Some(start) = rumor.start() else {
        out.push(Problem::new(
            "at",
            Code::Time,
            format!("{:?} isn't HH:MM", rumor.at),
        ));
        return out;
    };
    let Some(source) = against.villagers.iter().find(|v| v.id() == rumor.source) else {
        out.push(Problem::new(
            "source",
            Code::Source,
            format!("{} isn't a villager in the town", rumor.source),
        ));
        return out;
    };
    if placed {
        match source.at(against.seed, start) {
            Placement::At { node, .. } if node == rumor.node => {}
            Placement::At { node, .. } => out.push(Problem::new(
                "node",
                Code::Source,
                format!(
                    "{} stands at {node} at {} on day {}, not at {}",
                    source.npc.name, rumor.at, rumor.day, rumor.node
                ),
            )),
            Placement::Walking { to, .. } => out.push(Problem::new(
                "at",
                Code::Source,
                format!(
                    "{} is walking to {to} at {} on day {}; start it where they stand",
                    source.npc.name, rumor.at, rumor.day
                ),
            )),
        }
    }
    out
}

/// The admitted rumors from the rumor files `files`, against the loaded
/// town: a rumor loads only when the roster admits its ID with its exact
/// digest and it passes [`check`]. Every one that doesn't is left out and
/// reported.
#[must_use]
pub fn load(roster: &Roster, files: &[&str], tree: &Tree) -> (Vec<Rumor>, Vec<Problem>) {
    load_town(&roster.town, &roster.villagers, files, tree)
}

fn load_town(
    town: &Town,
    villagers: &[Villager],
    files: &[&str],
    tree: &Tree,
) -> (Vec<Rumor>, Vec<Problem>) {
    let mut loaded: Vec<Rumor> = Vec::new();
    let mut left_out = Vec::new();
    for (i, json) in files.iter().enumerate() {
        let rumor = match Rumor::parse(json) {
            Ok(r) => r,
            Err(e) => {
                left_out.push(Problem::new(format!("rumor file {i}"), Code::Schema, e));
                continue;
            }
        };
        let digest = rumor.digest();
        match town.rumor(&rumor.id) {
            None => {
                left_out.push(
                    Problem::new("id", Code::NotAdmitted, "the roster doesn't admit it")
                        .of(&rumor.id),
                );
                continue;
            }
            Some(a) if a.digest != digest => {
                left_out.push(
                    Problem::new(
                        "digest",
                        Code::Digest,
                        format!("is {digest}, but the roster admits {}", a.digest),
                    )
                    .of(&rumor.id),
                );
                continue;
            }
            Some(_) => {}
        }
        if loaded.iter().any(|r| r.id == rumor.id) {
            left_out.push(Problem::new("id", Code::Duplicate, "two files define it").of(&rumor.id));
            continue;
        }
        let found = check(
            &rumor,
            &Against {
                tree,
                villagers,
                seed: town.seed,
                screen: &NoScreen,
                scored: true,
            },
        );
        if found.is_empty() {
            loaded.push(rumor);
        } else {
            let id = rumor.id.clone();
            left_out.extend(found.into_iter().map(|p| p.of(&id)));
        }
    }
    for a in &town.rumors {
        if !loaded.iter().any(|r| r.id == a.id)
            && !left_out
                .iter()
                .any(|p| p.field.starts_with(&format!("{}:", a.id)))
        {
            left_out.push(Problem::new("file", Code::Missing, "no rumor file holds it").of(&a.id));
        }
    }
    loaded.sort_by_key(|r| town.rumors.iter().position(|a| a.id == r.id));
    (loaded, left_out)
}
