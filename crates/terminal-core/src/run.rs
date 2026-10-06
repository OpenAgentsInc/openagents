//! The sheet's run page: the Coder run a conversation started, drawn
//! natively beside the thread (#10660).
//!
//! The page reads the task owner's own view of the run
//! (`openagents --json task view ID`, the durable task store's record and
//! its retained ATIF steps) through the mount's transport, and shows its
//! lifecycle, engine, steps and tool calls, checks, cost, artifacts, and the
//! child runs the trace actually records. It never invents a child: a run
//! whose trace records no spawn says so, and a trace that is missing or
//! unreadable leaves the linkage unknown.
//!
//! Steering and cancelling go through the task owner's existing commands
//! (`openagents task correct|cancel --file -`) on the original task, at the
//! revision the page read. Each command keeps its ID and exact bytes, so
//! sending it again after an unknown outcome is a retry the owner
//! deduplicates, never a second command. A cancel the owner accepted is
//! `requested` until the run acknowledges it. A run on another host, or one
//! the page cannot read, offers no controls. Reading and reopening the page
//! only read.

use serde::Deserialize;
use serde_json::Value;
use std::sync::mpsc::Receiver;
use web_time::{Duration, Instant};

/// The most bytes of helper output the page reads.
pub const READ_MAX: usize = 4 * 1024 * 1024;

/// How many trace steps the page asks the owner for.
pub const STEPS: usize = 200;

/// How often an open page reads a live run again.
pub const LIVE_EVERY: Duration = Duration::from_secs(2);

/// The task command schema the owner accepts.
pub const COMMAND_SCHEMA: &str = "openagents.coder.task-command.v1";

/// Tool names engines give a subagent spawn (Claude Code's `Task` and
/// `Agent`, OpenCode's `task`, Codex's `spawn_agent`, Grok Build's
/// `spawn_subagent`), as `openagents_chat_app::subagents` reads them.
pub const SPAWN_TOOLS: [&str; 5] = ["Task", "Agent", "task", "spawn_agent", "spawn_subagent"];

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Configuration {
    #[serde(default)]
    pub adapter: String,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Intent {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub configuration: Configuration,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Outcome {
    #[serde(default)]
    pub ending: String,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub stop_requested: bool,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default)]
    pub artifact_digest: Option<String>,
    #[serde(default)]
    pub output_incomplete: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Execution {
    #[serde(default)]
    pub result: Option<Outcome>,
}

/// The task owner's record of the run, the fields the page shows.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Task {
    pub task_id: String,
    pub revision: u64,
    #[serde(default)]
    pub intent: Intent,
    pub status: String,
    pub execution: String,
    pub checks: String,
    #[serde(default)]
    pub cancellation_reason: Option<String>,
    #[serde(default)]
    pub run: Option<Execution>,
    #[serde(default)]
    pub follow_ups: Vec<Value>,
    #[serde(default)]
    pub corrections: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Evidence {
    pub state: String,
    #[serde(default)]
    pub total_steps: usize,
    #[serde(default)]
    pub more_available: bool,
    #[serde(default)]
    pub steps: Vec<Value>,
}

/// `openagents --json task view`: the run and its retained evidence.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Run {
    pub task: Task,
    pub evidence: Evidence,
    #[serde(default)]
    pub artifacts: Option<Value>,
    #[serde(default)]
    pub artifact_error: Option<String>,
    #[serde(default)]
    pub verification: String,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub cost_status: String,
    /// The route this run belongs to, from the shared route journal
    /// (`route_contract::view`, #10698); absent for a task no route
    /// started.
    #[serde(default)]
    pub route: Option<Value>,
}

impl Run {
    /// The route view, when the owner sent one: `None` when absent,
    /// `Some(Err)` when it does not read as a route view.
    #[must_use]
    pub fn route(&self) -> Option<Result<route_contract::view::RouteView, ()>> {
        self.route
            .as_ref()
            .map(|value| serde_json::from_value(value.clone()).map_err(|_| ()))
    }

    /// Whether the run may still change: queued, running, or cancelling.
    #[must_use]
    pub fn live(&self) -> bool {
        matches!(
            self.task.status.as_str(),
            "queued" | "running" | "cancel_requested"
        ) || self.task.checks == "running"
    }

    /// The child runs the trace records: each spawn call's name and what
    /// it names the child, in step order.
    #[must_use]
    pub fn children(&self) -> Vec<(usize, String)> {
        let mut found = Vec::new();
        for (index, step) in self.evidence.steps.iter().enumerate() {
            let call = &step["call"];
            let name = call["name"].as_str().unwrap_or_default();
            let mut sessions = Vec::new();
            references(step, &mut sessions, 0);
            let spawn = SPAWN_TOOLS.contains(&name) || name.starts_with("Agent: ");
            if !spawn && sessions.is_empty() {
                continue;
            }
            let child = match sessions.first() {
                Some(session) => format!("session {session}"),
                None => format!("a {name} call; the child's own steps are not in this trace"),
            };
            found.push((index + 1, child));
        }
        found
    }
}

/// Collects every `subagent_trajectory_ref` session ID under `value`.
fn references(value: &Value, out: &mut Vec<String>, depth: usize) {
    if depth > 8 {
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "subagent_trajectory_ref" {
                    for reference in value.as_array().into_iter().flatten() {
                        if let Some(session) = reference["session_id"].as_str() {
                            out.push(crate::ascii::ascii(session));
                        }
                    }
                } else {
                    references(value, out, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                references(item, out, depth + 1);
            }
        }
        _ => {}
    }
}

/// Why a run could not be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unread {
    /// The task store keeps no such task. Nothing is created.
    Missing,
    /// The owner cannot answer now; the reason is plain text.
    Unavailable(String),
}

/// What reading a run answered.
pub type Read = Result<Run, Unread>;

/// The owner's error line: `{"error": {"code", "message"}}` or
/// `{"error": "message"}`.
fn error_of(value: &Value) -> Option<(String, String)> {
    let error = value.get("error")?;
    match error {
        Value::String(message) => Some((String::new(), crate::ascii::ascii(message))),
        Value::Object(_) => Some((
            crate::ascii::ascii(error["code"].as_str().unwrap_or_default()),
            crate::ascii::ascii(error["message"].as_str().unwrap_or_default()),
        )),
        _ => None,
    }
}

/// The last nonblank line of `output` as JSON.
fn last_json(output: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(output);
    let line = text.lines().rev().find(|line| !line.trim().is_empty())?;
    serde_json::from_str(line).ok()
}

/// Decodes the owner's answer to viewing task `asked`: the view on
/// standard output, or its error on standard error. An answer about
/// another task is unavailable, never shown in its place.
#[must_use]
pub fn decode(stdout: &[u8], stderr: &[u8], asked: &str) -> Read {
    if stdout.len() > READ_MAX {
        return Err(Unread::Unavailable("the run is too large to show".into()));
    }
    if let Some(value) = last_json(stdout) {
        if let Some((code, message)) = error_of(&value) {
            return Err(refusal(&code, &message));
        }
        let Ok(run) = serde_json::from_value::<Run>(value) else {
            return Err(Unread::Unavailable(
                "the task owner's answer was not a run".into(),
            ));
        };
        if run.task.task_id != asked {
            return Err(Unread::Unavailable(
                "the task owner answered for another run".into(),
            ));
        }
        return Ok(run);
    }
    match last_json(stderr).as_ref().and_then(error_of) {
        Some((code, message)) => Err(refusal(&code, &message)),
        None => Err(Unread::Unavailable(
            "the task owner's answer was not readable".into(),
        )),
    }
}

fn refusal(code: &str, message: &str) -> Unread {
    if code == "not_found" || message.contains("not found") || message.contains("unknown task") {
        Unread::Missing
    } else if code.is_empty() {
        Unread::Unavailable(message.to_owned())
    } else {
        Unread::Unavailable(format!("{message} ({code})"))
    }
}

/// A steering or cancelling command, kept with its exact bytes so a retry
/// after an unknown outcome is the same command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// `cancel` or `correct`, the owner's subcommand.
    pub verb: &'static str,
    pub id: String,
    pub bytes: Vec<u8>,
    pub state: Sent,
}

/// Where a command stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sent {
    /// Waiting for CONFIRM.
    Armed,
    /// Sent; no answer yet.
    Sending,
    /// The owner accepted it; the task's status then.
    Accepted(String),
    /// The owner refused it, with its reason.
    Refused(String),
    /// Whether it landed is unknown; sending it again is a retry.
    Unknown(String),
}

impl Command {
    /// A command on task `task` at `revision`: cancelling it, or, with a
    /// `prompt`, steering it by correcting its instructions.
    #[must_use]
    pub fn new(task: &str, revision: u64, prompt: Option<&str>) -> Self {
        let id = crate::smart::id();
        let (verb, action) = match prompt {
            None => (
                "cancel",
                serde_json::json!({"type": "cancel", "reason": "Cancelled from the terminal."}),
            ),
            Some(prompt) => (
                "correct",
                serde_json::json!({
                    "type": "correct",
                    "prompt": prompt,
                    "reason": "Steered from the terminal.",
                }),
            ),
        };
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema": COMMAND_SCHEMA,
            "command_id": id,
            "task_id": task,
            "expected_revision": revision,
            "action": action,
        }))
        .unwrap_or_default();
        Command {
            verb,
            id,
            bytes,
            state: Sent::Armed,
        }
    }
}

/// Decodes the owner's answer to a command: its receipt's status, or its
/// refusal.
#[must_use]
pub fn decode_receipt(stdout: &[u8], stderr: &[u8], command: &str) -> Sent {
    if let Some(value) = last_json(stdout) {
        if value["command_id"].as_str() == Some(command)
            && let Some(status) = value["status"].as_str()
        {
            return Sent::Accepted(crate::ascii::ascii(status));
        }
        if let Some((code, message)) = error_of(&value) {
            return Sent::Refused(format!("{message} ({code})"));
        }
    }
    match last_json(stderr).as_ref().and_then(error_of) {
        Some((code, message)) => Sent::Refused(if code.is_empty() {
            message
        } else {
            format!("{message} ({code})")
        }),
        None => Sent::Unknown("the task owner's answer was not readable".into()),
    }
}

/// The page's state.
#[derive(Default)]
pub struct Page {
    /// The page is drawn in place of the transcript.
    pub open: bool,
    /// The run shown: its task ID and the host it is on (`local` is this
    /// computer).
    pub task: Option<(String, String)>,
    pub shown: Option<Read>,
    pub scroll: usize,
    pub reading: Option<Receiver<Read>>,
    pub dirty: bool,
    pub read_at: Option<Instant>,
    pub reads: u64,
    /// The last steering or cancelling command, and its answer in flight.
    pub command: Option<Command>,
    pub sending: Option<Receiver<Sent>>,
}

impl Page {
    /// Shows task `task` on `host`; the same run keeps its place and its
    /// last command.
    pub fn show(&mut self, task: &str, host: &str) {
        let wanted = (task.to_owned(), host.to_owned());
        if self.task.as_ref() != Some(&wanted) {
            *self = Page {
                task: Some(wanted),
                reads: self.reads,
                ..Page::default()
            };
        }
        self.open = true;
        self.dirty = true;
    }

    /// Whether the run is on this computer, where the task owner is.
    #[must_use]
    pub fn local(&self) -> bool {
        self.task.as_ref().is_some_and(|(_, host)| host == "local")
    }

    /// Whether controls are offered: a local run the page has read, still
    /// live, and no command waiting on its answer.
    #[must_use]
    pub fn controls(&self) -> bool {
        self.local() && matches!(&self.shown, Some(Ok(run)) if run.live()) && self.sending.is_none()
    }

    /// Whether the open page should read its run again now.
    #[must_use]
    pub fn due(&self, now: Instant) -> bool {
        if !self.open || !self.local() || self.reading.is_some() {
            return false;
        }
        let live = matches!(&self.shown, Some(Ok(run)) if run.live());
        self.dirty
            || (live
                && self
                    .read_at
                    .is_none_or(|at| now.duration_since(at) >= LIVE_EVERY))
    }
}

fn word(text: &str) -> String {
    text.replace('_', " ")
}

/// The page's text before wrapping.
#[must_use]
pub fn lines(page: &Page) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::{ascii, plain};
    use crate::paper::Tone;
    let mut out = Vec::new();
    let (task, host) = page
        .task
        .as_ref()
        .map_or(("-", "-"), |(task, host)| (task.as_str(), host.as_str()));
    let task = ascii(task);
    if !page.local() {
        out.push((format!("RUN {task} on host {}", ascii(host)), Tone::Loud));
        out.push((
            "This run is on another computer; this terminal reads and controls only runs on this one.".into(),
            Tone::Present,
        ));
        out.push(("CONTROLS none here".into(), Tone::Quiet));
        return out;
    }
    let run = match &page.shown {
        None => {
            out.push((format!("RUN {task}  [reading]"), Tone::Loud));
            return out;
        }
        Some(Err(Unread::Missing)) => {
            out.push((format!("RUN {task}  [missing]"), Tone::Loud));
            out.push((
                "The task store on this computer keeps no such run. Nothing was created in its place."
                    .into(),
                Tone::Present,
            ));
            out.push(("CONTROLS none".into(), Tone::Quiet));
            return out;
        }
        Some(Err(Unread::Unavailable(why))) => {
            out.push((format!("RUN {task}  [unavailable]"), Tone::Loud));
            out.push((
                format!("The run can't be read now: {why}. F9 twice reads it again."),
                Tone::Present,
            ));
            out.push(("CONTROLS none until the run is read".into(), Tone::Quiet));
            return out;
        }
        Some(Ok(run)) => run,
    };
    let t = &run.task;
    let title = if t.intent.title.trim().is_empty() {
        "Untitled run".to_owned()
    } else {
        ascii(&t.intent.title)
    };
    out.push((
        format!("RUN {title}  ({task}, revision {})", t.revision),
        Tone::Loud,
    ));
    let engine = match &t.intent.configuration.model {
        Some(model) => format!("{} {}", t.intent.configuration.adapter, model),
        None => t.intent.configuration.adapter.clone(),
    };
    out.push((
        format!(
            "STATUS {}  EXECUTION {}  CHECKS {}  ENGINE {}",
            word(&t.status),
            word(&t.execution),
            word(&t.checks),
            ascii(&engine)
        ),
        Tone::Present,
    ));
    let cost = match run.cost_usd {
        Some(usd) => format!("${usd:.4} ({})", word(&run.cost_status)),
        None => format!("unknown ({})", word(&run.cost_status)),
    };
    let artifacts = match (&run.artifacts, &run.artifact_error) {
        (_, Some(error)) => format!("unavailable: {}", ascii(error)),
        (Some(manifest), None) => {
            let files = manifest["entries"].as_array().map_or(0, Vec::len);
            let changes = manifest["changes"].as_array().map_or(0, Vec::len);
            format!("{files} retained, {changes} changes")
        }
        (None, None) => "none retained".into(),
    };
    out.push((
        format!(
            "COST {cost}  ARTIFACTS {artifacts}  TURNS {}",
            t.follow_ups.len() + 1
        ),
        Tone::Present,
    ));
    if let Some(result) = t.run.as_ref().and_then(|run| run.result.as_ref()) {
        let exit = result
            .exit_code
            .map_or_else(|| "?".to_owned(), |code| code.to_string());
        let mut ended = format!(
            "ENDED {}  exit {exit}  {:.1} s",
            word(&result.ending),
            result.elapsed_ms as f64 / 1000.0
        );
        if result.stop_requested {
            ended.push_str("  stop was requested");
        }
        if result.output_incomplete {
            ended.push_str("  output incomplete");
        }
        out.push((ended, Tone::Present));
    }
    match t.status.as_str() {
        "cancel_requested" => out.push((
            "CANCEL requested; the run has not acknowledged it yet".into(),
            Tone::Loud,
        )),
        "cancelled" => out.push((
            format!(
                "CANCELLED{}",
                t.cancellation_reason
                    .as_deref()
                    .map(|why| format!(": {}", ascii(why)))
                    .unwrap_or_default()
            ),
            Tone::Present,
        )),
        _ => {}
    }
    if let Some(command) = &page.command {
        let what = if command.verb == "cancel" {
            "Cancel"
        } else {
            "Steer"
        };
        let state = match &command.state {
            Sent::Armed => "waiting for ENTER to confirm or ESC to reject".to_owned(),
            Sent::Sending => "sent; waiting for the task owner".to_owned(),
            Sent::Accepted(status) => format!("accepted; the task is {}", word(status)),
            Sent::Refused(why) => format!("refused: {why}"),
            Sent::Unknown(why) => {
                format!("outcome unknown ({why}); F7 or ENTER sends the same command again")
            }
        };
        out.push((
            format!("{what} {}: {state}", ascii(&command.id)),
            Tone::Loud,
        ));
    }
    out.extend(route_lines(run));
    let controls = if page.controls() {
        "CONTROLS ENTER steers with the line, F7 cancels the run (ENTER confirms)"
    } else {
        "CONTROLS none: the run has ended or its state is not known"
    };
    out.push((controls.into(), Tone::Quiet));
    out.push((String::new(), Tone::Quiet));
    // The child runs the trace records, and only those.
    match run.evidence.state.as_str() {
        "sealed" | "unsealed" | "incomplete" => {
            let children = run.children();
            if children.is_empty() {
                let more = if run.evidence.more_available {
                    " in the steps read so far"
                } else {
                    ""
                };
                out.push((
                    format!("CHILDREN none recorded in this run's trace{more}"),
                    Tone::Present,
                ));
            } else {
                for (step, child) in children {
                    out.push((format!("CHILD at step {step}: {child}"), Tone::Present));
                }
            }
        }
        state => out.push((
            format!("CHILDREN unknown: the trace is {}", word(state)),
            Tone::Present,
        )),
    }
    out.push((String::new(), Tone::Quiet));
    let shown = run.evidence.steps.len();
    out.push((
        format!(
            "STEPS {shown} of {}{}",
            run.evidence.total_steps,
            if run.evidence.more_available {
                "; later steps are not read here"
            } else {
                ""
            }
        ),
        Tone::Quiet,
    ));
    for (index, step) in run.evidence.steps.iter().enumerate() {
        let source = step["source"].as_str().unwrap_or("?");
        let message = plain(step["message"].as_str().unwrap_or_default());
        let first = message.lines().find(|line| !line.trim().is_empty());
        let tone = if source == "agent" {
            Tone::Present
        } else {
            Tone::Quiet
        };
        out.push((
            format!(
                "{:>3} {}: {}",
                index + 1,
                ascii(source).to_uppercase(),
                first.unwrap_or("")
            ),
            tone,
        ));
        if let Some(name) = step["call"]["name"].as_str() {
            let outcome = step["call"]["outcome"].as_str().unwrap_or("?");
            out.push((
                format!("    TOOL {} ({})", ascii(name), word(outcome)),
                Tone::Quiet,
            ));
        }
    }
    out
}

fn microusd(value: u64) -> String {
    format!("${:.4}", value as f64 / 1_000_000.0)
}

/// A serialized enum's wire word with spaces.
fn wire<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(word))
        .unwrap_or_default()
}

/// The route lines (#10698): the request and thread the run belongs to,
/// where it runs, who executes and pays, and the route's outcome, cost,
/// and cancellation, each in the record's own words. An unknown cost, an
/// unverified finish, and a requested cancellation are never shown as a
/// settled cost, a verified one, or an acknowledged one.
#[must_use]
pub fn route_lines(run: &Run) -> Vec<(String, crate::paper::Tone)> {
    use crate::ascii::ascii;
    use crate::paper::Tone;
    use route_contract::view::{Cancellation, Cost};
    let view = match run.route() {
        None => {
            return vec![(
                "ROUTE none recorded: this task was not started by a routed request".into(),
                Tone::Quiet,
            )];
        }
        Some(Err(())) => {
            return vec![(
                "ROUTE unreadable: the route record is not a route view".into(),
                Tone::Present,
            )];
        }
        Some(Ok(view)) => view,
    };
    let mut out = Vec::new();
    out.push((
        format!(
            "ROUTE {}  REQUEST {}  THREAD {}  STATE {}",
            wire(view.family),
            ascii(&view.request),
            ascii(view.thread.as_deref().unwrap_or("-")),
            wire(view.state),
        ),
        Tone::Loud,
    ));
    let mut executor: Vec<String> = view.executor.engines.iter().map(|e| ascii(e)).collect();
    executor.extend(view.executor.model.as_deref().map(ascii));
    executor.extend(view.executor.capability.as_deref().map(ascii));
    out.push((
        format!(
            "COMPUTER {}  GRANT {}  EXECUTOR {}",
            ascii(view.computer.as_deref().unwrap_or("none")),
            ascii(view.grant.as_deref().unwrap_or("none")),
            if executor.is_empty() {
                "none".to_owned()
            } else {
                executor.join(", ")
            }
        ),
        Tone::Present,
    ));
    let payers: Vec<String> = view
        .payers
        .iter()
        .map(|line| format!("{} by {}", wire(line.resource), ascii(&line.payer)))
        .collect();
    out.push((
        format!(
            "PAYERS {}",
            if payers.is_empty() {
                "none named".to_owned()
            } else {
                payers.join(", ")
            }
        ),
        Tone::Present,
    ));
    let cost = match view.cost {
        Cost::None => "none: nothing ran".to_owned(),
        Cost::Unknown {
            known_microusd,
            missing,
        } => format!(
            "unknown: {missing} run(s) reported no cost ({} known so far)",
            microusd(known_microusd)
        ),
        Cost::Recorded { microusd: value } => {
            format!("{} recorded; the route may still move", microusd(value))
        }
        Cost::Settled { microusd: value } => format!("{} settled", microusd(value)),
    };
    let cancel = match view.cancellation {
        Cancellation::None => "none",
        Cancellation::Requested => "requested, not acknowledged",
        Cancellation::Acknowledged => "acknowledged",
    };
    out.push((
        format!(
            "OUTCOME {}  ROUTE COST {cost}  CANCEL {cancel}",
            view.outcome.label()
        ),
        Tone::Present,
    ));
    for line in &view.runs {
        out.push((
            format!(
                "  ROUTE RUN {}  {}  {}  check {}  {} artifacts",
                ascii(&line.task),
                ascii(line.engine.as_deref().unwrap_or("-")),
                wire(line.state),
                wire(line.check),
                line.artifacts
            ),
            Tone::Quiet,
        ));
    }
    out
}

#[cfg(test)]
mod route_tests {
    use super::{Page, Run, lines};

    fn run(route: Option<serde_json::Value>) -> Run {
        let mut value = serde_json::json!({
            "task": {"task_id": "task-1", "revision": 3, "status": "finished",
                     "execution": "finished", "checks": "not_run"},
            "evidence": {"state": "sealed", "total_steps": 0, "steps": []},
        });
        if let Some(route) = route {
            value["route"] = route;
        }
        serde_json::from_value(value).unwrap()
    }

    fn route(cost: serde_json::Value, outcome: &str, cancellation: &str) -> serde_json::Value {
        serde_json::json!({
            "schema": route_contract::view::VIEW_SCHEMA,
            "request": "req-1", "thread": "th-1", "family": "coder",
            "state": "completed", "snapshot": format!("sha256:{}", "a".repeat(64)),
            "computer": "this-computer", "grant": "grant-1 epoch 2",
            "executor": {"engines": ["codex"]},
            "payers": [{"resource": "executor", "payer": "login:codex"}],
            "runs": [{"task": "task-1", "engine": "codex", "state": "completed",
                      "check": "unchecked", "artifacts": 2}],
            "cost": cost, "outcome": outcome, "cancellation": cancellation,
        })
    }

    fn text(run: Run) -> String {
        let mut page = Page::default();
        page.show("task-1", "local");
        page.shown = Some(Ok(run));
        lines(&page)
            .into_iter()
            .map(|(line, _)| line)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_run_page_shows_the_routes_identities_and_keeps_states_apart() {
        let shown = text(run(Some(route(
            serde_json::json!({"state": "unknown", "known_microusd": 0, "missing": 1}),
            "unchecked",
            "requested",
        ))));
        assert!(shown.contains("ROUTE coder  REQUEST req-1  THREAD th-1  STATE completed"));
        assert!(shown.contains("COMPUTER this-computer  GRANT grant-1 epoch 2  EXECUTOR codex"));
        assert!(shown.contains("PAYERS executor by login:codex"));
        assert!(shown.contains("OUTCOME finished, not verified"));
        assert!(shown.contains("ROUTE COST unknown: 1 run(s) reported no cost"));
        assert!(shown.contains("CANCEL requested, not acknowledged"));
        assert!(shown.contains("ROUTE RUN task-1  codex  completed  check unchecked  2 artifacts"));
        assert!(!shown.contains("settled"));
        let shown = text(run(Some(route(
            serde_json::json!({"state": "settled", "microusd": 1_500}),
            "verified",
            "acknowledged",
        ))));
        assert!(
            shown.contains("OUTCOME verified  ROUTE COST $0.0015 settled  CANCEL acknowledged")
        );
        assert!(shown.is_ascii());
    }

    #[test]
    fn a_run_without_a_route_or_with_an_unreadable_one_says_so() {
        assert!(text(run(None)).contains("ROUTE none recorded"));
        assert!(text(run(Some(serde_json::json!({"request": 1})))).contains("ROUTE unreadable"));
    }
}
