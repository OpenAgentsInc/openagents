//! The report of a targeted Terminal-Bench experiment.
//!
//! `tbench experiment run` runs two or more arms over a few tasks, several
//! attempts per task per arm, interleaved, and writes a status file with
//! every trial's outcome, the trials lost and rerun, and the Claude quota
//! each drew. This reads that file and says what it supports:
//!
//! - each arm's passes over its graded attempts, with a 95% Wilson
//!   interval ([`crate::coder_matrix::wilson`]);
//! - a paired comparison of every arm against the first: an exact McNemar
//!   test on the attempts paired by task and attempt number, and an exact
//!   sign test on the tasks, by which arm passed more of its attempts;
//! - the attempts lost to credentials, quota, or infrastructure, which
//!   never enter a denominator;
//! - the Claude quota used against the experiment's budget.
//!
//! A graded attempt is a finished trial with a verifier reward. A finished
//! trial without one is ungraded, and a trial that never finished isn't
//! run; neither counts. The p-values are two-sided and exact, with no
//! correction for comparing several arms.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::coder_matrix::wilson;

/// The schema of `gym terminal-bench experiment report --json`.
pub const SCHEMA: &str = "openagents.gym.terminal-bench-experiment.v1";

/// The status file schema `tbench experiment` writes.
pub const STATUS_SCHEMA: &str = "openagents.tbench.experiment-status.v1";

/// Why a trial was set aside and run again.
pub const LOSS_CAUSES: [&str; 3] = ["credentials", "quota", "infrastructure"];

/// One scheduled attempt: an arm, a task, and an attempt number.
#[derive(Clone, Debug, PartialEq)]
pub struct Trial {
    pub arm: String,
    pub task: String,
    pub attempt: u64,
    pub job: String,
    pub state: String,
    pub reward: Option<f64>,
    pub quota_usd: f64,
    /// Each loss's cause and the quota it drew.
    pub losses: Vec<(String, f64)>,
}

impl Trial {
    /// A finished trial with a verifier reward.
    #[must_use]
    pub fn graded(&self) -> bool {
        self.state == "finished" && self.reward.is_some()
    }

    #[must_use]
    pub fn passed(&self) -> bool {
        self.graded() && self.reward.is_some_and(|reward| reward >= 1.0)
    }
}

/// One arm's attempts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArmSummary {
    pub arm: String,
    pub scheduled: usize,
    pub graded: usize,
    pub passes: usize,
    pub interval: (f64, f64),
    pub ungraded: usize,
    pub not_run: usize,
    pub lost: BTreeMap<String, usize>,
    pub quota_usd: f64,
}

/// One arm against the baseline, paired.
#[derive(Clone, Debug, PartialEq)]
pub struct Paired {
    pub baseline: String,
    pub arm: String,
    /// Attempts graded in both arms, paired by task and attempt number.
    pub pairs: usize,
    pub both_pass: usize,
    pub baseline_only: usize,
    pub arm_only: usize,
    pub both_fail: usize,
    pub mcnemar_p: f64,
    /// Tasks with graded attempts in both arms.
    pub tasks: usize,
    pub tasks_arm_better: usize,
    pub tasks_baseline_better: usize,
    pub tasks_tied: usize,
    pub sign_p: f64,
}

/// The whole report.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub id: String,
    pub profile: String,
    pub source: Option<PathBuf>,
    pub arms: Vec<String>,
    pub tasks: Vec<String>,
    pub attempts: u64,
    pub state: String,
    pub credential_source: Option<String>,
    pub updated_at: Option<String>,
    pub budget_usd: Option<f64>,
    pub quota_used_usd: f64,
    pub unpriced_sessions: u64,
    pub trials: Vec<Trial>,
    pub summaries: Vec<ArmSummary>,
    pub paired: Vec<Paired>,
}

/// The two-sided exact binomial test of `k` successes in `n` at one half.
#[must_use]
pub fn binomial_two_sided(k: usize, n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let low = k.min(n - k);
    // P(X <= low) for X ~ Binomial(n, 1/2). The coefficients are exact in
    // f64 up to n = 50 or so, and 2^-n is exact, so small tests are exact.
    if n <= 1000 {
        let mut choose = 1.0_f64;
        let mut sum = 1.0_f64;
        for i in 1..=low {
            choose = choose * (n - i + 1) as f64 / i as f64;
            sum += choose;
        }
        let scale = i32::try_from(n).map_or(0.0, |n| 0.5_f64.powi(n));
        return (2.0 * sum * scale).min(1.0);
    }
    // Past that, sum in log space.
    let ln_half_n = n as f64 * 0.5_f64.ln();
    let mut ln_choose = 0.0_f64;
    let mut tail = 0.0_f64;
    for i in 0..=low {
        if i > 0 {
            ln_choose += ((n - i + 1) as f64).ln() - (i as f64).ln();
        }
        tail += (ln_choose + ln_half_n).exp();
    }
    (2.0 * tail).min(1.0)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(str::to_owned)
                        .or_else(|| text(item, "id"))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn usd(value: Option<&Value>) -> f64 {
    value
        .and_then(|usage| usage.get("usd"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

impl Report {
    /// Read a status file `tbench experiment` wrote.
    ///
    /// # Errors
    /// When the file can't be read or isn't an experiment status file.
    pub fn read(path: &Path) -> Result<Self, String> {
        let body = std::fs::read_to_string(path)
            .map_err(|error| format!("can't read {}: {error}", path.display()))?;
        let value: Value = serde_json::from_str(&body)
            .map_err(|error| format!("{} isn't JSON: {error}", path.display()))?;
        let mut report = Self::from_status(&value)?;
        report.source = Some(path.to_path_buf());
        Ok(report)
    }

    /// Build the report from a parsed status document.
    ///
    /// # Errors
    /// When the document isn't an experiment status file.
    pub fn from_status(value: &Value) -> Result<Self, String> {
        if value.get("schema").and_then(Value::as_str) != Some(STATUS_SCHEMA) {
            return Err(format!("not a {STATUS_SCHEMA} document"));
        }
        let arms = strings(value, "arms");
        if arms.len() < 2 {
            return Err("an experiment status file names at least two arms".to_owned());
        }
        let trials: Vec<Trial> = value
            .get("trials")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| Trial {
                        arm: text(row, "arm").unwrap_or_default(),
                        task: text(row, "task").unwrap_or_default(),
                        attempt: row.get("attempt").and_then(Value::as_u64).unwrap_or(0),
                        job: text(row, "job").unwrap_or_default(),
                        state: text(row, "state").unwrap_or_else(|| "unknown".to_owned()),
                        reward: row.get("reward").and_then(Value::as_f64),
                        quota_usd: usd(row.get("claude_usage")),
                        losses: row
                            .get("losses")
                            .and_then(Value::as_array)
                            .map(|losses| {
                                losses
                                    .iter()
                                    .map(|loss| {
                                        (
                                            text(loss, "cause")
                                                .unwrap_or_else(|| "infrastructure".to_owned()),
                                            usd(loss.get("usage")),
                                        )
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let quota = value.get("quota").cloned().unwrap_or(Value::Null);
        let used = quota.get("used");
        let summaries = arms.iter().map(|arm| summarize(arm, &trials)).collect();
        let paired = arms[1..]
            .iter()
            .map(|arm| pair(&arms[0], arm, &trials))
            .collect();
        Ok(Self {
            id: text(value, "experiment").unwrap_or_default(),
            profile: text(value, "profile").unwrap_or_default(),
            source: None,
            tasks: strings(value, "tasks"),
            attempts: value.get("attempts").and_then(Value::as_u64).unwrap_or(0),
            state: text(value, "state").unwrap_or_else(|| "unknown".to_owned()),
            credential_source: text(value, "credential_source"),
            updated_at: text(value, "updated_at"),
            budget_usd: quota.get("budget_usd").and_then(Value::as_f64),
            quota_used_usd: usd(used),
            unpriced_sessions: used
                .and_then(|usage| usage.get("unpriced"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            arms,
            trials,
            summaries,
            paired,
        })
    }

    /// Every scheduled attempt is graded.
    #[must_use]
    pub fn complete(&self) -> bool {
        !self.trials.is_empty() && self.trials.iter().all(Trial::graded)
    }

    /// Attempts lost and rerun, by cause.
    #[must_use]
    pub fn losses(&self) -> BTreeMap<String, usize> {
        let mut losses: BTreeMap<String, usize> = LOSS_CAUSES
            .iter()
            .map(|cause| ((*cause).to_owned(), 0))
            .collect();
        for trial in &self.trials {
            for (cause, _) in &trial.losses {
                *losses.entry(cause.clone()).or_default() += 1;
            }
        }
        losses
    }

    /// The fewest graded attempts any arm has on any task.
    #[must_use]
    pub fn min_graded_per_cell(&self) -> usize {
        self.tasks
            .iter()
            .flat_map(|task| {
                self.arms.iter().map(move |arm| {
                    self.trials
                        .iter()
                        .filter(|t| &t.arm == arm && &t.task == task && t.graded())
                        .count()
                })
            })
            .min()
            .unwrap_or(0)
    }

    /// One task's passes over graded attempts per arm, in arm order.
    #[must_use]
    pub fn task_cells(&self, task: &str) -> Vec<(usize, usize)> {
        self.arms
            .iter()
            .map(|arm| {
                let graded: Vec<&Trial> = self
                    .trials
                    .iter()
                    .filter(|t| &t.arm == arm && t.task == task && t.graded())
                    .collect();
                (graded.iter().filter(|t| t.passed()).count(), graded.len())
            })
            .collect()
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "experiment": self.id,
            "profile": self.profile,
            "source": self.source,
            "state": self.state,
            "updated_at": self.updated_at,
            "arms": self.arms,
            "tasks": self.tasks,
            "attempts_per_task_per_arm": self.attempts,
            "credential_source": self.credential_source,
            "complete": self.complete(),
            "min_graded_attempts_per_task_per_arm": self.min_graded_per_cell(),
            "lost_and_rerun": self.losses(),
            "quota": {
                "budget_usd": self.budget_usd,
                "used_usd": self.quota_used_usd,
                "unpriced_sessions": self.unpriced_sessions,
                "measure": "Claude Code total_cost_usd over graded and lost trials: a list-price value, not a cash charge",
            },
            "arm_results": self.summaries.iter().map(|s| json!({
                "arm": s.arm,
                "scheduled": s.scheduled,
                "graded": s.graded,
                "passes": s.passes,
                "pass_rate": (s.graded > 0).then(|| s.passes as f64 / s.graded as f64),
                "wilson_95": [s.interval.0, s.interval.1],
                "ungraded": s.ungraded,
                "not_run": s.not_run,
                "lost_and_rerun": s.lost,
                "claude_quota_usd": s.quota_usd,
            })).collect::<Vec<_>>(),
            "tasks_by_arm": self.tasks.iter().map(|task| json!({
                "task": task,
                "cells": self.task_cells(task).iter().zip(&self.arms).map(|((passes, graded), arm)| json!({
                    "arm": arm, "passes": passes, "graded": graded,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "paired": self.paired.iter().map(|p| json!({
                "baseline": p.baseline,
                "arm": p.arm,
                "pairs": p.pairs,
                "both_pass": p.both_pass,
                "baseline_only": p.baseline_only,
                "arm_only": p.arm_only,
                "both_fail": p.both_fail,
                "mcnemar_exact_p": p.mcnemar_p,
                "tasks": p.tasks,
                "tasks_arm_better": p.tasks_arm_better,
                "tasks_baseline_better": p.tasks_baseline_better,
                "tasks_tied": p.tasks_tied,
                "sign_test_exact_p": p.sign_p,
            })).collect::<Vec<_>>(),
        })
    }

    /// The report as terminal text.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "experiment {} ({}): {} · {} tasks × {} attempts per arm · {}",
                self.id,
                self.profile,
                self.arms.join(" vs "),
                self.tasks.len(),
                self.attempts,
                self.state
            ),
            format!(
                "  credential {} · Claude quota {}",
                self.credential_source.as_deref().unwrap_or("none needed"),
                self.quota_text()
            ),
        ];
        for s in &self.summaries {
            lines.push(format!(
                "  {:<32} {:>3}/{:<3} graded  {:>5}  95% {}  ungraded {}  not run {}  lost {}  quota ${:.2}",
                s.arm,
                s.passes,
                s.graded,
                rate(s.passes, s.graded),
                interval_text(s),
                s.ungraded,
                s.not_run,
                loss_text(&s.lost),
                s.quota_usd,
            ));
        }
        for p in &self.paired {
            lines.push(format!(
                "  paired {} vs {}: {} pairs, both pass {}, only {} {}, only {} {}, both fail {}; exact McNemar p = {}",
                p.arm,
                p.baseline,
                p.pairs,
                p.both_pass,
                p.arm,
                p.arm_only,
                p.baseline,
                p.baseline_only,
                p.both_fail,
                p_text(p.mcnemar_p),
            ));
            lines.push(format!(
                "    by task: {} better on {}, {} better on {}, tied on {} of {}; sign test p = {}",
                p.arm,
                p.tasks_arm_better,
                p.baseline,
                p.tasks_baseline_better,
                p.tasks_tied,
                p.tasks,
                p_text(p.sign_p),
            ));
        }
        lines.push(format!(
            "  {} · fewest graded attempts on a task and arm: {} of {}",
            if self.complete() {
                "complete: every scheduled attempt graded"
            } else {
                "incomplete: some scheduled attempts aren't graded"
            },
            self.min_graded_per_cell(),
            self.attempts
        ));
        lines.push(
            "  Lost attempts ran again and never count; development observations, not promotion results."
                .to_owned(),
        );
        lines
    }

    fn quota_text(&self) -> String {
        let mut out = format!("${:.2}", self.quota_used_usd);
        if let Some(budget) = self.budget_usd {
            out.push_str(&format!(" of a ${budget:.2} budget"));
        } else {
            out.push_str(" (no budget)");
        }
        if self.unpriced_sessions > 0 {
            out.push_str(&format!(
                ", a lower bound: {} sessions reported no value",
                self.unpriced_sessions
            ));
        }
        out
    }

    /// The results sections of the targeted-experiment template, filled.
    #[must_use]
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("## Design\n\n");
        out.push_str("| Field | Value |\n| --- | --- |\n");
        out.push_str(&format!("| Experiment | `{}` |\n", self.id));
        out.push_str(&format!("| Profile | `{}` |\n", self.profile));
        out.push_str(&format!(
            "| Arms | {} (baseline first) |\n",
            self.arms
                .iter()
                .map(|arm| format!("`{arm}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str(&format!("| Tasks | {} |\n", self.tasks.len()));
        out.push_str(&format!(
            "| Attempts per task per arm | {}, interleaved |\n",
            self.attempts
        ));
        out.push_str(&format!(
            "| Claude credential | {} |\n",
            self.credential_source.as_deref().unwrap_or("none needed")
        ));
        out.push_str(&format!(
            "| Claude quota | {} |\n",
            self.quota_text().replace('$', "\\$")
        ));
        out.push_str(&format!(
            "| State | {}{} |\n\n",
            self.state,
            self.updated_at
                .as_deref()
                .map(|at| format!(", updated {at}"))
                .unwrap_or_default()
        ));
        out.push_str("## Pass rates\n\n");
        out.push_str("| Arm | Passes / graded | Pass rate | 95% Wilson interval | Ungraded | Not run | Lost and rerun | Claude quota |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
        for s in &self.summaries {
            out.push_str(&format!(
                "| `{}` | {} / {} | {} | {} | {} | {} | {} | \\${:.2} |\n",
                s.arm,
                s.passes,
                s.graded,
                rate(s.passes, s.graded),
                interval_text(s),
                s.ungraded,
                s.not_run,
                loss_text(&s.lost),
                s.quota_usd
            ));
        }
        out.push_str("\n## Paired comparison\n\n");
        out.push_str("| Comparison | Pairs | Both pass | Only the arm | Only the baseline | Both fail | Exact McNemar p | Tasks: arm better / baseline better / tied | Sign test p |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
        for p in &self.paired {
            out.push_str(&format!(
                "| `{}` vs `{}` | {} | {} | {} | {} | {} | {} | {} / {} / {} | {} |\n",
                p.arm,
                p.baseline,
                p.pairs,
                p.both_pass,
                p.arm_only,
                p.baseline_only,
                p.both_fail,
                p_text(p.mcnemar_p),
                p.tasks_arm_better,
                p.tasks_baseline_better,
                p.tasks_tied,
                p_text(p.sign_p)
            ));
        }
        out.push_str("\n## Per task\n\n| Task |");
        for arm in &self.arms {
            out.push_str(&format!(" `{arm}` |"));
        }
        out.push_str("\n| --- |");
        for _ in &self.arms {
            out.push_str(" --- |");
        }
        out.push('\n');
        for task in &self.tasks {
            out.push_str(&format!("| `{task}` |"));
            for (passes, graded) in self.task_cells(task) {
                out.push_str(&format!(" {passes} / {graded} |"));
            }
            out.push('\n');
        }
        out.push_str("\n## Completeness\n\n");
        let losses = self.losses();
        out.push_str(&format!(
            "- {}. The fewest graded attempts on any task and arm is {} of {}.\n",
            if self.complete() {
                "Every scheduled attempt is graded"
            } else {
                "Some scheduled attempts aren't graded"
            },
            self.min_graded_per_cell(),
            self.attempts
        ));
        out.push_str(&format!(
            "- Lost and rerun: {} to credentials, {} to quota, {} to infrastructure. \
             No lost attempt is in a denominator.\n",
            losses.get("credentials").copied().unwrap_or(0),
            losses.get("quota").copied().unwrap_or(0),
            losses.get("infrastructure").copied().unwrap_or(0)
        ));
        out
    }
}

fn summarize(arm: &str, trials: &[Trial]) -> ArmSummary {
    let mine: Vec<&Trial> = trials.iter().filter(|t| t.arm == arm).collect();
    let graded = mine.iter().filter(|t| t.graded()).count();
    let passes = mine.iter().filter(|t| t.passed()).count();
    let mut lost: BTreeMap<String, usize> = LOSS_CAUSES
        .iter()
        .map(|cause| ((*cause).to_owned(), 0))
        .collect();
    let mut quota = 0.0;
    for trial in &mine {
        quota += trial.quota_usd;
        for (cause, spent) in &trial.losses {
            *lost.entry(cause.clone()).or_default() += 1;
            quota += spent;
        }
    }
    ArmSummary {
        arm: arm.to_owned(),
        scheduled: mine.len(),
        graded,
        passes,
        interval: wilson(passes, graded),
        ungraded: mine
            .iter()
            .filter(|t| t.state == "finished" && t.reward.is_none())
            .count(),
        not_run: mine.iter().filter(|t| t.state != "finished").count(),
        lost,
        quota_usd: quota,
    }
}

fn pair(baseline: &str, arm: &str, trials: &[Trial]) -> Paired {
    let outcome = |who: &str, task: &str, attempt: u64| {
        trials
            .iter()
            .find(|t| t.arm == who && t.task == task && t.attempt == attempt && t.graded())
            .map(Trial::passed)
    };
    let (mut both_pass, mut baseline_only, mut arm_only, mut both_fail) = (0, 0, 0, 0);
    let mut per_task: BTreeMap<&str, [(usize, usize); 2]> = BTreeMap::new();
    for trial in trials.iter().filter(|t| t.arm == baseline) {
        if let (Some(b), Some(a)) = (
            outcome(baseline, &trial.task, trial.attempt),
            outcome(arm, &trial.task, trial.attempt),
        ) {
            match (b, a) {
                (true, true) => both_pass += 1,
                (true, false) => baseline_only += 1,
                (false, true) => arm_only += 1,
                (false, false) => both_fail += 1,
            }
        }
    }
    for trial in trials.iter().filter(|t| t.graded()) {
        let index = if trial.arm == baseline {
            0
        } else if trial.arm == arm {
            1
        } else {
            continue;
        };
        let cell = &mut per_task.entry(&trial.task).or_default()[index];
        cell.0 += usize::from(trial.passed());
        cell.1 += 1;
    }
    let (mut better, mut worse, mut tied, mut tasks) = (0, 0, 0, 0);
    for [(b_pass, b_graded), (a_pass, a_graded)] in per_task.values() {
        if *b_graded == 0 || *a_graded == 0 {
            continue;
        }
        tasks += 1;
        // Compare pass fractions exactly: a/ag against b/bg.
        let arm_side = a_pass * b_graded;
        let base_side = b_pass * a_graded;
        match arm_side.cmp(&base_side) {
            std::cmp::Ordering::Greater => better += 1,
            std::cmp::Ordering::Less => worse += 1,
            std::cmp::Ordering::Equal => tied += 1,
        }
    }
    Paired {
        baseline: baseline.to_owned(),
        arm: arm.to_owned(),
        pairs: both_pass + baseline_only + arm_only + both_fail,
        both_pass,
        baseline_only,
        arm_only,
        both_fail,
        mcnemar_p: binomial_two_sided(arm_only, arm_only + baseline_only),
        tasks,
        tasks_arm_better: better,
        tasks_baseline_better: worse,
        tasks_tied: tied,
        sign_p: binomial_two_sided(better, better + worse),
    }
}

fn rate(passes: usize, graded: usize) -> String {
    if graded == 0 {
        "—".to_owned()
    } else {
        format!("{:.0}%", 100.0 * passes as f64 / graded as f64)
    }
}

fn interval_text(summary: &ArmSummary) -> String {
    if summary.graded == 0 {
        "—".to_owned()
    } else {
        format!(
            "{:.0}–{:.0}%",
            100.0 * summary.interval.0,
            100.0 * summary.interval.1
        )
    }
}

fn loss_text(lost: &BTreeMap<String, usize>) -> String {
    let parts: Vec<String> = lost
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(cause, count)| format!("{count} {cause}"))
        .collect();
    if parts.is_empty() {
        "0".to_owned()
    } else {
        parts.join(", ")
    }
}

fn p_text(p: f64) -> String {
    if p >= 0.995 {
        "1".to_owned()
    } else if p < 0.001 {
        "< 0.001".to_owned()
    } else {
        format!("{p:.3}")
    }
}

/// Where `tbench experiment` keeps experiments: `TBENCH_STATE_DIR` or
/// `~/.openagents/terminal-bench`, then `experiments/`.
#[must_use]
pub fn default_dir() -> PathBuf {
    let state = std::env::var_os("TBENCH_STATE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".openagents/terminal-bench"))
        })
        .unwrap_or_else(|| PathBuf::from(".openagents/terminal-bench"));
    state.join("experiments")
}

/// The status file an experiment id or a path names.
#[must_use]
pub fn status_path(query: &str, dir: &Path) -> PathBuf {
    let path = Path::new(query);
    if path.is_file() {
        return path.to_path_buf();
    }
    if path.is_dir() {
        return path.join("status.json");
    }
    dir.join(query).join("status.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(arm: &str, task: &str, attempt: u64, state: &str, reward: Option<f64>) -> Value {
        json!({
            "arm": arm, "task": task, "attempt": attempt, "state": state, "reward": reward,
            "job": format!("tb4--{arm}--{task}--x-r{attempt}"),
            "claude_usage": {"usd": 1.0, "sessions": 1, "unpriced": 0},
            "losses": [],
        })
    }

    fn status(trials: Vec<Value>) -> Value {
        json!({
            "schema": STATUS_SCHEMA,
            "experiment": "x",
            "profile": "tb4",
            "state": "done",
            "arms": [{"id": "plain", "providers": ["anthropic"]}, {"id": "coder", "providers": ["anthropic"]}],
            "tasks": ["a", "b"],
            "attempts": 3,
            "credential_source": "setup-token",
            "quota": {"budget_usd": 50.0, "used": {"usd": 13.5, "unpriced": 0}},
            "trials": trials,
        })
    }

    #[test]
    fn the_exact_binomial_test_matches_known_values() {
        assert!((binomial_two_sided(0, 0) - 1.0).abs() < 1e-12);
        // Five discordant pairs all one way: 2 × 0.5^5.
        assert!((binomial_two_sided(5, 5) - 0.0625).abs() < 1e-12);
        assert!((binomial_two_sided(0, 5) - 0.0625).abs() < 1e-12);
        // 2 of 10: 2 × (1 + 10 + 45) / 1024.
        assert!((binomial_two_sided(2, 10) - 112.0 / 1024.0).abs() < 1e-12);
        assert!((binomial_two_sided(5, 10) - 1.0).abs() < 1e-12);
        assert!(binomial_two_sided(0, 2000) < 1e-300);
    }

    #[test]
    fn arms_count_graded_attempts_only_and_pair_by_task_and_attempt() {
        let mut trials = Vec::new();
        for attempt in 1..=3 {
            trials.push(row("plain", "a", attempt, "finished", Some(0.0)));
            trials.push(row("coder", "a", attempt, "finished", Some(1.0)));
            trials.push(row("plain", "b", attempt, "finished", Some(1.0)));
        }
        trials.push(row("coder", "b", 1, "finished", Some(1.0)));
        trials.push(row("coder", "b", 2, "finished", None));
        let mut lost = row("coder", "b", 3, "pending", None);
        lost["losses"] = json!([{"cause": "credentials", "usage": {"usd": 0.5}}, {"cause": "quota", "usage": {"usd": 0.25}}]);
        trials.push(lost);
        let report = Report::from_status(&status(trials)).unwrap();
        let plain = &report.summaries[0];
        let coder = &report.summaries[1];
        assert_eq!((plain.passes, plain.graded), (3, 6));
        assert_eq!(
            (coder.passes, coder.graded, coder.ungraded, coder.not_run),
            (4, 4, 1, 1)
        );
        assert_eq!(coder.lost["credentials"], 1);
        assert_eq!(coder.lost["quota"], 1);
        assert!((coder.quota_usd - 6.75).abs() < 1e-9);
        let (low, high) = coder.interval;
        assert!(low > 0.39 && high == 1.0);
        let paired = &report.paired[0];
        assert_eq!(
            (
                paired.pairs,
                paired.both_pass,
                paired.arm_only,
                paired.baseline_only,
                paired.both_fail
            ),
            (4, 1, 3, 0, 0)
        );
        assert!((paired.mcnemar_p - 0.25).abs() < 1e-12);
        assert_eq!(
            (paired.tasks, paired.tasks_arm_better, paired.tasks_tied),
            (2, 1, 1)
        );
        assert!(!report.complete());
        assert_eq!(report.min_graded_per_cell(), 1);
        assert_eq!(report.losses()["credentials"], 1);
        let value = report.to_json();
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["arm_results"][1]["graded"], 4);
        assert_eq!(value["paired"][0]["mcnemar_exact_p"], 0.25);
        let markdown = report.markdown();
        assert!(markdown.contains("| `coder` | 4 / 4 | 100% |"));
        assert!(markdown.contains("1 credentials, 1 quota"));
        assert!(markdown.contains("No lost attempt is in a denominator"));
        assert!(
            report
                .lines()
                .iter()
                .any(|line| line.contains("exact McNemar p = 0.250"))
        );
    }

    #[test]
    fn a_file_that_isnt_an_experiment_status_is_refused() {
        assert!(Report::from_status(&json!({"schema": "other"})).is_err());
        let mut one_arm = status(Vec::new());
        one_arm["arms"] = json!(["plain"]);
        assert!(Report::from_status(&one_arm).is_err());
    }

    #[test]
    fn an_id_resolves_under_the_experiments_directory() {
        let dir = Path::new("/nonexistent/experiments");
        assert_eq!(
            status_path("v7-vs-cc", dir),
            dir.join("v7-vs-cc/status.json")
        );
    }
}
