//! A repository turn on a Devin route: the local Devin CLI over ACP.
//!
//! Devin is a whole coding agent, so the Microcoder step loop does not run:
//! the host starts `devin acp` in the admitted workspace, opens a session
//! (or reattaches, with `session/load`, the Devin session an earlier turn of
//! the same task used), and prompts it with the turn's message. The task
//! owner keeps its authority and evidence:
//!
//! - **Effects**: starting the agent and each prompt are effect intents
//!   retained before dispatch, with their observations after.
//! - **Transcript**: Devin's streamed reply, reasoning, and completed tool
//!   calls are appended to the ATIF transcript as they arrive, bounded.
//! - **Identity**: the model the session reports must be the route's model;
//!   `default` admits Devin's own default and records what it reported.
//! - **Access**: full access is Devin's `bypass` mode. Under the boundary,
//!   Devin runs with its own `--sandbox`, in `accept-edits` mode (workspace
//!   edits only), and the host answers every permission request with
//!   Devin's own `reject` option, so it runs no command it had to ask for.
//! - **Cancellation**: a cancelled task, or one at its wall deadline, sends
//!   `session/cancel`, waits a grace, and stops the agent's process group.
//! - **Capacity**: a prompt refusal Devin marks retryable (a typed
//!   `data.retryable: true`) is a rate-limit refusal for the capacity book,
//!   and the run fails over to the next admitted route.
//! - **Marker**: the session opens with the engine mark in its `_meta`, so
//!   the chat list leaves Coder's own Devin sessions out.
//!
//! Usage is Devin's: tokens from its `usage_update`s and `turn_stats`.
//! Devin bills in its own credits and reports no dollar price over ACP, so
//! the turn's cost is unknown.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use acp_client::wire::Update;
use acp_client::{ClientError, Handler, Opening, PermissionAnswer, PermissionRequest, StopReason};
use atif::{Call, Outcome as CallOutcome, Source, Step};
use coder::task::adapter::{Access, Host, Route as GrantRoute};
use coder::task::capacity::{Provider, Refusal};
use serde_json::{Value, json};

/// The step extension that names the Devin session a turn used, which the
/// next turn of the task reattaches.
pub const SESSION_NOTE: &str = "devin_session";
/// The engine a Devin turn records.
pub const ENGINE: &str = "devin-acp";
/// The longest Devin may write nothing during a prompt. A turn has no
/// time limit; this is the whole agent's stuck guard.
pub(super) const SILENCE: Duration = Duration::from_secs(20 * 60);
/// How long a cancelled prompt may take to answer `cancelled`.
pub(super) const CANCEL_GRACE: Duration = Duration::from_secs(10);
/// How long the agent may take to exit before its group is killed.
pub(super) const STOP_GRACE: Duration = Duration::from_secs(5);
/// The most bytes of one tool output the transcript keeps.
const TOOL_OUTPUT: usize = 16 * 1024;
/// The most bytes of one reply or reasoning segment the transcript keeps.
const SEGMENT: usize = 256 * 1024;

/// How a Devin turn ended.
pub(crate) enum Turn {
    /// The turn ran; the host finishes the task with this.
    Ended(Ended),
    /// Devin refused for capacity before doing the turn's work.
    Refused(Refusal),
}

/// What a Devin turn that ran left.
#[derive(Debug, Default)]
pub(crate) struct Ended {
    /// The engine, such as `devin-acp` or `opencode-acp`.
    pub engine: &'static str,
    /// The agent in the transcript's own words, such as `Grok Build`.
    pub agent: &'static str,
    /// The first tool the host refused this turn, and why, when it
    /// refused one.
    pub refused: Option<String>,
    pub session: Option<String>,
    pub resumed: bool,
    pub model: Option<String>,
    pub stop: Option<StopReason>,
    pub reply: String,
    pub error: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub stats: BTreeMap<String, f64>,
    pub tool_calls: usize,
    /// The turn's cost in US dollars, when the agent reported one.
    pub cost_usd: Option<f64>,
    /// The host ended the turn because the delegate recipe's frozen checks
    /// passed while the agent worked (#10208): done, not stopped.
    pub checks_passed: bool,
    /// The turn ended asking the person (#10571): the reply is the
    /// question or the approval, and the task waits for the answer.
    pub asked: Option<coder::task::interaction::Kind>,
}

impl Ended {
    /// The task's result ending and whether the turn completed.
    ///
    /// `cancelled_or_host_refusal` is the host's own stop: the task was
    /// cancelled, reached its time limit, or the host refused the turn. An
    /// agent that ended the turn itself after the host refused a tool it
    /// asked for is `engine_stopped_after_refusal`; one that reported
    /// `cancelled` with no stop from the host is `engine_cancelled`. A turn
    /// the host ended because the delegate recipe's frozen checks passed
    /// is `checks_passed`, and completed (#10208).
    pub fn ending(&self, cancelled: bool) -> (&'static str, bool) {
        match self.stop {
            _ if cancelled => ("cancelled_or_host_refusal", false),
            _ if self.asked.is_some() && self.error.is_none() => {
                (self.asked.map_or("", |kind| kind.ending()), true)
            }
            _ if self.checks_passed => ("checks_passed", true),
            Some(StopReason::EndTurn) => ("model_finished", true),
            _ if self.refused.is_some() && self.error.is_none() => {
                ("engine_stopped_after_refusal", false)
            }
            Some(StopReason::Cancelled) => ("engine_cancelled", false),
            _ => ("engine_incomplete", false),
        }
    }

    /// What a person reads when the turn stopped without finishing, in
    /// plain words, for an ending the agent caused; `None` otherwise.
    pub fn stop_message(&self, cancelled: bool) -> Option<String> {
        let agent = if self.agent.is_empty() {
            "The coding agent"
        } else {
            self.agent
        };
        match self.ending(cancelled).0 {
            "engine_stopped_after_refusal" => Some(format!(
                "{agent} stopped after the host refused a tool it asked to run ({}).",
                self.refused.as_deref().unwrap_or("no detail")
            )),
            "engine_cancelled" => Some(format!(
                "{agent} ended the turn as cancelled on its own; nobody stopped the task."
            )),
            "engine_incomplete" => Some(match &self.error {
                Some(error) => format!("{agent} could not finish the turn: {error}"),
                None => format!(
                    "{agent} stopped before finishing ({}).",
                    self.stop.map_or("no stop reason", StopReason::as_str)
                ),
            }),
            _ => None,
        }
    }

    pub fn summary(&self) -> Value {
        json!({
            "engine": self.engine,
            "session": self.session,
            "resumed": self.resumed,
            "model": self.model,
            "stop_reason": self.stop.map(StopReason::as_str),
            "error": self.error,
            "refused": self.refused,
            "tool_calls": self.tool_calls,
            "usage": {
                "input_tokens": self.input_tokens,
                "output_tokens": self.output_tokens,
                "turn_stats": self.stats,
            },
            "cost_usd": self.cost_usd,
            "asked": self.asked.map(|kind| kind.ending()),
            "cost_unknown": if self.cost_usd.is_some() {
                Value::Null
            } else if self.engine == ENGINE {
                json!("Devin bills in its own credits and reports no dollar price over ACP")
            } else {
                json!("the agent reported no dollar price")
            },
        })
    }
}

/// A tool call in flight: what it asked for, as its updates arrive.
struct Pending {
    title: String,
    kind: String,
    tool: Option<String>,
    input: Value,
    output: String,
    started: std::time::Instant,
}

/// How the host answers an agent's `session/request_permission`.
#[derive(Clone, Debug)]
pub(super) enum Answering {
    /// Full access: every ask is allowed.
    Allow,
    /// The agent runs its own tools outside the host's boundary (Devin,
    /// OpenCode under the boundary): every ask is refused.
    Reject,
    /// The agent's whole process runs inside the host's operating-system
    /// boundary (Grok Build under the boundary or toolchains), which holds
    /// whatever it runs: an ask is allowed, except a tool that writes
    /// files and names one outside `roots` (the workspace and the
    /// boundary's scratch), relative names read against `base`.
    Contained { base: PathBuf, roots: Vec<PathBuf> },
}

/// `path` as an absolute, resolved name: relative to `base`, `.` and `..`
/// taken lexically, and the longest part that exists resolved through its
/// symbolic links, so `/tmp` and `/private/tmp` compare equal.
pub(super) fn resolved(base: &Path, path: &str) -> PathBuf {
    let mut lexical = PathBuf::new();
    for component in base.join(path).components() {
        match component {
            std::path::Component::ParentDir => {
                lexical.pop();
            }
            std::path::Component::CurDir => {}
            other => lexical.push(other),
        }
    }
    let mut existing = lexical.as_path();
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return lexical,
        }
    }
}

impl Answering {
    /// The answer to `request`: the option chosen, and, for a refusal under
    /// [`Answering::Contained`], why.
    fn answer<'r>(&self, request: &'r PermissionRequest) -> (Option<&'r str>, Option<String>) {
        match self {
            Answering::Allow => (request.allow(), None),
            Answering::Reject => (
                request.reject(),
                Some("this run's access refuses every tool the agent asks for".to_owned()),
            ),
            Answering::Contained { base, roots } => {
                let outside = request.tool_call.written_paths().into_iter().find(|path| {
                    let path = resolved(base, path);
                    !roots.iter().any(|root| path.starts_with(root))
                });
                match outside {
                    Some(path) => (
                        request.reject(),
                        Some(format!("it would write {path}, outside the workspace")),
                    ),
                    // Once, so the agent asks again for its next tool.
                    None => (request.allow_once(), None),
                }
            }
        }
    }
}

/// The handler that turns an ACP agent's stream (Devin's, or OpenCode's)
/// into transcript steps.
pub(super) struct Recorder<'a> {
    host: &'a Host,
    /// The agent's name in the transcript's own words, such as `Devin`.
    name: &'static str,
    /// The prefix of the step extensions it notes, such as `devin`.
    note: &'static str,
    model: String,
    access: Access,
    /// How the host answers the agent's permission requests.
    answering: Answering,
    /// The first tool the host refused, and why.
    pub(super) refused: Option<String>,
    text: String,
    thought: String,
    tools: BTreeMap<String, Pending>,
    pub(super) input_tokens: u64,
    pub(super) output_tokens: u64,
    pub(super) stats: BTreeMap<String, f64>,
    pub(super) tool_calls: usize,
    /// The newest cost the agent reported, in US dollars.
    pub(super) cost_usd: Option<f64>,
    /// The last reply segment, which is the turn's answer.
    pub(super) reply: String,
}

fn bounded(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

impl<'a> Recorder<'a> {
    /// A recorder for `name`'s turn on `model`, noting under `note`.
    pub(super) fn new(
        host: &'a Host,
        name: &'static str,
        note: &'static str,
        model: String,
        access: Access,
    ) -> Self {
        Recorder {
            host,
            name,
            note,
            model,
            access,
            answering: match access {
                Access::Full => Answering::Allow,
                Access::Boundary | Access::Toolchains => Answering::Reject,
            },
            refused: None,
            text: String::new(),
            thought: String::new(),
            tools: BTreeMap::new(),
            input_tokens: 0,
            output_tokens: 0,
            stats: BTreeMap::new(),
            tool_calls: 0,
            cost_usd: None,
            reply: String::new(),
        }
    }

    /// Answer permission requests as `answering` says, rather than by the
    /// access alone.
    pub(super) fn answering(mut self, answering: Answering) -> Self {
        self.answering = answering;
        self
    }

    /// Write what is still gathered and close the tool calls still open
    /// as cancelled, once the prompt has ended.
    pub(super) fn close(&mut self) {
        self.flush();
        let pending: Vec<String> = self.tools.keys().cloned().collect();
        for id in pending {
            self.finish_tool(&id, "cancelled");
        }
    }

    fn append(&self, step: &Step) {
        if let Err(error) = self.host.append(step) {
            self.host.fail(error.to_string());
        }
    }

    /// Write the reasoning and reply gathered so far as their own steps.
    fn flush(&mut self) {
        if !self.thought.trim().is_empty() {
            let thought = std::mem::take(&mut self.thought);
            self.append(&Step::thought(&bounded(&thought, SEGMENT)).by(&self.model));
        }
        self.thought.clear();
        if !self.text.trim().is_empty() {
            let text = bounded(&std::mem::take(&mut self.text), SEGMENT);
            self.append(&Step::said(Source::Agent, &text).by(&self.model));
            self.reply = text;
        }
        self.text.clear();
    }

    fn finish_tool(&mut self, id: &str, status: &str) {
        let Some(pending) = self.tools.remove(id) else {
            return;
        };
        self.flush();
        self.tool_calls += 1;
        let outcome = match status {
            "completed" => CallOutcome::Completed,
            "cancelled" => CallOutcome::Cancelled,
            _ => CallOutcome::Failed,
        };
        let call = Call {
            id: id.to_owned(),
            name: pending.tool.unwrap_or(pending.kind.clone()),
            arguments: pending.input,
            output: bounded(&pending.output, TOOL_OUTPUT),
            outcome,
            milliseconds: pending
                .started
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            purpose: Some(pending.title),
            extra: serde_json::Map::new(),
        };
        self.append(&Step::called(call).by(&self.model).noting(
            &format!("{}_tool", self.note),
            json!({"kind": pending.kind, "status": status}),
        ));
    }
}

impl Handler for Recorder<'_> {
    fn update(&mut self, update: Update) {
        match update {
            Update::AgentText(piece) => {
                if !self.thought.is_empty() {
                    let thought = std::mem::take(&mut self.thought);
                    self.append(&Step::thought(&bounded(&thought, SEGMENT)).by(&self.model));
                }
                self.text.push_str(&piece);
            }
            Update::Thought(piece) => self.thought.push_str(&piece),
            Update::ToolCall {
                id,
                title,
                kind,
                raw_input,
                tool,
                status,
            } => {
                self.flush();
                self.tools.insert(
                    id.clone(),
                    Pending {
                        title,
                        kind,
                        tool,
                        input: raw_input,
                        output: String::new(),
                        started: std::time::Instant::now(),
                    },
                );
                if matches!(status.as_str(), "completed" | "failed") {
                    self.finish_tool(&id, &status);
                }
            }
            Update::ToolCallUpdate {
                id,
                status,
                title,
                text,
            } => {
                if let Some(pending) = self.tools.get_mut(&id) {
                    if let Some(title) = title {
                        pending.title = title;
                    }
                    // Devin sends the tool's whole output so far in each
                    // update, so the latest replaces the earlier.
                    if let Some(text) = text {
                        pending.output = text;
                    }
                }
                if let Some(status) = status
                    && matches!(status.as_str(), "completed" | "failed" | "cancelled")
                {
                    self.finish_tool(&id, &status);
                }
            }
            Update::Usage(usage) if !usage.subagent => {
                self.input_tokens = self
                    .input_tokens
                    .saturating_add(usage.input_tokens.unwrap_or_default());
                self.output_tokens = self
                    .output_tokens
                    .saturating_add(usage.output_tokens.unwrap_or_default());
                if usage.cost_usd.is_some() {
                    self.cost_usd = usage.cost_usd;
                }
            }
            Update::Plan(entries) => {
                self.flush();
                self.append(
                    &Step::said(Source::System, &format!("{}'s plan.", self.name))
                        .noting(&format!("{}_plan", self.note), json!(entries)),
                );
            }
            _ => {}
        }
    }

    fn notification(&mut self, method: &str, params: &Value) {
        if method == acp_client::devin::TURN_STATS
            && let Some(stats) = acp_client::devin::TurnStats::parse(params)
        {
            self.stats = stats.numbers();
        }
    }

    fn permission(&mut self, request: &PermissionRequest) -> PermissionAnswer {
        // Full access asks nothing (Devin's bypass mode, OpenCode's allow
        // rule, Grok Build's `--always-approve`); anything still asked is
        // allowed. Under the boundary, Devin and OpenCode get nothing; Grok
        // Build, inside the host's own boundary, gets what that holds.
        let (chosen, why) = self.answering.answer(request);
        let refused = why.is_some() || chosen.is_none();
        if refused && self.refused.is_none() {
            let tool = request
                .tool_call
                .title
                .clone()
                .or_else(|| request.tool_call.kind.clone())
                .unwrap_or_else(|| "a tool".to_owned());
            self.refused = Some(match &why {
                Some(why) => format!("{}: {why}", bounded(&tool, 200)),
                None => bounded(&tool, 200),
            });
        }
        self.append(
            &Step::said(
                Source::System,
                &format!("{} asked for a permission.", self.name),
            )
            .noting(
                &format!("{}_permission", self.note),
                json!({"kind": request.tool_call.kind, "title": request.tool_call.title,
                    "answer": chosen, "access": self.access.as_str(), "refused": why}),
            ),
        );
        chosen.map_or(PermissionAnswer::Cancelled, |option| {
            PermissionAnswer::Selected(option.to_owned())
        })
    }
}

// Delegate history is kept only where `coder-history` reads it.
#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Copy the agent's session `session` from its store `database` into the
/// task directory, beside the task's transcript, and note the copy on the
/// transcript ([`coder_history::delegate`]), so a device reads the delegate's
/// whole session from inside the Coder chat. A copy that fails is noted
/// with why; the turn's own result stands either way.
pub(super) fn keep_delegate(
    host: &Host,
    agent: coder_history::Harness,
    session: &str,
    database: Option<&std::path::Path>,
    copy: fn(&std::path::Path, &str, &std::path::Path) -> Result<bool, String>,
) {
    let Some(name) = coder_history::delegate::file_name(host.task_id(), agent, session) else {
        return;
    };
    let copied = match database {
        Some(database) => copy(database, session, &host.store().join(&name)),
        None => Err("no session store for this agent".to_owned()),
    };
    let word = match agent {
        coder_history::Harness::Devin => "Devin",
        _ => "OpenCode",
    };
    let (said, note) = match copied {
        Ok(_) => (
            format!("The {word} session's transcript is kept beside this task."),
            json!({"agent": agent, "session": session, "file": name}),
        ),
        Err(why) => (
            format!("The {word} session's transcript could not be kept."),
            json!({"agent": agent, "session": session, "error": why}),
        ),
    };
    let _ =
        host.append(&Step::said(Source::System, &said).noting(coder_history::delegate::NOTE, note));
}

/// The Devin CLI's session store for the agent process's environment
/// `variables` ([`environment`], which carries the whole environment):
/// under its `XDG_DATA_HOME`, else its `HOME`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn devin_database(variables: &[(String, String)]) -> Option<PathBuf> {
    let lookup = |name: &str| {
        variables
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| PathBuf::from(value))
    };
    let home = lookup("HOME")?;
    Some(coder_history::devin::database(
        &home,
        lookup("XDG_DATA_HOME").as_deref(),
    ))
}

/// The agent process's environment: the owner's login environment under
/// full access, else this process's, less credential variables either way.
pub(super) async fn environment(host: &Host) -> Vec<(String, String)> {
    let variables: Vec<(OsString, OsString)> = match host.login_environment().await {
        Some(login) => login.variables.clone(),
        None => std::env::vars_os().collect(),
    };
    variables
        .into_iter()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(key, _)| !acp_client::process::is_credential_name(key))
        .collect()
}

/// The Devin binary for this host, or why there is none.
pub(crate) fn binary() -> Result<PathBuf, String> {
    acp_client::devin::binary(&|name| std::env::var_os(name))
        .ok_or_else(|| "no devin binary in DEVIN_BIN, PATH, or ~/.local/bin".to_owned())
}

/// Run one turn on `route` with the Devin binary `program`.
pub(crate) async fn turn(
    host: &Host,
    route: &GrantRoute,
    program: PathBuf,
    mut recipe: Option<&mut super::recipe::Recipe>,
) -> Turn {
    let access = host.configuration().access;
    let mut arguments = acp_client::devin::arguments(&route.model);
    if access != Access::Full {
        arguments.insert(0, "--sandbox".into());
    }
    let permission = match access {
        Access::Full => acp_client::devin::Permission::Bypass,
        Access::Boundary | Access::Toolchains => acp_client::devin::Permission::AcceptEdits,
    };
    let resume = host.earlier_note(SESSION_NOTE).and_then(|note| {
        note.get("session")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let opening = Opening {
        spec: super::private_spec(
            host,
            acp_client::process::Spec {
                program: program.clone(),
                arguments: arguments.clone(),
                cwd: host.workspace().to_path_buf(),
                environment: environment(host).await,
            },
        ),
        resume: resume.clone(),
        meta: Some(acp_client::devin::engine_meta(coder_history_mark())),
        mode: Some(permission.mode_id().into()),
    };
    let mut ended = Ended {
        engine: ENGINE,
        agent: "Devin",
        ..Ended::default()
    };
    let sequence = match host.effect(
        "devin_session",
        json!({"program": program, "arguments": arguments, "cwd": host.workspace(),
            "mode": permission.mode_id(), "resume": resume, "model": route.model}),
    ) {
        Ok(sequence) => sequence,
        Err(error) => {
            ended.error = Some(error.to_string());
            return Turn::Ended(ended);
        }
    };
    let cancelled = || host.cancelled();
    let mut session = match acp_client::Session::open(&opening, &cancelled).await {
        Ok(session) => session,
        Err(failure) => {
            let why = failure.to_string();
            let _ = host.result(sequence, "devin_session", json!({"error": why}));
            ended.error = Some(why);
            return Turn::Ended(ended);
        }
    };
    let reported = session.opened.model().map(str::to_owned);
    ended.session = Some(session.id().to_owned());
    ended.resumed = session.resumed;
    ended.model.clone_from(&reported);
    let observed = json!({"session": session.id(), "pid": session.pid(), "resumed": session.resumed,
        "resume_refused": session.resume_refused, "model": reported,
        "agent": session.initialized.agent_info,
        "mode": session.opened.modes.as_ref().map(|modes| modes.current_mode_id.clone())});
    if let Err(error) = host.result(sequence, "devin_session", observed) {
        ended.error = Some(error.to_string());
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    if let Err(error) = host.append(
        &Step::said(Source::System, "The Devin session this turn runs in.").noting(
            SESSION_NOTE,
            json!({"session": session.id(), "model": reported, "resumed": session.resumed}),
        ),
    ) {
        ended.error = Some(error.to_string());
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    if !acp_client::devin::admits(&route.model, reported.as_deref()) {
        host.fail("Devin reported a model different from the admitted model");
        ended.error = Some(format!(
            "Requested {}, Devin reported {}; refusing the turn.",
            route.model,
            reported.as_deref().unwrap_or("no model")
        ));
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    // A reattached session remembers the conversation; a new one is told it.
    // With the delegate recipe, the briefing comes first (#10208).
    let prompt = super::recipe::agent_prompt(recipe.as_deref(), host, session.resumed);
    let model = reported.unwrap_or_else(|| route.model.clone());
    let prompted = match host.effect(
        "devin_prompt",
        json!({"session": session.id(), "prompt": prompt, "model": model}),
    ) {
        Ok(sequence) => sequence,
        Err(error) => {
            ended.error = Some(error.to_string());
            session.close(STOP_GRACE).await;
            return Turn::Ended(ended);
        }
    };
    let mut recorder = Recorder::new(host, "Devin", "devin", model, access);
    let silence = SILENCE;
    let (result, checks_passed) = super::recipe::prompt_watched(
        &mut session,
        &prompt,
        host,
        recipe.as_deref_mut(),
        silence,
        CANCEL_GRACE,
        &mut recorder,
    )
    .await;
    ended.checks_passed = checks_passed;
    recorder.close();
    ended.reply = std::mem::take(&mut recorder.reply);
    ended.refused = recorder.refused.take();
    ended.input_tokens = recorder.input_tokens;
    ended.output_tokens = recorder.output_tokens;
    ended.stats = std::mem::take(&mut recorder.stats);
    ended.tool_calls = recorder.tool_calls;
    let stderr = session.stderr_tail();
    let session_id = session.id().to_owned();
    let group_clear = session.close(STOP_GRACE).await;
    if !group_clear {
        host.fail("the Devin process group did not stop");
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = &session_id;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    keep_delegate(
        host,
        coder_history::Harness::Devin,
        &session_id,
        devin_database(&opening.spec.environment).as_deref(),
        coder_history::devin::delegate,
    );
    let refusal = match &result {
        Ok(reply) => {
            ended.stop = Some(reply.stop_reason);
            if ended.input_tokens == 0
                && let Some(usage) = reply.usage
            {
                ended.input_tokens = usage.input_tokens.unwrap_or_default();
                ended.output_tokens = usage.output_tokens.unwrap_or_default();
            }
            None
        }
        Err(ClientError::Refused { error, .. })
            if ended.tool_calls == 0
                && ended.reply.is_empty()
                && (error.retryable() || error.limited()) =>
        {
            coder::task::capacity::acp_refusal(
                Provider::Devin,
                error,
                coder::task::autostart::unix_now(),
            )
        }
        Err(error) => {
            ended.error = Some(error.to_string());
            book_limit(host, Provider::Devin, error);
            None
        }
    };
    let observation = json!({"stop_reason": ended.stop.map(StopReason::as_str),
        "error": ended.error, "refusal": refusal, "group_clear": group_clear,
        "input_tokens": ended.input_tokens, "output_tokens": ended.output_tokens,
        "turn_stats": ended.stats, "tool_calls": ended.tool_calls,
        "stderr_tail": if ended.error.is_some() { json!(stderr) } else { Value::Null },
        "cost_usd": null, "billing": "unknown"});
    if let Err(error) = host.result(prompted, "devin_prompt", observation) {
        ended.error = Some(error.to_string());
    }
    match refusal {
        Some(refusal) => Turn::Refused(refusal),
        None => Turn::Ended(ended),
    }
}

/// Record in the capacity book a usage or rate limit an ACP agent hit
/// after it started work (#10765). The turn still ends with what it did;
/// the book keeps the next turn, the auto-start policy, and other agents
/// off that login until its reset, and lets the policy resume the task.
pub(crate) fn book_limit(host: &Host, provider: Provider, error: &ClientError) {
    let ClientError::Refused { error, .. } = error else {
        return;
    };
    if !error.limited() {
        return;
    }
    let now = coder::task::autostart::unix_now();
    if let Some(refusal) = coder::task::capacity::acp_refusal(provider, error, now)
        && let Err(why) = coder::task::capacity::record(host.store(), refusal)
    {
        eprintln!("microcoder: the capacity book was not updated: {why}");
    }
}

/// The engine mark Coder's own sessions carry, which the chat list leaves
/// out.
fn coder_history_mark() -> &'static str {
    coder_history::engine::MARK
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::tests::fixture_with;
    use super::super::{AgentEngine, Stage, run_stages};
    use super::*;

    #[test]
    fn the_delegate_copy_reads_devins_store_under_the_agents_data_home() {
        let owned = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            devin_database(&owned(&[("HOME", "/home/owner")])),
            Some(PathBuf::from(
                "/home/owner/.local/share/devin/cli/sessions.db"
            ))
        );
        assert_eq!(
            devin_database(&owned(&[
                ("HOME", "/home/owner"),
                ("XDG_DATA_HOME", "/data")
            ])),
            Some(PathBuf::from("/data/devin/cli/sessions.db"))
        );
    }
    use acp_client::replay;
    use coder::task::adapter::Configuration;
    use coder::task::{self, Action, Command, Store};

    const MODEL: &str = "swe-2-high";

    fn devin(configuration: &mut Configuration, access: Access) {
        configuration.provider = "devin".into();
        configuration.model = MODEL.into();
        configuration.effort = None;
        configuration.generation_endpoint = coder::task::capacity::DEVIN_ENDPOINT.into();
        configuration.decision_endpoint = "https://decision.example.invalid".into();
        configuration.access = access;
    }

    fn jev() -> jev::Client {
        jev::Client::new(
            jev::Config::default()
                .api_key("unused-fixture-key")
                .base_url("https://decision.example.invalid")
                .default_model("fixture-judge"),
        )
        .unwrap()
    }

    fn route() -> GrantRoute {
        GrantRoute {
            provider: "devin".into(),
            model: MODEL.into(),
            effort: None,
            generation_endpoint: coder::task::capacity::DEVIN_ENDPOINT.into(),
        }
    }

    async fn run_turn(store: &std::path::Path, grant: &[u8], agent: PathBuf) -> task::Task {
        let host = Host::admit(store, grant).await.unwrap();
        let stages: Vec<Stage<codex_transport::codex::CodexTransport>> =
            vec![Stage::Agent(AgentEngine::Devin, route(), agent)];
        run_stages(
            host,
            store.to_path_buf(),
            stages,
            Ok(jev()),
            "fixture-session",
            &[],
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn a_devin_route_runs_the_recorded_turn_under_full_access() {
        let (_root, store, grant) = fixture_with(MODEL, |c| devin(c, Access::Full));
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = replay::script(agent_dir.path(), &replay::blocks(replay::DEVIN_TURN));
        let task = run_turn(&store, &grant, agent).await;
        assert_eq!(task.execution, task::Execution::Finished);
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, "model_finished");
        assert_eq!(result.exit_code, Some(0));
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        for expected in [
            "Created `hello.txt` with \\\"hi\\\"",
            "Ran echo, ls",
            "\"session\":\"cheddar-cashew\"",
            "\"kind\":\"devin_prompt\"",
            "Write result.txt containing output.",
            "\"output_tokens\":95.0",
        ] {
            assert!(trace.contains(expected), "missing {expected}");
        }
        assert_eq!(
            replay::arguments(agent_dir.path()),
            vec!["acp", "--model", MODEL]
        );
        let sent = replay::received(agent_dir.path());
        assert_eq!(sent[1]["method"], "session/new");
        assert_eq!(
            sent[1]["params"]["_meta"][acp_client::devin::ENGINE_META_KEY],
            coder_history::engine::MARK
        );
        assert_eq!(sent[2]["params"]["modeId"], "bypass");
        assert_eq!(
            sent[3]["params"]["prompt"][0]["text"],
            "Write result.txt containing output."
        );
    }

    /// The live smoke: the installed Devin CLI, with the owner's login, runs
    /// the fixture's turn under full access. Its Devin session carries the
    /// engine mark, so the host's Devin mirror leaves it out of the chats.
    /// Run with `cargo test -p microcoder --lib live_devin -- --ignored`.
    #[tokio::test]
    #[ignore = "runs the installed Devin CLI with the owner's login and spends Devin credits"]
    async fn live_devin_cli_runs_a_repository_turn() {
        let agent = binary().unwrap();
        let (root, store, grant) = fixture_with(MODEL, |c| devin(c, Access::Full));
        // A real turn takes longer than the fixture's eight seconds.
        let mut grant: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
        grant.wall_seconds = 300;
        let grant = serde_json::to_vec(&grant).unwrap();
        let task = run_turn(&store, &grant, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert_eq!(result.ending, "model_finished", "{result:?}\n{trace}");
        let written = std::fs::read_to_string(root.path().join("checkout/result.txt")).unwrap();
        assert!(written.contains("output"), "{written}");
        assert!(trace.contains("\"kind\":\"devin_prompt\""));
        eprintln!("{result:?}");
    }

    #[tokio::test]
    async fn the_boundary_runs_devin_sandboxed_and_refuses_what_it_asks() {
        let (_root, store, grant) = fixture_with(MODEL, |c| devin(c, Access::Boundary));
        let agent_dir = tempfile::tempdir().unwrap();
        let mut blocks = replay::blocks(replay::DEVIN_TURN);
        let ask = serde_json::json!({"jsonrpc":"2.0","id":"ask-1","method":"session/request_permission",
            "params":{"sessionId":"cheddar-cashew","toolCall":{"kind":"execute","title":"Ran echo"},
            "options":[{"optionId":"allow_once","kind":"allow_once"},{"optionId":"reject_once","kind":"reject_once"}]}});
        blocks[3].insert(0, ask);
        let agent = replay::script(agent_dir.path(), &blocks);
        let task = run_turn(&store, &grant, agent).await;
        assert_eq!(
            task.run.as_ref().unwrap().result.as_ref().unwrap().ending,
            "model_finished"
        );
        assert_eq!(
            replay::arguments(agent_dir.path()),
            vec!["--sandbox", "acp", "--model", MODEL]
        );
        let sent = replay::received(agent_dir.path());
        assert_eq!(sent[2]["params"]["modeId"], "accept-edits");
        let answer = sent
            .iter()
            .find(|line| line["id"] == "ask-1")
            .expect("the permission answer");
        assert_eq!(answer["result"]["outcome"]["optionId"], "reject_once");
    }

    #[tokio::test]
    async fn a_follow_up_reattaches_the_same_devin_session() {
        let (_root, store, grant) = fixture_with(MODEL, |c| devin(c, Access::Full));
        let first_dir = tempfile::tempdir().unwrap();
        let first = replay::script(first_dir.path(), &replay::blocks(replay::DEVIN_TURN));
        let ended = run_turn(&store, &grant, first).await;
        let follow_up = Command {
            schema: task::COMMAND_SCHEMA.into(),
            command_id: "follow-up-fixture".into(),
            task_id: "fixture".into(),
            expected_revision: Some(ended.revision),
            action: Action::Continue {
                prompt: "Now delete hello.txt.".into(),
            },
        };
        let receipt = Store::open(&store)
            .unwrap()
            .apply(&serde_json::to_vec(&follow_up).unwrap())
            .unwrap();
        let mut next: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
        next.expected_revision = receipt.revision;
        let mut blocks = replay::blocks(replay::DEVIN_TURN);
        // session/load answers with the session's modes and options, and
        // no new session ID.
        let mut loaded = blocks[1].last().unwrap().clone();
        loaded["result"]
            .as_object_mut()
            .unwrap()
            .remove("sessionId");
        blocks[1] = vec![loaded];
        let second_dir = tempfile::tempdir().unwrap();
        let second = replay::script(second_dir.path(), &blocks);
        let task = run_turn(&store, &serde_json::to_vec(&next).unwrap(), second).await;
        assert_eq!(task.turn(), 2);
        let sent = replay::received(second_dir.path());
        assert_eq!(sent[1]["method"], "session/load");
        assert_eq!(sent[1]["params"]["sessionId"], "cheddar-cashew");
        // The reattached session remembers the conversation: only the new
        // message is sent.
        assert_eq!(
            sent[3]["params"]["prompt"][0]["text"],
            "Now delete hello.txt."
        );
        let trace = std::fs::read_to_string(store.join("fixture.2.atif.jsonl")).unwrap();
        assert!(trace.contains("\"resumed\":true"));
    }

    #[tokio::test]
    async fn a_retryable_refusal_is_recorded_and_ends_without_capacity() {
        let (_root, store, grant) = fixture_with(MODEL, |c| devin(c, Access::Full));
        let agent_dir = tempfile::tempdir().unwrap();
        let mut blocks = replay::blocks(replay::DEVIN_TURN);
        blocks[3] = vec![serde_json::json!({"jsonrpc":"2.0","id":4,
            "error":{"code":-32000,"message":"rate limited","data":{"retryable":true}}})];
        let agent = replay::script(agent_dir.path(), &blocks);
        let task = run_turn(&store, &grant, agent).await;
        assert_eq!(
            task.run.as_ref().unwrap().result.as_ref().unwrap().ending,
            coder::task::capacity::NO_CAPACITY_ENDING
        );
        let now = coder::task::autostart::unix_now();
        let book = coder::task::capacity::Book::load(&store);
        assert!(!book.has_capacity(Provider::Devin, now));
        // The next turn passes over Devin without starting it.
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("route_exhausted"));
    }

    #[test]
    fn a_devin_route_is_closed() {
        let (_root, _store, grant) = fixture_with(MODEL, |c| devin(c, Access::Full));
        let grant = task::owner::Grant::parse(&grant).unwrap();
        let configuration = grant.adapter_configuration.unwrap();
        configuration.validate().unwrap();
        let mut effort = configuration.clone();
        effort.effort = Some("medium".into());
        assert!(effort.validate().is_err());
        let mut endpoint = configuration.clone();
        endpoint.generation_endpoint = "https://api.devin.ai".into();
        assert!(endpoint.validate().is_err());
        let mut fallback = configuration.clone();
        fallback.fallbacks = vec![GrantRoute {
            provider: "codex".into(),
            model: "gpt-6-luna".into(),
            effort: Some("medium".into()),
            generation_endpoint: coder::task::capacity::Provider::Codex.endpoint().into(),
        }];
        fallback.validate().unwrap();
        assert_eq!(
            configuration.capabilities()["steering"]["adapter"],
            "devin-acp"
        );
    }

    #[tokio::test]
    async fn a_model_other_than_the_admitted_one_is_refused() {
        let (_root, store, grant) = fixture_with("claude-opus-5-5-medium", |c| {
            devin(c, Access::Full);
            c.model = "claude-opus-5-5-medium".into();
        });
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = replay::script(agent_dir.path(), &replay::blocks(replay::DEVIN_TURN));
        let host = Host::admit(&store, &grant).await.unwrap();
        let mut admitted = route();
        admitted.model = "claude-opus-5-5-medium".into();
        let stages: Vec<Stage<codex_transport::codex::CodexTransport>> =
            vec![Stage::Agent(AgentEngine::Devin, admitted, agent)];
        let task = run_stages(
            host,
            store.clone(),
            stages,
            Ok(jev()),
            "fixture-session",
            &[],
        )
        .await
        .unwrap();
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_ne!(result.exit_code, Some(0));
        // No prompt was sent.
        let sent = replay::received(agent_dir.path());
        assert!(sent.iter().all(|line| line["method"] != "session/prompt"));
    }
}
