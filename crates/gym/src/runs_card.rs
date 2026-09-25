//! A run card: one trial characterized from its retained records.
//!
//! `gym runs characterize RUN` reads one trial directory and computes what
//! the hand reconstruction of the three v13 trials
//! (`docs/terminal-bench/2026-09-25-microluna-v13-embedding-trials.md`)
//! worked out over a day: who ran, where the time went phase by phase, each
//! session's anatomy, which briefing items pointed at the edited code, how
//! the session-written check changed, what the host and the sessions ran,
//! what was wasted, what was reversed, what the review changed, what each
//! session claimed beside what the verifier found, and the cheapest
//! passing public trajectory on the same task.
//!
//! Every number is arithmetic over retained files. A number whose record
//! is missing is `unknown`, and a section whose record no policy wrote yet
//! (check-line grades, host-executed commands) is `not recorded`; the card
//! never estimates one. [`crate::runs_card_render`] draws the card as one
//! Markdown page and compares two cards; [`crate::runs_card_source`] holds
//! the untouched-source and diff helpers.
//!
//! Jev answers nothing here that it can't answer offline. A command the
//! phase rules can't place takes a cached answer from the fingerprint
//! store when one exists and stays unplaced otherwise. Check-line grades
//! come from the record the grader wrote. Whether an edit addresses a
//! suspect whose line it didn't touch is never asked, so that answer is
//! `unknown`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use crate::runs::{Run, read_json, text};
use crate::runs_analysis::{Records, Session};
use crate::runs_card_source::{self as source, Change};
use crate::runs_phases::{ActionKind, Phase, Step, StepStore};
use crate::terminal_bench::timestamp_ms;

/// The card's JSON schema.
pub const SCHEMA: &str = "openagents.gym.run-card.v1";

/// The card's rules version. Change it with any change to what it
/// computes, so a stored card says which rules made it.
pub const VERSION: &str = "run-card-v1";

/// A command at least this long counts as slow.
pub const SLOW_COMMAND_MS: u64 = 5_000;

/// The file of commands the host ran itself (#9633, #9636), in the
/// episode's `artifacts/` or a lean group's directory under it.
pub const EXECUTED_FILE: &str = "executed-commands.jsonl";

/// The schema of each line of [`EXECUTED_FILE`].
pub const EXECUTED_SCHEMA: &str = "openagents.coder-one.executed-command.v1";

/// The file of check-line grades written at the freeze (#9635).
pub const GRADES_FILE: &str = "check-grades.json";

/// The schema of [`GRADES_FILE`].
pub const GRADES_SCHEMA: &str = "openagents.coder-one.check-grades.v1";

/// The schema of the task-level defect-site record.
pub const SITES_SCHEMA: &str = "openagents.gym.defect-sites.v1";

/// What a card reads beyond the trial directory: task-level records only.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// The experiments' pins files, to say whether a task is in a policy's
    /// development set.
    pub pins: Vec<(PathBuf, Value)>,
    /// The task-level defect-site record, when there is one.
    pub sites: Option<Value>,
    /// The public-attempt manifest, for the reference trajectory.
    pub fable: Option<Value>,
    /// Whether to load the reference trajectory's body for its fingerprint.
    pub reference_body: bool,
    /// Cached Jev phase answers; never asked, only read.
    pub store: Option<StepStore>,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The pins files under `bench/terminal-bench/experiments/*/records/`.
#[must_use]
pub fn read_pins(root: &Path) -> Vec<(PathBuf, Value)> {
    let dir = root.join("bench/terminal-bench/experiments");
    let mut found: Vec<(PathBuf, Value)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path().join("records/pins.json"))
        .filter_map(|path| read_json(&path).map(|value| (path, value)))
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// Where the defect-site record lives in the checkout.
#[must_use]
pub fn sites_path(root: &Path) -> PathBuf {
    root.join("bench/terminal-bench/reference/defect-sites.json")
}

impl Options {
    /// This checkout's pins, defect sites, and public-attempt manifest, and
    /// the fingerprint store's cached answers.
    #[must_use]
    pub fn standard() -> Self {
        let root = repository_root();
        Options {
            pins: read_pins(&root),
            sites: read_json(&sites_path(&root)),
            fable: read_json(&crate::runs_analysis::fable_manifest_path()),
            reference_body: true,
            store: Some(StepStore::open(crate::runs_phases::default_dir())),
        }
    }

    /// What `gym runs show` reads: the standard records and cached answers,
    /// without parsing the reference trajectory's body.
    #[must_use]
    pub fn for_show() -> Self {
        Options {
            reference_body: false,
            ..Options::standard()
        }
    }

    /// The checkout's records with no cached Jev answers and no public
    /// trajectory bodies: the same card on every computer.
    #[must_use]
    pub fn offline() -> Self {
        Options {
            store: None,
            reference_body: false,
            ..Options::standard()
        }
    }
}

// ---------------------------------------------------------------------------
// The card
// ---------------------------------------------------------------------------

/// Who ran what, and how it came out.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Identity {
    pub run: String,
    pub task: String,
    /// The task checkout's pinned commit, or the task's checksum.
    pub revision: Option<String>,
    pub policy: Option<String>,
    pub policy_digest: Option<String>,
    pub binary: Option<String>,
    pub arm: Option<String>,
    pub attempt_id: Option<String>,
    /// `fresh`, `resume`, and so on, as the attempt record says.
    pub attempt_kind: Option<String>,
    pub reward: Option<f64>,
    pub tests_passed: Option<u64>,
    pub tests_total: Option<u64>,
    pub cost_usd: Option<f64>,
    pub cost_estimated: bool,
    pub trial_ms: Option<u64>,
    pub agent_ms: Option<u64>,
    /// Whether the task is in the policy's development set, from the pins.
    pub development: Option<bool>,
    pub development_source: Option<String>,
}

/// One row of the phase timeline.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PhaseRow {
    /// `environment_setup`, `session_1`, `host_after_1`, and so on.
    pub key: String,
    pub name: String,
    /// Milliseconds from the trial's start.
    pub start_ms: Option<i64>,
    pub duration_ms: Option<u64>,
    /// The share of trial time.
    pub share: Option<f64>,
}

/// When things first and last happened in a session, from its start.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Moments {
    pub first_read_ms: Option<u64>,
    pub first_command_ms: Option<u64>,
    pub first_edit_ms: Option<u64>,
    pub last_edit_ms: Option<u64>,
    pub finish_ms: Option<u64>,
}

/// The time and turns after a session's last edit.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Tail {
    /// From the last edit to the finish call.
    pub to_finish_ms: Option<u64>,
    /// From the last edit to the session's end.
    pub to_end_ms: u64,
    /// Turns after the last edit's turn, the finish turn included.
    pub turns: usize,
    /// Commands after the last edit that ran a program: the session's own
    /// tests, the score, the task's program.
    pub runs: usize,
    /// Of those, runs of the session-written check.
    pub score_runs: usize,
}

/// A run of edits with no program run between them.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditRound {
    pub first_turn: usize,
    pub last_turn: usize,
    pub files: Vec<String>,
    /// Whether a program run followed before the next round or the finish.
    pub checked: bool,
}

/// A session command, classed.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SessionCommand {
    pub turn: usize,
    pub tool: String,
    pub input: String,
    /// The phase, or `None` when neither the rules nor a cached Jev answer
    /// placed it.
    pub phase: Option<String>,
    /// `rule`, `jev`, or `none`.
    pub by: String,
    pub exit: Option<i64>,
    pub ms: u64,
}

/// One session's anatomy.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SessionCard {
    pub number: usize,
    pub id: String,
    /// `session`, `self-check`, `suite writer`, and so on.
    pub role: String,
    pub start_ms: Option<i64>,
    pub duration_ms: u64,
    pub turns: usize,
    pub calls: usize,
    pub model_ms: u64,
    pub model_share: Option<f64>,
    pub command_ms: u64,
    pub command_share: Option<f64>,
    /// Every tool call's time.
    pub tool_ms: u64,
    /// Session time neither the model nor a tool accounts for.
    pub overhead_ms: u64,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub cached_share: Option<f64>,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cost_usd: Option<f64>,
    pub moments: Moments,
    pub tail: Option<Tail>,
    pub edit_rounds: Vec<EditRound>,
    /// Steps by phase, `unplaced` included.
    pub phases: BTreeMap<String, usize>,
    pub by_rule: usize,
    pub by_jev: usize,
    pub unplaced: usize,
    pub commands: Vec<SessionCommand>,
    pub finish_status: Option<String>,
    pub finish_summary: Option<String>,
}

/// One suspect from the briefing's likely-defects list.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Suspect {
    pub file: String,
    pub line: Option<usize>,
    pub text: String,
    pub p: Option<f64>,
    /// Whether a session edited the file.
    pub file_edited: bool,
    /// Whether the submitted workspace changed the suspect's line, or
    /// `None` when the untouched or the submitted text isn't retained.
    pub line_changed: Option<bool>,
}

/// One edited file and what pointed at it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditedFile {
    pub path: String,
    /// The briefing's file item for it: `complete` or `trimmed`.
    pub file_item: Option<String>,
    pub suspects: usize,
    /// Lines the submitted workspace changed against the untouched text,
    /// or `None` when either is missing.
    pub changed_lines: Option<usize>,
}

/// A defect site from the task-level record.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Site {
    pub id: String,
    pub file: String,
    pub what: String,
    pub named: bool,
    pub edited: bool,
}

/// Which briefing items pointed at the code the sessions edited.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Provenance {
    pub suspects: Vec<Suspect>,
    pub edited: Vec<EditedFile>,
    /// Where the untouched text of each file came from.
    pub untouched: BTreeMap<String, String>,
    /// Suspects whose line the submitted workspace changed, over suspects
    /// with a known answer.
    pub suspect_line_hits: Option<(usize, usize)>,
    /// Suspects that named an edited file, over suspects.
    pub suspect_file_hits: Option<(usize, usize)>,
    /// Edited files a suspect named, over edited files.
    pub pointer_coverage: Option<(usize, usize)>,
    /// Edited files any briefing item named, over edited files.
    pub briefing_coverage: Option<(usize, usize)>,
    /// The task's defect sites, when the task-level record has them.
    pub sites: Vec<Site>,
    pub sites_source: Option<String>,
}

/// One version of the session-written check.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CheckVersion {
    pub session: usize,
    pub turn: usize,
    /// Milliseconds from the session's start.
    pub at_ms: u64,
    pub how: String,
    /// Whether a workspace edit came between the previous version and
    /// this one: a rewrite that resolves a test against the code.
    pub after_code_edit: bool,
    /// Whether the last score run before this version was short of full.
    pub after_failing_score: bool,
    /// The version's score on the untouched workspace, when it ran before
    /// any edit.
    pub untouched_score: Option<(u64, u64)>,
}

/// One run of the check inside a session.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ScoreRun {
    pub session: usize,
    pub turn: usize,
    pub at_ms: u64,
    pub passed: u64,
    pub total: u64,
}

/// The host's score and snapshot after a session.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Frozen {
    pub group: String,
    pub after_session: usize,
    pub self_check: bool,
    pub score: Option<(u64, u64)>,
    pub kept: Option<bool>,
    pub evaluator_digest: Option<String>,
    pub hardcoded_p: Option<f64>,
}

/// The session-written check across the trial.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CheckLineage {
    /// The check's path, from the briefing.
    pub path: Option<String>,
    pub versions: Vec<CheckVersion>,
    pub runs: Vec<ScoreRun>,
    pub frozen: Vec<Frozen>,
    /// The grades record (#9635), as written, or `None` when not recorded.
    pub grades: Option<Value>,
    pub grades_path: Option<String>,
}

/// A host operation from the episode log.
#[derive(Clone, Debug, Default, Serialize)]
pub struct HostOperation {
    pub label: String,
    pub outcome: Option<String>,
    pub exit: Option<i64>,
    pub ms: Option<u64>,
    pub output_sha256: Option<String>,
    pub refused: Option<String>,
}

/// What the host and the sessions ran.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Executed {
    pub host_operations: Vec<HostOperation>,
    /// The host-executed command records (#9633, #9636), as written, or
    /// `None` when not recorded.
    pub host_commands: Option<Vec<Value>>,
    pub host_commands_path: Option<String>,
    /// Session steps by phase over all sessions.
    pub session_phases: BTreeMap<String, usize>,
    /// Where the session steps' phases came from.
    pub placement: String,
}

/// One cause of wasted turns.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Missing {
    pub program: String,
    /// `session.turn` for each turn it cost.
    pub turns: Vec<String>,
}

/// A refused tool call.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Refused {
    pub session: usize,
    pub turn: usize,
    pub tool: String,
    pub cause: String,
}

/// Slow commands with one first line.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Slow {
    pub command: String,
    pub runs: usize,
    pub ms: u64,
}

/// What cost time and did nothing.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Waste {
    pub not_found: Vec<Missing>,
    pub refused: Vec<Refused>,
    /// Reads of files the session's own briefing carried in full.
    pub briefing_rereads: usize,
    pub briefing_reread_paths: Vec<String>,
    pub turns_without_call: usize,
    /// Program runs while the session's last score was full.
    pub runs_after_full: usize,
    pub slow: Vec<Slow>,
    pub slow_ms: u64,
}

impl Waste {
    /// Turns lost to a program that isn't installed; a turn that missed two
    /// programs counts once.
    #[must_use]
    pub fn not_found_turns(&self) -> usize {
        self.not_found
            .iter()
            .flat_map(|m| &m.turns)
            .collect::<BTreeSet<_>>()
            .len()
    }
}

/// A file whose digest went back to an earlier one between snapshots.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DigestReturn {
    pub file: String,
    /// The snapshot it returned at and the earlier one it matches.
    pub at_session: usize,
    pub matches_session: usize,
    pub digest: String,
}

/// Reversals within and between sessions.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Reversals {
    /// From the host's per-session workspace digests, or `None` when no
    /// snapshots were recorded.
    pub between_sessions: Option<Vec<DigestReturn>>,
    /// Patch-level reversals between sessions, from the run analysis.
    pub patches: Vec<crate::runs_analysis::Reversal>,
    /// Per-edit digests inside a session, which no record keeps yet.
    pub within_sessions: Option<usize>,
}

/// What a review or self-check session changed.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ReviewDelta {
    pub session: usize,
    pub role: String,
    /// Changed files with their digests before and after.
    pub files: Vec<(String, Option<String>, Option<String>)>,
    /// Each changed file's change, when both versions are retained.
    pub changes: Vec<Change>,
    pub score_before: Option<(u64, u64)>,
    pub score_after: Option<(u64, u64)>,
    /// The host's executed-command records for the two candidates, or
    /// `None` when not recorded.
    pub executed_before: Option<Vec<Value>>,
    pub executed_after: Option<Vec<Value>>,
}

impl ReviewDelta {
    /// The delta in words: `nothing`, `one file: 1 docstring, no code`.
    #[must_use]
    pub fn words(&self) -> String {
        if self.files.is_empty() {
            return "nothing".to_owned();
        }
        let described: Vec<String> = self
            .files
            .iter()
            .map(|(path, _, _)| {
                self.changes.iter().find(|c| &c.path == path).map_or_else(
                    || format!("{path}: unknown"),
                    |c| format!("{path}: {}", c.words()),
                )
            })
            .collect();
        described.join("; ")
    }
}

/// Each session's claim beside the verifier.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Claims {
    pub reward: Option<f64>,
    pub tests_passed: Option<u64>,
    pub tests_total: Option<u64>,
    /// The host's final score on the session-written check.
    pub final_score: Option<(u64, u64)>,
    /// Whether a full self-score and the verifier agree.
    pub self_score_agrees: Option<bool>,
    /// Jev's `verify.close` probability that the task is done.
    pub close_p: Option<f64>,
    /// Per check line: its grade, whether it passed on the submitted
    /// candidate, and whether that agrees with the reward. `None` when no
    /// grades were recorded.
    pub lines: Option<Vec<Value>>,
}

/// The cheapest passing public trajectory on the task.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Reference {
    pub id: String,
    pub effort: String,
    pub cost_usd: f64,
    pub seconds: f64,
    pub steps: u64,
    /// From the fingerprint, when the body is on this computer.
    pub first_edit_ms: Option<u64>,
    pub edit_runs: Option<usize>,
    pub sequence: Option<String>,
    pub attempts: usize,
    pub passes: usize,
}

/// One trial's card.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Card {
    pub schema: String,
    pub version: String,
    pub identity: Identity,
    pub phases: Vec<PhaseRow>,
    pub sessions: Vec<SessionCard>,
    pub provenance: Provenance,
    pub checks: CheckLineage,
    pub executed: Executed,
    pub waste: Waste,
    pub reversals: Reversals,
    pub review: Vec<ReviewDelta>,
    pub claims: Claims,
    pub reference: Option<Reference>,
    /// Why a section is unknown, in words.
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

fn ms_between(start: Option<i64>, end: Option<i64>) -> Option<u64> {
    u64::try_from(end? - start?).ok()
}

fn share(part: u64, whole: u64) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

fn clip(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= limit {
        return text;
    }
    let mut clipped: String = text.chars().take(limit.saturating_sub(1)).collect();
    clipped.push('…');
    clipped
}

/// The last `SCORE p t` line in a command's output.
#[must_use]
pub fn score_of(output: &str) -> Option<(u64, u64)> {
    output.lines().rev().find_map(|line| {
        let rest = line.trim().strip_prefix("SCORE ")?;
        let mut parts = rest.split_whitespace();
        let passed = parts.next()?.parse().ok()?;
        let total = parts.next()?.parse().ok()?;
        Some((passed, total))
    })
}

/// The program a shell said it couldn't find: `python` in
/// `/bin/sh: 1: python: not found`.
#[must_use]
pub fn missing_program(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let line = line.trim();
        let shell = [
            "/bin/sh:",
            "sh:",
            "bash:",
            "/bin/bash:",
            "/usr/bin/bash:",
            "zsh:",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix));
        if !shell {
            return None;
        }
        let head = line
            .strip_suffix(": command not found")
            .or_else(|| line.strip_suffix(": not found"))?;
        let program = head.rsplit(": ").next()?.trim();
        (!program.is_empty() && !program.contains(' ')).then(|| program.to_owned())
    })
}

/// One lean group's `selection.json`, read.
#[derive(Clone, Debug, Default)]
struct Selection {
    suspects: Vec<Suspect>,
    snapshots: Vec<Snapshot>,
    /// The submitted session number and its workspace digests.
    submitted: Option<(String, usize)>,
}

#[derive(Clone, Debug, Default)]
struct Snapshot {
    group: String,
    after_session: usize,
    self_check: bool,
    score: Option<(u64, u64)>,
    kept: Option<bool>,
    files: Option<BTreeMap<String, String>>,
    candidate: Option<PathBuf>,
    evaluator_digest: Option<String>,
    hardcoded_p: Option<f64>,
}

fn score_value(value: &Value) -> Option<(u64, u64)> {
    Some((value["passed"].as_u64()?, value["total"].as_u64()?))
}

fn digests(value: &Value) -> Option<BTreeMap<String, String>> {
    value.as_object().map(|map| {
        map.iter()
            .filter_map(|(path, digest)| Some((path.clone(), digest.as_str()?.to_owned())))
            .collect()
    })
}

/// A retained path the host recorded inside its container, mapped into the
/// episode directory: `/opt/openagents/episode/artifacts/lean-1/session-1`.
fn retained_path(episode: &Path, recorded: &str) -> Option<PathBuf> {
    let at = recorded.find("artifacts/")?;
    let path = episode.join(&recorded[at..]);
    path.is_dir().then_some(path)
}

fn read_selection(episode: &Path) -> Selection {
    let mut selection = Selection::default();
    let mut groups: Vec<PathBuf> = std::fs::read_dir(episode.join("artifacts"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("lean-"))
        })
        .collect();
    groups.sort();
    for dir in groups {
        let group = dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(Value::Array(entries)) = read_json(&dir.join("selection.json")) else {
            continue;
        };
        for entry in &entries {
            match entry["kind"].as_str() {
                Some("lean.suspects") => {
                    for comment in entry["comments"].as_array().into_iter().flatten() {
                        let raw = comment["comment"].as_str().unwrap_or_default();
                        let mut parts = raw.splitn(3, ':');
                        let file = parts.next().unwrap_or_default().trim().to_owned();
                        let line = parts.next().and_then(|n| n.trim().parse().ok());
                        let text = parts.next().unwrap_or_default().trim().to_owned();
                        selection.suspects.push(Suspect {
                            file,
                            line,
                            text,
                            p: comment["p"].as_f64(),
                            ..Suspect::default()
                        });
                    }
                }
                Some("lean") => selection.snapshots.push(Snapshot {
                    group: group.clone(),
                    after_session: entry["after_session"].as_u64().unwrap_or(0) as usize,
                    self_check: entry["self_check"].as_bool().unwrap_or(false),
                    score: score_value(&entry["score"]),
                    kept: entry["kept"].as_bool(),
                    files: digests(&entry["workspace_files"]),
                    candidate: entry["candidate"]
                        .as_str()
                        .and_then(|recorded| retained_path(episode, recorded)),
                    evaluator_digest: entry["evaluator_files"]
                        .as_object()
                        .and_then(|files| files.values().next())
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    hardcoded_p: entry.pointer("/hardcoded/p").and_then(Value::as_f64),
                }),
                Some("lean.submitted") => {
                    if let Some(session) = entry["selected_session"].as_u64() {
                        selection.submitted = Some((group.clone(), session as usize));
                    }
                }
                _ => {}
            }
        }
    }
    selection
}

/// JSON lines of every file named `name` in `artifacts/` or a lean
/// group's directory under it.
fn artifact_files(episode: &Path, name: &str) -> Vec<PathBuf> {
    let artifacts = episode.join("artifacts");
    let mut found: Vec<PathBuf> = std::iter::once(artifacts.join(name))
        .chain(
            std::fs::read_dir(&artifacts)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path().join(name)),
        )
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found.dedup();
    found
}

fn read_executed(episode: &Path) -> (Option<Vec<Value>>, Option<String>) {
    let files = artifact_files(episode, EXECUTED_FILE);
    if files.is_empty() {
        return (None, None);
    }
    let mut records = Vec::new();
    for path in &files {
        for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
            if let Ok(value) = serde_json::from_str::<Value>(line)
                && value["schema"] == EXECUTED_SCHEMA
            {
                records.push(value);
            }
        }
    }
    let shown = files
        .iter()
        .map(|path| {
            path.strip_prefix(episode)
                .unwrap_or(path)
                .display()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(", ");
    (Some(records), Some(shown))
}

fn read_grades(episode: &Path) -> (Option<Value>, Option<String>) {
    for path in artifact_files(episode, GRADES_FILE) {
        if let Some(value) = read_json(&path).filter(|value| value["schema"] == GRADES_SCHEMA) {
            let shown = path
                .strip_prefix(episode)
                .unwrap_or(&path)
                .display()
                .to_string();
            return (Some(value), Some(shown));
        }
    }
    (None, None)
}

/// The development-set answer from the pins: a pins file that names the
/// policy says whether the task is among its tasks and what the
/// experiment's classification was.
fn development(
    pins: &[(PathBuf, Value)],
    policy: Option<&str>,
    task: &str,
) -> (Option<bool>, Option<String>) {
    let Some(policy) = policy else {
        return (None, None);
    };
    for (path, pin) in pins {
        let names = pin["policies"]
            .as_object()
            .is_some_and(|policies| policies.contains_key(policy));
        if !names {
            continue;
        }
        let tasks: Vec<&str> = pin["tasks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let classification = pin["classification"].as_str().unwrap_or_default();
        let shown = path
            .strip_prefix(repository_root())
            .unwrap_or(path)
            .display()
            .to_string();
        if classification.contains("development") {
            return (
                Some(tasks.contains(&task)),
                Some(format!("{shown}: {classification}")),
            );
        }
        if classification.contains("held-out") || classification.contains("held out") {
            return (Some(false), Some(format!("{shown}: {classification}")));
        }
    }
    (None, None)
}

/// A session's steps, placed by the rules, one per call in call order.
fn session_steps(session: &Session) -> Vec<Step> {
    let mut replay = crate::runs_replay::Replay::default();
    for call in &session.calls {
        let record = json!({
            "source": "Agent",
            "call": {
                "name": call.name,
                "arguments": call.arguments,
                "output": call.output,
                "outcome": if call.ok { "Completed" } else { "Failed" },
            }
        });
        replay.events.push(crate::runs_replay::Event {
            elapsed_ms: u64::try_from(call.at - session.start).unwrap_or(0),
            timing: "message timestamp",
            title: String::new(),
            text: String::new(),
            parts: Vec::new(),
            record: String::new(),
        });
        replay.extracted.push(crate::runs_phases::extract(&record));
    }
    crate::runs_phases::steps(&replay)
}

/// Each session's steps, one per call. The trial's full replay places
/// them with the whole trial as context and takes cached Jev answers; when
/// it can't be matched to the session logs call for call, each session is
/// placed alone by the rules.
fn placed_steps(
    run: &Run,
    sessions: &[Session],
    store: Option<&StepStore>,
) -> (Vec<Vec<Step>>, String) {
    let alone: Vec<Vec<Step>> = sessions.iter().map(session_steps).collect();
    let expected: usize = alone.iter().map(Vec::len).sum();
    let steps_per_call = sessions
        .iter()
        .zip(&alone)
        .all(|(session, steps)| session.calls.len() == steps.len());
    if !steps_per_call {
        return (
            Vec::new(),
            "unknown: a session log holds calls the phase rules skip".to_owned(),
        );
    }
    let full =
        crate::runs_fingerprint::load(&crate::runs_replay::Source::Local(Box::new(run.clone())));
    if let Ok(mut loaded) = full {
        loaded
            .steps
            .retain(|step| !matches!(step.kind, ActionKind::Check));
        if loaded.steps.len() == expected {
            if let Some(store) = store {
                crate::runs_phases::apply(&mut loaded.steps, &loaded.task, store);
            }
            let mut rest = loaded.steps.into_iter();
            let split: Vec<Vec<Step>> = alone
                .iter()
                .map(|steps| rest.by_ref().take(steps.len()).collect())
                .collect();
            let jev = if store.is_some() {
                "cached Jev answers where the rules leave a step"
            } else {
                "no Jev answers read"
            };
            return (split, format!("phase rules over the whole trial; {jev}"));
        }
    }
    (
        alone,
        "phase rules over each session alone; no Jev answers read".to_owned(),
    )
}

/// The turn a call belongs to: the number of turns that arrived before it.
fn turn_of(session: &Session, at: i64) -> usize {
    session.turns.iter().filter(|turn| turn.at <= at).count()
}

/// When the turn that made a call arrived. A call's own time is when it
/// ended, so a long command's time is its end; the card places every moment
/// at the model turn that decided it.
fn turn_at(session: &Session, at: i64) -> i64 {
    match turn_of(session, at) {
        0 => at,
        turn => session.turns[turn - 1].at,
    }
}

/// The workspace files each call changed, relative to the session's
/// repository: an edit tool's paths inside it, and the files a shell
/// command wrote that the phase rules name. A file named like a test, such
/// as `statistical_tests.py`, is still the workspace's file here; the card
/// counts every change to the task's files as an edit.
fn edits_of(session: &Session, steps: Option<&Vec<Step>>) -> Vec<Vec<String>> {
    session
        .calls
        .iter()
        .enumerate()
        .map(|(index, call)| {
            let mut paths: Vec<String> = source::edited_paths(call)
                .iter()
                .map(|path| source::relative(path, &session.repository))
                .filter(|path| !path.starts_with('/'))
                .collect();
            if let Some(step) = steps.and_then(|steps| steps.get(index))
                && step.phase == Some(Phase::Edit)
            {
                for path in &step.writes {
                    let path = source::relative(path, &session.repository);
                    if !path.starts_with('/') && !paths.contains(&path) {
                        paths.push(path);
                    }
                }
            }
            paths
        })
        .collect()
}

fn runs_program(step: &Step) -> bool {
    matches!(step.kind, ActionKind::Command)
        && (step.executes || matches!(step.phase, Some(Phase::Test | Phase::Verify)))
}

/// The path of the session-written check, from the briefing: a backticked
/// path ending in the evaluator's file name.
fn check_path(brief: &str, names: &BTreeSet<String>) -> Option<String> {
    brief.split('`').skip(1).step_by(2).find_map(|quoted| {
        let quoted = quoted.trim();
        let name = quoted.rsplit('/').next()?;
        (quoted.starts_with('/') && !quoted.contains(' ') && names.contains(name))
            .then(|| quoted.to_owned())
    })
}

/// Whether a call writes `path`, and how.
fn writes_check(call: &crate::runs_analysis::Call, path: &str) -> Option<String> {
    if !call.ok {
        return None;
    }
    match call.name.as_str() {
        "write_file" | "apply_patch" | "edit_file" | "create_file" => source::edited_paths(call)
            .iter()
            .any(|written| written == path)
            .then(|| call.name.clone()),
        "run_command" => {
            let command = call.arguments["command"].as_str().unwrap_or_default();
            if !command.contains(path) {
                return None;
            }
            let redirect = [">", ">>", "> ", ">> ", "tee ", "tee -a "]
                .iter()
                .any(|head| command.contains(&format!("{head}{path}")));
            let script = (command.contains("open(")
                && (command.contains("'w'") || command.contains("\"w\"")))
                || command.contains("write_text(");
            if redirect {
                Some("shell redirect".to_owned())
            } else if script {
                Some("inline script".to_owned())
            } else {
                None
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Characterize
// ---------------------------------------------------------------------------

/// Characterizes one run from its trial directory and `options`' task-level
/// records.
#[must_use]
pub fn characterize(run: &Run, options: &Options) -> Card {
    // The card reads the trial directory, never the task's checkout.
    let mut run = run.clone();
    run.task_path = None;
    let records = Records::load(&run);
    let analysis = crate::runs_analysis::compute(&records, options.fable.as_ref());
    let mut card = Card {
        schema: SCHEMA.to_owned(),
        version: VERSION.to_owned(),
        ..Card::default()
    };
    let episode_dir = records.episode_dir.clone();
    let selection = episode_dir
        .as_deref()
        .map(read_selection)
        .unwrap_or_default();
    identity(&mut card, &run, &records, &analysis, options);
    phases(&mut card, &records);
    let (steps, placement) = if records.sessions.is_empty() {
        (Vec::new(), "no session log was retained".to_owned())
    } else {
        placed_steps(&run, &records.sessions, options.store.as_ref())
    };
    card.executed.placement = placement;
    sessions(&mut card, &records, &steps, &selection);
    waste(&mut card, &records, &steps);
    provenance(
        &mut card,
        &records,
        &steps,
        &selection,
        episode_dir.as_deref(),
        options,
    );
    checks(
        &mut card,
        &records,
        &steps,
        &selection,
        episode_dir.as_deref(),
    );
    executed(&mut card, &records, episode_dir.as_deref());
    reversals(&mut card, &selection, &analysis);
    review(&mut card, &selection);
    claims(&mut card, &records);
    reference(&mut card, &analysis, options);
    if records.sessions.is_empty() {
        card.notes.push(
            "No Microluna session log was retained, so the session anatomy, the check lineage, and the waste are unknown."
                .to_owned(),
        );
    }
    if records.episode.is_none() {
        card.notes.push(
            "No Coder One episode log was retained, so the host's phases and operations are unknown."
                .to_owned(),
        );
    }
    card
}

fn identity(
    card: &mut Card,
    run: &Run,
    records: &Records,
    analysis: &crate::runs_analysis::Analysis,
    options: &Options,
) {
    let attempt = run.files.attempt.as_deref().and_then(read_json);
    let manifest = records
        .episode_dir
        .as_ref()
        .and_then(|dir| read_json(&dir.join("manifest.json")));
    let policy = analysis
        .run
        .policy
        .clone()
        .or_else(|| manifest.as_ref().and_then(|m| text(m, "/policy/name")));
    let revision = attempt.as_ref().and_then(|a| {
        text(a, "/task/pin/git_commit_id")
            .or_else(|| text(a, "/task/git_commit_id"))
            .map(|commit| commit.chars().take(12).collect())
            .or_else(|| {
                text(a, "/task/checksum")
                    .map(|sum| format!("checksum {}", &sum[..sum.len().min(12)]))
            })
    });
    let trial_ms = analysis
        .spans
        .iter()
        .find(|span| span.span == "Trial")
        .map(|span| span.duration_ms);
    let (development, development_source) =
        development(&options.pins, policy.as_deref(), &run.task);
    card.identity = Identity {
        run: run.id(),
        task: run.task.clone(),
        revision,
        policy_digest: analysis
            .run
            .policy_digest
            .clone()
            .or_else(|| manifest.as_ref().and_then(|m| text(m, "/policy/digest"))),
        policy,
        binary: analysis.run.artifact.clone(),
        arm: attempt
            .as_ref()
            .and_then(|a| text(a, "/attempt/arm"))
            .or_else(|| run.job.split("--").nth(1).map(str::to_owned)),
        attempt_id: attempt.as_ref().and_then(|a| text(a, "/attempt/id")),
        attempt_kind: attempt.as_ref().and_then(|a| text(a, "/attempt/kind")),
        reward: run.reward,
        tests_passed: run.tests.map(|t| t.passed),
        tests_total: run.tests.map(|t| t.total),
        cost_usd: run.cost_usd,
        cost_estimated: run.cost_estimated,
        trial_ms,
        agent_ms: run.agent_ms,
        development,
        development_source,
    };
}

fn phases(card: &mut Card, records: &Records) {
    let result = records.result.as_ref();
    let at = |pointer: &str| {
        result
            .and_then(|r| text(r, pointer))
            .and_then(|t| timestamp_ms(&t))
    };
    let trial_start = at("/started_at");
    let trial_ms = card.identity.trial_ms;
    let mut rows: Vec<(String, String, Option<i64>, Option<i64>)> = Vec::new();
    rows.push((
        "environment_setup".into(),
        "Environment setup".into(),
        at("/environment_setup/started_at"),
        at("/environment_setup/finished_at"),
    ));
    rows.push((
        "agent_setup".into(),
        "Agent setup".into(),
        at("/agent_setup/started_at"),
        at("/agent_setup/finished_at"),
    ));
    let agent_start = at("/agent_execution/started_at");
    let agent_end = at("/agent_execution/finished_at");
    let sessions = &records.sessions;
    let episode = records.episode.as_ref();
    if sessions.is_empty() {
        rows.push((
            "agent_execution".into(),
            "Agent execution".into(),
            agent_start,
            agent_end,
        ));
    } else {
        let first = sessions[0].start;
        rows.push((
            "host_before_1".into(),
            "Host before session 1".into(),
            agent_start,
            Some(first),
        ));
        let last_end = sessions.last().map(|s| s.end).unwrap_or(first);
        let episode_end = episode.map(|e| e.end);
        // The last session's host work ends when the delegation that ran
        // the sessions returns, or at the closing check, whichever is first.
        let close_start = episode
            .into_iter()
            .flat_map(|e| &e.invocations)
            .filter_map(|invocation| match invocation.component.as_str() {
                "exec.session" => invocation.end.filter(|end| *end >= last_end),
                c if c.starts_with("verify.close") => {
                    Some(invocation.start).filter(|start| *start >= last_end)
                }
                _ => None,
            })
            .min()
            .or(episode_end);
        for (index, session) in sessions.iter().enumerate() {
            let number = index + 1;
            rows.push((
                format!("session_{number}"),
                format!("Session {number}"),
                Some(session.start),
                Some(session.end),
            ));
            let next = sessions.get(index + 1).map(|s| s.start).or(close_start);
            rows.push((
                format!("host_after_{number}"),
                format!("Host after session {number}"),
                Some(session.end),
                next,
            ));
        }
        rows.push(("close".into(), "Close".into(), close_start, episode_end));
        rows.push((
            "agent_exit".into(),
            "Agent exit".into(),
            episode_end,
            agent_end,
        ));
    }
    rows.push((
        "gap_to_verifier".into(),
        "Gap to verifier".into(),
        agent_end,
        at("/verifier/started_at"),
    ));
    rows.push((
        "verifier".into(),
        "Verifier".into(),
        at("/verifier/started_at"),
        at("/verifier/finished_at"),
    ));
    card.phases = rows
        .into_iter()
        .map(|(key, name, start, end)| {
            let duration_ms = ms_between(start, end);
            PhaseRow {
                key,
                name,
                start_ms: start.zip(trial_start).map(|(s, t)| s - t),
                duration_ms,
                share: duration_ms.zip(trial_ms).and_then(|(d, t)| share(d, t)),
            }
        })
        .collect();
    if result.is_none() {
        card.notes
            .push("No Harbor result was retained, so the trial's phases are unknown.".to_owned());
    }
}

fn session_role(session: &Session, number: usize, selection: &Selection) -> String {
    let group = session
        .id
        .strip_prefix("microluna-")
        .and_then(|rest| rest.split('-').next())
        .map(|g| format!("lean-{g}"));
    let snapshot = selection.snapshots.iter().find(|snapshot| {
        group.as_deref().is_none_or(|g| g == snapshot.group)
            && snapshot.after_session
                == session
                    .id
                    .rsplit('-')
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(number)
    });
    if snapshot.is_some_and(|s| s.self_check) || session.directive.contains("self-check") {
        "self-check".to_owned()
    } else if matches!(
        session.role,
        crate::runs_analysis::Role::Other | crate::runs_analysis::Role::Edit
    ) {
        "session".to_owned()
    } else {
        session.role.word().to_owned()
    }
}

fn sessions(card: &mut Card, records: &Records, steps: &[Vec<Step>], selection: &Selection) {
    let trial_start = records
        .result
        .as_ref()
        .and_then(|r| text(r, "/started_at"))
        .and_then(|t| timestamp_ms(&t));
    for (index, session) in records.sessions.iter().enumerate() {
        let number = index + 1;
        let duration_ms = u64::try_from(session.end - session.start).unwrap_or(0);
        let model_ms = session.model_ms();
        let tool_ms = session.tool_ms();
        let command_ms: u64 = session
            .calls
            .iter()
            .filter(|call| call.name == "run_command")
            .map(|call| call.ms)
            .sum();
        let input: u64 = session.turns.iter().map(|t| t.input).sum();
        let cached: u64 = session.turns.iter().map(|t| t.cached).sum();
        let since = |at: i64| u64::try_from(turn_at(session, at) - session.start).ok();
        let session_steps = steps.get(index);
        let mut card_session = SessionCard {
            number,
            id: session.id.clone(),
            role: session_role(session, number, selection),
            start_ms: trial_start.map(|t| session.start - t),
            duration_ms,
            turns: session.turns.len(),
            calls: session.calls.len(),
            model_ms,
            model_share: share(model_ms, duration_ms),
            command_ms,
            command_share: share(command_ms, duration_ms),
            tool_ms,
            overhead_ms: duration_ms.saturating_sub(model_ms + tool_ms),
            input_tokens: input,
            cached_tokens: cached,
            cached_share: share(cached, input),
            output_tokens: session.turns.iter().map(|t| t.output).sum(),
            reasoning_tokens: session.turns.iter().map(|t| t.reasoning).sum(),
            cost_usd: (!session.turns.is_empty()).then(|| session.cost_usd()),
            finish_status: session.finish.as_ref().map(|(status, _)| status.clone()),
            finish_summary: session
                .finish
                .as_ref()
                .map(|(_, summary)| clip(summary, 400)),
            ..SessionCard::default()
        };
        let calls = &session.calls;
        let edits = edits_of(session, session_steps);
        let reads = |i: usize| {
            calls[i].name == "read_file"
                || session_steps
                    .and_then(|steps| steps.get(i))
                    .is_some_and(|step| step.phase == Some(Phase::Read))
        };
        let edited: Vec<usize> = (0..calls.len()).filter(|&i| !edits[i].is_empty()).collect();
        card_session.moments = Moments {
            first_read_ms: (0..calls.len())
                .find(|&i| reads(i))
                .and_then(|i| since(calls[i].at)),
            first_command_ms: calls
                .iter()
                .find(|c| c.name == "run_command")
                .and_then(|c| since(c.at)),
            first_edit_ms: edited.first().and_then(|&i| since(calls[i].at)),
            last_edit_ms: edited.last().and_then(|&i| since(calls[i].at)),
            finish_ms: calls
                .iter()
                .find(|c| c.name == "finish")
                .and_then(|c| since(c.at)),
        };
        if let Some(steps) = session_steps {
            if let Some(&last) = edited.last() {
                let last_turn = turn_of(session, calls[last].at);
                let finish = calls.iter().position(|c| c.name == "finish");
                let after = &steps[last + 1..];
                card_session.tail = Some(Tail {
                    to_finish_ms: finish.and_then(|f| {
                        u64::try_from(
                            turn_at(session, calls[f].at) - turn_at(session, calls[last].at),
                        )
                        .ok()
                    }),
                    to_end_ms: u64::try_from(session.end - turn_at(session, calls[last].at))
                        .unwrap_or(0),
                    turns: session.turns.len().saturating_sub(last_turn),
                    runs: after.iter().filter(|s| runs_program(s)).count(),
                    score_runs: calls[last + 1..]
                        .iter()
                        .filter(|c| c.name == "run_command" && score_of(&c.output).is_some())
                        .count(),
                });
            }
            // Edit rounds: edits with no program run between them.
            let mut rounds: Vec<EditRound> = Vec::new();
            let mut open = false;
            for (i, step) in steps.iter().enumerate() {
                if !edits[i].is_empty() {
                    let turn = turn_of(session, calls[i].at);
                    if open {
                        let round = rounds.last_mut().expect("an open round");
                        round.last_turn = turn;
                        for file in &edits[i] {
                            if !round.files.contains(file) {
                                round.files.push(file.clone());
                            }
                        }
                    } else {
                        rounds.push(EditRound {
                            first_turn: turn,
                            last_turn: turn,
                            files: edits[i].clone(),
                            checked: false,
                        });
                        open = true;
                    }
                } else if runs_program(step) && open {
                    rounds.last_mut().expect("an open round").checked = true;
                    open = false;
                }
            }
            card_session.edit_rounds = rounds;
            for (i, step) in steps.iter().enumerate() {
                let phase = step.phase.map(|p| p.name().to_owned());
                *card_session
                    .phases
                    .entry(phase.clone().unwrap_or_else(|| "unplaced".to_owned()))
                    .or_default() += 1;
                match step.by {
                    "rule" => card_session.by_rule += 1,
                    "jev" => card_session.by_jev += 1,
                    _ => card_session.unplaced += 1,
                }
                if matches!(step.kind, ActionKind::Command) {
                    card_session.commands.push(SessionCommand {
                        turn: turn_of(session, calls[i].at),
                        tool: step.tool.clone(),
                        input: clip(&step.input, 120),
                        phase,
                        by: step.by.to_owned(),
                        exit: calls[i].exit,
                        ms: calls[i].ms,
                    });
                }
            }
        }
        card.sessions.push(card_session);
    }
}

fn waste(card: &mut Card, records: &Records, steps: &[Vec<Step>]) {
    let mut missing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut slow: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for (index, session) in records.sessions.iter().enumerate() {
        let number = index + 1;
        let mut with_call = BTreeSet::new();
        let briefed: BTreeSet<String> = source::briefing_files(&session.brief)
            .into_keys()
            .map(|path| source::relative(&path, &session.repository))
            .collect();
        let mut full = false;
        for (i, call) in session.calls.iter().enumerate() {
            let turn = turn_of(session, call.at);
            with_call.insert(turn);
            if call.name == "run_command"
                && let Some(program) = missing_program(&call.output)
            {
                let key = format!("{number}.{turn}");
                let turns = missing.entry(program).or_default();
                if !turns.contains(&key) {
                    turns.push(key);
                }
            }
            if !call.ok && call.exit.is_none() {
                card.waste.refused.push(Refused {
                    session: number,
                    turn,
                    tool: call.name.clone(),
                    cause: clip(call.output.lines().next().unwrap_or_default(), 160),
                });
            }
            if call.name == "read_file"
                && let Some(path) = call.arguments["path"].as_str()
            {
                let path = source::relative(path, &session.repository);
                if briefed.contains(&path) {
                    card.waste.briefing_rereads += 1;
                    if !card.waste.briefing_reread_paths.contains(&path) {
                        card.waste.briefing_reread_paths.push(path);
                    }
                }
            }
            let program = steps
                .get(index)
                .and_then(|steps| steps.get(i))
                .is_some_and(runs_program);
            let scored = score_of(&call.output);
            if program && full && scored.is_none_or(|(p, t)| p == t && t > 0) {
                card.waste.runs_after_full += 1;
            }
            if let Some((passed, total)) = scored {
                full = total > 0 && passed == total;
            }
            if call.name == "run_command" && call.ms >= SLOW_COMMAND_MS {
                let command = call.arguments["command"].as_str().unwrap_or_default();
                let key = clip(command.lines().next().unwrap_or_default(), 80);
                let entry = slow.entry(key).or_default();
                entry.0 += 1;
                entry.1 += call.ms;
                card.waste.slow_ms += call.ms;
            }
        }
        card.waste.turns_without_call += (1..=session.turns.len())
            .filter(|turn| !with_call.contains(turn))
            .count();
    }
    card.waste.not_found = missing
        .into_iter()
        .map(|(program, turns)| Missing { program, turns })
        .collect();
    let mut slow: Vec<Slow> = slow
        .into_iter()
        .map(|(command, (runs, ms))| Slow { command, runs, ms })
        .collect();
    slow.sort_by(|a, b| b.ms.cmp(&a.ms).then(a.command.cmp(&b.command)));
    card.waste.slow = slow;
}

/// The text of `path` in a retained workspace directory.
fn workspace_text(dir: Option<&Path>, path: &str) -> Option<String> {
    std::fs::read_to_string(dir?.join(path)).ok()
}

/// The submitted workspace: the lean loop's selected candidate, or the
/// graded copy Harbor kept.
fn submitted_dir(episode: Option<&Path>, selection: &Selection) -> Option<PathBuf> {
    let episode = episode?;
    selection
        .submitted
        .as_ref()
        .and_then(|(group, session)| {
            let dir = episode
                .join("artifacts")
                .join(group)
                .join(format!("session-{session}"));
            dir.is_dir().then_some(dir)
        })
        .or_else(|| {
            let produced = episode.join("produced/app");
            produced.is_dir().then_some(produced)
        })
}

fn provenance(
    card: &mut Card,
    records: &Records,
    steps: &[Vec<Step>],
    selection: &Selection,
    episode: Option<&Path>,
    options: &Options,
) {
    let repository = records
        .sessions
        .first()
        .map(|s| s.repository.clone())
        .unwrap_or_default();
    let mut edited: BTreeSet<String> = BTreeSet::new();
    for (index, session) in records.sessions.iter().enumerate() {
        for paths in edits_of(session, steps.get(index)) {
            edited.extend(paths);
        }
    }
    let untouched = source::untouched(&records.sessions);
    let submitted = submitted_dir(episode, selection);
    let pack = episode.and_then(|dir| read_json(&dir.join("artifacts/briefing-pack.json")));
    let items: Vec<&Value> = pack
        .as_ref()
        .and_then(|p| p.pointer("/record/items"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item["source"] == "file" && item["selected"] != false)
        .collect();
    let mut provenance = Provenance {
        untouched: untouched
            .iter()
            .map(|(path, (_, from))| (path.clone(), (*from).to_owned()))
            .collect(),
        ..Provenance::default()
    };
    for suspect in &selection.suspects {
        let file = source::relative(&suspect.file, &repository);
        let line_changed = suspect.line.and_then(|line| {
            let (old, _) = untouched.get(&file)?;
            let new = workspace_text(submitted.as_deref(), &file)?;
            Some(source::changed(old, &new)?.old.contains(&line))
        });
        provenance.suspects.push(Suspect {
            file_edited: edited.contains(&file),
            line_changed,
            file,
            ..suspect.clone()
        });
    }
    for path in &edited {
        let changed_lines = untouched.get(path).and_then(|(old, _)| {
            let new = workspace_text(submitted.as_deref(), path)?;
            let diff = source::changed(old, &new)?;
            Some(diff.old.len().max(diff.new.len()))
        });
        provenance.edited.push(EditedFile {
            path: path.clone(),
            file_item: items
                .iter()
                .find(|item| item["label"].as_str() == Some(path.as_str()))
                .map(|item| item["state"].as_str().unwrap_or("complete").to_owned()),
            suspects: provenance
                .suspects
                .iter()
                .filter(|s| &s.file == path)
                .count(),
            changed_lines,
        });
    }
    let suspects = &provenance.suspects;
    if !suspects.is_empty() {
        let known: Vec<bool> = suspects.iter().filter_map(|s| s.line_changed).collect();
        provenance.suspect_line_hits =
            (!known.is_empty()).then(|| (known.iter().filter(|hit| **hit).count(), known.len()));
        provenance.suspect_file_hits = Some((
            suspects.iter().filter(|s| s.file_edited).count(),
            suspects.len(),
        ));
    }
    if !edited.is_empty() && !records.sessions.is_empty() {
        let edited_files = &provenance.edited;
        provenance.pointer_coverage = selection.suspects.first().map(|_| {
            (
                edited_files.iter().filter(|f| f.suspects > 0).count(),
                edited_files.len(),
            )
        });
        provenance.briefing_coverage = pack.as_ref().map(|_| {
            (
                edited_files
                    .iter()
                    .filter(|f| f.suspects > 0 || f.file_item.is_some())
                    .count(),
                edited_files.len(),
            )
        });
    }
    if let Some(record) = options
        .sites
        .as_ref()
        .filter(|sites| sites["schema"] == SITES_SCHEMA)
        .and_then(|sites| sites.pointer(&format!("/tasks/{}", card.identity.task)))
    {
        provenance.sites_source = text(record, "/source");
        for site in record["sites"].as_array().into_iter().flatten() {
            let file = text(site, "/file").unwrap_or_default();
            provenance.sites.push(Site {
                id: text(site, "/id").unwrap_or_default(),
                named: provenance.suspects.iter().any(|s| s.file == file),
                edited: edited.contains(&file),
                what: text(site, "/what").unwrap_or_default(),
                file,
            });
        }
    }
    card.provenance = provenance;
}

fn checks(
    card: &mut Card,
    records: &Records,
    steps: &[Vec<Step>],
    selection: &Selection,
    episode: Option<&Path>,
) {
    let mut lineage = CheckLineage {
        frozen: selection
            .snapshots
            .iter()
            .map(|s| Frozen {
                group: s.group.clone(),
                after_session: s.after_session,
                self_check: s.self_check,
                score: s.score,
                kept: s.kept,
                evaluator_digest: s.evaluator_digest.clone(),
                hardcoded_p: s.hardcoded_p,
            })
            .collect(),
        ..CheckLineage::default()
    };
    (lineage.grades, lineage.grades_path) = episode.map(read_grades).unwrap_or_default();
    let names: BTreeSet<String> = episode
        .map(|dir| dir.join("artifacts"))
        .into_iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|entry| entry.path().join("evaluator"))
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .chain(std::iter::once("score.sh".to_owned()))
        .collect();
    lineage.path = records
        .sessions
        .iter()
        .find_map(|session| check_path(&session.brief, &names));
    let Some(path) = lineage.path.clone() else {
        card.checks = lineage;
        return;
    };
    let mut edited_since_version = false;
    let mut edited_ever = false;
    let mut last_score_full: Option<bool> = None;
    for (index, session) in records.sessions.iter().enumerate() {
        let number = index + 1;
        let edits = edits_of(session, steps.get(index));
        for (i, call) in session.calls.iter().enumerate() {
            let turn = turn_of(session, call.at);
            let at_ms = u64::try_from(turn_at(session, call.at) - session.start).unwrap_or(0);
            if let Some(how) = writes_check(call, &path) {
                lineage.versions.push(CheckVersion {
                    session: number,
                    turn,
                    at_ms,
                    how,
                    after_code_edit: !lineage.versions.is_empty() && edited_since_version,
                    after_failing_score: last_score_full == Some(false),
                    untouched_score: None,
                });
                edited_since_version = false;
            }
            if call.name == "run_command"
                && call.arguments["command"]
                    .as_str()
                    .is_some_and(|c| c.contains(&path))
                && let Some((passed, total)) = score_of(&call.output)
            {
                lineage.runs.push(ScoreRun {
                    session: number,
                    turn,
                    at_ms,
                    passed,
                    total,
                });
                last_score_full = Some(total > 0 && passed == total);
                if !edited_ever
                    && let Some(version) = lineage.versions.last_mut()
                    && version.untouched_score.is_none()
                {
                    version.untouched_score = Some((passed, total));
                }
            }
            if !edits[i].is_empty() {
                edited_since_version = true;
                edited_ever = true;
            }
        }
    }
    card.checks = lineage;
}

fn executed(card: &mut Card, records: &Records, episode: Option<&Path>) {
    if let Some(log) = &records.episode {
        for invocation in log
            .invocations
            .iter()
            .filter(|invocation| invocation.component == "host.operation")
        {
            let summary = &invocation.summary;
            card.executed.host_operations.push(HostOperation {
                label: invocation.name.clone(),
                outcome: invocation.outcome.clone(),
                exit: summary["exit"].as_i64(),
                ms: invocation
                    .end
                    .and_then(|end| u64::try_from(end - invocation.start).ok()),
                output_sha256: text(summary, "/sha256"),
                refused: summary["refused"].as_str().map(str::to_owned).or_else(|| {
                    summary["refused"]
                        .is_object()
                        .then(|| summary["refused"].to_string())
                }),
            });
        }
    }
    (
        card.executed.host_commands,
        card.executed.host_commands_path,
    ) = episode.map(read_executed).unwrap_or_default();
    for session in &card.sessions {
        for (phase, count) in &session.phases {
            *card
                .executed
                .session_phases
                .entry(phase.clone())
                .or_default() += count;
        }
    }
}

fn reversals(card: &mut Card, selection: &Selection, analysis: &crate::runs_analysis::Analysis) {
    card.reversals.patches = analysis.reversals.clone();
    let snapshots: Vec<&Snapshot> = selection
        .snapshots
        .iter()
        .filter(|s| s.files.is_some())
        .collect();
    if snapshots.is_empty() {
        return;
    }
    let mut found = Vec::new();
    for (k, later) in snapshots.iter().enumerate().skip(2) {
        let previous = snapshots[k - 1];
        for (file, digest) in later.files.as_ref().into_iter().flatten() {
            if previous.files.as_ref().and_then(|f| f.get(file)) == Some(digest) {
                continue;
            }
            if let Some(earlier) = snapshots[..k - 1]
                .iter()
                .rev()
                .find(|s| s.files.as_ref().and_then(|f| f.get(file)) == Some(digest))
            {
                found.push(DigestReturn {
                    file: file.clone(),
                    at_session: later.after_session,
                    matches_session: earlier.after_session,
                    digest: digest.chars().take(12).collect(),
                });
            }
        }
    }
    card.reversals.between_sessions = Some(found);
}

fn review(card: &mut Card, selection: &Selection) {
    let executed = card.executed.host_commands.clone();
    for session in &card.sessions {
        if session.role != "self-check" && session.role != "audit" {
            continue;
        }
        let group = session
            .id
            .strip_prefix("microluna-")
            .and_then(|rest| rest.split('-').next())
            .map(|g| format!("lean-{g}"));
        let find = |after: usize| {
            selection
                .snapshots
                .iter()
                .find(|s| s.after_session == after && group.as_deref().is_none_or(|g| g == s.group))
        };
        let after_number = session
            .id
            .rsplit('-')
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(session.number);
        let (Some(after), Some(before)) =
            (find(after_number), find(after_number.saturating_sub(1)))
        else {
            continue;
        };
        let (Some(new), Some(old)) = (&after.files, &before.files) else {
            continue;
        };
        let paths: BTreeSet<&String> = new.keys().chain(old.keys()).collect();
        let mut delta = ReviewDelta {
            session: session.number,
            role: session.role.clone(),
            score_before: before.score,
            score_after: after.score,
            ..ReviewDelta::default()
        };
        for path in paths {
            let (was, now) = (old.get(path), new.get(path));
            if was == now {
                continue;
            }
            delta.files.push((path.clone(), was.cloned(), now.cloned()));
            let old_text = workspace_text(before.candidate.as_deref(), path);
            let new_text = workspace_text(after.candidate.as_deref(), path);
            if let (Some(old_text), Some(new_text)) = (old_text, new_text)
                && let Some(change) = source::classify(path, &old_text, &new_text)
            {
                delta.changes.push(change);
            }
        }
        if let Some(records) = &executed {
            let of = |n: usize| -> Vec<Value> {
                records
                    .iter()
                    .filter(|r| r["session"].as_u64() == Some(n as u64))
                    .cloned()
                    .collect()
            };
            delta.executed_before = Some(of(before.after_session));
            delta.executed_after = Some(of(after.after_session));
        }
        card.review.push(delta);
    }
}

fn claims(card: &mut Card, records: &Records) {
    let close_p = records.episode.as_ref().and_then(|log| {
        log.invocations
            .iter()
            .filter(|i| i.component == "verify.close")
            .find_map(|i| i.summary["done"].as_f64())
    });
    let final_score = card
        .checks
        .frozen
        .iter()
        .rev()
        .find(|f| f.kept != Some(false))
        .and_then(|f| f.score);
    let reward = card.identity.reward;
    let lines = card.checks.grades.as_ref().map(|grades| {
        grades["lines"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|line| {
                let passed = line["results"]
                    .as_array()
                    .and_then(|results| results.last())
                    .and_then(|result| result["passed"].as_bool());
                json!({
                    "id": line["id"],
                    "text": line["text"],
                    "grade": line["grade"],
                    "passed": passed,
                    "agrees": passed.zip(reward).map(|(passed, reward)| passed == (reward >= 1.0)),
                })
            })
            .collect()
    });
    card.claims = Claims {
        reward,
        tests_passed: card.identity.tests_passed,
        tests_total: card.identity.tests_total,
        final_score,
        self_score_agrees: final_score
            .zip(reward)
            .map(|((passed, total), reward)| (total > 0 && passed == total) == (reward >= 1.0)),
        close_p,
        lines,
    };
}

fn reference(card: &mut Card, analysis: &crate::runs_analysis::Analysis, options: &Options) {
    let Some(fable) = &analysis.fable else {
        return;
    };
    let Some(cheapest) = &fable.cheapest_pass else {
        return;
    };
    let mut reference = Reference {
        id: cheapest.id.clone(),
        effort: cheapest.effort.clone(),
        cost_usd: cheapest.cost_usd,
        seconds: cheapest.seconds,
        steps: cheapest.steps,
        attempts: fable.attempts,
        passes: fable.passes,
        ..Reference::default()
    };
    if options.reference_body {
        let cache = crate::runs_replay::cache_dir();
        let manifest = cache.join("manifest.json");
        let manifest = if manifest.is_file() {
            manifest
        } else {
            repository_root().join("bench/terminal-bench/reference/fable-5.1-replays.json")
        };
        if let Ok(public) = crate::runs_replay::public_sources(&cache, &manifest)
            && let Some(found) = public.iter().find(|s| s.id() == cheapest.id)
            && let Ok(loaded) = crate::runs_fingerprint::load(found)
        {
            let print = crate::runs_fingerprint::fingerprint(
                found,
                &loaded.steps,
                &loaded.task,
                loaded.duration_ms,
            );
            reference.first_edit_ms = print.first_edit_ms;
            reference.edit_runs = Some(print.edit_runs);
            reference.sequence = Some(print.sequence);
        }
    }
    card.reference = Some(reference);
}

#[cfg(test)]
#[path = "runs_card_tests.rs"]
mod tests;
