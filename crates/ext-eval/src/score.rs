//! Scoring: runs, cases, the suite, and the change between arms.
//!
//! - A **run** scores the weighted mean of its scored graders, from 0 to 1,
//!   and passes when every scored grader passes.
//! - A **case** scores the mean of its scored runs and passes in an arm when
//!   a majority of its planned runs pass. When runs are missing and they
//!   could still tip the majority, the pass is unknown, never guessed.
//! - The **suite** reports cases passed and the mean case score per arm,
//!   the change per case and overall, and cost and time per arm. A missing
//!   cost is unknown, never zero.
//! - The **comparison** the gate reads uses compared cases only (scored in
//!   both arms), and measures the spread between repeats of the same arm.

use std::collections::{BTreeMap, BTreeSet};

use gym::gate::{ArmMeasure, ExtEvalComparison};
use serde::Serialize;

use crate::artifact::ArtifactRef;
use crate::case::{Case, Kind};
use crate::discover::Suite;
use crate::door::Doors;
use crate::grade::{GraderResult, grade_run};
use crate::record::{Arm, RunOutcome, RunRecord};

/// What runs a suite plans, and which operations the extension supplies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Whether the baseline arm runs. Without it there is no verdict.
    pub baseline: bool,
    /// Runs per arm for every case, overriding each case's `runs`.
    pub runs: Option<u32>,
    /// The operations the extension supplies, which make an
    /// `operation_used` grader subject-only by default.
    pub extension_operations: BTreeSet<String>,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            baseline: true,
            runs: None,
            extension_operations: BTreeSet::new(),
        }
    }
}

impl Plan {
    /// Runs per arm for `case`.
    #[must_use]
    pub fn runs_for(&self, case: &Case) -> u32 {
        self.runs.unwrap_or(case.runs)
    }

    /// Whether the baseline arm runs `case`: the plan runs a baseline, and
    /// the case has a grader that is not subject-only.
    #[must_use]
    pub fn baseline_runs(&self, case: &Case) -> bool {
        self.baseline && !case.subject_only(&self.extension_operations)
    }

    /// Every planned attempt, in case order, subject arm first.
    #[must_use]
    pub fn attempts(&self, suite: &Suite) -> Vec<(String, Arm, u32)> {
        let mut out = Vec::new();
        for case in &suite.cases {
            for arm in [Arm::Subject, Arm::Baseline] {
                if arm == Arm::Baseline && !self.baseline_runs(case) {
                    continue;
                }
                for attempt in 1..=self.runs_for(case) {
                    out.push((case.name.clone(), arm, attempt));
                }
            }
        }
        out
    }
}

/// What went wrong matching runs to the plan.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlanError {
    /// A record for a run the plan does not have.
    #[error("a run of `{case}` in the {arm} arm, attempt {attempt}, is not in the plan")]
    Unplanned {
        /// The case.
        case: String,
        /// The arm word.
        arm: &'static str,
        /// The attempt.
        attempt: u32,
    },
    /// Two records for one run.
    #[error("two records for `{case}` in the {arm} arm, attempt {attempt}")]
    Duplicate {
        /// The case.
        case: String,
        /// The arm word.
        arm: &'static str,
        /// The attempt.
        attempt: u32,
    },
}

/// One graded run.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GradedRun {
    /// The case name.
    pub case: String,
    /// The arm.
    pub arm: Arm,
    /// The attempt, from 1.
    pub attempt: u32,
    /// How it ended.
    #[serde(flatten)]
    pub outcome: RunOutcome,
    /// Each grader's verdict.
    pub graders: Vec<GraderResult>,
    /// The weighted mean of the scored graders; absent when not scored.
    pub score: Option<f64>,
    /// Whether every scored grader passed; absent when not scored.
    pub passed: Option<bool>,
    /// What it cost, when known.
    pub cost_usd: Option<f64>,
    /// Wall seconds, when known.
    pub seconds: Option<f64>,
    /// The trajectory's reference, when it wrote one.
    pub trajectory: Option<ArtifactRef>,
    /// The files it created.
    pub created_files: Vec<String>,
    /// Receipts it left.
    pub receipts: Vec<ArtifactRef>,
}

/// Grades every planned run. Records the plan lacks are an error; planned
/// runs with no record are graded as unknown.
///
/// # Errors
///
/// Returns [`PlanError`] for an unplanned or duplicate record.
pub fn grade_all(
    suite: &Suite,
    plan: &Plan,
    records: Vec<RunRecord>,
    doors: Doors<'_>,
) -> Result<Vec<GradedRun>, PlanError> {
    let planned: Vec<(String, Arm, u32)> = plan.attempts(suite);
    let known: BTreeSet<(&str, Arm, u32)> = planned
        .iter()
        .map(|(case, arm, attempt)| (case.as_str(), *arm, *attempt))
        .collect();
    let mut by_key: BTreeMap<(String, Arm, u32), RunRecord> = BTreeMap::new();
    for record in records {
        let key = (record.case.clone(), record.arm, record.attempt);
        if !known.contains(&(key.0.as_str(), key.1, key.2)) {
            return Err(PlanError::Unplanned {
                case: key.0,
                arm: key.1.word(),
                attempt: key.2,
            });
        }
        if by_key.contains_key(&key) {
            return Err(PlanError::Duplicate {
                case: key.0,
                arm: key.1.word(),
                attempt: key.2,
            });
        }
        by_key.insert(key, record);
    }
    let mut graded = Vec::with_capacity(planned.len());
    for (case_name, arm, attempt) in planned {
        let record = by_key
            .remove(&(case_name.clone(), arm, attempt))
            .unwrap_or_else(|| RunRecord::new(&case_name, arm, attempt, RunOutcome::Unknown));
        let Some(case) = suite.case(&case_name) else {
            continue;
        };
        let graders = grade_run(case, &record, &plan.extension_operations, doors);
        let (score, passed) = run_score(&graders, record.outcome);
        graded.push(GradedRun {
            case: record.case,
            arm,
            attempt,
            outcome: record.outcome,
            graders,
            score,
            passed,
            cost_usd: record
                .cost_usd
                .filter(|cost| cost.is_finite() && *cost >= 0.0),
            seconds: record
                .seconds
                .filter(|seconds| seconds.is_finite() && *seconds >= 0.0),
            trajectory: record.trajectory.map(|trajectory| trajectory.artifact),
            created_files: record.created_files,
            receipts: record.receipts,
        });
    }
    Ok(graded)
}

/// A run's score and pass from its graders.
fn run_score(graders: &[GraderResult], outcome: RunOutcome) -> (Option<f64>, Option<bool>) {
    if !outcome.scored() {
        return (None, None);
    }
    let scored: Vec<&GraderResult> = graders.iter().filter(|grader| grader.scored).collect();
    let weight: f64 = scored.iter().map(|grader| grader.weight).sum();
    if scored.is_empty() || weight <= 0.0 {
        return (None, None);
    }
    let earned: f64 = scored
        .iter()
        .filter(|grader| grader.passed)
        .map(|grader| grader.weight)
        .sum();
    (
        Some(earned / weight),
        Some(scored.iter().all(|grader| grader.passed)),
    )
}

/// One case in one arm.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaseArm {
    /// Runs planned.
    pub planned: u32,
    /// Runs scored (completed or errored).
    pub scored: u32,
    /// Runs that passed.
    pub runs_passed: u32,
    /// The mean score of the scored runs.
    pub score: Option<f64>,
    /// Whether a majority of the planned runs passed; unknown while
    /// missing runs could still tip it.
    pub passed: Option<bool>,
}

impl CaseArm {
    fn of(runs: &[&GradedRun], planned: u32) -> Self {
        let scored: Vec<&&GradedRun> = runs.iter().filter(|run| run.score.is_some()).collect();
        let scored_count = u32::try_from(scored.len()).unwrap_or(u32::MAX);
        let runs_passed =
            u32::try_from(scored.iter().filter(|run| run.passed == Some(true)).count())
                .unwrap_or(u32::MAX);
        let score = mean(scored.iter().filter_map(|run| run.score));
        let failed = scored_count - runs_passed;
        let passed = if runs_passed * 2 > planned {
            Some(true)
        } else if (planned - failed) * 2 <= planned {
            Some(false)
        } else {
            None
        };
        Self {
            planned,
            scored: scored_count,
            runs_passed,
            score,
            passed: if scored_count == 0 { None } else { passed },
        }
    }
}

/// One case across both arms.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaseScore {
    /// The case name.
    pub id: String,
    /// Whether the tool ought to be used.
    pub kind: Kind,
    /// Whether the case is compared: scored and decided in both arms.
    pub compared: bool,
    /// Whether every grader is subject-only, so the case scores the subject
    /// arm alone.
    pub subject_only: bool,
    /// The subject arm.
    pub subject: CaseArm,
    /// The baseline arm, when it ran the case.
    pub baseline: Option<CaseArm>,
    /// Subject score minus baseline score, for a compared case.
    pub change: Option<f64>,
}

/// NIP-EVAL coverage for one arm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// Cases planned in the arm.
    pub planned: u64,
    /// Attempts admitted: every planned run.
    pub attempted: u64,
    /// Runs that finished.
    pub completed: u64,
    /// Runs refused before they ran.
    pub refused: u64,
    /// Runs that ended with an error.
    pub failed: u64,
    /// Runs the operator stopped.
    pub cancelled: u64,
    /// Runs nobody knows the end of.
    pub unknown: u64,
    /// Cases excluded from the arm.
    pub excluded: u64,
}

/// One arm across the suite.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArmSummary {
    /// Cases scored in the arm (their pass is decided).
    pub cases_scored: usize,
    /// Cases that passed.
    pub cases_passed: usize,
    /// Cases whose pass is unknown.
    pub cases_unknown: usize,
    /// The mean case score over cases with a score.
    pub mean_score: Option<f64>,
    /// Total cost; absent when any run's cost is unknown.
    pub cost_usd: Option<f64>,
    /// Runs whose cost is unknown.
    pub cost_unknown: usize,
    /// Total wall seconds; absent when any run's time is unknown.
    pub seconds: Option<f64>,
    /// Runs whose time is unknown.
    pub seconds_unknown: usize,
    /// NIP-EVAL coverage.
    pub coverage: Coverage,
}

/// Everything scoring concludes.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Scores {
    /// Per case, in suite order.
    pub cases: Vec<CaseScore>,
    /// The subject arm.
    pub subject: ArmSummary,
    /// The baseline arm, when it ran.
    pub baseline: Option<ArmSummary>,
    /// Mean subject score minus mean baseline score over compared cases.
    pub change: Option<f64>,
    /// What the gate reads.
    pub comparison: ExtEvalComparison,
}

impl Scores {
    /// Scores graded runs against the plan.
    #[must_use]
    pub fn compute(suite: &Suite, plan: &Plan, runs: &[GradedRun], group: &str) -> Self {
        let mut cases = Vec::with_capacity(suite.cases.len());
        for case in &suite.cases {
            let planned = plan.runs_for(case);
            let of = |arm: Arm| -> Vec<&GradedRun> {
                runs.iter()
                    .filter(|run| run.case == case.name && run.arm == arm)
                    .collect()
            };
            let subject = CaseArm::of(&of(Arm::Subject), planned);
            let baseline = plan
                .baseline_runs(case)
                .then(|| CaseArm::of(&of(Arm::Baseline), planned));
            let compared = baseline.as_ref().is_some_and(|baseline| {
                baseline.score.is_some()
                    && baseline.passed.is_some()
                    && subject.score.is_some()
                    && subject.passed.is_some()
            });
            let change = if compared {
                match (subject.score, baseline.as_ref().and_then(|b| b.score)) {
                    (Some(s), Some(b)) => Some(s - b),
                    _ => None,
                }
            } else {
                None
            };
            cases.push(CaseScore {
                id: case.name.clone(),
                kind: case.kind,
                compared,
                subject_only: case.subject_only(&plan.extension_operations),
                subject,
                baseline,
                change,
            });
        }

        let subject = summary(&cases, runs, Arm::Subject, suite.cases.len());
        let baseline = plan
            .baseline
            .then(|| summary(&cases, runs, Arm::Baseline, suite.cases.len()));
        let compared: Vec<&CaseScore> = cases.iter().filter(|case| case.compared).collect();
        let change = match (
            mean(compared.iter().filter_map(|case| case.subject.score)),
            mean(
                compared
                    .iter()
                    .filter_map(|case| case.baseline.as_ref().and_then(|b| b.score)),
            ),
        ) {
            (Some(subject), Some(baseline)) => Some(subject - baseline),
            _ => None,
        };
        let comparison = comparison(group, plan, &compared, runs);
        Self {
            cases,
            subject,
            baseline,
            change,
            comparison,
        }
    }
}

fn summary(cases: &[CaseScore], runs: &[GradedRun], arm: Arm, total: usize) -> ArmSummary {
    let arms: Vec<&CaseArm> = cases
        .iter()
        .filter_map(|case| match arm {
            Arm::Subject => Some(&case.subject),
            Arm::Baseline => case.baseline.as_ref(),
        })
        .collect();
    let arm_runs: Vec<&GradedRun> = runs.iter().filter(|run| run.arm == arm).collect();
    let mut coverage = Coverage {
        planned: arms.len() as u64,
        attempted: arm_runs.len() as u64,
        excluded: (total - arms.len()) as u64,
        ..Coverage::default()
    };
    for run in &arm_runs {
        match run.outcome.coverage_word() {
            "completed" => coverage.completed += 1,
            "refused" => coverage.refused += 1,
            "failed" => coverage.failed += 1,
            "cancelled" => coverage.cancelled += 1,
            _ => coverage.unknown += 1,
        }
    }
    let cost_unknown = arm_runs.iter().filter(|run| run.cost_usd.is_none()).count();
    let seconds_unknown = arm_runs.iter().filter(|run| run.seconds.is_none()).count();
    ArmSummary {
        cases_scored: arms.iter().filter(|case| case.passed.is_some()).count(),
        cases_passed: arms.iter().filter(|case| case.passed == Some(true)).count(),
        cases_unknown: arms.iter().filter(|case| case.passed.is_none()).count(),
        mean_score: mean(arms.iter().filter_map(|case| case.score)),
        cost_usd: (cost_unknown == 0).then(|| arm_runs.iter().filter_map(|run| run.cost_usd).sum()),
        cost_unknown,
        seconds: (seconds_unknown == 0)
            .then(|| arm_runs.iter().filter_map(|run| run.seconds).sum()),
        seconds_unknown,
        coverage,
    }
}

/// The comparison the gate reads, over compared cases.
fn comparison(
    group: &str,
    plan: &Plan,
    compared: &[&CaseScore],
    runs: &[GradedRun],
) -> ExtEvalComparison {
    let ids: BTreeSet<&str> = compared.iter().map(|case| case.id.as_str()).collect();
    let fewest = compared
        .iter()
        .flat_map(|case| [Some(&case.subject), case.baseline.as_ref()])
        .flatten()
        .map(|arm| arm.scored as usize)
        .min()
        .unwrap_or(0);
    let passed = |arm: Arm| {
        compared
            .iter()
            .filter(|case| {
                let side = match arm {
                    Arm::Subject => Some(&case.subject),
                    Arm::Baseline => case.baseline.as_ref(),
                };
                side.and_then(|side| side.passed) == Some(true)
            })
            .count()
    };
    let lost = compared
        .iter()
        .filter(|case| case.kind == Kind::ShouldNotFire)
        .filter(|case| {
            case.baseline.as_ref().and_then(|b| b.passed) == Some(true)
                && case.subject.passed == Some(false)
        })
        .map(|case| case.id.clone())
        .collect();
    let series = |arm: Arm, read: fn(&GradedRun) -> Option<f64>, total: bool| {
        attempt_series(runs, &ids, arm, read, total)
    };
    let measure = |read: fn(&GradedRun) -> Option<f64>, total: bool| {
        let subject = series(Arm::Subject, read, total);
        let baseline = series(Arm::Baseline, read, total);
        ArmMeasure {
            subject: subject
                .as_deref()
                .and_then(|values| mean(values.iter().copied())),
            baseline: baseline
                .as_deref()
                .and_then(|values| mean(values.iter().copied())),
            spread: match (spread(subject.as_deref()), spread(baseline.as_deref())) {
                (Some(left), Some(right)) => Some(left.max(right)),
                _ => None,
            },
        }
    };
    ExtEvalComparison {
        group: group.to_string(),
        baseline_present: plan.baseline,
        cases: compared.len(),
        runs: fewest,
        subject_passed: passed(Arm::Subject),
        baseline_passed: passed(Arm::Baseline),
        should_not_fire_lost: lost,
        mean_score: measure(|run| run.score, false),
        cost_usd: measure(|run| run.cost_usd, true),
        seconds: measure(|run| run.seconds, true),
    }
}

/// Per attempt, over the compared cases: the mean of a run measure (a
/// score), or its total (a cost or a time). `None` when any compared case
/// is missing the measure at some attempt it planned, for a total: a
/// partial sum is not a total.
fn attempt_series(
    runs: &[GradedRun],
    cases: &BTreeSet<&str>,
    arm: Arm,
    read: fn(&GradedRun) -> Option<f64>,
    total: bool,
) -> Option<Vec<f64>> {
    let mut by_attempt: BTreeMap<u32, Vec<Option<f64>>> = BTreeMap::new();
    for run in runs
        .iter()
        .filter(|run| run.arm == arm && cases.contains(run.case.as_str()))
    {
        by_attempt.entry(run.attempt).or_default().push(read(run));
    }
    let mut out = Vec::new();
    for values in by_attempt.values() {
        if total {
            if values.iter().any(Option::is_none) {
                return None;
            }
            out.push(values.iter().flatten().sum());
        } else if let Some(value) = mean(values.iter().flatten().copied()) {
            out.push(value);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// The spread between repeats: the highest minus the lowest. None for
/// fewer than two repeats, which have no spread.
///
/// The `ext-eval-v1` gate's `min_runs` bound cites this function.
#[must_use]
pub fn spread(values: Option<&[f64]>) -> Option<f64> {
    let values = values?;
    if values.len() < 2 {
        return None;
    }
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    Some(high - low)
}

#[allow(clippy::cast_precision_loss)]
fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, count) = values.fold((0.0, 0usize), |(sum, count), value| {
        (sum + value, count + 1)
    });
    (count > 0).then(|| sum / count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spread_needs_two_repeats() {
        assert_eq!(spread(None), None);
        assert_eq!(spread(Some(&[0.5])), None);
        assert_eq!(spread(Some(&[0.5, 0.75, 0.25])), Some(0.5));
    }

    fn run(passed: Option<bool>) -> GradedRun {
        GradedRun {
            case: "c".into(),
            arm: Arm::Subject,
            attempt: 1,
            outcome: if passed.is_some() {
                RunOutcome::Completed
            } else {
                RunOutcome::Unknown
            },
            graders: Vec::new(),
            score: passed.map(|p| if p { 1.0 } else { 0.0 }),
            passed,
            cost_usd: None,
            seconds: None,
            trajectory: None,
            created_files: Vec::new(),
            receipts: Vec::new(),
        }
    }

    #[test]
    fn a_case_passes_on_a_majority_of_planned_runs() {
        let (p, f, u) = (run(Some(true)), run(Some(false)), run(None));
        assert_eq!(CaseArm::of(&[&p, &p, &f], 3).passed, Some(true));
        assert_eq!(CaseArm::of(&[&p, &f, &f], 3).passed, Some(false));
        // One pass, one fail, one unknown: the unknown run could tip it.
        assert_eq!(CaseArm::of(&[&p, &f, &u], 3).passed, None);
        // Two passes of three decide it whatever the third did.
        assert_eq!(CaseArm::of(&[&p, &p, &u], 3).passed, Some(true));
        // Two fails of three decide it too.
        assert_eq!(CaseArm::of(&[&f, &f, &u], 3).passed, Some(false));
        // Nothing scored is unknown.
        assert_eq!(CaseArm::of(&[&u, &u, &u], 3).passed, None);
        // An even split of an even count is not a majority.
        assert_eq!(CaseArm::of(&[&p, &f], 2).passed, Some(false));
    }
}
