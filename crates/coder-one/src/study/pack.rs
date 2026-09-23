//! The first study: `evidence.pack`'s parameters, scored by replaying every
//! retained briefing.
//!
//! Each retained episode that delegated is one case: its briefing's
//! inputs, read back from the retained text and restored from the
//! episode's survey, and the requirement map the rule extracts from the
//! task text. A candidate packs every case under its manifest's `brief`
//! policy, and the objective reads four replay metrics:
//!
//! - **Selected delivered**: the share of Jev-selected items (relevance at
//!   or above 0.5) that reached the briefing.
//! - **Label coverage**: the share of the task's hand-authored requirement
//!   labels (#9546; `labeled--<task>` fixtures, context labels left out)
//!   stated by a requirement some delivered item informs.
//! - **Duplicate bytes**: listing lines a delivered listing repeats.
//! - **Briefing size**: characters, as the cost proxy.
//!
//! J = ½·selected delivered + ½·label coverage − duplicate bytes / 12,000
//! − λ·characters / 12,000, with λ = 0.1. Tasks weigh equally: a set's J
//! is the mean of its tasks' mean J.
//!
//! The split is by task: three of the eight retained tasks, chosen by the
//! SHA-256 of the seed and the task name, are held out for confirmation.
//! The development tasks' briefings alternate between the search and the
//! selection partitions.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::space::{self, Slot};
use super::{
    Archive, Change, Paired, Point, Proposer, RESULT_SCHEMA, STUDY_SCHEMA, Spend, TRIAL_SCHEMA,
    Tier, frontier, keep, paired, round, total, write_json, write_lines,
};
use crate::delegate::{Briefing, BriefingInputs};
use crate::pack::{Params, delivered_by_pack, delivered_by_sections, measure, pack};
use crate::policy::{Manifest, Packer};
use crate::requirements::{Kind, Label, RequirementMap};

/// The component this study tunes.
pub const COMPONENT: &str = "evidence.pack";

/// The objective's price of briefing size, per 12,000 characters.
pub const LAMBDA_SIZE: f64 = 0.1;
/// The objective's price of repeated listing bytes, per 12,000.
pub const DUPLICATE_WEIGHT: f64 = 1.0;
/// The characters the size and duplicate terms are measured in.
pub const REFERENCE_CHARS: f64 = 12_000.0;
/// The smallest held-out gain in J that counts, beyond the interval.
pub const EPSILON: f64 = 0.01;
/// The successive-halving rate: each rung keeps the best third.
pub const ETA: usize = 3;
/// Held-out tasks.
pub const HELD_OUT_TASKS: usize = 3;
/// Bootstrap resamples for the noise floor.
pub const RESAMPLES: usize = 2_000;

/// The objective, in words, as the study plan records it.
pub const OBJECTIVE: &str = "J = 0.5 * selected_delivered_rate + 0.5 * label_coverage \
- 1.0 * duplicate_bytes / 12000 - 0.1 * chars / 12000, per retained briefing; a set's J \
is the mean over its tasks of each task's mean J";

/// The acceptance rule, in words.
pub const ACCEPTANCE: &str = "The selected candidate beats the baseline when, on the \
held-out tasks' briefings, the mean paired difference in J is at least 0.01 and the \
2.5th percentile of a task-clustered bootstrap (2,000 resamples) is above zero";

// ---------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------

/// One retained briefing, ready to pack under any candidate.
#[derive(Debug, Clone)]
pub struct Case {
    /// `<trace>/<episode>`.
    pub id: String,
    pub task: String,
    /// The first packer's inputs, omitted items clipped as it clipped them.
    pub first: BriefingInputs,
    /// The inputs with every surveyed item whole.
    pub whole: BriefingInputs,
    pub map: RequirementMap,
    /// Each item's requirement ids, by item id.
    pub informs: BTreeMap<String, Vec<String>>,
    /// The task's binding labels, and the requirements that state each.
    pub labels: Vec<(Label, Vec<String>)>,
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The hand-authored labels for each task, from the `labeled--<task>`
/// fixtures under `fixtures`.
#[must_use]
pub fn labels(fixtures: &Path) -> BTreeMap<String, Vec<Label>> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(fixtures) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(task) = name.strip_prefix("labeled--") else {
            continue;
        };
        let Some(value) = read_json(&entry.path().join("task.requirements.json")) else {
            continue;
        };
        let labels: Vec<Label> = value
            .pointer("/retained/labels")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        out.insert(task.to_string(), labels);
    }
    out
}

/// Reads one retained episode into a case.
///
/// # Errors
///
/// Returns why the episode has no replayable briefing.
pub fn load_case(dir: &Path, labels: &BTreeMap<String, Vec<Label>>) -> Result<Case, String> {
    let manifest = read_json(&dir.join("manifest.json")).ok_or("no manifest")?;
    let record = manifest
        .pointer("/delegate/delegation/briefing")
        .ok_or("no delegation")?;
    let strings = |key: &str| -> Vec<String> {
        record[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    };
    let included = strings("included");
    let omitted = strings("omitted");
    let text = std::fs::read_to_string(dir.join("artifacts/delegate-1.briefing.md"))
        .map_err(|_| "no retained briefing text")?;
    let state = read_json(&dir.join("artifacts/state.json")).ok_or("no state.json")?;
    let parsed = crate::component::pack::parse(&text, &included, &omitted)?;
    let (first, whole, _, _) = crate::component::replay::restore(&parsed, &omitted, &state);
    let map = crate::requirements::mechanical(&whole.instruction);
    // Which requirements each item informs doesn't depend on the cap, so
    // one unbounded pack reads it off.
    let open = pack(
        &whole,
        &map,
        None,
        Params {
            cap: usize::MAX / 4,
            ..Params::default()
        },
    );
    let informs = open
        .record
        .items
        .iter()
        .map(|item| (item.id.clone(), item.informs.clone()))
        .collect();
    let episode = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let trace = dir
        .parent()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let task = episode.split("__").next().unwrap_or(&episode).to_string();
    let labels = labels
        .get(&task)
        .into_iter()
        .flatten()
        .filter(|label| label.kind != Kind::Context)
        .map(|label| {
            let stating = map
                .requirements
                .iter()
                .filter(|r| squash(&r.text).contains(&squash(&label.anchor)))
                .map(|r| r.id.clone())
                .collect();
            (label.clone(), stating)
        })
        .collect();
    Ok(Case {
        id: format!("{trace}/{episode}"),
        task,
        first,
        whole,
        map,
        informs,
        labels,
    })
}

/// Every retained briefing under `traces`, and the ones that couldn't be
/// read back, with why.
#[must_use]
pub fn load_cases(traces: &Path, fixtures: &Path) -> (Vec<Case>, Vec<(String, String)>) {
    let labels = labels(fixtures);
    let mut cases = Vec::new();
    let mut skipped = Vec::new();
    for dir in crate::component::replay::episodes(traces) {
        let delegated = read_json(&dir.join("manifest.json"))
            .is_some_and(|m| m.pointer("/delegate/delegation/briefing").is_some());
        if !delegated {
            continue;
        }
        match load_case(&dir, &labels) {
            Ok(case) => cases.push(case),
            Err(error) => skipped.push((
                dir.strip_prefix(traces)
                    .unwrap_or(&dir)
                    .display()
                    .to_string(),
                error,
            )),
        }
    }
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    (cases, skipped)
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// One case packed under one candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scored {
    pub selected: usize,
    pub selected_delivered: usize,
    pub labels: usize,
    pub labels_covered: usize,
    /// Labels some item informs at all, delivered or not: the ceiling.
    pub labels_coverable: usize,
    pub duplicate_bytes: usize,
    pub chars: usize,
    pub omitted: usize,
    pub j: f64,
    pub quality: f64,
}

/// Packs `case` under `brief` and scores it.
#[must_use]
pub fn score(case: &Case, brief: &crate::policy::BriefPolicy) -> Scored {
    let (m, delivered) = match brief.packer {
        Packer::Sections => {
            let briefing = Briefing::build(&case.first, brief.cap);
            let delivered = delivered_by_sections(&case.first, &briefing);
            (measure(&briefing, &delivered), delivered)
        }
        // Replay has no recorded coverage judgments for most briefings, so
        // `coverage-jev` packs as `coverage` here.
        Packer::Coverage | Packer::CoverageJev => {
            let packed = pack(&case.whole, &case.map, None, brief.pack_params());
            let delivered = delivered_by_pack(&case.whole, &packed);
            (measure(&packed.briefing, &delivered), delivered)
        }
    };
    let covered: BTreeSet<&String> = delivered
        .iter()
        .filter(|(_, text)| text.is_some())
        .filter_map(|(item, _)| case.informs.get(&item.id))
        .flatten()
        .collect();
    let informed: BTreeSet<&String> = case.informs.values().flatten().collect();
    let labels = case.labels.len();
    let labels_covered = case
        .labels
        .iter()
        .filter(|(_, stating)| stating.iter().any(|r| covered.contains(r)))
        .count();
    let labels_coverable = case
        .labels
        .iter()
        .filter(|(_, stating)| stating.iter().any(|r| informed.contains(r)))
        .count();
    let rate = |n: usize, d: usize| if d == 0 { 1.0 } else { n as f64 / d as f64 };
    let quality = 0.5 * rate(m.selected_delivered, m.selected) + 0.5 * rate(labels_covered, labels);
    let j = quality
        - DUPLICATE_WEIGHT * m.duplicate_bytes as f64 / REFERENCE_CHARS
        - LAMBDA_SIZE * m.chars as f64 / REFERENCE_CHARS;
    Scored {
        selected: m.selected,
        selected_delivered: m.selected_delivered,
        labels,
        labels_covered,
        labels_coverable,
        duplicate_bytes: m.duplicate_bytes,
        chars: m.chars,
        omitted: m.omitted,
        j,
        quality,
    }
}

/// A candidate's scores over a set of cases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aggregate {
    pub cases: usize,
    pub tasks: usize,
    /// The mean over tasks of each task's mean J.
    pub j: f64,
    pub quality: f64,
    pub selected_delivered_rate: f64,
    pub label_coverage: f64,
    pub label_coverable: f64,
    pub duplicate_bytes: usize,
    pub chars_mean: f64,
    pub omitted: usize,
    pub per_task: BTreeMap<String, TaskAggregate>,
}

/// One task's share of an aggregate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskAggregate {
    pub cases: usize,
    pub j: f64,
    pub quality: f64,
    pub chars_mean: f64,
    pub label_coverage: f64,
}

/// Aggregates `scored` (case, score) pairs.
#[must_use]
pub fn aggregate(scored: &[(&Case, Scored)]) -> Aggregate {
    let mut by_task: BTreeMap<String, Vec<&Scored>> = BTreeMap::new();
    for (case, s) in scored {
        by_task.entry(case.task.clone()).or_default().push(s);
    }
    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len().max(1) as f64;
    let ratio = |n: usize, d: usize| if d == 0 { 1.0 } else { n as f64 / d as f64 };
    let per_task: BTreeMap<String, TaskAggregate> = by_task
        .iter()
        .map(|(task, s)| {
            let labels: usize = s.iter().map(|x| x.labels).sum();
            (
                task.clone(),
                TaskAggregate {
                    cases: s.len(),
                    j: round(mean(&s.iter().map(|x| x.j).collect::<Vec<_>>())),
                    quality: round(mean(&s.iter().map(|x| x.quality).collect::<Vec<_>>())),
                    chars_mean: round(mean(&s.iter().map(|x| x.chars as f64).collect::<Vec<_>>())),
                    label_coverage: round(ratio(s.iter().map(|x| x.labels_covered).sum(), labels)),
                },
            )
        })
        .collect();
    let all: Vec<&Scored> = scored.iter().map(|(_, s)| s).collect();
    let labels: usize = all.iter().map(|x| x.labels).sum();
    Aggregate {
        cases: all.len(),
        tasks: per_task.len(),
        j: round(mean(&per_task.values().map(|t| t.j).collect::<Vec<_>>())),
        quality: round(mean(
            &per_task.values().map(|t| t.quality).collect::<Vec<_>>(),
        )),
        selected_delivered_rate: round(ratio(
            all.iter().map(|x| x.selected_delivered).sum(),
            all.iter().map(|x| x.selected).sum(),
        )),
        label_coverage: round(ratio(all.iter().map(|x| x.labels_covered).sum(), labels)),
        label_coverable: round(ratio(all.iter().map(|x| x.labels_coverable).sum(), labels)),
        duplicate_bytes: all.iter().map(|x| x.duplicate_bytes).sum(),
        chars_mean: round(mean(
            &all.iter().map(|x| x.chars as f64).collect::<Vec<_>>(),
        )),
        omitted: all.iter().map(|x| x.omitted).sum(),
        per_task,
    }
}

// ---------------------------------------------------------------------------
// The split
// ---------------------------------------------------------------------------

/// The partitions, by case id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Split {
    pub seed: u64,
    pub development_tasks: Vec<String>,
    pub held_out_tasks: Vec<String>,
    pub search: Vec<String>,
    pub selection: Vec<String>,
    pub confirmation: Vec<String>,
    /// Cases with no labels for their task, left out of every partition.
    pub excluded: Vec<String>,
}

/// Splits `cases` by task: the `HELD_OUT_TASKS` tasks whose
/// `sha256("<seed>:<task>")` sorts first are held out; each development
/// task's cases, in id order, alternate between search and selection.
#[must_use]
pub fn split(cases: &[Case], seed: u64) -> Split {
    let labeled: BTreeSet<&str> = cases
        .iter()
        .filter(|c| !c.labels.is_empty())
        .map(|c| c.task.as_str())
        .collect();
    let mut tasks: Vec<(String, &str)> = labeled
        .iter()
        .map(|task| {
            let digest = Sha256::digest(format!("{seed}:{task}").as_bytes());
            (
                digest
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                *task,
            )
        })
        .collect();
    tasks.sort();
    let held: BTreeSet<&str> = tasks
        .iter()
        .take(HELD_OUT_TASKS.min(tasks.len().saturating_sub(1)))
        .map(|(_, t)| *t)
        .collect();
    let mut out = Split {
        seed,
        development_tasks: labeled
            .iter()
            .filter(|t| !held.contains(*t))
            .map(|t| (*t).to_string())
            .collect(),
        held_out_tasks: held.iter().map(|t| (*t).to_string()).collect(),
        search: Vec::new(),
        selection: Vec::new(),
        confirmation: Vec::new(),
        excluded: Vec::new(),
    };
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for case in cases {
        if !labeled.contains(case.task.as_str()) {
            out.excluded.push(case.id.clone());
        } else if held.contains(case.task.as_str()) {
            out.confirmation.push(case.id.clone());
        } else {
            let n = seen.entry(case.task.as_str()).or_default();
            if n.is_multiple_of(2) {
                out.search.push(case.id.clone());
            } else {
                out.selection.push(case.id.clone());
            }
            *n += 1;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The study
// ---------------------------------------------------------------------------

/// How to run the study.
#[derive(Debug, Clone)]
pub struct Options {
    pub traces: PathBuf,
    pub fixtures: PathBuf,
    /// Where studies are recorded; the study writes `<out>/<study id>/`.
    pub out: PathBuf,
    /// Also copy the plan, candidates, and result here.
    pub retain: Option<PathBuf>,
    pub seed: u64,
    /// The highest tier to run.
    pub through: Tier,
    /// Whether Terminal-Bench tiers may run.
    pub terminal_bench: Option<super::harness::TerminalBench>,
    /// The operators to run, in order.
    pub operators: Vec<String>,
    /// A reflective edit file for the `reflect` operator.
    pub reflection: Option<PathBuf>,
    /// The search space; `None` uses [`default_space`].
    pub space: Option<Vec<Slot>>,
    /// The climb's move limit.
    pub climb_moves: usize,
    /// Random draws for the `random` operator.
    pub random_draws: usize,
}

impl Options {
    /// The checked-in traces and fixtures, recorded under `out`.
    #[must_use]
    pub fn new(out: PathBuf) -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        Options {
            traces: root.join("../../bench/terminal-bench/traces"),
            fixtures: root.join("fixtures/components"),
            out,
            retain: None,
            seed: 9557,
            through: Tier::Replay,
            terminal_bench: None,
            operators: vec!["swap".into(), "grid".into(), "climb".into()],
            reflection: None,
            space: None,
            climb_moves: 12,
            random_draws: 24,
        }
    }
}

/// The study's baseline: `pack-luna.json` with the coverage packer at its
/// hand-authored defaults. Replay can't reproduce `coverage-jev` without
/// recorded coverage judgments for every briefing, so the baseline names
/// `coverage`.
///
/// # Errors
///
/// Returns a message when the reference manifest is missing or invalid.
pub fn baseline() -> Result<Manifest, String> {
    let path = crate::policy::reference_dir().join("pack-luna.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut manifest = Manifest::parse(&text)?;
    manifest.name = Some("evidence.pack study baseline".to_string());
    manifest.note =
        Some("pack-luna.json with the coverage packer at its hand-authored defaults".to_string());
    manifest.policy.brief.packer = Packer::Coverage;
    manifest.search = vec![
        "policy.brief.cap".to_string(),
        "policy.brief.pack.slice".to_string(),
        "policy.brief.pack.item_max".to_string(),
        "policy.brief.pack.instruction_share".to_string(),
        "policy.brief.packer".to_string(),
    ];
    manifest.validate()?;
    Ok(manifest)
}

/// The budget, reserve, and span parameters, with grid values around the
/// defaults and climb steps.
#[must_use]
pub fn default_space() -> Vec<Slot> {
    vec![
        Slot::numeric(
            "policy.brief.cap",
            &[8_000.0, 10_000.0, 12_000.0, 16_000.0],
            1_000.0,
            4_000.0,
            20_000.0,
        ),
        Slot::numeric(
            "policy.brief.pack.slice",
            &[400.0, 800.0, 1_200.0, 2_000.0],
            200.0,
            200.0,
            4_000.0,
        ),
        Slot::numeric(
            "policy.brief.pack.item_max",
            &[2_000.0, 4_000.0, 8_000.0, 12_000.0],
            1_000.0,
            2_000.0,
            16_000.0,
        ),
        Slot::numeric(
            "policy.brief.pack.instruction_share",
            &[0.3, 0.5, 0.7],
            0.1,
            0.1,
            0.9,
        ),
    ]
}

/// The component swap: the first packer in place of the coverage packer.
#[must_use]
pub fn swap_slot() -> Slot {
    Slot::choice("policy.brief.packer", "composition", &["sections"])
}

/// One case's score: its id, its task, and the score.
type Row = (String, String, Scored);

/// Evaluations, cached by (candidate, partition).
struct Evaluator<'a> {
    cases: &'a [Case],
    index: BTreeMap<&'a str, usize>,
    cache: BTreeMap<(String, String), (Aggregate, Vec<Row>)>,
    wall_ms: BTreeMap<String, u64>,
}

impl<'a> Evaluator<'a> {
    fn new(cases: &'a [Case]) -> Self {
        Evaluator {
            index: cases
                .iter()
                .enumerate()
                .map(|(i, c)| (c.id.as_str(), i))
                .collect(),
            cases,
            cache: BTreeMap::new(),
            wall_ms: BTreeMap::new(),
        }
    }

    fn run(&mut self, digest: &str, manifest: &Manifest, phase: &str, ids: &[String]) -> Aggregate {
        let key = (digest.to_string(), phase.to_string());
        if let Some((aggregate, _)) = self.cache.get(&key) {
            return aggregate.clone();
        }
        let started = Instant::now();
        let mut scored = Vec::new();
        let mut rows = Vec::new();
        for id in ids {
            let Some(&i) = self.index.get(id.as_str()) else {
                continue;
            };
            let case = &self.cases[i];
            let s = score(case, &manifest.policy.brief);
            rows.push((case.id.clone(), case.task.clone(), s.clone()));
            scored.push((case, s));
        }
        let us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        *self.wall_ms.entry(digest.to_string()).or_default() += us / 1_000;
        let aggregate = aggregate(&scored);
        self.cache.insert(key, (aggregate.clone(), rows));
        aggregate
    }

    fn rows(&self, digest: &str, phase: &str) -> Option<&Vec<Row>> {
        self.cache
            .get(&(digest.to_string(), phase.to_string()))
            .map(|(_, rows)| rows)
    }
}

/// The plan's digest: SHA-256 of its canonical JSON.
fn plan_digest(plan: &Value) -> String {
    crate::policy::digest_value(plan)
}

/// What a finished study reports.
#[derive(Debug, Clone)]
pub struct Finished {
    pub dir: PathBuf,
    pub result: Value,
}

/// Runs the study and records it under `options.out`.
///
/// # Errors
///
/// Returns a message when the baseline, the space, or the records can't
/// be built or written. A study with no winner is a finished study.
#[allow(clippy::too_many_lines)]
pub async fn run(options: &Options) -> Result<Finished, String> {
    let started = Instant::now();
    let mut spend: Vec<Spend> = Vec::new();
    let loading = Instant::now();
    let (cases, skipped) = load_cases(&options.traces, &options.fixtures);
    if cases.is_empty() {
        return Err(format!(
            "no replayable briefings under {}",
            options.traces.display()
        ));
    }
    let split = split(&cases, options.seed);
    let base = baseline()?;
    let baseline_digest = base.digest();
    let slots = options.space.clone().unwrap_or_else(default_space);
    space::validate(&slots)?;
    let swap = swap_slot();
    spend.push(Spend {
        category: "build".to_string(),
        tier: Tier::Replay,
        calls: Some(0),
        wall_ms: millis(loading),
        spend_microusd: Some(0),
        note: format!("read {} retained briefings back into cases", cases.len()),
    });

    let tiers: Vec<Value> = Tier::ALL
        .iter()
        .map(|tier| {
            let (what, cost) = tier.describe();
            json!({
                "tier": tier.word(),
                "number": tier.number(),
                "runs": what,
                "cost": cost,
                "gate": tier.gate(),
                "enabled": *tier <= options.through
                    && (*tier < Tier::Screen || options.terminal_bench.is_some()),
            })
        })
        .collect();
    let plan = json!({
        "schema": STUDY_SCHEMA,
        "nip_opt": "openagents.optimization-study.v1",
        "component": COMPONENT,
        "owner": "local",
        "signature": "evidence.pack: the briefing within its cap, packed from the episode's evidence, every omission named",
        "baseline": { "digest": baseline_digest, "manifest": base },
        "space": {
            "slots": slots,
            "swap": swap,
            "validator": "coder_one::policy::Manifest::validate, slots limited to coder_one::policy::SEARCHABLE",
        },
        "data": {
            "suite": "retained Terminal-Bench briefings under bench/terminal-bench/traces, labels from crates/coder-one/fixtures/components/labeled--*",
            "leakage_unit": "task",
            "seed": split.seed,
            "development_tasks": split.development_tasks,
            "held_out_tasks": split.held_out_tasks,
            "search": split.search.len(),
            "selection": split.selection.len(),
            "confirmation": split.confirmation.len(),
            "excluded": split.excluded.len(),
            "exposure": "The labels were written from the public instructions of all eight tasks before this study; the held-out tasks are held out from the search, not from label authoring.",
        },
        "objective": OBJECTIVE,
        "acceptance": ACCEPTANCE,
        "optimizer": {
            "operators": options.operators,
            "halving": { "eta": ETA },
            "climb_moves": options.climb_moves,
            "random_draws": options.random_draws,
            "seed": options.seed,
            "nondeterminism": "none in the scores; pack wall time is measured and recorded but never ranked",
        },
        "tiers": tiers,
        "confirmation": { "max_candidates": 1, "max_attempts": 1, "policy": ACCEPTANCE },
        "bounds": { "spend_usd": 0.0, "note": "tiers 0 and 1 make no model or Jev call" },
    });
    let digest = plan_digest(&plan);
    let id = format!("evidence-pack-{}", &digest[..12]);
    let dir = options.out.join(&id);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    let mut plan = plan;
    plan["study"] = json!(id);
    plan["digest"] = json!(digest);
    write_json(&dir.join("study.json"), &plan)?;

    // Proposals.
    let proposing = Instant::now();
    let mut archive = Archive::default();
    archive.propose(
        &base,
        Vec::new(),
        Vec::new(),
        Proposer {
            operator: "baseline".to_string(),
            algorithm: "hand-authored: the coverage packer's defaults".to_string(),
        },
    );
    let mut evaluator = Evaluator::new(&cases);
    let mut climb_record = None;
    for operator in &options.operators {
        match operator.as_str() {
            "swap" => {
                for value in &swap.values {
                    archive.propose(
                        &base,
                        vec![baseline_digest.clone()],
                        vec![Change {
                            slot: swap.id.clone(),
                            value: value.clone(),
                        }],
                        Proposer {
                            operator: "swap".to_string(),
                            algorithm: "component swap, everything else fixed".to_string(),
                        },
                    );
                }
            }
            "grid" => {
                let proposer = Proposer {
                    operator: "grid".to_string(),
                    algorithm: format!("grid search: every combination of {} slots", slots.len()),
                };
                for changes in space::grid(&slots) {
                    archive.propose(
                        &base,
                        vec![baseline_digest.clone()],
                        changes,
                        proposer.clone(),
                    );
                }
            }
            "random" => {
                let proposer = Proposer {
                    operator: "random".to_string(),
                    algorithm: format!(
                        "random search: {} uniform draws, seed {}",
                        options.random_draws, options.seed
                    ),
                };
                for changes in space::random(&slots, options.random_draws, options.seed) {
                    archive.propose(
                        &base,
                        vec![baseline_digest.clone()],
                        changes,
                        proposer.clone(),
                    );
                }
            }
            "climb" => {
                let search = split.search.clone();
                let mut score = |digest: &str, manifest: &Manifest| {
                    Some(evaluator.run(digest, manifest, "search", &search).j)
                };
                climb_record = Some(space::climb(
                    &mut archive,
                    &base,
                    &baseline_digest,
                    &slots,
                    options.climb_moves,
                    1e-4,
                    &mut score,
                ));
            }
            "reflect" => {
                let path = options
                    .reflection
                    .as_ref()
                    .ok_or("the reflect operator needs --reflection FILE")?;
                space::reflect(&mut archive, &base, &baseline_digest, path)?;
            }
            "router-refit" => space::router_refit(
                &mut archive,
                json!({ "source": "gym coder router", "note": "not fitted in this study" }),
            ),
            other => return Err(format!("unknown operator {other}")),
        }
    }
    spend.push(Spend {
        category: "proposal".to_string(),
        tier: Tier::Replay,
        calls: Some(0),
        wall_ms: millis(proposing),
        spend_microusd: Some(0),
        note: format!(
            "{} proposals, {} candidates; the climb's search evaluations are counted here",
            archive.proposals.len(),
            archive.built().len()
        ),
    });

    // Rung 0: every candidate on the search partition.
    let screening = Instant::now();
    let built = archive.built();
    let manifest_of = |digest: &str| -> Manifest {
        archive
            .get(digest)
            .and_then(|p| p.manifest.clone())
            .expect("a built candidate has a manifest")
    };
    let mut search: Vec<(String, Aggregate)> = built
        .iter()
        .map(|d| {
            (
                d.clone(),
                evaluator.run(d, &manifest_of(d), "search", &split.search),
            )
        })
        .collect();
    search.sort_by(|a, b| b.1.j.total_cmp(&a.1.j).then(a.0.cmp(&b.0)));
    let rung0_keep = keep(search.len(), ETA);
    let survivors: Vec<String> = search
        .iter()
        .take(rung0_keep)
        .map(|(d, _)| d.clone())
        .collect();
    spend.push(Spend {
        category: "tool".to_string(),
        tier: Tier::Replay,
        calls: Some(0),
        wall_ms: millis(screening),
        spend_microusd: Some(0),
        note: format!(
            "search rung: {} candidates × {} briefings",
            search.len(),
            split.search.len()
        ),
    });

    // Per-task frontiers over the search rung: replay quality against mean
    // briefing characters.
    let mut frontiers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for task in &split.development_tasks {
        let points: Vec<(String, Point)> = search
            .iter()
            .filter_map(|(d, a)| {
                a.per_task.get(task).map(|t| {
                    (
                        d.clone(),
                        Point {
                            quality: t.quality,
                            cost: t.chars_mean,
                            time: None,
                        },
                    )
                })
            })
            .collect();
        frontiers.insert(task.clone(), frontier(&points));
    }

    // Rung 1: survivors, and the baseline for reference, on selection.
    let selecting = Instant::now();
    let mut on_selection: Vec<String> = survivors.clone();
    if !on_selection.contains(&baseline_digest) {
        on_selection.push(baseline_digest.clone());
    }
    let mut selection: Vec<(String, Aggregate)> = on_selection
        .iter()
        .map(|d| {
            (
                d.clone(),
                evaluator.run(d, &manifest_of(d), "selection", &split.selection),
            )
        })
        .collect();
    selection.sort_by(|a, b| b.1.j.total_cmp(&a.1.j).then(a.0.cmp(&b.0)));
    let ranked_survivors: Vec<&(String, Aggregate)> = selection
        .iter()
        .filter(|(d, _)| survivors.contains(d))
        .collect();
    let rung1_keep = keep(ranked_survivors.len(), ETA);
    let promoted: Vec<String> = ranked_survivors
        .iter()
        .take(rung1_keep)
        .map(|(d, _)| d.clone())
        .collect();
    spend.push(Spend {
        category: "tool".to_string(),
        tier: Tier::Replay,
        calls: Some(0),
        wall_ms: millis(selecting),
        spend_microusd: Some(0),
        note: format!(
            "selection rung: {} candidates × {} briefings",
            selection.len(),
            split.selection.len()
        ),
    });

    // Tier 1: mini-task episodes with the scripted executor.
    let mut mini: BTreeMap<String, super::harness::MiniScreen> = BTreeMap::new();
    let mut finalists = promoted.clone();
    if options.through >= Tier::Mini {
        let screening = Instant::now();
        let mut screened = promoted.clone();
        if !screened.contains(&baseline_digest) {
            screened.push(baseline_digest.clone());
        }
        for digest in &screened {
            let manifest = manifest_of(digest);
            let result = super::harness::mini(
                &manifest.policy.brief,
                &dir.join("mini").join(&digest[..12]),
            )
            .await;
            mini.insert(digest.clone(), result);
        }
        finalists.retain(|d| mini.get(d).is_some_and(|m| m.passed));
        spend.push(Spend {
            category: "tool".to_string(),
            tier: Tier::Mini,
            calls: Some(0),
            wall_ms: millis(screening),
            spend_microusd: Some(0),
            note: format!(
                "{} candidates × {} mini-tasks, scripted executor, Jev off",
                screened.len(),
                crate::minitask::CATALOG.len()
            ),
        });
    }

    // Selection, written down before the held-out partition is read.
    let selection_j = |d: &str| {
        selection
            .iter()
            .find(|(x, _)| x == d)
            .map(|(_, a)| a.j)
            .unwrap_or(f64::NEG_INFINITY)
    };
    let baseline_selection_j = selection_j(&baseline_digest);
    let selected = finalists
        .iter()
        .max_by(|a, b| selection_j(a).total_cmp(&selection_j(b)).then(b.cmp(a)))
        .filter(|d| selection_j(d) > baseline_selection_j)
        .cloned();
    let committed = json!({
        "study": id,
        "selected": selected,
        "selection_j": selected.as_deref().map(selection_j),
        "baseline_selection_j": baseline_selection_j,
        "committed_before_confirmation": true,
        "note": if selected.is_some() {
            "the best finalist on the selection partition"
        } else {
            "no finalist beat the baseline on the selection partition, so nothing goes to confirmation"
        },
    });
    write_json(&dir.join("selection.json"), &committed)?;

    // Confirmation: the selected candidate and the baseline on held-out.
    let confirming = Instant::now();
    let confirmation = if let Some(winner) = &selected {
        let winner_agg = evaluator.run(
            winner,
            &manifest_of(winner),
            "confirmation",
            &split.confirmation,
        );
        let base_agg = evaluator.run(
            &baseline_digest,
            &manifest_of(&baseline_digest),
            "confirmation",
            &split.confirmation,
        );
        let winner_rows = evaluator
            .rows(winner, "confirmation")
            .cloned()
            .unwrap_or_default();
        let base_rows = evaluator
            .rows(&baseline_digest, "confirmation")
            .cloned()
            .unwrap_or_default();
        let mut diffs: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for ((id_a, task, a), (id_b, _, b)) in winner_rows.iter().zip(&base_rows) {
            debug_assert_eq!(id_a, id_b);
            diffs.entry(task.clone()).or_default().push(a.j - b.j);
        }
        let floor = paired(&diffs, RESAMPLES, options.seed);
        let beats = floor.mean >= EPSILON && floor.low > 0.0;
        Some(confirmation_record(
            winner,
            &winner_agg,
            &base_agg,
            &floor,
            beats,
        ))
    } else {
        None
    };
    spend.push(Spend {
        category: "tool".to_string(),
        tier: Tier::Replay,
        calls: Some(0),
        wall_ms: millis(confirming),
        spend_microusd: Some(0),
        note: format!(
            "confirmation: {} held-out briefings, spent once",
            split.confirmation.len()
        ),
    });

    // Tiers 2 to 4: planned, and run only when allowed.
    let mut higher = Vec::new();
    for tier in [Tier::Screen, Tier::Measure, Tier::Confirm] {
        let candidates: Vec<String> = match tier {
            Tier::Confirm => selected.iter().cloned().collect(),
            _ => finalists.clone(),
        };
        let manifests: Vec<(String, Manifest)> = candidates
            .iter()
            .map(|d| (d.clone(), manifest_of(d)))
            .collect();
        let record = super::harness::terminal_bench(
            tier,
            &manifests,
            &dir,
            options
                .terminal_bench
                .as_ref()
                .filter(|_| tier <= options.through),
            &mut spend,
        )
        .await?;
        higher.push(record);
    }

    // Records.
    let mut trials = Vec::new();
    for ((digest, phase), (_, rows)) in &evaluator.cache {
        for (case, task, s) in rows {
            trials.push(json!({
                "schema": TRIAL_SCHEMA,
                "candidate": digest,
                "tier": Tier::Replay.word(),
                "phase": phase,
                "case": case,
                "task": task,
                "outcome": "completed",
                "evaluation": s,
            }));
        }
    }
    write_lines(&dir.join("candidates.jsonl"), &archive.proposals)?;
    write_lines(&dir.join("trials.jsonl"), &trials)?;

    let aggregates = |list: &[(String, Aggregate)], d: &str| {
        list.iter().find(|(x, _)| x == d).map(|(_, a)| a.clone())
    };
    let candidates: Vec<Value> = archive
        .proposals
        .iter()
        .filter(|p| p.construction == super::Construction::Built)
        .filter_map(|p| {
            let d = p.candidate.clone()?;
            let reached = if mini.contains_key(&d) {
                Tier::Mini
            } else {
                Tier::Replay
            };
            let frontier_tasks: Vec<&String> = frontiers
                .iter()
                .filter(|(_, ids)| ids.contains(&d))
                .map(|(task, _)| task)
                .collect();
            Some(json!({
                "id": d,
                "label": p.label,
                "operator": p.proposer.operator,
                "parents": p.parents,
                "changes": p.changes,
                "reached": reached.word(),
                "search": aggregates(&search, &d),
                "selection": aggregates(&selection, &d),
                "survived_search": survivors.contains(&d),
                "promoted": promoted.contains(&d),
                "mini": mini.get(&d),
                "finalist": finalists.contains(&d),
                "selected": selected.as_deref() == Some(d.as_str()),
                "baseline": d == baseline_digest,
                "frontier_tasks": frontier_tasks,
                "wall_ms": evaluator.wall_ms.get(&d).copied().unwrap_or(0),
                "spend_usd": 0.0,
            }))
        })
        .collect();
    let operators: Vec<Value> = {
        let mut names: Vec<&str> = archive
            .proposals
            .iter()
            .map(|p| p.proposer.operator.as_str())
            .collect();
        names.dedup();
        let mut unique = Vec::new();
        for name in names {
            if !unique.contains(&name) {
                unique.push(name);
            }
        }
        unique
            .into_iter()
            .map(|name| {
                let of: Vec<&super::Proposal> = archive
                    .proposals
                    .iter()
                    .filter(|p| p.proposer.operator == name)
                    .collect();
                let count = |f: &dyn Fn(&super::Construction) -> bool| {
                    of.iter().filter(|p| f(&p.construction)).count()
                };
                let best = of
                    .iter()
                    .filter_map(|p| p.candidate.clone())
                    .filter_map(|d| aggregates(&search, &d).map(|a| (d, a.j)))
                    .max_by(|a, b| a.1.total_cmp(&b.1));
                json!({
                    "operator": name,
                    "algorithm": of.first().map(|p| p.proposer.algorithm.clone()),
                    "proposals": of.len(),
                    "built": count(&|c| *c == super::Construction::Built),
                    "duplicates": count(&|c| matches!(c, super::Construction::Duplicate { .. })),
                    "refused": count(&|c| matches!(c, super::Construction::Refused { .. })),
                    "best_search": best.as_ref().map(|(d, j)| json!({ "id": d, "j": j })),
                    "best_selection": of
                        .iter()
                        .filter_map(|p| p.candidate.clone())
                        .filter_map(|d| aggregates(&selection, &d).map(|a| (d, a.j)))
                        .max_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(d, j)| json!({ "id": d, "j": j })),
                })
            })
            .collect()
    };
    let refusals: Vec<Value> = archive
        .proposals
        .iter()
        .filter_map(|p| match &p.construction {
            super::Construction::Refused { reason } => Some(json!({
                "operator": p.proposer.operator,
                "label": p.label,
                "reason": reason,
            })),
            _ => None,
        })
        .collect();
    spend.push(Spend {
        category: "storage".to_string(),
        tier: Tier::Replay,
        calls: Some(0),
        wall_ms: 0,
        spend_microusd: Some(0),
        note: "records under the study directory".to_string(),
    });
    let status = "completed";
    let result = json!({
        "schema": RESULT_SCHEMA,
        "nip_opt": "openagents.optimization-result.v1",
        "study": id,
        "component": COMPONENT,
        "status": status,
        "baseline": baseline_digest,
        "selected": selected,
        "objective": OBJECTIVE,
        "acceptance": ACCEPTANCE,
        "split": {
            "seed": split.seed,
            "development_tasks": split.development_tasks,
            "held_out_tasks": split.held_out_tasks,
            "search": split.search.len(),
            "selection": split.selection.len(),
            "confirmation": split.confirmation.len(),
            "excluded": split.excluded.len(),
            "skipped": skipped,
        },
        "headroom": {
            "baseline_search": aggregates(&search, &baseline_digest),
            "note": "label_coverable is the share of labels some item informs at all; no packing can cover more",
        },
        "rungs": [
            { "tier": "replay", "phase": "search", "evaluated": search.len(), "kept": survivors.len(), "eta": ETA },
            { "tier": "replay", "phase": "selection", "evaluated": selection.len(), "kept": promoted.len(), "eta": ETA },
            { "tier": "mini", "phase": "screen", "evaluated": mini.len(), "kept": if options.through >= Tier::Mini { Some(finalists.len()) } else { None }, "run": options.through >= Tier::Mini },
        ],
        "promotions": {
            "search_to_selection": survivors,
            "selection_to_mini": promoted,
            "finalists": finalists,
        },
        "frontier": frontiers,
        "climb": climb_record,
        "operators": operators,
        "refusals": refusals,
        "candidates": candidates,
        "confirmation": confirmation,
        "higher_tiers": higher,
        "spend": total(&spend),
        "spend_entries": spend,
        "wall_ms": millis(started),
        "limitations": [
            "Replay scores packing, not task outcomes: a better J says the briefing delivers more of the selected and labeled evidence per character, not that the executor passes more often.",
            "The requirement map is the rule's (no Jev), and an item informs a requirement only through exact paths and constants, so label coverage undercounts evidence Jev's coverage judgments would tie to a requirement.",
            "coverage-jev packs as coverage here: most retained briefings have no recorded coverage judgments.",
            "The objective doesn't price evidence that is neither Jev-selected nor tied to a labeled requirement: a smaller briefing that omits more of it scores higher. The result's omitted counts show how much.",
            "A trimmed item counts as delivered, so the objective can't see how much of a selected item arrived: a smaller item_max trims selected items and still scores them whole.",
            "Eight tasks: three held out leave a noise floor wide enough that only a gain on every held-out task can clear it.",
            "The mini-task tier's scripted executor doesn't read the briefing, so it screens for gross failures (a candidate that breaks the episode or overruns the cap), not for quality.",
            "No Terminal-Bench tier ran unless higher_tiers says so.",
        ],
    });
    write_json(&dir.join("result.json"), &result)?;
    if let Some(retain) = &options.retain {
        let target = retain.join(&id);
        std::fs::create_dir_all(target.join("manifests"))
            .map_err(|error| format!("cannot create {}: {error}", target.display()))?;
        for name in [
            "study.json",
            "selection.json",
            "candidates.jsonl",
            "result.json",
        ] {
            std::fs::copy(dir.join(name), target.join(name))
                .map_err(|error| format!("cannot retain {name}: {error}"))?;
        }
        // The promoted candidates' manifests, which the higher tiers'
        // commands name.
        if let Ok(entries) = std::fs::read_dir(dir.join("manifests")) {
            for entry in entries.flatten() {
                std::fs::copy(
                    entry.path(),
                    target.join("manifests").join(entry.file_name()),
                )
                .map_err(|error| format!("cannot retain a manifest: {error}"))?;
            }
        }
    }
    Ok(Finished { dir, result })
}

fn confirmation_record(
    winner: &str,
    winner_agg: &Aggregate,
    base_agg: &Aggregate,
    floor: &Paired,
    beats: bool,
) -> Value {
    let statement = if beats {
        format!(
            "The selected candidate beats the baseline beyond the noise floor on held-out \
             evidence: J +{:.4} (95% task-clustered bootstrap {:.4} to {:.4}, {} tasks, {} briefings).",
            floor.mean, floor.low, floor.high, floor.tasks, floor.cases
        )
    } else {
        format!(
            "The selected candidate does not beat the baseline beyond the noise floor on held-out \
             evidence: J {:+.4} (95% task-clustered bootstrap {:.4} to {:.4}, {} tasks, {} briefings; \
             the rule needs at least +{EPSILON} and a lower bound above zero).",
            floor.mean, floor.low, floor.high, floor.tasks, floor.cases
        )
    };
    json!({
        "selected": winner,
        "phase": "confirmation",
        "selected_result": winner_agg,
        "baseline_result": base_agg,
        "paired": floor,
        "epsilon": EPSILON,
        "beats_baseline": beats,
        "statement": statement,
        "deltas": {
            "j": round(winner_agg.j - base_agg.j),
            "selected_delivered_rate": round(winner_agg.selected_delivered_rate - base_agg.selected_delivered_rate),
            "label_coverage": round(winner_agg.label_coverage - base_agg.label_coverage),
            "duplicate_bytes": winner_agg.duplicate_bytes as i64 - base_agg.duplicate_bytes as i64,
            "chars_mean": round(winner_agg.chars_mean - base_agg.chars_mean),
            "omitted": winner_agg.omitted as i64 - base_agg.omitted as i64,
        },
    })
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(out: &Path) -> Options {
        let mut options = Options::new(out.to_path_buf());
        // A small space keeps the test fast; the real study uses the
        // default one.
        options.space = Some(vec![
            Slot::numeric(
                "policy.brief.cap",
                &[8_000.0, 12_000.0],
                2_000.0,
                6_000.0,
                14_000.0,
            ),
            Slot::numeric(
                "policy.brief.pack.slice",
                &[600.0, 1_200.0],
                600.0,
                600.0,
                1_800.0,
            ),
        ]);
        options.climb_moves = 2;
        options
    }

    #[test]
    fn the_split_holds_out_whole_tasks() {
        let (cases, skipped) = load_cases(
            &Options::new(PathBuf::new()).traces,
            &Options::new(PathBuf::new()).fixtures,
        );
        assert!(cases.len() >= 200, "{}", cases.len());
        assert!(skipped.len() <= 1, "{skipped:?}");
        let split = split(&cases, 9557);
        assert_eq!(split.held_out_tasks.len(), HELD_OUT_TASKS);
        assert_eq!(split.development_tasks.len(), 5);
        let task_of: BTreeMap<&str, &str> = cases
            .iter()
            .map(|c| (c.id.as_str(), c.task.as_str()))
            .collect();
        for id in &split.confirmation {
            assert!(
                split
                    .held_out_tasks
                    .iter()
                    .any(|t| t == task_of[id.as_str()])
            );
        }
        for id in split.search.iter().chain(&split.selection) {
            assert!(
                split
                    .development_tasks
                    .iter()
                    .any(|t| t == task_of[id.as_str()])
            );
        }
        assert_eq!(
            split.search.len()
                + split.selection.len()
                + split.confirmation.len()
                + split.excluded.len(),
            cases.len()
        );
        // Every labeled task's labels are read, and the baseline covers
        // some of them.
        let base = baseline().unwrap();
        let scored: Vec<(&Case, Scored)> = cases
            .iter()
            .map(|c| (c, score(c, &base.policy.brief)))
            .collect();
        let aggregate = aggregate(&scored);
        assert_eq!(aggregate.tasks, 8);
        assert!(
            aggregate.label_coverage > 0.0 && aggregate.label_coverage <= aggregate.label_coverable
        );
        assert_eq!(aggregate.selected_delivered_rate, 1.0);
    }

    #[test]
    fn a_smaller_cap_trades_coverage_for_size() {
        let paths = Options::new(PathBuf::new());
        let (cases, _) = load_cases(&paths.traces, &paths.fixtures);
        let base = baseline().unwrap();
        let mut small = base.clone();
        small.policy.brief.cap = 3_000;
        let run = |m: &Manifest| {
            let scored: Vec<(&Case, Scored)> = cases
                .iter()
                .map(|c| (c, score(c, &m.policy.brief)))
                .collect();
            aggregate(&scored)
        };
        let (a, b) = (run(&base), run(&small));
        assert!(b.chars_mean < a.chars_mean);
        assert!(b.selected_delivered_rate <= a.selected_delivered_rate);
        let mut sections = base.clone();
        sections.policy.brief.packer = Packer::Sections;
        let s = run(&sections);
        assert!(s.selected_delivered_rate < a.selected_delivered_rate);
        assert!(s.duplicate_bytes > 0);
    }

    #[tokio::test]
    async fn a_small_study_runs_end_to_end_and_records_everything() {
        let dir = tempfile::tempdir().unwrap();
        let options = options(dir.path());
        let finished = run(&options).await.unwrap();
        let result = &finished.result;
        assert_eq!(result["status"], "completed");
        for name in [
            "study.json",
            "selection.json",
            "candidates.jsonl",
            "trials.jsonl",
            "result.json",
        ] {
            assert!(finished.dir.join(name).is_file(), "{name}");
        }
        let candidates = result["candidates"].as_array().unwrap();
        // The baseline, the swap, four grid points (one of them the
        // baseline's own values, a duplicate), and the climb's new ones.
        assert!(candidates.len() >= 5, "{}", candidates.len());
        assert!(candidates.iter().all(|c| !c["search"].is_null()));
        assert_eq!(result["spend"]["spend_usd"], 0.0);
        assert_eq!(result["spend"]["complete"], true);
        assert!(result["frontier"].as_object().unwrap().len() == 5);
        // Tiers 2 to 4 were planned, not run.
        for tier in result["higher_tiers"].as_array().unwrap() {
            assert_eq!(tier["run"], false, "{tier}");
        }
        // Rerunning the same plan writes the same study id and the same
        // scores.
        let again = run(&options).await.unwrap();
        assert_eq!(again.result["study"], result["study"]);
        assert_eq!(again.result["selected"], result["selected"]);
        assert_eq!(again.result["confirmation"], result["confirmation"]);
    }
}
