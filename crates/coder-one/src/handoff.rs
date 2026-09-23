//! `control.handoff`: hand work to another executor, split it, or race it,
//! within one episode budget.
//!
//! Every episode used to run one executor from start to finish. A handoff
//! policy reads the monitor's judgments ([`crate::monitor`]), the checks'
//! results, and the budget, and picks one action: continue, steer, escalate
//! to another executor, split the work, or stop. Four patterns compose
//! those actions, and each one is a policy manifest's `control.handoff`:
//!
//! - **escalate**: the first executor runs under an acting monitor; when
//!   it stalls or repeats a failure, the host stops it and starts the
//!   second executor from a handoff brief.
//! - **planner-worker**: the second executor plans in a scratch copy of
//!   the task, where nothing it writes counts, and names the scenarios that
//!   would show the task done; the first executor implements from the plan.
//! - **steer**: the first executor keeps running, and the monitor's steer
//!   carries the last errors and the open requirements into the session.
//!   Only an adapter that demonstrated steering may run it.
//! - **race**: both executors run at once, each in its own copy of the
//!   task's state. The first to end with its checks passing wins; the host
//!   stops the other and waits for its cleanup before it copies the
//!   winner's state back. Both are charged.
//!
//! The handoff brief is built by code, never by a model: the requirement
//! states and diagnostic packets `verify.checks` finds in the workspace as
//! it is, what changed in the workspace, and the last failing commands.
//!
//! One [`Ledger`] holds the episode deadline. Each branch gets what is left
//! of it, a race charges wall time once and money twice, and every branch
//! is a row with its executor, time, cost, and how it ended.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use atif::document::{Source, Step};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::component::jev::JevMode;
use crate::delegate::{Agent, Briefing, BriefingInputs, Executor, Report};
use crate::minitask::MiniTask;
use crate::monitor::{Acting, Params as MonitorParams, Setup};
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};
use crate::scripted::{Act, SCRIPT_SCHEMA, Script, Scripted, Timed};
use crate::session::{self, Capabilities, Controller, Intent, Session};
use crate::stream::Format;

/// The component's ID.
pub const COMPONENT: &str = "control.handoff";

/// The step extension that holds one handoff.
pub const KEY: &str = "handoff";

/// The schema of a handoff brief's record.
pub const BRIEF_SCHEMA: &str = "openagents.coder-one.handoff-brief.v1";

/// The schema of a pattern comparison.
pub const COMPARE_SCHEMA: &str = "openagents.coder-one.handoff-compare.v1";

/// The isolation a race needs: each branch in its own copy of the task's
/// scratch directory.
pub const SCRATCH_COPY: &str = "scratch-copy";

/// A composition pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Pattern {
    /// One executor from start to finish.
    Single,
    Escalate,
    PlannerWorker,
    Steer,
    Race,
}

impl Pattern {
    /// The pattern's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Pattern::Single => "single",
            Pattern::Escalate => "escalate",
            Pattern::PlannerWorker => "planner-worker",
            Pattern::Steer => "steer",
            Pattern::Race => "race",
        }
    }
}

/// An executor: its adapter and model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tier {
    /// `claude-code`, `codex`, or `scripted`.
    pub agent: String,
    pub model: String,
}

impl Tier {
    #[must_use]
    pub fn new(agent: &str, model: &str) -> Self {
        Tier {
            agent: agent.to_string(),
            model: model.to_string(),
        }
    }

    /// `codex/gpt-6-luna`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}/{}", self.agent, self.model)
    }

    /// The model's family: `luna`, `opus`, `haiku`, `sonnet`, or `other`.
    #[must_use]
    pub fn family(&self) -> &'static str {
        ["luna", "opus", "haiku", "sonnet"]
            .into_iter()
            .find(|name| self.model.contains(name))
            .unwrap_or("other")
    }

    /// What the tier's adapter has demonstrated.
    #[must_use]
    pub fn capabilities(&self) -> Capabilities {
        match self.agent.as_str() {
            "claude-code" => crate::adapter::capabilities(Agent::ClaudeCode).0,
            "codex" => crate::adapter::capabilities(Agent::Codex).0,
            _ => Capabilities::all(),
        }
    }
}

/// What a handoff brief carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contents {
    #[serde(default = "yes")]
    pub requirements: bool,
    #[serde(default = "yes")]
    pub diff: bool,
    #[serde(default = "yes")]
    pub errors: bool,
    #[serde(default = "yes")]
    pub packets: bool,
}

impl Default for Contents {
    fn default() -> Self {
        Contents {
            requirements: true,
            diff: true,
            errors: true,
            packets: true,
        }
    }
}

fn yes() -> bool {
    true
}
fn default_on() -> Vec<String> {
    vec!["stalled".to_string(), "repeating".to_string()]
}
fn two() -> usize {
    2
}
fn one() -> usize {
    1
}

/// A handoff policy: a manifest's `control.handoff`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub pattern: Pattern,
    /// The executor escalated to, the planner, or the second racer. The
    /// manifest's own executor starts, implements, or races first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Tier>,
    /// The monitor flags that trigger an escalation or a steer.
    #[serde(default = "default_on")]
    pub on: Vec<String>,
    /// How many judgments in a row must flag.
    #[serde(default = "two")]
    pub after: usize,
    /// The most handoffs in one episode.
    #[serde(default = "one")]
    pub max_handoffs: usize,
    #[serde(default)]
    pub brief: Contents,
    /// How a race isolates each branch's state: only `scratch-copy`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation: Option<String>,
    /// The monitor behind the trigger; rules only, with a one-minute
    /// silence, when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<MonitorParams>,
}

impl Policy {
    /// A single-pass policy.
    #[must_use]
    pub fn single() -> Self {
        Policy {
            pattern: Pattern::Single,
            to: None,
            on: default_on(),
            after: two(),
            max_handoffs: one(),
            brief: Contents::default(),
            isolation: None,
            monitor: None,
        }
    }

    /// Whether the policy can run with `first` starting: [`Policy::check`],
    /// and a race only between sessions the host drives in lockstep.
    ///
    /// # Errors
    ///
    /// Returns why the policy can't run.
    pub fn validate(&self, first: &Tier) -> Result<(), String> {
        self.check(first)?;
        if self.pattern == Pattern::Race {
            let lockstep = |tier: &Tier| tier.agent == "scripted";
            if !lockstep(first) || !self.to.as_ref().is_some_and(lockstep) {
                return Err(
                    "a race drives two sessions in lockstep, which only the scripted adapter has demonstrated"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    /// What a manifest's validation checks: each pattern's second
    /// executor, a steer only on an adapter that demonstrated it, a stop
    /// for an escalation, and isolated state for a race.
    ///
    /// # Errors
    ///
    /// Returns why the policy can't run.
    pub fn check(&self, first: &Tier) -> Result<(), String> {
        for flag in &self.on {
            if !crate::monitor::Flags::NAMES.contains(&flag.as_str()) && flag != "intervene" {
                return Err(format!("control.handoff.on names an unknown flag, {flag}"));
            }
        }
        if self.after == 0 || self.max_handoffs == 0 {
            return Err("control.handoff.after and max_handoffs must be at least 1".to_string());
        }
        let needs_to = matches!(
            self.pattern,
            Pattern::Escalate | Pattern::PlannerWorker | Pattern::Race
        );
        if needs_to && self.to.is_none() {
            return Err(format!(
                "control.handoff {} needs a second executor in `to`",
                self.pattern.word()
            ));
        }
        if self.pattern == Pattern::Steer && !first.capabilities().steer {
            return Err(format!(
                "control.handoff steer needs an adapter that demonstrated steering, and the {} adapter has not",
                first.agent
            ));
        }
        if matches!(self.pattern, Pattern::Escalate) && !first.capabilities().stop {
            return Err(format!(
                "control.handoff escalate stops the first executor, and the {} adapter has not demonstrated stop",
                first.agent
            ));
        }
        if self.pattern == Pattern::Race && self.isolation.as_deref() != Some(SCRATCH_COPY) {
            return Err(format!(
                "a race needs each branch's state isolated: set control.handoff.isolation to {SCRATCH_COPY}"
            ));
        }
        Ok(())
    }

    /// The implementation record.
    #[must_use]
    pub fn implementation(&self) -> Implementation {
        Implementation::new(
            COMPONENT,
            &format!("{} handoff", self.pattern.word()),
            &json!({ "version": 1, "policy": self }),
        )
    }
}

/// One branch of an episode: one executor's session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub n: usize,
    /// `primary`, `escalation`, `planner`, `worker`, or `racer`.
    pub role: String,
    pub tier: Tier,
    /// Episode time when it started, in milliseconds.
    pub started_ms: u64,
    /// How long it ran.
    pub ms: u64,
    pub usd: Option<f64>,
    /// `reported` by the executor, `modeled` from a scripted tier's rate,
    /// or `unknown`.
    pub usd_basis: String,
    /// The session's status: `answered`, `timed_out`, and so on.
    pub status: String,
    /// Who stopped it, when someone did.
    pub stopped_by: Option<String>,
    /// Whether a stopped branch's process was reaped before the episode
    /// moved on.
    pub reaped: bool,
    /// A racer's result.
    pub won: Option<bool>,
}

/// The episode's one budget: a deadline every branch draws from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    pub deadline_ms: u64,
    /// Episode time used so far.
    pub clock_ms: u64,
    pub branches: Vec<Branch>,
}

impl Ledger {
    #[must_use]
    pub fn new(deadline_ms: u64) -> Self {
        Ledger {
            deadline_ms,
            clock_ms: 0,
            branches: Vec::new(),
        }
    }

    /// What is left of the deadline.
    #[must_use]
    pub fn remaining_ms(&self) -> u64 {
        self.deadline_ms.saturating_sub(self.clock_ms)
    }

    /// Every branch's cost, when every one is known.
    #[must_use]
    pub fn usd(&self) -> Option<f64> {
        self.branches.iter().map(|b| b.usd).sum()
    }

    /// Charges branches that ran at once: the clock moves by the longest,
    /// and every one is charged its own cost.
    pub fn charge(&mut self, mut branches: Vec<Branch>) {
        let longest = branches.iter().map(|b| b.ms).max().unwrap_or(0);
        for branch in &mut branches {
            branch.n = self.branches.len() + 1;
            branch.started_ms = self.clock_ms;
            self.branches.push(branch.clone());
        }
        self.clock_ms += longest;
    }

    /// The ledger's totals.
    #[must_use]
    pub fn record(&self) -> Value {
        json!({
            "deadline_ms": self.deadline_ms,
            "clock_ms": self.clock_ms,
            "remaining_ms": self.remaining_ms(),
            "usd": self.usd().map(|usd| (usd * 1e6).round() / 1e6),
            "branches": self.branches,
        })
    }
}

/// A scripted tier's rate, from the retained arms' mean spend per second
/// of dispatch time.
#[must_use]
pub fn usd_per_second(family: &str) -> (f64, &'static str) {
    match family {
        "luna" => (
            0.000_064,
            "coder-one-jevprobe3-luna: $0.0032 over 50 s per dispatch",
        ),
        "opus" => (
            0.002_44,
            "coder-one-jevprobe2-opus-lean-low-5m: $0.0532 over 22 s per dispatch",
        ),
        "haiku" => (
            0.001_96,
            "coder-one-jevprobe-haiku-lean: $0.1583 over 81 s per dispatch",
        ),
        "sonnet" => (
            0.002_81,
            "coder-one-jevprobe-sonnet-lean-low: $0.1251 over 44 s per dispatch",
        ),
        _ => (0.0, "no retained arm"),
    }
}

/// The part of the workspace a brief reads: every file but `.git`, with
/// its content when it is text.
fn snapshot(dir: &Path) -> BTreeMap<String, Option<String>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".git" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                files.insert(relative, std::fs::read_to_string(&path).ok());
            }
        }
    }
    files
}

/// Copies `from` into `to`, `.git` included, replacing what `to` held.
///
/// # Errors
///
/// Returns a message when a file can't be copied.
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        std::fs::remove_dir_all(to).map_err(|error| format!("{}: {error}", to.display()))?;
    }
    std::fs::create_dir_all(to).map_err(|error| format!("{}: {error}", to.display()))?;
    for entry in std::fs::read_dir(from)
        .map_err(|error| format!("{}: {error}", from.display()))?
        .flatten()
    {
        let path = entry.path();
        let target = to.join(entry.file_name());
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            copy_tree(&path, &target)?;
        } else if kind.is_symlink() {
            let link = std::fs::read_link(&path).map_err(|error| error.to_string())?;
            std::os::unix::fs::symlink(link, &target).map_err(|error| error.to_string())?;
        } else {
            std::fs::copy(&path, &target)
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

/// A handoff brief, built by code from the workspace and the session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brief {
    pub pattern: String,
    pub trigger: String,
    pub from: String,
    pub to: String,
    /// Each requirement's state as `verify.checks` found it.
    pub requirements: Vec<Value>,
    /// Files added, changed, or removed since the episode started.
    pub diff: Vec<Value>,
    pub errors: Vec<String>,
    /// Failed scenarios' diagnostic packets.
    pub packets: Vec<Value>,
    /// A planner's plan, for a worker.
    pub plan: Option<String>,
    pub remaining_ms: u64,
}

impl Brief {
    /// The brief's section of the next briefing.
    #[must_use]
    pub fn section(&self) -> String {
        let mut text = if let Some(plan) = &self.plan {
            format!(
                "\n\n## The plan from {}\n\nA planner read this task and wrote the plan below. Follow it, and check each scenario it names before you finish.\n\n{}\n",
                self.from,
                plan.trim()
            )
        } else {
            format!(
                "\n\n## Handoff from {} ({}: {})\n\nAnother executor worked on this task first and was stopped. Continue from the workspace as it is now; don't start over, and don't repeat what failed.\n",
                self.from, self.pattern, self.trigger
            )
        };
        if !self.requirements.is_empty() {
            text.push_str("\n### Requirement states\n\n");
            for r in &self.requirements {
                text.push_str(&format!(
                    "- {} ({}): {}\n",
                    r["id"].as_str().unwrap_or("?"),
                    r["state"].as_str().unwrap_or("unobserved"),
                    crate::judge::clip(r["text"].as_str().unwrap_or_default(), 240)
                ));
            }
        }
        if !self.diff.is_empty() {
            text.push_str("\n### What changed in the workspace\n");
            for file in &self.diff {
                text.push_str(&format!(
                    "\n- `{}` ({})\n",
                    file["path"].as_str().unwrap_or("?"),
                    file["change"].as_str().unwrap_or("?")
                ));
                if let Some(excerpt) = file["excerpt"].as_str().filter(|e| !e.is_empty()) {
                    text.push_str(&format!("\n```\n{excerpt}\n```\n"));
                }
            }
        }
        if !self.errors.is_empty() {
            text.push_str("\n### The last errors\n\n");
            for error in &self.errors {
                text.push_str(&format!("- {error}\n"));
            }
        }
        if !self.packets.is_empty() {
            text.push_str("\n### What the checks found\n\n");
            for packet in &self.packets {
                text.push_str(&format!(
                    "- {} · {}: {}{}\n",
                    packet["requirement"].as_str().unwrap_or("?"),
                    packet["scenario"].as_str().unwrap_or("?"),
                    crate::judge::clip(&packet["observations"].to_string(), 400),
                    packet["hypotheses"]
                        .as_array()
                        .filter(|h| !h.is_empty())
                        .map_or(String::new(), |h| format!(
                            "; hypotheses: {}",
                            h.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("; ")
                        ))
                ));
            }
        }
        text
    }

    /// The brief as a record: its size and digest, and what it carried.
    #[must_use]
    pub fn record(&self) -> Value {
        let section = self.section();
        json!({
            "schema": BRIEF_SCHEMA,
            "chars": section.chars().count(),
            "sha256": format!("{:x}", Sha256::digest(section.as_bytes())),
            "requirements": self.requirements.iter().map(|r| json!({ "id": r["id"], "state": r["state"] })).collect::<Vec<_>>(),
            "diff": self.diff.iter().map(|f| json!({ "path": f["path"], "change": f["change"] })).collect::<Vec<_>>(),
            "errors": self.errors.len(),
            "packets": self.packets.len(),
            "plan": self.plan.as_ref().map(|plan| plan.chars().count()),
            "remaining_ms": self.remaining_ms,
            "text": section,
        })
    }
}

/// Builds a brief from the workspace as it is: `verify.checks` for the
/// requirement states and packets, the files changed since `before`, and
/// the session's last errors.
#[allow(clippy::too_many_arguments)]
pub async fn build_brief(
    task: &MiniTask,
    work: &Path,
    run_dir: &Path,
    before: &BTreeMap<String, Option<String>>,
    errors: Vec<String>,
    contents: Contents,
    labels: (&str, &str, &str, &str),
    remaining_ms: u64,
    recorder: &Recorder,
) -> Brief {
    let (pattern, trigger, from, to) = labels;
    let mut requirements = Vec::new();
    let mut packets = Vec::new();
    if contents.requirements || contents.packets {
        let report = crate::checks::check_workspace_as(
            task,
            work,
            run_dir,
            recorder,
            &format!("verification/handoff-checks-{}.json", atif::now_ms()),
        )
        .await
        .1;
        if contents.requirements {
            requirements = report
                .coverage
                .iter()
                .map(|c| json!({ "id": c.id, "text": c.text, "state": c.state }))
                .collect();
        }
        if contents.packets {
            packets = report
                .packets
                .iter()
                .map(|p| serde_json::to_value(p).unwrap_or(Value::Null))
                .collect();
        }
    }
    let mut diff = Vec::new();
    if contents.diff {
        let now = snapshot(work);
        for (path, content) in &now {
            let change = match before.get(path) {
                None => "added",
                Some(old) if old != content => "changed",
                Some(_) => continue,
            };
            let excerpt = content.as_deref().map_or(String::new(), |text| {
                crate::judge::clip(&text.lines().take(40).collect::<Vec<_>>().join("\n"), 2_000)
            });
            diff.push(json!({ "path": path, "change": change, "excerpt": excerpt }));
        }
        for path in before.keys().filter(|path| !now.contains_key(*path)) {
            diff.push(json!({ "path": path, "change": "removed", "excerpt": "" }));
        }
    }
    Brief {
        pattern: pattern.to_string(),
        trigger: trigger.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        requirements,
        diff,
        errors: if contents.errors { errors } else { Vec::new() },
        packets,
        plan: None,
        remaining_ms,
    }
}

/// Records one handoff: an invocation, and a step the Gym's timeline reads.
fn record_handoff(recorder: &Recorder, record: &Value) {
    let id = recorder.begin(
        Start::new(
            COMPONENT,
            Implementation::new(COMPONENT, "handoff decision", &json!({ "version": 1 })),
        )
        .named(record["action"].as_str().unwrap_or("handoff"))
        .with_effects(),
    );
    recorder.push(
        Step::said(
            Source::System,
            &format!(
                "handoff: {} {} → {} ({})",
                record["action"].as_str().unwrap_or("?"),
                record["from"].as_str().unwrap_or("?"),
                record["to"].as_str().unwrap_or("?"),
                record["trigger"].as_str().unwrap_or("?"),
            ),
        )
        .noting(KEY, record.clone()),
    );
    recorder.end(
        &id,
        Finish::new(Outcome::Completed)
            .output(record.clone())
            .cost(crate::record::Cost::none()),
    );
}

// ---------------------------------------------------------------------------
// Scripted tiers: known behavior for each mini-task, so a pattern composes
// end to end in seconds with no model.
// ---------------------------------------------------------------------------

/// What a scripted executor does on a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Works alone from the briefing.
    Solo,
    /// Plans only.
    Planner,
    /// Implements from a plan.
    Worker,
}

const S: u64 = 1_000;

fn timed(at_ms: u64, act: Act) -> Timed {
    Timed { at_ms, act }
}

fn claim(text: &str) -> Act {
    Act::Claim {
        text: text.to_string(),
    }
}

fn check(command: &str, output: &str, exit_code: i64) -> Act {
    Act::Command {
        command: command.to_string(),
        output: output.to_string(),
        exit_code,
    }
}

/// A task's good or bad actions, from its mini-task scripts: the writes
/// and commands, without claims or the end.
fn work_acts(task: &MiniTask, variant: &str) -> Vec<Act> {
    crate::minitask::scripts(task)
        .into_iter()
        .find(|(name, _)| *name == variant)
        .map(|(_, script)| {
            script
                .events
                .into_iter()
                .map(|timed| timed.act)
                .filter(|act| matches!(act, Act::Write { .. } | Act::Run { .. }))
                .collect()
        })
        .unwrap_or_default()
}

/// The command each task's executor checks its work with, and what a
/// failure and a pass print.
fn task_check(task: &MiniTask) -> (&'static str, &'static str, &'static str) {
    match task.id {
        "log-severity" => (
            "python3 summarize.py logs summary.csv && cat summary.csv",
            "period,severity,count\ntoday,ERROR,2 …",
            "period,severity,count\ntoday,ERROR,1 …",
        ),
        "interactive-terminal" => (
            "python3 check_terminal.py",
            "TimeoutError: python3 started in the terminal never read the typed line",
            "ok: the interactive program read the typed line",
        ),
        "cancel-cleanup" => (
            "python3 check_cancel.py",
            "AssertionError: after the interrupt, cleanup ran for 0 of 2 started tasks",
            "ok: both started tasks ran their cleanup after the interrupt",
        ),
        _ => (
            "git log --oneline -3 && git status --short",
            "HEAD is at the reset commit; feature.txt is missing",
            "feature.txt restored on master; the tree is clean",
        ),
    }
}

/// The plan a planner writes for a task: the insight the task turns on,
/// and the scenarios that would show it done.
fn task_plan(task: &MiniTask) -> &'static str {
    match task.id {
        "log-severity" => {
            "1. Parse each log line's severity field, the token after the timestamp; don't search the whole line, because messages mention other severities.\n2. Count per period against 2025-08-12 and write summary.csv with the header and nine rows.\nScenario: a line whose message says ERROR but whose severity field is INFO counts as INFO."
        }
        "interactive-terminal" => {
            "1. Start an interactive bash on a pseudo-terminal (pty.fork), not one subprocess per line.\n2. Send keystrokes to the terminal's file descriptor, including control characters.\nScenario: starting python3 in the terminal and typing a line reaches the program's input."
        }
        "cancel-cleanup" => {
            "1. Keep every started task; on cancellation or KeyboardInterrupt, cancel them and await asyncio.gather(..., return_exceptions=True) before re-raising.\n2. Bound concurrency with a semaphore.\nScenario: after an interrupt, every started task's finally block has run."
        }
        _ => {
            "1. Find the lost commit in the reflog.\n2. Reset master to it and leave the tree clean.\nScenario: git log shows the commit, and feature.txt has its original content."
        }
    }
}

fn scripted_script(
    task: &MiniTask,
    name: &str,
    events: Vec<Timed>,
    on_steer: Vec<Timed>,
) -> Script {
    Script {
        schema: SCRIPT_SCHEMA.to_string(),
        name: format!("{}-{name}", task.id),
        format: Format::Codex,
        model: name.to_string(),
        capabilities: Capabilities::all(),
        events,
        on_steer,
        on_resume: Vec::new(),
        rebind: Vec::new(),
        opening: true,
        briefed: None,
    }
}

/// A scripted tier's behavior on `task` in `role`. The strong family
/// (Opus) solves every task in about 22 seconds. The cheap families solve
/// the recovery task, write the whole-line parser and call it done on the
/// log task, and loop on a failing check on the terminal and cancellation
/// tasks until stopped; a steer carrying the failure, or a plan, leads
/// them to the fix. These are modeled behaviors, not measurements: they
/// exercise the composition, not an executor.
#[must_use]
pub fn tier_script(task: &MiniTask, tier: &Tier, role: Role) -> Script {
    let (command, fails, passes) = task_check(task);
    let label = format!(
        "{}-{}",
        tier.family(),
        match role {
            Role::Solo => "solo",
            Role::Planner => "planner",
            Role::Worker => "worker",
        }
    );
    let place = |start: u64, acts: Vec<Act>| -> Vec<Timed> {
        acts.into_iter()
            .enumerate()
            .map(|(i, act)| timed(start + i as u64 * S, act))
            .collect()
    };
    if role == Role::Planner {
        return scripted_script(
            task,
            &label,
            vec![
                timed(0, claim("Reading the task to plan it; changing nothing.")),
                timed(12 * S, claim(task_plan(task))),
                timed(12 * S, Act::End { error: false }),
            ],
            Vec::new(),
        );
    }
    let strong = tier.family() == "opus";
    if strong || role == Role::Worker {
        let finish = if strong { 22 * S } else { 30 * S };
        let mut events = vec![timed(0, claim("Reading the task and the files it names."))];
        events.extend(place(6 * S, work_acts(task, "good")));
        events.push(timed(finish - 7 * S, check(command, passes, 0)));
        events.push(timed(
            finish,
            claim("Done: the checks pass and the task is complete."),
        ));
        events.push(timed(finish, Act::End { error: false }));
        return scripted_script(task, &label, events, Vec::new());
    }
    match task.id {
        "git-recovery" => {
            let mut events = vec![timed(0, claim("Reading the reflog."))];
            events.extend(place(5 * S, work_acts(task, "good")));
            events.push(timed(18 * S, check(command, passes, 0)));
            events.push(timed(
                25 * S,
                claim("Restored the lost commit; the tree is clean."),
            ));
            events.push(timed(25 * S, Act::End { error: false }));
            scripted_script(task, &label, events, Vec::new())
        }
        "log-severity" => {
            let mut events = vec![timed(0, claim("Reading the logs."))];
            events.extend(place(8 * S, work_acts(task, "bad")));
            events.push(timed(20 * S, check(command, fails, 0)));
            events.push(timed(
                30 * S,
                claim("Done: summary.csv has the header and nine integer rows."),
            ));
            events.push(timed(30 * S, Act::End { error: false }));
            scripted_script(task, &label, events, Vec::new())
        }
        _ => {
            let mut events = vec![timed(0, claim("Reading the task."))];
            events.extend(place(8 * S, work_acts(task, "bad")));
            for at in [20, 35, 50, 65] {
                events.push(timed(at * S, check(command, fails, 1)));
            }
            events.push(timed(
                70 * S,
                claim("Still failing; trying the same fix again."),
            ));
            events.push(timed(75 * S, Act::Hang));
            let mut on_steer: Vec<Timed> = place(5 * S, work_acts(task, "good"));
            on_steer.push(timed(15 * S, check(command, passes, 0)));
            on_steer.push(timed(
                20 * S,
                claim("Done: fixed after the steer; the check passes."),
            ));
            on_steer.push(timed(20 * S, Act::End { error: false }));
            scripted_script(task, &label, events, on_steer)
        }
    }
}

// ---------------------------------------------------------------------------
// Running a pattern.
// ---------------------------------------------------------------------------

/// An executor for one branch.
enum Exec {
    Scripted(Box<Scripted>),
    Cli(Box<crate::minitask::run::Bounded>),
}

impl Exec {
    async fn execute(&mut self, briefing: &Briefing) -> Report {
        match self {
            Exec::Scripted(scripted) => scripted.execute(briefing).await,
            Exec::Cli(cli) => cli.execute(briefing).await,
        }
    }

    fn last(&self) -> Value {
        match self {
            Exec::Scripted(scripted) => scripted.last.clone().unwrap_or(Value::Null),
            Exec::Cli(cli) => cli.cli.control.last.clone().unwrap_or(Value::Null),
        }
    }
}

/// How one pattern run is set up.
pub struct Options {
    pub task: MiniTask,
    pub policy: Policy,
    /// The manifest's executor: the one that starts, implements, or races
    /// first.
    pub first: Tier,
    /// The directory runs are recorded under.
    pub out: PathBuf,
    /// The one episode deadline every branch draws from.
    pub deadline: Duration,
    /// Jev for a monitor whose parameters ask for it.
    pub jev: Option<jev::Client>,
    /// Whether `verify.checks` observes the final workspace before the
    /// grader.
    pub checks: bool,
}

/// What a pattern run left.
#[derive(Clone, Debug)]
pub struct Ran {
    pub dir: PathBuf,
    pub manifest: Value,
    pub grade: crate::minitask::Grade,
    pub ledger: Ledger,
    pub handoffs: Vec<Value>,
}

struct Run<'a> {
    task: MiniTask,
    options: &'a Options,
    dir: PathBuf,
    recorder: Recorder,
    ledger: Ledger,
    handoffs: Vec<Value>,
    before: BTreeMap<String, Option<String>>,
}

impl Run<'_> {
    fn monitor_params(&self) -> MonitorParams {
        self.options
            .policy
            .monitor
            .clone()
            .unwrap_or(MonitorParams {
                jev: false,
                silence_ms: Some(60_000),
                ..MonitorParams::default()
            })
    }

    fn setup(&self, acting: Option<Acting>) -> Setup {
        let params = self.monitor_params();
        Setup {
            jev: match (&self.options.jev, params.jev) {
                (Some(client), true) => JevMode::Live(client.clone()),
                _ => JevMode::Off,
            },
            params,
            task: self.task.instruction.to_string(),
            acting,
        }
    }

    /// An executor for `tier` in `dir`, bounded to `remaining_ms`.
    fn exec(
        &self,
        tier: &Tier,
        role: Role,
        dir: &Path,
        artifacts: &Path,
        remaining_ms: u64,
        monitor: Option<Setup>,
    ) -> Result<Exec, String> {
        let _ = std::fs::create_dir_all(artifacts);
        if tier.agent == "scripted" {
            let mut scripted =
                Scripted::new(tier_script(&self.task, tier, role), dir.to_path_buf());
            scripted.artifacts = Some(artifacts.to_path_buf());
            scripted.recorder = self.recorder.clone();
            scripted.deadline = Duration::from_millis(remaining_ms);
            scripted.controls = session::Controls {
                deadline_ms: remaining_ms,
                tick_ms: 100,
                ..session::Controls::default()
            };
            scripted.monitor = monitor;
            return Ok(Exec::Scripted(Box::new(scripted)));
        }
        let agent = Agent::parse(&tier.agent)?;
        let mut bounded = crate::minitask::run::bounded_cli(
            agent,
            &tier.model,
            dir,
            artifacts,
            Duration::from_millis(remaining_ms),
            crate::deadline::Deadline::unbounded(),
            &self.recorder,
            &session::Controls::default(),
            0,
        )?;
        bounded.cli.control.monitor = monitor;
        Ok(Exec::Cli(Box::new(bounded)))
    }

    /// Runs one branch and returns its ledger row, its report, and its
    /// host-loop record.
    async fn branch(
        &self,
        tier: &Tier,
        role: Role,
        label: &str,
        dir: &Path,
        briefing: &Briefing,
        monitor: Option<Setup>,
    ) -> Result<(Branch, Report, Value), String> {
        let remaining = self.ledger.remaining_ms();
        let mut exec = self.exec(
            tier,
            role,
            dir,
            &self.dir.join("artifacts").join(label),
            remaining,
            monitor,
        )?;
        let started = Instant::now();
        let report = exec.execute(briefing).await;
        let last = exec.last();
        let wall = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let (ms, usd, basis) = if tier.agent == "scripted" {
            let ms = last["elapsed_ms"].as_u64().unwrap_or(0);
            let (rate, _) = usd_per_second(tier.family());
            (ms, Some(rate * ms as f64 / 1_000.0), "modeled")
        } else {
            let usd = report.summary.total_cost_usd;
            (
                wall,
                usd,
                if usd.is_some() { "reported" } else { "unknown" },
            )
        };
        let stopped_by = last["stopped_by"].as_str().map(str::to_string);
        Ok((
            Branch {
                n: 0,
                role: label.to_string(),
                tier: tier.clone(),
                started_ms: 0,
                ms,
                usd,
                usd_basis: basis.to_string(),
                status: report.status.word().to_string(),
                reaped: stopped_by.is_some(),
                stopped_by,
                won: None,
            },
            report,
            last,
        ))
    }

    fn handoff(
        &mut self,
        action: &str,
        from: &Tier,
        to: &Tier,
        trigger: &str,
        brief: Option<&Brief>,
    ) {
        let record = json!({
            "pattern": self.options.policy.pattern.word(),
            "action": action,
            "from": from.label(),
            "to": to.label(),
            "trigger": trigger,
            "at_ms": self.ledger.clock_ms,
            "remaining_ms": self.ledger.remaining_ms(),
            "spent_usd": self.ledger.usd(),
            "brief": brief.map(Brief::record),
        });
        record_handoff(&self.recorder, &record);
        self.handoffs.push(record);
    }
}

fn base_briefing(task: &MiniTask) -> Briefing {
    let map = crate::requirements::mechanical(task.instruction);
    Briefing::build(
        &BriefingInputs {
            instruction: task.instruction.to_string(),
            requirements: map
                .criteria(12)
                .into_iter()
                .map(|text| (text, None))
                .collect(),
            files: Vec::new(),
            spans: Vec::new(),
            commands: Vec::new(),
            last_output: None,
            conclusion: String::new(),
            directions: crate::policy::Manifest::builtin()
                .policy
                .brief
                .directions
                .text()
                .to_string(),
        },
        crate::delegate::BRIEFING_CAP,
    )
}

fn extended(base: &Briefing, section: &str) -> Briefing {
    let mut briefing = base.clone();
    briefing.text.push_str(section);
    briefing
}

/// The last failing commands in a host-loop record's session, from the
/// recorder's executor events since `from_step`.
fn last_errors(recorder: &Recorder, from_step: usize, n: usize) -> Vec<String> {
    let steps = recorder.steps();
    let mut errors: Vec<String> = steps[from_step.min(steps.len())..]
        .iter()
        .rev()
        .filter_map(|step| {
            let event = step.extensions.get(session::EVENT_KEY)?.get("event")?;
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

/// Races two scripted sessions in lockstep, each in its own directory.
/// The first to end answered, with `verify.checks` finding no failed
/// scenario in its directory, wins; the host stops the other and waits for
/// its cleanup before it names the winner. Returns each racer's elapsed
/// time, the winner, and whether the loser was reaped.
async fn race(
    run: &Run<'_>,
    racers: &mut [(Scripted, PathBuf); 2],
    briefing: &Briefing,
    deadline_ms: u64,
) -> ([u64; 2], Option<usize>, bool, [String; 2]) {
    let mut controllers = [Controller::default(), Controller::default()];
    let mut running = [true, true];
    let mut ended = [0u64; 2];
    let mut judged = [false, false];
    let mut winner = None;
    let mut reaped = false;
    for (i, (scripted, _)) in racers.iter_mut().enumerate() {
        if scripted.start(briefing).await.is_err() {
            running[i] = false;
        } else {
            controllers[i].begin(0, "raced");
        }
    }
    let tick = 100;
    let mut now = 0;
    while running.iter().any(|r| *r) && winner.is_none() {
        now += tick;
        for i in 0..2 {
            if !running[i] {
                continue;
            }
            let (scripted, _) = &mut racers[i];
            let still = scripted.advance(now).await;
            for event in scripted.observe() {
                let observation = controllers[i].observe(now, event);
                run.recorder.push(session::observation_step(
                    &format!("scripted-racer-{}", i + 1),
                    &observation,
                ));
            }
            if !still {
                running[i] = false;
                ended[i] = now;
            }
        }
        for i in 0..2 {
            if running[i] || judged[i] || winner.is_some() {
                continue;
            }
            judged[i] = true;
            let (scripted, dir) = &mut racers[i];
            let answered = scripted.report().status.word() == "answered";
            let report = crate::checks::check_workspace_as(
                &run.task,
                dir,
                &run.dir,
                &run.recorder,
                &format!("verification/race-{}-checks.json", i + 1),
            )
            .await
            .1;
            if answered && !report.detected() {
                winner = Some(i);
                let other = 1 - i;
                if running[other] {
                    // Stop and reap the loser before naming the winner.
                    let ack = racers[other].0.stop(now, "lost the race").await;
                    reaped = ack.is_ok();
                    running[other] = false;
                    ended[other] = now;
                }
            }
        }
        if now >= deadline_ms {
            for i in 0..2 {
                if running[i] {
                    let _ = racers[i].0.stop(now, "the episode deadline passed").await;
                    running[i] = false;
                    ended[i] = now;
                }
            }
        }
    }
    let statuses = [
        racers[0].0.report().status.word().to_string(),
        racers[1].0.report().status.word().to_string(),
    ];
    (ended, winner, reaped, statuses)
}

/// Runs one pattern on one mini-task and records it as a mini-task run.
///
/// # Errors
///
/// Returns a message when the policy can't run with its first executor,
/// the run's directory or setup can't be made, or an executor can't be
/// bounded on this host.
pub async fn run(options: Options) -> Result<Ran, String> {
    options.policy.validate(&options.first)?;
    // A real executor runs only inside a filesystem boundary; refuse before
    // any directory is made when this host can't enforce one.
    if std::iter::once(&options.first)
        .chain(options.policy.to.as_ref())
        .any(|tier| tier.agent != "scripted")
    {
        let probe = std::env::temp_dir();
        coder_boundary::Boundary::writing(&probe)
            .build()
            .map_err(|error| format!("cannot bound the executor: {error}"))?;
    }
    let started = Instant::now();
    let task = options.task;
    let at = atif::now_ms();
    let pattern = options.policy.pattern;
    let label = format!(
        "{}-{}{}",
        pattern.word(),
        options.first.family(),
        options
            .policy
            .to
            .as_ref()
            .map_or(String::new(), |to| format!("-{}", to.family()))
    );
    let dir = options
        .out
        .join(format!("minitask-{}-{label}-{at}", task.id));
    let work = dir.join("work");
    for sub in [&work, &dir.join("artifacts"), &dir.join("verification")] {
        std::fs::create_dir_all(sub)
            .map_err(|error| format!("cannot create {}: {error}", sub.display()))?;
    }
    let id = format!("minitask-{}-{label}-{at}", task.id);
    let mut session_doc = atif::Session::opening(
        &id,
        "none",
        "mini-task",
        &work.to_string_lossy(),
        &crate::episode::version(),
    );
    session_doc.directive = "Complete this task.".to_string();
    let log_path = dir.join(crate::episode::INVOCATION_LOG);
    let log = atif::Log::create_at(&log_path, &session_doc)
        .map_err(|error| format!("cannot create {}: {error}", log_path.display()))?;
    let recorder = Recorder::durable(log);
    let episode = recorder.enter(
        Start::new(
            "episode",
            Implementation::new(
                "episode",
                &format!("mini-task {} under {}", task.id, pattern.word()),
                &json!({ "task": task.id, "policy": options.policy, "first": options.first }),
            ),
        )
        .named("mini-task")
        .reading(&json!({ "instruction": task.instruction }))
        .with_effects(),
    );
    recorder.push(Step::said(
        Source::System,
        &format!(
            "Mini-task {} ({}), {} pattern.",
            task.id,
            task.family,
            pattern.word()
        ),
    ));
    recorder.push(Step::said(Source::User, task.instruction));
    crate::minitask::setup(&task, &work)?;
    let deadline_ms = u64::try_from(options.deadline.as_millis()).unwrap_or(u64::MAX);
    let mut run = Run {
        task,
        options: &options,
        before: snapshot(&work),
        dir: dir.clone(),
        recorder: recorder.clone(),
        ledger: Ledger::new(deadline_ms),
        handoffs: Vec::new(),
    };
    let base = base_briefing(&task);
    let first = options.first.clone();
    let policy = options.policy.clone();
    let contents = policy.brief;
    match pattern {
        Pattern::Single => {
            let setup = run.setup(None);
            let (branch, _, _) = run
                .branch(&first, Role::Solo, "primary", &work, &base, Some(setup))
                .await?;
            run.ledger.charge(vec![branch]);
        }
        Pattern::Escalate | Pattern::Steer => {
            let acting = Acting {
                intent: if pattern == Pattern::Steer {
                    Intent::Steer
                } else {
                    Intent::Stop
                },
                after: policy.after,
                on: policy.on.clone(),
                by: COMPONENT.to_string(),
            };
            let steps_before = recorder.steps().len();
            let setup = run.setup(Some(acting));
            let (branch, _, last) = run
                .branch(&first, Role::Solo, "primary", &work, &base, Some(setup))
                .await?;
            let stopped = branch.stopped_by.as_deref() == Some(COMPONENT);
            let steered = last["actions"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|a| a["capability"] == "steer" && a["outcome"] == "done");
            run.ledger.charge(vec![branch]);
            if pattern == Pattern::Steer && steered {
                let errors = last_errors(&recorder, steps_before, 2);
                let brief = Brief {
                    pattern: pattern.word().to_string(),
                    trigger: format!("monitor: {}", policy.on.join(" or ")),
                    from: first.label(),
                    to: first.label(),
                    requirements: Vec::new(),
                    diff: Vec::new(),
                    errors,
                    packets: Vec::new(),
                    plan: None,
                    remaining_ms: run.ledger.remaining_ms(),
                };
                run.handoff(
                    "steer",
                    &first,
                    &first,
                    &brief.trigger.clone(),
                    Some(&brief),
                );
            }
            if pattern == Pattern::Escalate && stopped {
                let to = policy.to.clone().expect("validated");
                let trigger = format!(
                    "monitor: {} {} times in a row",
                    policy.on.join(" or "),
                    policy.after
                );
                let brief = build_brief(
                    &task,
                    &work,
                    &dir,
                    &run.before,
                    last_errors(&recorder, steps_before, 2),
                    contents,
                    (pattern.word(), &trigger, &first.label(), &to.label()),
                    run.ledger.remaining_ms(),
                    &recorder,
                )
                .await;
                run.handoff("escalate", &first, &to, &trigger, Some(&brief));
                let briefing = extended(&base, &brief.section());
                let setup = run.setup(None);
                let (branch, _, _) = run
                    .branch(&to, Role::Solo, "escalation", &work, &briefing, Some(setup))
                    .await?;
                run.ledger.charge(vec![branch]);
            }
        }
        Pattern::PlannerWorker => {
            let planner = policy.to.clone().expect("validated");
            let scratch = dir.join("planner-scratch");
            copy_tree(&work, &scratch)?;
            let planning = extended(
                &base,
                "\n\n## Your role: planner\n\nDon't change any file. Reply with a numbered plan an implementer can follow, and the scenarios that would show the task done, one per line starting with `Scenario:`.\n",
            );
            let (branch, report, _) = run
                .branch(
                    &planner,
                    Role::Planner,
                    "planner",
                    &scratch,
                    &planning,
                    None,
                )
                .await?;
            run.ledger.charge(vec![branch]);
            let _ = std::fs::remove_dir_all(&scratch);
            let plan = report.output();
            let brief = Brief {
                pattern: pattern.word().to_string(),
                trigger: "the plan is written".to_string(),
                from: planner.label(),
                to: first.label(),
                requirements: Vec::new(),
                diff: Vec::new(),
                errors: Vec::new(),
                packets: Vec::new(),
                plan: Some(plan),
                remaining_ms: run.ledger.remaining_ms(),
            };
            run.handoff(
                "plan",
                &planner,
                &first,
                "the plan is written",
                Some(&brief),
            );
            let briefing = extended(&base, &brief.section());
            let setup = run.setup(None);
            let (branch, _, _) = run
                .branch(
                    &first,
                    Role::Worker,
                    "worker",
                    &work,
                    &briefing,
                    Some(setup),
                )
                .await?;
            run.ledger.charge(vec![branch]);
        }
        Pattern::Race => {
            let other = policy.to.clone().expect("validated");
            let dirs = [dir.join("race-1"), dir.join("race-2")];
            for racer_dir in &dirs {
                copy_tree(&work, racer_dir)?;
            }
            let make = |tier: &Tier, racer_dir: &Path| {
                let mut scripted = Scripted::new(
                    tier_script(&task, tier, Role::Solo),
                    racer_dir.to_path_buf(),
                );
                scripted.recorder = recorder.clone();
                scripted
            };
            let mut racers = [
                (make(&first, &dirs[0]), dirs[0].clone()),
                (make(&other, &dirs[1]), dirs[1].clone()),
            ];
            run.handoff("race", &first, &other, "both start", None);
            let (ended, winner, reaped, statuses) =
                race(&run, &mut racers, &base, run.ledger.remaining_ms()).await;
            let tiers = [first.clone(), other.clone()];
            let branches: Vec<Branch> = (0..2)
                .map(|i| {
                    let (rate, _) = usd_per_second(tiers[i].family());
                    Branch {
                        n: 0,
                        role: format!("racer-{}", i + 1),
                        tier: tiers[i].clone(),
                        started_ms: 0,
                        ms: ended[i],
                        usd: Some(rate * ended[i] as f64 / 1_000.0),
                        usd_basis: "modeled".to_string(),
                        status: statuses[i].clone(),
                        stopped_by: (winner.is_some() && winner != Some(i))
                            .then(|| "lost the race".to_string()),
                        reaped: winner.is_some() && winner != Some(i) && reaped,
                        won: Some(winner == Some(i)),
                    }
                })
                .collect();
            run.ledger.charge(branches);
            if let Some(i) = winner {
                copy_tree(&dirs[i], &work)?;
                run.handoff(
                    "race won",
                    &tiers[1 - i],
                    &tiers[i],
                    &format!("{} ended first with its checks passing", tiers[i].label()),
                    None,
                );
            }
            for racer_dir in &dirs {
                let _ = std::fs::remove_dir_all(racer_dir);
            }
        }
    }

    let checks = if options.checks {
        Some(crate::checks::check_workspace(&task, &work, &dir, &recorder).await)
    } else {
        None
    };
    let grading = recorder.enter(
        Start::new(
            "task.grade",
            Implementation::new(
                "task.grade",
                "mini-task grader",
                &json!({ "task": task.id }),
            ),
        )
        .named(task.id)
        .with_effects(),
    );
    let grade = crate::minitask::grade(&task, &work, &dir.join("grader")).await;
    recorder.end(
        &grading,
        Finish::new(match grade.verdict.as_str() {
            "passed" => Outcome::Completed,
            "failed" => Outcome::Failed,
            _ => Outcome::Skipped,
        })
        .output(json!({ "verdict": grade.verdict, "detail": grade.detail })),
    );
    recorder.end(
        &episode,
        Finish::new(if grade.verdict == "passed" {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .summary(json!({ "pattern": pattern.word(), "grade": grade.verdict, "ledger": run.ledger.record() })),
    );
    recorder.finish(atif::log::ENDED);
    crate::record::write_atomic(
        &dir.join("verification/grade.json"),
        serde_json::to_string_pretty(
            &json!({ "task": task.id, "grade": grade, "reward": grade.reward() }),
        )
        .map_err(|error| error.to_string())?
        .as_bytes(),
    )?;
    let ledger = run.ledger.clone();
    let handoffs = run.handoffs.clone();
    let rates: BTreeMap<String, Value> = ledger
        .branches
        .iter()
        .filter(|b| b.usd_basis == "modeled")
        .map(|b| {
            let (rate, source) = usd_per_second(b.tier.family());
            (
                b.tier.family().to_string(),
                json!({ "usd_per_second": rate, "source": source }),
            )
        })
        .collect();
    let manifest = json!({
        "schema": crate::minitask::RUN_SCHEMA,
        "kind": "mini-task",
        "id": id,
        "task": {
            "id": task.id,
            "family": task.family,
            "instruction_digest": atif::digest(&json!(task.instruction)),
        },
        "executor": {
            "kind": if first.agent == "scripted" { "scripted" } else { "cli" },
            "label": label,
            "tiers": ledger.branches.iter().map(|b| b.tier.label()).collect::<Vec<_>>(),
        },
        "pattern": pattern.word(),
        "policy": options.policy,
        "ledger": ledger.record(),
        "rates": rates,
        "handoffs": handoffs,
        "outcome": if grade.verdict == "passed" { "delegated" } else { "delegate_failed" },
        "grade": grade,
        "reward": grade.reward(),
        "checks": checks.as_ref().map(crate::checks::Report::summary),
        "started_at": atif::document::iso(at),
        "milliseconds": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "version": crate::episode::version(),
        "files": {
            "invocation_log": crate::episode::INVOCATION_LOG,
            "grade": "verification/grade.json",
            "workdir": "work",
            "artifacts": "artifacts",
        },
    });
    crate::record::write_atomic(
        &dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)
            .map_err(|error| error.to_string())?
            .as_bytes(),
    )?;
    Ok(Ran {
        dir,
        manifest,
        grade,
        ledger,
        handoffs,
    })
}

// ---------------------------------------------------------------------------
// Comparing patterns with single-pass policies.
// ---------------------------------------------------------------------------

/// A policy to compare: its name, its handoff policy, and the executor
/// that starts.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub name: String,
    pub policy: Policy,
    pub first: Tier,
}

/// The policies a comparison runs by default: the single-pass Luna and
/// Opus manifests, and the four handoff manifests.
///
/// # Errors
///
/// Returns a message when a manifest doesn't read.
pub fn manifest_candidates(dir: &Path) -> Result<Vec<Candidate>, String> {
    let mut out = Vec::new();
    for file in [
        "jevprobe3-luna.json",
        "jevprobe2-opus-lean-low-5m.json",
        "handoff-escalate.json",
        "handoff-planner-worker.json",
        "handoff-steer.json",
        "handoff-race.json",
    ] {
        let path = dir.join(file);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let manifest = crate::policy::Manifest::parse(&text)?;
        let mut candidate = manifest_candidate(&manifest);
        if candidate.name.is_empty() {
            candidate.name = file.trim_end_matches(".json").to_string();
        }
        out.push(candidate);
    }
    Ok(out)
}

/// A tier with its adapter replaced by the scripted one, keeping its model
/// family and so its behavior profile and rate.
#[must_use]
pub fn scripted(tier: &Tier) -> Tier {
    Tier::new("scripted", &tier.model)
}

/// One cell of a comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub policy: String,
    pub pattern: String,
    pub task: String,
    pub passed: Option<bool>,
    /// Episode time in milliseconds.
    pub ms: u64,
    pub usd: Option<f64>,
    pub handoffs: usize,
    pub branches: Vec<Branch>,
    pub error: Option<String>,
}

/// The objective's time price, in dollars per second: the outcome
/// matrix's default.
pub const USD_PER_SECOND: f64 = 0.0001;

/// The charge for a failure in the offline objective: the outcome
/// matrix's default.
pub const FAIL_USD: f64 = 1.0;

/// Compares `candidates` on every mini-task with scripted tiers.
pub async fn compare(candidates: &[Candidate], out: &Path, deadline: Duration) -> Value {
    let mut cells = Vec::new();
    for candidate in candidates {
        for task in crate::minitask::CATALOG {
            let mut policy = candidate.policy.clone();
            if let Some(to) = &policy.to {
                policy.to = Some(scripted(to));
            }
            let ran = run(Options {
                task: *task,
                policy,
                first: scripted(&candidate.first),
                out: out.to_path_buf(),
                deadline,
                jev: None,
                checks: false,
            })
            .await;
            cells.push(match ran {
                Ok(ran) => Cell {
                    policy: candidate.name.clone(),
                    pattern: candidate.policy.pattern.word().to_string(),
                    task: task.id.to_string(),
                    passed: match ran.grade.verdict.as_str() {
                        "passed" => Some(true),
                        "failed" => Some(false),
                        _ => None,
                    },
                    ms: ran.ledger.clock_ms,
                    usd: ran.ledger.usd().map(|usd| (usd * 1e6).round() / 1e6),
                    handoffs: ran.handoffs.len(),
                    branches: ran.ledger.branches.clone(),
                    error: None,
                },
                Err(error) => Cell {
                    policy: candidate.name.clone(),
                    pattern: candidate.policy.pattern.word().to_string(),
                    task: task.id.to_string(),
                    passed: None,
                    ms: 0,
                    usd: None,
                    handoffs: 0,
                    branches: Vec::new(),
                    error: Some(error),
                },
            });
        }
    }
    let policies: Vec<Value> = candidates
        .iter()
        .map(|candidate| {
            let mine: Vec<&Cell> = cells
                .iter()
                .filter(|c| c.policy == candidate.name)
                .collect();
            let graded: Vec<&&Cell> = mine.iter().filter(|c| c.passed.is_some()).collect();
            let passed = graded.iter().filter(|c| c.passed == Some(true)).count();
            let n = mine.len().max(1) as f64;
            let usd = mine.iter().map(|c| c.usd.unwrap_or(0.0)).sum::<f64>() / n;
            let seconds = mine.iter().map(|c| c.ms as f64 / 1_000.0).sum::<f64>() / n;
            let p = if graded.is_empty() {
                None
            } else {
                Some(passed as f64 / graded.len() as f64)
            };
            let runtime = usd + USD_PER_SECOND * seconds;
            json!({
                "policy": candidate.name,
                "pattern": candidate.policy.pattern.word(),
                "first": candidate.first.label(),
                "to": candidate.policy.to.as_ref().map(Tier::label),
                "passed": passed,
                "graded": graded.len(),
                "tasks": mine.len(),
                "mean_usd": (usd * 1e6).round() / 1e6,
                "mean_seconds": (seconds * 10.0).round() / 10.0,
                "handoffs": mine.iter().map(|c| c.handoffs).sum::<usize>(),
                "j_runtime": (runtime * 1e6).round() / 1e6,
                "j_offline": p.map(|p| ((runtime + (1.0 - p) * FAIL_USD) * 1e4).round() / 1e4),
                "errors": mine.iter().filter(|c| c.error.is_some()).count(),
            })
        })
        .collect();
    let rates: Vec<Value> = ["luna", "opus", "haiku", "sonnet"]
        .iter()
        .map(|family| {
            let (rate, source) = usd_per_second(family);
            json!({ "family": family, "usd_per_second": rate, "source": source })
        })
        .collect();
    json!({
        "schema": COMPARE_SCHEMA,
        "executor": "scripted tiers: each manifest's executors replaced by scripted ones of the same model family",
        "deadline_ms": u64::try_from(deadline.as_millis()).unwrap_or(u64::MAX),
        "objective": {
            "usd_per_second": USD_PER_SECOND,
            "fail_usd": FAIL_USD,
            "j_runtime": "mean cost + λ · mean episode seconds, handoffs and every branch included",
            "j_offline": "j_runtime + (1 − pass rate) · fail_usd",
        },
        "rates": rates,
        "notes": [
            "Scripted tiers play modeled behavior, not measurements: Opus solves each task in about 22 s; the cheap families solve git-recovery, call a wrong log parser done, and loop on a failing check on the terminal and cancellation tasks until stopped, and a steer or a plan leads them to the fix.",
            "Costs are modeled: each branch's episode seconds times its family's mean spend per second in the retained arms.",
            "Times are episode time on the host's virtual clock, the time boundary of the agent's sessions; a race charges its wall time once and both branches' cost.",
        ],
        "policies": policies,
        "cells": cells,
    })
}

/// The checkout's handoff directory: the checked-in pattern comparison.
#[must_use]
pub fn compare_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/handoff/minitask-patterns.json")
}

/// The handoff commands' usage.
pub const USAGE: &str = "usage: coder-one handoff run TASK --policy FILE [--executor scripted|real]
                                [--deadline SECONDS] [--checks] [--out DIR] [--json]
       coder-one handoff compare [--policies DIR] [--deadline SECONDS] [--out FILE] [--json]

run plays one control.handoff pattern on a mini-task: the policy manifest's
executor starts, and control.handoff names the pattern and the second
executor. --executor scripted (the default) replaces both with scripted tiers
of the same model family, on virtual time; real runs Claude Code or Codex
inside a coder-boundary filesystem boundary, and refuses where the boundary
can't be enforced. Every branch draws from one --deadline (600 seconds).
Runs record under ~/.openagents/coder-one/minitasks unless --out names another
directory, and the Gym shows them as mini-task runs.

compare runs the single-pass Luna and Opus manifests and the four handoff
manifests on every mini-task with scripted tiers, and reports pass, modeled
cost, and episode time per policy with the runtime objective. --out writes the
report, such as bench/terminal-bench/handoff/minitask-patterns.json, which
the Gym's outcome matrix reads.";

/// Runs a handoff command and returns the exit code.
///
/// # Errors
///
/// Returns a message for bad arguments or a run that can't start.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut positional = Vec::new();
    let mut policy_path = None;
    let mut policies = crate::policy::reference_dir().to_path_buf();
    let mut executor = "scripted".to_string();
    let mut deadline = 600u64;
    let mut checks = false;
    let mut out = None;
    let mut json_output = false;
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--policy" => policy_path = Some(PathBuf::from(value("--policy")?)),
            "--policies" => policies = PathBuf::from(value("--policies")?),
            "--executor" => executor = value("--executor")?,
            "--deadline" => {
                deadline = value("--deadline")?
                    .parse()
                    .map_err(|_| "--deadline takes whole seconds")?;
            }
            "--checks" => checks = true,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--json" => json_output = true,
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => positional.push(other.to_string()),
        }
    }
    match verb.as_str() {
        "run" => {
            let [id] = positional.as_slice() else {
                return Err(format!("handoff run needs one task ID\n{USAGE}"));
            };
            let task = crate::minitask::find(id)?;
            let path = policy_path.ok_or("handoff run needs --policy FILE")?;
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let manifest = crate::policy::Manifest::parse(&text)?;
            manifest.validate()?;
            let candidate = manifest_candidate(&manifest);
            let (first, mut policy) = (candidate.first, candidate.policy);
            let first = match executor.as_str() {
                "scripted" => {
                    if let Some(to) = &policy.to {
                        policy.to = Some(scripted(to));
                    }
                    scripted(&first)
                }
                "real" => first,
                other => return Err(format!("--executor takes scripted or real, not {other}")),
            };
            let out = out
                .or_else(crate::minitask::run::default_runs_dir)
                .ok_or("no --out and no HOME to record under")?;
            let ran = run(Options {
                task,
                policy,
                first,
                out,
                deadline: Duration::from_secs(deadline),
                jev: None,
                checks,
            })
            .await?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "dir": ran.dir.display().to_string(),
                        "manifest": ran.manifest,
                    }))
                    .map_err(|error| error.to_string())?
                );
            } else {
                println!(
                    "\n── {} · {} · {} ──",
                    task.id,
                    ran.manifest["pattern"].as_str().unwrap_or_default(),
                    ran.grade.verdict
                );
                for branch in &ran.ledger.branches {
                    println!(
                        "  {:<11} {:<30} {:>7.1}s  {:>9}  {}{}",
                        branch.role,
                        branch.tier.label(),
                        branch.ms as f64 / 1_000.0,
                        branch
                            .usd
                            .map_or("—".to_string(), |usd| format!("${usd:.4}")),
                        branch.status,
                        branch
                            .stopped_by
                            .as_deref()
                            .map_or(String::new(), |by| format!(" · stopped by {by}")),
                    );
                }
                for handoff in &ran.handoffs {
                    println!(
                        "  ⇢ {} {} → {} ({}), brief {} characters",
                        handoff["action"].as_str().unwrap_or_default(),
                        handoff["from"].as_str().unwrap_or_default(),
                        handoff["to"].as_str().unwrap_or_default(),
                        handoff["trigger"].as_str().unwrap_or_default(),
                        handoff["brief"]["chars"].as_u64().unwrap_or(0)
                    );
                }
                println!(
                    "  episode {:.1}s of {}s · {} · {}\nrecorded in {}",
                    ran.ledger.clock_ms as f64 / 1_000.0,
                    deadline,
                    ran.ledger
                        .usd()
                        .map_or("cost unknown".to_string(), |usd| format!("${usd:.4}")),
                    ran.grade.detail,
                    ran.dir.display()
                );
            }
            Ok(i32::from(ran.grade.verdict != "passed"))
        }
        "compare" => {
            let candidates = manifest_candidates(&policies)?;
            let scratch = std::env::temp_dir().join(format!(
                "coder-one-handoff-compare-{}-{}",
                std::process::id(),
                atif::now_ms()
            ));
            let report = compare(&candidates, &scratch, Duration::from_secs(deadline)).await;
            let _ = std::fs::remove_dir_all(&scratch);
            if let Some(path) = &out {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                let text =
                    serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
                crate::record::write_atomic(path, format!("{text}\n").as_bytes())?;
            }
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
                );
            } else {
                for line in compare_lines(&report) {
                    println!("{line}");
                }
                if let Some(path) = &out {
                    println!("written to {}", path.display());
                }
            }
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

/// A comparison's text form.
#[must_use]
pub fn compare_lines(report: &Value) -> Vec<String> {
    let mut lines = vec![
        "policy                               pattern          pass    mean $    mean s  handoffs  J runtime  J offline".to_string(),
    ];
    for policy in report["policies"].as_array().into_iter().flatten() {
        lines.push(format!(
            "{:<36} {:<15} {:>2}/{:<2}  {:>8.4}  {:>8.1}  {:>8}  {:>9.4}  {:>9}",
            policy["policy"].as_str().unwrap_or_default(),
            policy["pattern"].as_str().unwrap_or_default(),
            policy["passed"].as_u64().unwrap_or(0),
            policy["graded"].as_u64().unwrap_or(0),
            policy["mean_usd"].as_f64().unwrap_or(0.0),
            policy["mean_seconds"].as_f64().unwrap_or(0.0),
            policy["handoffs"].as_u64().unwrap_or(0),
            policy["j_runtime"].as_f64().unwrap_or(0.0),
            policy["j_offline"]
                .as_f64()
                .map_or("—".to_string(), |j| format!("{j:.4}")),
        ));
    }
    lines
}

/// A manifest's handoff policy and first executor.
#[must_use]
pub fn manifest_candidate(manifest: &crate::policy::Manifest) -> Candidate {
    let executor = &manifest.policy.executor;
    Candidate {
        name: manifest.name.clone().unwrap_or_default(),
        policy: manifest
            .policy
            .control
            .handoff
            .clone()
            .unwrap_or_else(Policy::single),
        first: Tier::new(
            match executor.agent {
                crate::policy::AgentName::ClaudeCode => "claude-code",
                crate::policy::AgentName::Codex => "codex",
            },
            &executor.model,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "coder-one-handoff-{label}-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn luna() -> Tier {
        Tier::new("scripted", "gpt-6-luna")
    }

    fn opus() -> Tier {
        Tier::new("scripted", "claude-opus-5-5")
    }

    fn policy(pattern: Pattern) -> Policy {
        Policy {
            pattern,
            to: Some(opus()),
            isolation: (pattern == Pattern::Race).then(|| SCRATCH_COPY.to_string()),
            ..Policy::single()
        }
    }

    async fn run_on(task: &str, pattern: Pattern, first: Tier) -> Ran {
        let dir = out(task);
        let ran = run(Options {
            task: crate::minitask::find(task).unwrap(),
            policy: policy(pattern),
            first,
            out: dir.clone(),
            deadline: Duration::from_secs(600),
            jev: None,
            checks: false,
        })
        .await
        .unwrap();
        let _ = std::fs::remove_dir_all(dir);
        ran
    }

    #[test]
    fn a_policy_runs_only_what_its_adapters_demonstrated() {
        let codex = Tier::new("codex", "gpt-6-luna");
        let claude = Tier::new("claude-code", "claude-opus-5-5");
        assert!(
            policy(Pattern::Steer)
                .validate(&codex)
                .unwrap_err()
                .contains("steering")
        );
        assert!(policy(Pattern::Steer).validate(&claude).is_ok());
        let mut race = policy(Pattern::Race);
        race.isolation = None;
        assert!(race.validate(&luna()).unwrap_err().contains("isolated"));
        let mut real = policy(Pattern::Race);
        real.to = Some(claude.clone());
        assert!(real.validate(&codex).unwrap_err().contains("lockstep"));
        let mut escalate = policy(Pattern::Escalate);
        escalate.to = None;
        assert!(escalate.validate(&codex).unwrap_err().contains("`to`"));
        assert!(policy(Pattern::Escalate).validate(&codex).is_ok());
    }

    #[tokio::test]
    async fn escalation_stops_a_looping_executor_and_briefs_the_next() {
        let ran = run_on("cancel-cleanup", Pattern::Escalate, luna()).await;
        assert_eq!(ran.ledger.branches.len(), 2, "{:?}", ran.ledger);
        let first = &ran.ledger.branches[0];
        assert_eq!(first.stopped_by.as_deref(), Some(COMPONENT));
        assert!(first.reaped);
        // The first branch was stopped at its third failing check.
        assert!((50_000..=50_200).contains(&first.ms), "{}", first.ms);
        let handoff = &ran.handoffs[0];
        assert_eq!(handoff["action"], "escalate");
        let brief = &handoff["brief"];
        assert!(brief["errors"].as_u64().unwrap() >= 1);
        assert!(
            brief["diff"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["path"] == "run.py")
        );
        assert!(
            brief["text"]
                .as_str()
                .unwrap()
                .contains("cleanup ran for 0 of 2")
        );
        // The second branch started where the first stopped, on the one
        // deadline.
        assert_eq!(ran.ledger.branches[1].started_ms, first.ms);
        assert_eq!(ran.ledger.clock_ms, first.ms + ran.ledger.branches[1].ms);
        assert_eq!(ran.manifest["pattern"], "escalate");
    }

    #[tokio::test]
    async fn a_steer_reaches_the_running_session_and_no_one_else_runs() {
        let ran = run_on("interactive-terminal", Pattern::Steer, luna()).await;
        assert_eq!(ran.ledger.branches.len(), 1);
        assert_eq!(ran.handoffs.len(), 1);
        assert_eq!(ran.handoffs[0]["action"], "steer");
        assert_eq!(ran.ledger.branches[0].status, "answered");
    }

    #[tokio::test]
    async fn a_planner_writes_nothing_and_its_plan_reaches_the_worker() {
        let ran = run_on("log-severity", Pattern::PlannerWorker, luna()).await;
        let roles: Vec<&str> = ran
            .ledger
            .branches
            .iter()
            .map(|b| b.role.as_str())
            .collect();
        assert_eq!(roles, vec!["planner", "worker"]);
        let text = ran.handoffs[0]["brief"]["text"].as_str().unwrap();
        assert!(text.contains("severity field"), "{text}");
    }

    #[tokio::test]
    async fn a_race_reaps_the_loser_before_the_winner_and_charges_both() {
        let ran = run_on("git-recovery", Pattern::Race, luna()).await;
        let racers = &ran.ledger.branches;
        assert_eq!(racers.len(), 2);
        let winner = racers
            .iter()
            .find(|b| b.won == Some(true))
            .expect("a winner");
        let loser = racers.iter().find(|b| b.won == Some(false)).unwrap();
        assert_eq!(winner.tier.family(), "opus");
        assert!(loser.reaped, "{loser:?}");
        assert_eq!(loser.ms, winner.ms);
        // Wall time once, money twice.
        assert_eq!(ran.ledger.clock_ms, winner.ms);
        assert!(loser.usd.unwrap() > 0.0 && winner.usd.unwrap() > 0.0);
        assert_eq!(ran.grade.verdict, "passed");
    }

    #[tokio::test]
    async fn a_real_executor_is_refused_where_no_boundary_can_be_enforced() {
        if coder_boundary::Boundary::writing(std::env::temp_dir())
            .build()
            .is_ok()
        {
            return;
        }
        let dir = out("real");
        let error = run(Options {
            task: crate::minitask::find("cancel-cleanup").unwrap(),
            policy: Policy {
                to: Some(Tier::new("claude-code", "claude-opus-5-5")),
                ..policy(Pattern::Escalate)
            },
            first: Tier::new("codex", "gpt-6-luna"),
            out: dir.clone(),
            deadline: Duration::from_secs(60),
            jev: None,
            checks: false,
        })
        .await
        .unwrap_err();
        assert!(error.contains("cannot bound the executor"), "{error}");
        // Nothing was set up.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn every_pattern_fixture_composes_as_expected() {
        let component = crate::component::find(COMPONENT).unwrap();
        let root = crate::component::default_fixtures();
        let dirs = crate::component::fixtures_for(&root, COMPONENT);
        assert!(dirs.len() >= 7, "{dirs:?}");
        let suite = crate::component::suite(
            component.as_ref(),
            &dirs,
            &crate::component::JevChoice::Off,
            &Recorder::default(),
            false,
        )
        .await
        .unwrap();
        for run in &suite.runs {
            assert!(run.error.is_none(), "{}: {:?}", run.fixture, run.error);
            if run.metrics["passed"].is_null() {
                // The grader needs python3 on this host.
                continue;
            }
            assert_eq!(
                run.metrics["matches_expected"],
                json!(true),
                "{}: {}",
                run.fixture,
                run.output
            );
            assert_eq!(run.metrics["reaped"], json!(true), "{}", run.fixture);
        }
    }

    #[test]
    fn the_checked_in_comparison_names_every_manifest() {
        let report: Value =
            serde_json::from_str(&std::fs::read_to_string(compare_path()).unwrap()).unwrap();
        assert_eq!(report["schema"], COMPARE_SCHEMA);
        let candidates = manifest_candidates(crate::policy::reference_dir()).unwrap();
        let named: Vec<&str> = report["policies"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p["policy"].as_str())
            .collect();
        for candidate in &candidates {
            assert!(
                named.contains(&candidate.name.as_str()),
                "{}",
                candidate.name
            );
        }
        let lines = compare_lines(&report);
        assert_eq!(lines.len(), candidates.len() + 1);
    }

    #[test]
    fn the_ledger_charges_concurrent_branches_once_in_time() {
        let branch = |ms: u64, usd: f64| Branch {
            n: 0,
            role: "racer".to_string(),
            tier: luna(),
            started_ms: 0,
            ms,
            usd: Some(usd),
            usd_basis: "modeled".to_string(),
            status: "answered".to_string(),
            stopped_by: None,
            reaped: false,
            won: None,
        };
        let mut ledger = Ledger::new(100_000);
        ledger.charge(vec![branch(10_000, 0.5)]);
        ledger.charge(vec![branch(30_000, 1.0), branch(20_000, 2.0)]);
        assert_eq!(ledger.clock_ms, 40_000);
        assert_eq!(ledger.remaining_ms(), 60_000);
        assert_eq!(ledger.usd(), Some(3.5));
        assert_eq!(ledger.branches[2].started_ms, 10_000);
    }
}
