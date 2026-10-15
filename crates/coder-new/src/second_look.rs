//! Second look (#11540): a fresh-context check of a bigger chat change
//! before the agent says it is done.
//!
//! The agent that wrote a change is the worst judge of it. When a chat
//! turn stops with a change that crosses a size/risk line, a separate
//! cheap model reads only three things: what the person asked, the change
//! (the turn's diff) and the results of the commands the agent ran. Never
//! the agent's conversation. It answers with a typed [`Verdict`]: pass,
//! send back with specific findings, or unsure.
//!
//! It is one Coder SDK decision hook on `Stop` (`coder_sdk::DecisionHook`),
//! so it runs on every harness the Stop hook fires on (Claude Code, the
//! Coder loop, Codex, ACP agents):
//!
//! 1. **Rule.** No request seen, or nothing changed in the turn: nothing
//!    to look at. After one round back and one report: done.
//! 2. **Decision.** Whether the change is worth a second look is one
//!    Decisions API choice over the diff's size, the files it touches, the
//!    commands that ran and how they ended, and the start of the diff. No
//!    keyword list: the model judges size and risk. Acting past
//!    [`COSTS`]' threshold.
//! 3. **Check.** The reviewer ([`Reviewer`]): the OpenAgents gateway's free
//!    model when signed in, else Claude Haiku. A send-back blocks the stop
//!    with the findings, once. On the next stop the reviewer looks again;
//!    what it still finds goes back one last time as a report-only note
//!    ("make no more changes; tell the person"), so the agent's reply
//!    carries it.
//!
//! Every verdict is one `hook-log` line (#11492) in `second-look.jsonl`,
//! and the turn's outcome (did the agent change the change after a
//! send-back, did the look after pass) is one line in
//! `second-look-outcomes.jsonl`, keyed by the turn's run id, so
//! `hook-log report second-look.jsonl --outcomes second-look-outcomes.jsonl`
//! gives the keep rate.
//!
//! It is its own plugin on the Plugins screen, off by default: it spends a
//! cheap model call on every bigger change to save a later fix, which is
//! the opposite of a Token saver, and the A/B did not show it pays yet.
//! `examples/second_look_ab.rs` replays seven scripted turns (three with a
//! planted bug that passes the agent's own weak test, four correct) through
//! the real hook. Reviewer alone (every change looked at): Haiku sent back
//! 3/3 bugs and 0/4 correct changes, twice; the gateway's free model sent
//! back 3/3 bugs but also 3/4 correct changes, and its free allowance ran
//! out within about sixteen reviews. With the size/risk line deciding,
//! these small tested changes rated 0.04 to 0.29, so only one of the three
//! bugs was looked at. Measure it in shadow (`CODER_SECOND_LOOK=shadow`)
//! on real chats before turning it on by default.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use coder_sdk::decide::Decider;
use coder_sdk::decision::{Acted, Costs, DecisionHook, Recorder, Ruled, Timing, choice_p};
use coder_sdk::{Call, Disclosure, Event, Hook, HookOutcome, Registry, Source, Stop};
use hook_log::{Decision, How};
use jev::decisions::{DecisionsRequest, DecisionsResponse};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The plugin's ID.
pub const PLUGIN: &str = "second-look";
/// The hook's ID, and its decision log's name.
pub const HOOK: &str = "second-look";
/// The question set's version.
pub const QUESTION_SET: &str = "second-look-v1";
/// The size/risk question.
pub const QUESTION: &str = "look";
/// The switch that overrides the plugin: `off`, `shadow` (decide, look and
/// record, but never send the agent back) or `on`.
pub const VAR: &str = "CODER_SECOND_LOOK";

/// Written costs, in minutes of the person's attention. A needless look
/// costs one cheap model call and, at worst, one short extra round (about
/// a minute). A missed defect the person finds later costs a fix round
/// after they read the change (about four). Look when
/// `p * 4 > (1 - p) * 1`, past p = 0.2.
pub const COSTS: Costs = Costs {
    wrong_act: 1.0,
    missed: 4.0,
};
/// How long the size/risk decision may take.
const DEADLINE: Duration = Duration::from_secs(8);
/// How long the reviewer may take.
const REVIEW_TIMEOUT: Duration = Duration::from_secs(120);
/// The hook's own timeout: the decision, then up to one review.
const HOOK_TIMEOUT: Duration = Duration::from_secs(150);
/// How much of the diff the reviewer reads.
const DIFF_MAX: usize = 60_000;
/// How much of the diff the decision reads.
const DIFF_HEAD: usize = 4_000;
/// The most commands kept for the check results.
const COMMANDS_MAX: usize = 12;

/// The Plugins screen's description.
pub const DESCRIPTION: &str = "Before a chat agent says a bigger change is done, a separate \
     low-cost model reads only your request, the change and its test results, and sends the \
     agent back once with anything it finds. Off by default: it adds a model call to each \
     bigger change, and hasn't yet shown it saves more than it costs.";

// ---------------------------------------------------------------------------
// The plugin's switch.

static STATE: AtomicU8 = AtomicU8::new(UNSET);
const UNSET: u8 = 0;
const OFF: u8 = 1;
const ON: u8 = 2;

/// Records whether the plugin is on, for every later turn in this process.
pub fn set_enabled(enabled: bool) {
    STATE.store(if enabled { ON } else { OFF }, Ordering::Relaxed);
}

/// Whether the plugin is on: what the app set, else the saved settings
/// (off when there are none).
#[must_use]
pub fn enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        _ if cfg!(test) => false,
        _ => crate::home::data_dir()
            .is_some_and(|dir| crate::bundled_settings::saved_enabled(&dir, PLUGIN)),
    }
}

/// The hook's mode: [`VAR`] when set, else on with the plugin.
#[must_use]
pub fn mode() -> crate::sdk_hooks::Mode {
    if std::env::var(VAR).is_ok_and(|value| !value.trim().is_empty()) {
        crate::sdk_hooks::mode(VAR)
    } else if enabled() {
        crate::sdk_hooks::Mode::On
    } else {
        crate::sdk_hooks::Mode::Off
    }
}

// ---------------------------------------------------------------------------
// What the reviewer sees, and what it says.

/// Everything the reviewer sees. Nothing of the agent's conversation is
/// in here, by construction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Review {
    /// What the person asked.
    pub request: String,
    /// The turn's change, as a unified diff.
    pub diff: String,
    /// The commands the agent ran and how each ended.
    pub checks: Vec<String>,
}

/// One thing the reviewer found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    pub problem: String,
}

impl Finding {
    fn bullet(&self) -> String {
        match (&self.file, self.line) {
            (Some(file), Some(line)) => format!("- {file}:{line}: {}", self.problem),
            (Some(file), None) => format!("- {file}: {}", self.problem),
            _ => format!("- {}", self.problem),
        }
    }
}

/// The reviewer's verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    SendBack { findings: Vec<Finding> },
    Unsure { why: String },
}

impl Verdict {
    /// The verdict's word in the decision log.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::SendBack { .. } => "send_back",
            Verdict::Unsure { .. } => "unsure",
        }
    }

    /// Reads the reviewer's reply: one JSON object, possibly inside other
    /// text. A send-back with nothing specific is unsure.
    ///
    /// # Errors
    ///
    /// When the reply has no verdict.
    pub fn parse(reply: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Wire {
            verdict: String,
            #[serde(default)]
            findings: Vec<Finding>,
            #[serde(default)]
            why: Option<String>,
        }
        let start = reply.find('{').ok_or("the reviewer gave no verdict")?;
        let end = reply.rfind('}').ok_or("the reviewer gave no verdict")?;
        let wire: Wire = serde_json::from_str(&reply[start..=end.max(start)])
            .map_err(|error| format!("the reviewer's verdict could not be read: {error}"))?;
        let findings: Vec<Finding> = wire
            .findings
            .into_iter()
            .filter(|finding| !finding.problem.trim().is_empty())
            .collect();
        Ok(match wire.verdict.trim() {
            "pass" => Verdict::Pass,
            "send_back" if !findings.is_empty() => Verdict::SendBack { findings },
            "send_back" => Verdict::Unsure {
                why: "it asked for changes but named none".to_owned(),
            },
            "unsure" => Verdict::Unsure {
                why: wire.why.unwrap_or_default(),
            },
            other => return Err(format!("the reviewer answered {other:?}")),
        })
    }
}

/// A review's verdict, with who gave it and how long it took.
#[derive(Clone, Debug, PartialEq)]
pub struct Reviewed {
    pub verdict: Verdict,
    pub model: String,
    pub door: String,
    pub latency_ms: u64,
}

/// Runs one review. Blocking; called off the session's async threads.
pub type Reviewer = Arc<dyn Fn(&Review) -> Result<Reviewed, String> + Send + Sync>;

/// The reviewer's instructions.
pub const INSTRUCTIONS: &str = "You review a code change with fresh eyes. You see only what the \
person asked for, the change as a unified diff, and the commands the agent ran with how each \
ended. Look for real defects: behavior that does not do what the person asked, bugs, an edge \
case the request implies that the change breaks, a check that failed, or changed behavior with \
no test where the project has tests. Do not comment on style, naming or formatting, and do not \
ask for work the person did not ask for.\n\nAnswer with one JSON object and nothing else:\n\
{\"verdict\": \"pass\" | \"send_back\" | \"unsure\", \"findings\": [{\"file\": \"path\", \
\"line\": 12, \"problem\": \"what is wrong and why, in one or two sentences\"}], \"why\": \
\"only for unsure: what you could not tell\"}\n\nUse send_back only for a defect you are sure of \
and can point to in the diff; trace the code by hand before you claim it misbehaves. The \
command results are what really happened: never say a test fails when its command passed. Use \
pass when you find no such defect, even if you would have written it differently. Use unsure \
when the diff is cut off or you cannot tell.";

/// The reviewer's prompt: the request, the diff and the check results.
#[must_use]
pub fn prompt(review: &Review) -> String {
    let checks = if review.checks.is_empty() {
        "(the agent ran no commands)".to_owned()
    } else {
        review.checks.join("\n")
    };
    format!(
        "What the person asked:\n{}\n\nThe change:\n```diff\n{}\n```\n\nCommands the agent ran:\n{}\n",
        review.request.trim(),
        cut(&review.diff, DIFF_MAX),
        checks
    )
}

// ---------------------------------------------------------------------------
// The default reviewer: the gateway's free model, else Haiku.

/// The OpenAgents account this computer is signed in with.
fn account() -> Option<(PathBuf, openagents_login::Saved)> {
    let dir = crate::home::data_dir()?;
    let account = crate::connection_tools::signed_in(&dir)?;
    crate::gateway_models::reachable(&account).then_some((dir, account))
}

/// The gateway model a review runs on: the first free one the account's
/// list names (kept on disk, refreshed when stale), else the gateway's own
/// router.
fn gateway_model(dir: &Path, account: &openagents_login::Saved) -> String {
    crate::gateway_models::listed_now(dir, Some(account))
        .into_iter()
        .find(|row| row.free)
        .map_or_else(|| crate::models::GATEWAY_AUTO.to_owned(), |row| row.id)
}

/// The default reviewer: the gateway when signed in, else (or when the
/// gateway fails) Claude Haiku through this computer's Claude Code.
#[must_use]
pub fn default_reviewer() -> Reviewer {
    Arc::new(|review: &Review| {
        let text = prompt(review);
        let mut failures = Vec::new();
        if let Some((dir, account)) = account() {
            match on_gateway(&dir, &account, &text) {
                Ok(reviewed) => return Ok(reviewed),
                Err(error) => failures.push(error),
            }
        }
        match on_haiku(&text) {
            Ok(reviewed) => Ok(reviewed),
            Err(error) => {
                failures.push(error);
                Err(failures.join("; "))
            }
        }
    })
}

/// Claude Haiku alone, through this computer's Claude Code: the
/// reviewer when nobody is signed in to OpenAgents, and the A/B's second
/// arm.
#[must_use]
pub fn haiku_reviewer() -> Reviewer {
    Arc::new(|review: &Review| on_haiku(&prompt(review)))
}

/// Runs `future` to its end on a thread of its own, so a caller inside an
/// async runtime can wait on it.
fn blocking<T: Send + 'static>(
    future: impl std::future::Future<Output = Result<T, String>> + Send + 'static,
) -> Result<T, String> {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(future)
    })
    .join()
    .map_err(|_| "the review stopped without an answer".to_owned())?
}

fn on_gateway(
    dir: &Path,
    account: &openagents_login::Saved,
    text: &str,
) -> Result<Reviewed, String> {
    let model = gateway_model(dir, account);
    let provider = crate::provider::Provider::gateway(account)?;
    let messages = vec![
        openrouter::Message {
            role: "system".into(),
            content: INSTRUCTIONS.into(),
        },
        openrouter::Message {
            role: "user".into(),
            content: text.to_owned(),
        },
    ];
    let started = Instant::now();
    let asked = model.clone();
    let streamed = blocking(async move {
        tokio::time::timeout(
            REVIEW_TIMEOUT,
            provider.stream(&asked, messages, &mut |_: &str| {}),
        )
        .await
        .map_err(|_| "the gateway review ran out of time".to_owned())?
    })?;
    Ok(Reviewed {
        verdict: Verdict::parse(&streamed.text)?,
        model: if streamed.model.is_empty() {
            model
        } else {
            streamed.model
        },
        door: "openagents-gateway".into(),
        latency_ms: elapsed(started),
    })
}

/// Haiku, through `claude -p` with no tools, in an empty folder so no
/// project instructions load.
fn on_haiku(text: &str) -> Result<Reviewed, String> {
    use std::io::Write;
    let agent = crate::bundled_runtime::claude_print::agent()
        .ok_or("neither the OpenAgents gateway nor Claude Code is available for a review")?;
    let empty = tempfile::tempdir().map_err(|error| error.to_string())?;
    let started = Instant::now();
    let mut child = acp_client::process::command(&agent.program)
        .current_dir(empty.path())
        .envs(acp_client::process::NON_INTERACTIVE)
        .args([
            "-p",
            "--model",
            crate::claude::MODEL,
            "--tools",
            "",
            "--no-session-persistence",
            "--output-format",
            "json",
            "--append-system-prompt",
            INSTRUCTIONS,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "Claude Code could not start for a review".to_owned())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(child.wait_with_output());
    });
    let output = receiver
        .recv_timeout(REVIEW_TIMEOUT)
        .map_err(|_| "the Haiku review ran out of time".to_owned())?
        .map_err(|error| error.to_string())?;
    let body: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Claude Code's review could not be read".to_owned())?;
    if body["is_error"].as_bool() == Some(true) {
        return Err("Claude Code could not run the review".to_owned());
    }
    Ok(Reviewed {
        verdict: Verdict::parse(body["result"].as_str().unwrap_or_default())?,
        model: crate::claude::MODEL.to_owned(),
        door: "claude-code".into(),
        latency_ms: elapsed(started),
    })
}

fn elapsed(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

// ---------------------------------------------------------------------------
// The turn: what the person asked, where the change started, what ran.

/// Where the turn is in its bounded rounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Round {
    #[default]
    Fresh,
    /// Sent back once with findings.
    SentBack,
    /// The remaining findings went back as a report: done.
    Reported,
}

#[derive(Default)]
struct Turn {
    request: Option<String>,
    /// The working tree when the turn started, as a git tree.
    base: Option<String>,
    checks: Vec<String>,
    round: Round,
    /// The tree the agent was sent back on, and the verdict line's `ts`.
    sent: Option<(String, u64, Vec<Finding>)>,
    run_id: String,
}

static TURNS: AtomicU64 = AtomicU64::new(0);

fn run_id() -> String {
    std::env::var(hook_log::RUN_ID_VAR)
        .ok()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "chat-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |since| since.as_millis()),
                TURNS.fetch_add(1, Ordering::Relaxed)
            )
        })
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The working tree in `cwd`, tracked and untracked files alike (ignored
/// ones aside), written as a git tree through a copy of the index so the
/// real index and the files are untouched.
#[must_use]
pub fn snapshot(cwd: &Path) -> Option<String> {
    let git = |args: &[&str], index: Option<&Path>| {
        let mut command = std::process::Command::new("git");
        command.arg("-C").arg(cwd).args(args);
        if let Some(index) = index {
            command.env("GIT_INDEX_FILE", index);
        }
        command
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let real = git(
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
        None,
    )?;
    // A path in a folder of its own, not an open temporary file: Git
    // replaces the index by renaming over it, which Windows refuses for a
    // file that is still open (#11695).
    let scratch = tempfile::tempdir().ok()?;
    let index = scratch.path().join("index");
    // A repository with nothing staged yet has no index: start empty.
    let _ = std::fs::copy(&real, &index);
    git(&["add", "-A"], Some(&index))?;
    git(&["write-tree"], Some(&index))
}

/// The change between two trees: the unified diff, and the per-file line
/// counts (`added`, `removed`, path).
fn changes(cwd: &Path, from: &str, to: &str) -> (String, Vec<(u64, u64, String)>) {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default()
    };
    let diff = git(&["diff", "--no-color", "--no-ext-diff", from, to]);
    let counts = git(&["diff", "--numstat", from, to])
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let added = parts.next()?.parse().unwrap_or(0);
            let removed = parts.next()?.parse().unwrap_or(0);
            Some((added, removed, parts.next()?.to_owned()))
        })
        .collect();
    (diff, counts)
}

/// `text` cut to `max` bytes on a character boundary, saying so.
fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[... the rest is cut]", &text[..end])
}

/// One command and how it ended, from a `PostToolUse` call, when the call
/// ran a command.
fn command_line(call: &Call) -> Option<String> {
    let command = match &call.input["command"] {
        Value::String(command) => command.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        _ => return None,
    };
    let response = &call.input["tool_response"];
    let output = [&response["stdout"], &response["output"], response]
        .into_iter()
        .find_map(Value::as_str)
        .unwrap_or_default();
    let exit = response["exit_code"]
        .as_i64()
        .or_else(|| response["exit"].as_i64())
        .or_else(|| call.input["exit"].as_i64());
    let failed = call.input["failed"].as_bool() == Some(true)
        || response["interrupted"].as_bool() == Some(true)
        || exit.is_some_and(|code| code != 0);
    let ended = match (failed, exit) {
        (true, Some(code)) => format!("failed (exit {code})"),
        (true, None) => "failed".to_owned(),
        (false, Some(code)) => format!("exit {code}"),
        (false, None) => "finished".to_owned(),
    };
    let tail: String = {
        let lines: Vec<&str> = output.trim().lines().collect();
        let start = lines.len().saturating_sub(6);
        lines[start..].join("\n")
    };
    let mut line = format!("$ {} -> {ended}", cut(command.trim(), 300));
    if !tail.is_empty() {
        line.push('\n');
        line.push_str(&cut(&tail, 600));
    }
    Some(line)
}

// ---------------------------------------------------------------------------
// The hooks.

/// What the hook sends out, and to whom.
#[must_use]
pub fn disclosure() -> Disclosure {
    Disclosure {
        sends: "the person's request, the turn's change and the results of the commands the agent ran"
            .to_owned(),
        to: "the Decisions API and a low-cost review model (the OpenAgents gateway, or Claude Haiku)"
            .to_owned(),
    }
}

/// Where outcome lines go.
pub type Outcomes = Arc<dyn Fn(&Value) + Send + Sync>;

/// `~/.openagents/decisions/second-look-outcomes.jsonl`.
#[must_use]
pub fn default_outcomes() -> Outcomes {
    Arc::new(|line: &Value| {
        use std::io::Write;
        let Some(path) = hook_log::default_path("second-look-outcomes") else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{line}");
        }
    })
}

/// What the hooks share.
struct Look {
    cwd: PathBuf,
    turn: Mutex<Turn>,
    reviewer: Reviewer,
    recorder: Recorder,
    outcomes: Outcomes,
    shadow: bool,
}

impl Look {
    fn started(&self, request: &str) {
        let base = snapshot(&self.cwd);
        *lock(&self.turn) = Turn {
            request: Some(request.to_owned()),
            base,
            run_id: run_id(),
            ..Turn::default()
        };
    }

    fn record(&self, mut line: Decision) {
        if line.run_id.is_none() {
            let id = lock(&self.turn).run_id.clone();
            line.run_id = (!id.is_empty()).then_some(id);
        }
        (self.recorder)(&line);
    }

    /// Reviews `review`, records the verdict, and returns it with the
    /// verdict line's `ts`.
    fn review(&self, review: &Review, round: &str, files: usize) -> (Option<Verdict>, u64) {
        let reviewed = (self.reviewer)(review);
        let mut line = match &reviewed {
            Ok(reviewed) => {
                let mut line =
                    Decision::new(HOOK, "verdict", reviewed.verdict.word(), How::Decision);
                line.model = Some(reviewed.model.clone());
                line.door = Some(reviewed.door.clone());
                line.latency_ms = Some(reviewed.latency_ms);
                line.detail = json!({"verdict": reviewed.verdict});
                line
            }
            Err(error) => {
                let mut line = Decision::new(HOOK, "verdict", "failed", How::Fallback);
                line.detail = json!({"error": error});
                line
            }
        };
        line.detail["round"] = json!(round);
        line.detail["files"] = json!(files);
        line.detail["diff_bytes"] = json!(review.diff.len());
        line.detail["shadow"] = json!(self.shadow);
        let ts = line.ts;
        self.record(line);
        (reviewed.ok().map(|reviewed| reviewed.verdict), ts)
    }

    /// The turn's change so far: the review input and the file counts.
    fn current(&self) -> Option<(String, Review, Vec<(u64, u64, String)>)> {
        let turn = lock(&self.turn);
        let request = turn.request.clone()?;
        let base = turn.base.clone()?;
        let checks = turn.checks.clone();
        drop(turn);
        let now = snapshot(&self.cwd)?;
        if now == base {
            return None;
        }
        let (diff, counts) = changes(&self.cwd, &base, &now);
        if diff.trim().is_empty() {
            return None;
        }
        Some((
            now,
            Review {
                request,
                diff,
                checks,
            },
            counts,
        ))
    }

    fn rule(&self) -> Ruled {
        let round = lock(&self.turn).round;
        if round == Round::Reported {
            return Ruled::Pass;
        }
        let Some((now, review, counts)) = self.current() else {
            return Ruled::Pass;
        };
        if round == Round::SentBack {
            return self.recheck(&now, &review, counts.len());
        }
        let added: u64 = counts.iter().map(|(added, _, _)| added).sum();
        let removed: u64 = counts.iter().map(|(_, removed, _)| removed).sum();
        let files: Vec<String> = counts
            .iter()
            .take(40)
            .map(|(added, removed, path)| format!("{path} (+{added} -{removed})"))
            .collect();
        let checks = if review.checks.is_empty() {
            "none".to_owned()
        } else {
            review
                .checks
                .iter()
                .map(|check| check.lines().next().unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let input = format!(
            "The person asked a coding agent:\n{}\n\nThe agent changed {} files (+{added} -{removed} lines):\n{}\n\nCommands the agent ran and how they ended:\n{checks}\n\nThe start of the change:\n{}",
            cut(review.request.trim(), 1_500),
            counts.len(),
            files.join("\n"),
            cut(&review.diff, DIFF_HEAD),
        );
        let questions = DecisionsRequest::new(coder_sdk::decision::MODEL, "")
            .choice(
                QUESTION,
                "Should a second reviewer, who sees only the request and this change, check it before the agent reports it done?",
                [
                    ("check", "Yes: the change is large, touches code where a mistake is costly (money, login, stored data, security, concurrency, migrations, public interfaces), or its tests did not run or failed."),
                    ("skip", "No: the change is small and low-risk and its checks ran and passed, or it only changes docs, comments or wording."),
                ],
            )
            .questions;
        Ruled::Unsure {
            input,
            questions,
            memo: json!({"tree": now, "files": counts.len(), "added": added, "removed": removed}),
        }
    }

    /// The second stop after a send-back: look again, and report what is
    /// left.
    fn recheck(&self, now: &str, review: &Review, files: usize) -> Ruled {
        let (sent_tree, sent_ts, sent_findings) = lock(&self.turn)
            .sent
            .clone()
            .unwrap_or_else(|| (String::new(), 0, Vec::new()));
        let changed = now != sent_tree;
        let (verdict, _) = if changed {
            self.review(review, "recheck", files)
        } else {
            (
                Some(Verdict::SendBack {
                    findings: sent_findings,
                }),
                0,
            )
        };
        let passed = verdict == Some(Verdict::Pass);
        let run_id = lock(&self.turn).run_id.clone();
        (self.outcomes)(&json!({
            "run_id": run_id,
            "passed": passed,
            "changed": changed,
            "labels": [{"decision_ts": sent_ts, "hook": HOOK, "question": "verdict", "correct": changed}],
        }));
        let remaining = match verdict {
            Some(Verdict::SendBack { findings }) => findings,
            _ => {
                if !self.shadow {
                    lock(&self.turn).round = Round::Reported;
                }
                return Ruled::Decided {
                    outcome: HookOutcome::none(),
                    action: if passed {
                        "recheck_pass"
                    } else {
                        "recheck_open"
                    }
                    .to_owned(),
                };
            }
        };
        if !self.shadow {
            lock(&self.turn).round = Round::Reported;
        }
        Ruled::Decided {
            outcome: HookOutcome::stopping(Stop::Block {
                reason: report(&remaining),
            }),
            action: "report".to_owned(),
        }
    }

    fn act(&self, memo: &Value, response: &DecisionsResponse) -> Acted {
        let p = choice_p(response, QUESTION, "check");
        let threshold = COSTS.threshold();
        let acted = |outcome, action: &str| Acted {
            outcome,
            action: action.to_owned(),
            question: QUESTION.to_owned(),
            p: Some(p),
            threshold: Some(threshold),
        };
        if p < threshold {
            return acted(HookOutcome::none(), "skip");
        }
        // The change as it is now (the decision took a moment).
        let Some((now, review, counts)) = self.current() else {
            return acted(HookOutcome::none(), "skip");
        };
        let tree = memo["tree"].as_str().map_or(now, str::to_owned);
        match self.review(&review, "first", counts.len()) {
            (Some(Verdict::SendBack { findings }), ts) => {
                if !self.shadow {
                    let mut turn = lock(&self.turn);
                    turn.round = Round::SentBack;
                    turn.sent = Some((tree, ts, findings.clone()));
                }
                acted(
                    HookOutcome::stopping(Stop::Block {
                        reason: send_back(&findings),
                    }),
                    "send_back",
                )
            }
            (Some(Verdict::Pass), _) => acted(HookOutcome::none(), "pass"),
            (Some(Verdict::Unsure { .. }), _) => acted(HookOutcome::none(), "unsure"),
            (None, _) => acted(HookOutcome::none(), "review_failed"),
        }
    }
}

/// What the agent reads when it is sent back.
#[must_use]
pub fn send_back(findings: &[Finding]) -> String {
    format!(
        "Before you finish: a second reviewer read only the person's request and your change, \
         and found:\n{}\nFix what holds up and rerun the checks that cover it. If a point is \
         wrong, say why in your reply. This comes once.",
        findings
            .iter()
            .map(Finding::bullet)
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// What the agent reads when the second look still finds something.
#[must_use]
pub fn report(findings: &[Finding]) -> String {
    format!(
        "The second reviewer still raises these points:\n{}\nMake no more changes. End your \
         reply to the person with these points, each with one line on whether you agree.",
        findings
            .iter()
            .map(Finding::bullet)
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// The hooks for a chat in `cwd`: the turn's start (`UserPromptSubmit`,
/// and `SessionStart` where the task arrives there), the commands it ran
/// (`PostToolUse`), and the decision hook on `Stop`.
#[must_use]
pub fn hooks(
    cwd: PathBuf,
    decider: Arc<dyn Decider>,
    reviewer: Reviewer,
    recorder: Recorder,
    outcomes: Outcomes,
    shadow: bool,
) -> Vec<Hook> {
    let look = Arc::new(Look {
        cwd,
        turn: Mutex::default(),
        reviewer,
        recorder: recorder.clone(),
        outcomes,
        shadow,
    });
    let on_prompt = look.clone();
    let prompted = Hook::new(
        "second-look-turn",
        Event::UserPromptSubmit,
        Source::BuiltIn,
        move |call: &Call| {
            if let Some(prompt) = call.input["prompt"].as_str() {
                on_prompt.started(prompt);
            }
            Ok(HookOutcome::none())
        },
    );
    let on_start = look.clone();
    let session = Hook::new(
        "second-look-start",
        Event::SessionStart,
        Source::BuiltIn,
        move |call: &Call| {
            // The Coder loop's task arrives here; Claude Code's prompt
            // arrives on `UserPromptSubmit`.
            if let Some(task) = call.input["task"].as_str() {
                on_start.started(task);
            }
            Ok(HookOutcome::none())
        },
    );
    let on_step = look.clone();
    let steps = Hook::new(
        "second-look-steps",
        Event::PostToolUse,
        Source::BuiltIn,
        move |call: &Call| {
            if let Some(line) = command_line(call) {
                let mut turn = lock(&on_step.turn);
                turn.checks.push(line);
                let over = turn.checks.len().saturating_sub(COMMANDS_MAX);
                turn.checks.drain(..over);
            }
            Ok(HookOutcome::none())
        },
    );
    let (on_rule, on_act, on_record) = (look.clone(), look.clone(), look);
    let decision = DecisionHook {
        id: HOOK.to_owned(),
        event: Event::Stop,
        matcher: None,
        question_set: QUESTION_SET.to_owned(),
        model: coder_sdk::decision::MODEL.to_owned(),
        disclosure: disclosure(),
        costs: COSTS,
        timing: Timing::Blocking,
        deadline: DEADLINE,
        rule: Arc::new(move |_call: &Call| on_rule.rule()),
        act: Arc::new(move |memo: &Value, response: &DecisionsResponse| on_act.act(memo, response)),
        fallback: HookOutcome::none(),
        decider,
        recorder: Arc::new(move |line: &Decision| on_record.record(line.clone())),
        shadow,
    };
    vec![
        prompted,
        session,
        steps,
        decision.into_hook().timeout(HOOK_TIMEOUT),
    ]
}

/// Registers the second look on a chat's `registry` in `cwd` when its
/// mode is on or shadow and the admission its switch gives covers what it
/// sends.
pub fn register(registry: &mut Registry, cwd: &Path) {
    register_with(
        registry,
        cwd,
        mode(),
        Arc::new(coder_sdk::decide::Api),
        default_reviewer(),
    );
}

/// [`register`] with the mode, the decider and the reviewer given.
pub fn register_with(
    registry: &mut Registry,
    cwd: &Path,
    mode: crate::sdk_hooks::Mode,
    decider: Arc<dyn Decider>,
    reviewer: Reviewer,
) {
    // The switch is the admission for what it sends (`disclosure`), as
    // for the other System One hooks.
    if mode == crate::sdk_hooks::Mode::Off {
        return;
    }
    for hook in hooks(
        cwd.to_owned(),
        decider,
        reviewer,
        coder_sdk::decision::default_recorder(),
        default_outcomes(),
        mode == crate::sdk_hooks::Mode::Shadow,
    ) {
        // Built-in IDs are distinct.
        let _ = registry.register(hook);
    }
}

#[cfg(test)]
#[path = "second_look_tests.rs"]
mod tests;
