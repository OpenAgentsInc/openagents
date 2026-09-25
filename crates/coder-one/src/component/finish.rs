//! `control.finish` as a component, and its offline replay over retained
//! session logs (issue #9638).
//!
//! A fixture holds a session's edits and runs as [`Mark`]s, the rule, and
//! the finish; the component folds them into a [`microluna::finish::Ledger`]
//! and returns the [`microluna::finish::Verdict`], so the rule runs alone,
//! in milliseconds, on the same code the session loop calls.
//!
//! [`replay`] reads every retained Microluna session log and every
//! retained Luna Codex stream under the traces it is given, finds each
//! `done` finish, and asks whether the rule would have refused it given
//! the session's history up to that point. Two populations, two rules:
//!
//! - **Microluna** sessions ran the lean loop, whose evaluation script is
//!   `score.sh`: the rule as it ships, with no baseline commands, since
//!   none are found yet (issue #9633).
//! - **Luna in Codex** sessions had no host score. The replay uses a
//!   proxy: any command that runs code ([`runs_code`]) counts as the score
//!   run. Reading, writing a file through a here-document, and moving
//!   files don't run code; an interpreter, a test runner, a build, or a
//!   script does.
//!   A Codex session has no typed finish, so a session that ended with a
//!   final message and no error counts as one `done` finish.
//!
//! An edit is a tool edit (`apply_patch`, `write_file`, or Codex's
//! `file_change`), a here-document a command wrote into a file
//! ([`crate::stream::shell_writes`]), or a `sed -i` or `perl -i` edit
//! ([`in_place_edits`]). A shell write under `/tmp`, under a hidden
//! directory, or into `__pycache__` isn't an edit, as the live listing
//! leaves those out. Other shell edits, such as a Python program that
//! writes a source file, aren't visible in the retained logs, so the
//! counts are lower bounds on what the live rule, which lists the
//! workspace around each command, sees.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use futures_util::future::LocalBoxFuture;
use microluna::FinishRule;
use microluna::finish::{Ledger, Run, Verdict};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::jev::JevMode;
use super::{Component, Fixture, Ran, input};
use crate::record::{Implementation, Recorder};
use crate::stream::{self, Format, Kind};

/// The component ID.
pub const COMPONENT: &str = "control.finish";

/// The schema of a replay's rows.
pub const ROW_SCHEMA: &str = "openagents.coder-one.finish-row.v1";

/// The schema of a replay's summary.
pub const SUMMARY_SCHEMA: &str = "openagents.coder-one.finish-summary.v1";

/// Tasks the replay never reads: the prospective cohort of issue #9584,
/// whose outcomes that protocol seals.
pub const SEALED: [&str; 8] = [
    "distributed-dedup",
    "formal-crypto",
    "freecad-impeller",
    "freecad-spring-clip",
    "math-eval-grader",
    "pretrain-shard-corruption",
    "shadow-relay",
    "vpp-loss-divergence",
];

/// One thing a session did, as the rule counts it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    /// A file edit.
    Edit { path: String },
    /// A command, with the files it changed.
    Ran {
        command: String,
        #[serde(default)]
        changed: Vec<String>,
    },
}

/// A fixture's input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinishInput {
    pub rule: FinishRule,
    pub marks: Vec<Mark>,
    pub status: microluna::FinishStatus,
    /// Refusals earlier in the session.
    #[serde(default)]
    pub refusals: u32,
}

/// Folds `marks` into a ledger under `rule`.
#[must_use]
pub fn ledger(rule: &FinishRule, marks: &[Mark]) -> Ledger {
    let mut ledger = Ledger::default();
    for mark in marks {
        match mark {
            Mark::Edit { path } => ledger.edited(path),
            Mark::Ran { command, changed } => {
                ledger.ran(rule, command, changed);
            }
        }
    }
    ledger
}

/// A verdict's word: `allowed`, `refused`, or `unverified`.
#[must_use]
pub fn word(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Allowed => "allowed",
        Verdict::Refused(_) => "refused",
        Verdict::Unverified(_) => microluna::finish::UNVERIFIED,
    }
}

/// The implementation this build runs.
#[must_use]
pub fn implementation() -> Implementation {
    Implementation::new(
        COMPONENT,
        "finish-rule-v1",
        &json!({
            "max_refusals": microluna::finish::MAX_REFUSALS,
            "listing_max": microluna::finish::LISTING_MAX,
            "score": crate::micro::lean::SCORE_NAME,
        }),
    )
}

/// `control.finish`.
pub struct FinishComponent;

impl Component for FinishComponent {
    fn id(&self) -> &'static str {
        COMPONENT
    }
    fn implementation(&self) -> Implementation {
        implementation()
    }
    fn about(&self) -> &'static str {
        "Code refuses a done finish until the score and a baseline command ran after the last edit."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        _recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: FinishInput = input(fixture)?;
            let ledger = ledger(&input.rule, &input.marks);
            let verdict =
                microluna::finish::verdict(&ledger, &input.rule, input.status, input.refusals);
            let text = match &verdict {
                Verdict::Allowed => String::new(),
                Verdict::Refused(text) | Verdict::Unverified(text) => text.clone(),
            };
            let mut metrics = Map::new();
            metrics.insert("verdict".to_string(), json!(word(&verdict)));
            if let Some(want) = fixture.retained.get("verdict").and_then(Value::as_str) {
                let names = fixture
                    .retained
                    .get("names")
                    .and_then(Value::as_str)
                    .is_none_or(|name| text.contains(&format!("`{name}`")));
                metrics.insert(
                    "matches_retained".to_string(),
                    json!(want == word(&verdict) && names),
                );
            }
            Ok(Ran {
                output: json!({
                    "verdict": word(&verdict),
                    "message": text,
                    "ledger": ledger,
                }),
                metrics,
            })
        })
    }
}

/// A session's history and its finishes, in order.
#[derive(Clone, Debug, Default)]
struct Session {
    /// Each item is a mark or a finish (`Some(done)`).
    items: Vec<Item>,
}

#[derive(Clone, Debug)]
enum Item {
    Mark(Mark),
    /// A command already classified, with the edits that came before it.
    Classified(Run),
    Finish {
        done: bool,
    },
}

/// Whether a shell write's `path` is one the live listing would see.
fn listed(path: &str) -> bool {
    let path = path.trim();
    !(path.is_empty()
        || path.starts_with("/tmp/")
        || path.starts_with("/dev/")
        || path.starts_with("/proc/")
        || path.ends_with(".pyc")
        || path.split('/').any(|part| {
            part.starts_with('.') && part != "." && part != ".." || part == "__pycache__"
        }))
}

/// Programs that read, write, or move files without running the code
/// under test.
const NO_RUN: [&str; 40] = [
    "cat", "ls", "head", "tail", "grep", "rg", "find", "wc", "nl", "tree", "pwd", "stat", "file",
    "du", "sort", "uniq", "cut", "awk", "echo", "printf", "which", "type", "cd", "true", "false",
    "sed", "mkdir", "rm", "cp", "mv", "chmod", "touch", "tee", "git", "diff", "test", "[", "ln",
    "sleep", "export",
];

/// Words a piece can start with before its program.
const PREFIXES: [&str; 8] = ["sudo", "env", "then", "do", "else", "if", "!", "time"];

/// Whether a shell script runs code: some piece of it, here-document
/// bodies left out, starts with a program outside [`NO_RUN`]. A
/// here-document fed to an interpreter (`python3 - <<'PY'`) runs code.
#[must_use]
pub fn runs_code(script: &str) -> bool {
    let mut residual = script.to_string();
    for doc in stream::heredocs(script) {
        residual = residual.replacen(&format!("{}{}", doc.body, doc.tag), "", 1);
    }
    residual
        .split(['|', ';', '&', '\n', '(', ')'])
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .any(|piece| {
            let first = piece
                .split_whitespace()
                .find(|word| !word.contains('=') && !PREFIXES.contains(word))
                .unwrap_or_default();
            let first = first.rsplit('/').next().unwrap_or(first);
            !first.is_empty()
                && !first.starts_with(['>', '<', '#'])
                && !matches!(first, "fi" | "done" | "for" | "while")
                && !NO_RUN.contains(&first)
        })
}

/// The files a script edits in place with `sed -i` or `perl -i`.
#[must_use]
pub fn in_place_edits(script: &str) -> Vec<String> {
    let words = stream::shell_words(script);
    let mut files = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let program = words[i].rsplit('/').next().unwrap_or_default();
        if !matches!(program, "sed" | "perl") {
            i += 1;
            continue;
        }
        let mut in_place = false;
        let mut script_given = false;
        let mut operands = Vec::new();
        let mut j = i + 1;
        while j < words.len() {
            let word = words[j].as_str();
            if matches!(word, "&&" | "||" | "|" | ";" | "&") {
                break;
            }
            let last = word.ends_with(';');
            let word = word.trim_end_matches(';');
            if word.starts_with('-') && word.len() > 1 {
                in_place |= word.starts_with("-i") || (program == "perl" && word.contains('i'));
                if word == "-e" || (program == "perl" && word.ends_with('e')) {
                    script_given = true;
                    j += 1;
                }
            } else {
                operands.push(word.to_string());
            }
            j += 1;
            if last {
                break;
            }
        }
        if in_place {
            files.extend(operands.into_iter().skip(usize::from(!script_given)));
        }
        i = j;
    }
    files
}

/// A Microluna session log's history.
fn microluna_session(text: &str) -> Session {
    let mut session = Session::default();
    for line in text.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if record["record"] != "step" {
            continue;
        }
        let Ok(step) = serde_json::from_value::<atif::document::Step>(record["step"].clone())
        else {
            continue;
        };
        let Some(call) = &step.call else {
            continue;
        };
        match call.name.as_str() {
            "run_command" if call.outcome != atif::Outcome::Cancelled => {
                let command = call
                    .arguments
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let written = stream::shell_writes(&command).into_iter().map(|w| w.path);
                for path in written.chain(in_place_edits(&command)) {
                    if listed(&path) {
                        session.items.push(Item::Mark(Mark::Edit { path }));
                    }
                }
                session.items.push(Item::Mark(Mark::Ran {
                    command,
                    changed: Vec::new(),
                }));
            }
            "apply_patch" | "write_file" => {
                for kind in crate::micro::events_of(&step) {
                    if let Kind::ArtifactChanged { path, .. } = kind {
                        session.items.push(Item::Mark(Mark::Edit { path }));
                    }
                }
            }
            "finish" => session.items.push(Item::Finish {
                done: call.arguments.get("status").and_then(Value::as_str) == Some("done"),
            }),
            _ => {}
        }
    }
    session
}

/// A Luna Codex stream's history under the proxy rule: a command that
/// runs code is the score run, or, `loose`, any command that doesn't only
/// read ([`microluna::tools::reads_only`]).
fn codex_session(text: &str, loose: bool) -> Session {
    let events = stream::normalize(Format::Codex, text);
    let mut session = Session::default();
    let mut ended_badly = false;
    let mut last_claim = false;
    let mut i = 0;
    while i < events.len() {
        let event = &events[i];
        match &event.kind {
            Kind::CommandCompleted { command, .. } => {
                let mut j = i + 1;
                while j < events.len()
                    && events[j].line == event.line
                    && let Kind::ArtifactChanged { path, .. } = &events[j].kind
                {
                    if listed(path) {
                        session
                            .items
                            .push(Item::Mark(Mark::Edit { path: path.clone() }));
                    }
                    j += 1;
                }
                let script = stream::unwrap_shell(command);
                for path in in_place_edits(&script) {
                    if listed(&path) {
                        session.items.push(Item::Mark(Mark::Edit { path }));
                    }
                }
                let runs = if loose {
                    !microluna::tools::reads_only(
                        "run_command",
                        &json!({ "command": script }).to_string(),
                    )
                } else {
                    runs_code(&script)
                };
                session
                    .items
                    .push(Item::Classified(if runs { Run::Score } else { Run::Other }));
                last_claim = false;
                i = j;
                continue;
            }
            Kind::ArtifactChanged { path, .. } => {
                session
                    .items
                    .push(Item::Mark(Mark::Edit { path: path.clone() }));
                last_claim = false;
            }
            Kind::AssistantClaim { .. } => last_claim = true,
            Kind::SessionEnded { error: true, .. } => ended_badly = true,
            _ => {}
        }
        i += 1;
    }
    if last_claim || !session.items.is_empty() {
        session.items.push(Item::Finish {
            done: last_claim && !ended_badly,
        });
    }
    session
}

/// One `done` finish, judged.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub schema: String,
    pub family: String,
    pub job: String,
    pub trial: String,
    pub task: String,
    pub session: String,
    /// Which `done` finish of the session, from 1.
    pub finish: usize,
    pub refused: bool,
    /// For a Codex session, whether the loose proxy, where any command
    /// that doesn't only read is the score run, would refuse it too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused_loose: Option<bool>,
    /// What the refusal would have said.
    pub missing: Option<String>,
    pub last_edit: Option<String>,
    pub edits: usize,
    pub score_runs: usize,
    /// The verifier's reward for the trial, when known.
    pub reward: Option<f64>,
}

fn judge(session: &Session, rule: &FinishRule, base: &Row, rows: &mut Vec<Row>) {
    let mut ledger = Ledger::default();
    let (mut edits, mut scores, mut finishes) = (0, 0, 0);
    for item in &session.items {
        match item {
            Item::Mark(Mark::Edit { path }) => {
                edits += 1;
                ledger.edited(path);
            }
            Item::Mark(Mark::Ran { command, changed }) => {
                if ledger.ran(rule, command, changed) != Run::Other {
                    scores += 1;
                }
            }
            Item::Classified(run) => {
                if *run != Run::Other {
                    scores += 1;
                }
                ledger.ran_as(*run, &[]);
            }
            Item::Finish { done: true } => {
                finishes += 1;
                let missing = ledger.missing(rule);
                rows.push(Row {
                    finish: finishes,
                    refused: missing.is_some(),
                    missing,
                    last_edit: ledger.last_edit.as_ref().map(|(_, p)| p.clone()),
                    edits,
                    score_runs: scores,
                    ..base.clone()
                });
            }
            Item::Finish { done: false } => {}
        }
    }
}

fn reward(dir: &Path) -> Option<f64> {
    if let Ok(text) = std::fs::read_to_string(dir.join("verifier/reward.txt")) {
        return text.trim().parse().ok();
    }
    let text = std::fs::read_to_string(dir.join("harbor-result.json")).ok()?;
    serde_json::from_str::<Value>(&text)
        .ok()?
        .pointer("/verifier_result/rewards/reward")
        .and_then(Value::as_f64)
}

/// Every trial directory (`*.episode`) under `root`.
fn episodes(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.extension().is_some_and(|e| e == "episode") {
                out.push(path);
            } else if depth < 3 {
                pending.push((path, depth + 1));
            }
        }
    }
    out.sort();
    out
}

/// What the replay read and why it left things out.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sources {
    pub microluna_logs: usize,
    pub codex_streams: usize,
    pub excluded_sealed: usize,
    pub excluded_truth: usize,
    /// Luna trials whose native stream wasn't retained.
    pub luna_without_stream: Vec<String>,
}

/// Judges every `done` finish under `roots`.
#[must_use]
pub fn replay(roots: &[PathBuf]) -> (Vec<Row>, Sources) {
    let microluna_rule = FinishRule::score(&[crate::micro::lean::SCORE_NAME]);
    let mut rows = Vec::new();
    let mut sources = Sources::default();
    for root in roots {
        for episode in episodes(root) {
            let job = episode
                .parent()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let trial = episode
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let task = trial.split("__").next().unwrap_or_default().to_string();
            if job.contains("truth-confirmation") || job.contains("truth-control") {
                sources.excluded_truth += 1;
                continue;
            }
            if SEALED.contains(&task.as_str()) {
                sources.excluded_sealed += 1;
                continue;
            }
            let arm = job.split("--").nth(1).unwrap_or_default();
            let base = Row {
                schema: ROW_SCHEMA.to_string(),
                family: String::new(),
                job: job.clone(),
                trial: trial.clone(),
                task,
                session: String::new(),
                finish: 0,
                refused: false,
                refused_loose: None,
                missing: None,
                last_edit: None,
                edits: 0,
                score_runs: 0,
                reward: reward(&episode),
            };
            let mut files: Vec<PathBuf> = std::fs::read_dir(episode.join("artifacts"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            files.sort();
            for path in &files {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with("microluna-") && name.ends_with(".atif.jsonl") {
                    let Ok(text) = std::fs::read_to_string(path) else {
                        continue;
                    };
                    sources.microluna_logs += 1;
                    let base = Row {
                        family: "microluna".to_string(),
                        session: name.trim_end_matches(".atif.jsonl").to_string(),
                        ..base.clone()
                    };
                    judge(&microluna_session(&text), &microluna_rule, &base, &mut rows);
                }
            }
            if !arm.contains("luna") || arm.contains("microluna") {
                continue;
            }
            let mut streams: Vec<(String, PathBuf)> = files
                .iter()
                .filter(|p| p.to_string_lossy().ends_with(".stream.jsonl"))
                .map(|p| {
                    (
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        p.clone(),
                    )
                })
                .collect();
            let native = episode.join("native/codex.txt");
            if native.is_file() {
                streams.push(("codex.txt".to_string(), native));
            }
            let mut any = false;
            for (name, path) in streams {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if Format::detect(&text) != Some(Format::Codex) {
                    continue;
                }
                any = true;
                sources.codex_streams += 1;
                let base = Row {
                    family: "luna-codex".to_string(),
                    session: name,
                    ..base.clone()
                };
                // The proxy classifies each command itself; the rule only
                // supplies the default bound.
                let mut strict = Vec::new();
                judge(
                    &codex_session(&text, false),
                    &microluna_rule,
                    &base,
                    &mut strict,
                );
                let mut loose = Vec::new();
                judge(
                    &codex_session(&text, true),
                    &microluna_rule,
                    &base,
                    &mut loose,
                );
                for (mut row, other) in strict.into_iter().zip(loose) {
                    row.refused_loose = Some(other.refused);
                    rows.push(row);
                }
            }
            if !any {
                sources.luna_without_stream.push(format!("{job}/{trial}"));
            }
        }
    }
    (rows, sources)
}

/// The jobs of the Luna TB4 baseline (issue #9583), counted apart too.
pub const BASELINE_COHORT: &str = "luna-tb4-9583";

/// A 95% Wilson interval for `k` of `n`.
#[must_use]
pub fn wilson(k: usize, n: usize) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let z = 1.959_964_f64;
    let (k, n) = (k as f64, n as f64);
    let p = k / n;
    let d = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / d;
    let h = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    Some(((c - h).max(0.0), (c + h).min(1.0)))
}

fn share(k: usize, n: usize) -> Value {
    json!({
        "k": k,
        "n": n,
        "rate": (n > 0).then(|| k as f64 / n as f64),
        "wilson95": wilson(k, n).map(|(lo, hi)| [lo, hi]),
    })
}

/// The counts the issue asks for, by family and in all.
#[must_use]
pub fn summary(rows: &[Row], sources: &Sources) -> Value {
    let mut families: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for row in rows {
        families.entry(row.family.clone()).or_default().push(row);
        families.entry("all".to_string()).or_default().push(row);
        if row.job.contains(BASELINE_COHORT) {
            families
                .entry(format!("{}-{BASELINE_COHORT}", row.family))
                .or_default()
                .push(row);
        }
    }
    let by_family: Map<String, Value> = families
        .into_iter()
        .map(|(family, rows)| {
            let graded: Vec<&Row> = rows
                .iter()
                .copied()
                .filter(|r| r.reward.is_some())
                .collect();
            let failed = |r: &Row| r.reward.is_some_and(|x| x < 1.0);
            let refused = graded.iter().filter(|r| r.refused).count();
            let refused_failed = graded.iter().filter(|r| r.refused && failed(r)).count();
            let allowed_failed = graded.iter().filter(|r| !r.refused && failed(r)).count();
            let failed_n = graded.iter().filter(|r| failed(r)).count();
            let loose: Vec<&Row> = graded
                .iter()
                .copied()
                .filter(|r| r.refused_loose.is_some())
                .collect();
            let loose_refused = loose
                .iter()
                .filter(|r| r.refused_loose == Some(true))
                .count();
            let loose_refused_failed = loose
                .iter()
                .filter(|r| r.refused_loose == Some(true) && failed(r))
                .count();
            // Trials: refused when any of their done finishes would be.
            let mut trials: BTreeMap<&str, (bool, Option<f64>)> = BTreeMap::new();
            for row in &rows {
                let entry = trials.entry(&row.trial).or_insert((false, row.reward));
                entry.0 |= row.refused;
            }
            let graded_trials: Vec<_> = trials.values().filter(|(_, r)| r.is_some()).collect();
            let trial_refused = graded_trials.iter().filter(|(r, _)| *r).count();
            let trial_refused_failed = graded_trials
                .iter()
                .filter(|(r, x)| *r && x.is_some_and(|x| x < 1.0))
                .count();
            let trial_failed = graded_trials
                .iter()
                .filter(|(_, x)| x.is_some_and(|x| x < 1.0))
                .count();
            (
                family,
                json!({
                    "done_finishes": rows.len(),
                    "graded_done_finishes": graded.len(),
                    "ungraded_done_finishes": rows.len() - graded.len(),
                    "refused_of_done": share(refused, graded.len()),
                    "verifier_failed_of_refused": share(refused_failed, refused),
                    "verifier_failed_of_allowed": share(allowed_failed, graded.len() - refused),
                    "refused_of_verifier_failed": share(refused_failed, failed_n),
                    "loose_refused_of_done": share(loose_refused, loose.len()),
                    "loose_verifier_failed_of_refused": share(loose_refused_failed, loose_refused),
                    "trials": graded_trials.len(),
                    "trials_refused": share(trial_refused, graded_trials.len()),
                    "trials_verifier_failed_of_refused": share(trial_refused_failed, trial_refused),
                    "trials_refused_of_verifier_failed": share(trial_refused_failed, trial_failed),
                }),
            )
        })
        .collect();
    json!({
        "schema": SUMMARY_SCHEMA,
        "implementation": implementation(),
        "sources": sources,
        "families": by_family,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_codex_heredoc_then_a_run_counts_the_run_after_the_write() {
        let stream = [
            json!({"type": "thread.started", "thread_id": "t"}),
            json!({"type": "item.completed", "item": {"id": "1", "type": "command_execution",
                "command": "/bin/bash -lc \"cat > /app/fix.py <<'PY'\nprint(1)\nPY\npython3 /app/fix.py\"",
                "aggregated_output": "1\n", "exit_code": 0, "status": "completed"}}),
            json!({"type": "item.completed", "item": {"id": "2", "type": "agent_message", "text": "Done."}}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
        let session = codex_session(&stream, false);
        let mut rows = Vec::new();
        judge(
            &session,
            &FinishRule::score(&["score.sh"]),
            &Row {
                schema: ROW_SCHEMA.to_string(),
                family: "luna-codex".to_string(),
                job: String::new(),
                trial: String::new(),
                task: String::new(),
                session: String::new(),
                finish: 0,
                refused: false,
                refused_loose: None,
                missing: None,
                last_edit: None,
                edits: 0,
                score_runs: 0,
                reward: None,
            },
            &mut rows,
        );
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].refused, "{rows:?}");
        assert_eq!(rows[0].edits, 1);
    }

    #[test]
    fn writing_and_reading_run_no_code_and_an_interpreter_does() {
        assert!(!runs_code(
            "cat > /app/fix.py <<'PY'\nimport os\nos.system('x')\nPY"
        ));
        assert!(!runs_code("sed -n 1,80p app.py && ls -la"));
        assert!(!runs_code("mkdir -p out && cp a b"));
        assert!(runs_code("cat > t.py <<'PY'\nprint(1)\nPY\npython3 t.py"));
        assert!(runs_code("python3 - <<'PY'\nprint(1)\nPY"));
        assert!(runs_code("cd /app && PYTHONPATH=. pytest -q"));
        assert!(runs_code("./run.sh"));
    }

    #[test]
    fn in_place_edits_name_their_files() {
        assert_eq!(
            in_place_edits("sed -i 's/a;b/c/' app/x.py app/y.py && python3 t.py"),
            ["app/x.py", "app/y.py"]
        );
        assert_eq!(in_place_edits("sed -i -e 's/a/b/' x.py"), ["x.py"]);
        assert_eq!(in_place_edits("perl -pi -e 's/a/b/' x.py"), ["x.py"]);
        assert!(in_place_edits("sed -n 1,20p x.py").is_empty());
    }

    #[test]
    fn shell_writes_to_scratch_and_hidden_paths_are_not_edits() {
        assert!(listed("/app/fix.py"));
        assert!(listed("fix.py"));
        assert!(!listed("/tmp/microluna-eval-1/score.sh"));
        assert!(!listed(".microluna-eval/score.sh"));
        assert!(!listed("app/__pycache__/x.cpython-312.pyc"));
    }

    #[test]
    fn wilson_matches_a_known_interval() {
        let (lo, hi) = wilson(0, 23).unwrap();
        assert!(lo.abs() < 1e-9 && (hi - 0.1431).abs() < 1e-3, "{hi}");
    }
}
