//! The loop the operator asked for, with the noise floor first because an
//! episode is much noisier than an item and one run per side is not a
//! comparison.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use gym::gate::{Comparison, Outcome, Scores, Verdict};
use gym::spread::Spread;

use crate::{Ending, Fault, Judgment, Observed, Task, observe};

/// One judged run, reduced to what a series compares.
#[derive(Clone, Debug)]
pub struct Run {
    pub trace: PathBuf,
    pub judgment: Judgment,
    /// Faults keyed so the same fault reads the same across runs.
    pub signatures: Vec<String>,
    pub steps: usize,
    pub milliseconds: u64,
    pub delegations: usize,
    pub delegations_verified: usize,
    pub ending: Ending,
}

/// Reduces an observed run to a `Run`. `trace` is only recorded.
#[must_use]
pub fn measure(task: &Task, run: &Observed, trace: &Path) -> Run {
    let judgment = task.judge(run);
    let mut signatures = Vec::new();
    for fault in &judgment.faults {
        signatures.push(signature(fault, run));
    }
    let delegations_verified = if task.grade.expects.is_empty() {
        run.delegations
            .iter()
            .filter(|delegation| delegation.verified())
            .count()
    } else {
        run.delegations
            .iter()
            .zip(&task.grade.expects)
            .filter(|(delegation, expected)| delegation.verified_against(expected))
            .count()
    };
    Run {
        trace: trace.to_path_buf(),
        judgment,
        signatures,
        steps: run.order.len(),
        milliseconds: run.milliseconds,
        delegations: run.delegations.len(),
        delegations_verified,
        ending: run.ending.clone(),
    }
}

/// Reads and measures one recorded trace.
///
/// Ending and workspace stay as the trace leaves them, which is what a
/// recorded trace can say.
pub fn measure_trace(task: &Task, trace: &Path) -> Result<Run, String> {
    let observed = observe(trace)?;
    Ok(measure(task, &observed, trace))
}

fn signature(fault: &Fault, run: &Observed) -> String {
    let slot = |id: &str| {
        run.delegations
            .iter()
            .position(|delegation| delegation.id == id)
            .map_or_else(|| id.to_string(), |index| format!("#{}", index + 1))
    };
    let normalized = match fault {
        Fault::DelegationMisattributed { id, wanted, found } => Fault::DelegationMisattributed {
            id: slot(id),
            wanted: wanted.clone(),
            found: found.clone(),
        },
        Fault::DelegationAnswered { id, wanted, found } => Fault::DelegationAnswered {
            id: slot(id),
            wanted: wanted.clone(),
            found: found.clone(),
        },
        Fault::DelegationWrong { id } => Fault::DelegationWrong { id: slot(id) },
        Fault::DelegationFailed { id, outcome } => Fault::DelegationFailed {
            id: slot(id),
            outcome: outcome.clone(),
        },
        Fault::DelegationUnverified { id } => Fault::DelegationUnverified { id: slot(id) },
        other => other.clone(),
    };
    normalized.to_string()
}

/// How often a fault appeared in a series.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Persistence {
    Persistent,
    Intermittent,
}

/// One fault's frequency and strongest evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultRate {
    pub signature: String,
    pub seen: usize,
    pub of: usize,
    pub verdict: Verdict,
    pub persistence: Persistence,
}

/// Spreads over the runs when nothing changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Floor {
    pub runs: usize,
    pub passed: usize,
    pub faults: Option<Spread>,
    pub steps: Option<Spread>,
    pub seconds: Option<Spread>,
    pub delegations_verified: Option<Spread>,
}

/// A labelled series of measured runs.
#[derive(Clone, Debug)]
pub struct Series {
    pub label: String,
    pub runs: Vec<Run>,
}

impl Series {
    /// Classifies faults by how consistently they appeared.
    #[must_use]
    pub fn faults(&self) -> Vec<FaultRate> {
        let mut rates: BTreeMap<String, (usize, Verdict)> = BTreeMap::new();
        for run in &self.runs {
            let mut seen_in_run = BTreeMap::new();
            for (signature, fault) in run.signatures.iter().zip(&run.judgment.faults) {
                let verdict = seen_in_run.entry(signature).or_insert(Verdict::Passed);
                *verdict = Verdict::over([*verdict, fault.verdict()]);
            }
            for (signature, verdict) in seen_in_run {
                let entry = rates
                    .entry(signature.clone())
                    .or_insert((0, Verdict::Passed));
                entry.0 += 1;
                entry.1 = Verdict::over([entry.1, verdict]);
            }
        }
        let of = self.runs.len();
        let mut rates = rates
            .into_iter()
            .map(|(signature, (seen, verdict))| FaultRate {
                signature,
                seen,
                of,
                verdict,
                persistence: if seen == of {
                    Persistence::Persistent
                } else {
                    Persistence::Intermittent
                },
            })
            .collect::<Vec<_>>();
        rates.sort_by(|left, right| {
            (right.persistence == Persistence::Persistent)
                .cmp(&(left.persistence == Persistence::Persistent))
                .then_with(|| right.seen.cmp(&left.seen))
                .then_with(|| left.signature.cmp(&right.signature))
        });
        rates
    }

    /// Measures the series' noise floor.
    #[must_use]
    pub fn floor(&self) -> Floor {
        Floor {
            runs: self.runs.len(),
            passed: self
                .runs
                .iter()
                .filter(|run| run.judgment.verdict == Verdict::Passed)
                .count(),
            faults: Spread::over(
                &self
                    .runs
                    .iter()
                    .map(|run| run.signatures.len() as f64)
                    .collect::<Vec<_>>(),
            ),
            steps: Spread::over(
                &self
                    .runs
                    .iter()
                    .map(|run| run.steps as f64)
                    .collect::<Vec<_>>(),
            ),
            seconds: Spread::over(
                &self
                    .runs
                    .iter()
                    .map(|run| run.milliseconds as f64 / 1000.0)
                    .collect::<Vec<_>>(),
            ),
            delegations_verified: Spread::over(
                &self
                    .runs
                    .iter()
                    .map(|run| run.delegations_verified as f64)
                    .collect::<Vec<_>>(),
            ),
        }
    }

    /// Returns the share of runs that passed.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn pass_rate(&self) -> Option<f64> {
        (!self.runs.is_empty()).then(|| {
            self.runs
                .iter()
                .filter(|run| run.judgment.verdict == Verdict::Passed)
                .count() as f64
                / self.runs.len() as f64
        })
    }

    /// Whether the series has no persistent faults.
    #[must_use]
    pub fn matches(&self) -> Verdict {
        if self.runs.len() < 2 {
            return Verdict::Unverifiable;
        }
        let persistent = self
            .faults()
            .into_iter()
            .filter(|fault| fault.persistence == Persistence::Persistent)
            .collect::<Vec<_>>();
        if persistent.is_empty() {
            Verdict::Passed
        } else {
            Verdict::over(persistent.into_iter().map(|fault| fault.verdict))
        }
    }
}

/// One metric's move between two series.
#[derive(Clone, Debug, PartialEq)]
pub struct Shift {
    pub metric: &'static str,
    pub before: f64,
    pub after: f64,
    pub detectable: Option<f64>,
}

impl Shift {
    /// Whether the move is inside the baseline's measured spread.
    #[must_use]
    pub fn inside_spread(&self) -> Option<bool> {
        self.detectable
            .map(|detectable| (self.after - self.before).abs() <= detectable)
    }
}

/// A baseline and candidate comparison.
#[derive(Clone, Debug)]
pub struct Tuning {
    pub baseline: Floor,
    pub candidate: Floor,
    pub shifts: Vec<Shift>,
    pub fixed: Vec<String>,
    pub introduced: Vec<String>,
    pub remaining: Vec<String>,
    pub gate: Outcome,
    pub verdict: Verdict,
}

/// Compares a candidate against the baseline's measured floor.
pub fn compare(task: &Task, baseline: &Series, candidate: &Series) -> Result<Tuning, String> {
    let baseline_floor = baseline.floor();
    let candidate_floor = candidate.floor();
    let baseline_faults = persistent_signatures(baseline);
    let candidate_faults = persistent_signatures(candidate);
    let mut fixed = baseline_faults
        .difference(&candidate_faults)
        .cloned()
        .collect::<Vec<_>>();
    let mut introduced = candidate_faults
        .difference(&baseline_faults)
        .cloned()
        .collect::<Vec<_>>();
    let mut remaining = baseline_faults
        .intersection(&candidate_faults)
        .cloned()
        .collect::<Vec<_>>();
    fixed.sort();
    introduced.sort();
    remaining.sort();
    let shifts = vec![
        shift(
            "faults",
            baseline,
            candidate,
            baseline_floor.faults.as_ref(),
            metric_faults,
        ),
        shift(
            "steps",
            baseline,
            candidate,
            baseline_floor.steps.as_ref(),
            metric_steps,
        ),
        shift(
            "seconds",
            baseline,
            candidate,
            baseline_floor.seconds.as_ref(),
            metric_seconds,
        ),
        shift(
            "verified delegations",
            baseline,
            candidate,
            baseline_floor.delegations_verified.as_ref(),
            metric_verified,
        ),
        Shift {
            metric: "pass_rate",
            before: baseline.pass_rate().unwrap_or(0.0),
            after: candidate.pass_rate().unwrap_or(0.0),
            detectable: None,
        },
    ];
    let comparison = Comparison::new(
        task.id.clone(),
        Scores {
            items: baseline.runs.len(),
            accuracy: baseline.pass_rate(),
            ..Scores::default()
        },
        Scores {
            items: candidate.runs.len(),
            accuracy: candidate.pass_rate(),
            ..Scores::default()
        },
    );
    let gate = gym::gate::load("decision-v1")
        .map_err(|error| error.to_string())?
        .judge(&comparison);
    Ok(Tuning {
        baseline: baseline_floor,
        candidate: candidate_floor,
        shifts,
        fixed,
        introduced,
        remaining,
        verdict: gate.verdict,
        gate,
    })
}

fn persistent_signatures(series: &Series) -> BTreeSet<String> {
    series
        .faults()
        .into_iter()
        .filter(|fault| fault.persistence == Persistence::Persistent)
        .map(|fault| fault.signature)
        .collect()
}

fn shift(
    metric: &'static str,
    baseline: &Series,
    candidate: &Series,
    spread: Option<&Spread>,
    metric_value: fn(&Series) -> f64,
) -> Shift {
    Shift {
        metric,
        before: metric_value(baseline),
        after: metric_value(candidate),
        detectable: spread
            .and_then(|spread| spread.detectable(2.0, baseline.runs.len(), candidate.runs.len())),
    }
}

fn metric_faults(series: &Series) -> f64 {
    mean(series.runs.iter().map(|run| run.signatures.len() as f64))
}

fn metric_steps(series: &Series) -> f64 {
    mean(series.runs.iter().map(|run| run.steps as f64))
}

fn metric_seconds(series: &Series) -> f64 {
    mean(
        series
            .runs
            .iter()
            .map(|run| run.milliseconds as f64 / 1000.0),
    )
}

fn metric_verified(series: &Series) -> f64 {
    mean(
        series
            .runs
            .iter()
            .map(|run| run.delegations_verified as f64),
    )
}

#[allow(clippy::cast_precision_loss)]
fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Finds recorded traces under a file or directory.
pub fn traces_in(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(format!("{} is not a trace or directory", path.display()));
    }
    let mut traces = std::fs::read_dir(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry| entry.extension().and_then(|extension| extension.to_str()) == Some("jsonl"))
        .filter(|entry| {
            entry
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".atif.jsonl"))
        })
        .collect::<Vec<_>>();
    traces.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    if traces.is_empty() {
        return Err(format!("{} contains no .atif.jsonl traces", path.display()));
    }
    Ok(traces)
}
