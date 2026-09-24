//! `control.persist`: spend a long task's unused budget.
//!
//! On Terminal-Bench 4.0, lean Opus ended its session after one to three
//! minutes of an eight-hour budget, and several of its failures were near
//! misses. After the executor stops and the checks, support, repair, and
//! second executor have run, `control.persist` starts a fresh executor
//! session, not a resume, from a brief the host builds: the task, each
//! requirement's current state, what the earlier sessions changed, the
//! last checks' diagnostic packets, and the previous session's final
//! report. The brief tells the executor to write and run its own tests
//! from the task's words, fix what fails, and stop only when they pass.
//!
//! The host reruns the checks after each round and stops when a round
//! changes nothing, when the checks and support confirm every binding
//! requirement, when the round cap is reached, or when less than the
//! floor is left of the one episode deadline. Every round is a child
//! invocation of one `control.persist` invocation, and every session is a
//! delegate call in the episode's one usage ledger.
//!
//! v8 adds three options, each off unless the manifest sets it. With
//! `own_tests`, the brief asks the executor to keep its tests behind one
//! runner, and the host runs it after each round. With
//! `stop_when_no_progress`, a round whose own tests and checks show no
//! change in outcome ends the rounds. With `cheap`, later rounds run a
//! cheaper executor from the same brief and candidate, and a cheap round
//! that makes no progress hands the next round back to the strong one.
//! `spend` caps what the rounds spend together at a share of what the
//! task's budget has left when they start. Every round records its delta:
//! own tests fixed and broken, check failures before and after, cost, and
//! executor. [`progress`] holds the rules.
//!
//! On Terminal-Bench 4.0, v8's own tests passed in every round while the
//! verifier failed, so its progress rule never saw a failure. Under
//! `judge: "checks"`, a round is judged against what the checks flag. The
//! brief lists every failed scenario, contradicted requirement, and
//! failing own test, and asks for one own test per flagged check. A round
//! makes progress only when, scenario by scenario, it resolves something
//! flagged and regresses nothing ([`super::scorecard`]); it is put back
//! when it regresses more than it resolves; and once nothing is flagged,
//! no further round starts, because none could show progress.

pub mod progress;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::scorecard::{Comparison, Scorecard};
use super::{
    Exec, Factory, Horizon, Setup, Snapshot, Standing, VerifyPolicy, claimed, judge_support,
    monitor_for, row, self_reported,
};
use crate::checks::{self, Subject};
use crate::delegate::{self, Briefing, Delegation, Executor, Mode, Reason, Report, Status};
use crate::handoff::Tier;
use crate::record::{Finish, Implementation, Outcome, Start};
use crate::requirements::{Binding, RequirementMap};
use progress::{CheapRounds, Class, Delta, Ladder, Next, OwnTests, SpendCap, TestRun};

/// The component's ID.
pub const COMPONENT: &str = "control.persist";

fn rounds() -> u32 {
    3
}
fn min_remaining() -> u64 {
    1_800
}
fn share() -> f64 {
    0.5
}
fn yes() -> bool {
    true
}
fn max_copy_mb() -> u64 {
    256
}

/// `control.persist`: more fresh executor rounds while a long task has
/// time left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistPolicy {
    /// The most rounds after the last verification step.
    #[serde(default = "rounds")]
    pub max_rounds: u32,
    /// A round starts only when at least this many seconds of the episode
    /// deadline are left.
    #[serde(default = "min_remaining")]
    pub min_remaining_sec: u64,
    /// The share of the time left that each round asks for.
    #[serde(default = "share")]
    pub share: f64,
    /// Run only on a long task, as `control.horizon.long_after_sec` reads
    /// the deadline.
    #[serde(default = "yes")]
    pub long_only: bool,
    /// Stop when a round leaves the workspace as it found it.
    #[serde(default = "yes")]
    pub stop_when_unchanged: bool,
    /// Don't start a round when no scenario fails, no requirement is
    /// contradicted, and every binding requirement is observed by a
    /// scenario or supported by `verify.support`.
    #[serde(default = "yes")]
    pub stop_when_confirmed: bool,
    /// Copy the workspace aside before each round, and put it back when
    /// the round's checks come out worse: more failed scenarios and
    /// contradicted requirements than before.
    #[serde(default = "yes")]
    pub guard: bool,
    /// The largest workspace the guard copies aside, in MiB. A larger one
    /// runs the round unguarded, and the record says so.
    #[serde(default = "max_copy_mb")]
    pub max_copy_mb: u64,
    /// Executors to alternate with. Round 1 runs the executor that
    /// produced the current candidate; later rounds cycle through it and
    /// then each of these that differs from it. Empty: every round runs
    /// the same executor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternate: Vec<Tier>,
    /// v8: the executor keeps its own tests behind one runner, and the
    /// host runs it after each round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_tests: Option<OwnTests>,
    /// v8: stop when a round's own tests and checks show no change in
    /// outcome. Under `cheap`, the ladder decides instead.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stop_when_no_progress: bool,
    /// v8: later rounds on a cheaper executor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cheap: Option<CheapRounds>,
    /// v8: what the rounds may spend together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend: Option<SpendCap>,
    /// What a round is judged against: `outcome` (v5 to v9) or `checks`.
    #[serde(default, skip_serializing_if = "Judge::is_default")]
    pub judge: Judge,
}

/// What a persistence round is judged against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Judge {
    /// Counts: fewer check failures or more confirmed requirements, or
    /// more own tests fixed than broken, is progress.
    #[default]
    Outcome,
    /// What the checks and own tests flag, scenario by scenario: the
    /// brief lists it, progress resolves some of it without a regression,
    /// and the rounds stop once nothing is flagged.
    Checks,
}

impl Judge {
    fn is_default(&self) -> bool {
        *self == Judge::Outcome
    }
}

impl Default for PersistPolicy {
    fn default() -> Self {
        PersistPolicy {
            max_rounds: rounds(),
            min_remaining_sec: min_remaining(),
            share: share(),
            long_only: true,
            stop_when_unchanged: true,
            stop_when_confirmed: true,
            guard: true,
            max_copy_mb: max_copy_mb(),
            alternate: Vec::new(),
            own_tests: None,
            stop_when_no_progress: false,
            cheap: None,
            spend: None,
            judge: Judge::Outcome,
        }
    }
}

impl PersistPolicy {
    /// Refuses a policy this build can't run.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.max_rounds == 0 || self.max_rounds > 10 {
            problems.push("control.persist.max_rounds must be 1 to 10".to_string());
        }
        if !(self.share > 0.0 && self.share <= 1.0) {
            problems.push("control.persist.share must be above 0 and at most 1".to_string());
        }
        for (i, tier) in self.alternate.iter().enumerate() {
            problems.extend(tier.validate(&format!("control.persist.alternate[{i}]")));
        }
        if let Some(own) = &self.own_tests {
            problems.extend(own.validate());
        }
        if let Some(cheap) = &self.cheap {
            problems.extend(cheap.validate());
        }
        if let Some(spend) = &self.spend {
            problems.extend(spend.validate());
        }
        problems
    }

    /// Every executor the rounds may run besides the candidate's.
    #[must_use]
    pub fn tiers(&self) -> Vec<Tier> {
        let mut out = self.alternate.clone();
        if let Some(cheap) = &self.cheap {
            out.extend(cheap.tiers.iter().cloned());
            out.extend(cheap.strong.iter().cloned());
        }
        out
    }

    /// The executor round `n` (from 1) runs, given the one that produced
    /// the candidate.
    #[must_use]
    pub fn tier_for(&self, n: u32, current: &Tier) -> Tier {
        let mut cycle = vec![current.clone()];
        cycle.extend(
            self.alternate
                .iter()
                .filter(|t| t.agent != current.agent || t.model != current.model)
                .cloned(),
        );
        let at = (n.saturating_sub(1) as usize) % cycle.len();
        cycle[at].clone()
    }
}

/// The requirement IDs a check observed passing or a fresh support
/// judgment read as supported.
#[must_use]
pub fn confirmed_ids(
    report: &checks::Report,
    support: Option<&crate::support::Report>,
) -> Vec<String> {
    let candidate = report.candidate["digest"].as_str().unwrap_or_default();
    let mut ids: Vec<String> = report
        .coverage
        .iter()
        .filter(|c| c.state == "observed")
        .map(|c| c.id.clone())
        .collect();
    for state in support.map(|s| s.states.as_slice()).unwrap_or_default() {
        if state.fresh_for(candidate) && state.state == "supported" && !ids.contains(&state.id) {
            ids.push(state.id.clone());
        }
    }
    ids
}

/// Whether the checks and support confirm the candidate: nothing failed
/// or contradicted, and every binding requirement confirmed. A map with no
/// binding requirement confirms nothing.
#[must_use]
pub fn confirmed(
    map: Option<&RequirementMap>,
    report: &checks::Report,
    support: Option<&crate::support::Report>,
) -> bool {
    let standing = Standing::of(report, support);
    if standing.failed + standing.contradicted > 0 {
        return false;
    }
    let binding: Vec<&str> = map
        .map(|m| m.requirements.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|r| r.binding != Binding::Uncertain)
        .map(|r| r.id.as_str())
        .collect();
    let ids = confirmed_ids(report, support);
    !binding.is_empty() && binding.iter().all(|id| ids.iter().any(|c| c == id))
}

/// One requirement's current state: the fresh support judgment when there
/// is one, else what the checks observed.
fn state_of(
    id: &str,
    report: Option<&checks::Report>,
    support: Option<&crate::support::Report>,
) -> String {
    let candidate = report
        .and_then(|r| r.candidate["digest"].as_str())
        .unwrap_or_default();
    if let Some(judged) = support
        .and_then(|s| s.states.iter().find(|x| x.id == id))
        .filter(|x| x.fresh_for(candidate))
    {
        return format!("{} by a judge", judged.state);
    }
    report
        .and_then(|r| r.coverage.iter().find(|c| c.id == id))
        .map_or_else(|| "unobserved".to_string(), |c| c.state.clone())
}

/// A file's content digest.
fn digest(text: &str) -> String {
    atif::digest(&json!(text))
}

/// The workspace's text files as path → digest, the view the checks read.
#[must_use]
pub fn files_of(subject: &Subject, workdir: &std::path::Path) -> BTreeMap<String, String> {
    subject
        .input(workdir)
        .candidate
        .files
        .iter()
        .map(|(path, text)| (path.clone(), digest(text)))
        .collect()
}

/// Each output file outside the workspace as path → digest, or `None`
/// when it doesn't exist.
fn outside_of(paths: &[PathBuf]) -> Vec<(String, Option<String>)> {
    paths
        .iter()
        .map(|path| {
            let bytes = std::fs::metadata(path)
                .ok()
                .filter(|m| m.is_file() && m.len() <= 64 * 1024 * 1024)
                .and_then(|_| std::fs::read(path).ok());
            (
                path.display().to_string(),
                bytes.map(|b| atif::digest(&json!(String::from_utf8_lossy(&b)))),
            )
        })
        .collect()
}

/// The paths added, modified, or removed between two file maps.
#[must_use]
pub fn changed_paths(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for (path, digest) in after {
        match before.get(path) {
            None => out.push((path.clone(), "added")),
            Some(old) if old != digest => out.push((path.clone(), "modified")),
            Some(_) => {}
        }
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            out.push((path.clone(), "removed"));
        }
    }
    out
}

/// One changed file in a line or three: its change, size, and first
/// lines.
fn summarize(workdir: &std::path::Path, path: &str, change: &str) -> String {
    if change == "removed" {
        return format!("- `{path}`: removed");
    }
    let text = std::fs::read_to_string(workdir.join(path)).unwrap_or_default();
    let head: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .collect();
    format!(
        "- `{path}`: {change}, {} lines, {} bytes{}",
        text.lines().count(),
        text.len(),
        if head.is_empty() {
            String::new()
        } else {
            format!("; starts: {}", crate::judge::clip(&head.join(" / "), 200))
        }
    )
}

/// What the brief is built from.
pub struct BriefInputs<'a> {
    pub instruction: &'a str,
    pub map: Option<&'a RequirementMap>,
    pub report: Option<&'a checks::Report>,
    pub support: Option<&'a crate::support::Report>,
    /// The workspace's changes since the episode started, summarized.
    pub changes: &'a [String],
    /// Output files outside the workspace and whether each exists.
    pub outside: &'a [(String, Option<String>)],
    /// The previous session's final report.
    pub previous: &'a str,
    pub round: u32,
    pub max_rounds: u32,
    /// v8: the runner the executor keeps its tests behind.
    pub own_tests: Option<&'a OwnTests>,
    /// v8: the host's last run of that runner.
    pub tests: Option<&'a TestRun>,
    /// Under `judge: "checks"`: what the checks and own tests flag, and
    /// the rule the brief states.
    pub flagged: Option<&'a [String]>,
}

/// The runner's contract, under `own_tests`.
#[must_use]
pub fn runner_directions(runner: &str) -> String {
    format!(
        "Keep your tests behind one runner at `{runner}`: `bash {runner}`, started in the \
working directory, runs every test and prints one line per test, `PASS <name>` or \
`FAIL <name>`, with a stable name for each. Keep the tests an earlier session left there, \
and add yours. The host runs the runner after your session and counts a round that fixes \
none of them, and changes no check, as a round without progress."
    )
}

/// The directions every persist round reads.
pub const DIRECTIONS: &str = "Another session worked on this task and stopped with time \
left. You continue in the same workspace with a fresh view. Treat the current result as \
unproven until your own tests show it is right.

1. Write your own rigorous tests from the task's words above: edge cases, variants of the \
inputs, scale, and the exact output formats, paths, and names the task states. Keep them \
outside the deliverables, for example under /tmp/persist-tests, so they don't change what \
is graded. Don't look for, read, or run the task's protected verifier or anything under \
/tests.
2. Run your tests against the current result.
3. Fix what fails in the deliverables, then run every test again.
4. When the output is visual or numeric, render or measure it and compare it with what the \
task asks: render a model's projections and compare them with the drawing, recompute a \
number another way, or run the program on inputs other than the example.
5. Stop only when your own tests pass, or when you are sure the result is right. End with \
what you tested, what failed, and what you changed.";

/// The rule a round is judged by, under `judge: "checks"`.
pub const JUDGED_BY_CHECKS: &str = "The host judges this round against what its checks \
and your tests flag. It counts the round as progress only when a flagged check now passes, \
a contradicted requirement is no longer contradicted, or a failing test of yours now \
passes, and nothing that passed before fails. It puts the workspace back when the round \
breaks more than it fixes. Your tests passing while a check still fails is not progress.";

/// What the brief asks for about each flagged failure, under
/// `judge: "checks"`.
pub const TEST_EACH_FLAG: &str = "For each flagged failure above, first add a test to \
your runner, named `check <scenario>`, that fails while the failure stands. Find the \
cause in the deliverables, fix it, and run every test again. When a flag is wrong about \
the task, say which and why in your final report instead of working around it.";

/// What the brief says when nothing is flagged, under `judge: "checks"`.
pub const NOTHING_FLAGGED: &str = "The host's checks flag no failure, and none of the \
earlier sessions' tests fail. The task's own verifier may still fail, so look for what \
the checks miss: test the exact output formats, paths, and names, edge cases, and \
inputs other than the example. When your tests find no failure either, stop: the host \
starts no further round when nothing is flagged.";

/// Where round `n`'s brief is kept, under the episode's directory.
#[must_use]
pub fn brief_path(n: u32) -> String {
    format!("artifacts/persist-{n}.brief.md")
}

/// Where the check after round `n` writes its report.
#[must_use]
pub fn checks_file(n: u32) -> String {
    format!("verification/checks-persist-{n}.json")
}

/// Where `verify.support` writes its judgment after round `n`.
#[must_use]
pub fn support_file(n: u32) -> String {
    format!("verification/support-persist-{n}.json")
}

/// The code-built brief for one round.
#[must_use]
pub fn brief(inputs: &BriefInputs<'_>) -> Briefing {
    let mut included = vec!["task".to_string()];
    let mut text = format!(
        "# Continue this task (round {} of at most {})\n\n## The task\n\n{}\n",
        inputs.round,
        inputs.max_rounds,
        inputs.instruction.trim()
    );
    if let Some(map) = inputs.map.filter(|m| !m.requirements.is_empty()) {
        included.push("requirement states".to_string());
        text.push_str("\n## Requirements and what the host's checks know about them\n\n");
        for requirement in &map.requirements {
            text.push_str(&format!(
                "- {} ({}{}): {}: {}\n",
                requirement.id,
                requirement.kind.word(),
                if requirement.binding == Binding::Uncertain {
                    ", may not bind"
                } else {
                    ""
                },
                state_of(&requirement.id, inputs.report, inputs.support),
                crate::judge::clip(
                    &requirement
                        .text
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    240
                )
            ));
        }
    }
    text.push_str("\n## What the earlier sessions changed\n\n");
    if inputs.changes.is_empty() {
        text.push_str("No text file in the working directory changed.\n");
    } else {
        included.push("changed files".to_string());
        let mut used = 0;
        for (i, line) in inputs.changes.iter().enumerate() {
            used += line.chars().count();
            if used > 4_000 {
                text.push_str(&format!("- and {} more files\n", inputs.changes.len() - i));
                break;
            }
            text.push_str(line);
            text.push('\n');
        }
    }
    if !inputs.outside.is_empty() {
        included.push("outputs outside the workspace".to_string());
        text.push_str("\nOutput files the task names outside the working directory:\n\n");
        for (path, digest) in inputs.outside {
            text.push_str(&format!(
                "- `{path}`: {}\n",
                if digest.is_some() {
                    "exists"
                } else {
                    "missing"
                }
            ));
        }
    }
    if let Some(report) = inputs.report {
        let failed: Vec<&checks::Verdict> = report
            .verdicts
            .iter()
            .filter(|v| v.verdict == "failed")
            .collect();
        text.push_str(&format!(
            "\n## The host's last checks\n\n{} scenarios ran; {} failed.\n",
            report.verdicts.len(),
            failed.len()
        ));
        if !report.packets.is_empty() {
            included.push("diagnostic packets".to_string());
        }
        let shown = if inputs.flagged.is_some() { 8 } else { 3 };
        for packet in report.packets.iter().take(shown) {
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
        text.push_str("\nThese checks are shallow: passing them doesn't show the task is done.\n");
    }
    if !inputs.previous.trim().is_empty() {
        included.push("previous report".to_string());
        text.push_str(&format!(
            "\n## The previous session's final report\n\n{}\n",
            crate::judge::clip(inputs.previous.trim(), 3_000)
        ));
    }
    if let Some(tests) = inputs.tests {
        included.push("own tests".to_string());
        text.push_str(&format!(
            "\n## The earlier sessions' own tests\n\nThe host's last run of the runner: {} passed, {} failed.\n",
            tests.passed(),
            tests.failed()
        ));
        for name in tests.failing().iter().take(20) {
            text.push_str(&format!("- failing: {}\n", crate::judge::clip(name, 200)));
        }
    }
    if let Some(flagged) = inputs.flagged {
        included.push("flagged failures".to_string());
        text.push_str("\n## What the host's checks and your tests flag\n\n");
        if flagged.is_empty() {
            text.push_str(NOTHING_FLAGGED);
            text.push('\n');
        } else {
            for flag in flagged.iter().take(30) {
                text.push_str(&format!("- {}\n", crate::judge::clip(flag, 200)));
            }
            if flagged.len() > 30 {
                text.push_str(&format!("- and {} more\n", flagged.len() - 30));
            }
            text.push_str(&format!("\n{TEST_EACH_FLAG}\n"));
        }
        text.push_str(&format!("\n{JUDGED_BY_CHECKS}\n"));
    }
    text.push_str(&format!("\n## What to do\n\n{DIRECTIONS}\n"));
    if let Some(own) = inputs.own_tests {
        text.push_str(&format!("\n{}\n", runner_directions(&own.runner)));
    }
    included.push("directions".to_string());
    Briefing {
        cap: text.chars().count(),
        text,
        included,
        omitted: Vec::new(),
    }
}

/// What a persist run reads besides its policy.
pub(super) struct Context<'a> {
    pub setup: &'a Setup<'a>,
    pub subject: &'a Subject,
    pub verify: &'a VerifyPolicy,
    pub horizon: &'a Horizon,
    pub long: bool,
    pub support_params: crate::support::Params,
    pub fallback: u64,
    pub isolation: &'a str,
    /// The workspace's files before the first executor, path → digest.
    pub initial: &'a BTreeMap<String, String>,
    /// Output paths the requirements name outside the workspace.
    pub outside: &'a [PathBuf],
}

/// Where the rounds start from.
pub(super) struct Current {
    pub tier: Tier,
    pub checked: Option<(checks::Input, checks::Report)>,
    pub support: Option<crate::support::Report>,
    /// The final report of the session that produced the candidate.
    pub previous: String,
}

/// What the rounds left.
pub(super) struct Persisted {
    pub record: Value,
    pub branches: Vec<Value>,
    pub checks_log: Vec<Value>,
    pub current: Current,
    /// The last kept round's report, for the episode's ending.
    pub report: Option<Report>,
}

/// Why no round starts now, or `None` when one may.
fn gate(
    context: &Context<'_>,
    policy: &PersistPolicy,
    n: u32,
    current: &Current,
) -> Option<String> {
    if n > policy.max_rounds {
        return Some(format!("reached the cap of {} rounds", policy.max_rounds));
    }
    let left = context.setup.deadline.allowance().map(|d| d.as_secs());
    if left.is_some_and(|left| left < policy.min_remaining_sec) {
        return Some(format!(
            "less than {} s left in the episode",
            policy.min_remaining_sec
        ));
    }
    if policy.stop_when_confirmed
        && let Some((_, report)) = &current.checked
        && confirmed(
            context.subject.requirements.as_ref(),
            report,
            current.support.as_ref(),
        )
    {
        return Some("the checks and support confirm every binding requirement".to_string());
    }
    None
}

/// What the episode has spent so far, as far as it is known.
fn spent(recorder: &crate::record::Recorder) -> f64 {
    crate::episode::usage(&recorder.steps(), true)["cost"]["lower_bound_usd"]
        .as_f64()
        .unwrap_or(0.0)
}

/// The key a round's cost is remembered under, for the next estimate.
fn cost_key(class: Class) -> &'static str {
    match class {
        Class::Cheap => "cheap",
        Class::Strong | Class::Escalated => "strong",
    }
}

/// The current candidate's scorecard, with the last run of the own tests.
fn score(current: &Current, tests: Option<&TestRun>) -> Scorecard {
    current.checked.as_ref().map_or_else(
        || Scorecard::default().with_tests(tests),
        |(_, report)| Scorecard::of(report, current.support.as_ref(), tests),
    )
}

/// Why the rounds stop under `judge: "checks"` once nothing is flagged.
pub const NOTHING_LEFT: &str = "nothing the checks or the own tests flag is left to fix";

/// Runs the rounds.
///
/// # Errors
///
/// Returns a message when an executor can't be made or a record can't be
/// written. A failed session is an outcome, not an error.
#[allow(clippy::too_many_lines)]
pub(super) async fn run<F: Factory>(
    context: &Context<'_>,
    policy: &PersistPolicy,
    mut current: Current,
    factory: &mut F,
    runs: &mut u32,
) -> Result<Persisted, String> {
    let setup = context.setup;
    let recorder = setup.recorder;
    let mut branches = Vec::new();
    let mut checks_log = Vec::new();
    let mut rounds: Vec<Value> = Vec::new();
    let mut last_report: Option<Report> = None;
    if policy.long_only && !context.long {
        let record = json!({ "policy": policy, "skipped": "not a long task", "rounds": [] });
        super::record_decision(recorder, COMPONENT, "persist skipped", &record);
        return Ok(Persisted {
            record,
            branches,
            checks_log,
            current,
            report: None,
        });
    }
    let parent = recorder.enter(
        Start::new(
            COMPONENT,
            Implementation::new(
                COMPONENT,
                "fresh rounds from a continue brief",
                &json!(policy),
            ),
        )
        .named("persist")
        .with_effects(),
    );
    let cheap = policy.cheap.as_ref();
    let strong = cheap
        .and_then(|c| c.strong.clone())
        .unwrap_or_else(|| current.tier.clone());
    let mut ladder = Ladder::default();
    let spent_before = spent(recorder);
    let cap = policy.spend.as_ref().and_then(|s| {
        s.cap(
            setup.manifest.protected.ceilings.spend_soft_usd,
            spent_before,
        )
    });
    let mut spent_rounds = 0.0;
    let mut last_cost: BTreeMap<&'static str, f64> = BTreeMap::new();
    let (mut fixed, mut broken) = (0, 0);
    // A runner an earlier session left is the first baseline.
    let mut tests: Option<TestRun> = match &policy.own_tests {
        Some(own) => progress::run_own(own, setup.workdir).await,
        None => None,
    };
    let baseline = tests.as_ref().map(TestRun::record);
    let by_checks = policy.judge == Judge::Checks;
    let mut n = 1;
    let stopped = loop {
        if super::limited(recorder).is_some() {
            break "a delegate session hit a usage limit".to_string();
        }
        if let Some(why) = gate(context, policy, n, &current) {
            break why;
        }
        let class = ladder.class_for(n, cheap);
        if let Some(why) =
            progress::over_cap(cap, spent_rounds, last_cost.get(cost_key(class)).copied())
        {
            break why;
        }
        let mut tier = match (class, cheap) {
            (Class::Cheap, Some(c)) => ladder.cheap_tier(c),
            (Class::Escalated, _) => strong.clone(),
            _ => policy.tier_for(n, &current.tier),
        };
        // A long task runs at the long effort, except on a cheap tier that
        // names its own.
        if context.long
            && let Some(effort) = &context.horizon.long_effort
            && (class != Class::Cheap || tier.effort.is_none())
        {
            tier.effort = Some(effort.clone());
        }
        let before_files = files_of(context.subject, setup.workdir);
        let before_outside = outside_of(context.outside);
        let changes: Vec<String> = changed_paths(context.initial, &before_files)
            .iter()
            .map(|(path, change)| summarize(setup.workdir, path, change))
            .collect();
        let before_card = score(&current, tests.as_ref());
        let flagged_before = before_card.flagged();
        let briefing = brief(&BriefInputs {
            instruction: setup.instruction,
            map: context.subject.requirements.as_ref(),
            report: current.checked.as_ref().map(|(_, r)| r),
            support: current.support.as_ref(),
            changes: &changes,
            outside: &before_outside,
            previous: &current.previous,
            round: n,
            max_rounds: policy.max_rounds,
            own_tests: policy.own_tests.as_ref(),
            tests: tests.as_ref(),
            flagged: by_checks.then_some(flagged_before.as_slice()),
        });
        let path = setup.dir.join(brief_path(n));
        crate::record::write_atomic(&path, briefing.text.as_bytes())?;
        let guard = policy.guard.then(|| {
            Snapshot::take(
                setup.workdir,
                context.outside,
                policy.max_copy_mb,
                "persist",
            )
        });
        let sec = context.horizon.dispatch_sec(
            setup.deadline.allowance(),
            context.fallback,
            policy.share,
        );
        println!("  persist ▸ round {n} · {} · asks {sec}s", tier.label());
        let round = recorder.enter(
            Start::new(
                COMPONENT,
                Implementation::new(
                    COMPONENT,
                    "round",
                    &json!({ "round": n, "tier": tier, "class": class }),
                ),
            )
            .named(&format!("round {n} · {}", tier.label()))
            .reading_digest(briefing.sha256())
            .with_effects(),
        );
        let mut exec: Exec = factory.make(&tier, Duration::from_secs(sec), *runs)?;
        exec.watch(monitor_for(setup, None, sec));
        let session = recorder.enter(
            Start::new(
                "exec.session",
                Implementation::new(
                    "exec.session",
                    &format!("{} {}", exec.agent(), exec.model()),
                    &json!({ "agent": exec.agent(), "model": exec.model(), "deadline_sec": sec, "role": format!("persist-{n}"), "session": "fresh", "resumes": null }),
                ),
            )
            .named(&format!(
                "persist {n} · fresh session · {} ({})",
                exec.agent(),
                exec.model()
            ))
            .reading_digest(briefing.sha256())
            .with_effects(),
        );
        let reason = Reason::Handoff(format!("control.persist round {n}"));
        let report = delegate::delegate(
            &mut exec,
            &briefing,
            &Delegation {
                mode: Mode::Always,
                reason: &reason,
                isolation: context.isolation,
            },
            recorder,
            *runs,
        )
        .await;
        let cost = crate::repair::session_cost(exec.agent(), &report);
        recorder.end(
            &session,
            Finish::new(if report.status == Status::Answered {
                Outcome::Completed
            } else {
                Outcome::Failed
            })
            .summary(json!({ "status": report.status.word(), "milliseconds": report.milliseconds, "turns": report.summary.num_turns }))
            .cost(cost.clone()),
        );
        *runs = exec.runs();
        let last = exec.last();
        drop(exec);
        ladder.ran(class);
        if let Some(usd) = cost.usd {
            spent_rounds += usd;
            last_cost.insert(cost_key(class), usd);
        }
        branches.push(row(&format!("persist-{n}"), &tier, &report, &last, sec));
        if let Some(limit) = report.limit(&tier.agent) {
            if let Some(g) = &guard {
                g.discard();
            }
            let entry = json!({
                "round": n,
                "tier": tier,
                "class": class,
                "executor": tier.label(),
                "requested_sec": sec,
                "status": report.status.word(),
                "milliseconds": report.milliseconds,
                "cost_usd": cost.usd,
                "session_id": report.summary.session_id,
                "usage_limit": limit.record(),
            });
            recorder.end(&round, Finish::new(Outcome::Failed).output(entry.clone()));
            rounds.push(entry);
            break format!("round {n}'s session hit a usage limit");
        }

        let after_files = files_of(context.subject, setup.workdir);
        let after_outside = outside_of(context.outside);
        let files_changed: Vec<Value> = changed_paths(&before_files, &after_files)
            .into_iter()
            .map(|(path, change)| json!({ "path": path, "change": change }))
            .chain(
                before_outside
                    .iter()
                    .zip(&after_outside)
                    .filter(|(a, b)| a.1 != b.1)
                    .map(|(_, b)| json!({ "path": b.0, "change": if b.1.is_some() { "written" } else { "removed" } })),
            )
            .collect();
        let changed = !files_changed.is_empty();
        let now = match &policy.own_tests {
            Some(own) => progress::run_own(own, setup.workdir).await,
            None => None,
        };
        let mut entry = json!({
            "round": n,
            "tier": tier,
            "class": class,
            "executor": tier.label(),
            "requested_sec": sec,
            "status": report.status.word(),
            "milliseconds": report.milliseconds,
            "turns": report.summary.num_turns,
            "cost_usd": cost.usd,
            "cost_provenance": cost.provenance,
            "session_id": report.summary.session_id,
            "brief": brief_path(n),
            "brief_chars": briefing.text.chars().count(),
            "changed": changed,
            "files_changed": files_changed,
            "tests": now.as_ref().map(TestRun::record),
        });
        if !changed {
            if let Some(g) = &guard {
                g.discard();
            }
            let delta = Delta::of(tests.as_ref(), now.as_ref(), None, None, false);
            fixed += delta.tests_fixed;
            broken += delta.tests_broken;
            entry["kept"] = json!(true);
            entry["delta"] = json!(delta);
            if report.status == Status::Answered {
                current.previous = report.output();
                last_report = Some(report);
            }
            if now.is_some() {
                tests = now;
            }
            let mut next = cheap.is_some().then(|| {
                progress::after_round(
                    n,
                    class,
                    delta.progress,
                    policy.stop_when_no_progress,
                    cheap,
                    &mut ladder,
                )
            });
            if by_checks {
                let flagged = score(&current, tests.as_ref()).flagged();
                entry["flagged_after"] = json!(flagged);
                if flagged.is_empty() {
                    ladder.cancel(next.as_ref());
                    next = Some(Next::Stop(NOTHING_LEFT.to_string()));
                }
            }
            if next == Some(Next::Escalate) {
                entry["next"] = json!("escalate");
            }
            recorder.end(
                &round,
                Finish::new(Outcome::Completed).output(entry.clone()),
            );
            rounds.push(entry);
            match next {
                Some(Next::Stop(why)) => break format!("round {n} changed nothing; {why}"),
                Some(_) => {}
                None if policy.stop_when_unchanged || policy.stop_when_no_progress => {
                    break format!("round {n} changed nothing");
                }
                None => {}
            }
            n += 1;
            continue;
        }

        // Check what the round left.
        let mut subject = context.subject.clone();
        if let Some(live) = &mut subject.live {
            live.claimed = claimed(recorder);
            live.report = Some(report.output());
        }
        let file = checks_file(n);
        let rechecked =
            checks::check_subject_as(&subject, setup.workdir, setup.dir, recorder, &file).await;
        let resupport = judge_support(
            setup,
            context.verify,
            Some(&rechecked),
            context.support_params,
            &support_file(n),
        )
        .await?;
        let before = current
            .checked
            .as_ref()
            .map(|(_, r)| Standing::of(r, current.support.as_ref()));
        let after = Standing::of(&rechecked.1, resupport.as_ref());
        let bad = |s: &Standing| s.failed + s.contradicted;
        let mut delta = Delta::of(
            tests.as_ref(),
            now.as_ref(),
            before.as_ref(),
            Some(&after),
            true,
        );
        let after_card = Scorecard::of(&rechecked.1, resupport.as_ref(), now.as_ref());
        // Own tests compare only when both sides ran them: a round that
        // wrote the first tests sets their baseline.
        let compared = Comparison::between(
            &if tests.is_some() {
                before_card.clone()
            } else {
                Scorecard {
                    tests: None,
                    ..before_card.clone()
                }
            },
            &if tests.is_some() {
                after_card.clone()
            } else {
                Scorecard {
                    tests: None,
                    ..after_card.clone()
                }
            },
        );
        let checks_worse = before.is_some_and(|b| bad(&after) > bad(&b));
        let tests_worse = policy.own_tests.is_some() && delta.tests_worse();
        let worse = if by_checks {
            compared.worse()
        } else {
            checks_worse || tests_worse
        };
        if by_checks {
            delta.progress = if compared.better() {
                Some(true)
            } else if tests.is_none()
                && flagged_before.is_empty()
                && now.as_ref().is_some_and(|t| !t.results.is_empty())
            {
                None
            } else {
                Some(false)
            };
            delta.resolved.clone_from(&compared.resolved);
            delta.regressed.clone_from(&compared.regressed);
        }
        entry["checks_file"] = json!(file);
        entry["before"] = json!(before);
        entry["after"] = json!(after);
        checks_log.push(json!({ "after": format!("persist-{n}"), "file": file, "summary": rechecked.1.summary(), "self_report": self_reported(&rechecked.1) }));
        let restored = match (&guard, worse) {
            (Some(g), true) if g.refused.is_none() => Some(g.restore(setup.workdir)),
            _ => None,
        };
        let worse_why = if by_checks {
            format!(
                "the round regressed {} ({}) and resolved {}",
                compared.regressed.len(),
                compared.regressed.join("; "),
                compared.resolved.len()
            )
        } else if checks_worse {
            format!(
                "the round's checks came out worse ({} failures against {})",
                bad(&after),
                before.map_or(0, |b| bad(&b))
            )
        } else {
            format!(
                "the round broke {} of its own tests and fixed {}",
                delta.tests_broken, delta.tests_fixed
            )
        };
        match &restored {
            Some(Ok(())) => {
                entry["kept"] = json!(false);
                entry["why"] = json!(format!("{worse_why}, so the host put the workspace back"));
            }
            Some(Err(error)) => {
                entry["kept"] = json!(true);
                entry["restore_error"] = json!(error);
            }
            None => {
                entry["kept"] = json!(true);
                if worse {
                    entry["why"] = json!(format!("{worse_why}, and no copy was kept to put back"));
                }
            }
        }
        if let Some(g) = &guard {
            if let Some(why) = &g.refused {
                entry["unguarded"] = json!(why);
            }
            g.discard();
        }
        let kept = entry["kept"] == true;
        if !kept {
            // A round that was put back left nothing behind.
            delta.progress = Some(false);
        } else {
            fixed += delta.tests_fixed;
            broken += delta.tests_broken;
        }
        entry["delta"] = json!(delta);
        let mut next = (cheap.is_some() || policy.stop_when_no_progress).then(|| {
            progress::after_round(
                n,
                class,
                delta.progress,
                policy.stop_when_no_progress,
                cheap,
                &mut ladder,
            )
        });
        if by_checks {
            // What the candidate the rounds now stand on still flags.
            let flagged = if kept {
                after_card.flagged()
            } else {
                score(&current, tests.as_ref()).flagged()
            };
            entry["flagged_after"] = json!(flagged);
            if flagged.is_empty() {
                ladder.cancel(next.as_ref());
                next = Some(Next::Stop(NOTHING_LEFT.to_string()));
            }
        }
        if next == Some(Next::Escalate) {
            entry["next"] = json!("escalate");
        }
        recorder.end(
            &round,
            Finish::new(if kept && !rechecked.1.detected() {
                Outcome::Completed
            } else {
                Outcome::Failed
            })
            .output(entry.clone()),
        );
        println!(
            "  persist ▸ round {n} {} · {} files changed · {} failed after · own tests +{} −{}",
            if kept { "kept" } else { "put back" },
            entry["files_changed"].as_array().map_or(0, Vec::len),
            after.failed,
            delta.tests_fixed,
            delta.tests_broken
        );
        rounds.push(entry);
        if kept {
            current.tier = tier;
            current.previous = report.output();
            current.checked = Some(rechecked);
            current.support = resupport;
            last_report = Some(report);
            if now.is_some() {
                tests = now;
            }
        }
        if let Some(Next::Stop(why)) = next {
            break why;
        }
        n += 1;
    };
    println!("  persist ▸ stopped: {stopped}");
    let record = json!({
        "policy": policy,
        "rounds": rounds,
        "stopped": stopped,
        "final_tier": current.tier,
        "baseline_tests": baseline,
        "totals": {
            "tests_fixed": fixed,
            "tests_broken": broken,
            "cost_usd": spent_rounds,
            "escalations": ladder.escalations,
        },
        "spend": {
            "spent_before_usd": spent_before,
            "cap_usd": cap,
        },
    });
    recorder.end(
        &parent,
        Finish::new(Outcome::Completed)
            .output(json!({ "rounds": rounds.len(), "stopped": stopped })),
    );
    Ok(Persisted {
        record,
        branches,
        checks_log,
        current,
        report: last_report,
    })
}
