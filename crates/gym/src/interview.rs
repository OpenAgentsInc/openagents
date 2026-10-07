//! Agent interviews: a pinned suite of questions about an agent's own work,
//! asked against a frozen fixture of its journal and memory
//! (`docs/verse/generative-agents.md`, item 7).
//!
//! This module owns what the Gym can own without knowing how an agent
//! remembers: the fixture's manifest and digest, the interview categories,
//! the code-checked scorer, and the row a run appends to a receipt-chained
//! [`crate::store::Store`]. Building a briefing and answering a question
//! belong to the agent, so the runner and its arms live in
//! `crates/coder/src/task/agent_interview.rs`.
//!
//! An item is an ordinary [`crate::suite::Item`]: `family` is the
//! [`Category`], `kind` says how it's scored, `question.text` is the
//! question, and `truth` is a canonical answer. Its `state` is an
//! [`ItemState`]: the fixture and its digest, when the interview happens,
//! the [`Check`] a code-checked answer must pass, and the fixture records
//! that hold the answer, as `journal:POS` (the 1-based line of
//! `journal.jsonl`) or `memory:ID`.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::row::DoorIdentity;
use crate::suite::{Item, Partition, Suite, SuiteError};

/// The schema an interview row carries.
pub const ROW_SCHEMA: &str = "openagents.gym.interview_row.v1";

/// The schema a fixture manifest carries.
pub const FIXTURE_SCHEMA: &str = "openagents.agent-interview-fixture.v1";

/// The `kind` of an item whose answer code checks.
pub const KIND_RECALL: &str = "recall";

/// The suite of Alice interviews, as committed.
pub const ALICE_V1_SUITE: &str = include_str!("../suites/alice-interview-v1.json");
/// Its fixture's manifest.
pub const ALICE_V1_FIXTURE: &str = include_str!("../suites/alice-interview-v1/fixture.json");
/// Its fixture's journal, one `openagents.agent-journal-entry.v1` per line.
pub const ALICE_V1_JOURNAL: &str = include_str!("../suites/alice-interview-v1/journal.jsonl");
/// Its fixture's memory, one `openagents.agent-memory-entry.v1` per line.
pub const ALICE_V1_MEMORY: &str = include_str!("../suites/alice-interview-v1/memory.jsonl");

/// The paper's five kinds of interview question.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// "Describe your work."
    #[serde(rename = "self")]
    SelfKnowledge,
    /// "What did you merge on October 5?"
    Memory,
    /// "What will you do at 3 PM?"
    Plan,
    /// "The default branch's checks just failed; what do you do?"
    Reaction,
    /// "What have you learned about how the owner wants commits?"
    Reflection,
}

impl Category {
    /// Every category, in the paper's order.
    pub const ALL: [Self; 5] = [
        Self::SelfKnowledge,
        Self::Memory,
        Self::Plan,
        Self::Reaction,
        Self::Reflection,
    ];

    /// The `family` an item of this category carries.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SelfKnowledge => "self",
            Self::Memory => "memory",
            Self::Plan => "plan",
            Self::Reaction => "reaction",
            Self::Reflection => "reflection",
        }
    }

    /// The category a `family` names.
    #[must_use]
    pub fn parse(family: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == family)
    }

    /// Whether code checks this category's answers against the fixture.
    /// The rest wait for a judge.
    #[must_use]
    pub const fn code_checked(self) -> bool {
        matches!(self, Self::Memory | Self::Plan)
    }
}

impl fmt::Display for Category {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// What a code-checked answer must hold.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// Every group needs one of its terms in the answer.
    pub all: Vec<Vec<String>>,
    /// No term here may appear, so an answer that lists every candidate
    /// doesn't pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub none: Vec<Vec<String>>,
}

/// An item's `state`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemState {
    /// The fixture's name.
    pub fixture: String,
    /// The fixture's digest, which [`Fixture::load`] computes.
    pub fixture_digest: String,
    /// When the interview happens, Unix seconds.
    pub as_of: u64,
    /// The check, for a code-checked item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<Check>,
    /// The fixture records that hold the answer.
    #[serde(default)]
    pub sources: Vec<String>,
}

impl ItemState {
    /// The state of `item`.
    ///
    /// # Errors
    /// When the state isn't an interview state.
    pub fn of(item: &Item) -> Result<Self, String> {
        serde_json::from_value(item.state.clone())
            .map_err(|e| format!("item {} has no interview state: {e}", item.id))
    }
}

/// The question text of `item`.
#[must_use]
pub fn question(item: &Item) -> &str {
    item.question
        .as_ref()
        .and_then(|q| q.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// What the scorer found.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grade {
    pub correct: bool,
    /// The `all` groups the answer missed, each as its first term.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// The `none` terms the answer holds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden: Vec<String>,
}

/// Lowercase ASCII with runs of whitespace folded to one space.
fn fold(text: &str) -> String {
    text.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Grades `answer` against `check`: case-insensitive substring matches,
/// with whitespace folded.
#[must_use]
pub fn grade(check: &Check, answer: &str) -> Grade {
    let answer = fold(answer);
    let holds = |term: &String| !term.is_empty() && answer.contains(&fold(term));
    let missing: Vec<String> = check
        .all
        .iter()
        .filter(|group| !group.iter().any(holds))
        .map(|group| group.first().cloned().unwrap_or_default())
        .collect();
    let forbidden: Vec<String> = check
        .none
        .iter()
        .flatten()
        .filter(|term| holds(term))
        .cloned()
        .collect();
    Grade {
        correct: missing.is_empty() && forbidden.is_empty() && !check.all.is_empty(),
        missing,
        forbidden,
    }
}

/// A fixture's manifest (`openagents.agent-interview-fixture.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub name: String,
    /// The agent the fixture records.
    pub agent: String,
    /// The workspace its briefings name.
    pub workspace: String,
    /// The first journal row's time, Unix seconds.
    pub from: u64,
    /// When the interviews happen unless an item says otherwise.
    pub as_of: u64,
    /// The content is made up, never recorded from a real host.
    pub synthetic: bool,
    pub journal_rows: usize,
    pub memory_entries: usize,
    pub journal_sha256: String,
    pub memory_sha256: String,
    /// [`fixture_digest`] over the two files.
    pub digest: String,
}

/// A fixture: its manifest and the two files' text, checked.
#[derive(Clone, Debug)]
pub struct Fixture {
    pub manifest: Manifest,
    pub journal: String,
    pub memory: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The digest over a fixture's two files.
#[must_use]
pub fn fixture_digest(journal: &str, memory: &str) -> String {
    sha256(
        format!(
            "{FIXTURE_SCHEMA}\njournal {}\nmemory {}\n",
            sha256(journal.as_bytes()),
            sha256(memory.as_bytes())
        )
        .as_bytes(),
    )
}

impl Fixture {
    /// Loads a fixture and checks its digests and counts.
    ///
    /// # Errors
    /// When the manifest doesn't read or doesn't describe the files.
    pub fn load(manifest: &str, journal: &str, memory: &str) -> Result<Self, String> {
        let manifest: Manifest = serde_json::from_str(manifest)
            .map_err(|e| format!("the fixture manifest doesn't read: {e}"))?;
        if manifest.schema != FIXTURE_SCHEMA {
            return Err(format!("the fixture manifest isn't {FIXTURE_SCHEMA}"));
        }
        let lines = |text: &str| text.lines().filter(|l| !l.trim().is_empty()).count();
        let checks = [
            (
                "journal digest",
                sha256(journal.as_bytes()),
                manifest.journal_sha256.clone(),
            ),
            (
                "memory digest",
                sha256(memory.as_bytes()),
                manifest.memory_sha256.clone(),
            ),
            (
                "fixture digest",
                fixture_digest(journal, memory),
                manifest.digest.clone(),
            ),
            (
                "journal rows",
                lines(journal).to_string(),
                manifest.journal_rows.to_string(),
            ),
            (
                "memory entries",
                lines(memory).to_string(),
                manifest.memory_entries.to_string(),
            ),
        ];
        for (what, computed, recorded) in checks {
            if computed != recorded {
                return Err(format!(
                    "the fixture's {what} is {computed}, and its manifest records {recorded}"
                ));
            }
        }
        Ok(Self {
            manifest,
            journal: journal.to_string(),
            memory: memory.to_string(),
        })
    }

    /// The text of a `journal:POS` or `memory:ID` reference.
    #[must_use]
    pub fn record(&self, reference: &str) -> Option<(u64, String)> {
        let (kind, key) = reference.split_once(':')?;
        let key: usize = key.parse().ok()?;
        let row: Value = match kind {
            "journal" => {
                serde_json::from_str(self.journal.lines().nth(key.checked_sub(1)?)?).ok()?
            }
            "memory" => self
                .memory
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .find(|e| e["id"].as_u64() == Some(key as u64))?,
            _ => return None,
        };
        Some((row["at"].as_u64()?, row["text"].as_str()?.to_string()))
    }
}

/// The committed Alice suite.
///
/// # Errors
/// When the suite doesn't load.
pub fn alice_v1_suite() -> Result<Suite, SuiteError> {
    Suite::load(ALICE_V1_SUITE)
}

/// The committed Alice fixture.
///
/// # Errors
/// When the fixture doesn't match its manifest.
pub fn alice_v1_fixture() -> Result<Fixture, String> {
    Fixture::load(ALICE_V1_FIXTURE, ALICE_V1_JOURNAL, ALICE_V1_MEMORY)
}

/// One interview answer, scored (`openagents.gym.interview_row.v1`).
///
/// The perturbation key the store enforces reads `suite_digest`,
/// `door_identity` (the answerer's model), `estimator` (the arm, as
/// `arm:NAME`), `seed_base` (the trial), and `item_id`, so the same arm
/// and answerer over the same item is one trial unless the trial differs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub schema: String,
    /// RFC 3339, UTC.
    pub recorded_at: String,
    pub suite: String,
    pub suite_digest: String,
    /// Always null: the items carry their question text.
    pub question_digest: Option<String>,
    pub fixture_digest: String,
    pub split: Partition,
    pub family: String,
    pub item_id: String,
    /// The arm that built the briefing.
    pub arm: String,
    /// `arm:NAME`, the arm as a perturbation axis.
    pub estimator: String,
    /// The answerer: `scripted`, `canned`, or `live`.
    pub door: String,
    pub door_identity: DoorIdentity,
    /// The trial, when a run names one.
    pub seed_base: Option<u64>,
    pub permutation: Option<Vec<usize>>,
    /// How many bytes the briefing held.
    pub briefing_bytes: usize,
    /// The fixture records the briefing carried.
    pub carried: Vec<String>,
    /// The item's sources the briefing carried.
    pub evidence_carried: Vec<String>,
    /// The answer, at most 2 KiB.
    pub answer: String,
    /// Null for an item code doesn't check.
    pub correct: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade: Option<Grade>,
}

/// The longest answer a row keeps, bytes.
pub const ANSWER_MAX: usize = 2048;

impl Row {
    /// The row for `answer` to `item`, graded when code checks the item.
    ///
    /// # Errors
    /// When the item has no interview state.
    #[allow(clippy::too_many_arguments)]
    pub fn graded(
        suite: &Suite,
        item: &Item,
        arm: &str,
        door: &str,
        door_identity: DoorIdentity,
        trial: Option<u64>,
        carried: Vec<String>,
        briefing_bytes: usize,
        answer: &str,
        recorded_at: String,
    ) -> Result<Self, String> {
        let state = ItemState::of(item)?;
        let checked = Category::parse(&item.family).is_some_and(Category::code_checked);
        let grade = state
            .check
            .as_ref()
            .filter(|_| checked)
            .map(|c| grade(c, answer));
        let mut kept = answer.to_string();
        if kept.len() > ANSWER_MAX {
            let mut end = ANSWER_MAX;
            while !kept.is_char_boundary(end) {
                end -= 1;
            }
            kept.truncate(end);
        }
        Ok(Self {
            schema: ROW_SCHEMA.into(),
            recorded_at,
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_digest: None,
            fixture_digest: state.fixture_digest,
            split: item.partition,
            family: item.family.clone(),
            item_id: item.id.clone(),
            arm: arm.into(),
            estimator: format!("arm:{arm}"),
            door: door.into(),
            door_identity,
            seed_base: trial,
            permutation: None,
            briefing_bytes,
            evidence_carried: state
                .sources
                .iter()
                .filter(|s| carried.contains(s))
                .cloned()
                .collect(),
            carried,
            answer: kept,
            correct: grade.as_ref().map(|g| g.correct),
            grade,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(all: &[&[&str]], none: &[&[&str]]) -> Check {
        let groups = |g: &[&[&str]]| {
            g.iter()
                .map(|terms| terms.iter().map(|t| (*t).to_string()).collect())
                .collect()
        };
        Check {
            all: groups(all),
            none: groups(none),
        }
    }

    #[test]
    fn the_scorer_needs_every_group_and_no_forbidden_term() {
        let c = check(&[&["t-101"], &["september 9", "2026-09-09"]], &[&["t-108"]]);
        assert!(grade(&c, "Task T-101,  on 2026-09-09.").correct);
        let missed = grade(&c, "Task t-101.");
        assert!(!missed.correct);
        assert_eq!(missed.missing, vec!["september 9"]);
        let listed = grade(&c, "t-101 on September 9, and t-108");
        assert!(!listed.correct);
        assert_eq!(listed.forbidden, vec!["t-108"]);
        assert!(!grade(&Check::default(), "anything").correct);
    }

    #[test]
    fn a_changed_fixture_is_refused() {
        let fixture = alice_v1_fixture().expect("the committed fixture loads");
        let edited = fixture.journal.replacen("t-101", "t-102", 1);
        let error = Fixture::load(ALICE_V1_FIXTURE, &edited, &fixture.memory).unwrap_err();
        assert!(error.contains("journal digest"), "{error}");
    }

    #[test]
    fn categories_round_trip_their_family() {
        for category in Category::ALL {
            assert_eq!(Category::parse(category.as_str()), Some(category));
        }
        assert!(Category::Memory.code_checked());
        assert!(!Category::Reflection.code_checked());
    }
}
