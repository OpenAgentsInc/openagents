//! Candidate moves: strategy differences that repeat across tasks.
//!
//! For each task, [`moves`] splits the task's fingerprints into two groups,
//! such as Fable's winners and Luna's attempts, and measures each feature's
//! difference with Cliff's delta: the chance that a trajectory from the
//! first group has the larger value, minus the chance that it has the
//! smaller one. A difference that points the same way in enough tasks is a
//! candidate move, reported with its task count, its mean effect, and a
//! citation per task: a run and step from each side.
//!
//! A candidate move is a hypothesis for a System One algorithm to test, not
//! a finding. Trajectories on one task share little beyond the task, and
//! the comparison controls nothing else.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::runs_fingerprint::Fingerprint;

/// A feature of a fingerprint the comparison measures.
pub struct Feature {
    pub id: &'static str,
    /// What a larger value means, in words.
    pub what: &'static str,
    pub value: fn(&Fingerprint) -> Option<f64>,
    /// The step that shows the feature, when one does.
    pub cite: fn(&Fingerprint) -> Option<usize>,
}

fn share(count: usize, of: usize) -> Option<f64> {
    (of > 0).then(|| count as f64 / of as f64)
}

fn phase(print: &Fingerprint, name: &str) -> usize {
    print.phases.get(name).copied().unwrap_or(0)
}

fn flag(value: bool) -> Option<f64> {
    Some(if value { 1.0 } else { 0.0 })
}

/// The features, in the order the report lists them.
pub const FEATURES: [Feature; 21] = [
    Feature {
        id: "runs_example_before_editing",
        what: "runs the task's own example before its first edit",
        value: |p| {
            p.task_has_example
                .then(|| flag(p.ran_example_before_edit))
                .flatten()
        },
        cite: |p| p.example_step,
    },
    Feature {
        id: "runs_before_editing",
        what: "runs a test, script, or program before its first edit",
        value: |p| flag(p.ran_before_edit),
        cite: |p| p.ran_before_edit_step,
    },
    Feature {
        id: "reproduces_before_editing",
        what: "tests or verifies before its first edit",
        value: |p| flag(p.verification.before_first_edit > 0),
        cite: |p| p.first_test_step,
    },
    Feature {
        id: "share_before_first_edit",
        what: "spends a larger share of its steps before the first edit",
        value: |p| p.first_edit_share,
        cite: |p| p.first_edit_step,
    },
    Feature {
        id: "minutes_before_first_edit",
        what: "spends more minutes before the first edit",
        value: |p| p.first_edit_ms.map(|ms| ms as f64 / 60_000.0),
        cite: |p| p.first_edit_step,
    },
    Feature {
        id: "test_rate",
        what: "tests on a larger share of its steps",
        value: |p| share(p.tests, p.steps),
        cite: |p| p.first_test_step,
    },
    Feature {
        id: "tests_per_edit",
        what: "runs more tests per edit",
        value: |p| p.tests_per_edit,
        cite: |p| p.first_test_step,
    },
    Feature {
        id: "verification_rate",
        what: "tests or verifies on a larger share of its steps",
        value: |p| share(p.verification.total, p.steps),
        cite: |p| p.first_test_step,
    },
    Feature {
        id: "verifies_after_last_edit",
        what: "tests or verifies after its last edit",
        value: |p| {
            p.first_edit_step.map(|_| {
                if p.verification.after_last_edit > 0 {
                    1.0
                } else {
                    0.0
                }
            })
        },
        cite: |p| p.verification.after_last_edit_step,
    },
    Feature {
        id: "late_verification_share",
        what: "puts a larger share of its verification in the last quarter",
        value: |p| p.verification.last_quarter_share,
        cite: |p| p.verification.after_last_edit_step,
    },
    Feature {
        id: "edits_checked_share",
        what: "checks each run of edits before the next one",
        value: |p| p.verification.edits_checked_share,
        cite: |p| p.first_edit_step,
    },
    Feature {
        id: "edit_runs",
        what: "edits in more separate rounds",
        value: |p| Some(p.edit_runs as f64),
        cite: |p| p.first_edit_step,
    },
    Feature {
        id: "retry_rate",
        what: "repeats failed steps more often",
        value: |p| share(p.retries.repeated_failed + p.retries.by_jev, p.steps),
        cite: |p| p.retries.first_step,
    },
    Feature {
        id: "failed_step_rate",
        what: "has more of its steps fail",
        value: |p| share(p.retries.failed_steps, p.steps),
        cite: |_| None,
    },
    Feature {
        id: "files_touched",
        what: "touches more files",
        value: |p| Some(p.files_touched as f64),
        cite: |p| p.first_edit_step,
    },
    Feature {
        id: "steps",
        what: "takes more steps",
        value: |p| Some(p.steps as f64),
        cite: |_| None,
    },
    Feature {
        id: "read_share",
        what: "spends a larger share of its steps reading",
        value: |p| share(phase(p, "read"), p.steps),
        cite: |_| None,
    },
    Feature {
        id: "orient_share",
        what: "spends a larger share of its steps orienting",
        value: |p| share(phase(p, "orient"), p.steps),
        cite: |_| None,
    },
    Feature {
        id: "writes_a_plan",
        what: "writes a plan or to-do list",
        value: |p| flag(phase(p, "plan") > 0),
        cite: |_| None,
    },
    Feature {
        id: "checks_assumptions",
        what: "has more steps Jev judges to check an assumption",
        value: |p| share(p.placement.checks_assumption, p.steps),
        cite: |_| None,
    },
    Feature {
        id: "uses_earlier_evidence",
        what: "has more steps Jev judges to act on earlier evidence",
        value: |p| share(p.placement.uses_evidence, p.steps),
        cite: |_| None,
    },
];

/// One group's fingerprints on a task, with a feature's value for each.
type Side<'a> = Vec<(&'a Fingerprint, f64)>;

/// Two groups of fingerprints to compare within each task.
pub struct Comparison {
    pub id: &'static str,
    pub a: &'static str,
    pub b: &'static str,
    pub in_a: fn(&Fingerprint) -> bool,
    pub in_b: fn(&Fingerprint) -> bool,
}

/// The comparisons, in report order.
pub const COMPARISONS: [Comparison; 4] = [
    Comparison {
        id: "fable-winners-vs-luna",
        a: "Fable winners",
        b: "Luna",
        in_a: |p| p.group == "fable" && p.passed == Some(true),
        in_b: |p| p.group == "luna" && p.passed.is_some(),
    },
    Comparison {
        id: "fable-winners-vs-coder-one",
        a: "Fable winners",
        b: "Coder One",
        in_a: |p| p.group == "fable" && p.passed == Some(true),
        in_b: |p| p.group == "coder-one" && p.passed.is_some(),
    },
    Comparison {
        id: "fable-winners-vs-fable-losers",
        a: "Fable winners",
        b: "Fable losers",
        in_a: |p| p.group == "fable" && p.passed == Some(true),
        in_b: |p| p.group == "fable" && p.passed == Some(false),
    },
    Comparison {
        id: "winners-vs-losers",
        a: "All winners",
        b: "All losers",
        in_a: |p| p.passed == Some(true),
        in_b: |p| p.passed == Some(false),
    },
];

/// Cliff's delta: P(a > b) − P(a < b) over all pairs.
#[must_use]
pub fn cliffs_delta(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut score = 0i64;
    for x in a {
        for y in b {
            score += match x.partial_cmp(y) {
                Some(std::cmp::Ordering::Greater) => 1,
                Some(std::cmp::Ordering::Less) => -1,
                _ => 0,
            };
        }
    }
    score as f64 / (a.len() * b.len()) as f64
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len().max(1) as f64
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

/// One task's effect.
#[derive(Clone, Debug, Serialize)]
pub struct TaskEffect {
    pub task: String,
    pub delta: f64,
    pub n_a: usize,
    pub n_b: usize,
    pub mean_a: f64,
    pub mean_b: f64,
    pub median_a: f64,
    pub median_b: f64,
}

/// A run and step that show a move, one from each side.
#[derive(Clone, Debug, Serialize)]
pub struct Citation {
    pub task: String,
    pub a_run: String,
    pub a_step: Option<usize>,
    pub a_value: f64,
    pub b_run: String,
    pub b_step: Option<usize>,
    pub b_value: f64,
}

/// One feature's difference across tasks.
#[derive(Clone, Debug, Serialize)]
pub struct Move {
    pub comparison: &'static str,
    pub a: &'static str,
    pub b: &'static str,
    pub feature: &'static str,
    pub what: &'static str,
    /// `more` when the first group does it more.
    pub direction: &'static str,
    pub tasks_with_data: usize,
    pub tasks_agreeing: usize,
    /// The mean of the per-task Cliff's deltas.
    pub mean_delta: f64,
    /// The median over tasks of each group's per-task median.
    pub typical_a: Option<f64>,
    pub typical_b: Option<f64>,
    /// Tasks agreeing times the mean delta's size.
    pub score: f64,
    pub candidate: bool,
    pub per_task: Vec<TaskEffect>,
    pub citations: Vec<Citation>,
}

/// How many fingerprints each comparison had.
#[derive(Clone, Debug, Serialize)]
pub struct Coverage {
    pub comparison: &'static str,
    pub tasks: usize,
    pub a: usize,
    pub b: usize,
}

/// The moves report.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub min_tasks: usize,
    pub coverage: Vec<Coverage>,
    /// Candidate moves, strongest first.
    pub candidates: Vec<Move>,
    /// Every other feature difference, strongest first, without citations.
    pub others: Vec<Move>,
}

/// Compares fingerprints per task and ranks the differences that repeat.
#[must_use]
pub fn moves(prints: &[Fingerprint], tasks: &[String], min_tasks: usize) -> Report {
    let mut by_task: BTreeMap<&str, Vec<&Fingerprint>> = BTreeMap::new();
    for print in prints {
        if tasks.contains(&print.task) {
            by_task.entry(print.task.as_str()).or_default().push(print);
        }
    }
    let mut coverage = Vec::new();
    let mut all: Vec<Move> = Vec::new();
    for comparison in &COMPARISONS {
        let mut cover = Coverage {
            comparison: comparison.id,
            tasks: 0,
            a: 0,
            b: 0,
        };
        for list in by_task.values() {
            let a = list.iter().filter(|p| (comparison.in_a)(p)).count();
            let b = list.iter().filter(|p| (comparison.in_b)(p)).count();
            if a > 0 && b > 0 {
                cover.tasks += 1;
                cover.a += a;
                cover.b += b;
            }
        }
        coverage.push(cover);
        for feature in &FEATURES {
            let mut effects = Vec::new();
            let mut sides: Vec<(Side, Side)> = Vec::new();
            for (task, list) in &by_task {
                let a: Vec<(&Fingerprint, f64)> = list
                    .iter()
                    .filter(|p| (comparison.in_a)(p))
                    .filter_map(|p| Some((*p, (feature.value)(p)?)))
                    .collect();
                let b: Vec<(&Fingerprint, f64)> = list
                    .iter()
                    .filter(|p| (comparison.in_b)(p))
                    .filter_map(|p| Some((*p, (feature.value)(p)?)))
                    .collect();
                if a.is_empty() || b.is_empty() {
                    continue;
                }
                let av: Vec<f64> = a.iter().map(|(_, v)| *v).collect();
                let bv: Vec<f64> = b.iter().map(|(_, v)| *v).collect();
                effects.push(TaskEffect {
                    task: (*task).to_owned(),
                    delta: cliffs_delta(&av, &bv),
                    n_a: av.len(),
                    n_b: bv.len(),
                    mean_a: mean(&av),
                    mean_b: mean(&bv),
                    median_a: median(&mut av.clone()).unwrap_or_default(),
                    median_b: median(&mut bv.clone()).unwrap_or_default(),
                });
                sides.push((a, b));
            }
            if effects.is_empty() {
                continue;
            }
            let mean_delta = mean(&effects.iter().map(|e| e.delta).collect::<Vec<_>>());
            let positive = mean_delta >= 0.0;
            let agreeing = effects
                .iter()
                .filter(|e| {
                    if positive {
                        e.delta > 0.0
                    } else {
                        e.delta < 0.0
                    }
                })
                .count();
            let candidate = agreeing >= min_tasks
                && agreeing * 3 >= effects.len() * 2
                && mean_delta.abs() >= 0.2;
            let mut citations = Vec::new();
            if candidate {
                for (effect, (a, b)) in effects.iter().zip(&sides) {
                    let agrees = if positive {
                        effect.delta > 0.0
                    } else {
                        effect.delta < 0.0
                    };
                    if !agrees {
                        continue;
                    }
                    // The clearest pair: the extreme on each side, the one
                    // with a step to show first.
                    if let (Some((pa, va)), Some((pb, vb))) =
                        (pick(a, positive, feature), pick(b, !positive, feature))
                    {
                        citations.push(Citation {
                            task: effect.task.clone(),
                            a_run: pa.run.clone(),
                            a_step: (feature.cite)(pa),
                            a_value: va,
                            b_run: pb.run.clone(),
                            b_step: (feature.cite)(pb),
                            b_value: vb,
                        });
                    }
                }
            }
            let score = agreeing as f64 * mean_delta.abs();
            all.push(Move {
                comparison: comparison.id,
                a: comparison.a,
                b: comparison.b,
                feature: feature.id,
                what: feature.what,
                direction: if positive { "more" } else { "less" },
                tasks_with_data: effects.len(),
                tasks_agreeing: agreeing,
                mean_delta,
                typical_a: median(&mut effects.iter().map(|e| e.median_a).collect::<Vec<_>>()),
                typical_b: median(&mut effects.iter().map(|e| e.median_b).collect::<Vec<_>>()),
                score,
                candidate,
                per_task: effects,
                citations,
            });
        }
    }
    all.sort_by(|x, y| y.score.total_cmp(&x.score));
    let (candidates, mut others): (Vec<Move>, Vec<Move>) =
        all.into_iter().partition(|m| m.candidate);
    for other in &mut others {
        other.citations.clear();
    }
    Report {
        min_tasks,
        coverage,
        candidates,
        others,
    }
}

/// The side's extreme trajectory, preferring one with a step to cite.
fn pick<'a>(
    side: &[(&'a Fingerprint, f64)],
    high: bool,
    feature: &Feature,
) -> Option<(&'a Fingerprint, f64)> {
    side.iter()
        .max_by(|x, y| {
            let order = if high {
                x.1.total_cmp(&y.1)
            } else {
                y.1.total_cmp(&x.1)
            };
            order.then_with(|| {
                (feature.cite)(x.0)
                    .is_some()
                    .cmp(&(feature.cite)(y.0).is_some())
            })
        })
        .map(|(p, v)| (*p, *v))
}

fn number(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 && value.abs() >= 1.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

/// The text `gym runs moves` prints.
#[must_use]
pub fn render(report: &Report) -> Vec<String> {
    let mut out = vec!["Coverage".to_owned()];
    for cover in &report.coverage {
        out.push(format!(
            "  {:<30} {} tasks with both sides · {} vs {} trajectories",
            cover.comparison, cover.tasks, cover.a, cover.b
        ));
    }
    out.push(String::new());
    out.push(format!(
        "Candidate moves: a difference in at least {} tasks and two-thirds of the tasks with data, mean Cliff's delta at least 0.20",
        report.min_tasks
    ));
    if report.candidates.is_empty() {
        out.push("  none".to_owned());
    }
    for (rank, one) in report.candidates.iter().enumerate() {
        out.push(format!(
            "{:>3}. {} · {} {} {}: {} of {} tasks, mean delta {:+.2}, typical {} vs {}",
            rank + 1,
            one.comparison,
            one.a,
            if one.direction == "more" {
                "does more:"
            } else {
                "does less:"
            },
            one.what,
            one.tasks_agreeing,
            one.tasks_with_data,
            one.mean_delta,
            one.typical_a.map_or("-".to_owned(), number),
            one.typical_b.map_or("-".to_owned(), number),
        ));
        for cite in &one.citations {
            out.push(format!(
                "       {}: {}{} ({}) vs {}{} ({})",
                cite.task,
                cite.a_run,
                cite.a_step.map_or(String::new(), |n| format!(" step {n}")),
                number(cite.a_value),
                cite.b_run,
                cite.b_step.map_or(String::new(), |n| format!(" step {n}")),
                number(cite.b_value),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cliffs_delta_counts_pairs() {
        assert_eq!(cliffs_delta(&[1.0, 1.0], &[0.0, 0.0]), 1.0);
        assert_eq!(cliffs_delta(&[0.0], &[1.0]), -1.0);
        assert_eq!(cliffs_delta(&[1.0, 0.0], &[1.0, 0.0]), 0.0);
    }
}
