//! Studies as data: a study descriptor (`openagents.gym.study.v1`) and its
//! per-attempt rows (`openagents.gym.attempt-row.v1`) make a board with no
//! Rust.
//!
//! An experiment's `summarize.py` writes its rows next to its own output,
//! and the study commits a `study.json` descriptor naming the board, its
//! question, issues, report, frozen commit, subject, reference, beat rule,
//! labels, the headline and caveats as templates, and where the rows and
//! bars are. [`build`] reads both, recomputes every attempt's beat under
//! the named rule, and runs the same cross-checks every adapter runs:
//!
//! - an attempt whose recorded `verdict` disagrees with the rule refuses;
//! - an attempt whose bar disagrees with the bars file refuses;
//! - a series whose counted tally disagrees with the rows file's own
//!   `tallies` refuses;
//! - a descriptor naming a rule this crate doesn't know refuses.
//!
//! Headline and caveat templates hold words; every number in them is a
//! `{placeholder}` the code fills from the tallies (see [`Values`]), and a
//! placeholder it doesn't know refuses.
//!
//! Rows in an older shape are read through a named converter
//! ([`RowsShape`]); #9776's `attempts.json` is the first.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::contract::{
    Attempt, Bar, Benchmark, Board, BoardKind, Caveat, Cost, CostBasis, EvidenceFile, JevSummary,
    Label, Miss, Phases, Provenance, Reference, Spend, Split, Subject, Tally, TaskKnowledge,
    TaskRow, TaskStatus, TraceRef, VerifierSummary,
};
use crate::evidence::{Reader, Result, count, fail};
use crate::tb4_delegate::TraceJob;
use crate::{THIN_MARGIN, pct};

/// The study descriptor's schema.
pub const STUDY_SCHEMA: &str = "openagents.gym.study.v1";

/// The attempt-row schema.
pub const ROW_SCHEMA: &str = "openagents.gym.attempt-row.v1";

/// Where descriptors are found: `<dir>/*/study.json`.
pub const STUDY_DIRS: [&str; 2] = [
    "bench/terminal-bench/experiments",
    "bench/terminal-bench/studies",
];

/// A study descriptor.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Descriptor {
    pub schema: String,
    pub board: String,
    pub title: String,
    pub benchmark: Benchmark,
    pub question: String,
    pub issues: Vec<u32>,
    /// The report, repository-relative. Without one, the rows file stands
    /// in as the board's report.
    #[serde(default)]
    pub report: Option<String>,
    #[serde(default)]
    pub frozen_commit: Option<String>,
    pub subject: SubjectFields,
    pub reference: Reference,
    /// The beat rule: a [`BoardKind`] other than `reference`.
    pub rule: BoardKind,
    /// Labels on every claim on the board. `cost_bound` and `in_sample`
    /// are added by code when an attempt carries them.
    #[serde(default)]
    pub labels: Vec<Label>,
    /// Labels every attempt carries, such as `knowledge_off`.
    #[serde(default)]
    pub attempt_labels: Vec<Label>,
    /// The headline template.
    pub headline: String,
    /// Caveat templates, in order.
    #[serde(default)]
    pub caveats: Vec<CaveatTemplate>,
    /// Which splits to add after one per series: `own_knowledge` adds
    /// tasks with and without their own knowledge.
    #[serde(default)]
    pub splits: Vec<String>,
    /// The cost basis when the rows don't say.
    #[serde(default)]
    pub cost_basis: Option<CostBasis>,
    pub rows: FileRef,
    #[serde(default)]
    pub bars: Option<FileRef>,
}

/// A file and the shape it's in.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileRef {
    /// Repository-relative.
    pub path: String,
    /// [`ROW_SCHEMA`], or a converter's name ([`RowsShape`]).
    #[serde(default)]
    pub shape: Option<String>,
}

/// Subject fields; any left out come from the rows.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SubjectFields {
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub arm: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub artifact: Option<String>,
}

/// A caveat as words with `{placeholders}`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CaveatTemplate {
    pub code: String,
    pub text: String,
    /// Shown only when this count is above zero: `thin`, `cost_unknown`,
    /// `faults`, or `beats`.
    #[serde(default)]
    pub when: Option<String>,
    /// Also shown on the rows of beats (for `in_sample` and `thin_margin`,
    /// only the beats that carry that label).
    #[serde(default)]
    pub on_beats: bool,
}

/// One attempt, as an experiment writes it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AttemptRow {
    /// Unique on the board, e.g. `coq-block-bound.p2`.
    pub id: String,
    pub task: String,
    /// The split it belongs to, in run order, e.g. `pass 2`.
    pub series: String,
    /// The harness trial ID or run record.
    pub trial: String,
    /// A harness or provider fault: not a result, counted apart.
    #[serde(default)]
    pub fault: bool,
    #[serde(default)]
    pub reward: Option<f64>,
    /// Whole-trial seconds, the time the bar compares against.
    #[serde(default)]
    pub seconds: Option<f64>,
    #[serde(default)]
    pub phases: Option<Phases>,
    /// Required unless `fault`.
    #[serde(default)]
    pub cost: Option<Cost>,
    #[serde(default)]
    pub cost_basis: Option<CostBasis>,
    /// Required unless `fault`.
    #[serde(default)]
    pub bar: Option<Bar>,
    /// The study's own beat verdict, which must agree with the rule.
    #[serde(default)]
    pub verdict: Option<bool>,
    #[serde(default)]
    pub knowledge: RowKnowledge,
    #[serde(default)]
    pub jev: Option<JevSummary>,
    #[serde(default)]
    pub how_it_ended: Option<String>,
    #[serde(default)]
    pub verifier: Option<VerifierSummary>,
    /// The retained episode directory to bundle, repository-relative.
    #[serde(default)]
    pub episode: Option<String>,
    #[serde(default)]
    pub subject: Option<SubjectFields>,
}

/// The knowledge an attempt was offered.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RowKnowledge {
    /// Entry IDs the attempt kept.
    #[serde(default)]
    pub kept: Vec<String>,
    /// Entry IDs offered to it that were written from its own task.
    #[serde(default)]
    pub own: Vec<String>,
}

/// A study's own count for one series, to check against the rows.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RecordedTally {
    pub attempts: u32,
    pub passes: u32,
    pub beats: u32,
    #[serde(default)]
    pub faults: u32,
    #[serde(default)]
    pub cost_unknown: u32,
    /// The known cost's sum, to the cent.
    #[serde(default)]
    pub known_cost_usd: Option<f64>,
}

/// A rows file in the shared shape.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rows {
    pub schema: String,
    pub rows: Vec<AttemptRow>,
    /// The study's own tallies by series name.
    #[serde(default)]
    pub tallies: BTreeMap<String, RecordedTally>,
}

/// Row shapes read through a converter, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowsShape {
    /// [`ROW_SCHEMA`].
    Shared,
    /// #9776's `attempts.json`, written by its `summarize.py`.
    FableDelegateRepro,
}

impl RowsShape {
    fn of(file: &FileRef) -> Result<Self> {
        match file.shape.as_deref() {
            None | Some(ROW_SCHEMA) => Ok(Self::Shared),
            Some(crate::tb4_delegate::ROWS_SHAPE) => Ok(Self::FableDelegateRepro),
            Some(other) => Err(fail!("{}: unknown row shape {other:?}", file.path)),
        }
    }
}

/// Reads a descriptor and its rows and builds the board.
pub fn build(reader: &Reader, descriptor_rel: &str) -> Result<(Board, Vec<TraceJob>)> {
    let (value, _) = reader.json(descriptor_rel)?;
    let d: Descriptor =
        serde_json::from_value(value).map_err(|e| fail!("{descriptor_rel}: {e}"))?;
    if d.schema != STUDY_SCHEMA {
        return Err(fail!("{descriptor_rel}: not a {STUDY_SCHEMA} descriptor"));
    }
    if !matches!(
        d.rule,
        BoardKind::BeatCheapestAndFastestWin
            | BoardKind::CostBelowCheapestWin
            | BoardKind::CostBelowReferencePerTrial
    ) {
        return Err(fail!(
            "{descriptor_rel}: names an unknown rule; a study's rule must be beat_cheapest_and_fastest_win, cost_below_cheapest_win, or cost_below_reference_per_trial"
        ));
    }

    let mut evidence: Vec<EvidenceFile> = Vec::new();
    let rows = match RowsShape::of(&d.rows)? {
        RowsShape::Shared => {
            let (value, file) = reader.json(&d.rows.path)?;
            evidence.push(file);
            let rows: Rows =
                serde_json::from_value(value).map_err(|e| fail!("{}: {e}", d.rows.path))?;
            if rows.schema != ROW_SCHEMA {
                return Err(fail!("{}: not {ROW_SCHEMA} rows", d.rows.path));
            }
            rows
        }
        RowsShape::FableDelegateRepro => {
            let (rows, file) = crate::tb4_delegate::rows(reader, &d.rows.path)?;
            evidence.push(file);
            rows
        }
    };
    // The bars file names every task in order; without one, the tasks are
    // in first-row order with the rows' own bars.
    let mut bars: Vec<(String, Bar)> = Vec::new();
    if let Some(file) = &d.bars {
        let (list, entry) = crate::tb4_delegate::bars(reader, file)?;
        evidence.push(entry);
        bars = list;
    }
    let from_file = !bars.is_empty();

    let mut attempts = Vec::new();
    let mut jobs = Vec::new();
    let mut faults: BTreeMap<String, u32> = BTreeMap::new();
    let mut series_order: Vec<String> = Vec::new();
    let mut own_by_task: BTreeMap<String, u32> = BTreeMap::new();
    let mut subject_from_rows: Option<SubjectFields> = None;
    for row in &rows.rows {
        if !series_order.contains(&row.series) {
            series_order.push(row.series.clone());
        }
        if row.fault {
            *faults.entry(row.series.clone()).or_default() += 1;
            continue;
        }
        let row_bar = row.bar.clone().ok_or_else(|| fail!("{}: no bar", row.id))?;
        let bar = match bars.iter().find(|(t, _)| *t == row.task) {
            Some((_, bar)) => {
                check_bar(&row.id, &row_bar, bar, from_file)?;
                bar.clone()
            }
            None if from_file => {
                return Err(fail!(
                    "{}: no bar for {} in the bars file",
                    row.id,
                    row.task
                ));
            }
            None => {
                bars.push((row.task.clone(), row_bar.clone()));
                row_bar
            }
        };
        own_by_task.insert(row.task.clone(), count(row.knowledge.own.len()));
        if subject_from_rows.is_none() {
            subject_from_rows = row.subject.clone();
        }
        let attempt = attempt(&d, row, &bar)?;
        if let Some(episode) = &row.episode {
            jobs.push(TraceJob {
                attempt: attempt.clone(),
                bar: bar.clone(),
                episode: episode.clone(),
                own_entries: row.knowledge.own.iter().cloned().collect(),
            });
        }
        attempts.push(attempt);
    }
    if attempts.is_empty() {
        return Err(fail!("{}: no graded attempts", d.rows.path));
    }

    let tasks: Vec<TaskRow> = bars
        .iter()
        .map(|(task, bar)| {
            let mine: Vec<&Attempt> = attempts.iter().filter(|a| &a.task == task).collect();
            let passes = count(mine.iter().filter(|a| a.passed).count());
            let beats = count(mine.iter().filter(|a| a.beat).count());
            let own = own_by_task.get(task).copied().unwrap_or(0);
            TaskRow {
                task: task.clone(),
                bar: bar.clone(),
                knowledge: if own > 0 {
                    TaskKnowledge::Own { candidates: own }
                } else if d.labels.contains(&Label::KnowledgeOff) {
                    TaskKnowledge::Off
                } else {
                    TaskKnowledge::OtherTasksOnly
                },
                attempts: mine.iter().map(|a| a.id.clone()).collect(),
                passes,
                beats,
                status: if beats > 0 {
                    TaskStatus::Beat
                } else if passes > 0 {
                    TaskStatus::PassedWithoutBeat
                } else {
                    TaskStatus::NeverPassed
                },
            }
        })
        .collect();

    let own_tasks: Vec<&str> = tasks
        .iter()
        .filter(|t| matches!(t.knowledge, TaskKnowledge::Own { .. }))
        .map(|t| t.task.as_str())
        .collect();
    let tally_of = |filter: &dyn Fn(&Attempt) -> bool, faults: u32| {
        let set: Vec<&Attempt> = attempts.iter().filter(|a| filter(a)).collect();
        Tally {
            attempts: count(set.len()),
            passes: count(set.iter().filter(|a| a.passed).count()),
            beats: count(set.iter().filter(|a| a.beat).count()),
            faults,
            cost_unknown: count(set.iter().filter(|a| a.cost.known().is_none()).count()),
        }
    };
    let totals = tally_of(&|_| true, faults.values().sum());
    let mut splits = Vec::new();
    for series in &series_order {
        let tally = tally_of(
            &|a| &a.series == series,
            faults.get(series).copied().unwrap_or(0),
        );
        if let Some(recorded) = rows.tallies.get(series) {
            check_tally(series, &tally, recorded, &attempts)?;
        }
        splits.push(Split {
            name: series.clone(),
            tally,
        });
    }
    for extra in &d.splits {
        match extra.as_str() {
            "own_knowledge" => {
                splits.push(Split {
                    name: "tasks with their own knowledge".into(),
                    tally: tally_of(&|a| own_tasks.contains(&a.task.as_str()), 0),
                });
                splits.push(Split {
                    name: "tasks without their own knowledge".into(),
                    tally: tally_of(&|a| !own_tasks.contains(&a.task.as_str()), 0),
                });
            }
            other => return Err(fail!("{descriptor_rel}: unknown split {other:?}")),
        }
    }

    let bases: BTreeSet<CostBasis> = rows
        .rows
        .iter()
        .filter(|r| !r.fault)
        .filter_map(|r| r.cost_basis)
        .collect();
    let basis = match bases.len() {
        0 => d.cost_basis.unwrap_or(CostBasis::ListPrice),
        1 => *bases.iter().next().expect("one"),
        _ => CostBasis::Mixed,
    };
    if basis == CostBasis::Mixed {
        for (a, r) in attempts
            .iter_mut()
            .zip(rows.rows.iter().filter(|r| !r.fault))
        {
            a.cost_basis = r.cost_basis;
        }
    }
    let reported_usd: f64 = attempts.iter().filter_map(|a| a.cost.known()).sum();
    let bound = |upper: bool| -> Option<f64> {
        let values: Vec<f64> = attempts
            .iter()
            .filter_map(|a| match a.cost {
                Cost::Unknown {
                    lower_bound_usd,
                    upper_bound_usd,
                } => {
                    if upper {
                        upper_bound_usd
                    } else {
                        lower_bound_usd
                    }
                }
                Cost::Reported { .. } => None,
            })
            .collect();
        (!values.is_empty() || !upper).then(|| values.iter().sum())
    };
    let spend = Spend {
        basis,
        reported_usd,
        estimated_lower_bound_usd: (totals.cost_unknown > 0).then(|| bound(false)).flatten(),
        estimated_upper_bound_usd: (totals.cost_unknown > 0).then(|| bound(true)).flatten(),
    };

    let values = Values::of(&d, &attempts, &tasks, &splits, &series_order, &totals);
    let headline = values.fill(&d.headline)?;
    let mut caveats = Vec::new();
    for template in &d.caveats {
        let shown = match template.when.as_deref() {
            None => true,
            Some("thin") => values.thin_count > 0,
            Some("cost_unknown") => totals.cost_unknown > 0,
            Some("faults") => totals.faults > 0,
            Some("beats") => totals.beats > 0,
            Some(other) => {
                return Err(fail!(
                    "{descriptor_rel}: unknown caveat condition {other:?}"
                ));
            }
        };
        if shown {
            caveats.push(Caveat {
                code: template.code.clone(),
                text: values.fill(&template.text)?,
            });
        }
        if template.on_beats {
            for a in attempts.iter_mut().filter(|a| a.beat) {
                let applies = match template.code.as_str() {
                    "in_sample" => a.labels.contains(&Label::InSample),
                    "thin_margin" => a.labels.contains(&Label::ThinMargin),
                    _ => true,
                };
                if applies && shown {
                    a.caveats.push(template.code.clone());
                }
            }
        }
    }

    let mut labels = d.labels.clone();
    if totals.cost_unknown > 0 && !labels.contains(&Label::CostBound) {
        labels.push(Label::CostBound);
    }
    if attempts.iter().any(|a| a.labels.contains(&Label::InSample))
        && !labels.contains(&Label::InSample)
    {
        labels.push(Label::InSample);
    }
    labels.sort();

    let report = match &d.report {
        Some(rel) => {
            evidence.push(reader.bytes(rel)?.1);
            rel.clone()
        }
        None => d.rows.path.clone(),
    };
    let from_rows = subject_from_rows.unwrap_or_default();
    let subject = Subject {
        agent: d
            .subject
            .agent
            .clone()
            .or(from_rows.agent)
            .ok_or_else(|| fail!("{descriptor_rel}: no subject agent"))?,
        arm: d
            .subject
            .arm
            .clone()
            .or(from_rows.arm)
            .ok_or_else(|| fail!("{descriptor_rel}: no subject arm"))?,
        model: d
            .subject
            .model
            .clone()
            .or(from_rows.model)
            .ok_or_else(|| fail!("{descriptor_rel}: no subject model"))?,
        effort: d.subject.effort.clone().or(from_rows.effort),
        artifact: d.subject.artifact.clone().or(from_rows.artifact),
    };

    let board = Board {
        id: d.board.clone(),
        title: d.title.clone(),
        benchmark: d.benchmark.clone(),
        kind: d.rule,
        question: d.question.clone(),
        headline,
        provenance: Provenance {
            issues: d.issues.clone(),
            report,
            frozen_commit: d.frozen_commit.clone(),
            evidence,
        },
        subject,
        reference: d.reference.clone(),
        labels,
        caveats,
        totals,
        splits,
        spend,
        tasks,
        attempts,
        reference_rows: Vec::new(),
        snapshot: None,
    };
    Ok((board, jobs))
}

/// A row's bar must be the bars file's (or, without one, the same for
/// every row of a task).
fn check_bar(id: &str, row: &Bar, bar: &Bar, from_file: bool) -> Result<()> {
    let same = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) => crate::evidence::close(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same(row.cost_usd, bar.cost_usd) || !same(row.seconds, bar.seconds) {
        return Err(fail!(
            "{id}: its bar ({:?}, {:?}) differs from {} ({:?}, {:?})",
            row.cost_usd,
            row.seconds,
            if from_file {
                "the bars file"
            } else {
                "another row of its task"
            },
            bar.cost_usd,
            bar.seconds
        ));
    }
    Ok(())
}

fn check_tally(
    series: &str,
    tally: &Tally,
    recorded: &RecordedTally,
    attempts: &[Attempt],
) -> Result<()> {
    for (key, got, want) in [
        ("attempts", tally.attempts, recorded.attempts),
        ("passes", tally.passes, recorded.passes),
        ("beats", tally.beats, recorded.beats),
        ("faults", tally.faults, recorded.faults),
        ("cost_unknown", tally.cost_unknown, recorded.cost_unknown),
    ] {
        if got != want {
            return Err(fail!(
                "{series}: counted {key} {got}, the rows' tallies say {want}"
            ));
        }
    }
    if let Some(want) = recorded.known_cost_usd {
        let known: f64 = attempts
            .iter()
            .filter(|a| a.series == series)
            .filter_map(|a| a.cost.known())
            .sum();
        // Studies round their sums to four places.
        if (known - want).abs() > 0.01 {
            return Err(fail!(
                "{series}: known cost {known} differs from the tallies' {want}"
            ));
        }
    }
    Ok(())
}

/// One row as an attempt, its beat recomputed under the study's rule.
fn attempt(d: &Descriptor, row: &AttemptRow, bar: &Bar) -> Result<Attempt> {
    let id = &row.id;
    let cost = row.cost.ok_or_else(|| fail!("{id}: no cost"))?;
    let passed = row.reward.is_some_and(|r| r >= 1.0);
    let cost_bar = bar.cost_usd.ok_or_else(|| fail!("{id}: no cost bar"))?;
    let timed = d.rule == BoardKind::BeatCheapestAndFastestWin;
    let mut misses = Vec::new();
    if !passed {
        misses.push(Miss::Failed);
    }
    match cost.known() {
        None => misses.push(Miss::CostUnknown),
        Some(c) if c >= cost_bar => misses.push(Miss::Cost),
        Some(_) => {}
    }
    if timed {
        let time_bar = bar.seconds.ok_or_else(|| fail!("{id}: no time bar"))?;
        if row.seconds.is_none_or(|s| s >= time_bar) {
            misses.push(Miss::Time);
        }
    }
    let beat = misses.is_empty();
    if let Some(recorded) = row.verdict
        && recorded != beat
    {
        return Err(fail!(
            "{id}: recomputed beat {beat} but the row says {recorded}"
        ));
    }
    let (cost_ratio, cost_ratio_is_bound) = match cost {
        Cost::Reported { usd } => (Some(usd / cost_bar), false),
        Cost::Unknown {
            lower_bound_usd, ..
        } => (lower_bound_usd.map(|l| l / cost_bar), true),
    };
    let time_ratio = row.seconds.zip(bar.seconds).map(|(s, b)| s / b);

    let mut labels = d.attempt_labels.clone();
    if !row.knowledge.kept.is_empty() {
        labels.push(Label::KnowledgeAssisted);
        if row
            .knowledge
            .kept
            .iter()
            .any(|k| row.knowledge.own.contains(k))
        {
            labels.push(Label::InSample);
        }
    }
    if cost.known().is_none() {
        labels.push(Label::CostBound);
    }
    if beat {
        let cost_margin = 1.0 - cost_ratio.unwrap_or(1.0);
        let time_margin = 1.0 - time_ratio.unwrap_or(1.0);
        if cost_margin < THIN_MARGIN || (timed && time_margin < THIN_MARGIN) {
            labels.push(Label::ThinMargin);
        }
    }
    labels.sort();
    labels.dedup();

    Ok(Attempt {
        id: id.clone(),
        task: row.task.clone(),
        series: row.series.clone(),
        trial: row.trial.clone(),
        reward: row.reward,
        passed,
        seconds: row.seconds,
        phases: row.phases,
        cost,
        cost_ratio,
        cost_ratio_is_bound,
        time_ratio,
        beat,
        misses,
        labels,
        how_it_ended: row.how_it_ended.clone(),
        jev: row.jev.clone(),
        verifier: row.verifier.clone(),
        trace: row.episode.as_ref().map(|_| TraceRef {
            path: crate::trace_path(&d.board, id),
            sha256: String::new(),
            bytes: 0,
        }),
        cost_basis: None,
        caveats: Vec::new(),
    })
}

/// The numbers a template may name, computed from the board.
pub struct Values {
    map: BTreeMap<&'static str, String>,
    thin_count: usize,
}

impl Values {
    fn of(
        d: &Descriptor,
        attempts: &[Attempt],
        tasks: &[TaskRow],
        splits: &[Split],
        series: &[String],
        totals: &Tally,
    ) -> Self {
        let beats: Vec<&Attempt> = attempts.iter().filter(|a| a.beat).collect();
        let without_own: Vec<&TaskRow> = tasks
            .iter()
            .filter(|t| !matches!(t.knowledge, TaskKnowledge::Own { .. }))
            .collect();
        let thin: Vec<String> = beats
            .iter()
            .filter(|a| a.labels.contains(&Label::ThinMargin))
            .map(|a| format!("{} by {}", a.task, pct(1.0 - a.cost_ratio.unwrap_or(1.0))))
            .collect();
        let per_series: Vec<&Split> = splits.iter().filter(|s| series.contains(&s.name)).collect();
        let mut map = BTreeMap::new();
        map.insert("attempts", totals.attempts.to_string());
        map.insert("passes", totals.passes.to_string());
        map.insert("beats", totals.beats.to_string());
        map.insert("faults", totals.faults.to_string());
        map.insert("cost_unknown", totals.cost_unknown.to_string());
        map.insert("tasks", tasks.len().to_string());
        map.insert("reference", d.reference.name.clone());
        map.insert(
            "beats_in_sample",
            beats
                .iter()
                .filter(|a| a.labels.contains(&Label::InSample))
                .count()
                .to_string(),
        );
        map.insert("tasks_without_own", without_own.len().to_string());
        map.insert(
            "beats_without_own",
            without_own.iter().map(|t| t.beats).sum::<u32>().to_string(),
        );
        map.insert("thin_count", thin.len().to_string());
        map.insert("thin_list", thin.join(", "));
        map.insert("thin_margin", pct(THIN_MARGIN));
        map.insert(
            "series_beats",
            per_series
                .iter()
                .map(|s| format!("{}: {} of {}", s.name, s.tally.beats, s.tally.attempts))
                .collect::<Vec<_>>()
                .join(", "),
        );
        map.insert(
            "series_beats_in",
            per_series
                .iter()
                .map(|s| format!("{} of {} in {}", s.tally.beats, s.tally.attempts, s.name))
                .collect::<Vec<_>>()
                .join(", "),
        );
        map.insert(
            "series_passes_in",
            per_series
                .iter()
                .map(|s| format!("{} of {} in {}", s.tally.passes, s.tally.attempts, s.name))
                .collect::<Vec<_>>()
                .join(", "),
        );
        Self {
            map,
            thin_count: thin.len(),
        }
    }

    /// The template with every `{name}` filled; an unknown name refuses.
    pub fn fill(&self, template: &str) -> Result<String> {
        let mut out = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let close = after
                .find('}')
                .ok_or_else(|| fail!("an unclosed placeholder in {template:?}"))?;
            let name = &after[..close];
            let value = self
                .map
                .get(name)
                .ok_or_else(|| fail!("unknown placeholder {{{name}}} in {template:?}"))?;
            out.push_str(value);
            rest = &after[close + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    /// The names a template may use.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.map.keys().copied().collect()
    }
}

/// Every descriptor under [`STUDY_DIRS`], by path: a `study.json` that
/// declares [`STUDY_SCHEMA`]. Other files of that name are other tools'.
pub fn discover(reader: &Reader) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for dir in STUDY_DIRS {
        if !reader.exists(dir) {
            continue;
        }
        for name in reader.list(dir)? {
            let rel = format!("{dir}/{name}/study.json");
            if !reader.exists(&rel) {
                continue;
            }
            let (value, _) = reader.json(&rel)?;
            if value.get("schema").and_then(|s| s.as_str()) == Some(STUDY_SCHEMA) {
                found.push(rel);
            }
        }
    }
    Ok(found)
}
