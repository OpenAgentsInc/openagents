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

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    Exec, Factory, Horizon, Setup, Snapshot, Standing, VerifyPolicy, claimed, judge_support,
    monitor_for, row, self_reported,
};
use crate::checks::{self, Subject};
use crate::delegate::{self, Briefing, Delegation, Executor, Mode, Reason, Report, Status};
use crate::handoff::Tier;
use crate::record::{Finish, Implementation, Outcome, Start};
use crate::requirements::{Binding, RequirementMap};

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
        problems
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
        text.push_str("\nThese checks are shallow: passing them doesn't show the task is done.\n");
    }
    if !inputs.previous.trim().is_empty() {
        included.push("previous report".to_string());
        text.push_str(&format!(
            "\n## The previous session's final report\n\n{}\n",
            crate::judge::clip(inputs.previous.trim(), 3_000)
        ));
    }
    text.push_str(&format!("\n## What to do\n\n{DIRECTIONS}\n"));
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
    let mut n = 1;
    let stopped = loop {
        if let Some(why) = gate(context, policy, n, &current) {
            break why;
        }
        let mut tier = policy.tier_for(n, &current.tier);
        if context.long
            && let Some(effort) = &context.horizon.long_effort
        {
            tier.effort = Some(effort.clone());
        }
        let before_files = files_of(context.subject, setup.workdir);
        let before_outside = outside_of(context.outside);
        let changes: Vec<String> = changed_paths(context.initial, &before_files)
            .iter()
            .map(|(path, change)| summarize(setup.workdir, path, change))
            .collect();
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
                Implementation::new(COMPONENT, "round", &json!({ "round": n, "tier": tier })),
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
        branches.push(row(&format!("persist-{n}"), &tier, &report, &last, sec));

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
        let mut entry = json!({
            "round": n,
            "tier": tier,
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
        });
        if !changed {
            if let Some(g) = &guard {
                g.discard();
            }
            entry["kept"] = json!(true);
            recorder.end(
                &round,
                Finish::new(Outcome::Completed).output(entry.clone()),
            );
            rounds.push(entry);
            if report.status == Status::Answered {
                current.previous = report.output();
                last_report = Some(report);
            }
            if policy.stop_when_unchanged {
                break format!("round {n} changed nothing");
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
        let worse = before.is_some_and(|b| bad(&after) > bad(&b));
        entry["checks_file"] = json!(file);
        entry["before"] = json!(before);
        entry["after"] = json!(after);
        checks_log.push(json!({ "after": format!("persist-{n}"), "file": file, "summary": rechecked.1.summary(), "self_report": self_reported(&rechecked.1) }));
        let restored = match (&guard, worse) {
            (Some(g), true) if g.refused.is_none() => Some(g.restore(setup.workdir)),
            _ => None,
        };
        match &restored {
            Some(Ok(())) => {
                entry["kept"] = json!(false);
                entry["why"] = json!(format!(
                    "the round's checks came out worse ({} failures against {}), so the host put the workspace back",
                    bad(&after),
                    before.map_or(0, |b| bad(&b))
                ));
            }
            Some(Err(error)) => {
                entry["kept"] = json!(true);
                entry["restore_error"] = json!(error);
            }
            None => {
                entry["kept"] = json!(true);
                if worse {
                    entry["why"] = json!(
                        "the round's checks came out worse, and no copy was kept to put back"
                    );
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
            "  persist ▸ round {n} {} · {} files changed · {} failed after",
            if kept { "kept" } else { "put back" },
            entry["files_changed"].as_array().map_or(0, Vec::len),
            after.failed
        );
        rounds.push(entry);
        if kept {
            current.tier = tier;
            current.previous = report.output();
            current.checked = Some(rechecked);
            current.support = resupport;
            last_report = Some(report);
        }
        n += 1;
    };
    println!("  persist ▸ stopped: {stopped}");
    let record = json!({
        "policy": policy,
        "rounds": rounds,
        "stopped": stopped,
        "final_tier": current.tier,
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
