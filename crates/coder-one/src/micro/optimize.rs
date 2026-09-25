//! The lean loop's `checks.metric_target` and `control.optimize` (issue
//! #9657).
//!
//! - **`checks.metric_target`** ([`MetricTargetRule`]). Before session 1,
//!   Jev extracts the goal the task states for its finished work
//!   ([`crate::checks::metric_target`]), and the host finds a provided
//!   script that measures it or has one Luna session write a harness from
//!   the goal and the workspace's file names alone. After every work
//!   session the host measures the workspace, with warmup runs and
//!   alternated repeats, and tells the next session the value, its spread,
//!   and the threshold. `control.finish`: a `done` finish doesn't settle
//!   the loop while the stated target is unmet or unmeasured.
//! - **`control.optimize`** ([`OptimizeRule`]). Once the loop ends on a
//!   workspace that passes the acceptance check, bounded rounds each ask
//!   one Luna session for one improvement, with the measurement and the
//!   harness's own output as evidence. The host keeps a round's change
//!   only when the acceptance check still passes and the metric improves by
//!   more than the measured spread; otherwise it restores the last passing
//!   snapshot. The rounds run after the self-check, when there is one.
//!
//! The acceptance check sits behind [`Acceptance`]. Until the shared
//! acceptance result from `checks.oracle` (#9656) is on main, the lean
//! loop's own executed checks answer it ([`LeanAcceptance`]): the frozen
//! score at full, and no `verify.executed` regression.

use std::collections::BTreeMap;

use super::lean::{Lean, LeanExecuted};
use super::*;
use crate::checks::contract::executed;
use crate::checks::metric_target::{
    self as metric, Extracted, Harness, Measurement, Protocol, Run, Side, Target,
};

/// `executor.microluna.lean.metric_target`: how the stated target is
/// found, measured, and held.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricTargetRule {
    /// Warmup, repeats, and bounds of one measurement.
    #[serde(default)]
    pub protocol: Protocol,
    /// Have one Luna session write a harness when the workspace provides
    /// none.
    #[serde(default = "yes")]
    pub write_harness: bool,
    /// The harness session's spend bound, in dollars.
    #[serde(default = "harness_usd")]
    pub harness_usd: f64,
    /// The harness session's wall-time bound, in seconds.
    #[serde(default = "harness_sec")]
    pub harness_sec: u64,
    /// Hold a `done` finish while the target is unmet or unmeasured
    /// (`control.finish`).
    #[serde(default = "yes")]
    pub finish: bool,
}

/// `executor.microluna.lean.optimize`: the improvement rounds' bounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptimizeRule {
    /// Rounds at most.
    #[serde(default = "rounds")]
    pub rounds: u32,
    /// All rounds together, in seconds, measurements included.
    #[serde(default = "optimize_sec")]
    pub wall_sec: u64,
    /// All rounds' sessions together, in dollars.
    #[serde(default = "optimize_usd")]
    pub spend_usd: f64,
}

fn yes() -> bool {
    true
}

fn harness_usd() -> f64 {
    0.25
}

fn harness_sec() -> u64 {
    600
}

fn rounds() -> u32 {
    3
}

fn optimize_sec() -> u64 {
    900
}

fn optimize_usd() -> f64 {
    1.0
}

impl MetricTargetRule {
    pub(super) fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let p = &self.protocol;
        if p.repeats == 0 || p.run_sec == 0 || p.budget_sec == 0 {
            problems.push(
                "executor.microluna.lean.metric_target: repeats, run_sec, and budget_sec must be \
                 at least 1"
                    .to_string(),
            );
        }
        if p.repeats > 50 || p.warmup > 10 {
            problems.push(
                "executor.microluna.lean.metric_target: at most 50 repeats and 10 warmup runs"
                    .to_string(),
            );
        }
        if !(0.0..=5.0).contains(&self.harness_usd) || self.harness_sec == 0 {
            problems.push(
                "executor.microluna.lean.metric_target: harness_usd is from 0 to 5 dollars and \
                 harness_sec at least 1"
                    .to_string(),
            );
        }
        problems
    }
}

impl OptimizeRule {
    pub(super) fn validate(&self, lean: &Lean) -> Vec<String> {
        let mut problems = Vec::new();
        if !(1..=12).contains(&self.rounds) || self.wall_sec == 0 || self.spend_usd <= 0.0 {
            problems.push(
                "executor.microluna.lean.optimize: rounds from 1 to 12, and wall_sec and \
                 spend_usd above 0"
                    .to_string(),
            );
        }
        if lean.metric_target.is_none() {
            problems.push("executor.microluna.lean.optimize requires metric_target".to_string());
        }
        if !lean.keep_best {
            problems.push("executor.microluna.lean.optimize requires keep_best".to_string());
        }
        if lean.protect_candidates || lean.retain_candidates {
            problems.push(
                "executor.microluna.lean.optimize changes the submitted workspace after \
                 selection, so it doesn't run with protect_candidates or retain_candidates"
                    .to_string(),
            );
        }
        problems
    }
}

/// The acceptance check's result.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Accepted {
    /// `None` when it couldn't run.
    pub passed: Option<bool>,
    pub detail: String,
}

/// The acceptance check `control.optimize` keeps a change behind.
pub(crate) trait Acceptance {
    /// Runs the check on the workspace as it is now; `round` names the
    /// run in its records.
    async fn check(&self, round: u32) -> Accepted;
}

/// The lean loop's own acceptance check: the frozen score at full, and no
/// command that exited 0 on the untouched workspace failing now.
pub(super) struct LeanAcceptance<'a> {
    pub micro: &'a Micro,
    pub lean: &'a Lean,
    /// The frozen score's directory and its digest at the freeze.
    pub frozen: &'a Path,
    pub digest: Option<&'a BTreeMap<String, String>>,
    pub executed: Option<(&'a LeanExecuted, &'a executed::Baseline)>,
    /// Where `verify.executed`'s records go.
    pub executed_file: &'a Path,
    pub deadline: Instant,
}

impl Acceptance for LeanAcceptance<'_> {
    async fn check(&self, round: u32) -> Accepted {
        let intact = self
            .digest
            .is_some_and(|d| lean::evidence_tree(self.frozen).as_ref() == Ok(d));
        if !intact {
            return Accepted {
                passed: None,
                detail: "the frozen score is missing or changed".to_string(),
            };
        }
        let left = self.deadline.saturating_duration_since(Instant::now());
        let (score, tail) = self.micro.lean_score(self.frozen, self.lean, left).await;
        let Some((p, t)) = score else {
            return Accepted {
                passed: None,
                detail: format!("the score didn't run: {}", crate::judge::clip(&tail, 400)),
            };
        };
        if p < t {
            return Accepted {
                passed: Some(false),
                detail: format!("the score is {p} of {t}"),
            };
        }
        if let Some((rule, plan)) = self.executed {
            let place = executed::Place {
                workdir: self.micro.workdir.clone(),
                contained: self.micro.isolation == Isolation::TaskContainer,
                wall: Duration::from_secs(rule.command_sec),
                budget: Duration::from_secs(rule.budget_sec)
                    .min(self.deadline.saturating_duration_since(Instant::now())),
            };
            match executed::after_session(plan, &place, 1_000 + round, None).await {
                Ok(records) => {
                    let _ = executed::append(self.executed_file, &records);
                    if executed::rejects(&records) {
                        return Accepted {
                            passed: Some(false),
                            detail: format!(
                                "the score is {p} of {t}, but a command that ran on the \
                                 untouched workspace fails now"
                            ),
                        };
                    }
                }
                Err(error) => {
                    return Accepted {
                        passed: None,
                        detail: format!("the executed checks didn't run: {error}"),
                    };
                }
            }
        }
        Accepted {
            passed: Some(true),
            detail: format!("the score is {p} of {t}"),
        }
    }
}

/// The stated target, its harness, and its last measurement.
#[derive(Default)]
pub(super) struct Metric {
    pub extracted: Extracted,
    /// The first extracted target: the one measured and held.
    pub target: Option<Target>,
    pub harness: Option<Harness>,
    pub protocol: Protocol,
    /// An untouched copy of the workspace, for a reference side.
    pub reference_dir: Option<PathBuf>,
    pub last: Option<Measurement>,
    /// The frozen harness's directory, removed when the loop ends.
    pub cleanup: Vec<Cleanup>,
}

impl Metric {
    /// Whether a target is held and can be measured.
    pub fn active(&self) -> bool {
        self.target.is_some() && self.harness.is_some()
    }

    /// The evidence every brief carries: the target and how it's measured.
    pub fn evidence(&self) -> Option<Evidence> {
        let target = self.target.as_ref()?;
        let mut text = format!("The task's stated target: {}.", target.line());
        match &self.harness {
            Some(harness) => text.push_str(&format!(
                "\nThe host measures it after every session with `{}{}`, from the workspace, \
                 {} warmup and {} recorded runs{}.",
                harness.command.join(" "),
                if harness.sided { " candidate" } else { "" },
                self.protocol.warmup,
                self.protocol.repeats,
                if target.relative.relative() {
                    ", alternating with the reference"
                } else {
                    ""
                }
            )),
            None => text.push_str("\nThe host has no harness that measures it."),
        }
        if let Some(last) = &self.last {
            text.push_str(&format!(
                "\nBefore any change the host measured: {}.",
                last.line(target)
            ));
        }
        Some(Evidence {
            label: "Stated target".to_string(),
            text,
        })
    }

    /// `control.finish`'s refusal for the last measurement.
    pub fn refusal(&self) -> Option<String> {
        metric::refusal(self.target.as_ref(), self.last.as_ref())
    }

    /// The measurement's record.
    pub fn record(&self, m: &Measurement) -> Value {
        json!({
            "value": m.value,
            "spread": m.spread,
            "values": m.values,
            "error": m.error,
            "verdict": self.target.as_ref().map(|t| m.verdict(t)),
            "confident": self.target.as_ref().map(|t| m.confident(t)),
            "runs": m.runs.len(),
        })
    }
}

/// Runs the frozen harness from the workspace.
pub(super) struct ShellRunner<'a> {
    pub micro: &'a Micro,
    pub harness: &'a Harness,
    pub reference_dir: Option<&'a Path>,
}

impl metric::Runner for ShellRunner<'_> {
    async fn run(&self, side: Side, wall: Duration) -> Run {
        if wall.is_zero() {
            return Run {
                side,
                ok: false,
                printed: None,
                seconds: 0.0,
                tail: "no time left".to_string(),
            };
        }
        let mut args: Vec<String> = self.harness.command.clone();
        if self.harness.sided {
            args.push(side.word().to_string());
        }
        let workdir = &self.micro.workdir;
        let set_env = |command: &mut std::process::Command| {
            command.current_dir(workdir).env("METRIC_SIDE", side.word());
            if let Some(dir) = self.reference_dir {
                command.env("METRIC_REFERENCE_DIR", dir);
            }
            microluna::tools::withhold_credentials(command);
        };
        let ended = if self.micro.isolation == Isolation::TaskContainer {
            let mut command = std::process::Command::new("/usr/bin/env");
            command.args(&args);
            set_env(&mut command);
            supervise::Job::from_command(command)
                .bounded(supervise::Limits::within(wall).keeping(64 * 1024))
                .run()
                .await
        } else {
            let spec = if self.micro.isolation == Isolation::ReadOnly {
                coder_boundary::Boundary::readonly()
            } else {
                coder_boundary::Boundary::writing(workdir)
            };
            let boundary = match spec.owned_scratch_under(std::env::temp_dir()).build() {
                Ok(boundary) => boundary,
                Err(error) => {
                    return Run {
                        side,
                        ok: false,
                        printed: None,
                        seconds: 0.0,
                        tail: format!("no enforced boundary: {error}"),
                    };
                }
            };
            let mut command = match boundary.command("/usr/bin/env", &args) {
                Ok(command) => command,
                Err(error) => {
                    return Run {
                        side,
                        ok: false,
                        printed: None,
                        seconds: 0.0,
                        tail: error.to_string(),
                    };
                }
            };
            set_env(&mut command);
            supervise::Job::from_command(command)
                .bounded(supervise::Limits::within(wall).keeping(64 * 1024))
                .run_holding(boundary.hold())
                .await
        };
        let stdout = ended.stdout.marked();
        let stderr = ended.stderr.marked();
        Run {
            side,
            ok: ended.ending.success() && !ended.stdout.truncated,
            printed: metric::parse_metric(&stdout),
            seconds: ended.elapsed.as_secs_f64(),
            tail: crate::judge::clip(
                &format!("{}\n{}", stdout.trim_end(), stderr.trim_end()),
                1_500,
            ),
        }
    }
}

/// Where a harness session writes: outside the workspace in a task
/// container, and inside it, hidden, where a boundary lets a session
/// write only the workspace.
#[must_use]
pub fn metric_dir(workdir: &Path, isolation: Isolation) -> PathBuf {
    if isolation == Isolation::TaskContainer {
        std::env::temp_dir().join(format!(
            "microluna-metric-{}",
            &sha256(&workdir.display().to_string())[..12]
        ))
    } else {
        workdir.join(".microluna-metric")
    }
}

/// The workspace's identity without the harness directory in it.
fn identity_without(
    scope: &candidate::Scope,
    workdir: &Path,
    dir: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let inside = dir
        .strip_prefix(workdir)
        .ok()
        .map(|p| p.display().to_string());
    let mut identity = scope.identity(workdir)?;
    if let Some(prefix) = inside {
        identity.retain(|path, _| !path.starts_with(&prefix));
    }
    Ok(identity)
}

impl Micro {
    /// Measures the workspace against the metric's target, when it has a
    /// harness; `None` otherwise.
    pub(super) async fn metric_measure(
        &self,
        metric: &Metric,
        remaining: Duration,
    ) -> Option<Measurement> {
        let target = metric.target.as_ref()?;
        let harness = metric.harness.as_ref()?;
        if !harness.intact() {
            return Some(Measurement::unmeasured(
                "the frozen harness changed after the freeze",
                Vec::new(),
            ));
        }
        let budget = Duration::from_secs(metric.protocol.budget_sec).min(remaining);
        let runner = ShellRunner {
            micro: self,
            harness,
            reference_dir: metric.reference_dir.as_deref(),
        };
        Some(metric::measure(&runner, target, &metric.protocol, budget).await)
    }

    /// `checks.metric_target`'s setup: extract the target, find or write
    /// the harness, and measure the untouched workspace. Returns the
    /// metric, the harness session when one ran, its record, and what Jev
    /// cost.
    #[allow(clippy::too_many_lines)]
    pub(super) async fn metric_setup(
        &self,
        prepared: &Prepared,
        rule: &MetricTargetRule,
        number: u32,
        base: Option<&Path>,
        spend_left: f64,
        remaining: Duration,
    ) -> (Metric, Option<Ran>, Value, f64) {
        let extracted = metric::extract(
            &prepared.jev,
            &self.recorder,
            &metric::Context {
                component: "microluna.lean",
                id: format!("jev-metric-target-{}", self.dispatch()),
                deadline: prepared.deadline.clone(),
            },
            &prepared.instruction,
            Some(&self.workdir),
        )
        .await;
        let usd = extracted.usd;
        let target = extracted.targets.first().cloned();
        let mut metric = Metric {
            target: target.clone(),
            protocol: rule.protocol,
            reference_dir: base.map(Path::to_path_buf),
            ..Metric::default()
        };
        let mut record = json!({
            "kind": "lean.metric_target",
            "answered": extracted.answered,
            "numbers": extracted.candidates.len(),
            "targets": extracted.targets,
            "provided": extracted.harness,
            "jev": extracted.call,
        });
        metric.extracted = extracted;
        let Some(target) = target else {
            crate::say::line("  microluna ▸ metric target: the task states none");
            return (metric, None, record, usd);
        };
        crate::say::line(&format!(
            "  microluna ▸ metric target: {}",
            crate::judge::clip(&target.line(), 200)
        ));
        let mut ran = None;
        // A provided script measures an absolute target as it is.
        if let Some(path) = metric
            .extracted
            .harness
            .clone()
            .filter(|_| !target.relative.relative())
        {
            metric.harness = Some(Harness::provided(&path));
            record["harness"] = json!({"source": "provided", "path": path});
        } else if rule.write_harness {
            let dir = metric_dir(&self.workdir, self.isolation);
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::create_dir_all(&dir);
            let scope = candidate::Scope::of(&self.workdir);
            let before = identity_without(&scope, &self.workdir, &dir);
            let saved = scratch("metric-harness-before");
            let saved_ok =
                scope.bound(&self.workdir).is_ok() && scope.snapshot(&self.workdir, &saved).is_ok();
            let file = dir.join(metric::HARNESS_FILE);
            let brief = Brief {
                task: format!(
                    "The goal to measure: {}.\n\nThe sentence it comes from: \"{}\"",
                    target.line(),
                    target.sentence.trim()
                ),
                guidance: metric::HARNESS_GUIDANCE.to_string(),
                evidence: vec![Evidence {
                    label: "The workspace's files".to_string(),
                    text: metric::interfaces(&self.workdir, 200),
                }],
                state: vec![format!(
                    "Write the harness at {}. The workspace is {}.",
                    file.display(),
                    self.workdir.display()
                )],
            };
            let session = self
                .session_at(
                    number,
                    &["the measurement harness".to_string()],
                    "checks.metric_target writes a harness for the stated target",
                    &brief,
                    false,
                    Place {
                        group: Some("the measurement harness".to_string()),
                        deadline: Some(Duration::from_secs(rule.harness_sec).min(remaining)),
                        spend_usd: Some(rule.harness_usd.min(spend_left).max(0.0)),
                        ..Place::default()
                    },
                )
                .await;
            let after = identity_without(&scope, &self.workdir, &dir);
            let changed = before.is_err() || after.is_err() || before != after;
            let restored = if changed && saved_ok {
                Some(
                    scope
                        .restore(&self.workdir, &saved)
                        .map_err(|e| e.to_string()),
                )
            } else {
                None
            };
            let _ = std::fs::remove_dir_all(&saved);
            let frozen = scratch("metric-harness-frozen");
            let harness = if file.is_file() && crate::handoff::copy_tree(&dir, &frozen).is_ok() {
                metric.cleanup.push(Cleanup(Some(frozen.clone())));
                Some(Harness {
                    source: metric::Source::Written,
                    command: vec![
                        "sh".to_string(),
                        frozen.join(metric::HARNESS_FILE).display().to_string(),
                    ],
                    sided: true,
                    digest: lean::evidence_tree(&frozen).ok(),
                    frozen: Some(frozen),
                })
            } else {
                None
            };
            let _ = std::fs::remove_dir_all(&dir);
            record["harness"] = json!({
                "source": "written",
                "session": number,
                "status": session.status(),
                "usd": session.cost_usd,
                "frozen": harness.is_some(),
                "workspace_changed": changed,
                "restored": restored.map(|r| r.err().unwrap_or_else(|| "ok".to_string())),
            });
            metric.harness = harness;
            ran = Some(session);
        } else {
            record["harness"] =
                json!({"source": null, "reason": "none provided, and writing one is off"});
        }
        if metric.harness.is_some() {
            let untouched = self.metric_measure(&metric, remaining).await;
            if let Some(m) = &untouched {
                record["untouched"] = metric.record(m);
                crate::say::line(&format!(
                    "  microluna ▸ metric target, before any change: {}",
                    m.line(&target)
                ));
            }
            metric.last = untouched;
        }
        (metric, ran, record, usd)
    }

    /// `control.optimize`: bounded improvement rounds on a workspace that
    /// passes `acceptance`, each kept only when the check still passes and
    /// the metric improves beyond the spread. Returns the sessions, the
    /// records, what they cost, and a line for why the loop stopped.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) async fn optimize<A: Acceptance>(
        &self,
        prepared: &Prepared,
        rule: &OptimizeRule,
        metric: &mut Metric,
        acceptance: &A,
        first: u32,
        spend_left: f64,
        remaining: Duration,
    ) -> (Vec<Ran>, Vec<Value>, f64, String) {
        let started = Instant::now();
        let mut sessions = Vec::new();
        let mut records = Vec::new();
        let mut spent = 0.0;
        let (Some(target), true) = (metric.target.clone(), metric.harness.is_some()) else {
            records.push(json!({"kind": "lean.optimize_skipped", "reason": "no stated target with a harness"}));
            return (sessions, records, spent, String::new());
        };
        let wall = Duration::from_secs(rule.wall_sec).min(remaining);
        let left = || wall.saturating_sub(started.elapsed());
        let accepted = acceptance.check(0).await;
        if accepted.passed != Some(true) {
            records.push(json!({
                "kind": "lean.optimize_skipped",
                "reason": "the workspace doesn't pass the acceptance check",
                "acceptance": accepted.detail,
            }));
            return (
                sessions,
                records,
                spent,
                "no optimization: the acceptance check doesn't pass".to_string(),
            );
        }
        let Some(mut before) = self
            .metric_measure(metric, left())
            .await
            .filter(|m| m.value.is_some())
        else {
            records.push(
                json!({"kind": "lean.optimize_skipped", "reason": "the target can't be measured"}),
            );
            return (
                sessions,
                records,
                spent,
                "no optimization: the target can't be measured".to_string(),
            );
        };
        metric.last = Some(before.clone());
        let scope = candidate::Scope::of(&self.workdir);
        let passing = scratch("optimize-passing");
        let _passing_cleanup = Cleanup(Some(passing.clone()));
        if let Err(error) = scope
            .bound(&self.workdir)
            .and_then(|()| scope.snapshot(&self.workdir, &passing))
        {
            records.push(
                json!({"kind": "lean.optimize_skipped", "reason": format!("no snapshot: {error}")}),
            );
            return (
                sessions,
                records,
                spent,
                "no optimization: the workspace can't be snapshotted".to_string(),
            );
        }
        let mut history: Vec<String> = Vec::new();
        let mut kept_rounds = 0;
        let mut stopped = format!("the {} optimization rounds ran", rule.rounds);
        for round in 1..=rule.rounds {
            if left() < Duration::from_secs(60) {
                stopped = "the optimization rounds' time ran out".to_string();
                break;
            }
            let budget = rule.spend_usd.min(spend_left) - spent;
            if budget <= 0.0 {
                stopped = "the optimization rounds' spend bound was reached".to_string();
                break;
            }
            let number = first + round - 1;
            let mut state = vec![
                format!("Optimization round {round} of at most {}.", rule.rounds),
                format!("The host's measurement now: {}.", before.line(&target)),
            ];
            state.extend(history.iter().cloned());
            if !before.tail.trim().is_empty() {
                state.push(format!(
                    "The harness's output on the last passing workspace, its tail:\n{}",
                    crate::judge::clip(&before.tail, 1_500)
                ));
            }
            let mut evidence = vec![];
            if let Some(e) = metric.evidence() {
                evidence.push(e);
            }
            let ran = self
                .session_at(
                    number,
                    &["one improvement".to_string()],
                    "control.optimize asks for one improvement to the measured target",
                    &Brief {
                        task: prepared.instruction.clone(),
                        guidance: metric::OPTIMIZE_GUIDANCE.to_string(),
                        evidence,
                        state,
                    },
                    false,
                    Place {
                        group: Some(format!("optimization round {round}")),
                        deadline: Some(left()),
                        spend_usd: Some(budget),
                        ..Place::default()
                    },
                )
                .await;
            spent += ran.cost_usd.unwrap_or(0.0);
            let status = ran.status();
            let lost = matches!(ran.ending, Ending::Transport(_));
            sessions.push(ran);
            let accepted = acceptance.check(round).await;
            let (after, keep, reason) = if accepted.passed == Some(true) {
                let after = self.metric_measure(metric, left()).await;
                let improved = after
                    .as_ref()
                    .is_some_and(|a| metric::improves(&before, a, target.direction));
                let reason = if improved {
                    "the acceptance check passes and the metric improved beyond the spread"
                        .to_string()
                } else {
                    "the acceptance check passes, but the metric didn't improve beyond the spread"
                        .to_string()
                };
                (after, improved, reason)
            } else {
                (
                    None,
                    false,
                    format!("the acceptance check doesn't pass: {}", accepted.detail),
                )
            };
            let mut restore_error = None;
            if keep {
                if let Some(a) = &after {
                    before = a.clone();
                }
                kept_rounds += 1;
                let _ = std::fs::remove_dir_all(&passing);
                if let Err(error) = scope.snapshot(&self.workdir, &passing) {
                    restore_error = Some(format!("the new snapshot failed: {error}"));
                }
            } else if let Err(error) = scope.restore(&self.workdir, &passing) {
                restore_error = Some(error);
            }
            crate::say::line(&format!(
                "  microluna ▸ optimization round {round} {status}; {}; {}",
                after
                    .as_ref()
                    .map_or("not measured".to_string(), |a| a.line(&target)),
                if keep {
                    "kept"
                } else {
                    "restored the last passing workspace"
                }
            ));
            history.push(format!(
                "Round {round}: {}{}.",
                reason,
                if keep {
                    "; the host kept the change"
                } else {
                    "; the host restored the last passing workspace"
                }
            ));
            records.push(json!({
                "kind": "lean.optimize",
                "round": round,
                "session": number,
                "status": status,
                "acceptance": {"passed": accepted.passed, "detail": accepted.detail},
                "before": metric.record(metric.last.as_ref().unwrap_or(&before)),
                "after": after.as_ref().map(|a| metric.record(a)),
                "kept": keep,
                "restored": !keep,
                "reason": reason,
                "restore_error": restore_error,
            }));
            metric.last = Some(before.clone());
            if lost {
                stopped = format!("optimization round {round} lost its provider");
                break;
            }
            if restore_error.is_some() {
                stopped = format!("optimization round {round} couldn't restore or keep a snapshot");
                break;
            }
        }
        (
            sessions,
            records,
            spent,
            format!(
                "{stopped}; {kept_rounds} kept; the metric is {}",
                before.line(&target)
            ),
        )
    }
}
