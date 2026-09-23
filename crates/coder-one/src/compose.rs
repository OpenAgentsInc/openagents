//! The tunable composition in a Terminal-Bench episode.
//!
//! Every component ran alone or in a mini-task episode; a Terminal-Bench
//! episode ran requirements, evidence, and one executor. This module runs
//! the whole composition inside one episode deadline:
//!
//! ```text
//! control.route    task.profile's features pick the first executor and its budget
//! control.handoff  planner-worker: a planner writes a plan in a scratch copy first
//! exec.session     the first executor, watched by control.monitor
//! verify.checks    requirement-derived and generic scenarios on the live workspace
//! verify.support   Jev's paired support and contradiction judgments
//! control.handoff  escalate: on a stall, a failed check, a contradicted
//!                  requirement, or no answer, the second executor continues
//!                  from a handoff brief
//! verify.repair    one fresh session from the diagnostic packets, then a recheck
//! ```
//!
//! The policy manifest turns each part on: `control.route`,
//! `control.horizon`, `control.handoff`, and `verify`. `control.horizon`
//! sizes each dispatch from the episode deadline, so an eight-hour task
//! gives its executors hours instead of the ten-minute default. The
//! episode writes the whole account to `artifacts/composition.json`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::Ended;
use crate::agent::{Generate, Shell};
use crate::checks::{self, Budget, Subject, TaskText, generic};
use crate::component::jev::JevMode;
use crate::deadline::Deadline;
use crate::delegate::{
    self, Briefing, Cli, Delegated, Delegation, Executor, Explorer, Mode, Plan, Reason, Report,
    Status,
};
use crate::handoff::{self, Pattern, Tier};
use crate::monitor::{Acting, Params as MonitorParams, Setup as MonitorSetup};
use crate::record::{Cost, Finish, Implementation, Outcome, Recorder, Start};
use crate::scripted::Scripted;
use crate::session::Intent;
use crate::state::State;

/// The schema of `artifacts/composition.json`.
pub const SCHEMA: &str = "openagents.coder-one.composition.v1";

/// Where the episode keeps the composition's record.
pub const FILE: &str = "artifacts/composition.json";

/// The routing component's ID.
pub const ROUTE: &str = "control.route";

/// Where the check after an escalation writes its report.
pub const ESCALATED_CHECKS: &str = "verification/checks-escalated.json";

fn half() -> f64 {
    0.5
}
fn seven_tenths() -> f64 {
    0.7
}
fn hard_features() -> Vec<String> {
    vec!["builds_code".to_string(), "concurrency".to_string()]
}
fn yes() -> bool {
    true
}

/// `control.route`: which executor starts, from task.profile's features.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutePolicy {
    /// The rule's name; only `profile-v1` in this build.
    pub rule: String,
    /// The executor for cheap, likely-easy tasks.
    pub cheap: Tier,
    /// The executor for hard tasks, and for any task the profile can't
    /// read.
    pub strong: Tier,
    /// A difficulty at or above this starts strong.
    #[serde(default = "half")]
    pub hard_at: f64,
    /// Features that start strong at or above `feature_at`.
    #[serde(default = "hard_features")]
    pub hard_features: Vec<String>,
    #[serde(default = "seven_tenths")]
    pub feature_at: f64,
    /// An episode deadline at least this long starts strong: the task's
    /// own timeout says it is long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_after_sec: Option<u64>,
}

impl RoutePolicy {
    /// Refuses a route this build can't run.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.rule != "profile-v1" {
            problems.push(format!(
                "control.route.rule must be profile-v1, not {}",
                self.rule
            ));
        }
        problems.extend(self.cheap.validate("control.route.cheap"));
        problems.extend(self.strong.validate("control.route.strong"));
        if !(0.0..=1.0).contains(&self.hard_at) || !(0.0..=1.0).contains(&self.feature_at) {
            problems.push("control.route.hard_at and feature_at must be 0 to 1".to_string());
        }
        for feature in &self.hard_features {
            if !crate::profile::NOULS.iter().any(|(id, _)| id == feature) {
                problems.push(format!(
                    "control.route.hard_features names {feature}, which task.profile doesn't ask"
                ));
            }
        }
        problems
    }
}

fn first_share() -> f64 {
    0.55
}
fn later_share() -> f64 {
    0.75
}
fn min_dispatch() -> u64 {
    300
}
fn check_share() -> f64 {
    0.05
}

/// `control.horizon`: how dispatches, checks, and effort scale with the
/// episode deadline, the task's own timeout less the harness's margins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Horizon {
    /// The share of the time left that the first dispatch asks for when
    /// an escalation can follow it.
    #[serde(default = "first_share")]
    pub first_share: f64,
    /// The share a dispatch asks for when only a repair can follow it.
    #[serde(default = "later_share")]
    pub later_share: f64,
    /// The least a dispatch asks for, in seconds.
    #[serde(default = "min_dispatch")]
    pub min_dispatch_sec: u64,
    /// The share of the whole deadline the checks may spend.
    #[serde(default = "check_share")]
    pub check_share: f64,
    /// A deadline at least this long is a long task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_after_sec: Option<u64>,
    /// The effort every executor runs at on a long task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_effort: Option<String>,
    /// On a long task, the most seconds one Claude Code shell command may
    /// run (`BASH_MAX_TIMEOUT_MS`); the CLI's own ten-minute cap otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_command_sec: Option<u64>,
}

impl Default for Horizon {
    fn default() -> Self {
        Horizon {
            first_share: first_share(),
            later_share: later_share(),
            min_dispatch_sec: min_dispatch(),
            check_share: check_share(),
            long_after_sec: None,
            long_effort: None,
            long_command_sec: None,
        }
    }
}

impl Horizon {
    /// Refuses a horizon this build can't run.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (field, share) in [
            ("first_share", self.first_share),
            ("later_share", self.later_share),
            ("check_share", self.check_share),
        ] {
            if !(share > 0.0 && share <= 1.0) {
                problems.push(format!(
                    "control.horizon.{field} must be above 0 and at most 1"
                ));
            }
        }
        if let Some(effort) = &self.long_effort
            && (effort.is_empty() || !effort.chars().all(|c| c.is_ascii_lowercase()))
        {
            problems.push("control.horizon.long_effort must be one lowercase word".to_string());
        }
        problems
    }

    /// Whether an episode of `total` seconds is a long task.
    #[must_use]
    pub fn long(&self, total: Option<u64>) -> bool {
        matches!((total, self.long_after_sec), (Some(total), Some(after)) if total >= after)
    }

    /// The seconds one dispatch asks for: `fallback` without a deadline;
    /// otherwise `share` of what is left, at least the minimum, or all of
    /// it when nothing can follow.
    #[must_use]
    pub fn dispatch_sec(&self, remaining: Option<Duration>, fallback: u64, share: f64) -> u64 {
        let Some(remaining) = remaining else {
            return fallback;
        };
        let left = remaining.as_secs();
        let asked = (left as f64 * share).round() as u64;
        asked.max(self.min_dispatch_sec).min(left).max(1)
    }

    /// The checks' budget for an episode of `total` seconds, and the bound
    /// on each command a check runs.
    #[must_use]
    pub fn checks(&self, total: Option<u64>) -> (Budget, u64) {
        match total {
            None => (
                Budget {
                    max_scenarios: 12,
                    seconds: 300,
                },
                60,
            ),
            Some(total) => {
                let seconds = ((total as f64 * self.check_share) as u64).clamp(180, 1_800);
                (
                    Budget {
                        max_scenarios: 12,
                        seconds,
                    },
                    (total / 60).clamp(60, 900),
                )
            }
        }
    }
}

/// `verify` in a Terminal-Bench episode: which checks run after the
/// executor, and whether one repair follows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyPolicy {
    /// `verify.checks` on the live workspace.
    #[serde(default = "yes")]
    pub checks: bool,
    /// `verify.support`, which needs Jev.
    #[serde(default)]
    pub support: bool,
    /// `verify.repair`: one fresh session from the packets; needs checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<RepairPolicy>,
}

/// One repair's brief and trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairPolicy {
    pub brief: crate::repair::BriefKind,
    pub trigger: crate::repair::Trigger,
}

/// The executor the manifest itself names, as a tier.
#[must_use]
pub fn manifest_tier(manifest: &crate::policy::Manifest) -> Tier {
    let executor = &manifest.policy.executor;
    Tier {
        agent: executor.agent.agent().word().to_string(),
        model: executor.model.clone(),
        effort: executor.effort.clone(),
        tools: executor.tools.clone(),
        prompt_cache_ttl: executor.prompt_cache_ttl.clone(),
        version: executor.version.clone(),
    }
}

/// Every tier a manifest can dispatch to: its executor, the route's two,
/// and the handoff's second.
#[must_use]
pub fn tiers(manifest: &crate::policy::Manifest) -> Vec<Tier> {
    let mut out = vec![manifest_tier(manifest)];
    let control = &manifest.policy.control;
    if let Some(route) = &control.route {
        out.push(route.cheap.clone());
        out.push(route.strong.clone());
    }
    if let Some(to) = control.handoff.as_ref().and_then(|h| h.to.clone()) {
        out.push(to);
    }
    out
}

/// Whether a manifest runs the composition rather than one executor.
#[must_use]
pub fn composes(manifest: &crate::policy::Manifest) -> bool {
    let control = &manifest.policy.control;
    manifest.policy.verify.is_some()
        || control.route.is_some()
        || control.horizon.is_some()
        || control
            .handoff
            .as_ref()
            .is_some_and(|h| h.pattern != Pattern::Single)
}

/// Where the route started, and why.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Routed {
    /// `cheap`, `strong`, or `manifest` when no route ran.
    pub start: String,
    pub tier: Tier,
    pub reason: String,
}

/// The `profile-v1` rule: strong when the profile can't be read, the task
/// is long, its difficulty is high, or a hard feature is likely; cheap
/// otherwise.
#[must_use]
pub fn decide(
    route: &RoutePolicy,
    profile: &crate::profile::Profile,
    total_sec: Option<u64>,
) -> Routed {
    let strong = |reason: String| Routed {
        start: "strong".to_string(),
        tier: route.strong.clone(),
        reason,
    };
    if let (Some(after), Some(total)) = (route.long_after_sec, total_sec)
        && total >= after
    {
        return strong(format!(
            "the episode deadline of {total} s is at least the long-task bound of {after} s"
        ));
    }
    let Some(difficulty) = profile.difficulty else {
        return strong("task.profile gave no difficulty, so the safe start is strong".to_string());
    };
    if difficulty >= route.hard_at {
        return strong(format!(
            "difficulty {difficulty:.2} is at least {:.2}",
            route.hard_at
        ));
    }
    for feature in &route.hard_features {
        if let Some(Some(p)) = profile.features.get(feature)
            && *p >= route.feature_at
        {
            return strong(format!(
                "{feature} is likely ({p:.2} at least {:.2})",
                route.feature_at
            ));
        }
    }
    Routed {
        start: "cheap".to_string(),
        tier: route.cheap.clone(),
        reason: format!(
            "difficulty {difficulty:.2} is below {:.2} and no hard feature is likely",
            route.hard_at
        ),
    }
}

// ---------------------------------------------------------------------------
// Executors.
// ---------------------------------------------------------------------------

/// A dispatch's executor: a real CLI or a script.
pub enum Exec {
    Cli(Box<Cli>),
    Scripted(Box<Scripted>),
}

impl Exec {
    /// The last session's host-loop record.
    #[must_use]
    pub fn last(&self) -> Value {
        match self {
            Exec::Cli(cli) => cli.control.last.clone().unwrap_or(Value::Null),
            Exec::Scripted(scripted) => scripted.last.clone().unwrap_or(Value::Null),
        }
    }

    /// Sets the monitor that watches its sessions.
    pub fn watch(&mut self, monitor: Option<MonitorSetup>) {
        match self {
            Exec::Cli(cli) => cli.control.monitor = monitor,
            Exec::Scripted(scripted) => scripted.monitor = monitor,
        }
    }

    /// Dispatches run so far.
    #[must_use]
    pub fn runs(&self) -> u32 {
        match self {
            Exec::Cli(cli) => cli.runs,
            Exec::Scripted(scripted) => scripted.runs,
        }
    }
}

impl Executor for Exec {
    fn agent(&self) -> &str {
        match self {
            Exec::Cli(cli) => cli.agent(),
            Exec::Scripted(scripted) => scripted.agent(),
        }
    }
    fn cost_provenance(&self) -> &'static str {
        match self {
            Exec::Cli(cli) => cli.cost_provenance(),
            Exec::Scripted(scripted) => scripted.cost_provenance(),
        }
    }
    fn model(&self) -> &str {
        match self {
            Exec::Cli(cli) => cli.model(),
            Exec::Scripted(scripted) => scripted.model(),
        }
    }
    fn deadline(&self) -> Duration {
        match self {
            Exec::Cli(cli) => cli.deadline(),
            Exec::Scripted(scripted) => scripted.deadline(),
        }
    }
    fn describe(&self) -> Map<String, Value> {
        match self {
            Exec::Cli(cli) => cli.describe(),
            Exec::Scripted(scripted) => scripted.describe(),
        }
    }
    async fn execute(&mut self, briefing: &Briefing) -> Report {
        match self {
            Exec::Cli(cli) => cli.execute(briefing).await,
            Exec::Scripted(scripted) => scripted.execute(briefing).await,
        }
    }
    fn system_options(&self) -> Vec<String> {
        match self {
            Exec::Cli(cli) => cli.system_options(),
            Exec::Scripted(scripted) => scripted.system_options(),
        }
    }
    fn select_system(&mut self, answers: Vec<(String, Option<f64>)>) {
        match self {
            Exec::Cli(cli) => cli.select_system(answers),
            Exec::Scripted(scripted) => scripted.select_system(answers),
        }
    }
}

/// Makes the executor for a tier.
pub trait Factory {
    /// An executor for `tier` that asks for `deadline` and has `runs`
    /// dispatches before it, so its files don't overwrite theirs.
    ///
    /// # Errors
    ///
    /// Returns why the tier can't run here.
    fn make(&mut self, tier: &Tier, deadline: Duration, runs: u32) -> Result<Exec, String>;
}

// ---------------------------------------------------------------------------
// The composition.
// ---------------------------------------------------------------------------

/// Everything the composition reads besides the executors.
pub struct Setup<'a> {
    pub manifest: &'a crate::policy::Manifest,
    pub instruction: &'a str,
    pub workdir: &'a Path,
    /// The episode's output directory: `verification/` and `artifacts/`
    /// go under it.
    pub dir: &'a Path,
    pub recorder: &'a Recorder,
    pub deadline: &'a Deadline,
    pub jev: JevMode,
    /// task.profile's answer when the caller already has one; the route
    /// asks Jev otherwise.
    pub profile: Option<crate::profile::Profile>,
    /// The workdir's Git base, for the handoff brief's diff.
    pub base: Option<&'a str>,
}

/// What the composition leaves.
pub struct Composed {
    pub ended: Ended,
    pub delegated: Option<Delegated>,
    pub record: Value,
}

/// One dispatch's ledger row.
fn row(role: &str, tier: &Tier, report: &Report, last: &Value, granted: u64) -> Value {
    let (charge, _) = delegate::charge(report);
    json!({
        "role": role,
        "tier": tier,
        "status": report.status.word(),
        "milliseconds": report.milliseconds,
        "requested_sec": granted,
        "usd": (charge == "priced").then_some(report.summary.total_cost_usd).flatten(),
        "charge": charge,
        "turns": report.summary.num_turns,
        "stopped_by": last["stopped_by"],
        "session_id": report.summary.session_id,
    })
}

/// The commands every executor session so far reported, in order.
#[must_use]
pub fn claimed(recorder: &Recorder) -> Vec<generic::Claimed> {
    recorder
        .steps()
        .iter()
        .filter_map(|step| {
            let event = step
                .extensions
                .get(crate::session::EVENT_KEY)?
                .get("event")?;
            (event.get("kind")?.as_str()? == "command_completed").then(|| generic::Claimed {
                command: crate::stream::unwrap_shell(event["command"].as_str().unwrap_or_default()),
                exit_code: event.get("exit_code").and_then(Value::as_i64),
            })
        })
        .collect()
}

/// The last failing commands since step `from`.
fn last_errors(recorder: &Recorder, from: usize, n: usize) -> Vec<String> {
    let steps = recorder.steps();
    let mut errors: Vec<String> = steps[from.min(steps.len())..]
        .iter()
        .rev()
        .filter_map(|step| {
            let event = step
                .extensions
                .get(crate::session::EVENT_KEY)?
                .get("event")?;
            (event.get("kind")?.as_str()? == "command_completed"
                && event.get("exit_code")?.as_i64()? != 0)
                .then(|| {
                    format!(
                        "`{}` exited {}: {}",
                        crate::stream::unwrap_shell(event["command"].as_str().unwrap_or_default()),
                        event["exit_code"],
                        crate::judge::clip_tail(
                            event["output"].as_str().unwrap_or_default().trim(),
                            400
                        )
                    )
                })
        })
        .take(n)
        .collect();
    errors.reverse();
    errors
}

/// The handoff section a second executor reads after the first briefing.
fn handoff_section(
    from: &Tier,
    trigger: &str,
    report: Option<&checks::Report>,
    support: Option<&crate::support::Report>,
    changes: &str,
    errors: &[String],
) -> String {
    let mut text = format!(
        "\n\n## Handoff from {} ({trigger})\n\nAnother executor worked on this task first. Continue from the workspace as it is now; don't start over, and don't repeat what failed.\n",
        from.label()
    );
    if let Some(report) = report {
        text.push_str("\n### Requirement states from the host's checks\n\n");
        for covered in &report.coverage {
            text.push_str(&format!(
                "- {} ({}): {}\n",
                covered.id,
                covered.state,
                crate::judge::clip(&covered.text, 240)
            ));
        }
        for packet in report.packets.iter().take(3) {
            text.push_str(&format!(
                "\nThe check `{}` expected: {}\nIt observed:\n```json\n{}\n```\n",
                packet.scenario,
                packet.expected.statement,
                crate::judge::clip(
                    &crate::support::scrub(
                        &serde_json::to_string_pretty(&packet.observations).unwrap_or_default()
                    ),
                    1_500
                )
            ));
        }
    }
    if let Some(support) = support {
        let contradicted: Vec<&crate::support::State> = support
            .states
            .iter()
            .filter(|s| s.state == "contradicted")
            .collect();
        if !contradicted.is_empty() {
            text.push_str("\n### Requirements a judge read as contradicted\n\n");
            for state in contradicted {
                text.push_str(&format!(
                    "- {}: {}\n",
                    state.id,
                    crate::judge::clip(&state.text, 240)
                ));
            }
        }
    }
    if !errors.is_empty() {
        text.push_str("\n### The last failing commands\n\n");
        for error in errors {
            text.push_str(&format!("- {error}\n"));
        }
    }
    text.push_str(&format!(
        "\n### What changed in the workspace\n\n```\n{}\n```\n",
        crate::judge::clip(changes, 3_000)
    ));
    text
}

fn record_decision(recorder: &Recorder, component: &str, name: &str, record: &Value) {
    let id = recorder.begin(
        Start::new(
            component,
            Implementation::new(component, name, &json!({ "version": 1 })),
        )
        .named(name)
        .with_effects(),
    );
    recorder.end(
        &id,
        Finish::new(Outcome::Completed)
            .output(record.clone())
            .cost(Cost::none()),
    );
}

/// Asks task.profile and routes, or keeps the manifest's executor.
async fn route(setup: &Setup<'_>, total: Option<u64>) -> (Routed, Value) {
    let Some(policy) = &setup.manifest.policy.control.route else {
        let tier = manifest_tier(setup.manifest);
        return (
            Routed {
                start: "manifest".to_string(),
                tier,
                reason: "no control.route: the manifest's executor starts".to_string(),
            },
            Value::Null,
        );
    };
    let profile = match &setup.profile {
        Some(profile) => profile.clone(),
        None => {
            let issue = json!({ "title": crate::judge::clip(setup.instruction.lines().next().unwrap_or("Task"), 120), "body": setup.instruction });
            match crate::profile::request(&issue, &[]) {
                Ok((state, questions)) => {
                    let asked = crate::component::jev::ask(
                        &setup.jev,
                        setup.recorder,
                        crate::component::jev::Ask {
                            component: "task.profile",
                            name: "jev_profile",
                            id: "jev-profile".to_string(),
                            state,
                            questions,
                            parent: None,
                            deadline: Some(setup.deadline.clone()),
                        },
                    )
                    .await;
                    crate::profile::read(asked.answers.as_ref())
                }
                Err(_) => crate::profile::Profile::default(),
            }
        }
    };
    let routed = decide(policy, &profile, total);
    let record = json!({
        "rule": policy.rule,
        "profile": profile,
        "start": routed.start,
        "tier": routed.tier,
        "reason": routed.reason,
        "episode_deadline_sec": total,
    });
    record_decision(
        setup.recorder,
        ROUTE,
        &format!("start {}", routed.start),
        &record,
    );
    println!(
        "  route ▸ {} {} · {}",
        routed.start,
        routed.tier.label(),
        routed.reason
    );
    (routed, record)
}

/// The monitor the first session runs under: acting for an escalation,
/// shadow otherwise, with its silence scaled to the dispatch.
fn monitor_for(
    setup: &Setup<'_>,
    escalating: Option<&handoff::Policy>,
    dispatch_sec: u64,
) -> Option<MonitorSetup> {
    let control = &setup.manifest.policy.control;
    let params = escalating
        .and_then(|h| h.monitor.clone())
        .or_else(|| control.monitor.clone())
        .or_else(|| {
            escalating.map(|_| MonitorParams {
                jev: false,
                silence_ms: Some(60_000),
                ..MonitorParams::default()
            })
        })?;
    let mut params = params;
    // A long command is not a stall: the silence scales with the dispatch.
    if let Some(silence) = params.silence_ms {
        params.silence_ms = Some(silence.max(dispatch_sec * 1_000 / 12));
    }
    Some(MonitorSetup {
        jev: if params.jev {
            setup.jev.clone()
        } else {
            JevMode::Off
        },
        params,
        task: setup.instruction.to_string(),
        acting: escalating.map(|h| Acting {
            intent: Intent::Stop,
            after: h.after,
            on: h.on.clone(),
            by: handoff::COMPONENT.to_string(),
        }),
    })
}

/// Runs the composition: route, an optional planner, the first executor
/// through the explore-then-delegate path, the checks and support, an
/// escalation, and one repair.
///
/// # Errors
///
/// Returns a message when a first executor can't be made or a record
/// can't be written. A failed session is an outcome, not an error.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn run<J, G, S, F>(
    setup: &Setup<'_>,
    state: &mut State,
    plan: &Plan<'_>,
    judge: &mut J,
    generator: &mut G,
    shell: &mut S,
    factory: &mut F,
    checkpoint: &mut dyn FnMut(&State),
) -> Result<Composed, String>
where
    J: Explorer,
    G: Generate,
    S: Shell,
    F: Factory,
{
    let manifest = setup.manifest;
    let control = &manifest.policy.control;
    let horizon = control.horizon.clone().unwrap_or_default();
    let verify = manifest.policy.verify.clone().unwrap_or(VerifyPolicy {
        checks: true,
        support: false,
        repair: None,
    });
    let total = setup.deadline.remaining().map(|left| left.as_secs());
    let long = horizon.long(total);
    let fallback = manifest.policy.executor.deadline_sec;
    let (routed, route_record) = route(setup, total).await;
    let mut first = routed.tier.clone();
    let handoff = control
        .handoff
        .clone()
        .filter(|h| matches!(h.pattern, Pattern::Escalate | Pattern::PlannerWorker));
    // An escalation to the executor that already started is no escalation.
    let escalate = handoff
        .clone()
        .filter(|h| h.pattern == Pattern::Escalate)
        .and_then(|h| h.to.clone().map(|to| (h, to)))
        .filter(|(_, to)| to.agent != first.agent || to.model != first.model);
    let mut escalate_to = escalate.as_ref().map(|(_, to)| to.clone());
    if long && let Some(effort) = &horizon.long_effort {
        first.effort = Some(effort.clone());
        if let Some(to) = &mut escalate_to {
            to.effort = Some(effort.clone());
        }
    }
    let mut runs: u32 = 0;
    let mut branches: Vec<Value> = Vec::new();
    let mut handoffs: Vec<Value> = Vec::new();
    let (budget, command_sec) = horizon.checks(total);
    let subject = Subject {
        label: "terminal-bench task".to_string(),
        task: TaskText {
            title: state.issue.title.clone(),
            instruction: setup.instruction.to_string(),
        },
        requirements: Some(judge.jev().requirements.clone()),
        provided: Vec::new(),
        inputs: (setup.instruction.contains("logs/") && setup.workdir.join("logs").is_dir())
            .then(|| "logs".to_string()),
        budget,
        live: Some(generic::Workspace {
            dir: setup.workdir.to_string_lossy().into_owned(),
            claimed: Vec::new(),
            command_sec,
        }),
    };

    // planner-worker: the planner writes a plan in a scratch copy first.
    let mut directions = plan.directions.to_string();
    let mut planned = Value::Null;
    if let Some(h) = handoff
        .as_ref()
        .filter(|h| h.pattern == Pattern::PlannerWorker)
        && let Some(planner) = h.to.clone()
    {
        planned = plan_first(setup, &planner, factory, &mut runs, &horizon, &mut branches).await;
        if let Some(text) = planned["plan"].as_str().filter(|t| !t.trim().is_empty()) {
            directions.push_str(&format!(
                "\n\n## The plan from {}\n\nA planner read this task and wrote the plan below. Follow it, and check each scenario it names before you finish.\n\n{}\n",
                planner.label(),
                text.trim()
            ));
            handoffs
                .push(json!({ "action": "plan", "from": planner.label(), "to": first.label() }));
        }
    }

    // The first executor.
    let follows = if escalate.is_some() {
        horizon.first_share
    } else if verify.repair.is_some() {
        horizon.later_share
    } else {
        1.0
    };
    let first_sec = horizon.dispatch_sec(setup.deadline.allowance(), fallback, follows);
    let mut exec = factory.make(&first, Duration::from_secs(first_sec), runs)?;
    exec.watch(monitor_for(
        setup,
        escalate.as_ref().map(|(h, _)| h),
        first_sec,
    ));
    let steps_before = setup.recorder.steps().len();
    let plan_first = Plan {
        mode: plan.mode,
        policy: plan.policy,
        max_steps: plan.max_steps,
        prompt: plan.prompt,
        instruction: plan.instruction,
        directions: &directions,
        cap: plan.cap,
        packer: plan.packer,
        pack: plan.pack,
        isolation: plan.isolation,
        base: plan.base,
    };
    let (mut ended, delegated) = delegate::explore_then_delegate(
        state,
        &plan_first,
        judge,
        generator,
        shell,
        &mut exec,
        setup.recorder,
        checkpoint,
    )
    .await;
    runs = exec.runs();
    let last = exec.last();
    if let Some(d) = &delegated {
        branches.push(row("primary", &first, &d.report, &last, first_sec));
    }
    drop(exec);
    let Some(first_delegation) = delegated.as_ref() else {
        // The explorer finished without delegating: nothing to verify here.
        let record = json!({ "schema": SCHEMA, "route": route_record, "branches": branches, "note": "the explorer ended without delegating" });
        return Ok(Composed {
            ended,
            delegated,
            record,
        });
    };

    // verify.checks and verify.support on what the first executor left.
    let mut checks_log = Vec::new();
    let check = |file: &'static str| {
        let mut subject = subject.clone();
        if let Some(live) = &mut subject.live {
            live.claimed = claimed(setup.recorder);
        }
        async move {
            checks::check_subject_as(&subject, setup.workdir, setup.dir, setup.recorder, file).await
        }
    };
    let mut checked = if verify.checks {
        Some(check(checks::COVERAGE_FILE).await)
    } else {
        None
    };
    if let Some((_, report)) = &checked {
        checks_log.push(json!({ "after": "primary", "file": checks::COVERAGE_FILE, "summary": report.summary() }));
    }
    let mut support = judge_support(setup, &verify, checked.as_ref()).await?;

    // control.handoff escalate.
    let mut escalated = false;
    if let Some((policy, to)) = &escalate {
        let to = escalate_to.clone().unwrap_or_else(|| to.clone());
        let stopped = last["stopped_by"].as_str() == Some(handoff::COMPONENT);
        let failed_check = checked.as_ref().is_some_and(|(_, r)| r.detected());
        let contradicted = support
            .as_ref()
            .is_some_and(|s| s.states.iter().any(|x| x.state == "contradicted"));
        let unanswered = first_delegation.report.status != Status::Answered && !stopped;
        let trigger = [
            (stopped, "the monitor stopped a stalled session"),
            (failed_check, "a check failed"),
            (contradicted, "a requirement was judged contradicted"),
            (unanswered, "the session ended without an answer"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, why)| *why)
        .collect::<Vec<_>>();
        let left = setup.deadline.allowance();
        let room = left.is_none_or(|left| left.as_secs() >= horizon.min_dispatch_sec.min(60));
        if !trigger.is_empty() && policy.max_handoffs > 0 && room {
            let trigger = trigger.join("; ");
            let section = handoff_section(
                &first,
                &trigger,
                checked.as_ref().map(|(_, r)| r),
                support.as_ref(),
                &delegate::changes(setup.workdir, setup.base),
                &last_errors(setup.recorder, steps_before, 2),
            );
            let mut briefing = first_delegation.briefing.clone();
            briefing.text.push_str(&section);
            briefing.included.push("handoff brief".to_string());
            let follows = if verify.repair.is_some() {
                horizon.later_share
            } else {
                1.0
            };
            let sec = horizon.dispatch_sec(setup.deadline.allowance(), fallback, follows);
            let record = json!({
                "pattern": "escalate",
                "action": "escalate",
                "from": first.label(),
                "to": to.label(),
                "trigger": trigger,
                "brief_chars": section.chars().count(),
                "requested_sec": sec,
                "remaining_ms": setup.deadline.allowance().map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
            });
            record_decision(setup.recorder, handoff::COMPONENT, "escalate", &record);
            println!(
                "  handoff ▸ escalate {} → {} ({trigger})",
                first.label(),
                to.label()
            );
            handoffs.push(record);
            let mut exec = factory.make(&to, Duration::from_secs(sec), runs)?;
            // The second executor runs under a shadow monitor: it records,
            // and nothing follows it to hand off to.
            exec.watch(monitor_for(setup, None, sec));
            let session = setup.recorder.enter(
                Start::new(
                    "exec.session",
                    Implementation::new(
                        "exec.session",
                        &format!("{} {}", exec.agent(), exec.model()),
                        &json!({ "agent": exec.agent(), "model": exec.model(), "deadline_sec": sec, "role": "escalation" }),
                    ),
                )
                .named(&format!("escalation · {} ({})", exec.agent(), exec.model()))
                .reading_digest(briefing.sha256())
                .with_effects(),
            );
            let reason = Reason::Handoff(trigger.clone());
            let report = delegate::delegate(
                &mut exec,
                &briefing,
                &Delegation {
                    mode: Mode::Always,
                    reason: &reason,
                    isolation: plan.isolation,
                },
                setup.recorder,
                runs,
            )
            .await;
            setup.recorder.end(
                &session,
                Finish::new(if report.status == Status::Answered {
                    Outcome::Completed
                } else {
                    Outcome::Failed
                })
                .summary(
                    json!({ "status": report.status.word(), "milliseconds": report.milliseconds }),
                ),
            );
            runs = exec.runs();
            branches.push(row("escalation", &to, &report, &exec.last(), sec));
            ended = delegate::ending(&ended, &report, state.history.len(), &state.issue.title);
            first = to.clone();
            escalated = true;
            if verify.checks {
                checked = Some(check(ESCALATED_CHECKS).await);
                if let Some((_, report)) = &checked {
                    checks_log.push(json!({ "after": "escalation", "file": ESCALATED_CHECKS, "summary": report.summary() }));
                }
                support = judge_support(setup, &verify, checked.as_ref()).await?;
            }
        } else if !trigger.is_empty() {
            handoffs.push(json!({ "action": "none", "why": "no time left for an escalation", "trigger": trigger.join("; ") }));
        }
    }

    // verify.repair: one fresh session from the packets.
    let mut repaired = Value::Null;
    if let (Some(policy), Some((input, report))) = (verify.repair, checked.as_ref()) {
        let place = crate::repair::Place {
            task: None,
            subject: &subject,
            work: setup.workdir,
            dir: setup.dir,
            artifacts: &setup.dir.join("artifacts"),
            recorder: setup.recorder,
            deadline: setup.deadline,
            jev: verify.support.then(|| setup.jev.clone()),
            previous_session: branches
                .last()
                .and_then(|b| b["session_id"].as_str().map(str::to_string)),
        };
        let allowance =
            Duration::from_secs(horizon.dispatch_sec(setup.deadline.allowance(), fallback, 1.0));
        let tier = first.clone();
        let made_runs = runs;
        let result = crate::repair::attempt(
            &place,
            (input, report),
            support.as_ref(),
            crate::repair::Policy {
                kind: policy.brief,
                trigger: policy.trigger,
                allowance,
            },
            |granted| factory.make(&tier, granted, made_runs),
        )
        .await?;
        repaired = json!({
            "tier": first,
            "triggered": result.record["triggered"],
            "ran": result.ran,
            "changed": result.changed,
            "skipped": result.record["skipped"],
            "session": result.record["session"],
            "recheck": result.record["recheck"],
            "cost_usd": result.cost_usd,
        });
    }

    let record = json!({
        "schema": SCHEMA,
        "route": route_record,
        "first": routed,
        "horizon": {
            "policy": horizon,
            "episode_deadline_sec": total,
            "long": long,
            "first_dispatch_sec": first_sec,
            "check_budget": budget,
            "command_sec": command_sec,
        },
        "planner": planned,
        "handoffs": handoffs,
        "escalated": escalated,
        "branches": branches,
        "checks": checks_log,
        "support": support.as_ref().map(crate::support::Report::summary),
        "repair": repaired,
        "verify": verify,
    });
    Ok(Composed {
        ended,
        delegated,
        record,
    })
}

async fn judge_support(
    setup: &Setup<'_>,
    verify: &VerifyPolicy,
    checked: Option<&(checks::Input, checks::Report)>,
) -> Result<Option<crate::support::Report>, String> {
    let (Some((input, report)), true) = (checked, verify.support) else {
        return Ok(None);
    };
    if matches!(setup.jev, JevMode::Off) {
        return Ok(None);
    }
    let judged = crate::support::judge(
        input,
        report,
        &setup.jev,
        setup.recorder,
        crate::support::Params::default(),
        Some(setup.deadline.clone()),
    )
    .await;
    crate::support::save(&judged, setup.dir)?;
    Ok(Some(judged))
}

/// The planner's dispatch in a scratch copy of the workspace, where
/// nothing it writes counts. Returns its record, with the plan's text.
async fn plan_first<F: Factory>(
    setup: &Setup<'_>,
    planner: &Tier,
    factory: &mut F,
    runs: &mut u32,
    horizon: &Horizon,
    branches: &mut Vec<Value>,
) -> Value {
    let scratch = std::env::temp_dir().join(format!("coder-one-planner-{}", atif::now_ms()));
    let size = tree_size(setup.workdir, 20_000);
    let Some((files, bytes)) = size.filter(|(_, bytes)| *bytes <= 256 * 1024 * 1024) else {
        let why = "the workspace is too large to copy for a planner";
        record_decision(
            setup.recorder,
            handoff::COMPONENT,
            "plan skipped",
            &json!({ "why": why }),
        );
        return json!({ "skipped": why });
    };
    if let Err(error) = handoff::copy_tree(setup.workdir, &scratch) {
        return json!({ "skipped": format!("cannot copy the workspace: {error}") });
    }
    let sec = horizon
        .dispatch_sec(setup.deadline.allowance(), 600, 0.15)
        .min(1_800);
    let text = format!(
        "{}\n\n## Your role: planner\n\nYou are in a scratch copy of the task's workspace; nothing you change here counts. Don't change any file. Read what you need, then reply with a numbered plan an implementer can follow, and the scenarios that would show the task done, one per line starting with `Scenario:`.\n",
        setup.instruction.trim()
    );
    let briefing = Briefing {
        cap: text.chars().count(),
        text,
        included: vec!["task".to_string(), "planner role".to_string()],
        omitted: Vec::new(),
    };
    let made = factory
        .make(planner, Duration::from_secs(sec), *runs)
        .map(|mut exec| {
            match &mut exec {
                Exec::Cli(cli) => cli.workdir.clone_from(&scratch),
                Exec::Scripted(scripted) => scripted.workdir.clone_from(&scratch),
            }
            exec
        });
    let record = match made {
        Err(error) => json!({ "skipped": error }),
        Ok(mut exec) => {
            let reason = Reason::Handoff("planner-worker: the planner writes the plan".to_string());
            let report = delegate::delegate(
                &mut exec,
                &briefing,
                &Delegation {
                    mode: Mode::Always,
                    reason: &reason,
                    isolation: "a scratch copy of the workspace",
                },
                setup.recorder,
                *runs,
            )
            .await;
            *runs = exec.runs();
            branches.push(row("planner", planner, &report, &exec.last(), sec));
            let plan = (report.status == Status::Answered).then(|| report.output());
            let record = json!({
                "tier": planner,
                "status": report.status.word(),
                "plan_chars": plan.as_ref().map(|p| p.chars().count()),
                "workspace_files": files,
                "workspace_bytes": bytes,
                "plan": plan,
            });
            record_decision(setup.recorder, handoff::COMPONENT, "plan", &record);
            record
        }
    };
    let _ = std::fs::remove_dir_all(&scratch);
    record
}

/// Files and bytes under `dir`, `.git` included, or `None` past `max`
/// files.
fn tree_size(dir: &Path, max: usize) -> Option<(usize, u64)> {
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    let (mut files, mut bytes) = (0usize, 0u64);
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).ok()?.flatten() {
            let kind = entry.file_type().ok()?;
            if kind.is_dir() {
                stack.push(entry.path());
            } else {
                files += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                if files > max {
                    return None;
                }
            }
        }
    }
    Some((files, bytes))
}

#[cfg(test)]
mod tests;
