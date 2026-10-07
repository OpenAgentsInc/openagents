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
    /// The judge's reading, for an item code doesn't check. Absent on rows
    /// written before judging existed, and on a judged item nobody judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judgment: Option<Judgment>,
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
            judgment: None,
        })
    }

    /// This row with `judgment` recorded.
    #[must_use]
    pub fn judged(mut self, judgment: Judgment) -> Self {
        self.judgment = Some(judgment);
        self
    }
}

/// The `kind` of an item a judge scores.
pub const KIND_JUDGED: &str = "judged";

/// The schema an owner's mark on one interview answer carries.
pub const MARK_SCHEMA: &str = "openagents.gym.interview_mark.v1";

/// The five-category Alice suite, as committed. It asks against the same
/// fixture as version 1.
pub const ALICE_V2_SUITE: &str = include_str!("../suites/alice-interview-v2.json");

/// The id of the gate a run of the five-category suite is judged by.
pub const GATE: &str = "interview-v1";

/// The gate's committed file, compiled in so a run needs no checkout.
pub const GATE_JSON: &str = include_str!("../gates/interview-v1.json");

/// The committed `interview-v1` gate.
///
/// # Errors
/// When the committed file doesn't validate.
pub fn gate() -> Result<crate::gate::Gate, crate::gate::GateError> {
    crate::gate::Gate::from_json(
        GATE_JSON,
        std::path::Path::new("crates/gym/gates/interview-v1.json"),
    )
}

/// The arm the gate judges.
pub const SUBJECT_ARM: &str = "full";
/// The arm it is judged against: today's briefing.
pub const BASELINE_ARM: &str = "word-overlap";

/// The committed five-category Alice suite.
///
/// # Errors
/// When the suite doesn't load.
pub fn alice_v2_suite() -> Result<Suite, SuiteError> {
    Suite::load(ALICE_V2_SUITE)
}

/// What a judge read in one answer to an item code doesn't check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Judgment {
    /// The kind of judge: `jev`, or `scripted` for the deterministic
    /// stand-in a run with no model uses.
    pub judge: String,
    /// The model that judged, as the judge reports it.
    pub identity: DoorIdentity,
    /// The question set it asked, by id.
    pub set: String,
    /// The probability the cited records support the answer.
    pub supported: f64,
    /// The probability the answer states something neither the cited
    /// records nor the briefing hold.
    pub embellished: f64,
    /// The thresholds at which each reads as yes.
    pub supported_at: f64,
    pub embellished_at: f64,
}

impl Judgment {
    /// Whether the answer reads as supported.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.supported >= self.supported_at
    }

    /// Whether the answer reads as embellished.
    #[must_use]
    pub fn is_embellished(&self) -> bool {
        self.embellished >= self.embellished_at
    }

    /// The judge as a comparison names it: `KIND:MODEL`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{}:{}", self.judge, self.identity.model)
    }
}

/// The interview rows among `rows`, read back as rows. Anything else in a
/// store is skipped.
#[must_use]
pub fn rows_of(rows: &[Value]) -> Vec<Row> {
    rows.iter()
        .filter(|row| row["schema"] == ROW_SCHEMA)
        .filter_map(|row| serde_json::from_value(row.clone()).ok())
        .collect()
}

/// One arm's counts over a set of rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub rows: usize,
    /// Rows code graded, and how many were right.
    pub checked: usize,
    pub correct: usize,
    /// Rows a judge read, how many it found supported, and how many
    /// embellished.
    pub judged: usize,
    pub supported: usize,
    pub embellished: usize,
}

impl Tally {
    /// Adds one row.
    pub fn add(&mut self, row: &Row) {
        self.rows += 1;
        if let Some(correct) = row.correct {
            self.checked += 1;
            self.correct += usize::from(correct);
        }
        if let Some(judgment) = &row.judgment {
            self.judged += 1;
            self.supported += usize::from(judgment.is_supported());
            self.embellished += usize::from(judgment.is_embellished());
        }
    }

    /// The tally of `rows`.
    #[must_use]
    pub fn of<'a>(rows: impl IntoIterator<Item = &'a Row>) -> Self {
        let mut tally = Self::default();
        for row in rows {
            tally.add(row);
        }
        tally
    }

    /// The share of checked rows that were right.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn accuracy(&self) -> Option<f64> {
        (self.checked > 0).then(|| self.correct as f64 / self.checked as f64)
    }

    /// These counts as the scores a `gym::ab` cell reports: accuracy over
    /// the code-checked rows. An answer reports no probability, so the
    /// calibration measures are absent, and no answer is wrong at a
    /// reported 0.9 or above: confident errors are zero by definition,
    /// not by omission.
    #[must_use]
    pub fn scores(&self) -> crate::gate::Scores {
        crate::gate::Scores {
            items: self.checked,
            accuracy: self.accuracy(),
            confident_errors: Some(0),
            ..crate::gate::Scores::default()
        }
    }
}

/// One arm's results over a run: the whole run and each category.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArmSummary {
    pub arm: String,
    pub blocks: Vec<u64>,
    pub total: Tally,
    /// Keyed by category family.
    pub categories: std::collections::BTreeMap<String, Tally>,
}

/// Every arm's results over `rows`, in arm order of first appearance.
#[must_use]
pub fn summarize(rows: &[Row]) -> Vec<ArmSummary> {
    let mut out: Vec<ArmSummary> = Vec::new();
    for row in rows {
        let index = if let Some(index) = out.iter().position(|s| s.arm == row.arm) {
            index
        } else {
            out.push(ArmSummary {
                arm: row.arm.clone(),
                ..ArmSummary::default()
            });
            out.len() - 1
        };
        let summary = &mut out[index];
        summary.total.add(row);
        summary
            .categories
            .entry(row.family.clone())
            .or_default()
            .add(row);
        if let Some(block) = row.seed_base
            && !summary.blocks.contains(&block)
        {
            summary.blocks.push(block);
        }
    }
    for summary in &mut out {
        summary.blocks.sort_unstable();
    }
    out
}

/// One arm's measures as the gate reads them.
#[must_use]
pub fn arm_scores(rows: &[Row], arm: &str) -> crate::gate::InterviewScores {
    let mine: Vec<&Row> = rows.iter().filter(|r| r.arm == arm).collect();
    let memory = |block: Option<u64>| {
        Tally::of(
            mine.iter()
                .copied()
                .filter(|r| r.family == Category::Memory.as_str())
                .filter(|r| block.is_none() || r.seed_base == block),
        )
    };
    let blocks: std::collections::BTreeSet<Option<u64>> =
        mine.iter().map(|r| r.seed_base).collect();
    let all = Tally::of(mine.iter().copied());
    crate::gate::InterviewScores {
        arm: arm.into(),
        recall_items: blocks
            .iter()
            .map(|block| memory(*block).checked)
            .min()
            .unwrap_or(0),
        recall: memory(None).accuracy(),
        judged: all.judged,
        embellished: all.embellished,
    }
}

/// The comparison the gate reads: [`SUBJECT_ARM`] against
/// [`BASELINE_ARM`] over `rows`, with the judge's agreement with the
/// owner's marks when there is one.
#[must_use]
pub fn comparison(
    group: &str,
    rows: &[Row],
    agreement: Option<&Agreement>,
) -> crate::gate::InterviewComparison {
    let blocks = |arm: &str| -> std::collections::BTreeSet<Option<u64>> {
        rows.iter()
            .filter(|r| r.arm == arm)
            .map(|r| r.seed_base)
            .collect()
    };
    let shared = blocks(SUBJECT_ARM)
        .intersection(&blocks(BASELINE_ARM))
        .count();
    let judges: std::collections::BTreeSet<String> = rows
        .iter()
        .filter(|r| r.arm == SUBJECT_ARM)
        .filter_map(|r| r.judgment.as_ref().map(Judgment::name))
        .collect();
    crate::gate::InterviewComparison {
        group: group.into(),
        blocks: shared,
        baseline: arm_scores(rows, BASELINE_ARM),
        subject: arm_scores(rows, SUBJECT_ARM),
        judge: if judges.is_empty() {
            "no judge".into()
        } else {
            judges.into_iter().collect::<Vec<_>>().join(", ")
        },
        judge_agreement: agreement.and_then(Agreement::rate),
        marked: agreement.map_or(0, |a| a.matched),
    }
}

/// The `gym::ab` rule an interview round runs under: accuracy on the
/// code-checked categories, on disjoint seed blocks.
///
/// Nobody has measured how recall moves between blocks with a live
/// answerer, so the accuracy floor is unmeasured and a win reads as
/// undecided rather than kept. The digested gate, `interview-v1`, is what
/// decides; this round is the comparison it rests on.
#[must_use]
pub fn ab_rule() -> crate::ab::Rule {
    use crate::ab::{MetricFloor, Pending as AbPending, Rule};
    use crate::gate::{Basis, Bound};
    let bound = |value: Option<f64>, basis: Basis, why: &str| Bound {
        value,
        basis,
        evidence: Vec::new(),
        why: why.into(),
    };
    Rule {
        id: "interview-ab-v1".into(),
        question: "Does the full architecture answer more of the code-checked interview items \
                   than today's word-overlap briefing, on seed blocks nobody has drawn?"
            .into(),
        metric_order: vec![MetricFloor {
            metric: crate::ab::Metric::Accuracy,
            block_sigma: bound(
                None,
                Basis::Unmeasured,
                "No live interview has drawn more than one block, so the spread of recall \
                 between blocks is unknown; a scripted answerer has none.",
            ),
        }],
        effect_size_sigmas: bound(
            Some(2.0),
            Basis::Convention,
            "Two standard deviations of the difference, the usual bar for a single comparison.",
        ),
        family_regression_sigmas: bound(
            Some(2.0),
            Basis::Convention,
            "The same two standard deviations: one category may not lose more than a win needs.",
        ),
        min_blocks_per_side: bound(
            Some(3.0),
            Basis::Derived,
            "The median of one block is that block and the median of two is their mean, so \
             three is the fewest at which the median can disagree with the mean.",
        ),
        requeue_limit: 1,
        covers: "Answerer resampling: the spread you would see if only the trial changed.".into(),
        does_not_cover: "Item sampling: the suite is one fixture and a few dozen items.".into(),
        pending_measurements: vec![AbPending {
            quantity: "the block-to-block spread of interview recall with a live answerer".into(),
            issue: "openagents#10794".into(),
            why: "Until it is measured, a win is undecided and only the digested gate rules."
                .into(),
        }],
    }
}

/// An owner's mark on one interview answer (`openagents.gym.interview_mark.v1`):
/// whether the cited records support it and whether it embellishes.
///
/// A mark names its row by the row's perturbation fields, so marking the
/// same answer twice is refused by the store as a repeat, and by the digest
/// of the answer it read, so a mark never moves to a different answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub schema: String,
    /// RFC 3339, UTC.
    pub recorded_at: String,
    pub suite: String,
    pub suite_digest: String,
    pub item_id: String,
    pub arm: String,
    /// `mark:ARM`, so a mark's perturbation key differs from its row's.
    pub estimator: String,
    /// The answerer of the row it marks.
    pub door_identity: DoorIdentity,
    /// The trial of the row it marks.
    pub seed_base: Option<u64>,
    /// SHA-256 of the answer it marks.
    pub answer_sha256: String,
    pub supported: bool,
    pub embellished: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl Mark {
    /// The owner's mark on `row`.
    #[must_use]
    pub fn on(row: &Row, supported: bool, embellished: bool, note: &str, at: String) -> Self {
        Self {
            schema: MARK_SCHEMA.into(),
            recorded_at: at,
            suite: row.suite.clone(),
            suite_digest: row.suite_digest.clone(),
            item_id: row.item_id.clone(),
            arm: row.arm.clone(),
            estimator: format!("mark:{}", row.arm),
            door_identity: row.door_identity.clone(),
            seed_base: row.seed_base,
            answer_sha256: sha256(row.answer.as_bytes()),
            supported,
            embellished,
            note: note.into(),
        }
    }

    /// Whether this mark reads `row`.
    #[must_use]
    pub fn marks(&self, row: &Row) -> bool {
        self.suite_digest == row.suite_digest
            && self.item_id == row.item_id
            && self.arm == row.arm
            && self.seed_base == row.seed_base
            && self.door_identity == row.door_identity
            && self.answer_sha256 == sha256(row.answer.as_bytes())
    }
}

/// The marks among `rows`, read back as marks.
#[must_use]
pub fn marks_of(rows: &[Value]) -> Vec<Mark> {
    rows.iter()
        .filter(|row| row["schema"] == MARK_SCHEMA)
        .filter_map(|row| serde_json::from_value(row.clone()).ok())
        .collect()
}

/// How the judge's readings agree with the owner's marks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agreement {
    /// Marks read.
    pub marks: usize,
    /// Marks whose row is in the store with a judgment.
    pub matched: usize,
    /// Of those, how many the judge read the same way on each question.
    pub supported_agree: usize,
    pub embellished_agree: usize,
    /// Answers the owner marked embellished and the judge didn't: the
    /// misses that would understate the embellishment rate.
    pub missed_embellishments: usize,
}

impl Agreement {
    /// The share of marked readings, over both questions, the judge got
    /// the owner's way.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn rate(&self) -> Option<f64> {
        (self.matched > 0).then(|| {
            (self.supported_agree + self.embellished_agree) as f64 / (2 * self.matched) as f64
        })
    }
}

/// The judge's agreement with `marks` over the judged rows they mark.
#[must_use]
pub fn agreement(rows: &[Row], marks: &[Mark]) -> Agreement {
    let mut out = Agreement {
        marks: marks.len(),
        ..Agreement::default()
    };
    for mark in marks {
        let Some(judgment) = rows
            .iter()
            .find(|row| mark.marks(row))
            .and_then(|row| row.judgment.as_ref())
        else {
            continue;
        };
        out.matched += 1;
        out.supported_agree += usize::from(judgment.is_supported() == mark.supported);
        out.embellished_agree += usize::from(judgment.is_embellished() == mark.embellished);
        out.missed_embellishments += usize::from(mark.embellished && !judgment.is_embellished());
    }
    out
}

/// A deterministic sample of `n` judged rows to mark, spread across items
/// and arms by the digest of each row's identity rather than taken from
/// the top of the store.
#[must_use]
pub fn sample(rows: &[Row], n: usize) -> Vec<&Row> {
    let mut judged: Vec<(String, &Row)> = rows
        .iter()
        .filter(|row| row.judgment.is_some())
        .map(|row| {
            let key = format!(
                "{}\n{}\n{}\n{:?}",
                row.suite_digest, row.item_id, row.arm, row.seed_base
            );
            (sha256(key.as_bytes()), row)
        })
        .collect();
    judged.sort_by(|a, b| a.0.cmp(&b.0));
    judged.into_iter().take(n).map(|(_, row)| row).collect()
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

    fn row(
        arm: &str,
        family: &str,
        block: u64,
        correct: Option<bool>,
        embellished: Option<bool>,
    ) -> Row {
        Row {
            schema: ROW_SCHEMA.into(),
            recorded_at: "2026-10-07T00:00:00Z".into(),
            suite: "s".into(),
            suite_digest: "d".into(),
            question_digest: None,
            fixture_digest: "f".into(),
            split: Partition::Development,
            family: family.into(),
            item_id: format!("{family}/{block}"),
            arm: arm.into(),
            estimator: format!("arm:{arm}"),
            door: "scripted".into(),
            door_identity: DoorIdentity::hosted("from-briefing-v1"),
            seed_base: Some(block),
            permutation: None,
            briefing_bytes: 0,
            carried: Vec::new(),
            evidence_carried: Vec::new(),
            answer: format!("an answer by {arm}"),
            correct,
            grade: None,
            judgment: embellished.map(|e| Judgment {
                judge: "jev".into(),
                identity: DoorIdentity::hosted("jev-test"),
                set: "openagents.interview-answer.v1".into(),
                supported: 0.9,
                embellished: if e { 0.9 } else { 0.1 },
                supported_at: 0.5,
                embellished_at: 0.5,
            }),
        }
    }

    /// `memory` code-checked rows per block per arm, with `right` of them
    /// correct, and `judged` judged rows of which `embellished` embellish.
    fn rows(arm: &str, memory: usize, right: usize, judged: usize, embellished: usize) -> Vec<Row> {
        let mut out = Vec::new();
        for block in 0..3 {
            for i in 0..memory {
                let mut r = row(arm, "memory", block, Some(i < right), None);
                r.item_id = format!("memory/{i}");
                out.push(r);
            }
            for i in 0..judged {
                let mut r = row(
                    arm,
                    "reflection",
                    block,
                    None,
                    Some(block == 0 && i < embellished),
                );
                r.item_id = format!("reflection/{i}");
                out.push(r);
            }
        }
        out
    }

    fn verdict(rows: &[Row], agreement: Option<&Agreement>) -> crate::gate::Outcome {
        gate()
            .expect("the gate loads")
            .judge_interview(&comparison("test", rows, agreement))
    }

    fn agreeing(matched: usize) -> Agreement {
        Agreement {
            marks: matched,
            matched,
            supported_agree: matched,
            embellished_agree: matched,
            missed_embellishments: 0,
        }
    }

    #[test]
    fn the_gate_passes_a_better_full_arm_only_with_a_calibrated_judge() {
        use crate::gate::Verdict;
        let mut all = rows(BASELINE_ARM, 7, 3, 10, 0);
        all.extend(rows(SUBJECT_ARM, 7, 5, 10, 0));
        let scores = comparison("test", &all, None);
        assert_eq!(scores.blocks, 3);
        assert_eq!(scores.subject.recall_items, 7);
        assert_eq!(scores.subject.judged, 30);
        assert_eq!(scores.judge, "jev:jev-test");
        let uncalibrated = verdict(&all, None);
        assert_eq!(
            uncalibrated.verdict,
            Verdict::Unverifiable,
            "{uncalibrated:?}"
        );
        let calibrated = verdict(&all, Some(&agreeing(20)));
        assert_eq!(calibrated.verdict, Verdict::Passed, "{calibrated:?}");

        // A full arm that recalls no more than word overlap fails.
        let mut worse = rows(BASELINE_ARM, 7, 5, 10, 0);
        worse.extend(rows(SUBJECT_ARM, 7, 5, 10, 0));
        assert_eq!(
            verdict(&worse, Some(&agreeing(20))).verdict,
            Verdict::Failed
        );

        // One embellished answer in 30 is over 1.3%.
        let mut loose = rows(BASELINE_ARM, 7, 3, 10, 0);
        loose.extend(rows(SUBJECT_ARM, 7, 5, 10, 1));
        let outcome = verdict(&loose, Some(&agreeing(20)));
        assert_eq!(outcome.verdict, Verdict::Failed);
        assert!(
            outcome
                .breaches()
                .any(|c| c.name == "embellishment_at_or_below_ceiling")
        );

        // One block can't be told from luck.
        let one: Vec<Row> = all
            .iter()
            .filter(|r| r.seed_base == Some(0))
            .cloned()
            .collect();
        assert_eq!(
            verdict(&one, Some(&agreeing(20))).verdict,
            Verdict::Unverifiable
        );
    }

    #[test]
    fn marks_measure_the_judge_on_the_answers_they_read() {
        let all = rows(SUBJECT_ARM, 1, 1, 2, 1);
        let judged: Vec<&Row> = all.iter().filter(|r| r.judgment.is_some()).collect();
        let marks = vec![
            // Agrees on both.
            Mark::on(judged[0], true, true, "", "t".into()),
            // The judge missed an embellishment.
            Mark::on(judged[1], true, true, "", "t".into()),
        ];
        let measured = agreement(&all, &marks);
        assert_eq!(measured.matched, 2);
        assert_eq!(measured.supported_agree, 2);
        assert_eq!(measured.embellished_agree, 1);
        assert_eq!(measured.missed_embellishments, 1);
        assert_eq!(measured.rate(), Some(0.75));
        // A mark never moves to a different answer.
        let mut changed = all.clone();
        for r in &mut changed {
            r.answer.push_str(" (edited)");
        }
        assert_eq!(agreement(&changed, &marks).matched, 0);
        assert_eq!(sample(&all, 3).len(), 3);
        assert_eq!(sample(&all, 100).len(), judged.len());
    }
}
