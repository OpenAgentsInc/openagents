//! The Microluna executor: short GPT-6 Luna sessions in this process.
//!
//! [`Micro`] is an [`Executor`] beside the Claude Code and Codex adapters,
//! so every policy, record, and Gym view that reads a dispatch reads this
//! one too. It spawns no CLI: each session is a `microluna` session on the
//! operator's Codex login, with Microluna's five native function tools.
//!
//! # Two modes
//!
//! - **`single`** runs one session on the briefing, as a CLI would.
//! - **`requirements`** is the mini-handoff loop of the Luna pivot
//!   (`docs/coder/design/luna-pivot.md`). The requirement map Jev extracted
//!   is split into a few groups. Each group gets short sessions whose
//!   context is rebuilt from scratch: the task first, so every session of
//!   the task shares the cached prefix, then the group's requirements and
//!   only the evidence that informs them, then the current state. After
//!   each session, code runs the checks, asks for the combined verdict
//!   (`checks::verdict`, issue #9584) over the session's report, and Jev
//!   chooses the next move: `next`, `retry`, `stuck`, or `done`. Code keeps
//!   the last word: a move past a requirement a check contradicts becomes a
//!   retry, a verdict of fail keeps the loop from ending, and every loop is
//!   bounded by sessions, spend, and time.
//!
//! # The record
//!
//! Each session is a `microluna.session` invocation. Its tool calls and
//! replies land in the episode log as normalized executor events, the
//! shape the CLI adapters record, so `gym runs show --transcript` and the
//! head-to-head replay show them. Microluna's own ATIF steps, with exact
//! usage and cost per request, go to `artifacts/microluna-<d>-<n>.atif.jsonl`
//! beside them. Each move between sessions is a Jev decision and a
//! `handoff` step, which the Gym shows as a hand-off.

pub mod lean;
pub mod parallel;

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use atif::document::{Source, Step};
use microluna::codex::{CodexTransport, Login};
use microluna::fake::FakeTransport;
use microluna::{Brief, Config, Ending, Evidence, Isolation, TokenUsage, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::checks::{self, Subject};
use crate::component::jev as jev_component;
use crate::delegate::{Briefing, Executor, Prepared, Report, Status, Summary};
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};
use crate::requirements::Kind;
use crate::session::{self, Observation, Version};
use crate::stream::{Event, Kind as EventKind};

/// The agent's name in the record.
pub const AGENT: &str = "microluna";

/// The component each session is recorded under.
pub const SESSION_COMPONENT: &str = "microluna.session";

/// The component each move between sessions is recorded under.
pub const HANDOFF_COMPONENT: &str = "microluna.handoff";

/// The schema of the loop's record.
pub const LOOP_SCHEMA: &str = "openagents.coder-one.microluna-loop.v1";

/// The Jev question that picks the move after a session.
pub const MOVE_QUESTION: &str = "Session `session.number` of a coding agent worked only on the requirements in `focus` of the task in `task`. Read what the session reported in `session` and what the host's checks observed afterwards in `checks`. What should happen next?";

/// The four moves, in the order the question lists them.
pub const MOVES: [(&str, &str); 4] = [
    (
        "next",
        "The requirements in `focus` are met, or as met as the workspace can show, and the task has other requirements left: move on to the next ones.",
    ),
    (
        "retry",
        "The requirements in `focus` are not met yet, and another short session that starts from the current state could make progress on them.",
    ),
    (
        "stuck",
        "The requirements in `focus` are not met, and another session is unlikely to help: the session was blocked, repeated a failed approach, or lacks something it can't get.",
    ),
    (
        "done",
        "The whole task in `task` is complete: every requirement is met, so no more sessions are needed.",
    ),
];

/// How the executor runs a dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// One session on the briefing.
    Single,
    /// The mini-handoff loop over the requirement map.
    Requirements,
}

impl Mode {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Mode::Single => "single",
            Mode::Requirements => "requirements",
        }
    }
}

/// The manifest's `executor.microluna`: the loop's shape and bounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub mode: Mode,
    /// The most sessions one dispatch runs.
    pub max_sessions: u32,
    /// The most sessions one requirement group gets.
    pub max_attempts: u32,
    /// The most requirement groups: a map with more requirements puts
    /// several in a group.
    pub max_groups: u32,
    /// The most model requests one session makes.
    pub session_turns: usize,
    /// One session's wall-time bound, in seconds.
    pub session_sec: u64,
    /// The dispatch's spend bound in dollars: Luna's list-price estimate
    /// plus Jev's.
    pub spend_usd: f64,
    /// The most characters of evidence one session's brief carries.
    pub evidence_chars: usize,
    /// Whether the checks run between sessions.
    pub checks: bool,
    /// Require evidence before a session's move ends the loop or advances
    /// a group: the session must have edited a file and run a command
    /// after its last edit. A `done` or advancing move without both is a
    /// retry. This is Fable's move that Luna lacks: a claim of done backed
    /// by an edit and a test, not by reading.
    #[serde(default = "yes")]
    pub require_evidence: bool,
    /// Run the first session of each group read-only: it reproduces and
    /// runs the task's own tests before any edit, then always retries into
    /// an edit session, without spending one of the group's attempts. This
    /// is Fable's move of reading longer before the first edit.
    #[serde(default)]
    pub read_first: bool,
    /// Gate a loop-ending move on the checks: the loop ends, or advances
    /// past the last group, only when the checks positively confirm the
    /// focus (every covered requirement observed, none contradicted). The
    /// thesis's "done is a program state, not a model's opinion"
    /// (`docs/coder/design/thesis.md`). A minimal acceptance gate until
    /// #9588's accept.define lands.
    #[serde(default)]
    pub accept: bool,
    /// Group only the requirements a session edits toward — behaviors,
    /// deliverables, and checks — and carry every constraint into each
    /// session's brief as a standing rule instead of grouping it. A
    /// constraint such as "set the random seed to 149" or "you have 28800
    /// seconds" is nothing a session can complete on its own, so a group
    /// of constraints spends sessions that make no edit and get downgraded
    /// to retry then stuck. Off keeps every non-context requirement in a
    /// group, as v1 through v4 do.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub focus_actionable: bool,
    /// Drive the loop by an acceptance suite (`accept.define`, issue
    /// #9588): before any fix, a Microluna session writes executable tests
    /// from the task, code proves them red on the untouched workspace and
    /// Jev checks them, and the suite is frozen. Then edit sessions run,
    /// each briefed with the frozen tests and the red tests' output, until
    /// the suite is green or a bound is hit. Done is the suite green, not a
    /// session's report. When the suite comes out with no tests, the
    /// dispatch falls back to the requirements loop.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub suite: bool,
    /// How the suite loop's `accept.define` writes the suite; absent, as
    /// v6 does: one writer, rewrites on any problem, three rounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suite_writer: Option<SuiteWriter>,
    /// Start the first edit session in the workspace while `accept.define`
    /// writes the suite and proves it red on a snapshot of the untouched
    /// workspace, instead of after the freeze.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub overlap_suite: bool,
    /// The most edit sessions that run at once in the suite loop. Above 1,
    /// red tests Jev reads as touching different files run in separate
    /// copies of the workspace, and code merges their changes back.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub parallel_edits: u32,
    /// Ask Jev after each suite-loop session whether the loop is done,
    /// stuck, or should retry the same tests or move to the others, with
    /// code keeping the last word; and stop when two runs in a row leave
    /// the same red tests with the same output.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub handoff_jev: bool,
    /// Once the suite is green, run the task's own visible tests once as a
    /// final guard, give one session to a failure it shows, and stop.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub final_guard: bool,
    /// Gap rounds after a green suite that is partial: each writes
    /// deciding tests for the suite's gaps, proven red on a snapshot of
    /// the untouched workspace, and the loop resumes. A partial suite's
    /// green isn't done while one is left.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gap_rounds: u32,
    /// Spend less time running the suite: reuse the last run when the
    /// workspace hasn't changed since, run only the tests that were red
    /// until they all pass and then the whole suite once, and tell each
    /// session to do the same.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fast_runs: bool,
    /// Before stopping on a green suite, join the evidence in one Jev
    /// judgment: the suite's result and gaps, the last session's report,
    /// and a real diff against the start, over the requirements a session
    /// can show (constraints left out). A partial suite or a done below
    /// [`CLOSE_MIN`] gets one audit session first.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub close_audit: bool,
    /// Characters of the workspace's current source files each suite-loop
    /// session's brief carries, the files its red tests name first, at the
    /// end of the stable prefix, so a session doesn't spend turns reading
    /// them. 0 carries none.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub prefix_sources: usize,
    /// Let a session call several tools in one turn, and run the reads
    /// among them at the same time.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub parallel_tools: bool,
    /// The reasoning effort for a session's turns before its first edit,
    /// such as `low`; the executor's effort holds after it. `null` keeps
    /// one effort throughout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orient_effort: Option<String>,
    /// Tests the host's suite runs and each session's `run.sh` run at
    /// once. 1 runs them one after another.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub test_jobs: u32,
    /// Don't let a guard, a test that passed on the untouched workspace,
    /// hold the loop once an edit turns it red. The task says the code
    /// needs changing, so such a test can pin a defect: the loop counts
    /// the suite green when every other test is, names the red guard in
    /// each later brief as possibly pinning a defect, and doubts the close
    /// so an audit session weighs it against the task.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub advisory_guards: bool,
    /// Run the first gap round as soon as the suite is frozen with gaps,
    /// beside session 1, instead of after the suite goes green.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gap_overlap: bool,
    /// Run the lean loop instead of the requirements or suite loop: a
    /// strong first session, bounded continuations, and a fresh
    /// self-check, with no acceptance suite ([`lean::Lean`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lean: Option<lean::Lean>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

/// Below this probability that the joined evidence shows the task done, a
/// green suite gets an audit session. Not calibrated yet.
pub const CLOSE_MIN: f64 = 0.7;

/// The joined closing question.
pub const CLOSE_QUESTION: &str = "Do the frozen acceptance suite's result in `suite`, the last session's report in `report`, and the changes to the workspace in `diff` together show that the task in `task` is complete, with every requirement in `requirements` met the way the task states it?";

/// The joined closing question for requirement `j`.
#[must_use]
pub fn close_requirement_question(j: usize) -> String {
    format!(
        "Do `suite`, `report`, and `diff` together show that the requirement `requirements[{j}]` \
         is met the way the task in `task` states it, including the standard definition of any \
         method it names?"
    )
}

/// [`close_requirement_question`] without the standard-definition clause.
#[must_use]
pub fn close_requirement_question_general(j: usize) -> String {
    format!(
        "Do `suite`, `report`, and `diff` together show that the requirement `requirements[{j}]` \
         is met the way the task in `task` states it?"
    )
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn one() -> u32 {
    1
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(n: &u32) -> bool {
    *n == 1
}

/// `executor.microluna.suite_writer`: how the suite is written, for a
/// suite that is on the critical path in seconds rather than minutes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteWriter {
    /// Writers in the first round, each on a consecutive share of the
    /// decidable requirements, at the same time; their suites merge into
    /// one that is verified once (`accept::Options::writers`).
    pub writers: u32,
    /// What sends the suite back to a writer: `any` problem, or only
    /// `hard` ones code checks, with Jev's doubts kept as notes on the
    /// tests (`accept::Rewrite`).
    pub rewrite: crate::accept::Rewrite,
    /// The most writing rounds, the first included.
    pub rounds: u32,
    /// A writer's model requests in the first round.
    pub turns: usize,
    /// A targeted repair round's model requests.
    pub repair_turns: usize,
    /// The writers' reasoning effort, or `null` for the provider's default.
    pub effort: Option<String>,
    /// Tell the writer to write every test, then run the suite once.
    pub one_pass: bool,
    /// Tell the writer to find the deciding facts by reading the code's
    /// documentation and running property probes on the untouched code.
    pub discover: bool,
    /// Expand a requirement that sweeps a set of modules into a defect
    /// inventory (`accept::Options::inventory`).
    #[serde(default)]
    pub inventory: bool,
    /// Count the standard definition of a method the task names as stated
    /// (`accept::Options::standard_methods`).
    #[serde(default)]
    pub standard_methods: bool,
    /// Keep tests green on the untouched workspace as guards
    /// (`accept::Options::guards`).
    #[serde(default)]
    pub guards: bool,
    /// Task-neutral guidance in place of the texts written after one
    /// task's analysis (`accept::Options::general`): for the writers, for
    /// Jev's faithfulness question, for the defended-comment scan, and for
    /// the first edit session and the audit.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub general: bool,
}

impl SuiteWriter {
    fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !(1..=4).contains(&self.writers) {
            problems
                .push("executor.microluna.suite_writer.writers must be from 1 to 4".to_string());
        }
        if self.rounds == 0 || self.turns == 0 || self.repair_turns == 0 {
            problems.push(
                "executor.microluna.suite_writer's rounds, turns, and repair_turns must be at least 1"
                    .to_string(),
            );
        }
        problems
    }

    /// The `accept.define` options this writer policy sets.
    #[must_use]
    pub fn options(&self) -> crate::accept::Options {
        crate::accept::Options {
            max_rounds: self.rounds,
            writers: self.writers as usize,
            rewrite: self.rewrite,
            repair_turns: Some(self.repair_turns),
            one_pass: self.one_pass,
            discover: self.discover,
            inventory: self.inventory,
            standard_methods: self.standard_methods,
            guards: self.guards,
            general: self.general,
            ..crate::accept::Options::default()
        }
    }
}

fn yes() -> bool {
    true
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            mode: Mode::Requirements,
            max_sessions: 8,
            max_attempts: 2,
            max_groups: 4,
            session_turns: 30,
            session_sec: 600,
            spend_usd: 0.50,
            evidence_chars: 14_000,
            checks: true,
            require_evidence: true,
            read_first: false,
            accept: false,
            focus_actionable: false,
            suite: false,
            suite_writer: None,
            overlap_suite: false,
            parallel_edits: 1,
            handoff_jev: false,
            final_guard: false,
            gap_rounds: 0,
            fast_runs: false,
            close_audit: false,
            prefix_sources: 0,
            parallel_tools: false,
            orient_effort: None,
            test_jobs: 1,
            advisory_guards: false,
            gap_overlap: false,
            lean: None,
        }
    }
}

/// What an edit session in the suite loop is told.
pub const SUITE_GUIDANCE: &str = "A frozen acceptance suite defines done for this task: it was \
written from the task before any fix and fails on the untouched workspace. Change the workspace \
until every test in it passes. You can't change the tests or anything in their directory. The \
current state shows each red test's output from the host's run just before this session: each red \
test names the fact it checks. Rerun the red tests with the command the evidence gives after every \
edit. Use the exact rule, value, and format the task states, not a simpler one. When every test \
passes, call finish with status done: the host reruns the whole suite after the session, and that \
run decides done. If a test seems to contradict the task, follow the task and say so in your \
summary.";

/// What the first edit session is told when it runs while the suite is
/// written.
pub const EARLY_GUIDANCE: &str = "An acceptance suite for this task is being written while you \
work, from a snapshot of the untouched workspace; it isn't ready yet. Make the change the task \
needs: read the task and its evidence, reproduce the problem, edit, and run the task's own tests \
or examples to see the change work. Use the exact rule, value, and format the task states, not a \
simpler one, and prefer the standard definition of any method the task names over what a comment \
in the code defends. When you're done, call finish with status done and say what you ran. The \
next session starts from the suite's red tests.";

/// [`EARLY_GUIDANCE`] without the standard-definition clause written after
/// one task's analysis: a defended simplification is a suspect to check
/// against the task.
pub const EARLY_GUIDANCE_GENERAL: &str = "An acceptance suite for this task is being written \
while you work, from a snapshot of the untouched workspace; it isn't ready yet. Make the change \
the task needs: read the task and its evidence, reproduce the problem, edit, and run the task's \
own tests or examples to see the change work. Use the exact rule, value, and format the task \
states, not a simpler one. A comment that defends a simplification or a shortcut may describe the \
defect itself: check it against the task. When you're done, call finish with status done and say \
what you ran. The next session starts from the suite's red tests.";

/// What a session reads about guards an edit turned red, with
/// `advisory_guards`.
#[must_use]
pub fn guard_note(red: &[String]) -> String {
    format!(
        "{} passed on the untouched workspace and {} now. The task says the workspace needs \
         changing, so a test that passes on the untouched code can hold on to a defect: the \
         host doesn't count {} against the suite. Where the task's words and {} disagree, follow \
         the task and keep the change, and say which you followed in your summary.",
        red.join(", "),
        if red.len() == 1 { "fails" } else { "fail" },
        if red.len() == 1 { "it" } else { "them" },
        if red.len() == 1 {
            "this test"
        } else {
            "these tests"
        },
    )
}

/// What an audit session on a green suite is told to do. With `general`,
/// it checks the code against the task and the code's own documentation,
/// without the standard-definition clause written after one task's
/// analysis; with `guards`, a red guard may give way to the task.
#[must_use]
pub fn audit_rule(general: bool, guards: bool) -> &'static str {
    match (general, guards) {
        (false, false) => {
            "Audit the work: for each requirement, read the code that implements it and check \
             it against the task's exact rule and the standard definition of any method the task \
             names, and against the choices the code defends in its comments. Fix what's wrong \
             without turning an acceptance test red, run the suite, and call finish."
        }
        (false, true) => {
            "Audit the work: for each requirement, read the code that implements it and check \
             it against the task's exact rule and the standard definition of any method the task \
             names, and against the choices the code defends in its comments. Fix what's wrong \
             without turning a counted acceptance test red; a guard the notes name may give way \
             to the task. Run the suite, and call finish."
        }
        (true, false) => {
            "Audit the work: for each requirement, read the code that implements it and check \
             it against the task's exact rule and against what the code's own documentation \
             states. Fix what's wrong without turning an acceptance test red, run the suite, and \
             call finish."
        }
        (true, true) => {
            "Audit the work: for each requirement, read the code that implements it and check \
             it against the task's exact rule and against what the code's own documentation \
             states. Fix what's wrong without turning a counted acceptance test red; a guard the \
             notes name may give way to the task. Run the suite, and call finish."
        }
    }
}

/// The tests of `result` that are red now and were green on the untouched
/// workspace: guards an edit turned red.
#[must_use]
pub fn red_guards(
    suite: &crate::accept::AcceptanceSuite,
    result: &crate::accept::RunResult,
) -> Vec<String> {
    let Some(start) = &suite.start else {
        return Vec::new();
    };
    result
        .tests
        .iter()
        .filter(|t| !t.green)
        .filter(|t| start.tests.iter().any(|s| s.id == t.id && s.green))
        .map(|t| t.id.clone())
        .collect()
}

/// `result` without the tests in `advisory`: what the loop decides on
/// when red guards don't count.
#[must_use]
pub fn without_tests(
    suite: &crate::accept::AcceptanceSuite,
    result: &crate::accept::RunResult,
    advisory: &[String],
) -> crate::accept::RunResult {
    let tests = result
        .tests
        .iter()
        .filter(|t| !advisory.contains(&t.id))
        .cloned()
        .collect();
    let mut out = crate::accept::RunResult::of(
        &result.label,
        &suite.digest,
        &suite.requirement_ids(),
        tests,
        result.milliseconds,
    );
    out.gaps.clone_from(&result.gaps);
    out.complete = out.green && out.gaps.is_empty();
    out
}

/// The Jev question that picks the suite loop's move after a round.
pub const SUITE_MOVE_QUESTION: &str = "Coding sessions work toward making every test in the frozen acceptance suite in `suite` pass for the task in `task`. `sessions` lists the sessions of the last round: what each worked on, how it ended, and what it changed. `suite` shows which tests are still red and what they print. What should the loop do next?";

/// The suite loop's moves, in the order the question lists them.
pub const SUITE_MOVES: [(&str, &str); 4] = [
    (
        "retry",
        "Another session on the same red tests, starting from the current workspace, is likely to turn more of them green.",
    ),
    (
        "next",
        "The last sessions stalled on their red tests; a session should work on the other red tests first.",
    ),
    (
        "stuck",
        "Another session is unlikely to help: the sessions repeat an approach, are blocked by something they can't change, or the red tests contradict the task.",
    ),
    (
        "done",
        "The work is complete: nothing a session could still change would solve the task more.",
    ),
];

/// A fresh directory name under the system's temporary directory, unique
/// to this process and call: `name-<pid>-<ms>-<n>`.
fn scratch(name: &str) -> PathBuf {
    static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "{name}-{}-{}-{}",
        std::process::id(),
        atif::now_ms(),
        MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

/// Files the task names as outputs that the workspace doesn't have
/// ([`crate::accept::named_outputs`]).
fn missing_outputs(instruction: &str, workdir: &Path, base: Option<&Path>) -> Vec<String> {
    crate::accept::named_outputs(instruction, workdir, base)
}

fn missing_note(missing: &[String]) -> String {
    if missing.is_empty() {
        String::new()
    } else {
        format!(
            " The task names outputs the workspace doesn't have yet: {}. Produce them where and \
             how the task says.",
            missing.join(", ")
        )
    }
}

/// Removes its directory when it goes out of scope.
struct Cleanup(Option<PathBuf>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(dir) = &self.0 {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// What the suite loop ran and why it stopped.
struct Looped {
    sessions: Vec<Ran>,
    moves: Vec<Value>,
    stopped: String,
    /// The timeline's summary ([`parallel::summary`]).
    parallel: Value,
}

/// The writing sessions of a suite as timeline tracks, `from` the
/// dispatch's start: one per round, or one per part of a round with
/// several writers at once.
fn writer_tracks(suite: &crate::accept::AcceptanceSuite, from: u64) -> Vec<parallel::Track> {
    let track = |written: &crate::accept::Written, batch: String, group: Option<String>| {
        written.started_at_ms.map(|at| parallel::Track {
            label: written
                .name
                .clone()
                .unwrap_or_else(|| "accept-writer".to_string()),
            kind: "writer".to_string(),
            batch,
            group,
            workspace: None,
            start_ms: at.saturating_sub(from),
            end_ms: (at + written.milliseconds).saturating_sub(from),
            turns: written.turns,
            input_tokens: 0,
            cached_tokens: 0,
            read_turns: 0,
            cost_usd: written.usd,
        })
    };
    let mut out = Vec::new();
    for round in &suite.rounds {
        if round.parts.is_empty() {
            out.extend(track(
                &round.writer,
                format!("writer round {}", round.number),
                None,
            ));
        } else {
            for part in &round.parts {
                out.extend(track(
                    &part.writer,
                    format!("writers round {}", round.number),
                    Some(part.requirements.join(", ")),
                ));
            }
        }
    }
    out
}

/// How many of the proof's tests fail differently in `now`, when more
/// than half do: the frozen suite doesn't reproduce its proof.
fn unreproduced(
    suite: &crate::accept::AcceptanceSuite,
    now: &crate::accept::RunResult,
) -> Option<usize> {
    let proof = suite.start.as_ref()?;
    let differ = now
        .tests
        .iter()
        .filter(|test| {
            proof
                .tests
                .iter()
                .find(|then| then.id == test.id)
                .is_some_and(|then| then.green != test.green || then.exit != test.exit)
        })
        .count();
    (differ * 2 > now.tests.len().max(1)).then_some(differ)
}

/// The command that runs the task's own visible tests, when this host
/// knows how: pytest over the workspace's `test_*.py` files, or a
/// `package.json` test script. It prints the tail of the output and exits
/// with the tests' status.
fn guard_command(workdir: &Path) -> Option<String> {
    let files = parallel::workspace_files(workdir);
    let pytest = files.iter().any(|f| {
        let name = f.rsplit('/').next().unwrap_or(f);
        (name.starts_with("test_") && name.ends_with(".py")) || name.ends_with("_test.py")
    });
    if pytest {
        return Some(
            "python3 -c 'import pytest' 2>/dev/null || { echo 'no pytest to run the task tests'; exit 0; }\n\
             python3 -m pytest -q -x -p no:cacheprovider > \"$ACCEPT_TMP/guard.out\" 2>&1\n\
             code=$?\n\
             tail -n 40 \"$ACCEPT_TMP/guard.out\"\n\
             exit $code"
                .to_string(),
        );
    }
    let package = std::fs::read_to_string(workdir.join("package.json")).ok()?;
    let scripts: Value = serde_json::from_str(&package).ok()?;
    scripts["scripts"]["test"].as_str()?;
    Some(
        "command -v npm >/dev/null 2>&1 || { echo 'no npm to run the task tests'; exit 0; }\n\
         npm test --silent > \"$ACCEPT_TMP/guard.out\" 2>&1\n\
         code=$?\n\
         tail -n 40 \"$ACCEPT_TMP/guard.out\"\n\
         exit $code"
            .to_string(),
    )
}

impl Policy {
    /// The problems with the bounds, for [`crate::policy::Manifest::validate`].
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.max_sessions == 0 || self.max_attempts == 0 || self.max_groups == 0 {
            problems.push(
                "executor.microluna's max_sessions, max_attempts, and max_groups must be at least 1"
                    .to_string(),
            );
        }
        if self.session_turns == 0 || self.session_sec == 0 {
            problems.push(
                "executor.microluna's session_turns and session_sec must be at least 1".to_string(),
            );
        }
        if self.spend_usd.is_nan() || self.spend_usd <= 0.0 {
            problems.push("executor.microluna.spend_usd must be above 0".to_string());
        }
        if let Some(writer) = &self.suite_writer {
            problems.extend(writer.validate());
            if !self.suite {
                problems.push("executor.microluna.suite_writer needs suite".to_string());
            }
        }
        if !(1..=4).contains(&self.parallel_edits) {
            problems.push("executor.microluna.parallel_edits must be from 1 to 4".to_string());
        }
        if !self.suite
            && (self.overlap_suite
                || self.parallel_edits > 1
                || self.handoff_jev
                || self.final_guard
                || self.gap_rounds > 0
                || self.fast_runs
                || self.close_audit)
        {
            problems.push(
                "executor.microluna's overlap_suite, parallel_edits, handoff_jev, and final_guard \
                 need suite"
                    .to_string(),
            );
        }
        if !(1..=8).contains(&self.test_jobs) {
            problems.push("executor.microluna.test_jobs must be from 1 to 8".to_string());
        }
        if (self.advisory_guards || self.gap_overlap) && !self.suite {
            problems.push(
                "executor.microluna's advisory_guards and gap_overlap need suite".to_string(),
            );
        }
        if let Some(lean) = &self.lean {
            problems.extend(lean.validate());
            if self.suite || self.mode != Mode::Requirements {
                problems.push(
                    "executor.microluna.lean needs mode requirements and no suite".to_string(),
                );
            }
        }
        if self.gap_overlap && (self.gap_rounds == 0 || !self.overlap_suite) {
            problems.push(
                "executor.microluna.gap_overlap needs gap_rounds and overlap_suite".to_string(),
            );
        }
        problems
    }
}

/// The transport a [`Micro`] sends through: the Codex login, or a script.
pub enum Wire {
    Codex(CodexTransport),
    Fake(FakeTransport),
}

impl Transport for Wire {
    async fn respond(
        &self,
        request: &microluna::Request,
    ) -> Result<microluna::Reply, microluna::TransportError> {
        match self {
            Wire::Codex(transport) => transport.respond(request).await,
            Wire::Fake(transport) => transport.respond(request).await,
        }
    }
}

/// The Codex login's path, as Microluna reads it: `$CODEX_HOME/auth.json`
/// or `~/.codex/auth.json`.
#[must_use]
pub fn login_path() -> Option<PathBuf> {
    Login::default_path()
}

/// The doctor's check of the Codex login, without any inference: the file
/// is there and its access token isn't about to expire. Nothing secret is
/// printed.
#[must_use]
pub fn check_login() -> Vec<String> {
    let Some(path) = login_path() else {
        return vec!["microluna: no CODEX_HOME or HOME to find the Codex login in".to_string()];
    };
    match Login::load(&path) {
        Ok(login) => {
            println!(
                "microluna credential: codex_auth_json at {}{}",
                path.display(),
                login
                    .expires_at()
                    .map(|at| format!(", access token valid until {at} (Unix time)"))
                    .unwrap_or_default()
            );
        }
        Err(error) => return vec![format!("microluna: {error}")],
    }
    if takes_login() {
        // The adapter reads this line to refuse an artifact that would
        // leave the login on disk for the model's commands.
        println!("microluna login: take (read into memory and removed when the run starts)");
    }
    Vec::new()
}

/// The variable that asks `episode run` to take the Codex login.
pub const TAKE_LOGIN_VAR: &str = "CODER_ONE_CODEX_LOGIN";

/// Whether `CODER_ONE_CODEX_LOGIN` is `take`.
#[must_use]
pub fn takes_login() -> bool {
    std::env::var(TAKE_LOGIN_VAR).is_ok_and(|value| value.trim().eq_ignore_ascii_case("take"))
}

/// The login [`take_login`] read, or why it couldn't, for this process.
static TAKEN: std::sync::OnceLock<Result<Login, String>> = std::sync::OnceLock::new();

/// Takes the Codex login into memory when `CODER_ONE_CODEX_LOGIN` is
/// `take`, before the episode runs any command: the process is marked
/// non-dumpable, the login is read, and the file and its link are removed
/// (`microluna::codex::Login::take`). Every Microluna session in this
/// process then sends with the login in memory. A missing or unusable
/// login is kept as the reason Microluna has no transport.
///
/// # Errors
///
/// When the file couldn't be removed, which must stop the episode before
/// the model runs anything.
pub fn take_login() -> Result<(), String> {
    if !takes_login() {
        return Ok(());
    }
    let path = login_path().ok_or("no CODEX_HOME or HOME to find the Codex login in")?;
    let taken = match Login::take(&path) {
        Err(error @ microluna::codex::LoginError::Unremovable(..)) => {
            return Err(error.to_string());
        }
        taken => taken.map_err(|error| error.to_string()),
    };
    let _ = TAKEN.set(taken);
    Ok(())
}

/// The Codex transport on the login, or why there is none: the login
/// [`take_login`] holds when it took one, else the file.
///
/// # Errors
///
/// The login's error, as a sentence.
pub fn codex_wire(session_id: &str) -> Result<Wire, String> {
    if let Some(taken) = TAKEN.get() {
        let login = taken.clone()?;
        return CodexTransport::holding(login, session_id)
            .map(Wire::Codex)
            .map_err(|error| error.to_string());
    }
    let path = login_path().ok_or("no CODEX_HOME or HOME to find the Codex login in")?;
    CodexTransport::new(path, session_id)
        .map(Wire::Codex)
        .map_err(|error| error.to_string())
}

/// The Microluna executor.
pub struct Micro {
    pub model: String,
    /// Reasoning effort, or `None` for the provider's default.
    pub effort: Option<String>,
    /// The dispatch's wall-time bound.
    pub deadline: Duration,
    /// The workspace the sessions act in.
    pub workdir: PathBuf,
    /// Where each session's own trace and the loop's record are written.
    pub artifacts: PathBuf,
    /// The episode's recorder: invocations, executor events, and hand-offs.
    pub recorder: Recorder,
    /// Dispatches before this one, so files don't overwrite theirs.
    pub runs: u32,
    pub policy: Policy,
    pub isolation: Isolation,
    /// The transport, or why there is none.
    pub wire: Result<Wire, String>,
    /// The episode deadline every session is bounded by.
    pub episode: crate::deadline::Deadline,
    /// What the checks between sessions read; built from the requirement
    /// map when `None`.
    pub subject: Option<Subject>,
    /// What the briefing was packed from, when the host said.
    pub prepared: Option<Prepared>,
    /// The last dispatch's loop record.
    pub last: Option<Value>,
}

impl Micro {
    /// A Microluna executor on the Codex login, confined by `isolation`.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model: &str,
        effort: Option<String>,
        deadline: Duration,
        workdir: &Path,
        artifacts: &Path,
        recorder: Recorder,
        runs: u32,
        policy: Policy,
        isolation: Isolation,
    ) -> Self {
        let id = format!("microluna-{}-{}", std::process::id(), atif::now_ms());
        Micro {
            model: model.to_string(),
            effort,
            deadline,
            workdir: workdir.to_path_buf(),
            artifacts: artifacts.to_path_buf(),
            recorder,
            runs,
            policy,
            isolation,
            wire: codex_wire(&id),
            episode: crate::deadline::Deadline::unbounded(),
            subject: None,
            prepared: None,
            last: None,
        }
    }

    fn dispatch(&self) -> u32 {
        self.runs + 1
    }
}

/// What one session did, as the loop reads it.
#[derive(Clone, Debug)]
struct Ran {
    number: u32,
    focus: Vec<String>,
    ending: Ending,
    finish: Option<microluna::Finish>,
    turns: usize,
    calls: usize,
    usage: TokenUsage,
    cost_usd: Option<f64>,
    milliseconds: u64,
    session_id: String,
    trace: String,
    commands: Vec<(String, Option<i64>)>,
    changed: Vec<String>,
    /// The session made at least one edit with a file tool.
    edited: bool,
    /// A command ran after the session's last file-tool edit: the edit was
    /// exercised.
    ran_after_edit: bool,
    /// The workspace changed during the session: a file-tool edit, or, in a
    /// Git work tree, a moved HEAD or a changed status. A task done through
    /// commands alone (a git recovery, an `mv`, a `sed -i`) changes it
    /// without a file-tool edit.
    changed_workspace: bool,
    /// A real command ran, not just a file read.
    ran_command: bool,
    /// The session ran read-only, so it was never meant to edit.
    read_only: bool,
    /// When it started and ended, in milliseconds since the Unix epoch.
    started_at_ms: u64,
    ended_at_ms: u64,
    /// Where it worked, and what on, for the timeline.
    place: Place,
    /// Model requests whose every call only read.
    read_turns: usize,
}

/// Where a session works and what it runs beside.
#[derive(Clone, Debug, Default, PartialEq)]
struct Place {
    /// The directory it works in; the workspace when `None`.
    workdir: Option<PathBuf>,
    /// What it works on, for a person, such as `group 2 of 3: T3, T5`.
    group: Option<String>,
    /// The sessions started together with it; its own number's when empty.
    batch: String,
    /// The sessions it runs beside.
    parallel_with: Vec<u32>,
    /// What else runs beside it that isn't a session, such as
    /// `accept.define`.
    alongside: Option<String>,
}

impl Place {
    fn parallel(&self) -> bool {
        !self.parallel_with.is_empty() || self.alongside.is_some()
    }

    fn record(&self, id: &str, number: u32, workdir: &Path) -> Value {
        json!({
            "id": id,
            "session": number,
            "group": self.group,
            "batch": if self.batch.is_empty() { format!("session {number}") } else { self.batch.clone() },
            "parallel_with": self.parallel_with,
            "alongside": self.alongside,
            "workspace": self.workdir.as_deref().unwrap_or(workdir).display().to_string(),
        })
    }
}

impl Ran {
    /// Whether the session did work worth ending or advancing on: it
    /// changed the workspace and ran a command that could exercise the
    /// change. A file-tool edit needs a command after it; a change made
    /// through commands already ran one.
    fn evidence(&self) -> bool {
        (self.edited && self.ran_after_edit) || (self.changed_workspace && self.ran_command)
    }
}

impl Ran {
    fn status(&self) -> String {
        match (&self.ending, &self.finish) {
            (Ending::Finished, Some(finish)) => serde_json::to_value(finish.status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "finished".to_string()),
            (Ending::Stopped, _) => "stopped".to_string(),
            (Ending::TurnLimit, _) => "turn_limit".to_string(),
            (Ending::Deadline, _) => "deadline".to_string(),
            (Ending::Transport(_), _) => "transport".to_string(),
            (Ending::Finished, None) => "finished".to_string(),
        }
    }

    fn summary(&self) -> String {
        match &self.finish {
            Some(finish) if finish.answer.trim().is_empty() => finish.summary.clone(),
            Some(finish) => format!("{}\n\nAnswer: {}", finish.summary, finish.answer),
            None => match &self.ending {
                Ending::Transport(why) => format!("The session lost its provider: {why}"),
                Ending::Deadline => "The session's time bound passed.".to_string(),
                Ending::TurnLimit => "The session used every turn it had.".to_string(),
                _ => "The session stopped without calling finish.".to_string(),
            },
        }
    }

    fn record(&self) -> Value {
        json!({
            "number": self.number,
            "focus": self.focus,
            "status": self.status(),
            "finish": self.finish,
            "turns": self.turns,
            "calls": self.calls,
            "usage": usage_json(self.usage),
            "cost_usd": self.cost_usd,
            "milliseconds": self.milliseconds,
            "session_id": self.session_id,
            "trace": self.trace,
            "commands": self.commands.iter().map(|(c, e)| json!({ "command": c, "exit": e })).collect::<Vec<_>>(),
            "changed": self.changed,
            "edited": self.edited,
            "ran_after_edit": self.ran_after_edit,
            "changed_workspace": self.changed_workspace,
            "ran_command": self.ran_command,
            "read_only": self.read_only,
            "cause": self.finish.as_ref().map(|f| f.cause.word()),
            "started_at_ms": self.started_at_ms,
            "ended_at_ms": self.ended_at_ms,
            "read_turns": self.read_turns,
            "group": self.place.group,
            "batch": self.place.batch,
            "parallel_with": self.place.parallel_with,
            "alongside": self.place.alongside,
            "workspace": self.place.workdir,
        })
    }

    /// The session as a timeline track, `from` the dispatch's start.
    fn track(&self, from: u64) -> parallel::Track {
        parallel::Track {
            label: format!("session {}", self.number),
            kind: "edit".to_string(),
            batch: if self.place.batch.is_empty() {
                format!("session {}", self.number)
            } else {
                self.place.batch.clone()
            },
            group: self.place.group.clone(),
            workspace: self.place.workdir.as_ref().map(|p| p.display().to_string()),
            start_ms: self.started_at_ms.saturating_sub(from),
            end_ms: self.ended_at_ms.saturating_sub(from),
            turns: self.turns,
            input_tokens: self.usage.input,
            cached_tokens: self.usage.cached,
            read_turns: self.read_turns,
            cost_usd: self.cost_usd.unwrap_or(0.0),
        }
    }

    fn blocked(&self) -> bool {
        self.status() == "blocked"
    }

    fn cause(&self) -> microluna::Cause {
        self.finish
            .as_ref()
            .map_or(microluna::Cause::None, |f| f.cause)
    }
}

fn usage_json(usage: TokenUsage) -> Value {
    json!({
        "input": usage.input,
        "cached": usage.cached,
        "output": usage.output,
        "reasoning": usage.reasoning,
    })
}

/// A group of requirements one or more sessions work on together.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub ids: Vec<String>,
    /// `- R2 (behavior): text` lines.
    pub lines: Vec<String>,
}

/// The requirement groups, in map order, split into at most `max`
/// consecutive groups of near-equal size. Context is always left out. When
/// `actionable_only`, constraints are left out too, so a group holds only
/// the behaviors, deliverables, and checks a session edits toward; the
/// constraints reach every session through the brief instead. When a task
/// has no actionable requirement, `actionable_only` falls back to every
/// non-context requirement so the loop still runs.
#[must_use]
pub fn groups(
    map: &crate::requirements::RequirementMap,
    max: u32,
    actionable_only: bool,
) -> Vec<Group> {
    let actionable = |k: Kind| matches!(k, Kind::Behavior | Kind::Deliverable | Kind::Check);
    let mut kept: Vec<_> = map
        .requirements
        .iter()
        .filter(|r| {
            if actionable_only {
                actionable(r.kind)
            } else {
                r.kind != Kind::Context
            }
        })
        .collect();
    if actionable_only && kept.is_empty() {
        kept = map
            .requirements
            .iter()
            .filter(|r| r.kind != Kind::Context)
            .collect();
    }
    if kept.is_empty() {
        return Vec::new();
    }
    let max = (max.max(1) as usize).min(kept.len());
    let size = kept.len().div_ceil(max);
    kept.chunks(size)
        .map(|chunk| Group {
            ids: chunk.iter().map(|r| r.id.clone()).collect(),
            lines: chunk
                .iter()
                .map(|r| {
                    format!(
                        "- {} ({}): {}",
                        r.id,
                        r.kind.word(),
                        r.text.split_whitespace().collect::<Vec<_>>().join(" ")
                    )
                })
                .collect(),
        })
        .collect()
}

/// The constraints every session must honor: the `- R7 (constraint): text`
/// lines for the map's constraint requirements, in map order. These are
/// the standing rules a focused session works under when `focus_actionable`
/// keeps them out of the groups. Context is left out, since it repeats the
/// task rather than constraining the work.
#[must_use]
pub fn constraints(map: &crate::requirements::RequirementMap) -> Vec<String> {
    map.requirements
        .iter()
        .filter(|r| r.kind == Kind::Constraint)
        .map(|r| {
            format!(
                "- {} ({}): {}",
                r.id,
                r.kind.word(),
                r.text.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect()
}

/// The evidence one group's sessions read: the items that inform any of
/// its requirements, by relevance, within `chars`. With none, the three
/// most relevant items.
#[must_use]
pub fn evidence_for(prepared: &Prepared, group: &Group, chars: usize) -> Vec<Evidence> {
    let informs = |id: &str| {
        prepared
            .informs
            .get(id)
            .is_some_and(|ids| ids.iter().any(|r| group.ids.contains(r)))
    };
    let mut chosen: Vec<&crate::pack::Item> = prepared
        .items
        .iter()
        .filter(|item| informs(&item.id) && !item.text.trim().is_empty())
        .collect();
    if chosen.is_empty() {
        chosen = prepared
            .items
            .iter()
            .filter(|item| !item.text.trim().is_empty())
            .collect();
        chosen.sort_by(|a, b| b.p.unwrap_or(0.0).total_cmp(&a.p.unwrap_or(0.0)));
        chosen.truncate(3);
    } else {
        chosen.sort_by(|a, b| b.p.unwrap_or(0.0).total_cmp(&a.p.unwrap_or(0.0)));
    }
    let share = (chars / chosen.len().max(1)).max(1_500);
    let mut left = chars;
    let mut out = Vec::new();
    for item in chosen {
        if left < 400 {
            break;
        }
        let room = share.min(left);
        let text = if item.text.chars().count() > room {
            format!(
                "{}\n[trimmed: {} of {} characters shown]",
                crate::judge::clip(&item.text, room),
                room,
                item.text.chars().count()
            )
        } else {
            item.text.clone()
        };
        left = left.saturating_sub(text.chars().count());
        out.push(Evidence {
            label: item.label.clone(),
            text,
        });
    }
    out
}

/// The next move after a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Next,
    Retry,
    Stuck,
    Done,
}

impl Move {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Move::Next => "next",
            Move::Retry => "retry",
            Move::Stuck => "stuck",
            Move::Done => "done",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        match word {
            "next" => Some(Move::Next),
            "retry" => Some(Move::Retry),
            "stuck" => Some(Move::Stuck),
            "done" => Some(Move::Done),
            _ => None,
        }
    }
}

/// What the checks said about one group, for the brief and for Jev.
#[derive(Clone, Debug, Default)]
struct Checked {
    /// `R2 observed`, one per requirement the checks covered.
    states: Vec<(String, String)>,
    /// Failed scenarios' short accounts, for the group's requirements.
    failures: Vec<String>,
    summary: Value,
}

impl Checked {
    fn from_report(report: &checks::Report, group: &Group) -> Self {
        let states = report
            .coverage
            .iter()
            .map(|c| (c.id.clone(), c.state.clone()))
            .collect();
        let failures = report
            .packets
            .iter()
            .filter(|p| group.ids.contains(&p.requirement))
            .map(|p| {
                format!(
                    "{} failed for {}: expected {}; observed {}",
                    p.scenario,
                    p.requirement,
                    crate::judge::clip(
                        &serde_json::to_string(&p.expected).unwrap_or_default(),
                        300
                    ),
                    crate::judge::clip(
                        &serde_json::to_string(&p.observations).unwrap_or_default(),
                        500
                    )
                )
            })
            .collect();
        Checked {
            states,
            failures,
            summary: report.summary(),
        }
    }

    fn contradicted(&self, group: &Group) -> bool {
        !self.failures.is_empty()
            || self
                .states
                .iter()
                .any(|(id, state)| group.ids.contains(id) && state == "contradicted")
    }

    /// Whether the checks positively confirm the group: every focus
    /// requirement the checks cover is observed, none contradicted, and at
    /// least one is observed. `None` when the checks didn't run or cover the
    /// group, so the caller can't gate on them. This is the thesis's "done
    /// is a program state": the loop ends on green checks, not the model's
    /// opinion.
    fn accepts(&self, group: &Group) -> Option<bool> {
        if self.contradicted(group) {
            return Some(false);
        }
        let covered: Vec<&str> = self
            .states
            .iter()
            .filter(|(id, _)| group.ids.contains(id))
            .map(|(_, state)| state.as_str())
            .collect();
        if covered.is_empty() {
            return None;
        }
        Some(covered.contains(&"observed"))
    }

    fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.states.is_empty() {
            out.push(
                self.states
                    .iter()
                    .map(|(id, state)| format!("{id} {state}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        out.extend(self.failures.iter().cloned());
        out
    }
}

/// What the code rule reads besides Jev's pick.
#[derive(Clone, Copy, Debug, Default)]
struct Signals {
    /// A check contradicts a requirement of the group.
    contradicted: bool,
    /// The combined verdict (`checks::verdict`) calls the candidate failed.
    verdict_fail: bool,
    /// The group is the last one.
    last: bool,
    /// The session ran read-only, so it only reconnoiters and retries.
    read_only: bool,
    /// The session edited a file and ran a command after its last edit.
    evidence: bool,
    /// Whether a move that ends the loop needs that evidence.
    require_evidence: bool,
    /// Whether the checks confirm the focus, when the accept gate is on:
    /// `Some(false)` blocks an ending move, `None` leaves it to the other
    /// rules (the gate is off, or the checks don't cover the group).
    accepts: Option<bool>,
    attempts: u32,
    max_attempts: u32,
}

/// The code rule over Jev's pick: a check that contradicts the group keeps
/// it from moving on; a combined verdict of fail keeps the loop from
/// ending, by `done` or past the last group; attempts are bounded; and
/// without an answer the session's own typed status decides.
fn settle(picked: Option<Move>, ran: &Ran, signals: Signals) -> (Move, Option<String>) {
    let Signals {
        contradicted,
        verdict_fail,
        last,
        read_only,
        evidence,
        require_evidence,
        accepts,
        attempts,
        max_attempts,
    } = signals;
    // A read-only session never advances: it reconnoiters, then an edit
    // session follows on the same group.
    if read_only {
        return (
            Move::Retry,
            Some("the read-only first session retries into an edit session".to_string()),
        );
    }
    let finished_done = matches!(
        ran.finish.as_ref().map(|f| f.status),
        Some(microluna::FinishStatus::Done)
    );
    let fallback = if finished_done && !contradicted {
        Move::Next
    } else {
        Move::Retry
    };
    let (mut chosen, mut why) = match picked {
        Some(chosen) => (chosen, None),
        None => (
            fallback,
            Some("Jev gave no answer, so the session's own status decided".to_string()),
        ),
    };
    if contradicted && matches!(chosen, Move::Next | Move::Done) {
        why = Some(format!(
            "a check contradicts the focus, so {} became retry",
            chosen.word()
        ));
        chosen = Move::Retry;
    }
    let ends = chosen == Move::Done || (last && chosen == Move::Next);
    if require_evidence && ends && !evidence {
        why = Some(format!(
            "{}the session made no edit it then tested, so {} became retry",
            why.map(|w| format!("{w}; ")).unwrap_or_default(),
            chosen.word()
        ));
        chosen = Move::Retry;
    }
    let ends = chosen == Move::Done || (last && chosen == Move::Next);
    if accepts == Some(false) && ends {
        why = Some(format!(
            "{}the checks don't confirm the focus yet, so {} became retry",
            why.map(|w| format!("{w}; ")).unwrap_or_default(),
            chosen.word()
        ));
        chosen = Move::Retry;
    }
    let ends = chosen == Move::Done || (last && chosen == Move::Next);
    if verdict_fail && ends {
        why = Some(format!(
            "the combined verdict calls the candidate failed, so {} became retry",
            chosen.word()
        ));
        chosen = Move::Retry;
    }
    if chosen == Move::Retry && attempts >= max_attempts {
        why = Some(format!(
            "{}the group had its {max_attempts} attempts, so retry became stuck",
            why.map(|w| format!("{w}; ")).unwrap_or_default()
        ));
        chosen = Move::Stuck;
    }
    (chosen, why)
}

fn clip_lines(text: &str, max: usize) -> String {
    crate::judge::clip(text.trim(), max)
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A signature of the Git work tree at `dir`: the HEAD commit and the
/// porcelain status, so a session that moves HEAD or changes a file is
/// seen even when it edits through commands. `None` when `dir` isn't a Git
/// work tree, where a file-tool edit is the only change worth counting.
fn git_signature(dir: &Path) -> Option<String> {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    if run(&["rev-parse", "--is-inside-work-tree"])?.trim() != "true" {
        return None;
    }
    let head = run(&["rev-parse", "HEAD"]).unwrap_or_default();
    let status = run(&["status", "--porcelain"]).unwrap_or_default();
    Some(format!("{head}\n{status}"))
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The normalized events one Microluna step becomes.
fn events_of(step: &Step) -> Vec<EventKind> {
    if let Some(call) = &step.call {
        let arguments = &call.arguments;
        let arg = |key: &str| arguments.get(key).and_then(Value::as_str).unwrap_or("");
        let completed = call.outcome == atif::Outcome::Completed;
        return match call.name.as_str() {
            "run_command" => vec![
                EventKind::CommandStarted {
                    command: arg("command").to_string(),
                },
                EventKind::CommandCompleted {
                    command: arg("command").to_string(),
                    exit_code: call.extra.get("exit").and_then(Value::as_i64),
                    output: crate::judge::clip(&call.output, crate::stream::OUTPUT_CHARS),
                },
            ],
            "read_file" => {
                let from = arguments
                    .get("start_line")
                    .and_then(Value::as_u64)
                    .unwrap_or(1);
                let command = format!("read_file {} (from line {from})", arg("path"));
                vec![
                    EventKind::CommandStarted {
                        command: command.clone(),
                    },
                    EventKind::CommandCompleted {
                        command,
                        exit_code: Some(i64::from(!completed)),
                        output: crate::judge::clip(&call.output, 600),
                    },
                ]
            }
            "apply_patch" if completed => arg("patch")
                .lines()
                .filter_map(|line| {
                    [
                        ("*** Add File: ", "add"),
                        ("*** Update File: ", "update"),
                        ("*** Delete File: ", "delete"),
                    ]
                    .iter()
                    .find_map(|(prefix, change)| {
                        line.strip_prefix(prefix)
                            .map(|path| EventKind::ArtifactChanged {
                                path: path.trim().to_string(),
                                change: (*change).to_string(),
                            })
                    })
                })
                .collect(),
            "apply_patch" => vec![
                EventKind::CommandStarted {
                    command: "apply_patch".to_string(),
                },
                EventKind::CommandCompleted {
                    command: "apply_patch".to_string(),
                    exit_code: Some(1),
                    output: crate::judge::clip(&call.output, 600),
                },
            ],
            "write_file" if completed => vec![EventKind::ArtifactChanged {
                path: arg("path").to_string(),
                change: "write".to_string(),
            }],
            "finish" => vec![EventKind::SessionEnded {
                error: arg("status") != "done",
                result: Some(
                    [arg("summary"), arg("answer")]
                        .iter()
                        .filter(|t| !t.trim().is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                ),
            }],
            _ => Vec::new(),
        };
    }
    if step.source == atif::Source::Agent && !step.message.trim().is_empty() {
        return vec![EventKind::AssistantClaim {
            text: crate::judge::clip(&step.message, crate::stream::CLAIM_CHARS),
        }];
    }
    Vec::new()
}

impl Micro {
    /// Runs one session in the workspace, recording it; `why` names what it
    /// works on.
    async fn session(
        &self,
        number: u32,
        focus: &[String],
        why: &str,
        brief: &Brief,
        read_only: bool,
    ) -> Ran {
        self.session_at(number, focus, why, brief, read_only, Place::default())
            .await
    }

    /// Runs one session where `place` says, recording it.
    #[allow(clippy::too_many_lines)]
    async fn session_at(
        &self,
        number: u32,
        focus: &[String],
        why: &str,
        brief: &Brief,
        read_only: bool,
        place: Place,
    ) -> Ran {
        let started_at_ms = atif::now_ms();
        let workdir = place
            .workdir
            .clone()
            .unwrap_or_else(|| self.workdir.clone());
        let dispatch = self.dispatch();
        let session_id = format!("microluna-{dispatch}-{number}");
        let text: String = brief
            .input()
            .iter()
            .filter_map(|item| item["content"][0]["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if self.policy.mode == Mode::Requirements {
            // A section per session: the Gym shows each as a takeover, and
            // its lane says what it worked on and what ran beside it.
            self.recorder.push(
                crate::delegate::with_briefing(
                    Step::said(
                        Source::System,
                        &format!(
                            "Delegating to {AGENT} ({}) because {why}. Briefing: {} characters, sha256 {}.",
                            self.model,
                            text.chars().count(),
                            sha256(&text)
                        ),
                    ),
                    &text,
                )
                .noting(
                    parallel::LANE_EXTENSION,
                    place.record(&session_id, number, &self.workdir),
                ),
            );
        }
        // Concurrent sessions' lines interleave, so each carries its number.
        let tag = if place.parallel() {
            format!("[{number}] ")
        } else {
            String::new()
        };
        let read_turns = Rc::new(Cell::new(0usize));
        let turn_reads = Rc::new(Cell::new(None::<bool>));
        let invocation = self.recorder.enter(
            Start::new(
                SESSION_COMPONENT,
                Implementation::new(
                    SESSION_COMPONENT,
                    &format!("{AGENT} {}", self.model),
                    &json!({
                        "model": self.model,
                        "effort": self.effort,
                        "turns": self.policy.session_turns,
                        "session_sec": self.policy.session_sec,
                        "instructions": microluna::session::INSTRUCTIONS,
                        "tools": microluna::tools::declarations(),
                        "isolation": self.isolation.word(),
                    }),
                ),
            )
            .named(&format!("session {number}: {}", focus.join(", ")))
            .reading_digest(sha256(&text))
            .with_effects(),
        );
        let trace = format!("microluna-{dispatch}-{number}.atif.jsonl");
        let mut atif_session = atif::Session::opening(
            &session_id,
            &self.model,
            "codex-login",
            &workdir.display().to_string(),
            &crate::episode::version(),
        );
        atif_session.directive = why.to_string();
        let started = Instant::now();
        let seq = Rc::new(Cell::new(0u64));
        let revision = Rc::new(Cell::new(0u64));
        let commands = Rc::new(std::cell::RefCell::new(Vec::<(String, Option<i64>)>::new()));
        let changed = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        // Set when an edit is seen, cleared when a real command runs after
        // it: at the end it says whether the last edit went untested.
        let untested_edit = Rc::new(Cell::new(false));
        let ran_after_edit = Rc::new(Cell::new(false));
        // Set when any command that isn't a file read runs.
        let ran_command = Rc::new(Cell::new(false));
        let before = git_signature(&workdir);
        let sink = {
            let recorder = self.recorder.clone();
            let session_id = session_id.clone();
            let seq = seq.clone();
            let revision = revision.clone();
            let commands = commands.clone();
            let changed = changed.clone();
            let untested_edit = untested_edit.clone();
            let ran_after_edit = ran_after_edit.clone();
            let ran_command = ran_command.clone();
            let read_turns = read_turns.clone();
            let turn_reads = turn_reads.clone();
            move |step: &Step| {
                if step.call.is_some() || step.source == atif::Source::Agent {
                    crate::say::line(&format!("  {tag}{}", microluna::session::line(step)));
                }
                // A reply opens a turn; the turn only read when each of its
                // calls did.
                if step.source == atif::Source::Agent && step.call.is_none() {
                    if turn_reads.get() == Some(true) {
                        read_turns.set(read_turns.get() + 1);
                    }
                    turn_reads.set(None);
                }
                if let Some(call) = &step.call {
                    let reads =
                        microluna::tools::reads_only(&call.name, &call.arguments.to_string());
                    turn_reads.set(Some(turn_reads.get().unwrap_or(true) && reads));
                }
                for kind in events_of(step) {
                    match &kind {
                        EventKind::CommandCompleted {
                            command, exit_code, ..
                        } => {
                            commands.borrow_mut().push((command.clone(), *exit_code));
                            // A read_file's synthetic command doesn't test an edit.
                            if !command.starts_with("read_file ") {
                                ran_command.set(true);
                                if untested_edit.get() {
                                    untested_edit.set(false);
                                    ran_after_edit.set(true);
                                }
                            }
                        }
                        EventKind::ArtifactChanged { path, .. } => {
                            revision.set(revision.get() + 1);
                            untested_edit.set(true);
                            ran_after_edit.set(false);
                            if !changed.borrow().contains(path) {
                                changed.borrow_mut().push(path.clone());
                            }
                        }
                        _ => {}
                    }
                    seq.set(seq.get() + 1);
                    let observation = Observation {
                        version: Version {
                            session_id: Some(session_id.clone()),
                            generation: 1,
                            seq: seq.get(),
                            revision: revision.get(),
                        },
                        at_ms: millis(started),
                        event: Event {
                            seq: seq.get(),
                            line: usize::try_from(seq.get()).unwrap_or(usize::MAX),
                            offset: None,
                            kind,
                        },
                    };
                    recorder.push(session::observation_step(AGENT, &observation));
                }
            }
        };
        let mut recorder = microluna::Recorder::new().forwarding(sink);
        match atif::Log::create_at(&self.artifacts.join(&trace), &atif_session) {
            Ok(log) => recorder = recorder.logging(log),
            Err(error) => crate::say::line(&format!(
                "  microluna ▸ not tracing session {number}: {error}"
            )),
        }
        let left = self
            .episode
            .allowance()
            .map_or(self.deadline, |left| left.min(self.deadline));
        let config = Config {
            model: self.model.clone(),
            effort: self.effort.clone(),
            max_turns: self.policy.session_turns,
            cache_key: format!(
                "microluna-{}",
                &sha256(
                    &self
                        .prepared
                        .as_ref()
                        .map_or(String::new(), |p| p.instruction.clone())
                )[..16]
            ),
            deadline: Some(Duration::from_secs(self.policy.session_sec).min(left)),
            orient_effort: self.policy.orient_effort.clone(),
            parallel_tools: self.policy.parallel_tools,
        };
        let workspace = microluna::Workspace::new(&workdir).map(|workspace| {
            let workspace = workspace.isolated_by(self.isolation);
            if read_only {
                workspace.reading_only()
            } else {
                workspace
            }
        });
        let report = match (&self.wire, workspace) {
            (Ok(wire), Ok(workspace)) => {
                microluna::run(wire, &workspace, brief, &config, &mut recorder).await
            }
            (Err(why), _) => microluna::Report {
                ending: Ending::Transport(why.clone()),
                finish: None,
                turns: 0,
                calls: 0,
                usage: TokenUsage::default(),
                cost_usd: Some(0.0),
                milliseconds: 0,
            },
            (_, Err(error)) => microluna::Report {
                ending: Ending::Transport(format!(
                    "the workspace {} can't be used: {error}",
                    workdir.display()
                )),
                finish: None,
                turns: 0,
                calls: 0,
                usage: TokenUsage::default(),
                cost_usd: Some(0.0),
                milliseconds: 0,
            },
        };
        recorder.close(match report.ending {
            Ending::Finished => atif::log::ENDED,
            _ => atif::log::INTERRUPTED,
        });
        let ran = Ran {
            number,
            focus: focus.to_vec(),
            ending: report.ending.clone(),
            finish: report.finish.clone(),
            turns: report.turns,
            calls: report.calls,
            usage: report.usage,
            cost_usd: report.cost_usd,
            milliseconds: report.milliseconds,
            session_id,
            trace: format!("artifacts/{trace}"),
            commands: commands.borrow().clone(),
            changed: changed.borrow().clone(),
            edited: !changed.borrow().is_empty(),
            ran_after_edit: ran_after_edit.get(),
            changed_workspace: !changed.borrow().is_empty()
                || git_signature(&workdir).is_some_and(|after| Some(&after) != before.as_ref()),
            ran_command: ran_command.get(),
            read_only,
            started_at_ms,
            ended_at_ms: atif::now_ms(),
            place,
            read_turns: read_turns.get() + usize::from(turn_reads.get() == Some(true)),
        };
        // The dispatch's exec.session carries the sessions' cost, as it
        // does for a CLI; each session states its own share in its summary,
        // so a reader that sums invocation costs counts it once.
        self.recorder.end(
            &invocation,
            Finish::new(match ran.ending {
                Ending::Finished => Outcome::Completed,
                _ => Outcome::Failed,
            })
            .summary(json!({
                "status": ran.status(),
                "summary": crate::judge::clip(&ran.summary(), 600),
                "turns": ran.turns,
                "calls": ran.calls,
                "usage": usage_json(ran.usage),
                "cost_usd": ran.cost_usd,
                "cost_provenance": match ran.ending {
                    Ending::Deadline => "price_estimate_lower_bound",
                    _ => "price_estimate",
                },
                "trace": ran.trace,
                "read_turns": ran.read_turns,
                "cause": ran.finish.as_ref().map(|f| f.cause.word()),
                "group": ran.place.group,
                "parallel_with": ran.place.parallel_with,
            })),
        );
        crate::say::line(&format!(
            "  microluna ▸ session {number} ({}) {}{} in {:.1}s · {} turns ({} read-only) · {} calls · in {} (cached {}) out {} · ${:.5}",
            focus.join(", "),
            ran.status(),
            match ran.cause() {
                microluna::Cause::None => String::new(),
                cause => format!(" ({})", cause.word()),
            },
            ran.milliseconds as f64 / 1000.0,
            ran.turns,
            ran.read_turns,
            ran.calls,
            ran.usage.input,
            ran.usage.cached,
            ran.usage.output,
            ran.cost_usd.unwrap_or_default()
        ));
        ran
    }

    /// The single mode: one session on the briefing.
    async fn single(&self, briefing: &Briefing) -> Vec<Ran> {
        let brief = Brief::task(&briefing.text);
        vec![
            self.session(
                1,
                &["the briefing".to_string()],
                "the host briefed it",
                &brief,
                false,
            )
            .await,
        ]
    }

    /// The checks' subject: the one the host set, or the task's words and
    /// the requirement map against the live workspace.
    fn subject(&self, prepared: &Prepared) -> Subject {
        let mut subject = self.subject.clone().unwrap_or_else(|| Subject {
            label: "microluna workspace".to_string(),
            task: checks::TaskText {
                title: prepared.title.clone(),
                instruction: prepared.instruction.clone(),
            },
            requirements: None,
            provided: Vec::new(),
            inputs: (prepared.instruction.contains("logs/") && self.workdir.join("logs").is_dir())
                .then(|| "logs".to_string()),
            budget: checks::Budget::default(),
            live: Some(checks::generic::Workspace {
                dir: self.workdir.to_string_lossy().into_owned(),
                claimed: Vec::new(),
                command_sec: 60,
                report: None,
                options: checks::generic::Options::default(),
                root: None,
                collected: Vec::new(),
                suite: None,
            }),
            distrust: Vec::new(),
        });
        subject.requirements = Some(prepared.requirements.clone());
        subject
    }

    /// Asks Jev for the move after `ran`, and settles it with the code rule.
    #[allow(clippy::too_many_arguments)]
    async fn decide(
        &self,
        prepared: &Prepared,
        group: &Group,
        later: &[Group],
        ran: &Ran,
        checked: &Checked,
        attempts: u32,
    ) -> (Move, Value, f64) {
        // The combined verdict over the task and this session's report:
        // the handoff signal issue #9584 calibrated against the verifier.
        let (_, verdict, verdict_asked) = checks::verdict::assess(
            &prepared.jev,
            &self.recorder,
            &prepared.instruction,
            &ran.summary(),
            prepared.deadline.clone(),
            &checks::verdict::fitted(),
        )
        .await;
        let state = json!({
            "task": clip_lines(&prepared.instruction, 4_000),
            "focus": group.lines,
            "later_requirements": later.iter().flat_map(|g| g.lines.clone()).collect::<Vec<_>>(),
            "session": {
                "number": ran.number,
                "attempt_on_focus": attempts,
                "status": ran.status(),
                "report": clip_lines(&ran.summary(), 1_500),
                "commands": ran.commands.iter().rev().take(8).rev()
                    .map(|(c, e)| json!({ "command": crate::judge::clip(c, 200), "exit": e }))
                    .collect::<Vec<_>>(),
                "changed_files": ran.changed,
            },
            "checks": {
                "ran": self.policy.checks,
                "requirement_states": checked.states.iter().map(|(id, s)| json!({ "id": id, "state": s })).collect::<Vec<_>>(),
                "failures_on_focus": checked.failures,
            },
            "verdict": {
                "call": verdict.call,
                "failure_probability": verdict.p_fail,
            },
        });
        let mut choice = jev::Choice::new(MOVE_QUESTION, indexmap::IndexMap::new());
        for (name, meaning) in MOVES {
            choice = choice.option(name, meaning);
        }
        let questions = jev::Questions::new().with("next_move", choice);
        let asked = jev_component::ask(
            &prepared.jev,
            &self.recorder,
            jev_component::Ask {
                component: HANDOFF_COMPONENT,
                name: "jev_next_move",
                id: format!("jev-next-move-{}-{}", self.dispatch(), ran.number),
                state,
                questions,
                parent: None,
                deadline: prepared.deadline.clone(),
            },
        )
        .await;
        let picked = asked.choice("next_move").and_then(Move::parse);
        let p = asked
            .answers
            .as_ref()
            .and_then(|a| a.pointer("/next_move/probabilities"))
            .cloned()
            .unwrap_or(Value::Null);
        let contradicted = checked.contradicted(group);
        let (chosen, overridden) = settle(
            picked,
            ran,
            Signals {
                contradicted,
                verdict_fail: verdict.call == "fail",
                last: later.is_empty(),
                read_only: ran.read_only,
                evidence: ran.evidence(),
                require_evidence: self.policy.require_evidence,
                accepts: if self.policy.accept {
                    checked.accepts(group)
                } else {
                    None
                },
                attempts,
                max_attempts: self.policy.max_attempts,
            },
        );
        let usd = [asked.input_tokens, verdict_asked.input_tokens]
            .iter()
            .flatten()
            .map(|tokens| *tokens as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0)
            .sum::<f64>();
        let record = json!({
            "after_session": ran.number,
            "focus": group.ids,
            "jev": { "how": asked.how, "picked": picked.map(Move::word), "probabilities": p, "error": asked.error },
            "contradicted": contradicted,
            "checks": checked.summary,
            "verdict": verdict,
            "edited": ran.edited,
            "ran_after_edit": ran.ran_after_edit,
            "read_only": ran.read_only,
            "attempts": attempts,
            "move": chosen.word(),
            "overridden": overridden,
            "jev_usd": usd,
        });
        (chosen, record, usd)
    }

    /// The mini-handoff loop.
    async fn requirements(
        &self,
        prepared: &Prepared,
        prior: Vec<Ran>,
    ) -> (Vec<Ran>, Vec<Value>, String) {
        let groups = groups(
            &prepared.requirements,
            self.policy.max_groups,
            self.policy.focus_actionable,
        );
        let subject = self.subject(prepared);
        let dir = self
            .artifacts
            .parent()
            .map_or_else(|| self.artifacts.clone(), Path::to_path_buf);
        let started = Instant::now();
        // Sessions an abandoned suite loop already ran count against the
        // bounds and keep their numbers.
        let mut spent = prior.iter().filter_map(|r| r.cost_usd).sum::<f64>();
        let mut sessions: Vec<Ran> = prior;
        let mut moves: Vec<Value> = Vec::new();
        let mut attempts = vec![0u32; groups.len()];
        let mut read_done = vec![false; groups.len()];
        let mut cursor = 0;
        let mut last_checked: Option<Checked> = None;
        let mut stopped = "every requirement group had its turn".to_string();
        while cursor < groups.len() {
            let number = u32::try_from(sessions.len()).unwrap_or(u32::MAX) + 1;
            if number > self.policy.max_sessions {
                stopped = format!("the bound of {} sessions", self.policy.max_sessions);
                break;
            }
            if spent >= self.policy.spend_usd {
                stopped = format!(
                    "the spend bound of ${:.2}: ${spent:.4} spent",
                    self.policy.spend_usd
                );
                break;
            }
            let left = self
                .episode
                .allowance()
                .map_or(self.deadline, |left| left.min(self.deadline))
                .saturating_sub(started.elapsed());
            if left < Duration::from_secs(30) {
                stopped = "the time bound".to_string();
                break;
            }
            let group = &groups[cursor];
            // The read-only first session of a group reconnoiters before an
            // edit session; it doesn't spend one of the group's attempts.
            let is_read = self.policy.read_first && !read_done[cursor];
            if !is_read {
                attempts[cursor] += 1;
            }
            let brief = self.brief(
                prepared,
                &groups,
                cursor,
                attempts[cursor],
                &sessions,
                last_checked.as_ref(),
                is_read,
            );
            let why = if is_read {
                format!(
                    "session {number} reads before editing {} (group {} of {})",
                    group.ids.join(", "),
                    cursor + 1,
                    groups.len()
                )
            } else {
                format!(
                    "session {number} works on {} (group {} of {}, attempt {})",
                    group.ids.join(", "),
                    cursor + 1,
                    groups.len(),
                    attempts[cursor]
                )
            };
            let ran = self
                .session(number, &group.ids, &why, &brief, is_read)
                .await;
            spent += ran.cost_usd.unwrap_or(0.0);
            let lost = matches!(ran.ending, Ending::Transport(_));
            sessions.push(ran);
            let ran = sessions.last().cloned().expect("a session was just pushed");
            if lost {
                stopped = format!("session {number} lost its provider");
                break;
            }
            let checked = if self.policy.checks {
                let file = format!(
                    "verification/checks-microluna-{}-{number}.json",
                    self.dispatch()
                );
                let mut subject = subject.clone();
                if let Some(live) = &mut subject.live {
                    live.report = Some(ran.summary());
                }
                let (_, report) =
                    checks::check_subject_as(&subject, &self.workdir, &dir, &self.recorder, &file)
                        .await;
                Checked::from_report(&report, group)
            } else {
                Checked::default()
            };
            let (chosen, record, usd) = self
                .decide(
                    prepared,
                    group,
                    &groups[cursor + 1..],
                    &ran,
                    &checked,
                    attempts[cursor],
                )
                .await;
            spent += usd;
            if is_read {
                read_done[cursor] = true;
            }
            let to = match chosen {
                Move::Retry => {
                    format!("{AGENT} session {} on {}", number + 1, group.ids.join(", "))
                }
                Move::Next | Move::Stuck if cursor + 1 < groups.len() => format!(
                    "{AGENT} session {} on {}",
                    number + 1,
                    groups[cursor + 1].ids.join(", ")
                ),
                _ => "the host's closing checks".to_string(),
            };
            let trigger = format!(
                "Jev chose {}{}{}",
                record["jev"]["picked"].as_str().unwrap_or("nothing"),
                record["overridden"]
                    .as_str()
                    .map(|why| format!(" and code settled on {} because {why}", chosen.word()))
                    .unwrap_or_default(),
                if checked.states.is_empty() {
                    String::new()
                } else {
                    format!("; the checks say {}", checked.lines().join("; "))
                }
            );
            let mut handoff = record.clone();
            handoff["from"] = json!(format!(
                "{AGENT} session {number} on {}",
                group.ids.join(", ")
            ));
            handoff["to"] = json!(to);
            handoff["trigger"] = json!(trigger);
            handoff["pattern"] = json!("microluna-requirements");
            handoff["action"] = json!(chosen.word());
            self.recorder.push(
                Step::said(
                    Source::System,
                    &format!(
                        "After session {number} on {}: {}.",
                        group.ids.join(", "),
                        chosen.word()
                    ),
                )
                .noting(crate::handoff::KEY, handoff),
            );
            crate::say::line(&format!(
                "  microluna ▸ after session {number}: {} ({trigger})",
                chosen.word()
            ));
            moves.push(record);
            last_checked = Some(checked);
            match chosen {
                Move::Retry => {}
                Move::Next | Move::Stuck => cursor += 1,
                Move::Done => {
                    stopped = format!("Jev judged the task done after session {number}");
                    break;
                }
            }
        }
        (sessions, moves, stopped)
    }

    /// The suite loop: `accept.define` writes and freezes an acceptance
    /// suite, then edit sessions run until it's green or a bound is hit.
    ///
    /// With `overlap_suite`, the first edit session works in the workspace
    /// while the suite is written and proven red on a snapshot. With
    /// `parallel_edits` above 1, red tests Jev reads as independent run in
    /// separate copies at once and are merged back. With `handoff_jev`,
    /// Jev picks the move after each round and code keeps the last word.
    /// With `final_guard`, a green suite is checked once against the
    /// task's own visible tests.
    ///
    /// `Err` with the sessions already run when there's no transport, the
    /// suite has no tests, or the frozen suite can't reproduce its proof,
    /// so the caller falls back to the requirements loop after them.
    #[allow(clippy::too_many_lines)]
    async fn suite_loop(&self, prepared: &Prepared) -> Result<Looped, Vec<Ran>> {
        let Ok(wire) = &self.wire else {
            return Err(Vec::new());
        };
        let started = Instant::now();
        let started_at = atif::now_ms();
        let time_left = || {
            self.episode
                .allowance()
                .map_or(self.deadline, |left| left.min(self.deadline))
                .saturating_sub(started.elapsed())
        };
        let base = self
            .artifacts
            .parent()
            .map_or_else(|| self.artifacts.clone(), Path::to_path_buf);
        let suite_dir = base.join(format!("accept-suite-{}", self.dispatch()));
        let task = crate::accept::Task {
            title: prepared.title.clone(),
            instruction: prepared.instruction.clone(),
        };
        let everything = Group {
            ids: prepared
                .requirements
                .requirements
                .iter()
                .map(|r| r.id.clone())
                .collect(),
            lines: Vec::new(),
        };
        let evidence = evidence_for(prepared, &everything, self.policy.evidence_chars);
        let key = format!("microluna-{}", &sha256(&prepared.instruction)[..16]);
        let written_by = self.policy.suite_writer.as_ref();
        let mut edit_evidence = evidence.clone();
        if let Some(w) = written_by.filter(|w| w.discover) {
            let defended = if w.general {
                crate::accept::defended_choices_general(&self.workdir)
            } else {
                crate::accept::defended_choices(&self.workdir)
            };
            if !defended.is_empty() {
                edit_evidence.insert(0, crate::accept::defended_evidence(&defended, w.general));
            }
        }
        let writer = crate::accept::MicrolunaWriter {
            transport: wire,
            config: Config {
                model: self.model.clone(),
                effort: written_by
                    .and_then(|w| w.effort.clone())
                    .or_else(|| self.effort.clone()),
                max_turns: written_by.map_or(self.policy.session_turns.max(40), |w| w.turns),
                cache_key: format!("{key}-writer"),
                deadline: Some(Duration::from_secs(self.policy.session_sec).min(time_left())),
                orient_effort: None,
                parallel_tools: self.policy.parallel_tools,
            },
            isolation: self.isolation,
            traces: Some(self.artifacts.clone()),
            echo: false,
        };
        let confine = match self.isolation {
            Isolation::TaskContainer => crate::accept::Confine::TaskContainer,
            Isolation::ReadOnly => crate::accept::Confine::ReadOnly,
            Isolation::Boundary => crate::accept::Confine::Writing,
        };
        let runner = crate::accept::Local {
            confine,
            test_sec: 120,
            jobs: self.policy.test_jobs as usize,
        };
        let general = written_by.is_some_and(|w| w.general);
        // A snapshot of the untouched workspace for the proof, so the first
        // edit session can start in the workspace at once.
        let base_copy = if self.policy.overlap_suite || self.policy.gap_rounds > 0 {
            let dir = scratch("accept-base");
            if !parallel::copyable(&self.workdir) {
                crate::say::line(
                    "  microluna ▸ the workspace is too large to snapshot, so the suite comes first",
                );
                None
            } else {
                match crate::handoff::copy_tree(&self.workdir, &dir) {
                    Ok(()) => Some(dir),
                    Err(error) => {
                        crate::say::line(&format!(
                            "  microluna ▸ no snapshot ({error}), so the suite comes first"
                        ));
                        None
                    }
                }
            }
        } else {
            None
        };
        let _cleanup = Cleanup(base_copy.clone());
        let snapshot = base_copy.clone().filter(|_| self.policy.overlap_suite);
        let real = self.workdir.display().to_string();
        let snapshot_note = |snap: &Path| {
            format!(
                "a snapshot of the task's workspace at {snap}, copied from {real} before any \
                 edit; your commands can read it but you must not change it. Another session \
                 is editing {real} while you write, so read the snapshot, not {real}. In tests, \
                 name the task's paths as the task states them: the host runs the tests \
                 against the snapshot while it proves them red, and against {real} after",
                snap = snap.display()
            )
        };
        let inputs = crate::accept::Inputs {
            task: &task,
            requirements: &prepared.requirements,
            evidence: &evidence,
            workspace: snapshot.as_deref().unwrap_or(&self.workdir),
            suite_dir: &suite_dir,
            workspace_note: match &snapshot {
                Some(snap) => snapshot_note(snap),
                None => format!(
                    "the task's workspace, {real}, which your commands can read but you must not \
                     change"
                ),
            },
            target: snapshot.as_ref().map(|_| self.workdir.as_path()),
        };
        let mut options =
            written_by.map_or_else(crate::accept::Options::default, SuiteWriter::options);
        options.test_jobs = self.policy.test_jobs as usize;
        crate::say::line(if snapshot.is_some() {
            "  microluna ▸ writing the acceptance suite on a snapshot while session 1 starts"
        } else {
            "  microluna ▸ writing the acceptance suite before any fix"
        });
        let define_started = atif::now_ms();
        let define = async {
            let mut suite = crate::accept::define(
                &inputs,
                &writer,
                &runner,
                &prepared.jev,
                &self.recorder,
                &options,
            )
            .await;
            // With gap_overlap, the first gap round runs as soon as the
            // suite is frozen with gaps, while session 1 still works.
            let mut overlapped = Value::Null;
            if self.policy.gap_overlap
                && !suite.tests.is_empty()
                && !suite.gaps.is_empty()
                && let Some(base_dir) = snapshot.as_deref()
            {
                let open: Vec<String> = suite.gaps.iter().map(|g| g.requirement.clone()).collect();
                crate::say::line(&format!(
                    "  microluna ▸ the suite is frozen partial (open: {}), so gap round 1 writes \
                     deciding tests beside session 1",
                    open.join(", ")
                ));
                let gap_inputs = crate::accept::Inputs {
                    task: &task,
                    requirements: &prepared.requirements,
                    evidence: &evidence,
                    workspace: base_dir,
                    suite_dir: &suite_dir,
                    workspace_note: snapshot_note(base_dir),
                    target: Some(&self.workdir),
                };
                let gap_started = atif::now_ms();
                let (tests_before, usd_before) = (suite.tests.len(), suite.writer_usd);
                suite = crate::accept::extend(
                    &suite,
                    &gap_inputs,
                    &writer,
                    &runner,
                    &self.recorder,
                    &options,
                    1,
                )
                .await;
                overlapped = json!({
                    "kind": "gap",
                    "round": 1,
                    "overlapped": true,
                    "open": open,
                    "added": suite.tests.len() - tests_before,
                    "gaps": suite.gaps,
                    "status": suite.status,
                    "digest": suite.digest,
                    "writer_usd": suite.writer_usd - usd_before,
                    "start_ms": gap_started.saturating_sub(started_at),
                    "end_ms": atif::now_ms().saturating_sub(started_at),
                });
            }
            (suite, atif::now_ms(), overlapped)
        };
        let ((mut suite, define_ended, overlapped_gap), early) = if snapshot.is_some() {
            let (defined, ran) =
                futures_util::future::join(define, self.early_session(prepared, &edit_evidence))
                    .await;
            (defined, Some(ran))
        } else {
            (define.await, None)
        };
        let mut tracks = vec![parallel::Track {
            label: "accept.define".to_string(),
            kind: "define".to_string(),
            batch: "suite".to_string(),
            group: Some(format!("{} tests", suite.tests.len())),
            workspace: snapshot.as_ref().map(|s| s.display().to_string()),
            start_ms: define_started.saturating_sub(started_at),
            end_ms: define_ended.saturating_sub(started_at),
            turns: suite.rounds.iter().map(|r| r.writer.turns).sum(),
            input_tokens: 0,
            cached_tokens: 0,
            read_turns: 0,
            cost_usd: suite.writer_usd + suite.jev_usd,
        }];
        tracks.extend(writer_tracks(&suite, started_at));
        crate::say::line(&format!(
            "  microluna ▸ {} in {:.1}s",
            suite.headline(),
            suite.milliseconds as f64 / 1000.0
        ));
        let mut moves = vec![json!({
            "kind": "suite",
            "headline": suite.headline(),
            "status": suite.status,
            "digest": suite.digest,
            "tests": suite.tests.len(),
            "rejected": suite.rejected.len(),
            "rounds": suite.rounds.len(),
            "gaps": suite.gaps,
            "writer_usd": suite.writer_usd,
            "writer_usage": {
                "input": suite.rounds.iter().map(|r| r.writer.input_tokens).sum::<u64>(),
                "cached": suite.rounds.iter().map(|r| r.writer.cached_tokens).sum::<u64>(),
                "output": suite.rounds.iter().map(|r| r.writer.output_tokens).sum::<u64>(),
            },
            "jev_usd": suite.jev_usd,
            "milliseconds": suite.milliseconds,
            "on_snapshot": snapshot.is_some(),
            "record": crate::accept::AcceptanceSuite::record_path(&suite_dir),
        })];
        let gap_overlapped = !overlapped_gap.is_null();
        if gap_overlapped {
            crate::say::line(&format!(
                "  microluna ▸ gap round 1 beside session 1: {} new test{}",
                overlapped_gap["added"],
                if overlapped_gap["added"] == 1 {
                    ""
                } else {
                    "s"
                }
            ));
            moves.push(overlapped_gap);
        }
        let mut sessions: Vec<Ran> = early.into_iter().collect();
        let mut spent = suite.writer_usd
            + suite.jev_usd
            + sessions.iter().filter_map(|r| r.cost_usd).sum::<f64>();
        if suite.tests.is_empty() {
            crate::say::line(
                "  microluna ▸ the suite has no tests, so the requirements loop runs instead",
            );
            return Err(sessions);
        }
        // The frozen suite must fail the way its red-first proof did. With
        // a snapshot, that check runs there, since session 1 has already
        // changed the workspace.
        if let Some(snap) = &snapshot {
            let rebased = crate::accept::Rebased {
                inner: &runner,
                real: self.workdir.clone(),
                snapshot: snap.clone(),
                test_sec: 120,
                jobs: self.policy.test_jobs as usize,
            };
            let start = crate::accept::run(
                &suite,
                snap,
                &rebased,
                Some(&self.recorder),
                "start (snapshot)",
            )
            .await;
            if let Ok(start) = &start
                && let Some(differ) = unreproduced(&suite, start)
            {
                crate::say::line(&format!(
                    "  microluna ▸ the frozen suite doesn't reproduce its proof on the snapshot \
                     ({differ} of {} tests differ), so the requirements loop runs instead",
                    start.tests.len()
                ));
                return Err(sessions);
            }
        }
        let mut best = 0usize;
        let mut since_progress = 0u32;
        let mut number = u32::try_from(sessions.len()).unwrap_or(u32::MAX);
        let mut rounds_blocked: Vec<bool> = sessions.iter().map(Ran::blocked).collect();
        let mut last_round: Vec<u32> = sessions.iter().map(|r| r.number).collect();
        let mut requeued: Vec<String> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        let mut merges: Vec<Value> = Vec::new();
        let mut last_signature: Option<(u32, String)> = None;
        let mut contradicted_rounds = 0u32;
        let mut round = 0u32;
        let mut guarded = false;
        let mut guard_failure: Option<String> = None;
        let mut audited = false;
        let mut audit: Option<String> = None;
        let mut latest: Option<crate::accept::RunResult> = None;
        let mut first_run = true;
        let mut gaps_left = self
            .policy
            .gap_rounds
            .saturating_sub(u32::from(gap_overlapped));
        let mut gap_number = u32::from(gap_overlapped);
        let mut last_run: Option<(String, String, crate::accept::RunResult)> = None;
        let mut advisory_red: Vec<String> = Vec::new();
        let stopped;
        loop {
            let label = if number == 0 {
                "start".to_string()
            } else {
                format!("after session {number}")
            };
            // With fast runs, an unchanged workspace reuses the last run.
            let tree = self
                .policy
                .fast_runs
                .then(|| crate::accept::digest_of(&parallel::tree(&self.workdir)));
            let reused = match (&tree, &last_run) {
                (Some(now), Some((then, digest, held)))
                    if now == then && *digest == suite.digest =>
                {
                    Some(held.clone())
                }
                _ => None,
            };
            let result = match reused {
                Some(mut held) => {
                    held.label =
                        format!("{label} (the workspace is unchanged, so the last run stands)");
                    held
                }
                None => {
                    let previous = last_run
                        .as_ref()
                        .filter(|(_, digest, _)| *digest == suite.digest)
                        .map(|(_, _, held)| held.clone());
                    match self
                        .run_suite(&suite, &runner, &label, previous.as_ref())
                        .await
                    {
                        Ok(result) => result,
                        Err(tampered) => {
                            stopped = format!("the suite can't be trusted: {tampered}");
                            break;
                        }
                    }
                }
            };
            if let Some(tree) = tree {
                last_run = Some((tree, suite.digest.clone(), result.clone()));
            }
            crate::say::line(&format!(
                "  microluna ▸ suite {label}: {} of {} green",
                result.passed, result.total
            ));
            // The frozen suite must fail the way its red-first proof did.
            // When most red tests now fail differently (a missing harness
            // file, a shell error), no edit can turn them green, so the
            // requirements loop takes over instead.
            if first_run
                && number == 0
                && let Some(differ) = unreproduced(&suite, &result)
            {
                crate::say::line(&format!(
                    "  microluna ▸ the frozen suite doesn't reproduce its proof ({differ} of {} tests \
                     differ), so the requirements loop runs instead",
                    result.tests.len()
                ));
                return Err(sessions);
            }
            first_run = false;
            // With advisory guards, a guard an edit turned red doesn't hold
            // the loop: it decides on the other tests, and each later brief
            // names the guard.
            let advisory = if self.policy.advisory_guards {
                red_guards(&suite, &result)
            } else {
                Vec::new()
            };
            let result = if advisory.is_empty() {
                result
            } else {
                crate::say::line(&format!(
                    "  microluna ▸ {} passed on the untouched workspace and {} red now; the loop \
                     doesn't count {}",
                    advisory.join(", "),
                    if advisory.len() == 1 { "is" } else { "are" },
                    if advisory.len() == 1 { "it" } else { "them" }
                ));
                notes.retain(|n| !n.contains("passed on the untouched workspace and"));
                notes.insert(0, guard_note(&advisory));
                without_tests(&suite, &result, &advisory)
            };
            latest = Some(result.clone());
            advisory_red.clone_from(&advisory);
            moves.push(json!({
                "kind": "run",
                "after_session": number,
                "milliseconds": result.milliseconds,
                "passed": result.passed,
                "total": result.total,
                "green": result.green,
                "complete": result.complete,
                "red": result.red_requirements(),
            }));
            if !advisory.is_empty()
                && let Some(last) = moves.last_mut()
            {
                last["advisory_guards"] = json!(advisory);
            }
            let partial = suite.status != crate::accept::Status::Accepted || !result.complete;
            if result.green
                && partial
                && gaps_left > 0
                && spent < self.policy.spend_usd
                && time_left() >= Duration::from_secs(60)
                && let Some(base_dir) = &base_copy
            {
                // A partial suite's green isn't done: write deciding tests
                // for its gaps on the untouched snapshot, then resume.
                gaps_left -= 1;
                gap_number += 1;
                let open: Vec<String> = suite.gaps.iter().map(|g| g.requirement.clone()).collect();
                crate::say::line(&format!(
                    "  microluna ▸ the suite is green but partial (open: {}), so gap round \
                     {gap_number} writes deciding tests on the snapshot",
                    open.join(", ")
                ));
                let gap_inputs = crate::accept::Inputs {
                    task: &task,
                    requirements: &prepared.requirements,
                    evidence: &evidence,
                    workspace: base_dir,
                    suite_dir: &suite_dir,
                    workspace_note: snapshot_note(base_dir),
                    target: Some(&self.workdir),
                };
                let gap_started = atif::now_ms();
                let (tests_before, usd_before) = (suite.tests.len(), suite.writer_usd);
                suite = crate::accept::extend(
                    &suite,
                    &gap_inputs,
                    &writer,
                    &runner,
                    &self.recorder,
                    &options,
                    gap_number,
                )
                .await;
                let added = suite.tests.len() - tests_before;
                spent += suite.writer_usd - usd_before;
                tracks.push(parallel::Track {
                    label: format!("gap round {gap_number}"),
                    kind: "gap".to_string(),
                    batch: format!("gap round {gap_number}"),
                    group: Some(open.join(", ")),
                    workspace: Some(base_dir.display().to_string()),
                    start_ms: gap_started.saturating_sub(started_at),
                    end_ms: atif::now_ms().saturating_sub(started_at),
                    turns: suite.rounds.last().map_or(0, |r| r.writer.turns),
                    input_tokens: suite.rounds.last().map_or(0, |r| r.writer.input_tokens),
                    cached_tokens: suite.rounds.last().map_or(0, |r| r.writer.cached_tokens),
                    read_turns: 0,
                    cost_usd: suite.writer_usd - usd_before,
                });
                moves.push(json!({
                    "kind": "gap",
                    "round": gap_number,
                    "open": open,
                    "added": added,
                    "gaps": suite.gaps,
                    "status": suite.status,
                    "digest": suite.digest,
                    "writer_usd": suite.writer_usd - usd_before,
                }));
                crate::say::line(&format!(
                    "  microluna ▸ gap round {gap_number}: {added} new test{}; {}",
                    if added == 1 { "" } else { "s" },
                    suite.headline()
                ));
                if added > 0 || !suite.gaps.iter().any(|g| open.contains(&g.requirement)) {
                    last_run = None;
                    last_signature = None;
                    first_run = false;
                    continue;
                }
            }
            let mut partial_note = if partial {
                format!(
                    ", but the suite is partial (open: {})",
                    suite
                        .gaps
                        .iter()
                        .map(|g| g.requirement.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                String::new()
            };
            if !advisory_red.is_empty() {
                partial_note.push_str(&format!(
                    "; the guard{} {} passed on the untouched workspace and {} red, not counted",
                    if advisory_red.len() == 1 { "" } else { "s" },
                    advisory_red.join(", "),
                    if advisory_red.len() == 1 { "is" } else { "are" }
                ));
            }
            // Before stopping on green, join the evidence once; a partial
            // suite or a doubtful done gets one audit session.
            if result.green && self.policy.close_audit && !audited {
                audited = true;
                let (done, record, usd) = self
                    .joined_close(prepared, &suite, &result, &sessions, base_copy.as_deref())
                    .await;
                spent += usd;
                let weak: Vec<String> = record["requirements"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|r| r["p"].as_f64().is_some_and(|p| p < 0.5))
                    .map(|r| {
                        format!(
                            "{} ({:.2})",
                            r["text"].as_str().unwrap_or_default(),
                            r["p"].as_f64().unwrap_or_default()
                        )
                    })
                    .collect();
                let missing =
                    missing_outputs(&prepared.instruction, &self.workdir, base_copy.as_deref());
                let doubtful = partial
                    || !missing.is_empty()
                    || !advisory_red.is_empty()
                    || done.is_none_or(|p| p < CLOSE_MIN);
                crate::say::line(&format!(
                    "  microluna ▸ joined close: done {}{}",
                    done.map_or("unanswered".to_string(), |p| format!("p={p:.2}")),
                    if doubtful {
                        ", so one audit session runs"
                    } else {
                        ""
                    }
                ));
                moves.push(record);
                if doubtful
                    && number < self.policy.max_sessions
                    && spent < self.policy.spend_usd
                    && time_left() >= Duration::from_secs(60)
                {
                    audit = Some(format!(
                        "The acceptance suite is green, but the host doubts the task is done \
                         (p={}){partial_note}. {}{}{}",
                        done.map_or("unanswered".to_string(), |p| format!("{p:.2}")),
                        audit_rule(general, !advisory_red.is_empty()),
                        if weak.is_empty() {
                            String::new()
                        } else {
                            format!(" The weakest requirements: {}.", weak.join("; "))
                        },
                        missing_note(&missing)
                    ));
                }
            }
            if result.green && audit.is_none() {
                // Stop the moment the suite is green; with a final guard,
                // the task's own visible tests run once first.
                if self.policy.final_guard && !guarded {
                    guarded = true;
                    let guard_started = atif::now_ms();
                    let guard = self.guard(&runner).await;
                    let guard_ended = atif::now_ms();
                    if let Some(guard) = &guard {
                        tracks.push(parallel::Track {
                            label: "final guard".to_string(),
                            kind: "guard".to_string(),
                            batch: "guard".to_string(),
                            group: None,
                            workspace: None,
                            start_ms: guard_started.saturating_sub(started_at),
                            end_ms: guard_ended.saturating_sub(started_at),
                            turns: 0,
                            input_tokens: 0,
                            cached_tokens: 0,
                            read_turns: 0,
                            cost_usd: 0.0,
                        });
                        moves.push(json!({
                            "kind": "guard",
                            "after_session": number,
                            "green": guard.green,
                            "exit": guard.exit,
                            "output": crate::judge::clip_tail(&guard.output, 1_500),
                        }));
                        crate::say::line(&format!(
                            "  microluna ▸ final guard: the task's own tests {}",
                            if guard.green { "pass" } else { "fail" }
                        ));
                    }
                    match guard {
                        Some(guard)
                            if !guard.green
                                && number < self.policy.max_sessions
                                && spent < self.policy.spend_usd
                                && time_left() >= Duration::from_secs(60) =>
                        {
                            guard_failure = Some(guard.output);
                        }
                        Some(guard) if !guard.green => {
                            stopped = format!(
                                "the acceptance suite is green after session {number} ({} of \
                                 {}){partial_note}, but the task's own tests fail and no bound is \
                                 left for a session",
                                result.passed, result.total
                            );
                            break;
                        }
                        Some(_) => {
                            stopped = format!(
                                "the acceptance suite is green after session {number} ({} of \
                                 {}){partial_note}, and the task's own tests pass",
                                result.passed, result.total
                            );
                            break;
                        }
                        None => {
                            stopped = format!(
                                "the acceptance suite is green after session {number} ({} of \
                                 {}){partial_note}; the task has no visible tests to guard with",
                                result.passed, result.total
                            );
                            break;
                        }
                    }
                } else {
                    stopped = format!(
                        "the acceptance suite is green after session {number} ({} of {}){partial_note}",
                        result.passed, result.total
                    );
                    break;
                }
            }
            if result.passed > best {
                best = result.passed;
                since_progress = 0;
            } else if number > 0 {
                since_progress += 1;
            }
            // The same red tests with the same output after two rounds in a
            // row: another session starts from where the last one did.
            let signature = result.red_lines(&suite, 800).join("\n");
            if self.policy.handoff_jev && !result.green && number > 0 {
                if let Some((then, held)) = &last_signature
                    && *held == signature
                    && number > *then
                {
                    stopped = format!(
                        "the same red tests failed the same way after sessions {then} and {number}"
                    );
                    break;
                }
                last_signature = Some((number, signature));
            }
            if number >= self.policy.max_sessions {
                stopped = format!("the bound of {} sessions", self.policy.max_sessions);
                break;
            }
            if spent >= self.policy.spend_usd {
                stopped = format!(
                    "the spend bound of ${:.2}: ${spent:.4} spent",
                    self.policy.spend_usd
                );
                break;
            }
            if time_left() < Duration::from_secs(60) {
                stopped = "the time bound".to_string();
                break;
            }
            // Act on the typed causes the last round's sessions gave.
            let ended: Vec<&Ran> = sessions
                .iter()
                .filter(|r| last_round.contains(&r.number))
                .collect();
            let mut contradicted = false;
            for ran in &ended {
                match ran.cause() {
                    microluna::Cause::HarnessBroken => {
                        let broken: Vec<String> = result
                            .tests
                            .iter()
                            .filter(|t| !t.green)
                            .filter_map(|t| {
                                crate::accept::verify::broken_reason(t)
                                    .map(|why| format!("{} ({why})", t.id))
                            })
                            .collect();
                        if !broken.is_empty() {
                            crate::say::line(&format!(
                                "  microluna ▸ session {} found the harness broken ({}), so the \
                                 requirements loop runs instead",
                                ran.number,
                                broken.join(", ")
                            ));
                            return Err(sessions);
                        }
                        notes.push(format!(
                            "Session {} reported the test harness broken, but the host's run \
                             shows every red test reaching its assertions: read each red test's \
                             output again for the rule it checks.",
                            ran.number
                        ));
                    }
                    microluna::Cause::TestContradictsTask => {
                        contradicted = true;
                        notes.push(format!(
                            "Session {} says a test contradicts the task: {} The suite is \
                             frozen. Follow the task; if a test truly contradicts it, meet the \
                             task, call finish with status blocked and cause \
                             test_contradicts_task, and name the test.",
                            ran.number,
                            crate::judge::clip(&ran.summary(), 300)
                        ));
                    }
                    microluna::Cause::MissingTool => notes.push(format!(
                        "Session {} found a program the work needs missing: {} Don't try it \
                         again; use what the workspace has.",
                        ran.number,
                        crate::judge::clip(&ran.summary(), 300)
                    )),
                    _ => {}
                }
            }
            contradicted_rounds = if contradicted {
                contradicted_rounds + 1
            } else {
                0
            };
            if contradicted_rounds >= 2 {
                stopped = format!(
                    "two rounds in a row say the suite contradicts the task (sessions up to \
                     {number})"
                );
                break;
            }
            notes.truncate(6);
            // Jev's move after the last round; code keeps the last word.
            if self.policy.handoff_jev && number > 0 && guard_failure.is_none() && audit.is_none() {
                let (chosen, record, usd) = self
                    .suite_move(prepared, &suite, &result, &ended, since_progress)
                    .await;
                spent += usd;
                let focus: Vec<String> = ended.iter().flat_map(|r| r.focus.clone()).collect();
                self.recorder.push(
                    Step::said(
                        Source::System,
                        &format!("After session {number}: {}.", chosen.word()),
                    )
                    .noting(
                        crate::handoff::KEY,
                        json!({
                            "from": format!("{AGENT} session {number}"),
                            "to": match chosen {
                                Move::Stuck => "the host's closing checks".to_string(),
                                _ => format!("{AGENT} session {}", number + 1),
                            },
                            "trigger": format!(
                                "Jev chose {}{}; the suite is {} of {} green",
                                record["jev"]["picked"].as_str().unwrap_or("nothing"),
                                record["overridden"].as_str().map(|w| format!(" and code settled on {} because {w}", chosen.word())).unwrap_or_default(),
                                result.passed,
                                result.total
                            ),
                            "pattern": "microluna-suite",
                            "action": chosen.word(),
                        }),
                    ),
                );
                crate::say::line(&format!(
                    "  microluna ▸ after session {number}: {}{}",
                    chosen.word(),
                    record["overridden"]
                        .as_str()
                        .map(|w| format!(" ({w})"))
                        .unwrap_or_default()
                ));
                moves.push(record);
                match chosen {
                    Move::Stuck => {
                        stopped = format!("Jev judged the loop stuck after session {number}");
                        break;
                    }
                    // Move on: the tests the last round didn't work on lead.
                    Move::Next => {
                        requeued = result
                            .tests
                            .iter()
                            .filter(|t| {
                                !t.green && !t.requirements.iter().any(|r| focus.contains(r))
                            })
                            .map(|t| t.id.clone())
                            .collect();
                    }
                    Move::Retry | Move::Done => {}
                }
            }
            // The next round: several sessions at once on independent red
            // tests, or one on all of them.
            let red: Vec<&crate::accept::TestRun> =
                result.tests.iter().filter(|t| !t.green).collect();
            let room = self.policy.max_sessions.saturating_sub(number);
            let planned = if guard_failure.is_none()
                && audit.is_none()
                && self.policy.parallel_edits > 1
                && red.len() >= 2
                && room >= 2
                && parallel::copyable(&self.workdir)
            {
                let (lanes, record, usd) =
                    self.plan_lanes(prepared, &suite, &result, &requeued).await;
                spent += usd;
                moves.push(record);
                lanes
            } else {
                Vec::new()
            };
            round += 1;
            let before = number;
            if planned.len() >= 2 {
                let lanes: Vec<parallel::Lane> = planned
                    .into_iter()
                    .take(room.min(self.policy.parallel_edits) as usize)
                    .collect();
                let (ran, merged) = self
                    .parallel_round(
                        prepared,
                        &suite,
                        &result,
                        &lanes,
                        &edit_evidence,
                        &sessions,
                        &notes,
                        round,
                        number,
                        best,
                    )
                    .await;
                requeued = merged["requeued"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                notes.clear();
                if let Some(note) = merged["note"].as_str() {
                    notes.push(note.to_string());
                }
                merges.push(merged.clone());
                moves.push(json!({ "kind": "merge", "round": round, "merge": merged }));
                number += u32::try_from(ran.len()).unwrap_or(0);
                spent += ran.iter().filter_map(|r| r.cost_usd).sum::<f64>();
                sessions.extend(ran);
            } else {
                number += 1;
                let brief = match (&guard_failure, &audit) {
                    (Some(output), _) => {
                        self.guard_brief(prepared, &suite, &edit_evidence, number, output)
                    }
                    (None, Some(note)) => {
                        let mut brief = self.suite_brief(
                            prepared,
                            &suite,
                            &result,
                            &edit_evidence,
                            &sessions,
                            &notes,
                            number,
                            best,
                            0,
                        );
                        brief.state.insert(1, note.clone());
                        brief
                    }
                    (None, None) => self.suite_brief(
                        prepared,
                        &suite,
                        &result,
                        &edit_evidence,
                        &sessions,
                        &notes,
                        number,
                        best,
                        since_progress,
                    ),
                };
                let red_ids = result.red_requirements();
                let (focus, why) = match (&guard_failure, &audit) {
                    (None, Some(_)) => (
                        vec!["an audit of the whole task".to_string()],
                        format!(
                            "session {number} audits the work: the suite is green, but the \
                             joined evidence doesn't show the task done"
                        ),
                    ),
                    (Some(_), _) => (
                        vec!["the task's own tests".to_string()],
                        format!(
                            "session {number} fixes what the task's own tests show, with the \
                             acceptance suite green"
                        ),
                    ),
                    (None, None) => (
                        red_ids.clone(),
                        format!(
                            "session {number} works toward a green suite ({} of {} green; red: {})",
                            result.passed,
                            result.total,
                            red_ids.join(", ")
                        ),
                    ),
                };
                guard_failure = None;
                audit = None;
                notes.clear();
                let ran = self
                    .session_at(
                        number,
                        &focus,
                        &why,
                        &brief,
                        false,
                        Place {
                            batch: format!("round {round}"),
                            ..Place::default()
                        },
                    )
                    .await;
                spent += ran.cost_usd.unwrap_or(0.0);
                sessions.push(ran);
            }
            last_round = (before + 1..=number).collect();
            let this_round: Vec<&Ran> = sessions
                .iter()
                .filter(|r| last_round.contains(&r.number))
                .collect();
            if this_round
                .iter()
                .any(|r| matches!(r.ending, Ending::Transport(_)))
            {
                stopped = format!("session {number} lost its provider");
                break;
            }
            rounds_blocked.push(!this_round.is_empty() && this_round.iter().all(|r| r.blocked()));
            // Two blocked rounds in a row: the next would be blocked by the
            // same thing.
            if rounds_blocked.len() >= 2
                && rounds_blocked[rounds_blocked.len() - 2..] == [true, true]
            {
                stopped = format!(
                    "the last two rounds were blocked, up to session {number}: {}",
                    crate::judge::clip(&sessions[sessions.len() - 1].summary(), 300)
                );
                break;
            }
        }
        // A loop that stops red on a suite its sessions dispute, or with an
        // output the task names still missing, gets one audit session on
        // the task itself before it ends.
        let mut stopped = stopped;
        let missing = missing_outputs(&prepared.instruction, &self.workdir, base_copy.as_deref());
        let disputed = sessions
            .iter()
            .rev()
            .take(2)
            .any(|r| r.cause() == microluna::Cause::TestContradictsTask);
        if let Some(red) = latest.as_ref().filter(|r| !r.green)
            && self.policy.close_audit
            && !audited
            && (disputed || !missing.is_empty())
            && number < self.policy.max_sessions
            && spent < self.policy.spend_usd
            && time_left() >= Duration::from_secs(60)
        {
            let (done, record, usd) = self
                .joined_close(prepared, &suite, red, &sessions, base_copy.as_deref())
                .await;
            spent += usd;
            moves.push(record);
            number += 1;
            let mut brief = self.suite_brief(
                prepared,
                &suite,
                red,
                &edit_evidence,
                &sessions,
                &notes,
                number,
                red.passed,
                0,
            );
            brief.state.insert(
                1,
                format!(
                    "The loop stopped with the suite red: {stopped}.{} Jev reads the task as done \
                     with p={}. Audit the work against the task itself: where a red test \
                     contradicts the task's words, follow the task, not the test; meet every \
                     requirement the task states, and produce every output it names, the way it \
                     names them. Then run the suite and the task's own example, and call finish, \
                     naming any test you believe is wrong.{}",
                    if disputed {
                        " The last sessions reported that a test contradicts the task."
                    } else {
                        ""
                    },
                    done.map_or("unanswered".to_string(), |p| format!("{p:.2}")),
                    missing_note(&missing)
                ),
            );
            crate::say::line(&format!(
                "  microluna ▸ the loop stopped red{}, so session {number} audits the task",
                if missing.is_empty() {
                    String::new()
                } else {
                    format!(" with {} missing", missing.join(", "))
                }
            ));
            let ran = self
                .session_at(
                    number,
                    &["an audit of the whole task".to_string()],
                    &format!(
                        "session {number} audits the task after the loop stopped with the suite red"
                    ),
                    &brief,
                    false,
                    Place {
                        batch: "audit".to_string(),
                        ..Place::default()
                    },
                )
                .await;
            sessions.push(ran);
            if let Ok(after) = crate::accept::run(
                &suite,
                &self.workdir,
                &runner,
                Some(&self.recorder),
                &format!("after session {number}"),
            )
            .await
            {
                moves.push(json!({
                    "kind": "run",
                    "after_session": number,
                    "milliseconds": after.milliseconds,
                    "passed": after.passed,
                    "total": after.total,
                    "green": after.green,
                    "complete": after.complete,
                    "red": after.red_requirements(),
                }));
                let left =
                    missing_outputs(&prepared.instruction, &self.workdir, base_copy.as_deref());
                stopped = format!(
                    "{stopped}; then audit session {number} left the suite {} of {}{}",
                    after.passed,
                    after.total,
                    if left.is_empty() {
                        String::new()
                    } else {
                        format!(" with {} still missing", left.join(", "))
                    }
                );
            }
        }
        tracks.extend(sessions.iter().map(|r| r.track(started_at)));
        tracks.sort_by_key(|t| (t.start_ms, t.end_ms));
        let wall = millis(started);
        let mut summary = parallel::summary(&tracks, wall, &merges);
        let runs: Vec<&Value> = moves.iter().filter(|m| m["kind"] == "run").collect();
        summary["suite_runs"] = json!({
            "count": runs.len(),
            "milliseconds": runs.iter().filter_map(|m| m["milliseconds"].as_u64()).sum::<u64>(),
        });
        summary["cost_usd"] = json!(spent);
        crate::say::line(&format!(
            "  microluna ▸ parallel: {}",
            parallel::headline(&summary)
        ));
        self.recorder.push(
            Step::said(
                Source::System,
                &format!("Parallel sessions: {}.", parallel::headline(&summary)),
            )
            .noting(parallel::SUMMARY_EXTENSION, summary.clone()),
        );
        Ok(Looped {
            sessions,
            moves,
            stopped,
            parallel: summary,
        })
    }

    /// The first edit session, in the workspace, while `accept.define`
    /// writes the suite on a snapshot.
    async fn early_session(&self, prepared: &Prepared, evidence: &[Evidence]) -> Ran {
        let general = self.policy.suite_writer.as_ref().is_some_and(|w| w.general);
        let mut guidance = if general {
            EARLY_GUIDANCE_GENERAL
        } else {
            EARLY_GUIDANCE
        }
        .to_string();
        let facts = constraints(&prepared.requirements);
        if !facts.is_empty() {
            guidance.push_str(&format!(
                "\n\nThe task's constraints hold throughout; honor each one exactly:\n\n{}",
                facts.join("\n")
            ));
        }
        let files = parallel::workspace_files(&self.workdir);
        let named = parallel::files_named(&prepared.instruction, &files);
        let mut evidence = evidence.to_vec();
        evidence.extend(self.source_evidence(&self.workdir, &named));
        let brief = Brief {
            task: prepared.instruction.clone(),
            guidance,
            evidence,
            state: vec![format!(
                "Session 1 of at most {}. It runs while the acceptance suite is written.",
                self.policy.max_sessions
            )],
        };
        self.session_at(
            1,
            &["the whole task".to_string()],
            "session 1 starts on the task while the acceptance suite is written, in parallel \
             with accept.define",
            &brief,
            false,
            Place {
                group: Some("the whole task, while the suite is written".to_string()),
                batch: "suite".to_string(),
                alongside: Some("accept.define".to_string()),
                ..Place::default()
            },
        )
        .await
    }

    /// One suite-loop session's brief: the task, the frozen tests and the
    /// evidence, then the state: the suite now, the notes the host keeps,
    /// the red tests' output, and the last sessions.
    #[allow(clippy::too_many_arguments)]
    fn suite_brief(
        &self,
        prepared: &Prepared,
        suite: &crate::accept::AcceptanceSuite,
        result: &crate::accept::RunResult,
        evidence: &[Evidence],
        sessions: &[Ran],
        notes: &[String],
        number: u32,
        best: usize,
        since_progress: u32,
    ) -> Brief {
        let mut state = vec![format!(
            "Session {number} of at most {}. The suite is {} of {} green; the best so far is \
             {best}.",
            self.policy.max_sessions, result.passed, result.total
        )];
        if since_progress >= 3 {
            state.push(format!(
                "The last {since_progress} sessions turned no new test green. Don't repeat \
                 their approach: reread the task and each red test's output for the exact \
                 rule it checks, and change what the earlier sessions left alone."
            ));
        }
        state.extend(notes.iter().cloned());
        state.push(format!(
            "The red tests and their output:\n{}",
            result.red_lines(suite, 800).join("\n")
        ));
        for ran in sessions.iter().rev().take(4).rev() {
            state.push(format!(
                "Session {} ended {}: {}",
                ran.number,
                ran.status(),
                crate::judge::clip(&ran.summary(), 400)
            ));
        }
        if !sessions.is_empty() {
            let changes = crate::delegate::changes(&self.workdir, None);
            state.push(format!(
                "What the workspace shows as changed now:\n{}",
                crate::judge::clip(&changes, 1_500)
            ));
        }
        let mut session_evidence = vec![suite.evidence()];
        session_evidence.extend(evidence.iter().cloned());
        session_evidence.extend(self.source_evidence(
            &self.workdir,
            &Self::red_files(suite, result, &self.workdir),
        ));
        Brief {
            task: prepared.instruction.clone(),
            guidance: self.suite_guidance(prepared),
            evidence: session_evidence,
            state,
        }
    }

    /// Runs the suite on the workspace. With fast runs and a previous run
    /// of the same suite, only the tests that were red run; when they all
    /// pass, the whole suite runs once to confirm.
    async fn run_suite(
        &self,
        suite: &crate::accept::AcceptanceSuite,
        runner: &crate::accept::Local,
        label: &str,
        previous: Option<&crate::accept::RunResult>,
    ) -> Result<crate::accept::RunResult, crate::accept::Tampered> {
        if self.policy.fast_runs
            && let Some(previous) = previous.filter(|p| !p.green)
        {
            let red: Vec<crate::accept::Test> = suite
                .tests
                .iter()
                .filter(|t| previous.tests.iter().any(|r| r.id == t.id && !r.green))
                .cloned()
                .collect();
            if !red.is_empty() && red.len() < suite.tests.len() {
                let only = crate::accept::AcceptanceSuite {
                    tests: red,
                    ..suite.clone()
                };
                let now = crate::accept::run(
                    &only,
                    &self.workdir,
                    runner,
                    Some(&self.recorder),
                    &format!("{label}, the red tests only"),
                )
                .await?;
                if !now.green {
                    let mut tests: Vec<crate::accept::TestRun> = previous
                        .tests
                        .iter()
                        .filter(|r| r.green)
                        .cloned()
                        .chain(now.tests.iter().cloned())
                        .collect();
                    tests.sort_by_key(|t| suite.tests.iter().position(|s| s.id == t.id));
                    let mut combined = crate::accept::RunResult::of(
                        label,
                        &suite.digest,
                        &suite.requirement_ids(),
                        tests,
                        now.milliseconds,
                    );
                    combined.gaps = suite.gaps.iter().map(|g| g.requirement.clone()).collect();
                    combined.complete = combined.green && combined.gaps.is_empty();
                    return Ok(combined);
                }
            }
        }
        crate::accept::run(suite, &self.workdir, runner, Some(&self.recorder), label).await
    }

    /// The workspace's current source files as evidence, within
    /// `prefix_sources` characters: the files in `first` before the rest,
    /// each whole or not at all. It goes last in the stable prefix, since
    /// an edit changes it.
    fn source_evidence(&self, workdir: &Path, first: &[String]) -> Vec<Evidence> {
        if self.policy.prefix_sources == 0 {
            return Vec::new();
        }
        let sources = crate::accept::source_files(workdir, 200);
        let mut ordered: Vec<&String> = first.iter().filter(|f| sources.contains(f)).collect();
        ordered.extend(sources.iter().filter(|f| !first.contains(f)));
        let mut left = self.policy.prefix_sources;
        let mut out = Vec::new();
        for path in ordered {
            let Ok(text) = std::fs::read_to_string(workdir.join(path)) else {
                continue;
            };
            let size = text.chars().count();
            if size > left {
                continue;
            }
            left -= size;
            out.push(Evidence {
                label: format!("The current {path}"),
                text,
            });
            if left < 200 {
                break;
            }
        }
        out
    }

    /// The workspace files the red tests of `result` name, in their
    /// sources or their output.
    fn red_files(
        suite: &crate::accept::AcceptanceSuite,
        result: &crate::accept::RunResult,
        workdir: &Path,
    ) -> Vec<String> {
        let files = parallel::workspace_files(workdir);
        let mut text = String::new();
        for run in result.tests.iter().filter(|t| !t.green) {
            if let Some(test) = suite.tests.iter().find(|t| t.id == run.id) {
                text.push_str(&test.source);
                text.push('\n');
            }
            text.push_str(&run.output);
            text.push('\n');
        }
        parallel::files_named(&text, &files)
    }

    fn suite_guidance(&self, prepared: &Prepared) -> String {
        let mut guidance = SUITE_GUIDANCE.to_string();
        if self.policy.fast_runs {
            guidance.push_str(
                " Rerun only the red tests, as `sh run.sh T2 T5`, until the last one passes, \
                 then run the whole suite once.",
            );
        }
        let facts = constraints(&prepared.requirements);
        if !facts.is_empty() {
            guidance.push_str(&format!(
                "\n\nThe task's constraints hold throughout; honor each one exactly:\n\n{}",
                facts.join("\n")
            ));
        }
        guidance
    }

    /// The brief for the one session a failing final guard gets.
    fn guard_brief(
        &self,
        prepared: &Prepared,
        suite: &crate::accept::AcceptanceSuite,
        evidence: &[Evidence],
        number: u32,
        output: &str,
    ) -> Brief {
        let mut session_evidence = vec![suite.evidence()];
        session_evidence.extend(evidence.iter().cloned());
        Brief {
            task: prepared.instruction.clone(),
            guidance: self.suite_guidance(prepared),
            evidence: session_evidence,
            state: vec![
                format!(
                    "Session {number} of at most {}. The acceptance suite is green, but the \
                     task's own tests fail. Fix what they show without turning any acceptance \
                     test red, run both, and call finish.",
                    self.policy.max_sessions
                ),
                format!(
                    "The task's own tests:\n{}",
                    crate::judge::clip_tail(output.trim(), 2_000)
                ),
            ],
        }
    }

    /// The task's own visible tests, run once on the workspace: `None`
    /// when the workspace has none this host knows how to run.
    async fn guard(&self, runner: &crate::accept::Local) -> Option<crate::accept::TestRun> {
        let command = guard_command(&self.workdir)?;
        let dir = scratch("coder-one-guard");
        std::fs::create_dir_all(dir.join(crate::accept::TESTS_DIR)).ok()?;
        std::fs::write(
            dir.join(crate::accept::TESTS_DIR).join("guard.sh"),
            format!("#!/bin/sh\n{command}\n"),
        )
        .ok()?;
        let test = crate::accept::Test {
            id: "guard".to_string(),
            requirements: Vec::new(),
            kind: "guard".to_string(),
            what: "the task's own visible tests pass".to_string(),
            path: format!("{}/guard.sh", crate::accept::TESTS_DIR),
            source: command,
            notes: Vec::new(),
        };
        let mut runs = crate::accept::Runner::run_all(runner, &[test], &dir, &self.workdir).await;
        let _ = std::fs::remove_dir_all(&dir);
        runs.pop()
    }

    /// The joined closing judgment: the suite's result and gaps, the last
    /// session's report, and the diff against the start (a snapshot when
    /// there is one, Git otherwise), over every requirement but context and
    /// constraints. Returns Jev's done, the record, and its cost.
    async fn joined_close(
        &self,
        prepared: &Prepared,
        suite: &crate::accept::AcceptanceSuite,
        result: &crate::accept::RunResult,
        sessions: &[Ran],
        base: Option<&Path>,
    ) -> (Option<f64>, Value, f64) {
        let diff = match base {
            Some(base) => crate::delegate::changes_since(base, &self.workdir),
            None => crate::delegate::changes(&self.workdir, None),
        };
        let general = self.policy.suite_writer.as_ref().is_some_and(|w| w.general);
        let requirements: Vec<&crate::requirements::Requirement> = prepared
            .requirements
            .requirements
            .iter()
            .filter(|r| !matches!(r.kind, Kind::Context | Kind::Constraint))
            .collect();
        let state = json!({
            "task": clip_lines(&prepared.instruction, 4_000),
            "requirements": requirements.iter().map(|r| json!({
                "id": r.id,
                "kind": r.kind.word(),
                "text": r.text.split_whitespace().collect::<Vec<_>>().join(" "),
            })).collect::<Vec<_>>(),
            "suite": {
                "status": suite.status,
                "green": result.passed,
                "total": result.total,
                "open_gaps": suite.gaps,
                "tests": suite.tests.iter().map(|t| format!("{} ({}): {}", t.id, t.requirements.join(", "), t.what)).collect::<Vec<_>>(),
            },
            "report": sessions.last().map(|r| clip_lines(&r.summary(), 1_500)),
            "diff": crate::judge::clip(&diff, 8_000),
        });
        let mut questions = jev::Questions::new().with("done", jev::Noul::new(CLOSE_QUESTION));
        for j in 0..requirements.len() {
            questions = questions.with(
                format!("requirement_{j}"),
                jev::Noul::new(
                    if general {
                        close_requirement_question_general(j)
                    } else {
                        close_requirement_question(j)
                    }
                    .as_str(),
                ),
            );
        }
        let asked = jev_component::ask(
            &prepared.jev,
            &self.recorder,
            jev_component::Ask {
                component: "verify.close",
                name: "jev_joined_close",
                id: format!("jev-joined-close-{}", self.dispatch()),
                state,
                questions,
                parent: None,
                deadline: prepared.deadline.clone(),
            },
        )
        .await;
        let done = asked.noul("done");
        let usd = asked.input_tokens.map_or(0.0, |t| {
            t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
        });
        let record = json!({
            "kind": "close",
            "done": done,
            "requirements": requirements.iter().enumerate().map(|(j, r)| json!({
                "id": r.id,
                "text": crate::judge::clip(&r.text, 200),
                "p": asked.noul(&format!("requirement_{j}")),
            })).collect::<Vec<_>>(),
            "suite_status": suite.status,
            "open_gaps": suite.gaps.iter().map(|g| g.requirement.clone()).collect::<Vec<_>>(),
            "diff_chars": diff.chars().count(),
            "jev": { "how": asked.how, "error": asked.error },
            "jev_usd": usd,
        });
        (done, record, usd)
    }

    /// Jev's move after a suite-loop round, settled by code: `done` while
    /// the suite is red becomes `retry`, and `stuck` after a round that
    /// turned a test green becomes `retry`.
    async fn suite_move(
        &self,
        prepared: &Prepared,
        suite: &crate::accept::AcceptanceSuite,
        result: &crate::accept::RunResult,
        ended: &[&Ran],
        since_progress: u32,
    ) -> (Move, Value, f64) {
        let red: Vec<Value> = result
            .tests
            .iter()
            .filter(|t| !t.green)
            .map(|t| {
                json!({
                    "id": t.id,
                    "requirements": t.requirements,
                    "what": suite.tests.iter().find(|s| s.id == t.id).map(|s| s.what.clone()),
                    "output": crate::judge::clip_tail(t.output.trim(), 500),
                })
            })
            .collect();
        let state = json!({
            "task": clip_lines(&prepared.instruction, 4_000),
            "suite": {
                "green": result.passed,
                "total": result.total,
                "red_tests": red,
                "rounds_without_a_new_green_test": since_progress,
            },
            "sessions": ended.iter().map(|r| json!({
                "number": r.number,
                "worked_on": r.focus,
                "status": r.status(),
                "cause": r.cause().word(),
                "report": clip_lines(&r.summary(), 1_000),
                "changed_files": r.changed,
            })).collect::<Vec<_>>(),
        });
        let mut choice = jev::Choice::new(SUITE_MOVE_QUESTION, indexmap::IndexMap::new());
        for (name, meaning) in SUITE_MOVES {
            choice = choice.option(name, meaning);
        }
        let asked = jev_component::ask(
            &prepared.jev,
            &self.recorder,
            jev_component::Ask {
                component: HANDOFF_COMPONENT,
                name: "jev_suite_move",
                id: format!(
                    "jev-suite-move-{}-{}",
                    self.dispatch(),
                    ended.last().map_or(0, |r| r.number)
                ),
                state,
                questions: jev::Questions::new().with("next_move", choice),
                parent: None,
                deadline: prepared.deadline.clone(),
            },
        )
        .await;
        let picked = asked.choice("next_move").and_then(Move::parse);
        let (chosen, overridden) = match picked {
            None => (
                Move::Retry,
                Some("Jev gave no answer, so the loop retries".to_string()),
            ),
            Some(Move::Done) => (
                Move::Retry,
                Some("the suite isn't green, so done became retry".to_string()),
            ),
            Some(Move::Stuck) if since_progress == 0 => (
                Move::Retry,
                Some("the last round turned a test green, so stuck became retry".to_string()),
            ),
            Some(chosen) => (chosen, None),
        };
        let usd = asked.input_tokens.map_or(0.0, |t| {
            t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
        });
        let record = json!({
            "kind": "move",
            "after_session": ended.last().map(|r| r.number),
            "jev": {
                "how": asked.how,
                "picked": picked.map(Move::word),
                "probabilities": asked.answers.as_ref().and_then(|a| a.pointer("/next_move/probabilities")).cloned(),
                "error": asked.error,
            },
            "move": chosen.word(),
            "overridden": overridden,
            "jev_usd": usd,
        });
        (chosen, record, usd)
    }

    /// Plans the next round's lanes: the red tests as units, the files
    /// their evidence names, Jev's Noul per pair on whether two units share
    /// a file (the code rule where Jev gives none), and the units packed
    /// into lanes. Returns the lanes, the record, and Jev's cost.
    async fn plan_lanes(
        &self,
        prepared: &Prepared,
        suite: &crate::accept::AcceptanceSuite,
        result: &crate::accept::RunResult,
        first: &[String],
    ) -> (Vec<parallel::Lane>, Value, f64) {
        let files = parallel::workspace_files(&self.workdir);
        let text = |id: &str| {
            prepared
                .requirements
                .requirements
                .iter()
                .find(|r| r.id == id)
                .map(|r| format!("{id}: {}", r.text))
                .unwrap_or_default()
        };
        let units = parallel::units(suite, result, &files, first, &text);
        if units.len() < 2 {
            return (
                parallel::lanes(&units, &|_, _| true, 1),
                json!({ "kind": "plan", "units": units, "lanes": 1 }),
                0.0,
            );
        }
        let (state, questions) = parallel::independence(&prepared.instruction, &units);
        let asked = jev_component::ask(
            &prepared.jev,
            &self.recorder,
            jev_component::Ask {
                component: parallel::COMPONENT,
                name: "jev_shared_files",
                id: format!("jev-shared-files-{}-{}", self.dispatch(), atif::now_ms()),
                state,
                questions,
                parent: None,
                deadline: prepared.deadline.clone(),
            },
        )
        .await;
        let mut pairs = Vec::new();
        let mut shared = std::collections::BTreeMap::new();
        for a in 0..units.len() {
            for b in a + 1..units.len() {
                let p = asked.noul(&parallel::pair_key(a, b));
                let (yes, by) = match p {
                    Some(p) => (p >= parallel::SHARED_MIN, "jev"),
                    None => (parallel::code_shared(&units[a], &units[b]), "code"),
                };
                shared.insert((a, b), yes);
                pairs.push(json!({ "a": a, "b": b, "p": p, "shared": yes, "by": by }));
            }
        }
        let lanes = parallel::lanes(
            &units,
            &|a, b| shared.get(&(a, b)).copied().unwrap_or(true),
            self.policy.parallel_edits as usize,
        );
        let usd = asked.input_tokens.map_or(0.0, |t| {
            t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
        });
        crate::say::line(&format!(
            "  microluna ▸ plan: {} red units in {} lane{} ({})",
            units.len(),
            lanes.len(),
            if lanes.len() == 1 { "" } else { "s" },
            lanes
                .iter()
                .map(|l| l.tests.join(", "))
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        let record = json!({
            "kind": "plan",
            "units": units,
            "pairs": pairs,
            "lanes": lanes,
            "jev": { "how": asked.how, "error": asked.error },
            "jev_usd": usd,
        });
        (lanes, record, usd)
    }

    /// One round of sessions at once, each on its lane's red tests in its
    /// own copy of the workspace, then the merge of their changes into the
    /// workspace. Returns the sessions and the merge's record.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    async fn parallel_round(
        &self,
        prepared: &Prepared,
        suite: &crate::accept::AcceptanceSuite,
        result: &crate::accept::RunResult,
        lanes: &[parallel::Lane],
        evidence: &[Evidence],
        sessions: &[Ran],
        notes: &[String],
        round: u32,
        number: u32,
        best: usize,
    ) -> (Vec<Ran>, Value) {
        let root = scratch(&format!("coder-one-lanes-{round}"));
        let base = root.join("base");
        let _ = std::fs::remove_dir_all(&root);
        let mut places: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut copied = crate::handoff::copy_tree(&self.workdir, &base);
        for i in 1..=lanes.len() {
            if copied.is_err() {
                break;
            }
            let (copy, suite_copy) = (
                root.join(format!("lane-{i}")),
                root.join(format!("suite-{i}")),
            );
            copied = crate::handoff::copy_tree(&self.workdir, &copy).and_then(|()| {
                crate::accept::runner::rebase_tree(
                    &suite.dir,
                    &suite_copy,
                    &[(&suite.dir, &suite_copy), (&self.workdir, &copy)],
                )
            });
            let _ = std::fs::write(
                suite_copy.join("run.sh"),
                crate::accept::runner::local_run_sh_with(
                    &copy,
                    120,
                    self.policy.test_jobs as usize,
                ),
            );
            places.push((copy, suite_copy));
        }
        if let Err(error) = copied {
            let _ = std::fs::remove_dir_all(&root);
            crate::say::line(&format!(
                "  microluna ▸ no copies for a parallel round ({error}); one session runs"
            ));
            let number = number + 1;
            let brief = self.suite_brief(
                prepared, suite, result, evidence, sessions, notes, number, best, 0,
            );
            let ran = self
                .session(
                    number,
                    &result.red_requirements(),
                    "the copies failed",
                    &brief,
                    false,
                )
                .await;
            return (vec![ran], json!({ "round": round, "skipped": error }));
        }
        let numbers: Vec<u32> = (1..=lanes.len())
            .map(|i| number + u32::try_from(i).unwrap_or(0))
            .collect();
        let n = lanes.len();
        crate::say::line(&format!(
            "  microluna ▸ round {round}: sessions {} at once on {}",
            numbers
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            lanes
                .iter()
                .map(|l| l.tests.join(", "))
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        let before = crate::delegate::fingerprint(&self.workdir);
        let runs = futures_util::future::join_all(lanes.iter().enumerate().map(|(i, lane)| {
            let (copy, suite_copy) = &places[i];
            let others: Vec<String> = lanes
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .flat_map(|(_, l)| l.tests.clone())
                .collect();
            let lane_suite = crate::accept::AcceptanceSuite {
                dir: suite_copy.clone(),
                ..suite.clone()
            };
            let rebase = |text: &str| crate::compose::best_of::rebase(text, &self.workdir, copy);
            let mut guidance = self.suite_guidance(prepared);
            guidance.push_str(&format!(
                "\n\nThis session works only on the red tests {tests} ({requirements}). Other \
                 sessions work on the red tests {others} at the same time, each in its own copy \
                 of the workspace; leave what their tests check to them. Your workspace is a \
                 private copy of the task's workspace at `{copy}`: where the task or the \
                 evidence names `{real}`, use `{copy}`. Run your tests with `sh {suite}/run.sh \
                 {tests}`. When they pass, call finish: the host merges your changes into \
                 `{real}` once every session of this round has ended.",
                tests = lane.tests.join(", "),
                requirements = lane.requirements.join(", "),
                others = others.join(", "),
                copy = copy.display(),
                real = self.workdir.display(),
                suite = suite_copy.display(),
            ));
            let red_lines: Vec<String> = result
                .red_lines(suite, 800)
                .into_iter()
                .enumerate()
                .filter(|(k, line)| {
                    *k == 0
                        || lane
                            .tests
                            .iter()
                            .any(|t| line.starts_with(&format!("{t} (")))
                })
                .map(|(_, line)| rebase(&line))
                .collect();
            let mut state = vec![format!(
                "Session {} of at most {}, group {} of {n}. The suite is {} of {} green; the \
                 best so far is {best}.",
                numbers[i],
                self.policy.max_sessions,
                i + 1,
                result.passed,
                result.total
            )];
            state.extend(notes.iter().map(|n| rebase(n)));
            state.push(format!(
                "Your red tests and their output:\n{}",
                red_lines.join("\n")
            ));
            for ran in sessions.iter().rev().take(3).rev() {
                state.push(format!(
                    "Session {} ended {}: {}",
                    ran.number,
                    ran.status(),
                    rebase(&crate::judge::clip(&ran.summary(), 300))
                ));
            }
            let mut session_evidence = vec![lane_suite.evidence()];
            session_evidence.extend(evidence.iter().map(|e| Evidence {
                label: rebase(&e.label),
                text: rebase(&e.text),
            }));
            session_evidence.extend(self.source_evidence(copy, &lane.files));
            let brief = Brief {
                task: rebase(&prepared.instruction),
                guidance,
                evidence: session_evidence,
                state,
            };
            let parallel_with: Vec<u32> = numbers
                .iter()
                .copied()
                .filter(|m| *m != numbers[i])
                .collect();
            let why = format!(
                "session {} works on {} ({}), group {} of {n}, in parallel with session{} {}",
                numbers[i],
                lane.tests.join(", "),
                lane.requirements.join(", "),
                i + 1,
                if parallel_with.len() == 1 { "" } else { "s" },
                parallel_with
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(" and ")
            );
            let place = Place {
                workdir: Some(copy.clone()),
                group: Some(format!("group {} of {n}: {}", i + 1, lane.tests.join(", "))),
                batch: format!("round {round}"),
                parallel_with,
                alongside: None,
            };
            let focus = lane.requirements.clone();
            let number = numbers[i];
            async move {
                self.session_at(number, &focus, &why, &brief, false, place)
                    .await
            }
        }))
        .await;
        let leaked = crate::delegate::fingerprint(&self.workdir) != before;
        let copies: Vec<PathBuf> = places.iter().map(|(copy, _)| copy.clone()).collect();
        let merged = parallel::merge(&self.workdir, &base, &copies);
        let requeued: Vec<String> = merged
            .conflicts
            .iter()
            .flat_map(|c| lanes[c.lane].tests.clone())
            .collect();
        let note = (!merged.conflicts.is_empty()).then(|| {
            merged
                .conflicts
                .iter()
                .map(|c| {
                    format!(
                        "Session {} worked on {} at the same time as others, but its changes to \
                         {} clashed with an earlier session's and were discarded: start from the \
                         merged workspace. It reported: {}",
                        numbers[c.lane],
                        lanes[c.lane].tests.join(", "),
                        c.files.join(", "),
                        crate::judge::clip(&runs[c.lane].summary(), 300)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        });
        crate::say::line(&format!(
            "  microluna ▸ merged round {round}: {} of {n} sessions applied ({} files, {} joined line by line){}{}",
            merged.applied.len(),
            merged.files.len(),
            merged.joined.len(),
            if merged.conflicts.is_empty() {
                String::new()
            } else {
                format!("; requeued {}", requeued.join(", "))
            },
            if leaked {
                "; the workspace changed while the sessions ran"
            } else {
                ""
            }
        ));
        let record = json!({
            "round": round,
            "sessions": numbers,
            "lanes": lanes,
            "applied": merged.applied.iter().map(|i| numbers[*i]).collect::<Vec<_>>(),
            "conflicts": merged.conflicts.iter().map(|c| json!({ "session": numbers[c.lane], "files": c.files })).collect::<Vec<_>>(),
            "files": merged.files,
            "joined": merged.joined,
            "requeued": requeued,
            "leaked": leaked,
            "note": note,
        });
        self.recorder.push(
            Step::said(
                Source::System,
                &format!(
                    "Merged sessions {}: {} applied, {} conflicted.",
                    numbers
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    merged.applied.len(),
                    merged.conflicts.len()
                ),
            )
            .noting(
                crate::handoff::KEY,
                json!({
                    "from": format!("{AGENT} sessions {}", numbers.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")),
                    "to": "the workspace, merged",
                    "trigger": format!(
                        "{} of {n} sessions' changes merged ({} files){}",
                        merged.applied.len(),
                        merged.files.len(),
                        if requeued.is_empty() { String::new() } else { format!("; {} requeued after a conflict", requeued.join(", ")) }
                    ),
                    "pattern": "microluna-parallel",
                    "action": "merge",
                }),
            ),
        );
        let _ = std::fs::remove_dir_all(&root);
        (runs, record)
    }

    /// One session's brief: the task first, then the group and its
    /// evidence, then the state.
    #[allow(clippy::too_many_arguments)]
    fn brief(
        &self,
        prepared: &Prepared,
        groups: &[Group],
        cursor: usize,
        attempt: u32,
        sessions: &[Ran],
        checked: Option<&Checked>,
        read_only: bool,
    ) -> Brief {
        let group = &groups[cursor];
        let later: Vec<String> = groups[cursor + 1..]
            .iter()
            .flat_map(|g| g.ids.clone())
            .collect();
        let earlier: Vec<String> = groups[..cursor]
            .iter()
            .flat_map(|g| g.ids.clone())
            .collect();
        let mut guidance = format!(
            "This task runs as several short sessions. This session works only on {}:\n\n{}\n\n",
            if group.ids.len() == 1 {
                "this requirement"
            } else {
                "these requirements"
            },
            group.lines.join("\n")
        );
        if !earlier.is_empty() {
            guidance.push_str(&format!(
                "Earlier sessions worked on {}; keep their work intact. ",
                earlier.join(", ")
            ));
        }
        if !later.is_empty() {
            guidance.push_str(&format!(
                "Later sessions work on {}; leave them unless this work needs them. ",
                later.join(", ")
            ));
        }
        if self.policy.focus_actionable {
            let constraints = constraints(&prepared.requirements);
            if !constraints.is_empty() {
                guidance.push_str(&format!(
                    "\n\nThese constraints are the decisive facts of the task; honor every one \
                     exactly in what you do here. Use the exact values, formulas, seeds, \
                     column names, ranges, and formats they state. Do not substitute a \
                     simpler rule or an approximation for what a constraint specifies, and do \
                     not skip one because it looks minor.\n\n{}\n\n",
                    constraints.join("\n")
                ));
            }
        }
        if read_only {
            guidance.push_str(
                "This is a read-only session: you can't edit files, only read and run \
                 commands. Reproduce the problem and run the task's own tests or example so \
                 the next session starts from what actually happens. When you've seen enough, \
                 call finish with a summary of what you found and what the edit session should \
                 change.",
            );
        } else if self.policy.require_evidence {
            guidance.push_str(
                "Make the change the focus needs, then run something that exercises it: the \
                 task's test, its example, or a command that shows the new behavior. Call \
                 finish with status done only once the workspace shows the change and you've \
                 seen it work; if the focus asks only a question, answer it in finish and say \
                 what you ran to be sure. If you can't meet the focus, call finish with blocked \
                 or failed and say why.",
            );
        } else {
            guidance.push_str(
                "Do what the focus needs: change a file if it asks for a change, or answer in \
                 finish if it asks a question. Check your work by running something when you \
                 can, then call finish with status done. If the focus needs no change, say so \
                 and finish. If you can't meet it, call finish with blocked or failed and say \
                 why.",
            );
        }
        let mut state = vec![format!(
            "Session {} of at most {}; attempt {attempt} on this focus.",
            sessions.len() + 1,
            self.policy.max_sessions
        )];
        for ran in sessions {
            state.push(format!(
                "Session {} on {} ended {}: {}",
                ran.number,
                ran.focus.join(", "),
                ran.status(),
                crate::judge::clip(&ran.summary(), 400)
            ));
        }
        if !sessions.is_empty() {
            let changes = crate::delegate::changes(&self.workdir, None);
            state.push(format!(
                "What the workspace shows as changed now:\n{}",
                crate::judge::clip(&changes, 1_500)
            ));
        }
        if let Some(checked) = checked {
            let lines = checked.lines();
            if !lines.is_empty() {
                state.push(format!(
                    "The host's checks after the last session: {}",
                    lines.join("; ")
                ));
            }
        }
        Brief {
            task: prepared.instruction.clone(),
            guidance,
            evidence: evidence_for(prepared, group, self.policy.evidence_chars),
            state,
        }
    }
}

impl Executor for Micro {
    fn agent(&self) -> &str {
        AGENT
    }

    fn cost_provenance(&self) -> &'static str {
        "price_estimate"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn deadline(&self) -> Duration {
        self.deadline
    }

    fn describe(&self) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert("transport".to_string(), json!("chatgpt-codex-responses"));
        map.insert("credential".to_string(), json!("codex_auth_json"));
        map.insert("isolation".to_string(), json!(self.isolation.word()));
        map.insert(
            "microluna".to_string(),
            serde_json::to_value(&self.policy).unwrap_or(Value::Null),
        );
        let (capabilities, note) = crate::adapter::capabilities(crate::delegate::Agent::Microluna);
        map.insert("capabilities".to_string(), capabilities.record(AGENT, note));
        map
    }

    fn take_evidence(&mut self, prepared: &Prepared) {
        self.prepared = Some(prepared.clone());
    }

    async fn execute(&mut self, briefing: &Briefing) -> Report {
        let started = Instant::now();
        let prepared = self.prepared.clone().filter(|p| {
            !groups(
                &p.requirements,
                self.policy.max_groups,
                self.policy.focus_actionable,
            )
            .is_empty()
        });
        let mut parallel_summary = Value::Null;
        let lean = self.policy.lean.clone();
        let (sessions, moves, stopped, mode) = match (&self.policy.mode, prepared, lean) {
            (Mode::Requirements, Some(prepared), Some(lean)) => {
                let (sessions, moves, stopped) = self.lean_loop(&prepared, &lean).await;
                (sessions, moves, stopped, "lean")
            }
            (Mode::Requirements, Some(prepared), None) => {
                let suited = if self.policy.suite {
                    Some(self.suite_loop(&prepared).await)
                } else {
                    None
                };
                match suited {
                    Some(Ok(looped)) => {
                        parallel_summary = looped.parallel;
                        (looped.sessions, looped.moves, looped.stopped, "suite")
                    }
                    Some(Err(prior)) => {
                        let (sessions, moves, stopped) = self.requirements(&prepared, prior).await;
                        (sessions, moves, stopped, "requirements")
                    }
                    None => {
                        let (sessions, moves, stopped) =
                            self.requirements(&prepared, Vec::new()).await;
                        (sessions, moves, stopped, "requirements")
                    }
                }
            }
            _ => (
                self.single(briefing).await,
                Vec::new(),
                "the one session ended".to_string(),
                "single",
            ),
        };
        // The suite writers are Luna sessions too: their cost and tokens
        // count with the edit sessions'.
        let (writer_usd, writer_usage) = moves.iter().filter(|m| m["kind"] == "suite").fold(
            (0.0, TokenUsage::default()),
            |(usd, mut usage), m| {
                usage.add(TokenUsage {
                    input: m["writer_usage"]["input"].as_u64().unwrap_or(0),
                    cached: m["writer_usage"]["cached"].as_u64().unwrap_or(0),
                    output: m["writer_usage"]["output"].as_u64().unwrap_or(0),
                    reasoning: 0,
                });
                (usd + m["writer_usd"].as_f64().unwrap_or(0.0), usage)
            },
        );
        let mut usage = writer_usage;
        let mut cost = Some(writer_usd);
        for ran in &sessions {
            usage.add(ran.usage);
            cost = match (cost, ran.cost_usd) {
                (Some(total), Some(one)) => Some(total + one),
                _ => None,
            };
        }
        let turns: usize = sessions.iter().map(|r| r.turns).sum();
        let calls: usize = sessions.iter().map(|r| r.calls).sum();
        let all_lost = !sessions.is_empty()
            && sessions
                .iter()
                .all(|r| matches!(r.ending, Ending::Transport(_)));
        let status = if sessions.is_empty() {
            Status::Harness(format!("no session ran: {stopped}"))
        } else if all_lost {
            let detail = match &sessions[0].ending {
                Ending::Transport(why) => why.clone(),
                _ => String::new(),
            };
            Status::Transport {
                detail,
                reached: turns > 0,
            }
        } else if matches!(sessions.last().map(|r| &r.ending), Some(Ending::Deadline)) {
            Status::TimedOut
        } else {
            Status::Answered
        };
        let last = sessions.last();
        let mut result = format!(
            "Microluna ran {} session{} ({mode}): {stopped}.",
            sessions.len(),
            if sessions.len() == 1 { "" } else { "s" },
        );
        for ran in &sessions {
            result.push_str(&format!(
                "\n- Session {} on {}: {}. {}",
                ran.number,
                ran.focus.join(", "),
                ran.status(),
                crate::judge::clip(&ran.summary(), 300)
            ));
        }
        if let Some(finish) = last.and_then(|r| r.finish.as_ref())
            && !finish.answer.trim().is_empty()
        {
            result.push_str(&format!("\n\nAnswer: {}", finish.answer));
        }
        let record = json!({
            "schema": LOOP_SCHEMA,
            "mode": mode,
            "policy": self.policy,
            "model": self.model,
            "stopped": stopped,
            "sessions": sessions.iter().map(Ran::record).collect::<Vec<_>>(),
            "moves": moves,
            "usage": usage_json(usage),
            "cost_usd": cost,
            "writer_usd": writer_usd,
            "parallel": parallel_summary,
            "stopped_by": Value::Null,
            "session_id": last.map(|r| r.session_id.clone()),
        });
        let file = self
            .artifacts
            .join(format!("microluna-{}.json", self.dispatch()));
        if let Ok(text) = serde_json::to_string_pretty(&record) {
            let _ = crate::record::write_atomic(&file, text.as_bytes());
        }
        self.last = Some(record);
        self.runs += 1;
        let milliseconds = millis(started);
        Report {
            status,
            summary: Summary {
                has_result: true,
                result: Some(result),
                is_error: Some(all_lost),
                subtype: Some(format!("microluna-{mode}")),
                num_turns: u64::try_from(turns).ok(),
                total_cost_usd: cost,
                duration_ms: Some(milliseconds),
                duration_api_ms: None,
                session_id: last.map(|r| r.session_id.clone()),
                usage: Some(json!({
                    "input_tokens": usage.uncached(),
                    "cache_read_input_tokens": usage.cached,
                    "output_tokens": usage.output,
                    "reasoning_output_tokens": usage.reasoning,
                })),
                model_usage: Some(json!({
                    self.model.clone(): usage_json(usage),
                })),
                model: Some(self.model.clone()),
                version: Some(crate::episode::version()),
                api_key_source: Some("codex_auth_json".to_string()),
                api_calls: u64::try_from(turns).ok(),
                completed_items: u64::try_from(calls).ok(),
                input_per_call: Vec::new(),
                cost_provenance: Some("price_estimate"),
                cost_note: Some(microluna::price::COST_NOTE),
                limit: None,
            },
            milliseconds,
            stderr: String::new(),
            stream: Some(json!({
                "path": format!("artifacts/microluna-{}.json", self.runs),
                "sessions": sessions.iter().map(|r| r.trace.clone()).collect::<Vec<_>>(),
            })),
        }
    }
}

#[cfg(test)]
mod tests;
