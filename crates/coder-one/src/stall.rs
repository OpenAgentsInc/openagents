//! Stall, loop, and done detection, and a typed next-step choice, over a
//! Microluna session's events (issue #9627, algorithms 4 and 5 of
//! `docs/coder/design/luna-pivot.md`).
//!
//! A [`Checkpoint`] is a point in a Microluna dispatch: the end of a
//! session, or every [`EVERY`] turns inside one. Code reads the events
//! before it and computes [`Features`]: turns since the last edit, turns
//! since the score last rose, repeated failures, and finishes the host
//! turned back without an edit. Only when those features make a stall
//! plausible ([`Features::suspect`]) does Jev's answer matter. One Jev
//! request over the last [`WINDOW`] events asks three Nouls, whether the
//! session repeats a failed step, whether it made progress toward the
//! task's core step, and whether the task already looks done, and one
//! Choice over the next steps code proposes ([`candidates`]).
//!
//! Code decides what happens ([`decide`], [`action`]): the first stall
//! re-briefs the next session with the evidence, and a stall right after
//! a re-brief stops the loop's work sessions. The done answer is recorded
//! and never acted on: a green self-score may not certify completion
//! (#9584). The next-step pick becomes a suggestion in the next brief;
//! Luna still writes every edit.
//!
//! The older live monitor ([`crate::monitor`]) stopped working sessions
//! 12 to 30 seconds in, on a stall flag that replays had found right 0 of
//! 22 times. This detector differs on purpose: it needs [`MIN_TURNS`] turns
//! of evidence and a code-side feature before Jev is consulted, it acts
//! only between sessions, and its first action is a re-brief, not a stop.
//! [`hindsight`] labels a checkpoint from what happened after it, for the
//! offline measurement in `docs/terminal-bench/`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The stall component's ID.
pub const STALL_COMPONENT: &str = "control.stall";

/// The next-step component's ID.
pub const NEXT_COMPONENT: &str = "control.next";

/// The Jev decision's name.
pub const DECISION: &str = "jev_stall";

/// The question set's revision. Any change to a question's wording is a
/// new revision, and misses every recorded answer.
pub const QUESTION_SET: &str = "stall-v1";

/// Events in a checkpoint's window.
pub const WINDOW: usize = 12;

/// Turns between checkpoints inside a session.
pub const EVERY: usize = 8;

/// A session turn before which no in-session checkpoint is taken.
pub const MIN_TURNS: usize = 8;

/// Turns without an edit, and without a score gain, that make a session
/// quiet.
pub const QUIET_TURNS: usize = 8;

/// Repeated failures in the window that make a session look like a loop.
pub const REPEATED_FAILURES: usize = 2;

/// Repeated commands in the window that make a session look like a loop.
pub const REPEATED_COMMANDS: usize = 4;

/// Finishes turned back without an edit since that make a session stuck.
pub const TURN_BACKS: usize = 2;

/// Turns after a checkpoint within which a score gain makes the next step
/// productive, for [`hindsight`].
pub const NEXT_HORIZON: usize = 8;

/// A later trial with fewer turns left than this isn't labeled: there is
/// nothing left to stop.
pub const MIN_LATER_TURNS: usize = 3;

/// The lowest probability of the pick for a next-step suggestion to be
/// put in the brief.
pub const NEXT_P: f64 = 0.5;

/// Jev's question on repetition.
pub const Q_REPEATING: &str = "Is the session in `recent_events` repeating a step that already \
failed: rerunning the same command, reapplying the same kind of change, or retrying the same \
approach after it failed, with no change that could alter the result?";

/// Jev's question on progress.
pub const Q_PROGRESS: &str = "Over `recent_events`, did the session make progress toward the \
core step the task in `task` requires: a change to the solution that a later run shows working \
better, a higher score in `measured`, or a new finding that decides how to do the core step? \
Reading, re-checking, or restating what is already known is not progress.";

/// Jev's question on completion. Report-only: no code acts on it.
pub const Q_DONE: &str = "Do `recent_events` show the task in `task` already complete, with every \
requirement met and checked, so that the session is only re-checking or polishing?";

/// Jev's next-step question.
pub const Q_NEXT: &str = "Which next step would move the session in `recent_events` closest to \
completing the task in `task`? Each option names a concrete step the host found in the state.";

/// The next-step option that defers to the session.
pub const CONTINUE_OPTION: &str = "None of the other steps: the session's own next step is better \
than any of them.";

/// One tool call in a session's log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    /// The session turn it belongs to, from 1.
    pub turn: usize,
    pub at_ms: u64,
    pub tool: String,
    /// The command, the path, or the files a patch touches.
    pub input: String,
    pub exit: Option<i64>,
    pub failed: bool,
    /// The head and tail of the output.
    pub output: String,
    /// A completed `apply_patch` or `write_file`.
    pub edit: bool,
    /// Files the call edited or read.
    pub files: Vec<String>,
    /// The last `SCORE <passed> <total>` line of a command's output, or the
    /// host's score in a turned-back finish.
    pub score: Option<(u64, u64)>,
    /// The last file and line an error in the output points at.
    pub location: Option<(String, u64)>,
}

/// One session's log, as the detectors read it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub started_ms: u64,
    /// Model requests the session made.
    pub turns: usize,
    pub calls: Vec<Call>,
    /// The turns at which the host turned a finish back.
    pub turn_backs: Vec<usize>,
    /// The task, from the session's first message.
    pub task: String,
}

fn clip(text: &str, max: usize) -> String {
    crate::judge::clip(text, max)
}

/// The first `head` and last `tail` characters of `text`.
fn ends(text: &str, head: usize, tail: usize) -> String {
    let count = text.chars().count();
    if count <= head + tail {
        return text.to_string();
    }
    let first: String = text.chars().take(head).collect();
    let last: String = text.chars().skip(count - tail).collect();
    format!("{first}\n…\n{last}")
}

/// The files an `apply_patch` patch names.
fn patch_files(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter_map(|line| {
            ["*** Update File: ", "*** Add File: ", "*** Delete File: "]
                .iter()
                .find_map(|prefix| line.strip_prefix(prefix))
        })
        .map(|file| file.trim().to_string())
        .collect()
}

/// The last file and line an error in `output` points at: a Python
/// traceback frame, or a `path:line:` or `File "path", line N` mention.
#[must_use]
pub fn error_location(output: &str) -> Option<(String, u64)> {
    let mut found = None;
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("File \"")
            && let Some((path, after)) = rest.split_once('"')
            && let Some(number) = after
                .trim_start_matches(',')
                .trim()
                .strip_prefix("line ")
                .and_then(|n| n.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|n| n.parse().ok())
        {
            found = Some((path.to_string(), number));
            continue;
        }
        // `path.ext:12:` as compilers print it.
        let mut parts = line.splitn(3, ':');
        if let (Some(path), Some(number), Some(_)) = (parts.next(), parts.next(), parts.next())
            && path.contains('.')
            && !path.contains(' ')
            && !path.starts_with("http")
            && let Ok(number) = number.trim().parse::<u64>()
        {
            found = Some((path.to_string(), number));
        }
    }
    found
}

/// The host's score in a turned-back finish's note.
fn turned_back_score(message: &str) -> Option<(u64, u64)> {
    let rest = message.split("scores the workspace ").nth(1)?;
    let mut words = rest.split_whitespace();
    let passed = words.next()?.parse().ok()?;
    (words.next()? == "of").then_some(())?;
    let total: u64 = words
        .next()?
        .trim_end_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .ok()?;
    (total > 0).then_some((passed, total))
}

/// Reads one session's ATIF log, as Microluna writes it.
#[must_use]
pub fn parse_session(text: &str) -> Session {
    let mut session = Session::default();
    for line in text.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match record["record"].as_str() {
            Some("session") => {
                session.id = record["session"]["id"].as_str().unwrap_or("").to_string();
                session.started_ms = record["at"].as_u64().unwrap_or(0);
            }
            Some("step") => {
                let step = &record["step"];
                let at_ms = step["at"].as_u64().unwrap_or(0);
                let source = step["source"].as_str().unwrap_or("");
                let message = step["message"].as_str().unwrap_or("");
                if let Some(call) = step.get("call").filter(|c| c.is_object()) {
                    session
                        .calls
                        .push(parse_call(call, session.turns.max(1), at_ms));
                } else if source == "Agent" {
                    session.turns += 1;
                } else if source == "User" && session.task.is_empty() {
                    if let Some(rest) = message.strip_prefix("# Task\n\n") {
                        let task = rest.split("\n\n# Guidance").next().unwrap_or(rest);
                        let task = task.split("\n\n# Evidence").next().unwrap_or(task);
                        session.task = task.trim().to_string();
                    }
                } else if source == "System"
                    && message.starts_with("The host turned this finish back")
                {
                    session.turn_backs.push(session.turns);
                    if let Some(score) = turned_back_score(message) {
                        session.calls.push(Call {
                            turn: session.turns.max(1),
                            at_ms,
                            tool: "host_score".to_string(),
                            input: "the host's score on a finish".to_string(),
                            exit: None,
                            failed: false,
                            output: clip(message, 300),
                            edit: false,
                            files: Vec::new(),
                            score: Some(score),
                            location: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    session
}

fn parse_call(call: &Value, turn: usize, at_ms: u64) -> Call {
    let tool = call["name"].as_str().unwrap_or("").to_string();
    let arguments = &call["arguments"];
    let output = call["output"].as_str().unwrap_or("");
    let completed = call["outcome"].as_str() == Some("Completed");
    let exit = output
        .strip_prefix("[exit ")
        .and_then(|rest| rest.split(']').next())
        .and_then(|n| n.trim().parse::<i64>().ok());
    let (input, files) = match tool.as_str() {
        "run_command" => (
            arguments["command"].as_str().unwrap_or("").to_string(),
            Vec::new(),
        ),
        "read_file" | "write_file" => {
            let path = arguments["path"].as_str().unwrap_or("").to_string();
            let input = match arguments["start_line"].as_u64() {
                Some(line) if tool == "read_file" => format!("{path} from line {line}"),
                _ => path.clone(),
            };
            (input, vec![path])
        }
        "apply_patch" => {
            let files = patch_files(arguments["patch"].as_str().unwrap_or(""));
            (format!("patch {}", files.join(", ")), files)
        }
        "finish" => (
            format!(
                "{}: {}",
                arguments["status"].as_str().unwrap_or(""),
                arguments["summary"].as_str().unwrap_or("")
            ),
            Vec::new(),
        ),
        _ => (arguments.to_string(), Vec::new()),
    };
    let failed = !completed || exit.is_some_and(|code| code != 0);
    let stdout = output.split("\n[stderr]\n").next().unwrap_or("");
    let score = (tool == "run_command")
        .then(|| microluna::session::parse_score(stdout))
        .flatten();
    Call {
        turn,
        at_ms,
        edit: completed && matches!(tool.as_str(), "apply_patch" | "write_file"),
        tool,
        input,
        exit,
        failed,
        output: ends(output, 200, 400),
        files,
        score,
        location: if failed { error_location(output) } else { None },
    }
}

/// Where a checkpoint sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum At {
    /// Inside a session, every [`EVERY`] turns.
    InSession,
    /// At a session's end, where the lean loop can act.
    SessionEnd,
}

/// What code reads from the events before a checkpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Features {
    /// Turns across the dispatch since the last edit, or since its start.
    pub turns_since_edit: usize,
    /// Turns since a score last rose, or since the start.
    pub turns_since_gain: usize,
    /// Seconds since the last edit, or since the start. Never in Jev's
    /// state, so a replay's request doesn't depend on timing.
    pub seconds_since_edit: u64,
    pub edits: usize,
    pub edits_in_window: usize,
    pub failed_in_window: usize,
    /// Failed commands in the window that repeat an earlier failed one.
    pub repeated_failures: usize,
    /// Commands in the window that repeat an earlier one.
    pub repeated_commands: usize,
    /// Finishes the host turned back in the session since its last edit.
    pub turn_backs_since_edit: usize,
    /// The best score seen, by its last total.
    pub best_score: Option<(u64, u64)>,
}

impl Features {
    /// No edit and no score gain for [`QUIET_TURNS`] turns.
    #[must_use]
    pub fn quiet(&self) -> bool {
        self.turns_since_edit >= QUIET_TURNS && self.turns_since_gain >= QUIET_TURNS
    }

    /// The same failure, or the same command, again and again.
    #[must_use]
    pub fn looping(&self) -> bool {
        self.repeated_failures >= REPEATED_FAILURES || self.repeated_commands >= REPEATED_COMMANDS
    }

    /// The host sent finishes back and nothing changed.
    #[must_use]
    pub fn turned_back(&self) -> bool {
        self.turn_backs_since_edit >= TURN_BACKS
    }

    /// Whether code finds a stall plausible, so Jev's answer matters.
    #[must_use]
    pub fn suspect(&self) -> bool {
        self.quiet() || self.looping() || self.turned_back()
    }

    /// Whether code alone calls a stall, for when Jev has no answer.
    #[must_use]
    pub fn strong(&self) -> bool {
        (self.quiet() && self.looping()) || self.turned_back()
    }

    /// The features that hold, as the evidence a re-brief names.
    #[must_use]
    pub fn evidence(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.turns_since_edit >= QUIET_TURNS {
            out.push(format!(
                "no file changed in the last {} turns",
                self.turns_since_edit
            ));
        }
        if self.turns_since_gain >= QUIET_TURNS {
            out.push(match self.best_score {
                Some((p, t)) => format!(
                    "the score has stayed at or below {p} of {t} for {} turns",
                    self.turns_since_gain
                ),
                None => format!("no score was measured in {} turns", self.turns_since_gain),
            });
        }
        if self.repeated_failures > 0 {
            out.push(format!(
                "{} failed commands repeated an earlier failure",
                self.repeated_failures
            ));
        }
        if self.turn_backs_since_edit > 0 {
            out.push(format!(
                "the host turned back {} finishes with no edit since",
                self.turn_backs_since_edit
            ));
        }
        out
    }
}

/// A next step code proposes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// `run_example`, `run_tests`, `read_region`, or `edit`.
    pub kind: String,
    /// The concrete step, as a brief would say it.
    pub step: String,
}

/// A point in a dispatch, with what code read before it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The session, from 1, and its turn.
    pub session: usize,
    pub turn: usize,
    pub at: At,
    /// Turns across the dispatch up to the checkpoint.
    pub global_turn: usize,
    pub features: Features,
    /// The last [`WINDOW`] events of the session, as Jev reads them.
    pub window: Vec<Value>,
    pub candidates: Vec<Candidate>,
    /// The latest failed command before the checkpoint, and how many
    /// times it failed, for a re-brief's evidence.
    #[serde(default)]
    pub repeated: Option<(String, usize)>,
}

fn normalized(command: &str) -> String {
    clip(
        &command.split_whitespace().collect::<Vec<_>>().join(" "),
        200,
    )
}

/// A score that beats every earlier one with the same total, or the first
/// with a new total that passes anything.
fn gains(best: &mut BTreeMap<u64, u64>, (passed, total): (u64, u64)) -> bool {
    match best.get(&total) {
        Some(before) if passed <= *before => false,
        Some(_) => {
            best.insert(total, passed);
            true
        }
        None => {
            best.insert(total, passed);
            passed > 0
        }
    }
}

/// Programs whose invocation in the task's text reads as an example to
/// run.
const PROGRAMS: [&str; 16] = [
    "python3", "python", "sh", "bash", "./", "make", "cargo", "npm", "node", "go", "java", "ruby",
    "coqc", "gcc", "pytest", "uv",
];

/// A command the task's text gives in backticks.
fn task_example(task: &str) -> Option<String> {
    task.split('`')
        .skip(1)
        .step_by(2)
        .map(str::trim)
        .find(|span| {
            let first = span.split_whitespace().next().unwrap_or("");
            span.contains(' ')
                && PROGRAMS
                    .iter()
                    .any(|p| first == *p || first.starts_with("./"))
        })
        .map(str::to_string)
}

fn is_test_command(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    lower.contains("score.sh") || lower.contains("pytest") || lower.contains(" test")
}

fn is_read_command(command: &str) -> bool {
    let first = command.split_whitespace().next().unwrap_or("");
    matches!(
        first,
        "cat" | "sed" | "head" | "tail" | "grep" | "nl" | "less" | "rg" | "ls" | "find" | "wc"
    )
}

/// The next steps code proposes from the task and the calls so far:
/// run the task's example, run the tests the session or task named, read
/// the region the latest error points at, and edit the file worked on
/// last.
#[must_use]
pub fn candidates(task: &str, prefix: &[&Call]) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Some(example) = task_example(task) {
        out.push(Candidate {
            kind: "run_example".to_string(),
            step: format!("run the task's own example, `{}`", clip(&example, 200)),
        });
    }
    if let Some(test) = prefix
        .iter()
        .rev()
        .find(|c| c.tool == "run_command" && is_test_command(&c.input))
    {
        out.push(Candidate {
            kind: "run_tests".to_string(),
            step: format!(
                "run the tests again and read what fails, `{}`",
                clip(&normalized(&test.input), 200)
            ),
        });
    }
    let located = prefix.iter().rev().find_map(|c| c.location.clone());
    let last_file = prefix
        .iter()
        .rev()
        .find(|c| c.edit)
        .or_else(|| prefix.iter().rev().find(|c| c.tool == "read_file"))
        .and_then(|c| c.files.first().cloned());
    match (&located, &last_file) {
        (Some((path, line)), _) => out.push(Candidate {
            kind: "read_region".to_string(),
            step: format!("read `{path}` around line {line}, where the latest error points"),
        }),
        (None, Some(path)) => out.push(Candidate {
            kind: "read_region".to_string(),
            step: format!("read `{path}` again before changing it"),
        }),
        (None, None) => {}
    }
    if let Some(path) = last_file.or(located.map(|(path, _)| path)) {
        out.push(Candidate {
            kind: "edit".to_string(),
            step: format!("edit `{path}` to fix what the last run showed wrong"),
        });
    }
    out
}

fn render(call: &Call) -> Value {
    json!({
        "turn": call.turn,
        "tool": call.tool,
        "input": clip(&call.input, 300),
        "exit": call.exit,
        "failed": call.failed,
        "output": clip(&call.output, 400),
    })
}

/// The checkpoint at `turn` of session `s` (from 1) of `sessions`: every
/// earlier session whole, and session `s` up to and including `turn`.
///
/// # Panics
///
/// Never: an `s` past the sessions reads the last one.
#[must_use]
pub fn checkpoint(sessions: &[Session], s: usize, turn: usize, at: At) -> Checkpoint {
    let s = s.clamp(1, sessions.len().max(1));
    let before: usize = sessions.iter().take(s - 1).map(|x| x.turns).sum();
    let global_turn = before + turn;
    let mut prefix: Vec<(usize, &Call)> = Vec::new();
    let mut offset = 0;
    for (i, session) in sessions.iter().enumerate().take(s) {
        for call in &session.calls {
            if i + 1 < s || call.turn <= turn {
                prefix.push((offset + call.turn, call));
            }
        }
        offset += session.turns;
    }
    let mut features = Features::default();
    let mut best: BTreeMap<u64, u64> = BTreeMap::new();
    let mut last_edit: Option<(usize, u64)> = None;
    let mut last_gain: Option<usize> = None;
    for (at_turn, call) in &prefix {
        if call.edit {
            features.edits += 1;
            last_edit = Some((*at_turn, call.at_ms));
        }
        if let Some(score) = call.score {
            if gains(&mut best, score) {
                last_gain = Some(*at_turn);
            }
            features.best_score = best.get(&score.1).map(|p| (*p, score.1));
        }
    }
    features.turns_since_edit = global_turn - last_edit.map_or(0, |(t, _)| t.min(global_turn));
    features.turns_since_gain = global_turn - last_gain.unwrap_or(0).min(global_turn);
    let end_ms = prefix.last().map_or(0, |(_, c)| c.at_ms);
    let start_ms = sessions.first().map_or(0, |x| x.started_ms);
    features.seconds_since_edit =
        end_ms.saturating_sub(last_edit.map_or(start_ms, |(_, ms)| ms)) / 1000;
    let current = &sessions.get(s - 1);
    let session_calls: Vec<&Call> = current
        .map(|x| x.calls.iter().filter(|c| c.turn <= turn).collect())
        .unwrap_or_default();
    let events: Vec<&Call> = session_calls
        .iter()
        .filter(|c| c.tool != "host_score")
        .copied()
        .collect();
    let window: Vec<&Call> = events[events.len().saturating_sub(WINDOW)..].to_vec();
    let mut seen_failed: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for call in &window {
        if call.edit {
            features.edits_in_window += 1;
        }
        if call.tool != "run_command" {
            continue;
        }
        let key = normalized(&call.input);
        let times = seen.entry(key.clone()).or_insert(0);
        if *times > 0 {
            features.repeated_commands += 1;
        }
        *times += 1;
        if call.failed {
            features.failed_in_window += 1;
            let times = seen_failed.entry(key).or_insert(0);
            if *times > 0 {
                features.repeated_failures += 1;
            }
            *times += 1;
        }
    }
    let repeated = seen_failed
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .max_by_key(|(_, n)| *n);
    if let Some(session) = current {
        let last_edit_turn = session
            .calls
            .iter()
            .filter(|c| c.edit && c.turn <= turn)
            .map(|c| c.turn)
            .max()
            .unwrap_or(0);
        features.turn_backs_since_edit = session
            .turn_backs
            .iter()
            .filter(|t| **t <= turn && **t >= last_edit_turn)
            .count();
    }
    let task = sessions
        .iter()
        .find(|x| !x.task.is_empty())
        .map_or("", |x| x.task.as_str());
    let all: Vec<&Call> = prefix.iter().map(|(_, c)| *c).collect();
    Checkpoint {
        session: s,
        turn,
        at,
        global_turn,
        features,
        window: window.iter().map(|c| render(c)).collect(),
        candidates: candidates(task, &all),
        repeated,
    }
}

/// Every checkpoint of a dispatch: each session's turns [`EVERY`] apart
/// from [`MIN_TURNS`], and each session's end but the last session's.
#[must_use]
pub fn checkpoints(sessions: &[Session]) -> Vec<Checkpoint> {
    let mut out = Vec::new();
    for (i, session) in sessions.iter().enumerate() {
        let last = i + 1 == sessions.len();
        let mut turn = MIN_TURNS;
        while turn < session.turns {
            out.push(checkpoint(sessions, i + 1, turn, At::InSession));
            turn += EVERY;
        }
        if !last && session.turns > 0 {
            out.push(checkpoint(sessions, i + 1, session.turns, At::SessionEnd));
        }
    }
    out
}

/// The Jev request at a checkpoint: its state and questions. `task` is
/// the task as the host holds it.
#[must_use]
pub fn request(task: &str, checkpoint: &Checkpoint) -> (Value, jev::Questions) {
    let f = &checkpoint.features;
    let options: BTreeMap<&str, &str> = checkpoint
        .candidates
        .iter()
        .map(|c| (c.kind.as_str(), c.step.as_str()))
        .collect();
    let state = json!({
        "task": clip(task.trim(), 2_500),
        "recent_events": checkpoint.window,
        "measured": {
            "session": checkpoint.session,
            "turn": checkpoint.turn,
            "turns_since_last_edit": f.turns_since_edit,
            "turns_since_score_rose": f.turns_since_gain,
            "best_score": f.best_score.map(|(p, t)| format!("{p} of {t}")),
            "failed_commands_in_window": f.failed_in_window,
            "repeated_failures_in_window": f.repeated_failures,
            "finishes_turned_back_since_last_edit": f.turn_backs_since_edit,
        },
        "candidates": options,
    });
    let mut questions = jev::Questions::new()
        .with("repeating", jev::Noul::new(Q_REPEATING))
        .with("progress", jev::Noul::new(Q_PROGRESS))
        .with("done", jev::Noul::new(Q_DONE));
    if !checkpoint.candidates.is_empty() {
        let mut choice = jev::Choice::new(Q_NEXT, indexmap::IndexMap::new());
        for candidate in &checkpoint.candidates {
            choice = choice.option(candidate.kind.clone(), candidate.step.clone());
        }
        choice = choice.option("continue", CONTINUE_OPTION);
        questions = questions.with("next", choice);
    }
    (state, questions)
}

/// The thresholds that turn Jev's answers into a stall call. Frozen from
/// the calibration tasks before the evaluation labels were read; see
/// `bench/terminal-bench/experiments/2026-09-25-stall-detection/`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Params {
    /// Below this probability of progress, a suspect checkpoint stalls.
    pub progress_below: f64,
    /// At or above this probability of repetition, a suspect checkpoint
    /// stalls; above 1 never.
    pub repeating_at: f64,
}

impl Default for Params {
    fn default() -> Self {
        FROZEN
    }
}

/// The frozen thresholds: the protocol's rule on the 327 labeled
/// calibration checkpoints picked the highest recall with precision of at
/// least 0.80 (`selection.json`: 80 correct of 88 calls, 80 of 216 stalls).
/// Both sit at the edge of the grid the protocol fixed.
pub const FROZEN: Params = Params {
    progress_below: 0.5,
    repeating_at: 0.5,
};

/// Jev's answers at a checkpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Answers {
    pub repeating: Option<f64>,
    pub progress: Option<f64>,
    pub done: Option<f64>,
    pub next: Option<String>,
    pub next_p: Option<f64>,
}

impl Answers {
    /// The answers in a Jev answers object.
    #[must_use]
    pub fn from_asked(asked: &crate::component::jev::Asked) -> Self {
        let next = asked.choice("next").map(str::to_string);
        let next_p = next.as_ref().and_then(|pick| {
            asked.answers.as_ref()?["next"]["probabilities"][pick.as_str()].as_f64()
        });
        Answers {
            repeating: asked.noul("repeating"),
            progress: asked.noul("progress"),
            done: asked.noul("done"),
            next,
            next_p,
        }
    }
}

/// What code concludes at a checkpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub suspect: bool,
    /// Jev alone, when it answered: progress below or repetition at the
    /// threshold.
    pub jev: Option<bool>,
    /// The call code acts on: a suspect checkpoint Jev confirms, or, when
    /// Jev has no answer, a strong code signal.
    pub stalled: bool,
    /// How the call was made: `not_suspect`, `jev`, or `code_fallback`.
    pub by: String,
}

/// The stall call at a checkpoint.
#[must_use]
pub fn decide(features: &Features, answers: &Answers, params: Params) -> Verdict {
    let jev = match (answers.progress, answers.repeating) {
        (None, None) => None,
        (progress, repeating) => Some(
            progress.is_some_and(|p| p < params.progress_below)
                || repeating.is_some_and(|p| p >= params.repeating_at),
        ),
    };
    let suspect = features.suspect();
    let (stalled, by) = match (suspect, jev) {
        (false, _) => (false, "not_suspect"),
        (true, Some(jev)) => (jev, "jev"),
        (true, None) => (features.strong(), "code_fallback"),
    };
    Verdict {
        suspect,
        jev,
        stalled,
        by: by.to_string(),
    }
}

/// `executor.microluna.lean.detect`: which detectors the lean loop runs
/// after each work session.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    /// Re-brief the next session on a stall, and stop the work sessions
    /// on a stall right after a re-brief.
    #[serde(default)]
    pub stall: bool,
    /// Put Jev's pick among the next steps code proposes in the next
    /// session's brief, as a suggestion.
    #[serde(default)]
    pub next_step: bool,
}

/// What the lean loop does after a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Continue,
    /// Brief the next session with the stall's evidence.
    Rebrief,
    /// Start no more work sessions.
    Stop,
}

/// The action for a stall call: a first stall re-briefs, and a stall
/// right after a re-brief stops.
#[must_use]
pub fn action(stalled: bool, rebriefed_last: bool) -> Action {
    match (stalled, rebriefed_last) {
        (false, _) => Action::Continue,
        (true, false) => Action::Rebrief,
        (true, true) => Action::Stop,
    }
}

/// What the next session is told after a stall.
#[must_use]
pub fn rebrief_note(checkpoint: &Checkpoint) -> String {
    let mut evidence = checkpoint.features.evidence();
    if let Some((command, times)) = &checkpoint.repeated {
        evidence.push(format!("`{}` failed {times} times", clip(command, 160)));
    }
    format!(
        "The host's stall check after session {} found no progress: {}. Don't repeat those \
         steps. First state in one line what has not worked and why, then re-read the task's \
         core requirement and take a different approach to it.",
        checkpoint.session,
        if evidence.is_empty() {
            "Jev read the last steps as repeating without progress".to_string()
        } else {
            evidence.join("; ")
        }
    )
}

/// The suggestion for the next session, when Jev picked a code-proposed
/// step with at least [`NEXT_P`].
#[must_use]
pub fn next_note(checkpoint: &Checkpoint, answers: &Answers) -> Option<String> {
    let pick = answers.next.as_deref()?;
    if answers.next_p.is_none_or(|p| p < NEXT_P) {
        return None;
    }
    let candidate = checkpoint.candidates.iter().find(|c| c.kind == pick)?;
    Some(format!(
        "The host suggests starting this session by this step: {}.",
        candidate.step
    ))
}

/// What happened after a checkpoint, for labels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hindsight {
    /// Agent turns left in the dispatch after the checkpoint.
    pub later_turns: usize,
    /// Whether the checkpoint is labeled at all: at least
    /// [`MIN_LATER_TURNS`] turns followed.
    pub labeled: bool,
    /// A later score beat the best before, or the attempt passed.
    pub progress_after: bool,
    /// The label: labeled, and no progress after.
    pub stall: bool,
    /// The kind of the first call after the checkpoint: a candidate's
    /// kind, `other`, or `none`.
    pub next_kind: String,
    /// A score gain within [`NEXT_HORIZON`] turns.
    pub productive_next: bool,
}

/// Labels a checkpoint from the rest of the dispatch and the attempt's
/// outcome. `host_scores` are the host's scores after each session, by
/// session from 1. A pass makes every checkpoint progress: a stop there
/// could have lost it.
#[must_use]
pub fn hindsight(
    sessions: &[Session],
    checkpoint: &Checkpoint,
    passed: bool,
    host_scores: &[(usize, (u64, u64))],
) -> Hindsight {
    let total: usize = sessions.iter().map(|x| x.turns).sum();
    let later_turns = total.saturating_sub(checkpoint.global_turn);
    let mut ordered: Vec<(usize, &Call)> = Vec::new();
    let mut offset = 0;
    let mut scores: Vec<(usize, (u64, u64))> = Vec::new();
    for (i, session) in sessions.iter().enumerate() {
        for call in &session.calls {
            ordered.push((offset + call.turn, call));
            if let Some(score) = call.score {
                scores.push((offset + call.turn, score));
            }
        }
        offset += session.turns;
        for (s, score) in host_scores {
            if *s == i + 1 {
                scores.push((offset, *score));
            }
        }
    }
    scores.sort_by_key(|(t, _)| *t);
    let mut best: BTreeMap<u64, u64> = BTreeMap::new();
    let mut gain_turns = Vec::new();
    for (turn, score) in &scores {
        let before = *turn <= checkpoint.global_turn;
        if gains(&mut best, *score) && !before {
            gain_turns.push(*turn);
        }
    }
    let progress_after = passed || !gain_turns.is_empty();
    let labeled = later_turns >= MIN_LATER_TURNS;
    let next = ordered
        .iter()
        .find(|(turn, call)| *turn > checkpoint.global_turn && call.tool != "host_score")
        .map(|(_, call)| *call);
    let next_kind = match next {
        None => "none".to_string(),
        Some(call) => kind_of(call, &checkpoint.candidates),
    };
    let productive_next = gain_turns
        .iter()
        .any(|t| *t <= checkpoint.global_turn + NEXT_HORIZON);
    Hindsight {
        later_turns,
        labeled,
        progress_after,
        stall: labeled && !progress_after,
        next_kind,
        productive_next,
    }
}

/// The kind of step a call is, in the candidates' terms.
#[must_use]
pub fn kind_of(call: &Call, candidates: &[Candidate]) -> String {
    match call.tool.as_str() {
        "apply_patch" | "write_file" => "edit".to_string(),
        "read_file" => "read_region".to_string(),
        "run_command" => {
            let example = candidates
                .iter()
                .find(|c| c.kind == "run_example")
                .and_then(|c| c.step.split('`').nth(1))
                .map(|command| {
                    command
                        .split_whitespace()
                        .take(2)
                        .collect::<Vec<_>>()
                        .join(" ")
                });
            if example.is_some_and(|e| normalized(&call.input).contains(&e)) {
                "run_example".to_string()
            } else if is_test_command(&call.input) {
                "run_tests".to_string()
            } else if is_read_command(&call.input) {
                "read_region".to_string()
            } else {
                "other".to_string()
            }
        }
        _ => "other".to_string(),
    }
}

/// The implementation record's parameters: the question set, the code
/// rule's constants, and the thresholds.
#[must_use]
pub fn implementation(params: Params) -> crate::record::Implementation {
    crate::record::Implementation::new(
        STALL_COMPONENT,
        "stall-v1",
        &json!({
            "questions": [Q_REPEATING, Q_PROGRESS, Q_DONE, Q_NEXT, CONTINUE_OPTION],
            "window": WINDOW,
            "every": EVERY,
            "min_turns": MIN_TURNS,
            "quiet_turns": QUIET_TURNS,
            "repeated_failures": REPEATED_FAILURES,
            "repeated_commands": REPEATED_COMMANDS,
            "turn_backs": TURN_BACKS,
            "params": params,
            "next_p": NEXT_P,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(value: Value) -> String {
        json!({ "record": "step", "step": value }).to_string()
    }

    fn reply(at: u64) -> String {
        step(json!({ "at": at, "source": "Agent", "message": "" }))
    }

    fn command(at: u64, command: &str, output: &str) -> String {
        step(
            json!({ "at": at, "source": "Agent", "message": "", "call": {
            "id": "c", "name": "run_command",
            "arguments": { "command": command, "timeout_seconds": 30 },
            "output": output, "outcome": "Completed" } }),
        )
    }

    fn patch(at: u64, file: &str) -> String {
        step(
            json!({ "at": at, "source": "Agent", "message": "", "call": {
            "id": "p", "name": "apply_patch",
            "arguments": { "patch": format!("*** Begin Patch\n*** Update File: {file}\n@@\n-a\n+b\n*** End Patch") },
            "output": "ok", "outcome": "Completed" } }),
        )
    }

    /// A session that edits once, then fails the same command `fails`
    /// times, one per turn.
    fn looping_session(fails: usize) -> String {
        let mut lines = vec![
            json!({ "record": "session", "at": 1_000, "session": { "id": "microluna-1-1" } })
                .to_string(),
            step(json!({ "at": 1_001, "source": "User",
                "message": "# Task\n\nFix `python3 run.py data.csv` so it prints the total.\n\n# Guidance\n\nWork." })),
            reply(2_000),
            patch(2_001, "run.py"),
        ];
        for i in 0..fails {
            let at = 3_000 + 1_000 * i as u64;
            lines.push(reply(at));
            lines.push(command(
                at + 1,
                "python3 run.py data.csv",
                "[exit 1]\nTraceback (most recent call last):\n  File \"run.py\", line 7, in <module>\nKeyError: 'total'",
            ));
        }
        lines.join("\n")
    }

    #[test]
    fn a_session_log_reads_as_turns_calls_edits_and_the_task() {
        let session = parse_session(&looping_session(3));
        assert_eq!(session.turns, 4);
        assert_eq!(session.calls.len(), 4);
        assert!(session.calls[0].edit);
        assert_eq!(session.calls[0].files, ["run.py"]);
        assert_eq!(session.calls[1].exit, Some(1));
        assert!(session.calls[1].failed);
        assert_eq!(session.calls[1].location, Some(("run.py".to_string(), 7)));
        assert!(session.task.starts_with("Fix `python3 run.py"));
    }

    #[test]
    fn repeated_failures_without_an_edit_are_a_strong_stall() {
        let sessions = vec![parse_session(&looping_session(10))];
        let at = checkpoint(&sessions, 1, 11, At::SessionEnd);
        assert_eq!(at.features.turns_since_edit, 10);
        assert!(at.features.quiet());
        assert!(at.features.looping());
        assert!(at.features.strong());
        assert_eq!(at.repeated.as_ref().map(|r| r.1), Some(10));
        let kinds: Vec<&str> = at.candidates.iter().map(|c| c.kind.as_str()).collect();
        assert_eq!(kinds, ["run_example", "read_region", "edit"]);
        assert!(at.candidates[1].step.contains("line 7"));
        // Jev's answer decides a suspect checkpoint; with none, the strong
        // code signal does.
        let none = decide(&at.features, &Answers::default(), FROZEN);
        assert!(none.stalled);
        assert_eq!(none.by, "code_fallback");
        let working = Answers {
            progress: Some(0.9),
            repeating: Some(0.1),
            ..Answers::default()
        };
        assert!(!decide(&at.features, &working, FROZEN).stalled);
        let stuck = Answers {
            progress: Some(0.1),
            repeating: Some(0.9),
            ..Answers::default()
        };
        assert!(decide(&at.features, &stuck, FROZEN).stalled);
        assert!(rebrief_note(&at).contains("failed 10 times"));
    }

    #[test]
    fn an_early_checkpoint_is_not_suspect_whatever_jev_says() {
        let sessions = vec![parse_session(&looping_session(2))];
        let at = checkpoint(&sessions, 1, 3, At::InSession);
        assert!(!at.features.suspect());
        let stuck = Answers {
            progress: Some(0.0),
            repeating: Some(1.0),
            ..Answers::default()
        };
        let verdict = decide(&at.features, &stuck, FROZEN);
        assert!(!verdict.stalled);
        assert_eq!(verdict.jev, Some(true));
    }

    #[test]
    fn the_first_stall_rebriefs_and_the_next_one_stops() {
        assert_eq!(action(false, true), Action::Continue);
        assert_eq!(action(true, false), Action::Rebrief);
        assert_eq!(action(true, true), Action::Stop);
    }

    #[test]
    fn hindsight_counts_a_later_score_gain_or_a_pass_as_progress() {
        let mut text = looping_session(10);
        text.push('\n');
        text.push_str(&reply(20_000));
        text.push('\n');
        text.push_str(&command(20_001, "sh score.sh", "[exit 0]\nSCORE 3 5"));
        let sessions = vec![parse_session(&text)];
        let early = checkpoint(&sessions, 1, 8, At::InSession);
        let seen = hindsight(&sessions, &early, false, &[]);
        assert!(seen.labeled);
        assert!(seen.progress_after);
        assert!(!seen.stall);
        assert_eq!(seen.next_kind, "run_example");
        let flat = vec![parse_session(&looping_session(12))];
        let at = checkpoint(&flat, 1, 8, At::InSession);
        let seen = hindsight(&flat, &at, false, &[]);
        assert!(seen.stall);
        assert!(!hindsight(&flat, &at, true, &[]).stall);
        // Too little left to stop is not labeled.
        let late = checkpoint(&flat, 1, 12, At::InSession);
        assert!(!hindsight(&flat, &late, false, &[]).labeled);
    }

    #[test]
    fn the_request_names_the_candidates_and_skips_timing() {
        let sessions = vec![parse_session(&looping_session(10))];
        let at = checkpoint(&sessions, 1, 11, At::SessionEnd);
        let (state, questions) = request(&sessions[0].task, &at);
        assert!(state["candidates"]["run_example"].is_string());
        assert!(state["measured"].get("seconds_since_edit").is_none());
        let body = serde_json::to_value(&questions).unwrap();
        assert_eq!(body["next"]["type"], "choice");
        assert!(body["next"]["criteria"]["continue"].is_string());
        let answers = Answers {
            next: Some("read_region".to_string()),
            next_p: Some(0.7),
            ..Answers::default()
        };
        assert!(next_note(&at, &answers).unwrap().contains("line 7"));
        let unsure = Answers {
            next_p: Some(0.3),
            ..answers
        };
        assert!(next_note(&at, &unsure).is_none());
    }

    #[test]
    fn a_turned_back_finish_carries_the_host_score() {
        let text = [
            json!({ "record": "session", "at": 1, "session": { "id": "s" } }).to_string(),
            reply(2),
            step(json!({ "at": 3, "source": "System",
                "message": "The host turned this finish back: the evaluation script scores the workspace 2 of 5. Keep working." })),
            reply(4),
            step(json!({ "at": 5, "source": "System",
                "message": "The host turned this finish back: you finished as failed with 40 turns left." })),
        ]
        .join("\n");
        let session = parse_session(&text);
        assert_eq!(session.turn_backs, [1, 2]);
        assert_eq!(session.calls[0].score, Some((2, 5)));
        let at = checkpoint(&[session], 1, 2, At::SessionEnd);
        assert!(at.features.turned_back());
    }
}
