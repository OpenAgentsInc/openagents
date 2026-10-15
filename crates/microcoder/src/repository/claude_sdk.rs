//! A repository turn on the Claude Agent SDK (#10571).
//!
//! A Claude route whose endpoint is [`CLAUDE_SDK_ENDPOINT`] (a seat on
//! `claude/sdk:MODEL`) runs its turn as one Claude Code session driven by
//! `claude_agent_sdk` over the CLI's control protocol, instead of a lean
//! session that bypasses permissions. The difference is the permission
//! callback: Claude Code sends each tool request it would prompt for to
//! the host ([`Gate`]), and the host answers it.
//!
//! - **Inside the worktree**: a file tool (`Read`, `Edit`, `Write`,
//!   `Glob`, `Grep`) whose every path resolves inside the task's worktree
//!   (the Coder SDK's worktree guard, `coder_sdk::worktree`) is allowed,
//!   unless a floor names it (`coder_sdk::floors`, #11527: a secret file).
//! - **Outside the worktree**: any other request, including every `Bash`
//!   command, whose effects the host cannot bound, and anything a floor
//!   names (a deploy, a merge, a secret, a payment, DNS), ends the turn asking
//!   the person to approve it. The host denies the request with
//!   `interrupt`, so Claude Code stops, and the turn's reply names the
//!   step in a fenced `openagents.coder.approval-step.v1` block
//!   ([`interaction::Step`]); the run's ending is `asked_approval`. The
//!   studio shows it as an approval, and the host's standing rules
//!   (`studio::rules`) answer it when one matches.
//! - **The answer**: the next turn resumes the session with the answer.
//!   **Allow once** (or a standing rule) allows that exact request, by
//!   the digest of its input, once; **Deny** refuses it, and Claude Code
//!   is told not to ask again; an answer in the person's own words
//!   approves nothing. An answer never widens the grant: the next turn
//!   runs under a fresh one with every usual check.
//!
//! The rest follows the lean session ([`super::claude_session`]):
//!
//! - **Login**: the owner's Claude Code login. The host reads no API key,
//!   and the CLI does not inherit `ANTHROPIC_API_KEY` or
//!   `ANTHROPIC_AUTH_TOKEN`, so it cannot use one in place of the login.
//!   No user or project settings load (`--setting-sources=`), so no
//!   allow rule from the owner's own Claude Code settings bypasses the
//!   host.
//! - **Process**: the route's model and effort, the lean session's six
//!   tools, in the worktree, with the owner's login environment, kept out
//!   of the checkout the worktree came from (`Host::private_argv`). Full
//!   access only: Claude Code's own tools run outside the host's boundary,
//!   and the policy refuses `claude/sdk` under any other access.
//! - **Follow-ups**: a later turn resumes the session (`--resume`); one
//!   that can't resume starts afresh, once.
//! - **Cost**: the result message's `total_cost_usd` (list price on a
//!   subscription), with the cache reads and writes in the turn's stats.
//! - **Capacity**: a session that ends on a usage or rate limit before it
//!   did any work is a refusal for the capacity book.
//! - **Cancellation**: a cancelled task stops the `claude` process group.

mod live;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use acp_client::StopReason;
use atif::{Call, Outcome as CallOutcome, Source, Step};
use claude_agent_sdk::{
    EffortLevel, ExecutableConfig, PermissionMode, PermissionResult, QueryOptions, SdkMessage,
    SdkResultMessage, ToolsConfig,
};
use coder::task::adapter::{Host, Route as GrantRoute};
use coder::task::capacity::{Kind, Provider, Refusal};
use coder::task::interaction;
use coder::task::studio::approvals::Verdict;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::devin::{Ended, Turn};
use super::lean_session::{bounded, input_parts};

pub use coder::task::capacity::CLAUDE_SDK_ENDPOINT;

/// The step extension that names the session a turn used and the request
/// it asked the person to approve, which the next turn reads.
pub const SESSION_NOTE: &str = "claude_sdk_session";
/// The engine an Agent SDK turn records.
pub const ENGINE: &str = "claude-agent-sdk";
/// The effect kind of one session.
const EFFECT: &str = "claude_sdk";
/// The tools the session runs with: the lean session's six.
pub const TOOLS: [&str; 6] = ["Bash", "Read", "Edit", "Write", "Glob", "Grep"];
/// The effort when neither the recipe nor the route names one.
pub const EFFORT: &str = "medium";
/// Credentials the CLI must not inherit, so it uses the Claude Code login.
pub const REMOVED_ENV: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];
/// How often the host looks whether the task was cancelled.
const TICK: Duration = Duration::from_millis(500);
/// How long Claude Code may take to end its turn after the host asked.
const ASK_GRACE: Duration = Duration::from_secs(10);
/// The most bytes of one tool output the transcript keeps.
const TOOL_OUTPUT: usize = 16 * 1024;
/// The most bytes of one reply the transcript keeps.
const SEGMENT: usize = 256 * 1024;
/// The most bytes of one string in a tool input the transcript keeps.
const INPUT_STRING: usize = 1024;
/// The most bytes of a step's command (`MAX_STEP_COMMAND`).
const STEP_COMMAND: usize = 512;
/// The most bytes of a step's reason (`MAX_STEP_REASON`).
const STEP_REASON: usize = 512;

/// What Claude Code tells the model when the host stops to ask.
const ASKED: &str = "This step is outside the task's worktree, so OpenAgents asks the person to approve it. Your turn ends here; the next message tells you their answer.";
/// What Claude Code tells the model about a step the person denied.
const DENIED: &str = "The person denied this step. Don't run it or ask for it again; go on with the task another way, or end the turn and say what you need.";

/// One tool request: the tool and the digest of its exact input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub tool: String,
    pub digest: String,
}

impl Request {
    pub(crate) fn of(tool: &str, input: &Value) -> Self {
        let digest = nostr::contracts::digest_value(input).unwrap_or_else(|_| {
            nostr::contracts::digest_bytes(&serde_json::to_vec(input).unwrap_or_default())
        });
        Self {
            tool: tool.to_owned(),
            digest,
        }
    }
}

/// The request a turn asked the person to approve, as the next turn
/// reads it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pending {
    pub request: Request,
    /// The step the approval named.
    pub step: interaction::Step,
}

/// How the host answered one request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Decision {
    /// A file tool inside the worktree: allowed.
    Inside,
    /// The request the person allowed once: allowed, once.
    AllowedOnce,
    /// The request the person denied: refused.
    Denied,
    /// Outside the worktree: refused, and the turn ends asking the person.
    Asked,
    /// Another request after the turn asked: refused while it ends.
    #[cfg(test)]
    Waiting,
}

#[derive(Default)]
struct GateState {
    once: Option<Request>,
    denied: Option<Request>,
    asked: Option<Pending>,
    log: Vec<(String, Value, Decision)>,
    native: HashMap<(u64, String, Option<String>), Request>,
    question: Option<route_contract::pending::Questionnaire>,
}

/// The host's side of Claude Code's permission callback for one turn.
pub(crate) struct Gate {
    /// The worktree, against which relative paths resolve.
    base: PathBuf,
    /// The Coder SDK's worktree guard on it (#11527).
    guard: coder_sdk::worktree::Guard,
    state: Mutex<GateState>,
}

impl Gate {
    /// A gate for `workspace` that allows `once` once and refuses `denied`.
    pub(crate) fn new(workspace: &Path, once: Option<Request>, denied: Option<Request>) -> Self {
        Self {
            base: workspace.to_path_buf(),
            guard: coder_sdk::worktree::Guard::new(workspace),
            state: Mutex::new(GateState {
                once,
                denied,
                ..GateState::default()
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The answer to Claude Code's request to run `tool` with `input`.
    #[cfg(test)]
    pub(crate) fn decide(&self, tool: &str, input: &Value) -> PermissionResult {
        let request = Request::of(tool, input);
        let mut state = self.state();
        let (decision, result) = if state.asked.is_some() {
            (
                Decision::Waiting,
                PermissionResult::deny_and_interrupt(ASKED),
            )
        } else if state.once.as_ref() == Some(&request) {
            state.once = None;
            (
                Decision::AllowedOnce,
                PermissionResult::allow(input.clone()),
            )
        } else if state.denied.as_ref() == Some(&request) {
            (Decision::Denied, PermissionResult::deny(DENIED))
        } else if self.inside(tool, input) {
            (Decision::Inside, PermissionResult::allow(input.clone()))
        } else {
            state.asked = Some(Pending {
                step: step(tool, input, &self.base),
                request,
            });
            (Decision::Asked, PermissionResult::deny_and_interrupt(ASKED))
        };
        state.log.push((tool.to_owned(), brief(input), decision));
        result
    }

    /// Whether `tool` with `input` runs without asking: a file tool the
    /// Coder SDK's worktree guard keeps inside the worktree (a `Glob` or
    /// `Grep` that names no path searches it, Claude Code's working
    /// directory), on nothing a floor names (`coder_sdk::floors`: a secret
    /// file is the person's to open).
    fn inside(&self, tool: &str, input: &Value) -> bool {
        tool == live::ASK_TOOL
            || coder_sdk::worktree::FILE_TOOLS.contains(&tool)
                && self.guard.refusal(tool, input).is_none()
                && coder_sdk::floors::classify(tool, input).is_none()
    }

    /// The request this turn stopped to ask about, if it did.
    pub(crate) fn asked(&self) -> Option<Pending> {
        self.state().asked.clone()
    }

    /// The decisions made since the last call, oldest first.
    fn drain(&self) -> Vec<(String, Value, Decision)> {
        std::mem::take(&mut self.state().log)
    }
}

/// `input` with each long string cut, for the transcript.
fn brief(input: &Value) -> Value {
    match input {
        Value::String(text) => Value::String(bounded(text, INPUT_STRING)),
        Value::Array(items) => Value::Array(items.iter().map(brief).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), brief(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The approval step for `tool` with `input`, run in `cwd`: a `Bash`
/// request's exact command, a file tool's path, or else the input itself.
pub(crate) fn step(tool: &str, input: &Value, cwd: &Path) -> interaction::Step {
    let text = |field: &str| input.get(field).and_then(Value::as_str);
    let command = match tool {
        "Bash" => text("command").map(str::to_owned),
        _ => coder_sdk::worktree::PATH_FIELDS
            .iter()
            .find_map(|field| text(field))
            .map(str::to_owned),
    }
    .unwrap_or_else(|| serde_json::to_string(input).unwrap_or_default());
    let reason = text("description")
        .filter(|reason| reason.len() <= STEP_REASON)
        .unwrap_or_default();
    interaction::Step {
        schema: interaction::STEP_SCHEMA.into(),
        tool: tool.to_owned(),
        // A longer command no longer names a valid step: the approval is
        // then text alone, and no standing rule can match it.
        command: bounded(&command, STEP_COMMAND * 4),
        cwd: cwd.display().to_string(),
        reason: reason.to_owned(),
    }
}

/// The turn's reply when it stops to ask: the request in words, and the
/// step in a fenced block when it is a valid one.
pub(crate) fn approval_reply(step: &interaction::Step) -> String {
    // Every command asks; a file tool asks only outside the worktree.
    let mut reply = if step.tool == "Bash" {
        "Claude Code asks to run a command. May it go ahead?".to_owned()
    } else {
        format!(
            "Claude Code asks to use {} outside this task's worktree. May it go ahead?",
            step.tool
        )
    };
    if step.valid()
        && let Ok(block) = serde_json::to_string_pretty(step)
    {
        reply.push_str(&format!("\n\n```json\n{block}\n```\n"));
    } else {
        reply.push_str(&format!("\n\n```text\n{}\n```\n", step.command));
    }
    reply
}

/// What an answer to an approval decides. A standing rule's answer
/// (`studio::rules::Due::answer`) approves, as **Allow once** does.
#[must_use]
pub fn verdict(answer: &str) -> Verdict {
    if answer
        .trim_start()
        .starts_with("Approved by a standing rule")
    {
        Verdict::Approve
    } else {
        Verdict::of(answer)
    }
}

/// What Claude Code is told about the person's answer to `step`.
fn answer_note(verdict: Verdict, step: &interaction::Step, answer: &str) -> String {
    let named = format!("{} `{}`", step.tool, bounded(&step.command, STEP_COMMAND));
    match verdict {
        Verdict::Approve => format!(
            "The person allowed this step once: {named}. Run it now, exactly as you asked for it, then go on with the task."
        ),
        Verdict::Deny => format!("The person denied this step: {named}. {DENIED}"),
        Verdict::Reply => format!(
            "The person answered your request to run {named} in their own words, which approves nothing:\n\n{answer}"
        ),
    }
}

/// What one session's messages leave in the transcript.
struct Transcript<'a> {
    host: &'a Host,
    model: String,
    tool_calls: Cell<usize>,
    reply: RefCell<String>,
    /// Tool uses waiting for their results: their names and inputs.
    calls: RefCell<HashMap<String, (String, Value)>>,
    observations: RefCell<coder_sdk::observation::Claude>,
    /// The first request the person denied this turn.
    refused: RefCell<Option<String>>,
}

impl Transcript<'_> {
    fn append(&self, step: &Step) {
        if let Err(error) = self.host.append(step) {
            self.host.fail(error.to_string());
        }
    }

    fn observe(&self, observations: Vec<coder_sdk::observation::Observation>) {
        use coder_sdk::observation::{Body, Channel, TRACE_KEY};
        for observation in observations {
            if observation.child.is_some() {
                if let Err(error) = self.host.observe(&observation) {
                    self.host.fail(error.to_string());
                }
                continue;
            }
            let step = match &observation.body {
                Body::Text {
                    channel: Channel::Text,
                    text,
                    ..
                } => {
                    *self.reply.borrow_mut() = text.clone();
                    Step::said(Source::Agent, text).by(&self.model)
                }
                Body::ToolStarted { name, input } => {
                    if let Some(id) = &observation.item {
                        self.calls.borrow_mut().insert(
                            id.clone(),
                            (
                                name.clone(),
                                serde_json::from_str(input).unwrap_or(Value::Null),
                            ),
                        );
                    }
                    Step::said(Source::System, "")
                }
                Body::ToolResult { text, failed, .. } => {
                    let call = observation
                        .item
                        .as_ref()
                        .and_then(|id| self.calls.borrow_mut().remove(id));
                    if let Some((name, input)) = call {
                        self.tool_calls.set(self.tool_calls.get() + 1);
                        Step::called(Call {
                            id: format!("{EFFECT}-{}", observation.item.as_deref().unwrap_or("")),
                            name,
                            arguments: brief(&input),
                            output: bounded(text, TOOL_OUTPUT),
                            outcome: if *failed {
                                CallOutcome::Failed
                            } else {
                                CallOutcome::Completed
                            },
                            milliseconds: 0,
                            purpose: None,
                            extra: serde_json::Map::new(),
                        })
                        .by(&self.model)
                        .noting("claude_sdk_tool", json!({"is_error": failed}))
                    } else {
                        Step::said(Source::System, "")
                    }
                }
                _ => {
                    if let Err(error) = self.host.observe(&observation) {
                        self.host.fail(error.to_string());
                    }
                    continue;
                }
            };
            self.append(&step.noting(TRACE_KEY, json!(observation)));
        }
    }

    fn assistant(&self, message: &Value, child: Option<&str>) {
        let mut observations = self.observations.borrow_mut().assistant(message);
        for observation in &mut observations {
            observation.child = child.map(str::to_owned);
        }
        self.observe(observations);
    }

    fn user(&self, message: &Value, child: Option<&str>) {
        let mut observations = self.observations.borrow_mut().user(message);
        for observation in &mut observations {
            observation.child = child.map(str::to_owned);
        }
        self.observe(observations);
    }

    fn session(&self, session: &str) {
        let observation = self.observations.borrow_mut().session(session);
        self.observe(vec![observation]);
    }

    /// Record the gate's decisions since the last call.
    fn decisions(&self, gate: &Gate) {
        for (tool, input, decision) in gate.drain() {
            let text = match decision {
                Decision::Inside => {
                    format!("The host allowed Claude Code's {tool} in the worktree.")
                }
                Decision::AllowedOnce => {
                    format!("The host allowed Claude Code's {tool} once, as the person answered.")
                }
                Decision::Denied => {
                    self.refused
                        .borrow_mut()
                        .get_or_insert_with(|| format!("the person denied {tool}"));
                    format!("The host refused Claude Code's {tool}: the person denied it.")
                }
                Decision::Asked => format!(
                    "Claude Code asked to use {tool} where the host asks first; the turn ends asking the person."
                ),
                Decision::Waiting => format!(
                    "The host refused Claude Code's {tool}: the turn is ending to ask the person."
                ),
            };
            let verdict = match decision {
                Decision::Inside => "allowed",
                Decision::AllowedOnce => "allowed once",
                Decision::Denied => "denied",
                Decision::Asked => "needs approval",
                Decision::Waiting => "waiting for approval",
            };
            let observation = self.observations.borrow_mut().next(
                None,
                coder_sdk::observation::Body::Policy {
                    tool: tool.clone(),
                    decision: verdict.into(),
                },
            );
            self.append(
                &Step::said(Source::System, &text)
                    .noting(
                        "claude_sdk_permission",
                        json!({"tool": tool, "input": input, "decision": decision}),
                    )
                    .noting(coder_sdk::observation::TRACE_KEY, json!(observation)),
            );
        }
    }
}

/// What a session's result message reported.
#[derive(Debug, Default)]
struct Finished {
    success: bool,
    text: String,
    cost_usd: Option<f64>,
    usage: Option<Value>,
    num_turns: Option<u32>,
    api_error_status: Option<i64>,
    errors: Vec<String>,
    session: Option<String>,
}

impl Finished {
    fn of(result: &SdkResultMessage) -> Self {
        match result {
            SdkResultMessage::Success(success) => Self {
                success: !success.is_error,
                text: success.result.clone(),
                cost_usd: Some(success.total_cost_usd),
                usage: serde_json::to_value(&success.usage).ok(),
                num_turns: Some(success.num_turns),
                api_error_status: success.api_error_status,
                errors: Vec::new(),
                session: Some(success.session_id.clone()),
            },
            SdkResultMessage::ErrorDuringExecution(error)
            | SdkResultMessage::ErrorMaxTurns(error)
            | SdkResultMessage::ErrorMaxBudget(error)
            | SdkResultMessage::ErrorMaxStructuredOutputRetries(error) => Self {
                success: false,
                text: String::new(),
                cost_usd: Some(error.total_cost_usd),
                usage: serde_json::to_value(&error.usage).ok(),
                num_turns: Some(error.num_turns),
                api_error_status: None,
                errors: error.errors.clone(),
                session: Some(error.session_id.clone()),
            },
        }
    }

    /// A result line a later CLI writes that the typed message does not
    /// read: its common fields.
    fn raw(raw: &Value) -> Self {
        let text = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_owned);
        Self {
            success: raw.get("subtype").and_then(Value::as_str) == Some("success")
                && raw.get("is_error").and_then(Value::as_bool) != Some(true),
            text: text("result").unwrap_or_default(),
            cost_usd: raw.get("total_cost_usd").and_then(Value::as_f64),
            usage: raw.get("usage").cloned(),
            num_turns: raw
                .get("num_turns")
                .and_then(Value::as_u64)
                .and_then(|turns| u32::try_from(turns).ok()),
            api_error_status: raw.get("api_error_status").and_then(Value::as_i64),
            errors: raw
                .get("errors")
                .and_then(Value::as_array)
                .map(|errors| {
                    errors
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            session: text("session_id"),
        }
    }

    /// The usage or rate limit the session ended on, if it ended on one.
    fn limit(&self) -> Option<Kind> {
        if self.success {
            return None;
        }
        let text = format!("{} {}", self.text, self.errors.join(" ")).to_lowercase();
        if self.api_error_status == Some(429) || text.contains("rate limit") {
            Some(Kind::RateLimit)
        } else if text.contains("usage limit") || text.contains("limit reached") {
            Some(Kind::UsageLimit)
        } else {
            None
        }
    }
}

/// What one session attempt left.
#[derive(Debug, Default)]
struct Attempt {
    session: Option<String>,
    finished: Option<Finished>,
    cancelled: bool,
    error: Option<String>,
}

/// The worktree gate as a Coder SDK floor (#11496): `can_use_tool` runs it
/// through the Claude Code backend's permission channel. An allow keeps the
/// request's input; a refusal that interrupts ends the turn.
#[cfg(test)]
fn gate_hook(gate: &Arc<Gate>) -> coder_sdk::Hook {
    let gate = gate.clone();
    coder_sdk::Hook::floor(
        "worktree",
        coder_sdk::Event::PreToolUse,
        coder_sdk::Permission::deny(
            "OpenAgents could not check whether this step stays in the task's worktree, so it did not run.",
        ),
        move |call: &coder_sdk::Call| {
            let tool = call.tool.as_deref().unwrap_or_default();
            Ok(match gate.decide(tool, &call.input) {
                PermissionResult::Allow { .. } => coder_sdk::HookOutcome::allow(),
                PermissionResult::Deny {
                    message, interrupt, ..
                } => coder_sdk::HookOutcome {
                    stop: if interrupt == Some(true) {
                        coder_sdk::Stop::End
                    } else {
                        coder_sdk::Stop::Continue
                    },
                    ..coder_sdk::HookOutcome::deny(&message)
                },
            })
        },
    )
}

/// The `can_use_tool` callback for a session gated by `gate`.
#[cfg(test)]
fn sdk_handler(gate: &Arc<Gate>) -> Arc<dyn claude_agent_sdk::PermissionHandler> {
    let mut registry = coder_sdk::Registry::new();
    let _ = registry.register(gate_hook(gate));
    coder_sdk::claude::Claude::new(registry, coder_sdk::Budget::default())
        .expect("every hook event is native on Claude Code")
        .permission_handler()
}

/// Run one session with `options` on `prompt` until its result, a
/// cancellation, or the end of its output, answering its permission
/// requests through `gate`. The `claude` process group is stopped before
/// this returns.
async fn attempt(
    host: &Host,
    gate: &Arc<Gate>,
    options: QueryOptions,
    prompt: &str,
    transcript: &Transcript<'_>,
) -> Attempt {
    let mut attempt = Attempt::default();
    let service = match host.pending(transcript.observations.borrow().scope()) {
        Ok(service) => service,
        Err(error) => {
            attempt.error = Some(error.to_string());
            return attempt;
        }
    };
    let handler = live::handler(gate, &service);
    let options = options.sdk_mcp_server(live::questions(gate, &service));
    let mut query = match claude_agent_sdk::query_with_permissions(prompt, options, handler).await {
        Ok(query) => query,
        Err(error) => {
            let _ = service.close();
            attempt.error = Some(format!("Claude Code didn't start: {error}"));
            return attempt;
        }
    };
    // Where the plan stands at session start, from Claude Code's own
    // `get_usage` (#11292): recorded beside the windows its
    // `rate_limit_event`s report. Nothing here reads the login.
    if let Ok(Ok(usage)) = tokio::time::timeout(USAGE_WAIT, query.get_usage()).await {
        record_usage(host.store(), &usage, unix_now());
    }
    let mut checked = Instant::now();
    let mut asked_at: Option<Instant> = None;
    loop {
        transcript.decisions(gate);
        if asked_at.is_none() && (gate.asked().is_some() || gate.state().question.is_some()) {
            asked_at = Some(Instant::now());
        }
        if asked_at.is_some_and(|at| at.elapsed() > ASK_GRACE) {
            break;
        }
        if checked.elapsed() >= TICK {
            checked = Instant::now();
            if host.cancelled() {
                attempt.cancelled = true;
                break;
            }
        }
        let next = tokio::select! {
            message = query.next() => Some(message),
            () = tokio::time::sleep(TICK) => None,
        };
        let message = match next {
            None => continue,
            Some(None) => {
                if attempt.finished.is_none() {
                    attempt.error = Some("Claude Code exited before it reported a result".into());
                }
                break;
            }
            Some(Some(Err(claude_agent_sdk::Error::UnrecognizedMessage { .. }))) => continue,
            Some(Some(Err(error))) => {
                attempt.error = Some(error.to_string());
                break;
            }
            Some(Some(Ok(message))) => message,
        };
        if let Some(session) = query.session_id()
            && attempt.session.as_deref() != Some(session)
        {
            transcript.session(session);
            attempt.session = Some(session.to_owned());
        }
        match message {
            SdkMessage::Assistant(assistant) => {
                let child = assistant
                    .agent_id
                    .as_deref()
                    .or(assistant.parent_tool_use_id.as_deref());
                if child.is_none() {
                    if attempt.session.as_deref() != Some(&assistant.session_id) {
                        transcript.session(&assistant.session_id);
                    }
                    attempt.session = Some(assistant.session_id.clone());
                }
                let mut message = assistant.message;
                if message.get("id").is_none() {
                    message["id"] = json!(assistant.uuid);
                }
                transcript.assistant(&message, child);
            }
            SdkMessage::User(user) => {
                transcript.user(&user.message, user.parent_tool_use_id.as_deref())
            }
            SdkMessage::RateLimitEvent(event) => {
                if let Ok(info) = serde_json::to_value(&event.rate_limit_info) {
                    record_window(host.store(), &info, unix_now());
                    let status = info["status"].as_str().unwrap_or("unknown").to_owned();
                    let observation = transcript.observations.borrow_mut().next(
                        None,
                        coder_sdk::observation::Body::RateLimit {
                            status,
                            resets_at: info["resetsAt"].as_u64(),
                        },
                    );
                    transcript.observe(vec![observation]);
                }
            }
            SdkMessage::Result(result) => {
                attempt.finished = Some(Finished::of(&result));
                break;
            }
            SdkMessage::Unknown { type_name, raw } if type_name == "result" => {
                attempt.finished = Some(Finished::raw(&raw));
                break;
            }
            _ => {}
        }
    }
    transcript.decisions(gate);
    // The CLI waits for more input after its result: stop it, and every
    // process it started.
    let _ = service.close();
    let _ = query.kill().await;
    if let Some(session) = attempt
        .finished
        .as_ref()
        .and_then(|finished| finished.session.clone())
    {
        if attempt.session.as_deref() != Some(&session) {
            transcript.session(&session);
        }
        attempt.session = Some(session);
    }
    attempt
}

/// How long a session start waits for Claude Code's `get_usage`.
const USAGE_WAIT: Duration = Duration::from_secs(5);

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Records one `rate_limit_event`'s window into the usage book in `store`.
fn record_window(store: &Path, info: &Value, now: u64) {
    if let Some(window) = microcoder_loop::usage::rate_limit_window(info, now) {
        let _ = microcoder_loop::usage::record_reported(store, Provider::Claude, window, now);
    }
}

/// Records the windows a `get_usage` answer names into the usage book in
/// `store`: the same `five_hour` / `seven_day` shape as the plan's usage
/// endpoint, at the top or under `usage`. Anything else is left unread.
fn record_usage(store: &Path, answer: &Value, now: u64) {
    let reading = [answer, &answer["usage"], &answer["response"]]
        .into_iter()
        .filter(|value| value.is_object())
        .find_map(|value| {
            microcoder_loop::usage::parse_claude(value.to_string().as_bytes(), now).ok()
        });
    for window in reading.into_iter().flat_map(|reading| reading.windows) {
        let _ = microcoder_loop::usage::record_reported(store, Provider::Claude, window, now);
    }
}

/// The session's options: the route's model and effort, the six tools,
/// no settings files, permission prompts to the host, and the owner's
/// login environment without an API key.
fn options(
    host: &Host,
    program: PathBuf,
    model: &str,
    effort: &str,
    resume: Option<String>,
    env: HashMap<String, String>,
) -> QueryOptions {
    let (program, wrapper) = host.private_argv(program, Vec::new());
    let mut options = QueryOptions::new()
        .cwd(host.workspace())
        .model(model)
        .permission_mode(PermissionMode::Default);
    options.effort = match effort {
        "low" => Some(EffortLevel::Low),
        "high" => Some(EffortLevel::High),
        "xhigh" => Some(EffortLevel::Xhigh),
        _ => Some(EffortLevel::Medium),
    };
    options.tools = Some(ToolsConfig::Names(
        TOOLS.iter().map(|&tool| tool.to_owned()).collect(),
    ));
    options.setting_sources = Some(Vec::new());
    options.resume = resume;
    options.executable = ExecutableConfig {
        path: Some(program),
        executable: None,
        executable_args: wrapper,
    };
    options.env = Some(env);
    options.env_remove = REMOVED_ENV.iter().map(|&name| name.to_owned()).collect();
    options
}

/// The CLI's environment: the owner's login environment and the guard's
/// additions, without the credentials in [`REMOVED_ENV`].
async fn environment(host: &Host) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = host
        .login_environment()
        .await
        .map(|login| {
            login
                .variables
                .iter()
                .filter_map(|(key, value)| {
                    Some((
                        key.clone().into_string().ok()?,
                        value.clone().into_string().ok()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    for (key, value) in host.guard_environment() {
        if let (Ok(key), Ok(value)) = (key.into_string(), value.into_string()) {
            env.insert(key, value);
        }
    }
    env.retain(|key, _| !REMOVED_ENV.contains(&key.as_str()));
    env
}

/// Runs the turn as one Claude Code session on the Agent SDK.
pub(crate) async fn turn(
    host: &Host,
    route: &GrantRoute,
    program: PathBuf,
    recipe: Option<&mut super::recipe::Recipe>,
) -> Turn {
    let mut ended = Ended {
        engine: ENGINE,
        agent: "Claude Code",
        model: Some(route.model.clone()),
        ..Ended::default()
    };
    let effort = recipe
        .as_deref()
        .and_then(|recipe| {
            recipe.effort(
                route_contract::recipe::CLAUDE_SESSION,
                route.effort.as_deref(),
            )
        })
        .or_else(|| route.effort.clone())
        .unwrap_or_else(|| EFFORT.to_owned());
    let note = host.earlier_note(SESSION_NOTE);
    let earlier = note
        .as_ref()
        .and_then(|note| note.get("session"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let pending: Option<Pending> = note
        .as_ref()
        .and_then(|note| note.get("pending"))
        .cloned()
        .and_then(|pending| serde_json::from_value(pending).ok());
    // The earlier turn asked: this turn's message is the answer.
    let answered = pending
        .as_ref()
        .map(|pending| (pending, verdict(host.prompt())));
    let (once, denied) = match answered {
        Some((pending, Verdict::Approve)) => (Some(pending.request.clone()), None),
        Some((pending, Verdict::Deny)) => (None, Some(pending.request.clone())),
        _ => (None, None),
    };
    if let Some((pending, verdict)) = answered {
        // The answer is read: a later turn never applies it again.
        let _ = host.append(
            &Step::said(
                Source::System,
                &format!(
                    "The person's answer to Claude Code's {} request: {}.",
                    pending.step.tool,
                    match verdict {
                        Verdict::Approve => "allowed once",
                        Verdict::Deny => "denied",
                        Verdict::Reply => "a reply that approves nothing",
                    }
                ),
            )
            .noting(
                SESSION_NOTE,
                json!({"session": earlier, "pending": null,
                    "answered": {"request": pending.request, "verdict": verdict}}),
            ),
        );
    }
    let gate = Arc::new(Gate::new(host.workspace(), once, denied));
    let env = environment(host).await;
    let transcript = Transcript {
        host,
        model: route.model.clone(),
        tool_calls: Cell::new(0),
        reply: RefCell::new(String::new()),
        calls: RefCell::new(HashMap::new()),
        observations: RefCell::new(coder_sdk::observation::Claude::new(host.observations())),
        refused: RefCell::new(None),
    };
    let mut resume = earlier.clone();
    let mut tries = 0;
    let attempt = loop {
        tries += 1;
        if tries > 1 {
            transcript.calls.borrow_mut().clear();
            transcript.reply.borrow_mut().clear();
            transcript
                .observations
                .replace(coder_sdk::observation::Claude::new(host.observations()));
        }
        let started = transcript.observations.borrow_mut().next(
            None,
            coder_sdk::observation::Body::Started {
                provider: "claude".into(),
                model: route.model.clone(),
            },
        );
        transcript.observe(vec![started]);
        let resumed = resume.is_some();
        let base = super::recipe::agent_prompt(recipe.as_deref(), host, resumed);
        let prompt = match answered {
            Some((pending, verdict)) => {
                let note = answer_note(verdict, &pending.step, host.prompt());
                if resumed {
                    note
                } else {
                    format!("{base}\n\n{note}")
                }
            }
            None => base,
        };
        let sequence = match host.effect(
            EFFECT,
            json!({"program": program, "cwd": host.workspace(), "model": route.model,
                "effort": effort, "tools": TOOLS, "resume": resume,
                "prompt_chars": prompt.chars().count(),
                "prompt_sha256": nostr::contracts::digest_bytes(prompt.as_bytes()),
                "answered": answered.map(|(pending, verdict)| json!({"request": pending.request,
                    "verdict": verdict}))}),
        ) {
            Ok(sequence) => sequence,
            Err(error) => {
                ended.error = Some(error.to_string());
                return Turn::Ended(ended);
            }
        };
        let options = options(
            host,
            program.clone(),
            &route.model,
            &effort,
            resume.clone(),
            env.clone(),
        );
        let attempt = attempt(host, &gate, options, &prompt, &transcript).await;
        let finished = attempt.finished.as_ref();
        if let Some(usage) = finished.and_then(|finished| finished.usage.as_ref()) {
            let observation = transcript.observations.borrow_mut().usage(usage);
            transcript.observe(vec![observation]);
        }
        let ending = if attempt.cancelled {
            coder_sdk::observation::Ending::Interrupted
        } else if finished.is_some_and(|finished| finished.success) {
            coder_sdk::observation::Ending::Completed
        } else {
            coder_sdk::observation::Ending::Failed
        };
        let observation = transcript.observations.borrow_mut().next(
            None,
            coder_sdk::observation::Body::Ended {
                ending,
                message: attempt.error.clone(),
            },
        );
        transcript.observe(vec![observation]);
        let (uncached, cache_read, cache_write) =
            input_parts(finished.and_then(|finished| finished.usage.as_ref()));
        let observed = json!({"session": attempt.session, "cancelled": attempt.cancelled,
            "error": attempt.error, "success": finished.map(|finished| finished.success),
            "cost_usd": finished.and_then(|finished| finished.cost_usd),
            "num_turns": finished.and_then(|finished| finished.num_turns),
            "usage": finished.and_then(|finished| finished.usage.clone()),
            "input_tokens": uncached, "cache_read_input_tokens": cache_read,
            "cache_creation_input_tokens": cache_write,
            "errors": finished.map(|finished| &finished.errors),
            "asked": gate.asked().map(|pending| pending.request)});
        if let Err(error) = host.result(sequence, EFFECT, observed) {
            ended.error = Some(error.to_string());
        }
        // A session that can't resume, such as one whose files are gone,
        // starts afresh, once.
        let lost = resumed
            && tries == 1
            && !attempt.cancelled
            && gate.asked().is_none()
            && transcript.tool_calls.get() == 0
            && finished.is_none_or(|finished| !finished.success && finished.text.is_empty());
        if lost {
            let _ = host.append(&Step::said(
                Source::System,
                "Claude Code couldn't resume the earlier session, so a new one starts.",
            ));
            resume = None;
            continue;
        }
        ended.resumed = resumed;
        break attempt;
    };
    ended.tool_calls = transcript.tool_calls.get();
    ended.reply = transcript.reply.borrow().clone();
    ended.refused = transcript.refused.borrow().clone();
    ended.session = attempt.session.clone().or(earlier);
    if attempt.cancelled {
        ended.stop = Some(StopReason::Cancelled);
        return Turn::Ended(ended);
    }
    if let Some(finished) = &attempt.finished {
        ended.cost_usd = finished.cost_usd;
        let (uncached, cache_read, cache_write) = input_parts(finished.usage.as_ref());
        ended.input_tokens = uncached + cache_read + cache_write;
        ended.output_tokens = finished
            .usage
            .as_ref()
            .and_then(|usage| usage.get("output_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or_default();
        #[allow(clippy::cast_precision_loss)]
        {
            ended.stats.insert("input_uncached".into(), uncached as f64);
            ended
                .stats
                .insert("cache_read_input_tokens".into(), cache_read as f64);
            ended
                .stats
                .insert("cache_creation_input_tokens".into(), cache_write as f64);
            if let Some(turns) = finished.num_turns {
                ended.stats.insert("num_turns".into(), f64::from(turns));
            }
        }
    }
    let question = gate.state().question.clone();
    if let Some(form) = question {
        ended.reply = form
            .questions
            .iter()
            .map(|q| {
                let options = q
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{}. {}", i + 1, c.label))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{}\n\n{}", q.text, options)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        ended.asked = Some(interaction::Kind::Question);
        ended.stop = Some(StopReason::EndTurn);
        return Turn::Ended(ended);
    }
    let asked = gate.asked();
    let _ = host.append(
        &Step::said(Source::System, "The Claude Code session this turn ran in.").noting(
            SESSION_NOTE,
            json!({"session": ended.session, "model": route.model, "resumed": ended.resumed,
                "pending": asked}),
        ),
    );
    if let Some(pending) = asked {
        // The turn ends asking the person; the step names what to approve.
        let reply = approval_reply(&pending.step);
        let _ = host.append(&Step::said(Source::Agent, &reply).by(&route.model).noting(
            "claude_sdk_approval",
            json!({"request": pending.request, "step": pending.step,
                "risk": pending.step.valid().then(|| pending.step.risk())}),
        ));
        ended.reply = reply;
        ended.asked = Some(interaction::Kind::Approval);
        ended.stop = Some(StopReason::EndTurn);
        return Turn::Ended(ended);
    }
    match &attempt.finished {
        Some(finished) if finished.success => {
            if !finished.text.trim().is_empty() {
                ended.reply = bounded(&finished.text, SEGMENT);
            }
            ended.stop = Some(StopReason::EndTurn);
        }
        Some(finished) => {
            // A usage or rate limit before any work is a refusal for the
            // capacity book, and the run moves to its next route.
            if let Some(kind) = finished.limit()
                && ended.tool_calls == 0
            {
                let now = coder::task::autostart::unix_now();
                return Turn::Refused(Refusal::new(Provider::Claude, kind, now, None));
            }
            ended.error = Some(if finished.errors.is_empty() {
                format!(
                    "Claude Code ended the turn on an error{}",
                    if finished.text.trim().is_empty() {
                        String::new()
                    } else {
                        format!(": {}", bounded(&finished.text, 2_000))
                    }
                )
            } else {
                bounded(&finished.errors.join("; "), 2_000)
            });
        }
        None => {
            ended.error = Some(
                attempt
                    .error
                    .clone()
                    .unwrap_or_else(|| "Claude Code reported no result".into()),
            );
        }
    }
    Turn::Ended(ended)
}

#[cfg(test)]
mod unit {
    use super::*;

    /// Through the Coder SDK's permission channel the gate answers as it
    /// does directly: inside runs with its input, outside asks and ends the
    /// turn, and the denied request is refused (#11496).
    #[test]
    fn the_sdk_permission_channel_answers_as_the_gate_does() {
        let dir = tempfile::tempdir().unwrap();
        let outside = json!({"command": "rm -rf /tmp/x"});
        let denied = Request::of("Bash", &outside);
        let gate = Arc::new(Gate::new(dir.path(), None, Some(denied)));
        let handler = sdk_handler(&gate);
        let ask = |tool: &str, input: &Value| {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(handler.can_use_tool(tool, input, None, None, None, "t", None))
                .unwrap();
            serde_json::to_value(result).unwrap()
        };
        let inside = json!({"file_path": dir.path().join("a.rs").display().to_string()});
        assert_eq!(
            ask("Edit", &inside),
            serde_json::to_value(PermissionResult::allow(inside.clone())).unwrap()
        );
        assert_eq!(
            ask("Bash", &outside),
            serde_json::to_value(PermissionResult::deny(DENIED)).unwrap()
        );
        assert_eq!(
            ask("Bash", &json!({"command": "ls /"})),
            serde_json::to_value(PermissionResult::deny_and_interrupt(ASKED)).unwrap()
        );
        assert!(gate.asked().is_some());
    }

    /// A session's `get_usage` answer and its `rate_limit_event`s land in
    /// the usage book (#11292), and only the book is written: no login or
    /// credentials file is read or made.
    #[test]
    fn get_usage_and_rate_limit_events_record_the_plan_windows() {
        let store = tempfile::tempdir().unwrap();
        record_usage(
            store.path(),
            &json!({"usage": {"five_hour": {"utilization": 62.0, "resets_at": "2026-10-11T14:20:00Z"}, "seven_day": {"utilization": 31.0}}}),
            1_791_700_000,
        );
        record_window(
            store.path(),
            &json!({"status": "allowed", "rateLimitType": "seven_day_opus", "utilization": 0.5}),
            1_791_700_000,
        );
        record_usage(store.path(), &json!({"subtype": "error"}), 1_791_700_000);
        let book = std::fs::read(store.path().join(microcoder_loop::usage::FILE)).unwrap();
        let usage = coder_engine_status::PlanUsage::from_book(&book, 1_791_700_000).unwrap();
        assert_eq!(usage.five_hour.unwrap().used_percent, 62);
        assert_eq!(usage.seven_day.unwrap().used_percent, 31);
        assert_eq!(usage.seven_day_top_model.unwrap().used_percent, 50);
        let mut names: Vec<_> = std::fs::read_dir(store.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert!(
            names
                .iter()
                .all(|name| !name.contains("credentials") && !name.contains(".claude")),
            "{names:?}"
        );
    }

    #[test]
    fn a_file_tool_inside_the_worktree_is_allowed_and_bash_asks() {
        let dir = tempfile::tempdir().unwrap();
        let gate = Gate::new(dir.path(), None, None);
        let inside = dir.path().join("src/lib.rs").display().to_string();
        assert!(matches!(
            gate.decide("Edit", &json!({"file_path": inside})),
            PermissionResult::Allow { .. }
        ));
        assert!(matches!(
            gate.decide("Grep", &json!({"pattern": "fn main"})),
            PermissionResult::Allow { .. }
        ));
        assert!(matches!(
            gate.decide(
                "Write",
                &json!({"file_path": "../outside.txt", "content": "x"})
            ),
            PermissionResult::Deny {
                interrupt: Some(true),
                ..
            }
        ));
        let asked = gate.asked().expect("the write outside asks");
        assert_eq!(asked.step.tool, "Write");
        assert_eq!(asked.step.command, "../outside.txt");
        // While the turn ends, every other request is refused too.
        assert!(matches!(
            gate.decide("Bash", &json!({"command": "ls"})),
            PermissionResult::Deny { .. }
        ));
        let decisions: Vec<Decision> = gate.drain().into_iter().map(|(_, _, d)| d).collect();
        assert_eq!(
            decisions,
            vec![
                Decision::Inside,
                Decision::Inside,
                Decision::Asked,
                Decision::Waiting
            ]
        );
    }

    #[test]
    fn an_answer_allows_or_denies_the_exact_request_once() {
        let dir = tempfile::tempdir().unwrap();
        let input = json!({"command": "make check"});
        let request = Request::of("Bash", &input);
        let gate = Gate::new(dir.path(), Some(request.clone()), None);
        assert!(matches!(
            gate.decide("Bash", &input),
            PermissionResult::Allow { .. }
        ));
        // Once: the same request again asks.
        assert!(matches!(
            gate.decide("Bash", &input),
            PermissionResult::Deny {
                interrupt: Some(true),
                ..
            }
        ));
        let gate = Gate::new(dir.path(), None, Some(request));
        assert!(matches!(
            gate.decide("Bash", &input),
            PermissionResult::Deny {
                interrupt: None,
                ..
            }
        ));
        assert!(gate.asked().is_none());
        // Another command is another request.
        assert!(matches!(
            gate.decide("Bash", &json!({"command": "make check && rm -rf /"})),
            PermissionResult::Deny {
                interrupt: Some(true),
                ..
            }
        ));
    }

    #[test]
    fn the_panel_and_a_standing_rule_answer_as_verdicts() {
        assert_eq!(verdict("Approved."), Verdict::Approve);
        assert_eq!(verdict("Denied."), Verdict::Deny);
        assert_eq!(
            verdict(
                "Approved by a standing rule: builder may run Bash `make check` in /w without asking again"
            ),
            Verdict::Approve
        );
        assert_eq!(verdict("Only after the tests."), Verdict::Reply);
    }

    #[test]
    fn the_approval_names_its_step_in_a_fenced_block() {
        let step = step(
            "Bash",
            &json!({"command": "make check", "description": "Runs the checks"}),
            Path::new("/work/repo"),
        );
        let reply = approval_reply(&step);
        let named = interaction::Step::in_reply(&reply).expect("a step block");
        assert_eq!(named.tool, "Bash");
        assert_eq!(named.command, "make check");
        assert_eq!(named.cwd, "/work/repo");
        assert_eq!(named.reason, "Runs the checks");
        // A command too long for a step is text alone.
        let long = super::step(
            "Bash",
            &json!({"command": "x".repeat(STEP_COMMAND + 1)}),
            Path::new("/work/repo"),
        );
        assert!(interaction::Step::in_reply(&approval_reply(&long)).is_none());
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::tests::fixture_with;
    use super::super::{AgentEngine, Stage, run_stages};
    use super::*;
    use coder::task::adapter::{Access, Configuration};
    use coder::task::commands::{self, Kind as Asked, Outcome, Request as Answer, Sender, State};
    use coder::task::{self, studio};

    const MODEL: &str = "claude-opus-5-5";

    /// A stand-in for `claude` that speaks the SDK's stream-json and
    /// control protocol: it answers `initialize`, asks to run `make check`
    /// with `Bash` after the prompt, and then acts on the host's answer.
    /// It logs its arguments, the names of its environment variables, and
    /// every line it reads beside itself.
    const FAKE_CLAUDE: &str = r#"#!/bin/sh
dir=$(dirname "$0")
printf '%s\n' "$*" >> "$dir/arguments"
env | cut -d= -f1 >> "$dir/environment"
session='"session_id":"sdk-session-1"'
tool=$(cat "$dir/tool" 2>/dev/null || printf Bash)
input=$(cat "$dir/input" 2>/dev/null || printf '%s' '{"command":"make check","description":"Runs the checks"}')
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$dir/stdin"
  case "$line" in
    *'"subtype":"initialize"'*)
      id=$(printf '%s\n' "$line" | sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')
      printf '{"type":"control_response","response":{"subtype":"success","request_id":"%s","response":{}}}\n' "$id"
      ;;
    *'"type":"user"'*)
      printf '%s\n' '{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"I will run the checks."},{"type":"tool_use","id":"tu-1","name":"'"$tool"'","input":'"$input"'}]},"parent_tool_use_id":null,"uuid":"a-1",'"$session"'}'
      printf '%s\n' '{"type":"control_request","request_id":"cli-1","request":{"subtype":"can_use_tool","tool_name":"'"$tool"'","input":'"$input"',"tool_use_id":"tu-1"}}'
      ;;
    *'"behavior":"allow"'*)
      printf '%s\n' '{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu-1","content":"checks passed","is_error":false}]},"parent_tool_use_id":null,'"$session"'}'
      printf '%s\n' '{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"The checks pass."}]},"parent_tool_use_id":null,"uuid":"a-2",'"$session"'}'
      printf '%s\n' '{"type":"result","subtype":"success","duration_ms":12,"duration_api_ms":10,"is_error":false,"num_turns":2,"result":"The checks pass.","total_cost_usd":0.0125,"usage":{"input_tokens":100,"output_tokens":20,"cache_read_input_tokens":900,"cache_creation_input_tokens":50},"modelUsage":{},"permission_denials":[],"uuid":"r-2",'"$session"'}'
      exit 0
      ;;
    *'"behavior":"deny"'*'"interrupt":true'*)
      printf '%s\n' '{"type":"result","subtype":"error_during_execution","duration_ms":5,"duration_api_ms":4,"is_error":true,"num_turns":1,"total_cost_usd":0.004,"usage":{"input_tokens":80,"output_tokens":10},"modelUsage":{},"permission_denials":[{"tool_name":"Bash","tool_use_id":"tu-1","tool_input":{"command":"make check"}}],"errors":["interrupted"],"uuid":"r-1",'"$session"'}'
      exit 0
      ;;
    *'"behavior":"deny"'*)
      printf '%s\n' '{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu-1","content":"The person denied this step.","is_error":true}]},"parent_tool_use_id":null,'"$session"'}'
      printf '%s\n' '{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"I left the checks alone."}]},"parent_tool_use_id":null,"uuid":"a-3",'"$session"'}'
      printf '%s\n' '{"type":"result","subtype":"success","duration_ms":12,"duration_api_ms":10,"is_error":false,"num_turns":2,"result":"I left the checks alone.","total_cost_usd":0.006,"usage":{"input_tokens":90,"output_tokens":12},"modelUsage":{},"permission_denials":[],"uuid":"r-3",'"$session"'}'
      exit 0
      ;;
  esac
done
"#;

    /// The stand-in in a directory of its own, which the test keeps.
    fn fake() -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude");
        std::fs::write(&path, FAKE_CLAUDE).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        (dir, path)
    }

    /// The lines the stand-in logged in `name`.
    fn logged(dir: &tempfile::TempDir, name: &str) -> Vec<String> {
        std::fs::read_to_string(dir.path().join(name))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// The host's reply to the stand-in's `can_use_tool` request.
    fn permission_reply(dir: &tempfile::TempDir) -> Value {
        logged(dir, "stdin")
            .iter()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|line| {
                line["type"] == "control_response" && line["response"]["request_id"] == "cli-1"
            })
            .map(|line| line["response"]["response"].clone())
            .expect("a reply to the permission request")
    }

    fn sdk_route(configuration: &mut Configuration) {
        configuration.provider = "claude".into();
        configuration.model = MODEL.into();
        configuration.effort = None;
        configuration.generation_endpoint = CLAUDE_SDK_ENDPOINT.into();
        configuration.decision_endpoint = "https://decision.example.invalid".into();
        configuration.access = Access::Full;
    }

    fn route() -> GrantRoute {
        GrantRoute {
            provider: "claude".into(),
            model: MODEL.into(),
            effort: None,
            generation_endpoint: CLAUDE_SDK_ENDPOINT.into(),
        }
    }

    async fn run_turn(store: &Path, grant: &[u8], program: &Path) -> task::Task {
        let host = Host::admit(store, grant).await.unwrap();
        let stages: Vec<Stage<codex_transport::codex::CodexTransport>> = vec![Stage::Agent(
            AgentEngine::ClaudeSdk,
            route(),
            program.to_path_buf(),
        )];
        run_stages(
            host,
            store.to_path_buf(),
            stages,
            Err("no decision service in this test".into()),
            "fixture-session",
            &[],
        )
        .await
        .unwrap()
    }

    /// The last agent message in the turn's trace.
    fn last_reply(store: &Path, turn: u64) -> String {
        std::fs::read_to_string(store.join(format!("fixture.{turn}.atif.jsonl")))
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|line| {
                line["record"] == "step"
                    && matches!(line["step"]["source"].as_str(), Some("Agent" | "agent"))
            })
            .filter_map(|line| line["step"]["message"].as_str().map(str::to_owned))
            .filter(|message| !message.is_empty())
            .last()
            .unwrap_or_default()
    }

    /// The person answers the waiting approval with `text`, as the
    /// decision panel sends it; the task is queued for its next turn.
    fn answer(store: &Path, waiting: &task::Task, id: &str, text: &str) -> task::Task {
        let now = coder::task::autostart::unix_now();
        let always = |_: &Sender| true;
        let sender = Sender {
            device: "phone".into(),
            grant: None,
            epoch: None,
        };
        let request = Answer {
            command: id.repeat(64),
            task: "fixture".into(),
            kind: Asked::Answer,
            based_on: waiting.revision,
            text: text.into(),
            emulate: false,
            issued_at: now,
        };
        let (recorded, _) =
            commands::record(store, &sender, &request, &crate::STEERING, &always, now).unwrap();
        assert!(matches!(
            recorded.state,
            State::Done(Outcome::Applied { .. })
        ));
        recorded.task.unwrap()
    }

    fn next(grant: &[u8], revision: u64) -> Vec<u8> {
        let mut next: task::owner::Grant = serde_json::from_slice(grant).unwrap();
        next.expected_revision = revision;
        serde_json::to_vec(&next).unwrap()
    }

    /// The first turn: Claude Code asks to run `make check`, and the turn
    /// ends waiting on a studio approval that names the step.
    async fn asked(store: &Path, grant: &[u8]) -> task::Task {
        let (dir, program) = fake();
        let waiting = run_turn(store, grant, &program).await;
        let result = waiting.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, interaction::APPROVAL_ENDING);
        assert_eq!(
            interaction::pending(&waiting),
            Some(interaction::Kind::Approval)
        );
        // The studio's approval subject for this exact step.
        assert!(studio::approvals::Action::of(&waiting).is_some());
        let step = interaction::Step::in_reply(&last_reply(store, 1)).expect("the step");
        assert_eq!(step.tool, "Bash");
        assert_eq!(step.command, "make check");
        assert_eq!(step.reason, "Runs the checks");
        assert!(step.cwd.starts_with('/'));
        // The host stopped Claude Code to ask.
        let reply = permission_reply(&dir);
        assert_eq!(reply["behavior"], "deny");
        assert_eq!(reply["interrupt"], true);
        // The CLI runs on the login, with prompts sent to the host, no
        // settings files, and no API key in its environment.
        let arguments = logged(&dir, "arguments").join("\n");
        assert!(
            arguments.contains("--permission-prompt-tool=stdio"),
            "{arguments}"
        );
        assert!(arguments.contains("--setting-sources="), "{arguments}");
        assert!(
            arguments.contains(&format!("--model={MODEL}")),
            "{arguments}"
        );
        assert!(!arguments.contains("--resume"), "{arguments}");
        assert!(
            !logged(&dir, "environment")
                .iter()
                .any(|name| REMOVED_ENV.contains(&name.as_str()))
        );
        // The result message's cost is the turn's.
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("\"kind\":\"claude_sdk\""), "{trace}");
        assert!(trace.contains("\"cost_usd\":0.004"), "{trace}");
        waiting
    }

    #[tokio::test]
    async fn a_live_desktop_reply_completes_the_same_turn_and_follower() {
        let (_root, store, grant) = fixture_with(MODEL, sdk_route);
        let (dir, program) = fake();
        let local = coder::task::local::Local::here(store.clone());
        let viewer = async {
            for _ in 0..500 {
                if let Some(snapshot) = local.pending_view("fixture").unwrap()
                    && let Some(entry) = snapshot
                        .entries
                        .iter()
                        .find(|entry| entry.resolution.is_none())
                {
                    let request = &entry.request;
                    local
                        .respond(
                            "fixture",
                            &route_contract::pending::Reply {
                                request: request.id.clone(),
                                scope: request.scope.clone(),
                                input_digest: request.input_digest.clone(),
                                answer: route_contract::pending::Answer::AllowOnce,
                            },
                        )
                        .unwrap();
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            panic!("the desktop did not receive an approval");
        };
        let (task, ()) = tokio::join!(run_turn(&store, &grant, &program), viewer);
        assert_eq!(task.turn(), 1);
        assert_eq!(task.status, task::Status::Finished);
        assert_eq!(permission_reply(&dir)["behavior"], "allow");
        assert_eq!(last_reply(&store, 1), "The checks pass.");
        let mut follow = local.follow("fixture", None, None);
        let (lines, state) = follow.poll().unwrap();
        assert_eq!(state, coder::task::local::State::Ended);
        assert!(lines.iter().any(|line| matches!(
            &line.event,
            openagents_chat::coder_events::CoderEvent::Result(_)
        )));
        assert!(!lines.iter().any(|line| matches!(
            &line.event,
            openagents_chat::coder_events::CoderEvent::Approval(_)
        )));
        assert!(!local.pending_view("fixture").unwrap().unwrap().active);
    }

    #[tokio::test]
    async fn observations_cannot_claim_another_owner_task_turn_or_generation() {
        use coder_sdk::observation::Body;
        let (_root, store, grant) = fixture_with(MODEL, sdk_route);
        let host = Host::admit(&store, &grant).await.unwrap();
        let mut recorder = host.observations();
        let observation = recorder.next(
            None,
            Body::Started {
                provider: "claude".into(),
                model: MODEL.into(),
            },
        );
        host.observe(&observation).unwrap();
        for field in ["host", "task", "turn", "generation"] {
            let mut wrong = observation.clone();
            match field {
                "host" => wrong.scope.host = "another-host".into(),
                "task" => wrong.scope.task = "another-task".into(),
                "turn" => wrong.scope.turn += 1,
                _ => wrong.scope.generation += 1,
            }
            assert!(host.observe(&wrong).is_err(), "{field}");
        }
    }

    #[tokio::test]
    async fn a_bash_request_becomes_an_approval_and_allow_once_continues_the_turn() {
        let (_root, store, grant) = fixture_with(MODEL, sdk_route);
        let waiting = asked(&store, &grant).await;
        let queued = answer(&store, &waiting, "a", "Approved.");
        let (dir, program) = fake();
        let done = run_turn(&store, &next(&grant, queued.revision), &program).await;
        let result = done.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, "model_finished");
        assert_eq!(interaction::pending(&done), None);
        // The session resumed, and the exact request ran, once.
        let arguments = logged(&dir, "arguments").join("\n");
        assert!(arguments.contains("--resume=sdk-session-1"), "{arguments}");
        assert_eq!(permission_reply(&dir)["behavior"], "allow");
        let prompt = logged(&dir, "stdin")
            .iter()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|line| line["type"] == "user")
            .expect("the prompt");
        assert!(
            prompt["message"]["content"]
                .as_str()
                .unwrap()
                .contains("allowed this step once")
        );
        // The actual SDK adapter records full observations into the task's
        // durable trace. The normal follower and a new follower replay them
        // with identical identities and delivery sequences.
        let local = coder::task::local::Local::new(store.clone());
        let mut follow = local.follow(&done.task_id, None, None);
        let (lines, state) = follow.poll().unwrap();
        assert_eq!(state, coder::task::local::State::Ended);
        let observations: Vec<_> = lines
            .iter()
            .filter_map(|line| match &line.event {
                openagents_chat::coder_events::CoderEvent::Observation { observation } => {
                    Some(observation)
                }
                _ => None,
            })
            .collect();
        assert!(
            observations
                .iter()
                .any(|observation| observation.native_session.as_deref() == Some("sdk-session-1"))
        );
        assert!(observations.iter().any(|observation| matches!(
            observation.body,
            coder_sdk::observation::Body::ToolResult { .. }
        )));
        assert!(observations.iter().any(|observation| matches!(
            observation.body,
            coder_sdk::observation::Body::Usage { .. }
        )));
        let mut again = local.follow(&done.task_id, None, None);
        let (replayed, state) = again.poll().unwrap();
        assert_eq!(state, coder::task::local::State::Ended);
        assert_eq!(
            serde_json::to_value(lines).unwrap(),
            serde_json::to_value(replayed).unwrap()
        );
        assert!(follow.poll().unwrap().0.is_empty());
    }

    /// #11527: a file read inside the worktree runs without asking, but a
    /// secret file there is a floor (`coder_sdk::floors`): the seat's
    /// registry holds it, and the turn ends asking the person.
    #[tokio::test]
    async fn a_floored_read_inside_the_worktree_ends_the_turn_asking() {
        let (_root, store, grant) = fixture_with(MODEL, sdk_route);
        let (dir, program) = fake();
        std::fs::write(dir.path().join("tool"), "Read").unwrap();
        std::fs::write(dir.path().join("input"), r#"{"file_path":".env"}"#).unwrap();
        let waiting = run_turn(&store, &grant, &program).await;
        let result = waiting.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, interaction::APPROVAL_ENDING);
        let step = interaction::Step::in_reply(&last_reply(&store, 1)).expect("the step");
        assert_eq!(step.tool, "Read");
        assert_eq!(step.command, ".env");
        let reply = permission_reply(&dir);
        assert_eq!(reply["behavior"], "deny");
        assert_eq!(reply["interrupt"], true);
        // The same tool on a plain file inside runs without asking.
        let gate = Gate::new(dir.path(), None, None);
        assert!(gate.inside("Read", &json!({"file_path": "src/lib.rs"})));
        assert!(!gate.inside("Read", &json!({"file_path": ".env"})));
        assert!(!gate.inside("Glob", &json!({"pattern": "/etc/*"})));
    }

    #[tokio::test]
    async fn deny_refuses_that_step_and_the_turn_goes_on_without_it() {
        let (_root, store, grant) = fixture_with(MODEL, sdk_route);
        let waiting = asked(&store, &grant).await;
        let queued = answer(&store, &waiting, "b", "Denied.");
        let (dir, program) = fake();
        let done = run_turn(&store, &next(&grant, queued.revision), &program).await;
        let result = done.run.as_ref().unwrap().result.as_ref().unwrap();
        // The denied step was refused, not asked about again.
        assert_eq!(result.ending, "model_finished");
        assert_eq!(interaction::pending(&done), None);
        let reply = permission_reply(&dir);
        assert_eq!(reply["behavior"], "deny");
        assert!(reply.get("interrupt").is_none(), "{reply}");
        assert!(
            !logged(&dir, "stdin")
                .iter()
                .any(|line| line.contains("\"behavior\":\"allow\""))
        );
    }
}
