//! Hill-climbing studies over policy manifests.
//!
//! A study proposes candidate manifests, screens them cheapest first, keeps
//! a per-task Pareto frontier, and records everything, losers and their
//! spend included. The records follow NIP-OPT's shapes
//! (`nips/openagents/NIP-OPT.md`): a frozen study plan with its search
//! space, data partitions, objective, and confirmation allowance; one
//! candidate per manifest digest with its parents, changes, and proposer;
//! trials by phase; and a result that names the selected candidate, the
//! confirmation verdict, and the complete spend.
//!
//! The tiers, cheapest first:
//!
//! | Tier | What runs | Gate |
//! | --- | --- | --- |
//! | 0 `replay` | The component on retained evidence and fixtures | Always |
//! | 1 `mini` | Mini-task episodes with the scripted executor | `--through mini` |
//! | 2 `screen` | One Terminal-Bench trial per development task | `--allow-terminal-bench` |
//! | 3 `measure` | Three trials per development task | `--allow-terminal-bench` |
//! | 4 `confirm` | The selected candidate on held-out tasks | `--allow-terminal-bench` |
//!
//! Successive halving moves the best third of each rung to the next one,
//! and confirmation reads the held-out partition only after the selection
//! is written down.
//!
//! [`pack`] is the first study: `evidence.pack`'s parameters, scored by
//! replaying every retained briefing.

pub mod cli;
pub mod harness;
pub mod pack;
pub mod space;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::policy::Manifest;

/// The schema of a study plan.
pub const STUDY_SCHEMA: &str = "openagents.coder-one.study.v1";
/// The schema of a candidate record.
pub const CANDIDATE_SCHEMA: &str = "openagents.coder-one.study-candidate.v1";
/// The schema of a trial record.
pub const TRIAL_SCHEMA: &str = "openagents.coder-one.study-trial.v1";
/// The schema of a study result.
pub const RESULT_SCHEMA: &str = "openagents.coder-one.study-result.v1";

/// The NIP-OPT bodies each record corresponds to.
pub const NIP_OPT: &[(&str, &str)] = &[
    (STUDY_SCHEMA, "openagents.optimization-study.v1"),
    (CANDIDATE_SCHEMA, "openagents.optimization-candidate.v1"),
    (TRIAL_SCHEMA, "openagents.optimization-trial.v1"),
    (RESULT_SCHEMA, "openagents.optimization-result.v1"),
];

/// Where studies are recorded: `~/.openagents/coder-one/studies`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/coder-one/studies"))
}

/// Where a study may be retained in the checkout.
#[must_use]
pub fn retained_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/studies")
}

// ---------------------------------------------------------------------------
// Tiers
// ---------------------------------------------------------------------------

/// One evaluation tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// The component alone on retained evidence and fixtures.
    Replay,
    /// Mini-task episodes with the scripted executor.
    Mini,
    /// One Terminal-Bench trial per development task.
    Screen,
    /// Three Terminal-Bench trials per development task.
    Measure,
    /// The frozen candidate on held-out Terminal-Bench tasks.
    Confirm,
}

impl Tier {
    /// Every tier, cheapest first.
    pub const ALL: [Tier; 5] = [
        Tier::Replay,
        Tier::Mini,
        Tier::Screen,
        Tier::Measure,
        Tier::Confirm,
    ];

    #[must_use]
    pub fn number(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Tier::Replay => "replay",
            Tier::Mini => "mini",
            Tier::Screen => "screen",
            Tier::Measure => "measure",
            Tier::Confirm => "confirm",
        }
    }

    /// Parses a tier's word or number.
    ///
    /// # Errors
    ///
    /// Returns a message naming the tiers when `text` is none of them.
    pub fn parse(text: &str) -> Result<Self, String> {
        Tier::ALL
            .into_iter()
            .find(|tier| tier.word() == text || tier.number().to_string() == text)
            .ok_or_else(|| {
                format!("unknown tier {text}: use replay, mini, screen, measure, or confirm")
            })
    }

    /// What the tier runs, and what it costs per candidate.
    #[must_use]
    pub fn describe(self) -> (&'static str, &'static str) {
        match self {
            Tier::Replay => (
                "the component alone on retained evidence and fixtures, with recorded or no Jev",
                "free: local compute only",
            ),
            Tier::Mini => (
                "mini-task episodes with the scripted executor, graded",
                "free: local compute only, seconds per task",
            ),
            Tier::Screen => (
                "one Terminal-Bench trial per development task",
                "about $0.01–0.10 per trial with Luna, plus about 5 minutes",
            ),
            Tier::Measure => (
                "three Terminal-Bench trials per development task",
                "about $0.09 per task with Luna, about $1.30 with Opus",
            ),
            Tier::Confirm => (
                "the selected candidate on held-out Terminal-Bench tasks",
                "the measure tier's scale, on held-out tasks",
            ),
        }
    }

    /// The flag that must be given before the tier runs, if any.
    #[must_use]
    pub fn gate(self) -> Option<&'static str> {
        match self {
            Tier::Replay => None,
            Tier::Mini => Some("--through mini"),
            Tier::Screen | Tier::Measure | Tier::Confirm => Some("--allow-terminal-bench"),
        }
    }
}

// ---------------------------------------------------------------------------
// Candidates and the archive
// ---------------------------------------------------------------------------

/// One slot replacement: a searchable manifest field and its new value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub slot: String,
    pub value: Value,
}

/// Who proposed a candidate: the operator, and the algorithm by its real
/// name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposer {
    /// `baseline`, `grid`, `random`, `climb`, `swap`, `reflect`, or
    /// `router-refit`.
    pub operator: String,
    /// The algorithm and its configuration, such as
    /// `coordinate ascent, one step per slot`.
    pub algorithm: String,
}

/// What building a proposal produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Construction {
    /// A valid manifest; the candidate is its digest.
    Built,
    /// The manifest already exists in the archive under this digest.
    Duplicate { of: String },
    /// The proposal could not become a valid manifest.
    Refused { reason: String },
}

/// One proposal and what it became.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub schema: String,
    /// The proposal's sequence number in the study.
    pub seq: usize,
    /// Parent candidates (manifest digests); empty for the baseline.
    pub parents: Vec<String>,
    pub changes: Vec<Change>,
    pub proposer: Proposer,
    pub construction: Construction,
    /// The candidate's digest, when built or a duplicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate: Option<String>,
    /// The complete manifest, when built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<Manifest>,
    /// A short label for people: the changes in a few words.
    pub label: String,
}

/// Every proposal of a study, and the candidates they built, keyed by
/// manifest digest.
#[derive(Debug, Clone, Default)]
pub struct Archive {
    pub proposals: Vec<Proposal>,
    /// Digest to the proposal that built it.
    pub by_digest: BTreeMap<String, usize>,
}

impl Archive {
    /// Records a proposal: builds `manifest` from `base` and `changes`,
    /// validates it, and files it under its digest. Returns the candidate's
    /// digest when the proposal built one or matched an existing one.
    pub fn propose(
        &mut self,
        base: &Manifest,
        parents: Vec<String>,
        changes: Vec<Change>,
        proposer: Proposer,
    ) -> Option<String> {
        let seq = self.proposals.len();
        let label = label(&changes);
        let built = space::apply(base, &changes);
        let (construction, candidate, manifest) = match built {
            Err(reason) => (Construction::Refused { reason }, None, None),
            Ok(manifest) => {
                let digest = manifest.digest();
                if let Some(&index) = self.by_digest.get(&digest) {
                    let _ = index;
                    (
                        Construction::Duplicate { of: digest.clone() },
                        Some(digest),
                        None,
                    )
                } else {
                    self.by_digest.insert(digest.clone(), seq);
                    (Construction::Built, Some(digest), Some(manifest))
                }
            }
        };
        self.proposals.push(Proposal {
            schema: CANDIDATE_SCHEMA.to_string(),
            seq,
            parents,
            changes,
            proposer,
            construction,
            candidate: candidate.clone(),
            manifest,
            label,
        });
        candidate
    }

    /// Records a proposal that could not be built, with why.
    pub fn refuse(&mut self, changes: Vec<Change>, proposer: Proposer, reason: String) {
        let seq = self.proposals.len();
        self.proposals.push(Proposal {
            schema: CANDIDATE_SCHEMA.to_string(),
            seq,
            parents: Vec::new(),
            label: label(&changes),
            changes,
            proposer,
            construction: Construction::Refused { reason },
            candidate: None,
            manifest: None,
        });
    }

    /// The built candidate under `digest`.
    #[must_use]
    pub fn get(&self, digest: &str) -> Option<&Proposal> {
        self.by_digest.get(digest).map(|&i| &self.proposals[i])
    }

    /// Every built candidate's digest, in proposal order.
    #[must_use]
    pub fn built(&self) -> Vec<String> {
        self.proposals
            .iter()
            .filter(|p| p.construction == Construction::Built)
            .filter_map(|p| p.candidate.clone())
            .collect()
    }
}

/// The changes in a few words, such as `cap=9000 slice=800`.
#[must_use]
pub fn label(changes: &[Change]) -> String {
    if changes.is_empty() {
        return "baseline".to_string();
    }
    changes
        .iter()
        .map(|change| {
            let slot = change.slot.rsplit('.').next().unwrap_or(&change.slot);
            let value = match &change.value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            format!("{slot}={value}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Frontier
// ---------------------------------------------------------------------------

/// A candidate's place on one task: higher `quality` is better (a pass
/// rate at tiers 2 and up, the replay quality at tier 0), lower `cost` and
/// `time` are better. A missing time is not compared.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub quality: f64,
    pub cost: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<f64>,
}

impl Point {
    /// Whether `self` is at least as good as `other` on every axis and
    /// better on one.
    #[must_use]
    pub fn dominates(&self, other: &Point) -> bool {
        let times = match (self.time, other.time) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        };
        let no_worse = self.quality >= other.quality
            && self.cost <= other.cost
            && times.is_none_or(|(a, b)| a <= b);
        let better = self.quality > other.quality
            || self.cost < other.cost
            || times.is_some_and(|(a, b)| a < b);
        no_worse && better
    }
}

/// The Pareto frontier: every id no other point dominates.
#[must_use]
pub fn frontier(points: &[(String, Point)]) -> Vec<String> {
    points
        .iter()
        .filter(|(_, p)| !points.iter().any(|(_, q)| q.dominates(p)))
        .map(|(id, _)| id.clone())
        .collect()
}

/// How many of `n` candidates a successive-halving rung keeps: the best
/// `1/eta`, rounded up, and at least one.
#[must_use]
pub fn keep(n: usize, eta: usize) -> usize {
    if n == 0 {
        0
    } else {
        n.div_ceil(eta.max(1)).max(1)
    }
}

// ---------------------------------------------------------------------------
// Noise floor
// ---------------------------------------------------------------------------

/// A small seeded generator (SplitMix64), so a bootstrap reruns exactly.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform index below `n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

/// A paired difference and its bootstrap interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paired {
    /// The mean of the per-task mean differences (tasks weighted equally).
    pub mean: f64,
    /// The 2.5th and 97.5th percentiles of the task-clustered bootstrap.
    pub low: f64,
    pub high: f64,
    pub tasks: usize,
    pub cases: usize,
    pub resamples: usize,
    /// Each task's mean difference.
    pub per_task: BTreeMap<String, f64>,
}

/// Bootstraps the mean difference, resampling whole tasks and, within
/// each, its cases, so briefings of one task never stand in for another.
#[must_use]
pub fn paired(diffs: &BTreeMap<String, Vec<f64>>, resamples: usize, seed: u64) -> Paired {
    let tasks: Vec<(&String, &Vec<f64>)> = diffs.iter().filter(|(_, d)| !d.is_empty()).collect();
    let mean_of = |values: &[f64]| values.iter().sum::<f64>() / values.len().max(1) as f64;
    let per_task: BTreeMap<String, f64> = tasks
        .iter()
        .map(|(task, d)| ((*task).clone(), mean_of(d)))
        .collect();
    let mean = mean_of(&per_task.values().copied().collect::<Vec<_>>());
    let mut rng = Rng::new(seed);
    let mut means = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let mut task_means = Vec::with_capacity(tasks.len());
        for _ in 0..tasks.len() {
            let (_, d) = tasks[rng.below(tasks.len())];
            let sample: Vec<f64> = (0..d.len()).map(|_| d[rng.below(d.len())]).collect();
            task_means.push(mean_of(&sample));
        }
        means.push(mean_of(&task_means));
    }
    means.sort_by(f64::total_cmp);
    let at = |q: f64| {
        if means.is_empty() {
            mean
        } else {
            means[((means.len() - 1) as f64 * q).round() as usize]
        }
    };
    Paired {
        mean,
        low: at(0.025),
        high: at(0.975),
        tasks: tasks.len(),
        cases: tasks.iter().map(|(_, d)| d.len()).sum(),
        resamples,
        per_task,
    }
}

/// Rounds to four decimals for records that rerun byte for byte.
#[must_use]
pub fn round(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

// ---------------------------------------------------------------------------
// Spend
// ---------------------------------------------------------------------------

/// One spend entry, in NIP-OPT's usage categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spend {
    /// `proposal`, `reflection`, `student`, `judge`, `build`, `tool`,
    /// `storage`, or `cleanup`.
    pub category: String,
    pub tier: Tier,
    /// Model or Jev calls; `Some(0)` when none ran.
    pub calls: Option<u64>,
    pub wall_ms: u64,
    /// Spend in millionths of a US dollar; `None` when unknown.
    pub spend_microusd: Option<u64>,
    pub note: String,
}

/// Sums spend entries: calls, wall time, and dollars, with `complete`
/// false when any dollar amount is unknown.
#[must_use]
pub fn total(entries: &[Spend]) -> Value {
    let known = entries.iter().all(|e| e.spend_microusd.is_some());
    serde_json::json!({
        "entries": entries.len(),
        "calls": entries.iter().filter_map(|e| e.calls).sum::<u64>(),
        "wall_ms": entries.iter().map(|e| e.wall_ms).sum::<u64>(),
        "spend_usd": entries.iter().filter_map(|e| e.spend_microusd).sum::<u64>() as f64 / 1e6,
        "complete": known,
    })
}

/// Writes `value` as pretty JSON at `path`, creating its directory.
///
/// # Errors
///
/// Returns a message when the file can't be written.
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    crate::record::write_atomic(path, format!("{text}\n").as_bytes())
}

/// Writes one JSON value per line at `path`.
///
/// # Errors
///
/// Returns a message when the file can't be written.
pub fn write_lines<T: Serialize>(path: &Path, values: &[T]) -> Result<(), String> {
    let mut text = String::new();
    for value in values {
        text.push_str(&serde_json::to_string(value).map_err(|error| error.to_string())?);
        text.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    crate::record::write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(quality: f64, cost: f64) -> Point {
        Point {
            quality,
            cost,
            time: None,
        }
    }

    #[test]
    fn the_frontier_keeps_every_undominated_point() {
        let points = vec![
            ("a".to_string(), point(0.9, 10.0)),
            ("b".to_string(), point(0.8, 5.0)),
            ("c".to_string(), point(0.8, 6.0)),
            ("d".to_string(), point(0.9, 10.0)),
            ("e".to_string(), point(0.7, 5.0)),
        ];
        // c is beaten by b; e by b; a and d tie, so both stay.
        assert_eq!(frontier(&points), vec!["a", "b", "d"]);
        let timed = Point {
            quality: 0.8,
            cost: 5.0,
            time: Some(3.0),
        };
        let slower = Point {
            time: Some(4.0),
            ..timed
        };
        assert!(timed.dominates(&slower));
        assert!(!slower.dominates(&timed));
    }

    #[test]
    fn halving_keeps_the_best_third_rounded_up() {
        assert_eq!(keep(0, 3), 0);
        assert_eq!(keep(1, 3), 1);
        assert_eq!(keep(9, 3), 3);
        assert_eq!(keep(10, 3), 4);
    }

    #[test]
    fn the_bootstrap_is_seeded_and_resamples_tasks() {
        let mut diffs = BTreeMap::new();
        diffs.insert("a".to_string(), vec![0.1, 0.1, 0.1]);
        diffs.insert("b".to_string(), vec![0.2, 0.2]);
        let one = paired(&diffs, 500, 7);
        assert_eq!(one, paired(&diffs, 500, 7));
        assert!((one.mean - 0.15).abs() < 1e-9);
        assert!(one.low >= 0.1 - 1e-9 && one.high <= 0.2 + 1e-9);
        let mut mixed = diffs.clone();
        mixed.insert("c".to_string(), vec![-0.3]);
        let mixed = paired(&mixed, 500, 7);
        assert!(mixed.low < 0.0, "{mixed:?}");
    }

    #[test]
    fn tiers_parse_by_word_or_number_and_name_their_gates() {
        assert_eq!(Tier::parse("mini").unwrap(), Tier::Mini);
        assert_eq!(Tier::parse("3").unwrap(), Tier::Measure);
        assert!(Tier::parse("gepa").is_err());
        assert_eq!(Tier::Replay.gate(), None);
        assert_eq!(Tier::Confirm.gate(), Some("--allow-terminal-bench"));
    }
}
