//! Adapters for the host-owned CLI, Microcoder, and configured ACP agents.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use acp_client::{Handler, Opening, Session, Update};
use microcoder_loop::{
    env::Env,
    models::{Generate, JevJudge, Judge, Judgment, OpenRouterGenerator, QuestionSet},
    run::{Event, Limits, Models, Observer},
    state::{CommandResult, State},
};
use model_access::ApiKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const TEXT_MAX: usize = 64 * 1024;
const ARGUMENT_MAX: usize = 128;
const ARGUMENT_BYTES: usize = 64 * 1024;
const POLL: Duration = Duration::from_millis(50);

/// Codex starts with full access. Explicitly gated chats keep Codex
/// read-only without requesting write approval.
static CODEX_WRITES: AtomicBool = AtomicBool::new(true);

/// Lets Codex delegations in this process edit their working directory, or
/// keeps them read-only. A chat sets it for its run.
pub fn allow_codex_writes(on: bool) {
    CODEX_WRITES.store(on, Ordering::SeqCst);
}

/// The sandbox the native Codex bridge runs a delegation in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexSandbox {
    FullAccess,
    ReadOnly,
    WorkspaceWrite,
}

impl CodexSandbox {
    fn word(self) -> &'static str {
        match self {
            Self::FullAccess => "danger-full-access",
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
        }
    }
}

/// Full access by default; explicitly gated chats keep Codex read-only.
fn codex_sandbox() -> Result<CodexSandbox, String> {
    if !crate::approval::tools_allowed() {
        return Err("The crew charter refuses Codex delegation, including read-only work.".into());
    }
    Ok(sandbox_for(
        CODEX_WRITES.load(Ordering::SeqCst),
        crate::approval::gated(),
    ))
}

/// Full access requires write permission and an ungated chat.
fn sandbox_for(writes: bool, gated: bool) -> CodexSandbox {
    if writes && !gated {
        CodexSandbox::FullAccess
    } else {
        CodexSandbox::ReadOnly
    }
}

/// The protocol used by a registered local agent.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentTransport {
    #[default]
    Acp,
    /// The built-in bridge to Codex's native JSON event stream.
    CodexCli,
    /// The built-in bridge to the unmodified Claude Code binary in print
    /// mode, `claude -p` (BYO-03, [`claude_print`]).
    ClaudeCli,
}

#[path = "claude_print.rs"]
pub mod claude_print;

/// A detected or configured local agent executable, addressed by its stable ID.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AcpAgent {
    pub id: String,
    pub name: String,
    pub program: PathBuf,
    #[serde(default)]
    pub transport: AgentTransport,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default = "enabled")]
    pub enabled: bool,
}

fn enabled() -> bool {
    true
}

impl AcpAgent {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(
                "An ACP agent ID must use 1 to 64 letters, digits, dots, underscores, or hyphens."
                    .into(),
            );
        }
        if self.name.trim().is_empty()
            || self.name.len() > 128
            || self.name.chars().any(char::is_control)
        {
            return Err(
                "An ACP agent needs a name of up to 128 bytes without control characters.".into(),
            );
        }
        if self.program.as_os_str().is_empty() || self.program.to_string_lossy().contains('\0') {
            return Err("An ACP agent needs an executable path or program name.".into());
        }
        validate_arguments(&self.arguments)?;
        if self.transport == AgentTransport::CodexCli
            && (self.id != "codex" || !self.arguments.is_empty() || self.mode.is_some())
        {
            return Err("The built-in Codex bridge requires the codex agent ID and does not accept ACP arguments or modes.".into());
        }
        if self.transport == AgentTransport::ClaudeCli
            && (!matches!(self.id.as_str(), "claude" | "claude-code")
                || !self.arguments.is_empty()
                || self.mode.is_some())
        {
            return Err("The built-in Claude Code bridge requires the claude or claude-code agent ID and does not accept ACP arguments or modes.".into());
        }
        if self.transport == AgentTransport::CodexCli
            && cfg!(windows)
            && self.program.extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
            })
        {
            return Err(
                "The native Codex bridge requires codex.exe instead of a Windows batch shim."
                    .into(),
            );
        }
        if self.mode.as_ref().is_some_and(|mode| {
            mode.is_empty() || mode.len() > 128 || mode.chars().any(char::is_control)
        }) {
            return Err("An ACP mode must use 1 to 128 bytes without control characters.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum RuntimeEvent {
    Text(String),
    Model(String),
    Tokens(u64),
    Delegation {
        id: String,
        name: String,
        task: String,
        event: Box<RuntimeEvent>,
    },
    Tool {
        name: String,
        input: Value,
        output: Value,
        running: bool,
    },
    /// A Microcoder step Jev judged: the loop's step, from one, and Jev's
    /// estimate of how much of the task is complete (0 to 1), when Jev
    /// answered. There is never a step or time budget, and no other source
    /// fills `complete`.
    Progress {
        step: usize,
        complete: Option<f64>,
    },
    /// One model call's usage, as the provider reported it, sent as soon as
    /// the call's stream ends.
    Usage(Box<RoundUsage>),
    /// The harness a chat's tool turns run under, sent when it is decided
    /// (#11289).
    Harness(Box<crate::harness::Run>),
    /// Where the Claude plan's windows stand, each time Claude Code reports
    /// one (#11292): run cards show it.
    Plan(coder_engine_status::PlanUsage),
    /// The model's thinking started (`true`) or stopped streaming in: the
    /// input's rail says `Thinking` while it streams.
    Thinking(bool),
    /// A piece of the model's thinking, as it streams: it joins the running
    /// thinking entry, which the finished block's own entry
    /// ([`crate::claude_stream::THINKING`], `running: false`) then closes.
    /// See [`crate::claude_stream::Thought`].
    ThinkingText(String),
    /// One model attempt, when it starts and when it ends (#11580): a
    /// failed or still-open attempt goes into the export, so a turn that
    /// stopped answering can be diagnosed.
    Attempt(Box<ModelAttempt>),
    /// How the Coder loop sent a finished tool call to the model (#11654):
    /// the call as it went out (its id and any thought signature), the
    /// round's text and call count on a round's first call, and the
    /// observation's text when it is not the output itself. Sent just
    /// before the call's finished [`RuntimeEvent::Tool`], so the next turn
    /// sends the same history and the prompt cache holds.
    ToolWire(Box<Value>),
}

/// How one model attempt ended, or that it is still open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptOutcome {
    InFlight,
    Answered,
    Failed,
}

/// One request to a model inside a turn (#11580).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelAttempt {
    pub model: String,
    /// When the request went out, in milliseconds since the epoch; it
    /// names the attempt.
    pub started_ms: u64,
    pub ended_ms: Option<u64>,
    pub outcome: AttemptOutcome,
    /// The HTTP status of a refused attempt.
    pub status: Option<u16>,
    /// The `Retry-After` seconds the provider asked for.
    pub retry_after: Option<u64>,
    /// What went wrong, in fixed words.
    pub reason: Option<String>,
}

impl ModelAttempt {
    /// An attempt that just went out.
    #[must_use]
    pub fn started(model: &str, started_ms: u64) -> Self {
        Self {
            model: model.to_owned(),
            started_ms,
            ended_ms: None,
            outcome: AttemptOutcome::InFlight,
            status: None,
            retry_after: None,
            reason: None,
        }
    }

    /// The export's record of it, times as ISO 8601.
    #[must_use]
    pub fn record(&self) -> Value {
        let mut record = json!({
            "model": self.model,
            "started_at": atif::iso(self.started_ms),
            "outcome": self.outcome,
        });
        if let Some(ended) = self.ended_ms {
            record["ended_at"] = json!(atif::iso(ended));
            record["duration_ms"] = json!(ended.saturating_sub(self.started_ms));
        }
        if let Some(status) = self.status {
            record["status"] = json!(status);
        }
        if let Some(seconds) = self.retry_after {
            record["retry_after_seconds"] = json!(seconds);
        }
        if let Some(reason) = &self.reason {
            record["reason"] = json!(reason);
        }
        record
    }
}

/// What one model call used and how long it took.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoundUsage {
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: Option<u64>,
    /// Dollars, when the provider priced the call.
    pub cost: Option<f64>,
    pub milliseconds: u64,
    pub first_token_ms: Option<u64>,
    /// Prompt tokens read from the provider's prompt cache (#11550).
    pub cached_tokens: Option<u64>,
    /// Prompt tokens written to the provider's prompt cache (#11550).
    pub cache_write_tokens: Option<u64>,
    /// The model the gateway tried first, when `model` answered instead
    /// (#11654).
    pub fell_back: Option<openrouter::FellBack>,
}

impl RoundUsage {
    /// The one grey line a gateway fallback shows (#11654), as "Flash
    /// answered: Argon was slow to start".
    #[must_use]
    pub fn fallback_line(&self) -> Option<String> {
        let fell = self.fell_back.as_ref()?;
        let why = match fell.reason.as_str() {
            "first_token_deadline" | "timeout" => "was slow to start",
            "rate_limited" => "was busy",
            "server" | "empty_stream" | "stream_failed" => "had a server error",
            "network" => "couldn't be reached",
            _ => "couldn't answer",
        };
        Some(format!(
            "{} answered: {} {why}",
            short_model_name(&self.model),
            short_model_name(&fell.model)
        ))
    }
}

/// A model's short name for a notice: `google/gemini-4-argon` is "Argon",
/// `google/gemini-3.8-flash` "Flash".
#[must_use]
pub fn short_model_name(model: &str) -> String {
    let base = model.split(':').next().unwrap_or(model);
    let bare = base.rsplit('/').next().unwrap_or(base);
    let word = bare
        .rsplit('-')
        .find(|word| word.chars().next().is_some_and(char::is_alphabetic))
        .unwrap_or(bare);
    let mut letters = word.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

impl RuntimeEvent {
    /// "step 3 · ≈40% done", or "step 3" while Jev has given no estimate.
    #[must_use]
    pub fn progress_line(step: usize, complete: Option<f64>) -> String {
        match complete.filter(|complete| complete.is_finite()) {
            Some(complete) => format!(
                "step {step} · ≈{:.0}% done",
                complete.clamp(0.0, 1.0) * 100.0
            ),
            None => format!("step {step}"),
        }
    }

    pub(crate) fn redact(&mut self, keys: &[ApiKey]) {
        match self {
            Self::Tokens(_)
            | Self::Progress { .. }
            | Self::Usage(_)
            | Self::Harness(_)
            | Self::Plan(_)
            | Self::Attempt(_)
            | Self::ToolWire(_)
            | Self::Thinking(_) => {}
            Self::Text(text) | Self::Model(text) | Self::ThinkingText(text) => {
                *text = redact_text(text, keys);
            }
            Self::Delegation {
                id,
                name,
                task,
                event,
            } => {
                *id = redact_text(id, keys);
                *name = redact_text(name, keys);
                *task = redact_text(task, keys);
                event.redact(keys);
            }
            Self::Tool {
                name,
                input,
                output,
                ..
            } => {
                *name = redact_text(name, keys);
                for key in keys {
                    crate::plugin_tools::redact_value(input, key.expose());
                    crate::plugin_tools::redact_value(output, key.expose());
                }
            }
        }
    }
}

pub fn cli_tool_definition() -> Value {
    json!({"type":"function","function":{
        "name":"openagents_cli",
        "description":"Run the bundled OpenAgents CLI in the current working directory. Pass an argument array, never a shell command. Use [\"--help\"] to discover command groups, then [\"GROUP\",\"--help\"] for syntax. The CLI supports computers, tasks, issues, settings, knowledge, relays, plugins, worlds, and wallets; commands retain their own access checks. The person's projects (their projects, or a project they name, such as finances) are their openagents.com projects, the Projects list on the website: read them with [\"projects\",\"list\"] and [\"projects\",\"show\",NAME] for one project and its chats (by name, repository, or id); every chat's messages with [\"projects\",\"read\",NAME], or one chat's with [\"chat\",\"read\",\"--thread\",ID] (website and phone chats included; [\"chat\",\"threads\",\"--account\"] lists them all); its files and notes with [\"projects\",\"files\",\"list\",NAME]. A project needs only a name: make one with [\"projects\",\"create\",NAME] (add [\"--repo\",\"OWNER/NAME\"] only when the person names one). A project can have no repository, one, or several with one primary ([\"projects\",\"repo\",\"add\"|\"remove\"|\"primary\",PROJECT,REPO]); one repository can be in several projects and names can repeat. When a name matches more than one project the command lists them: ask the person which one, then use its id. Projects other people shared with the person list too, each with their role (owner, editor, or viewer) and marked shared. In a shared project, [\"projects\",\"show\",NAME] lists every member's chats, each with started_by (name, and you when the person started it): say who started a chat when it wasn't the person. A viewer reads a shared project and sends nothing: don't start or reply to chats there for a viewer. When the website refuses to start a chat or send a message, tell the person its reason as it gave it. Join a shared project from an invitation link (openagents.com/projects/join/...) with [\"projects\",\"join\",LINK], only when the person asks. Start a new chat inside a project (\"make a new chat in my finances project\") with [\"chat\",\"send\",MESSAGE,\"--project\",NAME]: OpenAgents answers it on openagents.com and it shows in that project. Files of an only_me project are locked to the person's devices: answer from them with [\"vault\",\"ask\",FILE,\"--project\",ID,\"--question\",TEXT] and never send their contents elsewhere. GitHub Project boards are a different thing, under [\"board\",...]. JSON output is added by the host. Call only for work the user requested; do not send messages, publish, pay, or delete data without their authorization.",
        "parameters":{"type":"object","properties":{"arguments":{"type":"array","items":{"type":"string"},"maxItems":128}},"required":["arguments"],"additionalProperties":false}
    }})
}

/// The `run` tool of an agent-driven chat: one shell command in the
/// working directory, under the approval gate.
pub fn run_tool_definition() -> Value {
    json!({"type":"function","function":{
        "name":"Run",
        "description":"Run one shell command in the working directory and return its exit status and output. Commands have full filesystem and network access by default. Follow the user's instructions. An explicitly gated host can require CONFIRM or REJECT for changes. Never start an interactive program or a pager. To search file contents, use the Grep tool, or rg in a shell: both skip build output and other files .gitignore ignores. A recursive grep over the whole checkout (grep -r PATTERN .) is refused; to grep one folder, name it.",
        "parameters":{"type":"object","properties":{"command":{"type":"string","minLength":1,"maxLength":8192}},"required":["command"],"additionalProperties":false}
    }})
}

/// Runs `script` with full access unless the host explicitly installs a gate.
///
/// # Errors
/// The working directory is unavailable or the boundary cannot be built.
pub async fn run_command(
    script: &str,
    cwd: &Path,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let directory = cwd
        .canonicalize()
        .map_err(|_| "The working directory is unavailable.".to_string())?;
    let boundary = command_boundary(&directory, false)?;
    let sink = RefCell::new(emit);
    let checkout = Checkout {
        asked: std::sync::Mutex::default(),
        hooks: Some(crate::sdk_hooks::coder_loop(&directory, cancel)),
        directory,
        boundary,
        cancel: Arc::clone(cancel),
        redaction_keys,
        events: Some(&sink),
    };
    // The same hooks as a loop command: the floors hold a production
    // deploy, a merge, a secret or a payment for the person (#11475, #11497).
    let hooked =
        microcoder_loop::hooks::run(&checkout, script, Duration::from_secs(120), true).await;
    let ran = hooked.result;
    let mut output = json!({"exit":ran.exit,"output":ran.output,"timed_out":ran.timed_out,"seconds":ran.seconds});
    // What the hooks added (a folder's instructions, #11520) reaches the
    // model with the result, as Claude Code's additionalContext does.
    if !hooked.context.is_empty() {
        output["context"] = json!(hooked.context.join("\n\n"));
    }
    Ok(output)
}

/// `word` as a shell word, quoted only when it needs quoting.
pub(crate) fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '-' | '_' | '.' | '/' | ':' | '=' | ',' | '@' | '+')
        });
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

pub fn acp_tool_definition(agents: &[AcpAgent]) -> Option<Value> {
    let ids: Vec<&str> = agents
        .iter()
        .filter(|a| a.enabled && a.validate().is_ok())
        .map(|a| a.id.as_str())
        .collect();
    if ids.is_empty() {
        return None;
    }
    Some(json!({"type":"function","function":{
        "name":"acp_subagent",
        "description":"Delegate a task to a registered local agent through ACP or the built-in native Codex bridge. Use exactly the agent the user names; never substitute another agent. If that agent is unavailable, report the reason and let the user choose. The host supplies the executable. Include the task, relevant context, and the result you need. This starts one child session in the current working directory, streams its work, and closes it when the task ends; the result names the folder under cwd. Agents start with full permissions by default: native Codex has no sandbox and Coder answers its approval requests, and ACP permission requests are approved. Explicit host approval policies still apply.",
        "parameters":{"type":"object","properties":{"agent":{"type":"string","enum":ids},"task":{"type":"string","minLength":1,"maxLength":65536},"model":{"type":"string","description":"The model the agent runs; only devin-cli takes one."},"read_only":{"type":"boolean","description":"Set for an audit, review or check that must not change files. Codex, Claude Code and Cursor are held to it; another agent is switched to its own read-only mode when it has one. The result's permissions field says the mode it ran in and whether it was enforced."}},"required":["agent","task"],"additionalProperties":false}
    }}))
}

pub fn microcoder_tool_definition() -> Value {
    json!({"type":"function","function":{
        "name":"microcoder",
        "description":"Hand a concrete coding task to the bundled Microcoder loop. The loop uses structured next actions, Jev judgments when configured, and commands bounded to writes in the current checkout. State the desired result and relevant constraints. It runs on the OpenRouter plugin's saved model when an OpenRouter key is saved in /plugins, otherwise on this computer's Codex login, then its Claude Code login, then the OpenAgents gateway, falling over to the next one mid-task. The delegation runs until completion or cancellation and returns the reply, the provider that ran it, the actual model, token usage, the cost basis (billed or list_price), the ending, the folder its commands ran in (cwd), and its permissions.",
        "parameters":{"type":"object","properties":{"task":{"type":"string","minLength":1,"maxLength":65536},"read_only":{"type":"boolean","description":"Set for an audit, review or check that must not change files: commands then run where they cannot write."}},"required":["task"],"additionalProperties":false}
    }})
}

fn validate_arguments(arguments: &[String]) -> Result<(), String> {
    if arguments.len() > ARGUMENT_MAX
        || arguments.iter().map(String::len).sum::<usize>() > ARGUMENT_BYTES
        || arguments.iter().any(|a| a.contains('\0'))
    {
        Err("A command accepts up to 128 arguments and 64 KiB without NUL bytes.".into())
    } else {
        Ok(())
    }
}

/// Find the release companion first, then a CLI explicitly installed on PATH.
pub fn cli_binary() -> Option<PathBuf> {
    let siblings = std::env::current_exe()
        .ok()
        .map(|exe| cli_companions(&exe))
        .unwrap_or_default();
    acp_client::process::first_executable(siblings.into_iter().chain(acp_client::process::on_path(
        "openagents",
        std::env::var_os("PATH").as_deref(),
    )))
}

fn cli_companions(executable: &Path) -> Vec<PathBuf> {
    let Some(parent) = executable.parent() else {
        return vec![];
    };
    let mut companions = vec![];
    if let Some(suffix) = executable
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| {
            name.strip_prefix("coder-new-openagents-")
                .or_else(|| name.strip_prefix("coder-openagents-"))
        })
    {
        companions.push(parent.join(format!("openagents-openagents-{suffix}")));
    }
    companions.push(parent.join(if cfg!(windows) {
        "openagents.exe"
    } else {
        "openagents"
    }));
    companions
}

pub async fn cli(
    arguments: &[String],
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    // A command this build's instructions know and the CLI here lacks is
    // answered with an update, not "unknown command" (#11595).
    crate::cli_capabilities::run(arguments, cwd, cancel, emit).await
}

pub(crate) async fn cli_at(
    program: &Path,
    arguments: &[String],
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    validate_arguments(arguments)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("The CLI call was canceled before it started.".into());
    }
    let shown = std::iter::once("openagents".to_owned())
        .chain(arguments.iter().map(|argument| shell_word(argument)))
        .collect::<Vec<_>>()
        .join(" ");
    crate::floors::hold(
        "Coder",
        "openagents_cli",
        &json!({ "arguments": arguments }),
        cwd,
        cancel,
    )
    .await?;
    if let crate::approval::Verdict::Refused(why) = crate::approval::check(&shown) {
        return Err(why);
    }
    cli_checked(program, arguments, cwd, cancel, emit).await
}

/// Run the CLI after the caller applied its own approval policy, as the
/// `computer` tool does for each action.
pub(crate) async fn cli_checked(
    program: &Path,
    arguments: &[String],
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    cli_checked_within(program, arguments, cwd, cancel, emit, None).await
}

/// [`cli_checked`] with its own deadline, for a command that runs longer
/// than a lookup: a deploy builds images and smoke-tests them (#11170).
pub(crate) async fn cli_checked_within(
    program: &Path,
    arguments: &[String],
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
    deadline_seconds: Option<u64>,
) -> Result<Value, String> {
    validate_arguments(arguments)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("The CLI call was canceled before it started.".into());
    }
    let mut command = acp_client::process::command(program);
    command.arg("--json").args(arguments).current_dir(cwd);
    prepare_cli_child(&mut command);
    let sink = RefCell::new(emit);
    let mut bridge = crate::delegation_events::Bridge::new()?;
    bridge.prepare(&mut command);
    let cloud_job = arguments.first().is_some_and(|word| word == "coder")
        && (arguments.iter().any(|word| word == "--on")
            || arguments
                .windows(2)
                .any(|words| words == ["remote", "follow"]));
    let seconds = deadline_seconds.unwrap_or(if cloud_job { 12 * 3600 + 1200 } else { 300 });
    let job = supervise::Job::from_command(command)
        .bounded(supervise::Limits::within(Duration::from_secs(seconds)).keeping(TEXT_MAX));
    let stopped = wait_job(job, cancel, Some((&mut bridge, &sink)), &[], None).await?;
    Ok(
        json!({"exit":stopped.ending.code(),"stdout":String::from_utf8_lossy(&stopped.rest.bytes),"stderr":stopped.stderr.marked(),"timed_out":matches!(stopped.ending,supervise::Ending::TimedOut),"canceled":stopped.requested,"group_clear":stopped.group_clear,"truncated":!stopped.rest.gaps.is_empty()}),
    )
}

async fn wait_job(
    job: supervise::Job,
    cancel: &Arc<AtomicBool>,
    mut observing: Option<(
        &mut crate::delegation_events::Bridge,
        &dyn crate::delegation_events::Sink,
    )>,
    keys: &[ApiKey],
    progress: Option<(&dyn crate::delegation_events::Sink, &str)>,
) -> Result<supervise::Stopped, String> {
    let live = job.start(supervise::Input::Null)?;
    let mut captured = Vec::new();
    let started = Instant::now();
    let mut last_output = started;
    let mut last_tick = None;
    while !live.finished() {
        if let Some((sink, script)) = progress {
            let delivery = live.take();
            let changed = !delivery.is_empty();
            if changed {
                last_output = Instant::now();
            }
            let tick = started.elapsed().as_secs();
            if changed || last_tick != Some(tick) {
                last_tick = Some(tick);
                captured.extend_from_slice(
                    &delivery.bytes[..delivery
                        .bytes
                        .len()
                        .min(TEXT_MAX.saturating_sub(captured.len()))],
                );
                sink.emit(RuntimeEvent::Tool {
                    name: "Run".into(),
                    input: json!({"command":script}),
                    output: json!({"output":redact_stream(&captured, keys),"elapsed_seconds":tick,"silent_seconds":last_output.elapsed().as_secs(),"activity":script.lines().next().unwrap_or("Executing command").trim_start_matches("# ")}),
                    running: true,
                });
            }
        }
        if let Some((bridge, sink)) = &mut observing {
            bridge.drain(*sink, keys);
        }
        if cancel.load(Ordering::Relaxed) {
            let mut stopped = live.stop().await;
            captured.extend_from_slice(
                &stopped.rest.bytes[..stopped
                    .rest
                    .bytes
                    .len()
                    .min(TEXT_MAX.saturating_sub(captured.len()))],
            );
            stopped.rest.bytes = captured;
            if let Some((bridge, sink)) = observing {
                bridge.finish(sink, keys);
            }
            return Ok(stopped);
        }
        tokio::time::sleep(POLL).await;
    }
    let mut stopped = live.wait().await;
    captured.extend_from_slice(
        &stopped.rest.bytes[..stopped
            .rest
            .bytes
            .len()
            .min(TEXT_MAX.saturating_sub(captured.len()))],
    );
    stopped.rest.bytes = captured;
    if let Some((bridge, sink)) = observing {
        bridge.finish(sink, keys);
    }
    Ok(stopped)
}

pub(crate) fn scrub_credentials(command: &mut std::process::Command) {
    for (name, _) in std::env::vars_os() {
        if acp_client::process::is_credential_name(&name.to_string_lossy()) {
            command.env_remove(name);
        }
    }
    apply_child_env(command);
}

thread_local! {
    /// Variables every engine and command started on this thread gets: a
    /// background agent's thread sets its shared build folder here
    /// ([`crate::fleet`]). Empty everywhere else.
    static CHILD_ENV: RefCell<Vec<(std::ffi::OsString, std::ffi::OsString)>> =
        const { RefCell::new(Vec::new()) };
}

thread_local! {
    static CODEX_MODEL: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The model the next Codex run on this thread uses, ahead of
/// `CODER_CODEX_MODEL`: a delegation that named one (#11511).
pub(crate) fn set_next_codex_model(model: Option<String>) {
    CODEX_MODEL.with(|next| *next.borrow_mut() = model);
}

fn next_codex_model() -> Option<String> {
    CODEX_MODEL.with(|next| next.borrow().clone())
}

/// Codex on a delegated task (#11511): on app-server under Coder's hooks
/// with full access, or in Codex's read-only sandbox, on `model` when the
/// call named one. Codex reads its own `AGENTS.md`; the task is the
/// child's briefing.
pub(crate) async fn codex_delegated(
    program: &Path,
    task: &str,
    cwd: &Path,
    model: Option<&str>,
    read_only: bool,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let sandbox = if read_only {
        codex_sandbox().map(|_| CodexSandbox::ReadOnly)?
    } else {
        codex_sandbox()?
    };
    set_next_codex_model(model.map(str::to_owned));
    let result = codex_cli(program, task, cwd, sandbox, false, cancel, emit).await;
    set_next_codex_model(None);
    result
}

/// Sets the variables children started on this thread get.
pub(crate) fn set_child_env(vars: Vec<(std::ffi::OsString, std::ffi::OsString)>) {
    CHILD_ENV.with(|env| *env.borrow_mut() = vars);
}

/// The variables children started on this thread get: this thread's
/// ([`set_child_env`]), with the `openagents` beside Coder's own program
/// first on `PATH` when one sits there (#11538). Every process Coder
/// starts (Claude Code, Codex, OpenCode, the ACP agents, the Coder loop's
/// commands, delegations) takes its variables from here, so none of them
/// runs an older `openagents` installed elsewhere on `PATH`.
pub(crate) fn child_env() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    let vars = CHILD_ENV.with(|env| env.borrow().clone());
    with_own_cli(vars, coder_lease::shim::own_cli_dir().as_deref())
}

/// `vars` with `PATH` (theirs, else this process's) led by `cli`, the
/// directory of the `openagents` beside this program.
fn with_own_cli(
    mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    cli: Option<&Path>,
) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    let path = vars
        .iter()
        .rev()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var_os("PATH"));
    if let Some(fronted) = coder_lease::shim::child_path_with(
        path.as_deref(),
        cli,
        coder_lease::shim::enabled().as_deref(),
    ) {
        vars.retain(|(name, _)| name != "PATH");
        vars.push(("PATH".into(), fronted));
    }
    vars
}

pub(crate) fn apply_child_env(command: &mut std::process::Command) {
    for (name, value) in child_env() {
        command.env(name, value);
    }
}

fn prepare_cli_child(command: &mut std::process::Command) {
    command.env(crate::programmatic::MODEL_INPUT_ENV, "model");
    for name in ["OPENROUTER_API_KEY", "TYPESAFE_API_KEY"] {
        command.env_remove(name);
    }
}

/// Keep this future alive after cancellation until process-group cleanup returns.
///
/// `model` names the model the agent runs, and only `devin-cli` takes one;
/// any other agent refuses it.
pub async fn acp(
    agent: &AcpAgent,
    task: &str,
    cwd: &Path,
    model: Option<&str>,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    acp_with(agent, task, cwd, model, false, cancel, emit).await
}

/// The ids of agent modes that make no changes, in the order they are
/// tried for a read-only task (#11470).
const READ_ONLY_MODES: [&str; 4] = ["read-only", "readonly", "ask", "plan"];

/// What a delegation reports about its access (#11470): the mode it ran
/// in, whether that mode was enforced or only asked for in words, and how.
fn permissions(read_only: bool, enforced: bool, how: &str) -> Value {
    json!({
        "mode": if read_only { "read-only" } else { "full" },
        "enforced": enforced,
        "how": how,
    })
}

/// [`acp`] on a task that may be read-only: each engine is held to it the
/// way it can be, and the result says which way under `permissions`, with
/// the folder it ran in under `cwd` (#11469, #11470).
pub async fn acp_with(
    agent: &AcpAgent,
    task: &str,
    cwd: &Path,
    model: Option<&str>,
    read_only: bool,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    acp_briefed(agent, task, cwd, model, read_only, None, cancel, emit).await
}

/// [`acp_with`] carrying the chat's briefing (#11473): Codex reads it ahead
/// of the task, with its own `AGENTS.md` reading off
/// (`project_doc_max_bytes=0`), so it gets what every harness gets and
/// nothing twice. A briefing that would push the task past its limit is
/// left out, and Codex reads its own files as before.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub async fn acp_briefed(
    agent: &AcpAgent,
    task: &str,
    cwd: &Path,
    model: Option<&str>,
    read_only: bool,
    briefing: Option<&str>,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    if !crate::approval::tools_allowed() {
        return Err(
            "The crew charter refuses ACP and Codex delegation, including read-only work.".into(),
        );
    }
    if model.is_some() && !crate::agent_models::lists_models(&agent.id) {
        return Err(format!(
            "A model names only the devin-cli and grok-build agents, not {}.",
            agent.id
        ));
    }
    agent.validate()?;
    if !agent.enabled {
        return Err("This ACP agent is turned off.".into());
    }
    validate_task(task)?;
    // No time limit (owner rule): the task runs until it ends or is stopped.
    let canceled = || cancel.load(Ordering::Relaxed);
    if canceled() {
        return Err("The ACP task was canceled before it started.".into());
    }
    let program = if agent.program.is_absolute() || agent.program.components().count() > 1 {
        acp_client::process::first_executable([if agent.program.is_absolute() {
            agent.program.clone()
        } else {
            cwd.join(&agent.program)
        }])
    } else {
        acp_client::process::first_executable(acp_client::process::on_path(
            &agent.program.to_string_lossy(),
            std::env::var_os("PATH").as_deref(),
        ))
    }
    .ok_or_else(|| format!("The executable for {} is unavailable.", agent.name))?;
    let cwd_text = cwd.display().to_string();
    let with = |mut result: Value, access: Value| {
        result["cwd"] = json!(cwd_text);
        result["permissions"] = access;
        result
    };
    if agent.transport == AgentTransport::CodexCli {
        let sandbox = if read_only {
            codex_sandbox().map(|_| CodexSandbox::ReadOnly)?
        } else {
            codex_sandbox()?
        };
        let access = permissions(
            sandbox == CodexSandbox::ReadOnly,
            true,
            if sandbox == CodexSandbox::ReadOnly {
                "Codex's read-only sandbox"
            } else {
                "Codex runs with no sandbox"
            },
        );
        let briefed = briefing
            .filter(|briefing| !briefing.trim().is_empty())
            .map(|briefing| briefed_task(briefing, task))
            .filter(|briefed| briefed.len() <= TEXT_MAX);
        let task = briefed.as_deref().unwrap_or(task);
        return codex_cli(
            &program,
            task,
            cwd,
            sandbox,
            briefed.is_some(),
            cancel,
            emit,
        )
        .await
        .map(|result| with(result, access));
    }
    if agent.transport == AgentTransport::ClaudeCli {
        // An automated run on a Claude plan login takes a plan slot first
        // (#11237); a chat the person types into never waits. A turn on the
        // person's own key (#11295) isn't on the plan.
        let turn = claude_print::take_next_turn();
        let _slot = match turn.key {
            Some(_) => None,
            None => {
                Some(crate::claude_plan::admit(cancel, &|name| std::env::var(name).ok()).await?)
            }
        };
        let mut turn = turn;
        turn.read_only = read_only;
        let restricted = read_only || crate::approval::gated();
        let access = permissions(
            restricted,
            true,
            if restricted {
                "Claude Code may only read, search and list files"
            } else {
                "Claude Code runs with its permission asks skipped"
            },
        );
        return claude_print::run(&program, task, cwd, cancel, emit, turn)
            .await
            .map(|result| with(result, access));
    }
    let cursor = agent.id == "cursor";
    let admitted = std::env::var("OA_CODER_CLOUD_CREDENTIAL_NAMES").unwrap_or_default();
    // `vars_os`, not `vars`: a non-UTF-8 variable is left out rather than
    // crashing the run.
    let mut environment: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(name, _)| {
            admitted.split(',').any(|allowed| allowed == name)
                || (cursor && acp_client::cursor::CREDENTIAL_VARS.contains(&name.as_str()))
                || !(name.ends_with("_API_KEY")
                    || name.ends_with("_TOKEN")
                    || name.ends_with("_SECRET"))
        })
        .collect();
    for (name, value) in child_env() {
        let name = name.to_string_lossy().into_owned();
        environment.retain(|(existing, _)| existing != &name);
        environment.push((name, value.to_string_lossy().into_owned()));
    }
    // Devin's session mode and sandbox come from the chat's gate, the same
    // access a `devin:` route's grant names: an ungated chat is full access,
    // so Devin runs `bypass`; a gated chat is the boundary, so it runs
    // `accept-edits` under `devin --sandbox` and every ask is refused
    // (#10929). An explicit mode on the agent still wins.
    let mut arguments = match (agent.id.as_str(), model) {
        ("devin-cli", Some(model)) if model != acp_client::devin::DEFAULT_MODEL => {
            acp_client::devin::arguments(model)
        }
        // Devin's account default is never what Coder asks for: Coder's
        // fastest SWE-2 is (#11556, #11583). Devin's server starts on its
        // account default when it doesn't allow the model asked for; that
        // is refused below once the session reports its model.
        // The person's pick in /models comes first, so a delegation the
        // call names no model for runs it too.
        ("devin-cli", _) => acp_client::devin::arguments(
            &crate::agent_models::turn_model(crate::agent_models::DEVIN)
                .unwrap_or_else(|| acp_client::devin::CODER_MODEL.to_owned()),
        ),
        // Grok Build runs the pick, else the default `grok models` names,
        // read before the session starts so it never opens on Grok's
        // built-in fallback model while its login refreshes (#11556).
        (crate::agent_models::GROK, _) => {
            let mut arguments = agent.arguments.clone();
            let model = model
                .filter(|model| *model != acp_client::grok::DEFAULT_MODEL)
                .map(str::to_owned)
                .or_else(|| {
                    crate::agent_models::turn_model_reading(
                        crate::agent_models::GROK,
                        &agent.program,
                    )
                });
            if let Some(model) = model
                && !arguments.iter().any(|argument| argument == "--model")
            {
                let at = usize::from(arguments.first().is_some_and(|arg| arg == "agent"));
                arguments.splice(at..at, ["--model".to_owned(), model]);
            }
            arguments
        }
        _ => agent.arguments.clone(),
    };
    // Cursor's ask mode makes no edits and runs no commands.
    let ask = (read_only && cursor).then(|| acp_client::cursor::Mode::Ask.id().to_owned());
    let mode = ask.or_else(|| agent.mode.clone()).or_else(|| {
        if cursor {
            return Some(acp_client::cursor::Mode::Agent.id().to_owned());
        }
        (agent.id == "devin-cli").then(|| {
            if crate::approval::gated() {
                acp_client::devin::Permission::AcceptEdits
            } else {
                acp_client::devin::Permission::Bypass
            }
            .mode_id()
            .to_owned()
        })
    });
    if agent.id == "devin-cli"
        && crate::approval::gated()
        && !arguments.first().is_some_and(|arg| arg == "--sandbox")
    {
        arguments.insert(0, "--sandbox".into());
    }
    // Cursor's build is checked first: a build Cursor already refused stops
    // here, before the turn, with the command that updates it (#11463).
    let engine_path = crate::engine_update::path();
    let checked = crate::engine_update::before_turn(
        &agent.id,
        &program,
        &environment,
        cwd,
        engine_path.as_deref(),
    )
    .await?;
    if cursor {
        cursor_signed_in(&program, &environment, cwd).await?;
    }
    // Coder's hooks on the agent (#11508): the fence, the floors and the
    // rest answer its asks, and an agent that runs its own hooks runs
    // Coder's bridge before every tool. What it can't be stopped from is
    // said plainly.
    let (hooks, refused) = crate::sdk_hooks::acp(cwd, &agent.id, &agent.name, cancel);
    let checks = crate::sdk_hooks::unkept(&agent.name, &refused);
    let _gate = acp_gate(agent, &hooks, &mut arguments, &mut environment)?;
    // The model Devin was asked for, held against the one it reports.
    let devin_asked = (agent.id == crate::agent_models::DEVIN)
        .then(|| acp_client::devin::asked_model(&arguments))
        .flatten();
    // What it can't be stopped from is a status for the rail and the
    // result's `checks`, never words of the agent's reply (#11556).
    let briefed = briefing
        .filter(|briefing| !briefing.trim().is_empty())
        .map(|briefing| briefed_task(briefing, task))
        .filter(|briefed| briefed.len() <= TEXT_MAX);
    let task = briefed.as_deref().unwrap_or(task);
    let opening = Opening {
        spec: acp_client::process::Spec {
            program,
            arguments,
            cwd: cwd.to_path_buf(),
            environment,
        },
        resume: None,
        meta: None,
        mode,
        authenticate: cursor.then(|| acp_client::cursor::AUTH_METHOD.to_owned()),
    };
    // Oh My Pi runs a model Coder names (#11556): the pick, else its first
    // model that can chat, never a Clef decision model. With none, the
    // turn says so in one plain line.
    let named_model = if agent.id == crate::agent_models::OMP {
        let wanted = model
            .map(str::to_owned)
            .filter(|model| !crate::agent_models::is_clef(model))
            .or_else(|| crate::agent_models::turn_model_reading(&agent.id, &agent.program))
            .filter(|model| !crate::agent_models::is_clef(model));
        match wanted {
            Some(wanted) => Some(wanted),
            None => {
                return Err(format!(
                    "{} has no model here that can chat. Log in to a provider in {}, then pick one with /models.",
                    agent.name, agent.name
                ));
            }
        }
    } else {
        None
    };
    // A temporary failure before the agent did anything (a provider whose
    // endpoint is down, Devin's retryable internal error) is tried again
    // after a pause, a bounded number of times (#11464, #11465).
    let mut attempt = 0;
    loop {
        attempt += 1;
        let mut session =
            Session::open(&opening, &canceled)
                .await
                .map_err(|failure| match &failure {
                    _ if cursor && failure.unauthenticated() => {
                        acp_client::cursor::SIGN_IN.to_owned()
                    }
                    acp_client::Failure::Protocol {
                        error: acp_client::ClientError::Silent { method, .. },
                        ..
                    } if cursor && method == acp_client::wire::method::AUTHENTICATE => {
                        acp_client::cursor::SILENT_SIGN_IN.to_owned()
                    }
                    _ => failure.to_string(),
                })?;
        // A read-only task switches the agent to a mode of its own that
        // makes no changes, when it offers one; its asks to change anything
        // are refused either way.
        let held = if !read_only {
            None
        } else if cursor {
            Some(acp_client::cursor::Mode::Ask.id().to_owned())
        } else {
            let offered = session
                .opened
                .modes
                .as_ref()
                .map(|modes| modes.available_modes.clone())
                .unwrap_or_default();
            match READ_ONLY_MODES
                .iter()
                .find(|wanted| offered.iter().any(|mode| mode.id == **wanted))
            {
                Some(wanted) => match session.set_mode(wanted, &canceled).await {
                    Ok(()) => Some((*wanted).to_owned()),
                    Err(error) => {
                        session.close(Duration::from_secs(2)).await;
                        return Err(format!(
                            "{} could not switch to its read-only mode: {error}",
                            agent.name
                        ));
                    }
                },
                None => None,
            }
        };
        let access = match (&held, read_only) {
            (Some(mode), _) => permissions(true, true, &format!("the agent's {mode} mode")),
            (None, true) => permissions(
                true,
                false,
                "This agent has no read-only mode. Its asks to change files are refused, but it can still change them without asking.",
            ),
            (None, false) if crate::approval::gated() => permissions(
                false,
                false,
                "The agent's asks to change files are refused.",
            ),
            (None, false) => permissions(
                false,
                false,
                "The agent's asks are answered by Coder's checks.",
            ),
        };
        let id = session.id().to_owned();
        // Its version, for the routing records (#11535).
        crate::routing_record::note_agent_version(
            &agent.id,
            session
                .initialized
                .agent_info
                .as_ref()
                .and_then(|info| info.version.as_deref()),
        );
        let mut model = session.opened.model().map(str::to_owned);
        if let Some(named) = &named_model
            && model.as_deref() != Some(named.as_str())
        {
            match session.set_config_option("model", named, &canceled).await {
                Ok(()) => model = Some(named.clone()),
                Err(_) => {
                    session.close(Duration::from_secs(2)).await;
                    return Err(format!(
                        "{} couldn't switch to {named}. Pick another with /models.",
                        agent.name
                    ));
                }
            }
        }
        // Devin starts on its account default when it doesn't allow the
        // model asked for (a Free plan offers only SWE-1.6 Slow): that turn
        // is refused in one plain line, never run on the slower model
        // (#11583).
        if let Some(asked) = &devin_asked
            && !acp_client::devin::admits(asked, model.as_deref())
        {
            let line = devin_model_refused(
                &agent.name,
                asked,
                model.as_deref(),
                &acp_client::devin::offered_models(&session.opened),
            );
            session.close(Duration::from_secs(2)).await;
            return Err(line);
        }
        if model.as_deref().is_some_and(crate::agent_models::is_clef) {
            session.close(Duration::from_secs(2)).await;
            return Err(format!(
                "{} couldn't answer: its model can't chat. Pick another with /models.",
                agent.name
            ));
        }
        if let Some(model) = &model {
            emit(RuntimeEvent::Model(model.clone()));
        }
        // The host's turn events (#11508): SessionStart and
        // UserPromptSubmit add their context ahead of the task, or refuse
        // it.
        let mut turns = coder_sdk::acp::Turns::new(hooks.clone());
        let prompt = match turns.prompt(cwd, task).await {
            Ok(prompt) => prompt,
            Err(reason) => {
                session.close(Duration::from_secs(2)).await;
                return Err(format!("A Coder check refused the task: {reason}"));
            }
        };
        let refusals = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut handler = AcpEvents {
            thought: crate::claude_stream::Thought::default(),
            emit: &mut *emit,
            text: String::new(),
            tools: BTreeMap::new(),
            read_only,
            hooks: Some(hooks.clone()),
            cwd: cwd.to_path_buf(),
            seen: BTreeMap::new(),
            refusals: Arc::clone(&refusals),
            last_thought: LastThought::default(),
        };
        let mut result = session
            .prompt(
                &prompt,
                Duration::from_secs(120),
                &canceled,
                Duration::from_secs(2),
                &mut handler,
            )
            .await;
        // A refused step is told to the agent (ACP's refusal carries no
        // reason), and a Stop hook's block sends its reason, each as the
        // next prompt in the same session, a few times at most.
        let mut follow_ups = 0;
        while result.is_ok() && follow_ups < coder_sdk::acp::MAX_CONTINUED && !canceled() {
            let refused: Vec<String> = std::mem::take(
                &mut *refusals
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            let next = if refused.is_empty() {
                turns.stopped(&handler.text).await
            } else {
                Some(refused_steps(&refused))
            };
            let Some(next) = next else { break };
            follow_ups += 1;
            result = session
                .prompt(
                    &next,
                    Duration::from_secs(120),
                    &canceled,
                    Duration::from_secs(2),
                    &mut handler,
                )
                .await;
        }
        handler.end_thought();
        // The answer, else the last thinking block when the agent put its
        // answer there and wrote no message (#11583).
        let (text, reply_source) = turn_reply(&handler.text, &handler.last_thought.text);
        let worked = !handler.tools.is_empty() || !text.is_empty();
        let group_clear = session.close(Duration::from_secs(2)).await;
        turns.end("close").await;
        // An agent that put its model's failure into its reply ("400 …
        // raw-http-request=…") failed the turn: one plain line shows, and
        // the raw words are kept for the record only (#11556).
        if result.is_ok() && crate::agent_models::raw_failure(&text) {
            crate::agent_models::keep_raw_failure(&text);
            return Err(crate::agent_models::failure_line(&agent.name, &text));
        }
        let error = match result {
            Ok(reply) => {
                // A turn that ended with no words is a failure the caller
                // sees, never an empty success (#11583).
                if text.trim().is_empty() && reply.stop_reason == acp_client::StopReason::EndTurn {
                    return Err(empty_reply(
                        &agent.name,
                        reply.usage.as_ref().and_then(|usage| usage.output_tokens),
                    ));
                }
                let mut result = json!({"session":id,"reply":text,"model":model,"stop_reason":reply.stop_reason.as_str(),"usage":reply.usage,"group_clear":group_clear,"attempts":attempt});
                if let Some(source) = reply_source {
                    // Its words came as thinking, not as a message.
                    result["reply_source"] = json!(source);
                }
                if let Some(note) = &checked.note {
                    result["update"] = json!(note);
                }
                result["checks"] = json!({
                    "before_each_step": checks.is_none(),
                    "note": checks.clone().unwrap_or_else(|| format!("Every step {} takes goes through Coder's checks first.", agent.name)),
                });
                return Ok(with(result, access));
            }
            Err(error) => error,
        };
        let refusal = match &error {
            acp_client::ClientError::Refused { error, .. } => Some(saved_reason(agent, &id, error)),
            _ => None,
        };
        // Cursor's own Update Required is kept with the build it refused,
        // so the next turn to that build stops before it starts.
        if cursor
            && let Some(refusal) = &refusal
            && refusal.needs_update()
            && let Some(installed) = &checked.installed
        {
            crate::engine_update::record_refused(
                engine_path.as_deref(),
                crate::engine_update::CURSOR,
                installed,
            );
        }
        if let Some(refusal) = &refusal
            && refusal.transient()
            && !worked
            && group_clear
            && attempt <= ACP_RETRY_PAUSES.len()
            && !canceled()
        {
            let pause = ACP_RETRY_PAUSES[attempt - 1];
            let until = Instant::now() + pause;
            while Instant::now() < until && !canceled() {
                tokio::time::sleep(POLL).await;
            }
            if !canceled() {
                continue;
            }
        }
        return Err(acp_failure(
            agent,
            model.as_deref(),
            &error,
            refusal.as_ref(),
            cwd,
            attempt,
            group_clear,
        ));
    }
}

/// The line a Devin turn ends with when Devin started on another model
/// than the one asked for (#11583): the model asked for, what the account
/// offers, and where to change either.
fn devin_model_refused(
    name: &str,
    asked: &str,
    reported: Option<&str>,
    offered: &[String],
) -> String {
    let words = |model: &str| crate::agent_models::display(crate::agent_models::DEVIN, model);
    let offers = match offered {
        [] => reported.map_or_else(|| "another model".to_owned(), words),
        [only] => format!("only {}", words(only)),
        many => many
            .iter()
            .map(|model| words(model))
            .collect::<Vec<_>>()
            .join(", "),
    };
    format!(
        "{name} can't run {} on this Devin account: it offers {offers} here. Upgrade the Devin plan at https://app.devin.ai, or pick a model it offers with /models.",
        words(asked)
    )
}

/// What a session keeps alive so an agent's own hooks reach Coder's
/// (#11508): [`coder_sdk::acp::Gate`].
pub(crate) type AcpGate = coder_sdk::acp::Gate;

/// Starts `agent` so every tool reaches the session's hooks first, where
/// it can (#11508): [`coder_sdk::acp::gate`], with this program as the
/// bridge.
///
/// # Errors
/// The bridge, its socket or its files can't be set up: the agent would
/// run unchecked, so it doesn't run.
pub(crate) fn acp_gate(
    agent: &AcpAgent,
    hooks: &coder_sdk::Hooks,
    arguments: &mut Vec<String>,
    environment: &mut Vec<(String, String)>,
) -> Result<AcpGate, String> {
    coder_sdk::acp::gate(
        &agent.id,
        &agent.name,
        hooks,
        bridge_program(),
        arguments,
        environment,
    )
}

/// The program an agent's hook runs for the bridge: this one. A debug build
/// may name another in `CODER_HOOK_PROGRAM`, for the live runs started from
/// a test binary (`tests/acp_live.rs`).
fn bridge_program() -> std::io::Result<PathBuf> {
    if cfg!(debug_assertions)
        && let Some(program) = std::env::var_os("CODER_HOOK_PROGRAM").filter(|p| !p.is_empty())
    {
        return Ok(program.into());
    }
    std::env::current_exe()
}

/// What the agent reads after Coder refused some of its steps: ACP's
/// refusal carries no reason, so this is where it learns why.
fn refused_steps(reasons: &[String]) -> String {
    let mut text = String::from("Coder refused these steps:\n");
    for reason in reasons {
        text.push_str("- ");
        text.push_str(reason);
        text.push('\n');
    }
    text.push_str("Don't try them another way. Finish what you can without them, and say what the person needs to do.");
    text
}

/// The pauses before each new try of a turn that failed for a temporary
/// reason; their count is how many more tries a turn gets.
#[cfg(not(test))]
const ACP_RETRY_PAUSES: [Duration; 2] = [Duration::from_secs(5), Duration::from_secs(20)];
#[cfg(test)]
const ACP_RETRY_PAUSES: [Duration; 2] = [Duration::from_millis(10), Duration::from_millis(10)];

/// The refusal `error`, with the provider's own words OpenCode saved on the
/// failed message added to it: OpenCode's ACP server answers a failed
/// prompt with a bare `Internal error`, and keeps the reason (`Endpoint is
/// unavailable`, Cursor's `Update Required`) in its database.
fn saved_reason(
    agent: &AcpAgent,
    session: &str,
    error: &acp_client::wire::RpcError,
) -> acp_client::wire::RpcError {
    let mut error = error.clone();
    if agent.id != "opencode" {
        return error;
    }
    #[cfg(unix)]
    {
        let variable = |name: &str| std::env::var_os(name);
        let database = std::env::var_os(acp_client::opencode::DATABASE_VAR)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                acp_client::opencode::data_dir(&variable).map(|dir| dir.join("opencode.db"))
            });
        if let Some(saved) =
            database.and_then(|database| coder_history::opencode::last_error(&database, session))
        {
            let mut data = error.data.take().unwrap_or_else(|| json!({}));
            if let Some(object) = data.as_object_mut() {
                object.insert("details".into(), saved);
            }
            error.data = Some(data);
        }
    }
    #[cfg(not(unix))]
    let _ = session;
    error
}

/// What a failed ACP turn tells the model that asked for it: the agent, the
/// model it ran, what went wrong in the agent's own words, and what to do.
fn acp_failure(
    agent: &AcpAgent,
    model: Option<&str>,
    error: &acp_client::ClientError,
    refusal: Option<&acp_client::wire::RpcError>,
    cwd: &Path,
    attempts: usize,
    group_clear: bool,
) -> String {
    let on = model
        .map(|model| format!(" on {model}"))
        .unwrap_or_default();
    let tries = if attempts > 1 {
        format!(" It failed the same way {attempts} times.")
    } else {
        String::new()
    };
    let still = still_running(group_clear);
    let Some(refusal) = refusal else {
        return format!("The agent could not finish the task: {error}.{still}");
    };
    let reason = refusal.reason();
    if reason.to_lowercase().contains("untrusted workspace") {
        return format!(
            "{} does not trust {} yet, so it refused to run there. Open {} in that folder once and trust it, then try again.{still}",
            agent.name,
            cwd.display(),
            agent.name
        );
    }
    if refusal.needs_update() {
        if agent.id == "cursor" {
            return format!("{} It said: {reason}.{still}", acp_client::cursor::UPDATE);
        }
        return format!(
            "{}{on} refused this version: {reason}. Update {} (or the provider its model uses), then try again.{still}",
            agent.name, agent.name
        );
    }
    if refusal.transient() {
        let trace = refusal
            .trace_id()
            .map(|id| format!(" Trace ID: {id}."))
            .unwrap_or_default();
        return format!(
            "{}{on} reported a temporary error: {reason}.{tries} Trying again later usually works.{trace}{still}",
            agent.name
        );
    }
    format!(
        "The agent could not finish the task: {}{on} said: {reason} ({}).{still}",
        agent.name, refusal.code
    )
}

/// Refuse a Cursor delegation that has neither a credential variable nor a
/// stored login, before the agent starts.
async fn cursor_signed_in(
    program: &Path,
    environment: &[(String, String)],
    cwd: &Path,
) -> Result<(), String> {
    if !acp_client::cursor::has_credential(environment)
        && acp_client::cursor::signed_in(program, environment, cwd).await == Some(false)
    {
        return Err(acp_client::cursor::SIGN_IN.into());
    }
    Ok(())
}

fn codex_failure_reason(ending: &supervise::Ending, error: Option<&str>, stderr: &str) -> String {
    if matches!(ending, supervise::Ending::TimedOut) {
        return "The Codex task timed out.".into();
    }
    if let Some(error) = error {
        return bounded(&error.replace(['\n', '\r'], " "), 512);
    }
    if coder_delegate::limit::says_limited(stderr) {
        return "Codex reached a usage limit or rate limit.".into();
    }
    match ending {
        supervise::Ending::Exited(Some(code)) => format!("Codex exited with status {code}."),
        supervise::Ending::Exited(None) => "Codex was terminated by a signal.".into(),
        supervise::Ending::Failed(error) => bounded(&error.replace(['\n', '\r'], " "), 512),
        supervise::Ending::TimedOut => unreachable!(),
    }
}

/// The refusal a Codex limit's words carry: Codex's typed
/// `usage_limit_reached` body when the reason is one, else the kind and the
/// reset its message names. The same record Microcoder keeps in its
/// result's `refusals` (#11466).
fn codex_refusal(reason: &str, now: u64) -> microcoder_loop::capacity::Refusal {
    use microcoder_loop::capacity::{Provider, Refusal, kind_of_message};
    Refusal::codex(429, reason, now).unwrap_or_else(|| {
        Refusal::new(
            Provider::Codex,
            kind_of_message(reason),
            now,
            coder_delegate::limit::reset_from_message(reason, now),
        )
    })
}

/// The error a Codex delegation that hit a limit ends with: one sentence
/// that names the limit and its reset, says no other engine ran the task,
/// what Codex said, and, on its own last line, the typed refusal.
fn codex_out_of_capacity(
    refusal: &microcoder_loop::capacity::Refusal,
    reason: &str,
    group_clear: bool,
) -> String {
    format!(
        "Codex is out of capacity ({}), so continue without Codex. No other engine ran this task: it is back with you, to do on your own model or hand to another agent. Codex said: {reason}.{}\n{}",
        refusal.describe(),
        still_running(group_clear),
        refusal.carry()
    )
}

/// A delegated task with the chat's briefing ahead of it.
#[must_use]
pub fn briefed_task(briefing: &str, task: &str) -> String {
    format!("{}\n\n# The task\n\n{task}", briefing.trim_end())
}

/// Drive Codex's native protocol without treating its executable as an ACP
/// server. `briefed` says the task carries the chat's briefing, so Codex's
/// own `AGENTS.md` reading is off.
async fn codex_cli(
    program: &Path,
    task: &str,
    cwd: &Path,
    sandbox: CodexSandbox,
    briefed: bool,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    if !crate::approval::tools_allowed() {
        return Err("The crew charter refuses Codex delegation, including read-only work.".into());
    }
    // Full access runs on app-server under Coder's hooks, never Codex's
    // bypass (#11510); exec stays the fallback where app-server won't start.
    if sandbox == CodexSandbox::FullAccess
        && let Some(result) = codex_app_server(program, task, cwd, briefed, cancel, emit).await
    {
        return result;
    }
    // An npm `codex.cmd` runs as Node and its script (#11645).
    let mut command = acp_client::process::command(program);
    command
        .args(["exec", "--json", "--skip-git-repo-check"])
        .envs(acp_client::process::NON_INTERACTIVE);
    let variable = |name| std::env::var(name).ok().filter(|value| !value.is_empty());
    let model = next_codex_model().or_else(|| variable("CODER_CODEX_MODEL"));
    let reasoning = variable("CODER_CODEX_REASONING");
    if let Some(model) = &model {
        command.args(["--model", model]);
        emit(RuntimeEvent::Model(
            crate::models::GenerationOptions {
                reasoning: reasoning.clone(),
                max_tokens: None,
            }
            .slug(model),
        ));
    }
    if let Some(effort) = &reasoning {
        command.args([
            "-c",
            &format!(
                "model_reasoning_effort={}",
                serde_json::to_string(&effort)
                    .map_err(|_| "Cannot encode Codex reasoning settings.")?
            ),
        ]);
    }
    if sandbox == CodexSandbox::FullAccess {
        command.arg("--dangerously-bypass-approvals-and-sandbox");
    } else {
        command.args(["--sandbox", sandbox.word()]);
    }
    if briefed {
        command.args(["-c", "project_doc_max_bytes=0"]);
    }
    // The floors' hook (#11475): a production deploy, a merge, a secret or
    // a payment is held for the person.
    if let Ok(this) = std::env::current_exe() {
        command.args(crate::floors::codex_args(&this));
    }
    command
        .args(["-c", "approval_policy=\"never\"", "-"])
        .current_dir(cwd);
    let (mark, value) = coder_delegate::delegate::Agent::Codex.engine_mark();
    command.env(mark, value);
    scrub_credentials(&mut command);
    // The run's hooks on Codex's command tier (#11500): its hook bridge
    // asks this host, kept until the run ends.
    let hook_host = crate::sdk_hooks::codex_host(cwd);
    if let Some(host) = &hook_host {
        command.env(coder_sdk::bridge::SOCKET_VAR, host.path());
    }
    let mut live = supervise::Job::from_command(command)
        .bounded(supervise::Limits::until_stopped().keeping(TEXT_MAX))
        .start(supervise::Input::Piped)?;
    if let Err(error) = live.send(task.as_bytes()).await {
        let stopped = live.stop().await;
        return Err(format!(
            "Codex could not read the task: {error}.{}",
            still_running(stopped.group_clear)
        ));
    }
    live.close_input();
    let mut events = CodexEvents {
        model,
        reasoning,
        ..CodexEvents::default()
    };
    let mut reader = coder_delegate::tail::Reader::new(TEXT_MAX);
    let mut usage_log = crate::codex_usage::Reader::new();
    let stopped = loop {
        events.delivery(&mut reader, live.take(), emit);
        usage_log.poll(events.session.as_deref(), &mut |event| {
            match &event {
                RuntimeEvent::Tokens(tokens) => events.live_tokens = Some(*tokens),
                RuntimeEvent::Model(model) => {
                    let (model, reasoning) = model
                        .split_once(':')
                        .map_or((model.as_str(), None), |(model, reasoning)| {
                            (model, Some(reasoning.to_owned()))
                        });
                    events.model = Some(model.into());
                    events.reasoning = reasoning;
                }
                _ => {}
            }
            emit(event);
        });
        if live.finished() {
            break live.wait().await;
        }
        if cancel.load(Ordering::Relaxed) {
            break live.stop().await;
        }
        tokio::time::sleep(POLL).await;
    };
    events.delivery(&mut reader, stopped.rest.clone(), emit);
    if let Some(record) = reader.finish() {
        events.record(record, emit);
    }
    if stopped.requested || cancel.load(Ordering::Relaxed) {
        return Err(format!(
            "The Codex task was canceled.{}",
            still_running(stopped.group_clear)
        ));
    }
    if !stopped.ending.success() || events.error.is_some() {
        let reason = codex_failure_reason(
            &stopped.ending,
            events.error.as_deref(),
            &stopped.stderr.marked(),
        );
        // A usage or rate limit says so first, in one sentence, so the
        // chat carries on without Codex and the host can book the limit.
        if coder_delegate::limit::says_limited(&reason) {
            return Err(codex_out_of_capacity(
                &codex_refusal(&reason, coder_delegate::limit::now()),
                &reason,
                stopped.group_clear,
            ));
        }
        return Err(format!(
            "Codex could not finish the task: {reason}.{}",
            still_running(stopped.group_clear)
        ));
    }
    if !events.completed {
        return Err(format!(
            "Codex stopped before it finished its reply.{}",
            still_running(stopped.group_clear)
        ));
    }
    let tokens = events
        .usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(
            events
                .usage
                .get("output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        );
    let tokens = if events.usage.is_null() {
        events.live_tokens.unwrap_or(tokens)
    } else {
        tokens
    };
    Ok(
        json!({"session":events.session,"reply":events.text,"model":events.model.map(|model| crate::models::GenerationOptions { reasoning: events.reasoning, max_tokens: None }.slug(&model)),"stop_reason":"end_turn","usage":events.usage,"tokens":tokens,"group_clear":stopped.group_clear,"transport":"codex-cli","sandbox":sandbox.word(),"truncated":!reader.gaps().is_empty()}),
    )
}

/// A delegated Codex task on `codex app-server` (#11510): one thread, one
/// turn, each command and patch answered by Coder's hooks. `None` when
/// app-server didn't start or open a thread, so the caller falls back to
/// `exec`.
async fn codex_app_server(
    program: &Path,
    task: &str,
    cwd: &Path,
    briefed: bool,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Option<Result<Value, String>> {
    use coder_sdk::codex::{Codex, Control, Open, Output};
    let variable = |name| {
        std::env::var(name)
            .ok()
            .filter(|value: &String| !value.is_empty())
    };
    let launch = crate::codex::launch(program.to_owned(), cwd, briefed);
    let mut codex = Codex::spawn(&launch, crate::sdk_hooks::codex(cwd))
        .await
        .ok()?;
    let model = next_codex_model().or_else(|| variable("CODER_CODEX_MODEL"));
    let reasoning = variable("CODER_CODEX_REASONING");
    let mut session = None;
    codex
        .open(
            &Open {
                cwd: cwd.to_owned(),
                model: model.clone(),
                effort: reasoning.clone(),
                ..Open::default()
            },
            &mut |output| {
                if let Output::Thread(id) = output {
                    session = Some(id);
                }
            },
        )
        .await
        .ok()?;
    let (control, mut controls) = tokio::sync::mpsc::unbounded_channel();
    let mut tokens = 0_u64;
    // Codex's reasoning summary streams into the delegation's transcript.
    let mut thought = crate::claude_stream::Thought::default();
    let mut out = |output: Output| match output {
        Output::Thinking(true) => emit(RuntimeEvent::Thinking(true)),
        Output::ThinkingText(text) => thought.piece(&text).into_iter().for_each(&mut *emit),
        Output::Thinking(false) if thought.open() => {
            thought.end(None).into_iter().for_each(&mut *emit);
        }
        Output::Thinking(false) => emit(RuntimeEvent::Thinking(false)),
        Output::Model(model) => emit(RuntimeEvent::Model(
            crate::models::GenerationOptions {
                reasoning: reasoning.clone(),
                max_tokens: None,
            }
            .slug(&model),
        )),
        Output::Text(text) => emit(RuntimeEvent::Text(text)),
        Output::Tool {
            name,
            input,
            output,
            running,
        } => emit(RuntimeEvent::Tool {
            name,
            input,
            output,
            running,
        }),
        Output::Usage(usage) => {
            tokens = usage["total"]["totalTokens"].as_u64().unwrap_or(tokens);
            emit(RuntimeEvent::Tokens(tokens));
        }
        Output::RateLimits(limits) => {
            crate::codex::observe_limits(&limits, coder_delegate::limit::now());
        }
        _ => {}
    };
    let watch = async {
        while !cancel.load(Ordering::Relaxed) {
            tokio::time::sleep(POLL).await;
        }
        let _ = control.send(Control::Interrupt);
        std::future::pending::<()>().await;
    };
    let ended = tokio::select! {
        ended = codex.turn(task, model.as_deref(), Some(&mut controls), &mut out) => ended,
        () = watch => unreachable!("the watcher never ends"),
    };
    codex.close().await;
    Some(match ended {
        Ok(ended) if ended.interrupted() || cancel.load(Ordering::Relaxed) => {
            Err("The Codex task was canceled.".into())
        }
        Ok(ended) if ended.status == "failed" => {
            let reason = ended
                .error
                .unwrap_or_else(|| "The Codex turn failed.".into());
            if coder_delegate::limit::says_limited(&reason) {
                Err(codex_out_of_capacity(
                    &codex_refusal(&reason, coder_delegate::limit::now()),
                    &reason,
                    true,
                ))
            } else {
                Err(format!("Codex could not finish the task: {reason}."))
            }
        }
        Ok(ended) => Ok(json!({
            "session": session.or(Some(ended.thread)),
            "reply": ended.text,
            "model": ended.model.map(|model| crate::models::GenerationOptions { reasoning, max_tokens: None }.slug(&model)),
            "stop_reason": "end_turn",
            "usage": ended.usage,
            "tokens": tokens,
            "group_clear": true,
            "transport": "codex-app-server",
            "sandbox": CodexSandbox::FullAccess.word(),
            "truncated": false,
        })),
        Err(error) => Err(format!("Codex could not finish the task: {error}")),
    })
}

#[derive(Default)]
struct CodexEvents {
    text: String,
    session: Option<String>,
    model: Option<String>,
    reasoning: Option<String>,
    usage: Value,
    live_tokens: Option<u64>,
    error: Option<String>,
    completed: bool,
    seq: u64,
}

impl CodexEvents {
    fn delivery(
        &mut self,
        reader: &mut coder_delegate::tail::Reader,
        delivery: supervise::Delivery,
        emit: &mut dyn FnMut(RuntimeEvent),
    ) {
        for gap in delivery.gaps {
            reader.dropped(gap.offset, gap.bytes);
        }
        for record in reader.feed(delivery.offset, &delivery.bytes) {
            self.record(record, emit);
        }
    }

    fn record(&mut self, record: coder_delegate::tail::Record, emit: &mut dyn FnMut(RuntimeEvent)) {
        self.line(&record.text, emit);
        for event in coder_delegate::stream::normalize_line(
            coder_delegate::stream::Format::Codex,
            &record.text,
            record.line,
            &mut self.seq,
        ) {
            self.event(event.kind, emit);
        }
    }

    fn line(&mut self, line: &str, emit: &mut dyn FnMut(RuntimeEvent)) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if let Some(model) = value.get("model").and_then(Value::as_str)
            && self.model.as_deref() != Some(model)
        {
            self.model = Some(model.into());
            emit(RuntimeEvent::Model(
                crate::models::GenerationOptions {
                    reasoning: self.reasoning.clone(),
                    max_tokens: None,
                }
                .slug(model),
            ));
        }
        if value["type"] == "item.completed"
            && value["item"]["type"] == "agent_message"
            && let Some(text) = value["item"]["text"].as_str()
        {
            let separator = if self.text.is_empty() { "" } else { "\n\n" };
            let text = bounded(
                &format!("{separator}{text}"),
                TEXT_MAX.saturating_sub(self.text.len()),
            );
            self.text.push_str(&text);
            emit(RuntimeEvent::Text(text));
        }
        if value["type"] == "turn.completed" {
            self.completed = true;
        }
    }

    fn event(&mut self, event: coder_delegate::stream::Kind, emit: &mut dyn FnMut(RuntimeEvent)) {
        use coder_delegate::stream::Kind;
        match event {
            Kind::SessionStarted { session_id } => self.session = session_id,
            Kind::CommandStarted { command } => emit(RuntimeEvent::Tool {
                name: "Run".into(),
                input: json!({"command":command}),
                output: Value::Null,
                running: true,
            }),
            Kind::CommandCompleted {
                command,
                exit_code,
                output,
            } => emit(RuntimeEvent::Tool {
                name: "Run".into(),
                input: json!({"command":command}),
                output: json!({"exit":exit_code,"output":output}),
                running: false,
            }),
            Kind::ArtifactChanged { path, change } => emit(RuntimeEvent::Tool {
                name: "Edit".into(),
                input: json!({"path":path}),
                output: json!({"change":change}),
                running: false,
            }),
            Kind::UsageUpdate { usage } => {
                let tokens = usage["input_tokens"]
                    .as_u64()
                    .unwrap_or(0)
                    .saturating_add(usage["output_tokens"].as_u64().unwrap_or(0));
                self.usage = usage;
                emit(RuntimeEvent::Tokens(tokens));
            }
            Kind::SessionEnded {
                error: true,
                result,
            } => self.error = Some(result.unwrap_or_else(|| "The Codex turn failed.".into())),
            _ => {}
        }
    }
}

struct AcpEvents<'a> {
    emit: &'a mut dyn FnMut(RuntimeEvent),
    text: String,
    /// The agent's thought chunks, streaming into a thinking entry
    /// (#11552).
    thought: crate::claude_stream::Thought,
    tools: BTreeMap<String, (String, Value)>,
    /// A read-only task: only asks to read, search, think or fetch are
    /// approved (#11470).
    read_only: bool,
    /// The session's hooks, which answer each ask (#11508).
    hooks: Option<coder_sdk::Hooks>,
    cwd: PathBuf,
    /// Each tool call's input as its `tool_call` update gave it, for an ask
    /// that carries none (Cursor's).
    seen: BTreeMap<String, Value>,
    /// Why the hooks refused a step, for the agent's next prompt.
    refusals: Arc<std::sync::Mutex<Vec<String>>>,
    /// The agent's last thinking block, for a turn whose answer never
    /// came as message text (#11583).
    last_thought: LastThought,
}

/// The agent's thinking since its last step or words (#11583). Devin's
/// SWE models think in `agent_thought_chunk`s, then answer in
/// `agent_message_chunk`s; sometimes the answer stays in the thinking and
/// the turn ends with no message text at all. That last block is what the
/// turn said.
#[derive(Default)]
struct LastThought {
    text: String,
    /// A step or words came after it: the next thought starts a new block.
    closed: bool,
}

impl LastThought {
    fn push(&mut self, piece: &str) {
        if self.closed {
            self.text.clear();
            self.closed = false;
        }
        if self.text.len() < TEXT_MAX {
            self.text
                .push_str(&bounded(piece, TEXT_MAX.saturating_sub(self.text.len())));
        }
    }

    fn close(&mut self) {
        self.closed = true;
    }
}

/// The reply a turn hands back (#11583): its message text; else, when the
/// agent wrote none, its last thinking block, marked as such; else
/// nothing, which the caller reports as a failure.
fn turn_reply(text: &str, last_thought: &str) -> (String, Option<&'static str>) {
    if !text.trim().is_empty() {
        return (text.to_owned(), None);
    }
    if !last_thought.trim().is_empty() {
        return (last_thought.trim().to_owned(), Some("thought"));
    }
    (String::new(), None)
}

/// The failure a turn that ended with no words is (#11583): it is never
/// handed back as an empty success.
fn empty_reply(name: &str, output_tokens: Option<u64>) -> String {
    let tokens = output_tokens
        .filter(|tokens| *tokens > 0)
        .map(|tokens| format!(", after writing {tokens} tokens"))
        .unwrap_or_default();
    format!("{name} ended without an answer{tokens}. Nothing came back to use; ask it again.")
}

/// Whether an ACP tool kind only looks, never changes anything.
fn looks_only(kind: Option<&str>) -> bool {
    matches!(kind, Some("read" | "search" | "think" | "fetch"))
}

impl Handler for AcpEvents<'_> {
    fn permission(
        &mut self,
        request: &acp_client::wire::PermissionRequest,
    ) -> acp_client::wire::PermissionAnswer {
        let option = if !crate::approval::gated()
            && (!self.read_only || looks_only(request.tool_call.kind.as_deref()))
        {
            request.allow()
        } else {
            request.reject()
        };
        option.map_or(acp_client::wire::PermissionAnswer::Cancelled, |option| {
            acp_client::wire::PermissionAnswer::Selected(option.to_owned())
        })
    }

    /// An ask the chat would approve goes to the session's hooks first
    /// (#11508): the fence, the floors and the rest decide, and a floor's
    /// ask waits on the approval card. A gated chat or a read-only task
    /// refuses what it always refused, at once.
    fn permission_later(
        &mut self,
        request: &acp_client::wire::PermissionRequest,
    ) -> Option<acp_client::PendingAnswer> {
        let hooks = self.hooks.clone()?;
        if crate::approval::gated()
            || (self.read_only && !looks_only(request.tool_call.kind.as_deref()))
        {
            return None;
        }
        let seen = request
            .tool_call
            .tool_call_id
            .as_ref()
            .and_then(|id| self.seen.get(id));
        let call = coder_sdk::acp::call(request, seen, &self.cwd);
        let request = request.clone();
        let refusals = Arc::clone(&self.refusals);
        Some(Box::pin(async move {
            let applied = hooks.fire(call).await;
            if let Some(reason) = coder_sdk::acp::refusal(&applied) {
                refusals
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(reason);
            }
            coder_sdk::acp::answer(&applied.permission, &request)
        }))
    }

    fn notification(&mut self, method: &str, params: &Value) {
        if let Some(notice) = acp_client::cursor::Notice::parse(method, params) {
            self.cursor_notice(notice, Value::Null);
        }
    }

    fn reverse(
        &mut self,
        method: &str,
        params: &Value,
    ) -> Result<Value, acp_client::wire::RpcError> {
        let answer = acp_client::cursor::answer(method, params)
            .ok_or_else(|| acp_client::wire::RpcError::method_not_found(method))?;
        if let Some(notice) = acp_client::cursor::Notice::parse(method, params) {
            self.cursor_notice(notice, answer["outcome"].clone());
        }
        Ok(answer)
    }

    fn update(&mut self, update: Update) {
        // The agent's thinking ends when it answers or acts.
        if matches!(
            update,
            Update::AgentText(_) | Update::ToolCall { .. } | Update::ToolCallUpdate { .. }
        ) {
            self.end_thought();
            self.last_thought.close();
        }
        match update {
            Update::Thought(text) => {
                self.last_thought.push(&text);
                for event in self.thought.piece(&bounded(&text, TEXT_MAX)) {
                    (self.emit)(event);
                }
            }
            Update::Usage(usage) if !usage.subagent => {
                if usage.input_tokens.is_some() || usage.output_tokens.is_some() {
                    (self.emit)(RuntimeEvent::Tokens(
                        usage
                            .input_tokens
                            .unwrap_or(0)
                            .saturating_add(usage.output_tokens.unwrap_or(0)),
                    ));
                }
            }
            Update::AgentText(text) => {
                let text = bounded(&text, TEXT_MAX.saturating_sub(self.text.len()));
                self.text.push_str(&text);
                (self.emit)(RuntimeEvent::Text(text));
            }
            Update::ToolCall {
                id,
                title,
                raw_input,
                status,
                ..
            } => {
                if raw_input
                    .as_object()
                    .is_some_and(|fields| !fields.is_empty())
                {
                    self.seen.insert(id.clone(), raw_input.clone());
                }
                self.tools.insert(id, (title.clone(), raw_input.clone()));
                (self.emit)(RuntimeEvent::Tool {
                    name: title,
                    input: raw_input,
                    output: Value::Null,
                    running: status != "completed" && status != "failed",
                });
            }
            Update::ToolCallUpdate {
                id,
                title,
                status,
                text,
                raw_input,
            } => {
                let (name, input) = self
                    .tools
                    .entry(id.clone())
                    .or_insert_with(|| (title.unwrap_or(id), Value::Null));
                // Input that arrives after the call opened (OpenCode sends
                // the command and its description here) joins what it had.
                if let Some(Value::Object(fields)) = raw_input {
                    if !input.is_object() {
                        *input = json!({});
                    }
                    if let Some(held) = input.as_object_mut() {
                        held.extend(fields);
                    }
                }
                (self.emit)(RuntimeEvent::Tool {
                    name: name.clone(),
                    input: input.clone(),
                    output: json!(text.map(|text| bounded(&text, TEXT_MAX))),
                    running: status
                        .as_deref()
                        .is_none_or(|status| status != "completed" && status != "failed"),
                });
            }
            _ => {}
        }
    }
}

impl AcpEvents<'_> {
    /// The thinking under way closes with what it said.
    fn end_thought(&mut self) {
        for event in self.thought.end(None) {
            (self.emit)(event);
        }
    }

    /// Show one of Cursor's extension methods as a finished tool row, with
    /// the client's `answer` when the agent waited on one.
    fn cursor_notice(&mut self, notice: acp_client::cursor::Notice, answer: Value) {
        use acp_client::cursor::Notice;
        let todos = |todos: &[acp_client::cursor::Todo]| {
            todos
                .iter()
                .map(|todo| json!({"content": bounded(&todo.content, 512), "status": todo.status}))
                .collect::<Vec<_>>()
        };
        let (name, input) = match notice {
            Notice::Question { title, prompts } => (
                "Question",
                json!({"title": title, "questions": prompts.iter().map(|p| bounded(p, 512)).collect::<Vec<_>>()}),
            ),
            Notice::Plan {
                name,
                overview,
                plan,
                todos: steps,
            } => (
                "Plan",
                json!({"name": name, "overview": overview, "plan": bounded(&plan, TEXT_MAX), "todos": todos(&steps)}),
            ),
            Notice::Todos {
                todos: entries,
                merge,
            } => ("Todos", json!({"todos": todos(&entries), "merge": merge})),
            Notice::Task {
                description,
                subagent,
                model,
                duration_ms,
            } => (
                "Task",
                json!({"description": bounded(&description, 512), "subagent": subagent, "model": model, "duration_ms": duration_ms}),
            ),
            Notice::Image { description, path } => (
                "Image",
                json!({"description": bounded(&description, 512), "path": path}),
            ),
        };
        (self.emit)(RuntimeEvent::Tool {
            name: name.into(),
            input,
            output: answer,
            running: false,
        });
    }
}

fn validate_task(task: &str) -> Result<(), String> {
    if task.trim().is_empty() || task.len() > TEXT_MAX {
        Err("A task must contain 1 to 65,536 bytes.".into())
    } else {
        Ok(())
    }
}

pub async fn microcoder_local(
    task: &str,
    cwd: &Path,
    read_only: bool,
    claude: bool,
    cheap: bool,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    validate_task(task)?;
    // Tests never resolve this computer's real logins or write to its home.
    #[cfg(test)]
    {
        let _ = (
            cwd,
            read_only,
            claude,
            cheap,
            jev,
            redaction_keys,
            cancel,
            emit,
        );
        return Err("The local model logins are unavailable in tests.".into());
    }
    #[cfg(not(test))]
    {
        let session = format!("coder-new-{}", std::process::id());
        // The Claude Code plugin off: its login is left out, and the loop's
        // commands get `CLAUDE_BIN=/dev/null`, so `claude` never starts
        // (#11516). A cheap answer runs each login's cheapest model
        // (#11537).
        let generator = coder::local_chain::local_generator_for(
            &session,
            cheap.then(|| crate::route::CHEAP_CODEX_MODEL.to_owned()),
            None,
            claude,
            cheap.then(|| crate::route::CHEAP_CLAUDE_MODEL.to_owned()),
        )?;
        if !claude {
            let mut vars = child_env();
            vars.retain(|(name, _)| name != "CLAUDE_BIN");
            vars.push(("CLAUDE_BIN".into(), "/dev/null".into()));
            set_child_env(vars);
        }
        microcoder_on(
            &generator,
            task,
            cwd,
            read_only,
            jev,
            redaction_keys,
            cancel,
            emit,
        )
        .await
    }
}

/// A model the person picked runs on their own plan (#11550): `login`'s
/// login only, on `model`, never another login's model. `Err` before the
/// loop starts when the login isn't here; a login out of capacity ends
/// with `no_capacity`, for the caller to take the turn elsewhere.
#[allow(clippy::too_many_arguments)]
pub async fn microcoder_plan(
    task: &str,
    cwd: &Path,
    login: coder::local_chain::PlanLogin,
    model: &str,
    effort: Option<String>,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    validate_task(task)?;
    #[cfg(test)]
    {
        let _ = (cwd, login, model, effort, jev, redaction_keys, cancel, emit);
        return Err("The local model logins are unavailable in tests.".into());
    }
    #[cfg(not(test))]
    {
        let session = format!("coder-new-{}", std::process::id());
        let generator = coder::local_chain::plan_generator_for(&session, login, model, effort)?;
        microcoder_on(
            &generator,
            task,
            cwd,
            false,
            jev,
            redaction_keys,
            cancel,
            emit,
        )
        .await
    }
}

/// The local loop on `generator`, with its provider, fallbacks and
/// refusals in the result.
#[cfg(not(test))]
#[allow(clippy::too_many_arguments)]
async fn microcoder_on(
    generator: &coder::local_chain::LocalGenerator,
    task: &str,
    cwd: &Path,
    read_only: bool,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    // What the Claude Code login's calls print, as it arrives: its
    // thinking for the transcript and its plan windows for the rail
    // (#11537).
    let (lines, thoughts) = std::sync::mpsc::channel::<Value>();
    generator.watch_claude(&claude_watcher(lines));
    let mut result = run_microcoder_with(
        task,
        cwd,
        read_only,
        generator,
        jev,
        redaction_keys,
        cancel,
        emit,
        Some(thoughts),
    )
    .await?;
    result["provider"] = json!(generator.provider());
    let fell_back_from = generator.fell_back_from();
    if !fell_back_from.is_empty() {
        result["fell_back_from"] = json!(fell_back_from);
    }
    let refusals = generator.refusals();
    if !refusals.is_empty() {
        result["refusals"] = json!(refusals);
    }
    Ok(result)
}

/// The watcher on the Claude Code login's calls: the plan's windows go
/// to [`crate::claude_plan`] at once; thinking lines go to `lines`, for
/// the loop to show as they stream ([`Thoughts`]).
#[cfg_attr(test, allow(dead_code))]
fn claude_watcher(lines: std::sync::mpsc::Sender<Value>) -> microcoder_loop::claude::Watcher {
    let lines = std::sync::Mutex::new(lines);
    Arc::new(move |line: &Value| match line["type"].as_str() {
        Some("rate_limit_event") => {
            crate::claude_plan::observe_rate_limit(
                &line["rate_limit_info"],
                coder_delegate::limit::now(),
            );
        }
        Some("stream_event" | "assistant") => {
            if let Ok(lines) = lines.lock() {
                let _ = lines.send(line.clone());
            }
        }
        _ => {}
    })
}

/// The model's thinking from the Claude Code login's lines, as the
/// Claude Code harness shows it (#11537): `Thinking` on the rail while a
/// thinking block streams, and each finished block as a thinking entry.
#[derive(Default)]
pub(crate) struct Thoughts {
    streaming: bool,
    /// The thinking's words as they stream (#11552).
    words: crate::claude_stream::ClaudeThoughts,
}

impl Thoughts {
    /// What `line` shows.
    pub(crate) fn read(&mut self, line: &Value) -> Vec<RuntimeEvent> {
        let mut shown = Vec::new();
        match line["type"].as_str() {
            Some("stream_event") => {
                let event = &line["event"];
                let now = match event["type"].as_str() {
                    Some("content_block_start") => event["content_block"]["type"] == "thinking",
                    Some("content_block_delta") => {
                        event["delta"]["type"] == "thinking_delta"
                            || (self.streaming && event["delta"]["type"] == "signature_delta")
                    }
                    Some("content_block_stop" | "message_stop") => false,
                    _ => self.streaming,
                };
                if now != self.streaming {
                    self.streaming = now;
                    shown.push(RuntimeEvent::Thinking(now));
                }
                shown.extend(self.words.event(event));
            }
            Some("assistant") => {
                for block in line["message"]["content"].as_array().into_iter().flatten() {
                    if block["type"] == "thinking"
                        && let Some(entry) = self.words.block(block)
                    {
                        shown.push(entry);
                    }
                }
            }
            _ => {}
        }
        shown
    }

    /// The rail's `Thinking` stops with the call, and thinking still
    /// streaming closes with what it said.
    pub(crate) fn end(&mut self) -> Vec<RuntimeEvent> {
        let mut shown = self.words.end();
        if std::mem::take(&mut self.streaming) {
            shown.push(RuntimeEvent::Thinking(false));
        }
        shown
    }
}

/// Coder's own agent on the OpenAgents AI Gateway only, for a task a
/// Claude plan's usage limit paused that the person handed to OpenAgents
/// (#11295). It runs no Codex or Claude Code login, and its commands get
/// `CLAUDE_BIN=/dev/null`, so `claude` never starts.
pub async fn microcoder_gateway(
    task: &str,
    cwd: &Path,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    validate_task(task)?;
    let generator = coder::local_chain::gateway_generator()?;
    let mut vars = child_env();
    vars.retain(|(name, _)| name != "CLAUDE_BIN");
    vars.push(("CLAUDE_BIN".into(), "/dev/null".into()));
    set_child_env(vars);
    let mut result = run_microcoder_with(
        task,
        cwd,
        false,
        &generator,
        jev,
        redaction_keys,
        cancel,
        emit,
        None,
    )
    .await?;
    result["provider"] = json!(generator.provider());
    let refusals = generator.refusals();
    if !refusals.is_empty() {
        result["refusals"] = json!(refusals);
    }
    Ok(result)
}

pub async fn microcoder_openrouter(
    task: &str,
    cwd: &Path,
    read_only: bool,
    client: openrouter::Client,
    model: String,
    effort: Option<String>,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    validate_task(task)?;
    let runs_on = crate::ui::rail_model_name(&model);
    let generator = Stated {
        inner: OpenRouterGenerator {
            client,
            model,
            effort,
        },
        runs_on,
    };
    let mut result = run_microcoder_with(
        task,
        cwd,
        read_only,
        &generator,
        jev,
        redaction_keys,
        cancel,
        emit,
        None,
    )
    .await?;
    result["usage"] = json!({"cost": result["outcome"]["model_usd"]});
    result["provider"] = json!("openrouter");
    Ok(result)
}

/// A generator that tells its model which model it is (#11593): the
/// loop's identity line names `runs_on`, so the agent states the real
/// model instead of guessing.
pub(crate) struct Stated<G> {
    pub inner: G,
    pub runs_on: String,
}

impl<G: Generate> Generate for Stated<G> {
    async fn generate(&self, system: &str, prompt: &str) -> microcoder_loop::models::Generated {
        let system = format!("{system}{}", microcoder_loop::run::runs_on(&self.runs_on));
        self.inner.generate(&system, prompt).await
    }

    fn out_of_capacity(&self) -> Option<microcoder_loop::models::Exhausted> {
        self.inner.out_of_capacity()
    }
}

/// Microcoder's model when nobody names one (#11593): Gemini 4 Argon
/// through the OpenAgents gateway, Google first.
pub const MICROCODER_MODEL: &str = coder_sdk::auto_policy::ARGON;

/// Gateway calls in a row that come back with no reply before Microcoder
/// leaves the gateway for this computer's logins.
const GATEWAY_MISSES: u32 = 2;

/// Microcoder's default door (#11593): every step on [`MICROCODER_MODEL`]
/// through the OpenAgents gateway as the signed-in account. When the
/// gateway stops answering ([`GATEWAY_MISSES`] calls in a row with no
/// reply), the rest of the task runs on `fallback`'s generator (this
/// computer's logins, Codex first), without replaying completed steps,
/// and [`GatewayFirst::left`] says why. A reply the model got wrong is the
/// loop's to recover, never a reason to leave.
pub(crate) struct GatewayFirst<G, F> {
    gateway: G,
    fallback: Box<dyn Fn() -> Result<F, String>>,
    local: std::cell::OnceCell<F>,
    misses: Cell<u32>,
    left: RefCell<Option<String>>,
}

impl<G: Generate, F: Generate> GatewayFirst<G, F> {
    pub(crate) fn new(gateway: G, fallback: Box<dyn Fn() -> Result<F, String>>) -> Self {
        GatewayFirst {
            gateway,
            fallback,
            local: std::cell::OnceCell::new(),
            misses: Cell::new(0),
            left: RefCell::new(None),
        }
    }

    /// Why the task left the gateway, once it did.
    pub(crate) fn left(&self) -> Option<String> {
        self.left.borrow().clone()
    }

    /// The generator the task fell back to, once it did.
    pub(crate) fn fallback(&self) -> Option<&F> {
        self.local.get()
    }
}

impl<G: Generate, F: Generate> Generate for GatewayFirst<G, F> {
    async fn generate(&self, system: &str, prompt: &str) -> microcoder_loop::models::Generated {
        if let Some(local) = self.local.get() {
            return local.generate(system, prompt).await;
        }
        let stated = format!(
            "{system}{}",
            microcoder_loop::run::runs_on(&format!(
                "{} through the OpenAgents gateway",
                crate::ui::rail_model_name(MICROCODER_MODEL)
            ))
        );
        let generated = self.gateway.generate(&stated, prompt).await;
        let unanswered = generated.action.is_err()
            && generated.prompt_tokens == 0
            && generated.completion_tokens == 0;
        if !unanswered {
            self.misses.set(0);
            return generated;
        }
        self.misses.set(self.misses.get().saturating_add(1));
        if self.misses.get() < GATEWAY_MISSES {
            return generated;
        }
        let why = generated.action.as_ref().err().cloned().unwrap_or_default();
        match (self.fallback)() {
            Ok(local) => {
                let local = self.local.get_or_init(|| local);
                *self.left.borrow_mut() = Some(why);
                let next = local.generate(system, prompt).await;
                microcoder_loop::failover::merge(Some(generated), next)
            }
            Err(error) => {
                *self.left.borrow_mut() = Some(format!("{why}; and {error}"));
                generated
            }
        }
    }

    fn out_of_capacity(&self) -> Option<microcoder_loop::models::Exhausted> {
        self.local.get().and_then(Generate::out_of_capacity)
    }
}

/// Microcoder on its default door (#11593): Gemini 4 Argon through the
/// OpenAgents gateway as `account`, falling back to this computer's
/// logins when the gateway stops answering. The result's `provider` is
/// `openagents`, or the login it ended on with `fell_back_from` and
/// `fallback` saying why.
#[allow(clippy::too_many_arguments)]
pub async fn microcoder_argon(
    task: &str,
    cwd: &Path,
    read_only: bool,
    account: &openagents_login::Saved,
    claude: bool,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    validate_task(task)?;
    let client = crate::provider::Provider::gateway(account)?.client();
    let mut keys = redaction_keys.to_vec();
    keys.push(ApiKey::new(account.token()));
    let session = format!("coder-new-{}", std::process::id());
    let fallback: Box<dyn Fn() -> Result<coder::local_chain::LocalGenerator, String>> =
        Box::new(move || {
            #[cfg(test)]
            {
                let _ = (&session, claude);
                Err("The local model logins are unavailable in tests.".to_owned())
            }
            #[cfg(not(test))]
            {
                coder::local_chain::local_generator_for(&session, None, None, claude, None)
            }
        });
    let gateway = OpenRouterGenerator {
        client,
        model: MICROCODER_MODEL.to_owned(),
        effort: None,
    };
    let generator = GatewayFirst::new(gateway, fallback);
    let mut result = run_microcoder_with(
        task, cwd, read_only, &generator, jev, &keys, cancel, emit, None,
    )
    .await?;
    match (generator.left(), generator.fallback()) {
        (Some(why), Some(local)) => {
            result["provider"] = json!(local.provider());
            result["fell_back_from"] = json!(["openagents"]);
            result["fallback"] = json!(gateway_left(&why, Some(local.provider())));
        }
        (Some(why), None) => {
            result["provider"] = json!("openagents");
            result["fallback"] = json!(gateway_left(&why, None));
        }
        (None, _) => {
            result["provider"] = json!("openagents");
            result["usage"] = json!({"cost": result["outcome"]["model_usd"]});
        }
    }
    Ok(result)
}

/// The sentence a task that left the gateway shows (#11593).
pub(crate) fn gateway_left(why: &str, to: Option<&str>) -> String {
    let why = why.trim().trim_end_matches('.');
    let why = if why.is_empty() {
        "it stopped answering"
    } else {
        why
    };
    match to {
        Some(to) => format!(
            "The OpenAgents gateway couldn't be reached ({why}), so Microcoder finished on this computer's {} login instead of Gemini 4 Argon.",
            login_name(to)
        ),
        None => format!(
            "The OpenAgents gateway couldn't be reached ({why}), and this computer has no login to fall back to."
        ),
    }
}

/// A local chain provider's name, as a person reads it.
pub(crate) fn login_name(provider: &str) -> &'static str {
    match provider {
        "codex" => "Codex",
        "claude" => "Claude Code",
        "openagents" => "OpenAgents cloud",
        _ => "own",
    }
}

#[cfg(test)]
async fn run_microcoder<G: Generate>(
    task: &str,
    cwd: &Path,
    generator: &G,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    run_microcoder_with(
        task,
        cwd,
        false,
        generator,
        jev,
        redaction_keys,
        cancel,
        emit,
        None,
    )
    .await
}

/// The Microcoder loop in `cwd`: every command runs in that folder, the
/// result names it under `cwd` (#11469), and a read-only task's commands
/// run where they cannot write, reported under `permissions` (#11470).
#[allow(clippy::too_many_arguments)]
async fn run_microcoder_with<G: Generate>(
    task: &str,
    cwd: &Path,
    read_only: bool,
    generator: &G,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
    thoughts: Option<std::sync::mpsc::Receiver<Value>>,
) -> Result<Value, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("The coding task was canceled before it started.".into());
    }
    let directory = cwd
        .canonicalize()
        .map_err(|_| format!("The folder {} is unavailable.", cwd.display()))?;
    let boundary = command_boundary(&directory, read_only)?;
    let access = if read_only {
        permissions(true, true, "commands run where they cannot change files")
    } else if boundary.is_some() {
        permissions(false, true, "commands may change files in this folder only")
    } else {
        permissions(false, false, "commands have full access")
    };
    let sink = RefCell::new(emit);
    let environment = Checkout {
        asked: std::sync::Mutex::default(),
        hooks: Some(crate::sdk_hooks::coder_loop(&directory, cancel)),
        directory: directory.clone(),
        boundary,
        cancel: Arc::clone(cancel),
        redaction_keys,
        events: Some(&sink),
    };
    let state = State {
        task: redact_text(task, redaction_keys),
        environment: format!(
            "Working directory: every command runs in {}. {}",
            directory.display(),
            if read_only {
                "This task is read-only: commands cannot change the project's files or the person's. $TMPDIR is a private scratch folder for this task: to build or test, copy the project there (cargo already builds into $CARGO_TARGET_DIR there)."
            } else {
                "Commands have full filesystem and network access unless the host explicitly installs an approval policy."
            }
        ),
        ..State::default()
    };
    let judge = PluginJudge(jev.map(|client| JevJudge { client }));
    let canceled_generator = CancellableGenerator {
        generator,
        cancel,
        redaction_keys,
        failures: Cell::new(0),
        repeated: Cell::new(0),
        previous_commands: RefCell::new(vec![]),
        bases: RefCell::new(vec![]),
        thoughts: thoughts.map(|lines| {
            (
                lines,
                RefCell::new(Thoughts::default()),
                &sink as &dyn crate::delegation_events::Sink,
            )
        }),
    };
    let set = microcoder_loop::models::question_set();
    let route = microcoder_loop::models::route_set();
    let models = Models {
        generator: &canceled_generator,
        judge: &judge,
        set: &set,
        route: &route,
        strong: None,
        knowledge: None,
    };
    let limits = Limits {
        max_steps: None,
        max_seconds: None,
        max_usd: f64::MAX,
        max_bad_replies: usize::MAX,
        max_idle_replies: usize::MAX,
        max_refused_finishes: usize::MAX,
        command_seconds: 120,
        test_seconds: 30,
        acceptance: false,
        stuck_steps: None,
        ..Limits::default()
    };
    let mut observer = MicrocoderEvents {
        emit: &sink,
        reply: String::new(),
        model: None,
        tokens: 0,
        redaction_keys,
    };
    let (_, outcome) = microcoder_loop::run::run(
        state,
        "Complete the user's task.",
        &environment,
        &models,
        &limits,
        &mut observer,
    )
    .await;
    // What the model cost rests on: `billed` when the provider reported
    // what it charged, `list_price` for reported tokens at published prices
    // (the Codex and Claude Code logins), `mixed` after a failover between
    // the two, as Terminal-Bench study records state it (#11276).
    let cost_basis = match canceled_generator.bases.borrow().as_slice() {
        [] => Value::Null,
        [basis] => json!(basis),
        _ => json!("mixed"),
    };
    let mut result = json!({"reply":observer.reply,"model":observer.model,"tokens":observer.tokens,"cost_basis":cost_basis,"outcome":outcome,"cwd":directory.display().to_string(),"permissions":access});
    for key in redaction_keys {
        crate::plugin_tools::redact_value(&mut result, key.expose());
    }
    Ok(result)
}

struct PluginJudge(Option<JevJudge>);

struct CancellableGenerator<'a, G> {
    generator: &'a G,
    cancel: &'a Arc<AtomicBool>,
    redaction_keys: &'a [ApiKey],
    failures: Cell<usize>,
    repeated: Cell<usize>,
    previous_commands: RefCell<Vec<String>>,
    /// The cost basis of each kind every answered generation rested on.
    bases: RefCell<Vec<microcoder_loop::models::Basis>>,
    /// The Claude Code login's lines as they arrive, what they show, and
    /// where it goes (#11537).
    thoughts: Option<(
        std::sync::mpsc::Receiver<Value>,
        RefCell<Thoughts>,
        &'a dyn crate::delegation_events::Sink,
    )>,
}

impl<G> CancellableGenerator<'_, G> {
    /// Shows the thinking that arrived since the last look.
    fn show_thoughts(&self, ended: bool) {
        let Some((lines, thoughts, sink)) = &self.thoughts else {
            return;
        };
        while let Ok(line) = lines.try_recv() {
            for mut event in thoughts.borrow_mut().read(&line) {
                event.redact(self.redaction_keys);
                sink.emit(event);
            }
        }
        if ended {
            for event in thoughts.borrow_mut().end() {
                sink.emit(event);
            }
        }
    }
}

impl<G: Generate> Generate for CancellableGenerator<'_, G> {
    async fn generate(&self, system: &str, prompt: &str) -> microcoder_loop::models::Generated {
        let system = redact_text(system, self.redaction_keys);
        let mut prompt = redact_text(prompt, self.redaction_keys);
        if self.failures.get() > 0 || self.repeated.get() >= 2 {
            prompt.push_str("\n\n# Recovery\n\nThe previous approach did not produce a usable next step or repeated earlier actions. Read the error and command results, choose another approach, and continue toward the task. Return the required JSON action. Do not repeat a completed change just to recover a reply. If the task is complete, return its final answer.");
        }
        let mut generated = tokio::select! {
            generated = async {
                if self.failures.get() > 0 {
                    tokio::time::sleep(Duration::from_millis((self.failures.get() as u64).saturating_mul(250).min(5000))).await;
                }
                self.generator.generate(&system, &prompt).await
            } => generated,
            () = async { while !self.cancel.load(Ordering::Relaxed) { self.show_thoughts(false); tokio::time::sleep(POLL).await; } } => microcoder_loop::models::Generated {
                action: Err("The user canceled this generation.".into()),
                model: String::new(), prompt_tokens: 0, completion_tokens: 0, usd: None, known_usd: 0.0,
                cost_unknown: Some("A generation was canceled after it may have reached the provider.".into()),
                usd_upper: None, cost_basis: microcoder_loop::models::Basis::ListPrice, milliseconds: 0,
            },
        };
        self.show_thoughts(true);
        match &generated.action {
            Ok(action) => {
                self.failures.set(0);
                if !self.bases.borrow().contains(&generated.cost_basis) {
                    self.bases.borrow_mut().push(generated.cost_basis);
                }
                let repeats = !action.commands.is_empty()
                    && *self.previous_commands.borrow() == action.commands;
                self.repeated.set(if repeats {
                    self.repeated.get().saturating_add(1)
                } else {
                    0
                });
                self.previous_commands.replace(action.commands.clone());
            }
            Err(_) => self.failures.set(self.failures.get().saturating_add(1)),
        }
        generated.model = redact_text(&generated.model, self.redaction_keys);
        generated.cost_unknown = generated
            .cost_unknown
            .map(|text| redact_text(&text, self.redaction_keys));
        match &mut generated.action {
            Ok(action) => {
                action.rationale = redact_text(&action.rationale, self.redaction_keys);
                action.reply = redact_text(&action.reply, self.redaction_keys);
                for text in action
                    .commands
                    .iter_mut()
                    .chain(&mut action.view)
                    .chain(&mut action.expand)
                {
                    *text = redact_text(text, self.redaction_keys);
                }
            }
            Err(error) => *error = redact_text(error, self.redaction_keys),
        }
        generated
    }

    fn out_of_capacity(&self) -> Option<microcoder_loop::models::Exhausted> {
        self.generator.out_of_capacity()
    }
}

impl Judge for PluginJudge {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        match &self.0 {
            Some(judge) => judge.judge(set, state).await,
            None => Judgment {
                error: Some("Jev is not configured; no decision was made.".into()),
                ..Judgment::free()
            },
        }
    }
}

fn command_boundary(
    directory: &Path,
    read_only: bool,
) -> Result<Option<coder_boundary::Boundary>, String> {
    if read_only {
        // A read-only task never runs unbounded: where the boundary cannot
        // hold, the task is refused rather than run with write access. It
        // still gets space to work (#11593): a private scratch folder
        // (its TMPDIR, cargo's target and home, the tool caches; see
        // [`scratch_env`]) and the package caches cargo fills, never the
        // project or the person's own files.
        let mut spec = coder_boundary::Boundary::readonly()
            .protecting(directory)
            .owned_scratch_under(std::env::temp_dir());
        for cache in package_caches() {
            let resolved = cache.canonicalize().unwrap_or_else(|_| cache.clone());
            if !resolved.starts_with(directory) && !directory.starts_with(&resolved) {
                spec = spec.writable(resolved);
            }
        }
        return spec.build().map(Some).map_err(|error| {
            format!("Coder could not keep this task from changing files here: {error}")
        });
    }
    if !crate::approval::gated() {
        return Ok(None);
    }
    coder_boundary::Boundary::writing(directory)
        .owned_scratch_under(std::env::temp_dir())
        .build()
        .map(Some)
        .map_err(|error| format!("Coder could not limit the command to this project: {error}"))
}

/// The person's cargo home: `CARGO_HOME`, else `~/.cargo`.
fn cargo_home() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(|| user_home::home_os().map(|home| PathBuf::from(home).join(".cargo")))
}

/// The package caches a read-only task may fill (#11593): cargo's
/// downloaded crates and Git checkouts, never the cargo home itself, which
/// holds its configuration, installed programs and credentials.
fn package_caches() -> Vec<PathBuf> {
    cargo_home()
        .map(|home| vec![home.join("registry"), home.join("git")])
        .unwrap_or_default()
        .into_iter()
        .filter(|path| path.is_dir())
        .collect()
}

/// A read-only task's variables (#11593), each a folder in its private
/// `scratch`, made if missing: cargo builds into `target` there, and its
/// home is `cargo-home` there, whose `registry` and `git` lead to the
/// person's caches and whose configuration is a copy of theirs (never
/// their credentials), so its locks and records land in the scratch; the
/// other tools cache under `cache`.
fn scratch_env(scratch: &Path) -> Vec<(&'static str, PathBuf)> {
    let target = scratch.join("target");
    let home = scratch.join("cargo-home");
    let cache = scratch.join("cache");
    for folder in [&target, &home, &cache] {
        let _ = std::fs::create_dir_all(folder);
    }
    if let Some(real) = cargo_home() {
        for cache in ["registry", "git"] {
            let link = home.join(cache);
            if real.join(cache).is_dir() && std::fs::symlink_metadata(&link).is_err() {
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(real.join(cache), &link);
            }
        }
        for config in ["config.toml", "config"] {
            let copy = home.join(config);
            if real.join(config).is_file() && !copy.exists() {
                let _ = std::fs::copy(real.join(config), copy);
            }
        }
    }
    let mut vars = vec![
        ("CARGO_TARGET_DIR", target),
        ("CARGO_HOME", home),
        ("XDG_CACHE_HOME", cache.clone()),
        ("GOCACHE", cache.join("go-build")),
        ("npm_config_cache", cache.join("npm")),
        ("PIP_CACHE_DIR", cache.join("pip")),
    ];
    // rustup finds its toolchains without the person's cargo home.
    if let Some(rustup) = std::env::var_os("RUSTUP_HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(|| user_home::home_os().map(|home| PathBuf::from(home).join(".rustup")))
    {
        vars.push(("RUSTUP_HOME", rustup));
    }
    vars
}

struct Checkout<'a> {
    /// The session's Coder SDK hooks, fired by the loop (#11497): the
    /// floors among them.
    hooks: Option<coder_sdk::Hooks>,
    directory: PathBuf,
    boundary: Option<coder_boundary::Boundary>,
    cancel: Arc<AtomicBool>,
    redaction_keys: &'a [ApiKey],
    events: Option<&'a dyn crate::delegation_events::Sink>,
    /// The command the agent asked to run, as its `PreToolUse` carried it:
    /// when a hook rewrote what runs (the squeezer, #11520), the result and
    /// the progress still name the agent's own command, so they stay on its
    /// row and in its words.
    asked: std::sync::Mutex<Option<String>>,
}

impl Env for Checkout<'_> {
    async fn run(&self, script: &str, deadline: Duration) -> CommandResult {
        let started = Instant::now();
        let shown = self
            .asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .unwrap_or_else(|| script.to_owned());
        // The floors are a hook the loop fires before this (#11497).
        // An agent-driven chat asks before anything that is not read-only.
        if let crate::approval::Verdict::Refused(why) = crate::approval::check(script) {
            return CommandResult {
                command: redact_text(script, self.redaction_keys),
                exit: None,
                timed_out: false,
                seconds: started.elapsed().as_secs_f64(),
                output: why,
            };
        }
        let result = async {
            #[cfg(unix)]
            let shell = PathBuf::from("/bin/sh");
            #[cfg(windows)]
            let shell = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .filter(|root| root.is_absolute())
                .ok_or_else(|| "Cannot locate the Windows system directory.".to_string())?
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            #[cfg(windows)]
            let arguments = ["-NoProfile", "-NonInteractive", "-Command", script];
            #[cfg(unix)]
            let streamed_script = format!("exec 2>&1\n{script}");
            #[cfg(unix)]
            let arguments = ["-c", streamed_script.as_str()];
            let mut command = match &self.boundary {
                Some(boundary) => boundary
                    .command(shell, arguments)
                    .map_err(|error| error.to_string())?,
                None => {
                    let mut command = std::process::Command::new(shell);
                    command.args(arguments);
                    command
                }
            };
            command.current_dir(&self.directory);
            if let Some(boundary) = &self.boundary
                && let Some(scratch) = boundary.scratch()
            {
                command.env("TMPDIR", scratch);
                // A read-only task builds and caches in its scratch
                // (#11593).
                if boundary.checkout().is_none() {
                    for (name, value) in scratch_env(scratch) {
                        command.env(name, value);
                    }
                }
            }
            scrub_credentials(&mut command);
            let mut bridge = self
                .events
                .map(|_| crate::delegation_events::Bridge::new())
                .transpose()?;
            if let Some(bridge) = &bridge {
                bridge.prepare(&mut command);
            }
            wait_job(
                supervise::Job::from_command(command)
                    .bounded(supervise::Limits::within(deadline).keeping(TEXT_MAX)),
                &self.cancel,
                bridge.as_mut().zip(self.events),
                self.redaction_keys,
                self.events.map(|sink| (sink, shown.as_str())),
            )
            .await
        }
        .await;
        let (output, exit, timed_out) = match result {
            Ok(stopped) => {
                let mut text = String::from_utf8_lossy(&stopped.rest.bytes).into_owned();
                if !stopped.stderr.text.is_empty() {
                    text.push('\n');
                    text.push_str(&stopped.stderr.marked());
                }
                if !stopped.group_clear {
                    text.push('\n');
                    text.push_str(still_running(false).trim_start());
                }
                (
                    text,
                    stopped.ending.code(),
                    stopped.requested || matches!(stopped.ending, supervise::Ending::TimedOut),
                )
            }
            Err(error) => (error, None, false),
        };
        CommandResult {
            command: redact_text(&shown, self.redaction_keys),
            exit,
            timed_out,
            seconds: started.elapsed().as_secs_f64(),
            output: bounded(&redact_text(&output, self.redaction_keys), TEXT_MAX),
        }
    }

    async fn read(&self, path: &str) -> Option<String> {
        if !crate::approval::tools_allowed() {
            return None;
        }
        let path = self.directory.join(path).canonicalize().ok()?;
        if self.boundary.is_some() && !path.starts_with(&self.directory) {
            return None;
        }
        let file = std::fs::File::open(path).ok()?;
        use std::io::Read;
        let mut bytes = Vec::new();
        file.take(TEXT_MAX as u64).read_to_end(&mut bytes).ok()?;
        Some(redact_text(
            &String::from_utf8_lossy(&bytes),
            self.redaction_keys,
        ))
    }

    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    async fn hook(&self, call: coder_sdk::Call) -> coder_sdk::Applied {
        if call.event == coder_sdk::Event::PreToolUse
            && let Some(command) = call.input["command"].as_str()
        {
            *self
                .asked
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(command.to_owned());
        }
        match &self.hooks {
            Some(hooks) => hooks.fire(call).await,
            None => coder_sdk::Applied::unchanged(&call),
        }
    }
}

struct MicrocoderEvents<'a> {
    emit: &'a dyn crate::delegation_events::Sink,
    reply: String,
    model: Option<String>,
    tokens: u64,
    redaction_keys: &'a [ApiKey],
}

impl Observer for MicrocoderEvents<'_> {
    fn event(&mut self, _seconds: f64, event: &Event) {
        match event {
            Event::Generated { generated, .. } => {
                if !generated.model.is_empty() {
                    let model = redact_text(&generated.model, self.redaction_keys);
                    self.model = Some(model.clone());
                    self.emit.emit(RuntimeEvent::Model(model));
                }
                self.tokens = self
                    .tokens
                    .saturating_add(generated.prompt_tokens)
                    .saturating_add(generated.completion_tokens);
                self.emit.emit(RuntimeEvent::Tokens(self.tokens));
                // Each call's own report, so the rail's context figure
                // reads what the model was really sent (#11537). Its cost
                // is the turn's to report.
                if generated.prompt_tokens > 0 {
                    self.emit.emit(RuntimeEvent::Usage(Box::new(RoundUsage {
                        fell_back: None,
                        model: self.model.clone().unwrap_or_default(),
                        prompt_tokens: generated.prompt_tokens,
                        completion_tokens: generated.completion_tokens,
                        reasoning_tokens: None,
                        cost: None,
                        milliseconds: generated.milliseconds,
                        first_token_ms: None,
                        cached_tokens: None,
                        cache_write_tokens: None,
                    })));
                }
                if let Ok(action) = &generated.action {
                    if !action.reply.is_empty() {
                        self.reply =
                            bounded(&redact_text(&action.reply, self.redaction_keys), TEXT_MAX);
                    }
                    for command in &action.commands {
                        self.emit.emit(RuntimeEvent::Tool {
                            name: "Run".into(),
                            input: json!({"command":redact_text(command, self.redaction_keys)}),
                            output: Value::Null,
                            running: true,
                        });
                    }
                    if action.finished && !self.reply.is_empty() {
                        self.emit.emit(RuntimeEvent::Text(self.reply.clone()));
                    }
                }
            }
            Event::Ran { result, .. } => self.emit.emit(RuntimeEvent::Tool {
                name: "Run".into(),
                input: json!({"command":redact_text(&result.command, self.redaction_keys)}),
                output: json!(result),
                running: false,
            }),
            // Jev's estimate, and only Jev's: a judgment that failed or
            // asked no `complete` score shows the step alone.
            Event::Judged { step, judgment } => self.emit.emit(RuntimeEvent::Progress {
                step: *step,
                complete: judgment
                    .error
                    .is_none()
                    .then(|| judgment.complete())
                    .flatten()
                    .filter(|complete| complete.is_finite()),
            }),
            _ => {}
        }
    }
}

// Withhold a suffix that could be the beginning of a credential.
fn redact_stream(bytes: &[u8], keys: &[ApiKey]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut end = text.len();
    for key in keys {
        for (length, _) in key.expose().char_indices().skip(1) {
            if text.ends_with(&key.expose()[..length]) {
                end = end.min(text.len() - length);
            }
        }
    }
    redact_text(&text[..end], keys)
}

fn redact_text(text: &str, keys: &[ApiKey]) -> String {
    let mut text = text.to_owned();
    for key in keys {
        if !key.expose().is_empty() {
            text = text.replace(key.expose(), "[redacted]");
        }
    }
    text
}

fn bounded(text: &str, bytes: usize) -> String {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].into()
}

/// What a person reads after a stopped command or agent: nothing when all
/// of its processes ended, a sentence when some may still be running.
pub(crate) fn still_running(group_clear: bool) -> &'static str {
    if group_clear {
        ""
    } else {
        " Some of its processes may still be running."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #11591: Coder reads openagents.com projects through the CLI tool.
    /// Against a stand-in CLI with an owned and a shared project, the
    /// tool hands the model each project's role and sharing, who started
    /// each chat in the shared one, and a viewer's refusal in the
    /// website's own words; the tool's description says how to use them.
    #[tokio::test]
    #[cfg(unix)]
    async fn shared_projects_reach_the_model_with_roles_starters_and_refusals() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("openagents");
        std::fs::write(
            &program,
            r#"#!/bin/sh
case "$*" in
"--json projects list")
  printf '%s\n' '{"data":[{"id":"prj_0123456789abcdef","name":"openagents","role":"owner","members":1,"shared":false},{"id":"prj_aaaaaaaaaaaaaaaa","name":"Garden","role":"viewer","members":3,"shared":true}]}' ;;
"--json projects show Garden")
  printf '%s\n' '{"project":{"id":"prj_aaaaaaaaaaaaaaaa","name":"Garden","role":"viewer","shared":true},"threads":[{"id":"t1","title":"Tomatoes","started_by":{"name":"Ada","you":false}},{"id":"t2","title":"Watering","started_by":{"name":"You","you":true}}]}' ;;
"--json chat send Hello --project Garden")
  echo "You can read this project's chats. Ask its owner to make you an editor to send messages." >&2
  exit 1 ;;
*) exit 2 ;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let run = |words: &[&str]| {
            let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
            let program = program.clone();
            let cwd = dir.path().to_owned();
            let cancel = cancel.clone();
            async move { cli_checked(&program, &words, &cwd, &cancel, &mut |_| {}).await }
        };
        let listed = run(&["projects", "list"]).await.unwrap();
        let listed: Value = serde_json::from_str(listed["stdout"].as_str().unwrap()).unwrap();
        assert_eq!(listed["data"][0]["shared"], json!(false));
        assert_eq!(listed["data"][1]["role"], json!("viewer"));
        assert_eq!(listed["data"][1]["shared"], json!(true));
        let shown = run(&["projects", "show", "Garden"]).await.unwrap();
        let shown: Value = serde_json::from_str(shown["stdout"].as_str().unwrap()).unwrap();
        assert_eq!(shown["threads"][0]["started_by"]["name"], json!("Ada"));
        assert_eq!(shown["threads"][1]["started_by"]["you"], json!(true));
        let refused = run(&["chat", "send", "Hello", "--project", "Garden"])
            .await
            .unwrap();
        assert_eq!(refused["exit"], json!(1));
        assert!(
            refused["stderr"]
                .to_string()
                .contains("Ask its owner to make you an editor"),
            "{refused}"
        );
        let description = cli_tool_definition()["function"]["description"]
            .as_str()
            .unwrap()
            .to_owned();
        for words in [
            "role (owner, editor, or viewer)",
            "started_by",
            "A viewer reads a shared project and sends nothing",
            "tell the person its reason as it gave it",
            r#"[\"projects\",\"join\",LINK]"#,
        ] {
            assert!(
                description.contains(&words.replace("\\\"", "\"")),
                "{words}"
            );
        }
    }

    /// #11537: the Claude Code login's thinking shows as Claude Code's
    /// own does: `Thinking` while a block streams, then the block as a
    /// thinking entry; a signature delta keeps it on, the call's end stops
    /// it.
    #[test]
    fn the_claude_logins_thinking_shows_as_claude_codes_does() {
        let mut thoughts = Thoughts::default();
        let event = |event: Value| json!({"type": "stream_event", "event": event});
        let shown = thoughts.read(&event(json!({"type": "content_block_start",
            "content_block": {"type": "thinking", "thinking": ""}})));
        assert!(matches!(shown.as_slice(), [RuntimeEvent::Thinking(true)]));
        // #11552: the words stream as they arrive.
        let shown = thoughts.read(&event(json!({"type": "content_block_delta",
            "delta": {"type": "thinking_delta", "thinking": "Look"}})));
        assert!(matches!(shown.as_slice(), [RuntimeEvent::ThinkingText(text)] if text == "Look"));
        assert!(
            thoughts
                .read(&event(json!({"type": "content_block_delta",
            "delta": {"type": "signature_delta", "signature": "s"}})))
                .is_empty()
        );
        let shown = thoughts.read(&json!({"type": "assistant", "message": {"content": [
            {"type": "thinking", "thinking": "Look first."}]}}));
        match shown.as_slice() {
            [RuntimeEvent::Tool { name, output, .. }] => {
                assert_eq!(name, crate::claude_stream::THINKING);
                assert_eq!(output["text"], "Look first.");
            }
            other => panic!("a thinking entry, not {other:?}"),
        }
        let shown = thoughts.read(&event(json!({"type": "content_block_start",
            "content_block": {"type": "tool_use", "name": "StructuredOutput"}})));
        assert!(matches!(shown.as_slice(), [RuntimeEvent::Thinking(false)]));
        assert!(thoughts.end().is_empty());
        thoughts.read(&event(json!({"type": "content_block_start",
            "content_block": {"type": "thinking"}})));
        thoughts.read(&event(json!({"type": "content_block_delta",
            "delta": {"type": "thinking_delta", "thinking": "Cut off"}})));
        // Thinking still streaming when the call ends keeps its words.
        match thoughts.end().as_slice() {
            [
                RuntimeEvent::Tool {
                    output,
                    running: false,
                    ..
                },
                RuntimeEvent::Thinking(false),
            ] => {
                assert_eq!(output["text"], "Cut off");
            }
            other => panic!("the closed entry and the rail, not {other:?}"),
        }
    }
    use crate::delegation_events::Sink;

    /// #11538: every child gets the `openagents` beside Coder first on
    /// `PATH`, ahead of an older one installed elsewhere, whatever else
    /// its thread sets; with none beside Coder, nothing changes.
    #[test]
    fn every_child_finds_the_openagents_beside_coder_first() {
        use std::ffi::OsString;
        let cli = Path::new("/opt/coder");
        let vars = with_own_cli(
            vec![
                ("PATH".into(), "/old/bin:/bin".into()),
                ("CARGO_TARGET_DIR".into(), "/t".into()),
            ],
            Some(cli),
        );
        assert_eq!(
            vars,
            vec![
                (OsString::from("CARGO_TARGET_DIR"), OsString::from("/t")),
                (
                    OsString::from("PATH"),
                    // This platform's separator before the old PATH (#11695).
                    OsString::from(format!(
                        "/opt/coder{}/old/bin:/bin",
                        if cfg!(windows) { ';' } else { ':' }
                    ))
                ),
            ]
        );
        // Applied twice: one copy, still first.
        assert_eq!(with_own_cli(vars.clone(), Some(cli)), vars);
        // No PATH of the thread's own: this process's, led by the CLI.
        let fronted = with_own_cli(Vec::new(), Some(cli));
        let path = &fronted.iter().find(|(name, _)| name == "PATH").unwrap().1;
        assert_eq!(std::env::split_paths(path).next().as_deref(), Some(cli));
        let plain = vec![(OsString::from("A"), OsString::from("1"))];
        assert_eq!(with_own_cli(plain.clone(), None), plain);
    }

    /// #11599: with an old fake `openagents` first on a scratch `PATH` and
    /// a new one beside a test Coder (reached through a link), the shell
    /// Run starts resolves the new one.
    #[cfg(unix)]
    #[test]
    fn run_resolves_the_openagents_beside_coder_over_an_older_one_on_path() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(scratch.path()).unwrap();
        let fake = |dir: &Path, version: &str| {
            std::fs::create_dir_all(dir).unwrap();
            let program = dir.join("openagents");
            std::fs::write(&program, format!("#!/bin/sh\necho openagents {version}\n")).unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        let old = root.join("old-bin");
        fake(&old, "1.0.0-rc.5");
        let release = root.join("release");
        fake(&release, "1.0.0");
        std::fs::write(release.join("coder"), "").unwrap();
        let links = root.join("links");
        std::fs::create_dir_all(&links).unwrap();
        std::os::unix::fs::symlink(release.join("coder"), links.join("coder")).unwrap();
        let path = format!("{}:/bin:/usr/bin", old.display());
        let run = |cli: Option<&Path>| {
            let mut command = std::process::Command::new("/bin/sh");
            command.args(["-c", "openagents --version"]).env_clear();
            command.envs(with_own_cli(
                vec![("PATH".into(), path.clone().into())],
                cli,
            ));
            String::from_utf8(command.output().unwrap().stdout).unwrap()
        };
        assert_eq!(run(None), "openagents 1.0.0-rc.5\n");
        let cli = coder_lease::shim::cli_dir_beside(&links.join("coder"));
        assert_eq!(cli.as_deref(), Some(release.as_path()));
        assert_eq!(run(cli.as_deref()), "openagents 1.0.0\n");
    }

    #[tokio::test]
    async fn microcoder_streaming_completion_keeps_one_successful_command() {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.live.busy = true;
        let id = app.request_id;
        let mut callback = |event| {
            if let RuntimeEvent::Tool {
                name,
                input,
                output,
                running,
            } = event
            {
                app.apply_update(crate::live::Update::Tool {
                    id,
                    name,
                    input,
                    output,
                    running,
                    at: None,
                });
            }
        };
        let sink = RefCell::new(&mut callback);
        let command = "printf streaming-ok";
        sink.emit(RuntimeEvent::Tool {
            name: "Run".into(),
            input: json!({"command":command}),
            output: Value::Null,
            running: true,
        });
        let result = run_command(
            command,
            root.path(),
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |event| sink.emit(event),
        )
        .await
        .unwrap();
        let mut observer = MicrocoderEvents {
            emit: &sink,
            reply: String::new(),
            model: None,
            tokens: 0,
            redaction_keys: &[],
        };
        observer.event(
            0.0,
            &Event::Ran {
                step: 1,
                result: CommandResult {
                    command: command.into(),
                    exit: Some(0),
                    timed_out: false,
                    seconds: 0.0,
                    output: result["output"].as_str().unwrap().into(),
                },
            },
        );
        drop(observer);
        drop(sink);
        app.apply_update(crate::live::Update::Finished {
            id,
            result: Ok(openrouter::Streamed {
                text: "The test passed.".into(),
                ..Default::default()
            }),
        });
        app.cancel_request();
        let tools = app
            .live
            .entries
            .iter()
            .filter(|entry| matches!(entry, crate::live::Entry::Tool { .. }))
            .collect::<Vec<_>>();
        assert_eq!(tools.len(), 1);
        assert!(
            matches!(tools[0], crate::live::Entry::Tool { running: false, output, .. } if output["output"] == "streaming-ok" && output.get("error").is_none())
        );
        assert!(app.live.entries.iter().any(|entry| matches!(entry, crate::live::Entry::Assistant { text, .. } if text == "The test passed.")));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn silent_run_reports_elapsed_and_silence() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let mut updates = Vec::new();
        let result = run_command(
            "# Waiting for fixture\nsleep 1.2",
            dir.path(),
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |event| {
                if let RuntimeEvent::Tool {
                    output,
                    running: true,
                    ..
                } = event
                {
                    updates.push(output);
                }
            },
        )
        .await
        .unwrap();
        assert!(
            updates
                .iter()
                .any(|output| output["elapsed_seconds"] == 1 && output["silent_seconds"] == 1)
        );
        assert_eq!(updates[0]["activity"], "Waiting for fixture");
        assert!(result["seconds"].as_f64().unwrap() >= 1.0);
        assert_eq!(result["exit"], 0);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn run_streams_both_pipes_before_exit_and_retains_final_output() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let mut updates = Vec::new();
        let result = run_command(
            "printf early; printf error >&2; sleep 0.3; printf late",
            dir.path(),
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |event| {
                if let RuntimeEvent::Tool {
                    output,
                    running: true,
                    ..
                } = event
                {
                    updates.push((started.elapsed(), output));
                }
            },
        )
        .await
        .unwrap();
        assert!(
            updates
                .iter()
                .any(|(elapsed, output)| elapsed.as_millis() < 250
                    && output["output"].as_str().unwrap().contains("earlyerror"))
        );
        assert_eq!(result["output"], "earlyerrorlate");
        assert_eq!(result["exit"], 0);
        let keys = [ApiKey::new("secret")];
        assert_eq!(redact_stream(b"hello sec", &keys), "hello ");
        assert_eq!(redact_stream(b"hello secret", &keys), "hello [redacted]");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn default_commands_can_write_and_read_outside_the_working_directory() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().join("checkout");
        std::fs::create_dir(&cwd).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let result = run_command(
            "printf 'full access' > ../result",
            &cwd,
            &[],
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result["exit"], 0, "{result}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("result")).unwrap(),
            "full access"
        );
        let environment = Checkout {
            asked: std::sync::Mutex::default(),
            hooks: None,
            directory: cwd.clone(),
            boundary: command_boundary(&cwd, false).unwrap(),
            cancel,
            redaction_keys: &[],
            events: None,
        };
        assert_eq!(
            environment.read("../result").await.as_deref(),
            Some("full access")
        );
        assert!(
            crate::plugins::Plugins::default()
                .execution_settings(cwd)
                .shell
        );
    }

    #[test]
    fn an_explicit_approval_gate_still_refuses_acp_permissions() {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let desk = crate::approval::Desk::new();
        crate::approval::install(Some(crate::approval::Gate {
            desk: Arc::clone(&desk),
            cancel: Arc::new(AtomicBool::new(false)),
        }));
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                crate::approval::install(None);
            }
        }
        let _reset = Reset;
        let request = serde_json::from_value(json!({
            "options": [
                {"optionId":"allow","name":"Allow","kind":"allow_once"},
                {"optionId":"deny","name":"Deny","kind":"reject_once"}
            ]
        }))
        .unwrap();
        let mut emit = |_| {};
        let mut handler = AcpEvents {
            thought: crate::claude_stream::Thought::default(),
            read_only: false,
            emit: &mut emit,
            text: String::new(),
            tools: BTreeMap::new(),
            hooks: None,
            cwd: PathBuf::new(),
            seen: BTreeMap::new(),
            refusals: Arc::default(),
            last_thought: LastThought::default(),
        };
        assert_eq!(
            handler.permission(&request),
            acp_client::wire::PermissionAnswer::Selected("deny".into())
        );
        desk.close();
        assert_eq!(codex_sandbox().unwrap(), CodexSandbox::ReadOnly);
        assert!(desk.drain().is_empty());
    }

    /// #11552: an ACP agent's thought chunks (Grok Build 1.0.52's
    /// `agent_thought_chunk`, recorded 2026-10-13 for "Is 9991 prime?")
    /// stream into one thinking entry, closed when the answer starts, the
    /// answer below it.
    #[test]
    fn an_acp_agents_thoughts_stream_into_a_thinking_entry() {
        let mut events = Vec::new();
        let mut emit = |event| events.push(event);
        let mut handler = AcpEvents {
            thought: crate::claude_stream::Thought::default(),
            read_only: false,
            emit: &mut emit,
            text: String::new(),
            tools: BTreeMap::new(),
            hooks: None,
            cwd: PathBuf::new(),
            seen: BTreeMap::new(),
            refusals: Arc::default(),
            last_thought: LastThought::default(),
        };
        for line in crate::claude_stream::recorded("grok-acp.jsonl") {
            if let Some((_, update)) = acp_client::wire::parse_update(&line["params"]) {
                handler.update(update);
            }
        }
        handler.end_thought();
        let reply = handler.text.clone();
        let (chat, pieces) = crate::claude_stream::shown_in_chat(&events);
        assert_eq!(pieces.len(), 5, "{pieces:?}");
        let entries = crate::claude_stream::thinking_entries(&chat);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert!(
            entries[0].0.starts_with("Checking whether 9991 is prime."),
            "{entries:?}"
        );
        assert!(!entries[0].1);
        assert!(
            matches!(chat.entries.last(), Some(crate::live::Entry::Assistant { text, .. }) if *text == reply.trim())
        );
        // The rail's `Thinking` came on with the first piece and went off.
        assert!(matches!(events.first(), Some(RuntimeEvent::Thinking(true))));
        assert!(!chat.thinking);
    }

    #[tokio::test]
    async fn crew_tool_free_scope_refuses_native_fallback_reads_and_commands() {
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().canonicalize().unwrap();
        std::fs::write(directory.join("private.txt"), "private fixture").unwrap();
        let Ok(boundary) = coder_boundary::Boundary::writing(&directory).build() else {
            return;
        };
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        crate::approval::install(Some(crate::approval::Gate {
            desk: crate::approval::Desk::tool_free(),
            cancel: Arc::new(AtomicBool::new(false)),
        }));
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                crate::approval::install(None);
            }
        }
        let _reset = Reset;
        let environment = Checkout {
            asked: std::sync::Mutex::default(),
            hooks: None,
            directory,
            boundary: Some(boundary),
            cancel: Arc::new(AtomicBool::new(false)),
            redaction_keys: &[],
            events: None,
        };
        assert_eq!(environment.read("private.txt").await, None);
        for command in ["cat private.txt", "touch changed"] {
            let result = environment.run(command, Duration::from_secs(1)).await;
            assert_eq!(result.exit, None);
            assert!(result.output.contains("crew charter"));
        }
        assert!(!dir.path().join("changed").exists());
    }

    #[test]
    fn a_gated_chat_keeps_codex_read_only_and_asks_nobody() {
        assert_eq!(sandbox_for(true, false), CodexSandbox::FullAccess);
        assert_eq!(sandbox_for(true, true), CodexSandbox::ReadOnly);
        assert_eq!(sandbox_for(false, false), CodexSandbox::ReadOnly);
        assert_eq!(sandbox_for(false, true), CodexSandbox::ReadOnly);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn crew_tool_free_scope_refuses_direct_acp_and_codex_without_asking() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("codex");
        let marker = dir.path().join("spawned");
        std::fs::write(&program, "#!/bin/sh\ntouch spawned\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let desk = crate::approval::Desk::tool_free();
        let cancel = Arc::new(AtomicBool::new(false));
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        crate::approval::install(Some(crate::approval::Gate {
            desk: Arc::clone(&desk),
            cancel: Arc::clone(&cancel),
        }));
        struct Reset(bool);
        impl Drop for Reset {
            fn drop(&mut self) {
                allow_codex_writes(self.0);
                crate::approval::install(None);
            }
        }
        let _reset = Reset(CODEX_WRITES.swap(true, Ordering::SeqCst));
        // A write setting cannot convert a tool-free refusal into an approval
        // question or a read-only child. Both native transports stop first.
        assert!(codex_sandbox().unwrap_err().contains("crew charter"));
        let mut events = vec![];
        for transport in [AgentTransport::Acp, AgentTransport::CodexCli] {
            let mut native = agent(program.clone());
            native.id = "codex".into();
            native.transport = transport;
            let error = acp(
                &native,
                "Draft supplied facts only.",
                dir.path(),
                None,
                &cancel,
                &mut |event| events.push(event),
            )
            .await
            .unwrap_err();
            assert!(error.contains("crew charter"));
        }
        for sandbox in [
            CodexSandbox::FullAccess,
            CodexSandbox::ReadOnly,
            CodexSandbox::WorkspaceWrite,
        ] {
            assert!(
                codex_cli(
                    &program,
                    "Draft supplied facts only.",
                    dir.path(),
                    sandbox,
                    false,
                    &cancel,
                    &mut |event| events.push(event),
                )
                .await
                .unwrap_err()
                .contains("crew charter")
            );
        }
        assert!(desk.drain().is_empty());
        assert!(events.is_empty());
        assert!(!marker.exists());
    }

    fn agent(program: PathBuf) -> AcpAgent {
        AcpAgent {
            id: "reviewer".into(),
            name: "Reviewer".into(),
            program,
            transport: AgentTransport::Acp,
            arguments: vec![],
            mode: None,
            enabled: true,
        }
    }

    #[test]
    fn legacy_agents_default_to_acp_and_native_codex_rejects_other_ids() {
        let legacy: AcpAgent =
            serde_json::from_value(json!({"id":"reviewer","name":"Reviewer","program":"reviewer"}))
                .unwrap();
        assert_eq!(legacy.transport, AgentTransport::Acp);
        let mut native = legacy;
        native.transport = AgentTransport::CodexCli;
        assert!(native.validate().is_err());
        native.id = "codex".into();
        assert!(native.validate().is_ok());
        native.arguments = vec!["--dangerously-bypass-approvals-and-sandbox".into()];
        assert!(native.validate().is_err());
        native.arguments.clear();
        native.mode = Some("bypass".into());
        assert!(native.validate().is_err());
    }

    /// Codex launches as the permission table says, with the floors' hook
    /// (#11475): a drift in either fails here.
    #[tokio::test]
    #[cfg(unix)]
    async fn codex_launches_as_the_permission_table_says_with_the_floors_hook() {
        use std::os::unix::fs::PermissionsExt;
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("codex");
        std::fs::write(
            &program,
            r#"#!/bin/sh
printf '%s\n' "$@" > args
cat > /dev/null
printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"ok"}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut native = agent(program);
        native.id = "codex".into();
        native.name = "Codex".into();
        native.transport = AgentTransport::CodexCli;
        let _ = acp(
            &native,
            "Fix the test.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await;
        let args: Vec<String> = std::fs::read_to_string(dir.path().join("args"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        // A stand-in that is no app-server: the exec fallback ran.
        let row = &crate::floors::CODEX_EXEC_FALLBACK;
        assert_eq!(row.permission, crate::floors::PERMISSION);
        for flag in row.native {
            assert!(args.iter().any(|arg| arg == flag), "{flag} in {args:?}");
        }
        let this = std::env::current_exe().unwrap();
        for hook in crate::floors::codex_args(&this) {
            assert!(args.contains(&hook), "{hook} in {args:?}");
        }
        assert!(
            args.iter().any(|arg| arg.starts_with("hooks.PreToolUse=")
                && arg.contains(crate::floors::HOOK_COMMAND))
        );
    }

    /// Codex reads the chat's briefing ahead of its task, with its own
    /// `AGENTS.md` reading off (#11473); without one it reads its own.
    #[tokio::test]
    #[cfg(unix)]
    async fn codex_reads_the_briefing_ahead_of_the_task() {
        use std::os::unix::fs::PermissionsExt;
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("codex");
        std::fs::write(
            &program,
            r#"#!/bin/sh
printf '%s\n' "$@" > args
cat > task
printf '%s\n' '{"type":"thread.started","thread_id":"scratch-codex","model":"gpt-test"}'
printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"ok"}}'
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut native = agent(program);
        native.id = "codex".into();
        native.name = "Codex".into();
        native.transport = AgentTransport::CodexCli;
        let briefing = "# Project instructions\n\n### Testing\n\nRun the tests.";
        acp_briefed(
            &native,
            "Fix the test.",
            dir.path(),
            None,
            false,
            Some(briefing),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("task")).unwrap(),
            briefed_task(briefing, "Fix the test.")
        );
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.contains("project_doc_max_bytes=0\n"), "{args}");
        acp(
            &native,
            "Fix the test.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("task")).unwrap(),
            "Fix the test."
        );
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(!args.contains("project_doc_max_bytes"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn native_codex_starts_with_full_permissions_and_streams_its_protocol() {
        use std::os::unix::fs::PermissionsExt;
        // The sandbox follows the process-wide approval gate and write
        // setting, which other tests change while they hold this lock.
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("codex");
        std::fs::write(&program, r#"#!/bin/sh
printf '%s\n' "$@" > args
cat > task
printf '%s' "$CODEX_INTERNAL_ORIGINATOR_OVERRIDE" > caller
printf '%s\n' '{"type":"thread.started","thread_id":"scratch-codex","model":"gpt-test"}'
printf '%s\n' '{"type":"item.started","item":{"type":"command_execution","command":"pwd"}}'
printf '%s\n' '{"type":"item.completed","item":{"type":"command_execution","command":"pwd","aggregated_output":"scratch","exit_code":0}}'
printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"Codex answered."}}'
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"output_tokens":7}}'
"#).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut native = agent(program);
        native.id = "codex".into();
        native.name = "Codex".into();
        native.transport = AgentTransport::CodexCli;
        let mut events = vec![];
        let result = acp(
            &native,
            "Review this scratch task.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(result["session"], "scratch-codex");
        assert_eq!(result["reply"], "Codex answered.");
        assert_eq!(result["tokens"], 18);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RuntimeEvent::Tokens(18)))
        );
        assert_eq!(result["model"], "gpt-test");
        assert_eq!(result["transport"], "codex-cli");
        assert_eq!(result["group_clear"], true);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("task")).unwrap(),
            "Review this scratch task."
        );
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.starts_with("exec\n--json\n"));
        assert_eq!(result["sandbox"], "danger-full-access");
        assert!(args.contains("--dangerously-bypass-approvals-and-sandbox\n"));
        assert!(args.contains("approval_policy=\"never\""));
        assert!(!args.contains("--sandbox\n"));
        assert!(!args.contains("-m\n"));
        assert!(events.iter().any(
            |event| matches!(event, RuntimeEvent::Tool { name, running: true, .. } if name == "Run")
        ));
        assert!(
            events.iter().any(
                |event| matches!(event, RuntimeEvent::Text(text) if text == "Codex answered.")
            )
        );
        let command = events
            .iter()
            .position(|event| matches!(event, RuntimeEvent::Tool { running: true, .. }))
            .unwrap();
        let reply = events
            .iter()
            .position(|event| matches!(event, RuntimeEvent::Text(_)))
            .unwrap();
        assert!(command < reply);
        let (_, mark) = coder_delegate::delegate::Agent::Codex.engine_mark();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("caller")).unwrap(),
            mark
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_writing_codex_delegation_uses_the_workspace_sandbox_and_names_a_limit() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("codex");
        std::fs::write(
            &program,
            r#"#!/bin/sh
printf '%s\n' "$@" > args
cat > task
printf '%s\n' '{"type":"thread.started","thread_id":"scratch-codex","model":"gpt-test"}'
printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"Edited."}}'
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":5,"output_tokens":2}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let result = codex_cli(
            &program,
            "Fix it.",
            dir.path(),
            CodexSandbox::WorkspaceWrite,
            false,
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result["sandbox"], "workspace-write");
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.contains("--sandbox\nworkspace-write\n"));
        assert!(!args.contains("--dangerously-bypass-approvals"));

        std::fs::write(
            &program,
            "#!/bin/sh\ncat > /dev/null\necho 'You have hit your usage limit. Try again later.' >&2\nexit 1\n",
        )
        .unwrap();
        let error = codex_cli(
            &program,
            "Fix it.",
            dir.path(),
            CodexSandbox::WorkspaceWrite,
            false,
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(
            error.starts_with("Codex is out of capacity (a usage limit, "),
            "{error}"
        );
        assert!(error.contains("No other engine ran this task"), "{error}");
        let refusal = microcoder_loop::capacity::Refusal::carried(&error).unwrap();
        assert_eq!(refusal.provider, microcoder_loop::capacity::Provider::Codex);
        assert_eq!(refusal.kind, microcoder_loop::capacity::Kind::UsageLimit);
    }

    #[test]
    fn a_codex_limit_carries_its_kind_and_reset_in_microcoders_shape() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // 2026-10-12T21:00:00Z, the demo's run.
        let now = 1_791_838_800;
        let said =
            "You've hit your usage limit. Upgrade to Pro or try again at Oct 17th, 2026 10:58 PM.";
        let refusal = codex_refusal(said, now);
        assert_eq!(refusal.kind, microcoder_loop::capacity::Kind::UsageLimit);
        assert_eq!(
            refusal
                .resets_at
                .map(microcoder_loop::capacity::utc)
                .as_deref(),
            Some("2026-10-17 22:58 UTC")
        );
        let error = codex_out_of_capacity(&refusal, said, true);
        assert!(error.starts_with(
            "Codex is out of capacity (a usage limit, until 2026-10-17 22:58 UTC), so continue without Codex."
        ), "{error}");
        assert_eq!(
            microcoder_loop::capacity::Refusal::carried(&error),
            Some(refusal)
        );
        // Codex's typed body keeps its plan, as Microcoder's record does.
        let body = r#"{"error":{"type":"usage_limit_reached","message":"The usage limit has been reached","plan_type":"pro","resets_at":1791050823,"eligible_promo":null,"limit_window_minutes":10080,"resets_in_seconds":478613}}"#;
        assert_eq!(codex_refusal(body, now).plan.as_deref(), Some("pro"));
    }

    #[test]
    fn codex_failure_reports_the_ending_instead_of_old_mcp_logs() {
        let logs = "ERROR rmcp: HTTP 401 unauthorized: bearer token required";
        assert_eq!(
            codex_failure_reason(&supervise::Ending::TimedOut, None, logs),
            "The Codex task timed out."
        );
        assert_eq!(
            codex_failure_reason(&supervise::Ending::Exited(None), None, logs),
            "Codex was terminated by a signal."
        );
        assert_eq!(
            codex_failure_reason(
                &supervise::Ending::Exited(Some(1)),
                Some("Codex needs a login."),
                logs
            ),
            "Codex needs a login."
        );
        assert!(
            codex_failure_reason(
                &supervise::Ending::Exited(Some(1)),
                Some(&"x".repeat(10_000)),
                logs
            )
            .len()
                <= 512
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn native_codex_failure_is_reported_without_starting_another_engine() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("codex");
        std::fs::write(&program, "#!/bin/sh\ncat > task\nprintf '%s\\n' '{\"type\":\"turn.failed\",\"error\":{\"message\":\"Codex needs a login.\"}}'\nexit 1\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut native = agent(program);
        native.id = "codex".into();
        native.transport = AgentTransport::CodexCli;
        let error = acp(
            &native,
            "Review scratch.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(error.contains("Codex needs a login."));
        assert!(!error.contains("still be running"), "{error}");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn canceling_native_codex_cleans_up_its_children() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("codex");
        std::fs::write(
            &program,
            "#!/bin/sh\ncat > task\n(sleep 1; touch escaped) &\nwait\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut native = agent(program);
        native.id = "codex".into();
        native.transport = AgentTransport::CodexCli;
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancel);
        let mut discard = |_| {};
        let (result, ()) = tokio::join!(
            acp(
                &native,
                "Review scratch.",
                dir.path(),
                None,
                &cancel,
                &mut discard
            ),
            async move {
                tokio::time::sleep(Duration::from_millis(150)).await;
                signal.store(true, Ordering::Relaxed);
            }
        );
        let error = result.unwrap_err();
        assert!(error.contains("canceled"), "{error}");
        assert!(!error.contains("still be running"), "{error}");
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(!dir.path().join("escaped").exists());
    }

    #[test]
    fn configured_acp_agents_are_the_only_model_choices() {
        let mut a = agent("/example/reviewer".into());
        let definition = acp_tool_definition(&[a.clone()]).unwrap();
        assert_eq!(
            definition["function"]["parameters"]["properties"]["agent"]["enum"],
            json!(["reviewer"])
        );
        assert!(definition.to_string().find("/example/reviewer").is_none());
        a.enabled = false;
        assert!(acp_tool_definition(&[a]).is_none());
        let invalid = agent("".into());
        assert!(acp_tool_definition(&[invalid]).is_none());
    }

    #[test]
    fn installer_companions_follow_the_exact_versioned_coder_build() {
        let parent = Path::new("/fixture/versions");
        assert_eq!(
            cli_companions(&parent.join("coder-openagents-abc-dirty"))[0],
            parent.join("openagents-openagents-abc-dirty")
        );
        assert_eq!(
            cli_companions(&parent.join("coder-new-openagents-xyz"))[0],
            parent.join("openagents-openagents-xyz")
        );
        assert_eq!(
            cli_companions(&parent.join("coder-new")),
            vec![parent.join(if cfg!(windows) {
                "openagents.exe"
            } else {
                "openagents"
            })]
        );
    }

    #[test]
    fn cli_keeps_its_normal_authentication_environment() {
        let mut command = std::process::Command::new("/fixture/openagents");
        for name in ["GH_TOKEN", "OPENAGENTS_API_KEY", "OA_TOKEN"] {
            command.env(name, "fixture-value");
        }
        prepare_cli_child(&mut command);
        let environment: BTreeMap<_, _> = command.get_envs().collect();
        assert_eq!(
            environment.get(std::ffi::OsStr::new(crate::programmatic::MODEL_INPUT_ENV)),
            Some(&Some(std::ffi::OsStr::new("model")))
        );
        for name in ["GH_TOKEN", "OPENAGENTS_API_KEY", "OA_TOKEN"] {
            assert_eq!(
                environment.get(std::ffi::OsStr::new(name)),
                Some(&Some(std::ffi::OsStr::new("fixture-value")))
            );
        }
        for name in ["OPENROUTER_API_KEY", "TYPESAFE_API_KEY"] {
            assert_eq!(environment.get(std::ffi::OsStr::new(name)), Some(&None));
        }
    }

    #[test]
    fn acp_tool_updates_keep_the_original_title_and_input() {
        let mut events = vec![];
        let input = json!({"path":"fixture.rs"});
        let mut handler = AcpEvents {
            thought: crate::claude_stream::Thought::default(),
            read_only: false,
            emit: &mut |event| events.push(event),
            text: String::new(),
            tools: BTreeMap::new(),
            hooks: None,
            cwd: PathBuf::new(),
            seen: BTreeMap::new(),
            refusals: Arc::default(),
            last_thought: LastThought::default(),
        };
        handler.update(Update::ToolCall {
            id: "read-1".into(),
            title: "Read fixture.rs".into(),
            kind: "read".into(),
            status: "in_progress".into(),
            raw_input: input.clone(),
            tool: None,
        });
        handler.update(Update::ToolCallUpdate {
            id: "read-1".into(),
            title: Some("Read finished".into()),
            status: None,
            text: None,
            raw_input: None,
        });
        handler.update(Update::ToolCallUpdate {
            id: "read-1".into(),
            title: None,
            status: Some("completed".into()),
            text: Some("contents".into()),
            raw_input: None,
        });
        drop(handler);
        assert_eq!(events.len(), 3);
        for event in &events {
            assert!(
                matches!(event, RuntimeEvent::Tool {name,input:actual,..} if name == "Read fixture.rs" && actual == &input)
            );
        }
        assert!(
            matches!(&events[2], RuntimeEvent::Tool {running:false,output,..} if output == "contents")
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn cli_passes_argv_without_a_shell_and_captures_bounded_json() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("openagents");
        std::fs::write(&program, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let result = cli_at(
            &program,
            &["doctor".into(), "$(touch injected)".into()],
            dir.path(),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result["exit"], 0);
        assert_eq!(result["stdout"], "--json\ndoctor\n$(touch injected)\n");
        assert!(result["group_clear"].as_bool().unwrap());
        assert!(!dir.path().join("injected").exists());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn canceling_cli_cleans_up_its_children() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("openagents");
        std::fs::write(&program, "#!/bin/sh\n(sleep 1; touch escaped) &\nwait\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancel);
        let mut emit = |_| {};
        let (result, ()) = tokio::join!(
            cli_at(&program, &[], dir.path(), &cancel, &mut emit),
            async move {
                tokio::time::sleep(Duration::from_millis(150)).await;
                signal.store(true, Ordering::Relaxed);
            }
        );
        let result = result.unwrap();
        assert_eq!(result["canceled"], true);
        assert_eq!(result["group_clear"], true);
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(!dir.path().join("escaped").exists());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn acp_uses_the_existing_protocol_and_closes_the_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let recording = acp_client::replay::GROK_TURN;
        let program =
            acp_client::replay::script(dir.path(), &acp_client::replay::blocks(recording));
        let mut events = vec![];
        let result = acp(
            &agent(program),
            "Review this scratch task.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(result["reply"], "done");
        assert_eq!(result["stop_reason"], "end_turn");
        assert_eq!(result["group_clear"], true);
        assert!(
            events
                .iter()
                .any(|event| matches!(event,RuntimeEvent::Text(text) if text == "done"))
        );
        let sent = acp_client::replay::received(dir.path());
        assert_eq!(sent[1]["method"], "session/new");
        assert_eq!(sent[2]["method"], "session/prompt");
    }

    /// The recorded Grok turn with its prompt refused by Devin's retryable
    /// internal error; with `once`, only the first process refuses.
    #[cfg(unix)]
    fn refusing(dir: &Path, once: bool) -> PathBuf {
        let mut blocks = acp_client::replay::blocks(acp_client::replay::GROK_TURN);
        let completed = std::mem::replace(
            &mut blocks[2],
            vec![
                json!({"jsonrpc":"2.0","id":3,"error":{"code":-32013,"message":"Permission denied: an internal error occurred (trace ID: 4843f770cc)","data":{"cognition.ai/errorKind":"internal","cognition.ai/retryable":true}}}),
            ],
        );
        blocks.push(completed);
        let program = acp_client::replay::script(dir, &blocks);
        if once {
            let script = std::fs::read_to_string(&program).unwrap().replace(
                "if [ -f \"$dir/$n.jsonl\" ]; then cat \"$dir/$n.jsonl\"; fi",
                "if [ $n = 3 ] && [ -f \"$dir/refused\" ]; then cat \"$dir/4.jsonl\"; elif [ $n = 3 ]; then touch \"$dir/refused\"; cat \"$dir/3.jsonl\"; elif [ -f \"$dir/$n.jsonl\" ]; then cat \"$dir/$n.jsonl\"; fi",
            );
            std::fs::write(&program, script).unwrap();
        }
        program
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_temporary_refusal_is_tried_again_and_then_named_with_its_trace() {
        let dir = tempfile::tempdir().unwrap();
        let program = refusing(dir.path(), false);
        let error = acp(
            &agent(program),
            "Review this scratch task.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(
            error.starts_with("Reviewer on grok-4.6 reported a temporary error: Permission denied"),
            "{error}"
        );
        assert!(error.contains("It failed the same way 3 times."), "{error}");
        assert!(error.contains("Trace ID: 4843f770cc."), "{error}");
        let prompts = acp_client::replay::received(dir.path())
            .iter()
            .filter(|frame| frame["method"] == "session/prompt")
            .count();
        assert_eq!(prompts, 3);

        let dir = tempfile::tempdir().unwrap();
        let program = refusing(dir.path(), true);
        let result = acp(
            &agent(program),
            "Review this scratch task.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result["reply"], "done");
        assert_eq!(result["attempts"], 2);
    }

    #[test]
    fn an_update_required_refusal_names_the_update() {
        let mut cursor = agent(PathBuf::from("cursor-agent"));
        cursor.id = "cursor".into();
        cursor.name = "Cursor".into();
        let refusal = acp_client::wire::RpcError {
            code: acp_client::wire::INTERNAL_ERROR,
            message: "Internal error".into(),
            data: Some(
                json!({"details":{"name":"UnknownError","data":{"message":"{\"message\":\"Update Required Your version of Cursor is no longer supported. Please update to the latest version at cursor.com/downloads to continue.\",\"type\":\"cursor_error\",\"code\":\"cursor_stream_error\"}"}}}),
            ),
        };
        let error = acp_client::ClientError::Refused {
            method: "session/prompt".into(),
            error: refusal.clone(),
        };
        let said = acp_failure(
            &cursor,
            None,
            &error,
            Some(&refusal),
            Path::new("/w"),
            1,
            true,
        );
        assert!(said.starts_with(acp_client::cursor::UPDATE), "{said}");
        assert!(said.contains("Update Required"), "{said}");
        let mut opencode = cursor.clone();
        opencode.id = "opencode".into();
        opencode.name = "OpenCode".into();
        let said = acp_failure(
            &opencode,
            Some("cursor/composer-2.5"),
            &error,
            Some(&refusal),
            Path::new("/w"),
            1,
            true,
        );
        assert!(
            said.starts_with("OpenCode on cursor/composer-2.5 refused this version: Internal error: Update Required"),
            "{said}"
        );
        let untrusted =
            acp_client::wire::RpcError::new(-32000, "Refusing to run in an untrusted workspace");
        let said = acp_failure(
            &opencode,
            None,
            &error,
            Some(&untrusted),
            Path::new("/w"),
            1,
            true,
        );
        assert!(said.starts_with("OpenCode does not trust /w yet"), "{said}");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn acp_permission_requests_are_approved_by_default() {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut blocks = acp_client::replay::blocks(acp_client::replay::GROK_TURN);
        let completed = std::mem::replace(
            &mut blocks[2],
            vec![
                json!({"jsonrpc":"2.0","id":"permission-1","method":"session/request_permission","params":{"sessionId":"grok-session-1","toolCall":{"toolCallId":"write-1","title":"Change files","kind":"edit"},"options":[{"optionId":"allow","name":"Allow","kind":"allow_once"},{"optionId":"deny","name":"Deny","kind":"reject_once"}]}}),
            ],
        );
        blocks.push(completed);
        let program = acp_client::replay::script(dir.path(), &blocks);
        let script = std::fs::read_to_string(&program).unwrap().replace(
            "case \"$line\" in",
            "case \"$line\" in\n    *'\"id\":\"permission-1\"'*) cat \"$dir/4.jsonl\" ;;",
        );
        std::fs::write(&program, script).unwrap();
        let _ = acp(
            &agent(program),
            "Review this scratch task.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        let sent = acp_client::replay::received(dir.path());
        let answer = sent
            .iter()
            .find(|frame| frame["id"] == "permission-1")
            .unwrap();
        assert_eq!(answer["result"]["outcome"]["optionId"], "allow");
    }

    /// A Cursor agent over a recorded `cursor-agent acp` session.
    fn cursor_agent(program: PathBuf) -> AcpAgent {
        let mut agent = agent(program);
        agent.id = "cursor".into();
        agent.name = "Cursor".into();
        agent.arguments = acp_client::cursor::arguments();
        agent
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_cursor_delegation_signs_in_runs_agent_mode_and_approves_its_command() {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let program = acp_client::replay::script(
            dir.path(),
            &acp_client::replay::blocks(acp_client::replay::CURSOR_TURN),
        );
        let mut events = vec![];
        let result = acp(
            &cursor_agent(program),
            "Run echo, then reply done.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert!(result["reply"].as_str().unwrap().ends_with("done"));
        assert_eq!(result["stop_reason"], "end_turn");
        assert_eq!(result["model"], "grok-4.5[effort=high,fast=true]");
        assert!(events.iter().any(|event| matches!(event,
            RuntimeEvent::Tool { name, running: false, .. } if name == "`echo hi > probe.txt`")));
        let sent = acp_client::replay::received(dir.path());
        assert_eq!(sent[1]["method"], "authenticate");
        assert_eq!(sent[1]["params"]["methodId"], "cursor_login");
        assert_eq!(sent[3]["method"], "session/set_mode");
        assert_eq!(sent[3]["params"]["modeId"], "agent");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_cursor_plan_is_accepted_and_shown_in_the_rail() {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let program = acp_client::replay::script(
            dir.path(),
            &acp_client::replay::blocks(acp_client::replay::CURSOR_PLAN),
        );
        let mut events = vec![];
        let result = acp(
            &cursor_agent(program),
            "Plan a README.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(result["stop_reason"], "end_turn");
        assert!(events.iter().any(|event| matches!(event,
            RuntimeEvent::Tool { name, input, output, running: false }
                if name == "Plan" && input["name"] == "Add folder README" && output["outcome"] == "accepted")));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_signed_out_cursor_says_how_to_sign_in() {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let program = acp_client::replay::script(
            dir.path(),
            &acp_client::replay::blocks(acp_client::replay::CURSOR_SIGNED_OUT),
        );
        let error = acp(
            &cursor_agent(program),
            "Say hi.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error, acp_client::cursor::SIGN_IN);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_cursor_without_a_login_or_key_is_refused_before_it_starts() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("cursor-agent");
        std::fs::write(
            &program,
            "#!/bin/sh\nprintf '{\"isAuthenticated\":%s}' \"$SIGNED\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let env = |signed: &str, key: &str| {
            vec![
                ("PATH".to_owned(), "/bin:/usr/bin".to_owned()),
                ("SIGNED".to_owned(), signed.to_owned()),
                ("CURSOR_API_KEY".to_owned(), key.to_owned()),
            ]
        };
        assert_eq!(
            cursor_signed_in(&program, &env("false", ""), dir.path()).await,
            Err(acp_client::cursor::SIGN_IN.into())
        );
        assert_eq!(
            cursor_signed_in(&program, &env("true", ""), dir.path()).await,
            Ok(())
        );
        assert_eq!(
            cursor_signed_in(&program, &env("false", "key"), dir.path()).await,
            Ok(())
        );
        assert_eq!(
            cursor_signed_in(&dir.path().join("missing"), &env("", ""), dir.path()).await,
            Ok(())
        );
    }

    /// A devin-cli agent over the recorded Devin turn.
    fn devin_agent(program: PathBuf) -> AcpAgent {
        let mut agent = agent(program);
        agent.id = "devin-cli".into();
        agent.name = "Devin CLI".into();
        agent.arguments = acp_client::devin::arguments(acp_client::devin::DEFAULT_MODEL);
        agent
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_devin_delegation_runs_bypass_ungated_and_names_its_model() {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let program = acp_client::replay::script(
            dir.path(),
            &acp_client::replay::blocks(acp_client::replay::DEVIN_TURN),
        );
        let mut events = vec![];
        let result = acp(
            &devin_agent(program),
            "Write result.txt.",
            dir.path(),
            Some("swe-2-high"),
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(result["stop_reason"], "end_turn");
        assert_eq!(result["group_clear"], true);
        // The model the call names becomes `devin acp --model MODEL`, and
        // an ungated chat sets Devin's `bypass` mode.
        // Devin starts with a configuration whose `PreToolUse` hook runs
        // Coder's bridge before every tool (#11508).
        let arguments = acp_client::replay::arguments(dir.path());
        assert_eq!(arguments[0], "--config");
        assert!(arguments[1].ends_with("config.json"), "{arguments:?}");
        assert_eq!(arguments[2..], ["acp", "--model", "swe-2-high"]);
        assert!(
            !Path::new(&arguments[1]).exists(),
            "the configuration is removed with the session"
        );
        assert_eq!(
            result["checks"]["before_each_step"], true,
            "{}",
            result["checks"]
        );
        let sent = acp_client::replay::received(dir.path());
        let set_mode = sent
            .iter()
            .find(|frame| frame["method"] == "session/set_mode")
            .expect("a set_mode");
        assert_eq!(set_mode["params"]["modeId"], "bypass");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_gated_devin_delegation_runs_accept_edits_under_its_sandbox() {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let desk = crate::approval::Desk::new();
        crate::approval::install(Some(crate::approval::Gate {
            desk: Arc::clone(&desk),
            cancel: Arc::new(AtomicBool::new(false)),
        }));
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                crate::approval::install(None);
            }
        }
        let _reset = Reset;
        let dir = tempfile::tempdir().unwrap();
        let program = acp_client::replay::script(
            dir.path(),
            &acp_client::replay::blocks(acp_client::replay::DEVIN_TURN),
        );
        // The recording opened on SWE-2 High, not the SWE-2 Medium asked
        // for: that turn is refused (#11583).
        let refused = acp(
            &devin_agent(program),
            "Write result.txt.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(refused.contains("can't run SWE-2 Medium"), "{refused}");
        let arguments = acp_client::replay::arguments(dir.path());
        assert_eq!(arguments[0], "--config");
        // No model named: Coder's SWE-2 default, never Devin's account
        // default (#11556).
        assert_eq!(
            arguments[2..],
            [
                "--sandbox",
                "acp",
                "--model",
                acp_client::devin::CODER_MODEL
            ]
        );
        assert_eq!(acp_client::devin::CODER_MODEL, "swe-2-medium");
        let sent = acp_client::replay::received(dir.path());
        let set_mode = sent
            .iter()
            .find(|frame| frame["method"] == "session/set_mode")
            .expect("a set_mode");
        assert_eq!(set_mode["params"]["modeId"], "accept-edits");
        desk.close();
    }

    /// The recorded Free-account Devin turn (#11583), with `keep` choosing
    /// which `session/update` kinds stay, run as a delegation on `model`.
    #[cfg(unix)]
    async fn free_devin_turn(
        model: Option<&str>,
        keep: impl Fn(&str) -> bool,
    ) -> Result<Value, String> {
        let _gate_lock = crate::approval::test_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let blocks: Vec<Vec<Value>> =
            acp_client::replay::blocks(acp_client::replay::DEVIN_FREE_TURN)
                .into_iter()
                .map(|block| {
                    block
                        .into_iter()
                        .filter(|frame| {
                            frame["params"]["update"]["sessionUpdate"]
                                .as_str()
                                .is_none_or(&keep)
                        })
                        .collect()
                })
                .collect();
        let program = acp_client::replay::script(dir.path(), &blocks);
        acp(
            &devin_agent(program),
            "READ-ONLY: Read crates/coder-delegate/src and tell me in three bullets what the terminal turn module does. Reply with the summary only.",
            dir.path(),
            model,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
    }

    /// A Devin Free account opens every session on SWE-1.6 Slow whatever
    /// `--model` asks for; Coder's SWE-2 default is refused in one line
    /// that names what the account offers, never run slow (#11583).
    #[tokio::test]
    #[cfg(unix)]
    async fn a_devin_session_on_another_model_than_asked_is_refused() {
        let refused = free_devin_turn(None, |_| true).await.unwrap_err();
        assert_eq!(
            refused,
            "Devin CLI can't run SWE-2 Medium on this Devin account: it offers only SWE-1.6 Slow here. Upgrade the Devin plan at https://app.devin.ai, or pick a model it offers with /models."
        );
    }

    /// The person's own pick of SWE-1.6 Slow runs, and the answer Devin
    /// writes after its last thinking block is the reply (#11583).
    #[tokio::test]
    #[cfg(unix)]
    async fn a_picked_devin_model_runs_and_its_answer_is_the_reply() {
        let result = free_devin_turn(Some("swe-1-6-slow"), |_| true)
            .await
            .unwrap();
        assert_eq!(result["model"], "swe-1-6-slow");
        assert_eq!(result["stop_reason"], "end_turn");
        let reply = result["reply"].as_str().unwrap();
        assert_eq!(reply.matches("- **").count(), 3, "{reply}");
        assert!(result.get("reply_source").is_none());
    }

    /// The reported shape (#11583): Devin ends its turn with `end_turn`
    /// and output tokens spent, but every word came as thinking. The last
    /// thinking block is the reply, marked as thought.
    #[tokio::test]
    #[cfg(unix)]
    async fn a_devin_answer_left_in_its_thinking_is_still_the_reply() {
        let result = free_devin_turn(Some("swe-1-6-slow"), |kind| kind != "agent_message_chunk")
            .await
            .unwrap();
        assert_eq!(result["reply_source"], "thought");
        let reply = result["reply"].as_str().unwrap();
        assert!(
            reply.starts_with("The user asked me to read the terminal turn module"),
            "{reply}"
        );
        assert!(reply.contains("four-phase process"), "{reply}");
        // Only the last block: the thinking before the earlier reads is
        // not part of it.
        assert!(!reply.contains("Let me read more"), "{reply}");
    }

    /// A turn that ends with no words at all is a failure the caller sees,
    /// never an empty success (#11583).
    #[tokio::test]
    #[cfg(unix)]
    async fn a_devin_turn_with_no_words_fails_visibly() {
        let failed = free_devin_turn(Some("swe-1-6-slow"), |kind| {
            kind != "agent_message_chunk" && kind != "agent_thought_chunk"
        })
        .await
        .unwrap_err();
        assert_eq!(
            failed,
            "Devin CLI ended without an answer, after writing 424 tokens. Nothing came back to use; ask it again."
        );
    }

    #[test]
    fn a_reply_is_the_message_else_the_last_thought() {
        let mut thought = LastThought::default();
        thought.push("first ");
        thought.close();
        thought.push("second");
        assert_eq!(turn_reply("answer", &thought.text), ("answer".into(), None));
        assert_eq!(
            turn_reply(" \n", &thought.text),
            ("second".into(), Some("thought"))
        );
        assert_eq!(turn_reply("", ""), (String::new(), None));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_model_names_only_the_devin_cli_agent() {
        let dir = tempfile::tempdir().unwrap();
        let program = acp_client::replay::script(
            dir.path(),
            &acp_client::replay::blocks(acp_client::replay::GROK_TURN),
        );
        let error = acp(
            &agent(program),
            "Write result.txt.",
            dir.path(),
            Some("swe-2-high"),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(error.contains("devin-cli"), "{error}");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn microcoder_uses_the_real_loop_with_offline_generation() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use microcoder_loop::models::{Ask, Basis, Generated, NextAction};
        struct Finished;
        impl Generate for Finished {
            async fn generate(&self, _system: &str, prompt: &str) -> Generated {
                assert!(prompt.contains("A scratch task"));
                Generated {
                    action: Ok(NextAction {
                        rationale: "Done.".into(),
                        commands: vec![],
                        view: vec![],
                        freeze_tests: false,
                        expand: vec![],
                        finished: true,
                        reply: "The scratch task is complete.".into(),
                        ask: Ask::None,
                    }),
                    model: "fixture/model".into(),
                    prompt_tokens: 25,
                    completion_tokens: 10,
                    usd: Some(0.0),
                    known_usd: 0.0,
                    cost_unknown: None,
                    usd_upper: Some(0.0),
                    cost_basis: Basis::Billed,
                    milliseconds: 1,
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        if coder_boundary::Boundary::writing(dir.path())
            .build()
            .is_err()
        {
            return;
        }
        let mut events = vec![];
        let result = run_microcoder(
            "A scratch task",
            dir.path(),
            &Finished,
            None,
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(result["reply"], "The scratch task is complete.");
        assert_eq!(result["model"], "fixture/model");
        assert_eq!(result["cost_basis"], "billed");
        assert_eq!(result["tokens"], 35);
        assert_eq!(result["outcome"]["ending"]["reason"], "finished");
        assert!(
            events.iter().any(
                |event| matches!(event,RuntimeEvent::Model(model) if model == "fixture/model")
            )
        );
    }

    /// A read-only Microcoder task runs its commands in the caller's folder
    /// where they cannot write, and its result names that folder and the
    /// enforced mode (#11469, #11470).
    #[tokio::test]
    async fn a_read_only_microcoder_task_runs_in_the_folder_and_cannot_write() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use microcoder_loop::models::{Ask, Basis, Generated, NextAction};
        struct Touching(std::cell::Cell<usize>);
        impl Generate for Touching {
            async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
                let first = self.0.get() == 0;
                self.0.set(self.0.get() + 1);
                Generated {
                    action: Ok(NextAction {
                        rationale: "Look.".into(),
                        commands: if first {
                            vec!["pwd > made; pwd".into()]
                        } else {
                            vec![]
                        },
                        view: vec![],
                        freeze_tests: false,
                        expand: vec![],
                        finished: !first,
                        reply: "Done.".into(),
                        ask: Ask::None,
                    }),
                    model: "fixture/model".into(),
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    usd: Some(0.0),
                    known_usd: 0.0,
                    cost_unknown: None,
                    usd_upper: Some(0.0),
                    cost_basis: Basis::Billed,
                    milliseconds: 1,
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        if coder_boundary::Boundary::readonly()
            .protecting(dir.path())
            .build()
            .is_err()
        {
            return;
        }
        let folder = dir.path().canonicalize().unwrap();
        let result = run_microcoder_with(
            "Check the folder",
            dir.path(),
            true,
            &Touching(std::cell::Cell::new(0)),
            None,
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
            None,
        )
        .await
        .unwrap();
        assert!(!dir.path().join("made").exists());
        assert_eq!(result["cwd"], folder.display().to_string());
        assert_eq!(result["permissions"]["mode"], "read-only");
        assert_eq!(result["permissions"]["enforced"], true);
    }

    /// A read-only ACP task approves only the agent's asks to look (#11470).
    #[test]
    fn a_read_only_acp_task_refuses_asks_to_change_files() {
        let _lock = crate::approval::test_lock().lock().unwrap();
        let ask = |kind: &str| -> acp_client::wire::PermissionRequest {
            serde_json::from_value(json!({"toolCall":{"kind":kind},"options":[{"optionId":"y","kind":"allow_once"},{"optionId":"n","kind":"reject_once"}]})).unwrap()
        };
        let mut emit = |_| {};
        let mut handler = AcpEvents {
            thought: crate::claude_stream::Thought::default(),
            emit: &mut emit,
            text: String::new(),
            tools: BTreeMap::new(),
            read_only: true,
            hooks: None,
            cwd: PathBuf::new(),
            seen: BTreeMap::new(),
            refusals: Arc::default(),
            last_thought: LastThought::default(),
        };
        let chosen = |answer| match answer {
            acp_client::wire::PermissionAnswer::Selected(option) => option,
            acp_client::wire::PermissionAnswer::Cancelled => String::new(),
        };
        assert_eq!(chosen(handler.permission(&ask("read"))), "y");
        assert_eq!(chosen(handler.permission(&ask("edit"))), "n");
        assert_eq!(chosen(handler.permission(&ask("execute"))), "n");
        handler.read_only = false;
        assert_eq!(chosen(handler.permission(&ask("edit"))), "y");
    }

    #[tokio::test]
    async fn canceling_generation_does_not_wait_for_the_provider() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        struct Waiting;
        impl Generate for Waiting {
            async fn generate(
                &self,
                _system: &str,
                _prompt: &str,
            ) -> microcoder_loop::models::Generated {
                std::future::pending().await
            }
        }
        let cancel = Arc::new(AtomicBool::new(true));
        let generator = CancellableGenerator {
            generator: &Waiting,
            cancel: &cancel,
            redaction_keys: &[],
            failures: Cell::new(0),
            repeated: Cell::new(0),
            previous_commands: RefCell::new(vec![]),
            bases: RefCell::new(vec![]),
            thoughts: None,
        };
        let result = tokio::time::timeout(Duration::from_secs(1), generator.generate("", ""))
            .await
            .unwrap();
        assert!(result.action.is_err());
        assert!(result.usd.is_none());
        assert!(result.cost_unknown.is_some());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn local_loop_commands_forward_cli_delegations_without_mixing_parent_text() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use microcoder_loop::models::{Ask, Basis, Generated, NextAction};
        struct Fixture {
            command: String,
            step: Cell<usize>,
        }
        impl Generate for Fixture {
            async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
                let first = self.step.replace(self.step.get() + 1) == 0;
                Generated {
                    action: Ok(NextAction {
                        rationale: "Run the fixture delegation.".into(),
                        commands: if first {
                            vec![self.command.clone()]
                        } else {
                            vec![]
                        },
                        view: vec![],
                        freeze_tests: false,
                        expand: vec![],
                        finished: !first,
                        reply: if first {
                            String::new()
                        } else {
                            "Parent reply.".into()
                        },
                        ask: Ask::None,
                    }),
                    model: "fixture/parent".into(),
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    usd: Some(0.0),
                    known_usd: 0.0,
                    cost_unknown: None,
                    usd_upper: Some(0.0),
                    cost_basis: Basis::Billed,
                    milliseconds: 1,
                }
            }
        }
        let root = tempfile::tempdir().unwrap();
        let cli = crate::delegation_events::tests::fixture(root.path());
        let generator = Fixture {
            command: format!(
                "{} --json coder delegate codex --task 'Review the fixture' > delegated.jsonl 2>&1",
                shell_word(&cli.to_string_lossy())
            ),
            step: Cell::new(0),
        };
        let mut events = Vec::new();
        let result = run_microcoder(
            "Run the fixture delegation.",
            root.path(),
            &generator,
            None,
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(result["reply"], "Parent reply.");
        assert_eq!(result["model"], "fixture/parent");
        assert!(events.iter().any(|event| matches!(event, RuntimeEvent::Delegation { name, event, .. } if name == "Codex" && matches!(event.as_ref(), RuntimeEvent::Text(text) if text == "Fixture child reply."))));
        assert!(events.iter().all(
            |event| !matches!(event, RuntimeEvent::Text(text) if text == "Fixture child reply.")
        ));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn microcoder_redacts_command_and_file_observations_before_the_next_generation() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use microcoder_loop::models::{Ask, Basis, Generated, NextAction};
        const MARKER: &str = "fixture-saved-provider-credential";
        // This test is about redaction: the floor that holds a secret read
        // for the person is tested in `floors`.
        crate::floors::open_for_test(true);
        struct Fixture(std::sync::atomic::AtomicUsize);
        impl Generate for Fixture {
            async fn generate(&self, _system: &str, prompt: &str) -> Generated {
                let first = self.0.fetch_add(1, Ordering::Relaxed) == 0;
                assert!(!prompt.contains(MARKER));
                if !first {
                    assert!(prompt.matches("[redacted]").count() >= 2);
                }
                Generated {
                    action: Ok(NextAction {
                        rationale: "Inspect the scratch configuration.".into(),
                        commands: if first {
                            vec!["cat .env".into()]
                        } else {
                            vec![]
                        },
                        view: vec![".env".into()],
                        freeze_tests: false,
                        expand: vec![],
                        finished: !first,
                        reply: if first {
                            String::new()
                        } else {
                            "The configuration was inspected.".into()
                        },
                        ask: Ask::None,
                    }),
                    model: "fixture/model".into(),
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    usd: Some(0.0),
                    known_usd: 0.0,
                    cost_unknown: None,
                    usd_upper: Some(0.0),
                    cost_basis: Basis::Billed,
                    milliseconds: 1,
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().canonicalize().unwrap();
        std::fs::write(
            directory.join(".env"),
            format!("OPENROUTER_API_KEY={MARKER}\n"),
        )
        .unwrap();
        let Ok(boundary) = coder_boundary::Boundary::writing(&directory).build() else {
            return;
        };
        let keys = vec![ApiKey::new(MARKER)];
        let cancel = Arc::new(AtomicBool::new(false));
        let environment = Checkout {
            asked: std::sync::Mutex::default(),
            hooks: None,
            directory,
            boundary: Some(boundary),
            cancel: Arc::clone(&cancel),
            redaction_keys: &keys,
            events: None,
        };
        assert_eq!(
            environment.read(".env").await.as_deref(),
            Some("OPENROUTER_API_KEY=[redacted]\n")
        );
        let output = environment.run("cat .env", Duration::from_secs(5)).await;
        assert_eq!(output.exit, Some(0));
        assert!(output.output.contains("[redacted]"));
        assert!(!output.output.contains(MARKER));
        drop(environment);
        let generator = Fixture(std::sync::atomic::AtomicUsize::new(0));
        let mut events = vec![];
        let result = run_microcoder(
            "Inspect this scratch configuration.",
            dir.path(),
            &generator,
            None,
            &keys,
            &cancel,
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(generator.0.load(Ordering::Relaxed), 2);
        assert_eq!(result["reply"], "The configuration was inspected.");
        assert!(!result.to_string().contains(MARKER));
        assert!(events.iter().any(|event| matches!(event,RuntimeEvent::Tool {running:false,output,..} if output["output"].as_str().is_some_and(|output| output.contains("[redacted]")))));
    }

    #[tokio::test]
    async fn unconfigured_jev_is_missing_evidence() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let judge = PluginJudge(None);
        let result = judge
            .judge(
                &microcoder_loop::models::question_set(),
                &json!({"task":"A scratch task"}),
            )
            .await;
        assert!(result.answers.is_empty());
        assert!(result.error.as_deref().unwrap().contains("no decision"));
        assert_eq!(result.usd, Some(0.0));
    }

    #[test]
    fn a_judged_step_reaches_the_run_as_jevs_estimate_and_only_jevs() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut events = Vec::new();
        {
            let mut push = |event: RuntimeEvent| events.push(event);
            let sink = RefCell::new(&mut push as &mut dyn FnMut(RuntimeEvent));
            let mut observer = MicrocoderEvents {
                emit: &sink,
                reply: String::new(),
                model: None,
                tokens: 0,
                redaction_keys: &[],
            };
            let judged = |judgment| Event::Judged { step: 3, judgment };
            observer.event(
                1.0,
                &judged(microcoder_loop::models::Judgment {
                    scores: vec![(microcoder_loop::models::COMPLETE.into(), 0.4)],
                    ..Default::default()
                }),
            );
            // No Jev, or a failed call: the step alone, never a number.
            observer.event(
                2.0,
                &judged(microcoder_loop::models::Judgment {
                    error: Some("Jev is not configured; no decision was made.".into()),
                    ..Default::default()
                }),
            );
            observer.event(3.0, &judged(microcoder_loop::models::Judgment::default()));
        }
        let progress: Vec<_> = events
            .iter()
            .map(|event| match event {
                RuntimeEvent::Progress { step, complete } => (*step, *complete),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(progress, [(3, Some(0.4)), (3, None), (3, None)]);
        assert_eq!(
            RuntimeEvent::progress_line(3, Some(0.4)),
            "step 3 · ≈40% done"
        );
        assert_eq!(RuntimeEvent::progress_line(3, None), "step 3");
        assert_eq!(
            RuntimeEvent::progress_line(9, Some(1.7)),
            "step 9 · ≈100% done"
        );
        // A synced or delegated stream carries the same event back.
        let value = json!({"event":"progress","step":3,"complete":0.4});
        assert!(matches!(
            crate::delegation_events::decode(&value, 0),
            Some(RuntimeEvent::Progress { step: 3, complete: Some(c) }) if (c - 0.4).abs() < 1e-9
        ));
    }

    struct GatewayFixture {
        calls: std::sync::atomic::AtomicUsize,
        replies: Vec<String>,
    }

    impl coder::generate::Generate for GatewayFixture {
        async fn generate<'a>(
            &'a self,
            instructions: &'a str,
            input: &'a [coder::generate::Message],
            _sink: &'a mut (dyn FnMut(&str) + Send),
            meta: &'a mut (dyn FnMut(coder::generate::Meta) + Send),
        ) -> Result<(String, Option<coder::generate::Usage>), coder::generate::GenerateError>
        {
            let index = self.calls.fetch_add(1, Ordering::Relaxed);
            assert!(instructions.contains("exactly one JSON object"));
            assert_eq!(input.len(), 1);
            if index > 0 && self.replies.len() > 3 {
                assert!(
                    input[0].text.contains("Recovery") || input[0].text.contains("ran no commands")
                );
            }
            meta(coder::generate::Meta::Model(
                "google/gemini-3.8-flash".into(),
            ));
            Ok((
                self.replies[index].clone(),
                Some(coder::generate::Usage {
                    input_tokens: 20,
                    output_tokens: 10,
                }),
            ))
        }
    }

    fn fixture_action(commands: Vec<String>, finished: bool) -> String {
        json!({
            "rationale":"Complete the scratch task.","commands":commands,"view":[],
            "freeze_tests":false,"expand":[],"finished":finished,
            "reply":if finished {"The scratch task is complete."} else {"Working."},"ask":"none"
        })
        .to_string()
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn no_login_gateway_repairs_bad_replies_and_continues_past_old_step_and_idle_limits() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        if coder_boundary::Boundary::writing(dir.path())
            .build()
            .is_err()
        {
            return;
        }
        let mut replies = vec!["not an action".into(); 4];
        replies.extend((0..26).map(|_| fixture_action(vec![], false)));
        replies.push(fixture_action(vec![], true));
        let fixture = Arc::new(GatewayFixture {
            calls: std::sync::atomic::AtomicUsize::new(0),
            replies,
        });
        let gateway = coder::cloud::CloudLane::new(Arc::clone(&fixture));
        let mut events = vec![];
        let result = run_microcoder(
            "A scratch task",
            dir.path(),
            &gateway,
            None,
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(fixture.calls.load(Ordering::Relaxed), 31);
        assert_eq!(result["outcome"]["ending"]["reason"], "finished");
        assert_eq!(result["model"], "google/gemini-3.8-flash");
        assert_eq!(result["tokens"], 31 * 30);
        assert!(events.iter().any(|event| matches!(event, RuntimeEvent::Text(text) if text == "The scratch task is complete.")));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_local_provider_failure_mid_task_falls_back_without_replaying_completed_commands() {
        // Another test's approval gate refuses or holds commands (#11564).
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        enum FixtureProvider {
            Unavailable(Arc<std::sync::atomic::AtomicUsize>),
            Gateway(coder::cloud::CloudLane<GatewayFixture>),
        }
        impl Generate for FixtureProvider {
            async fn generate(
                &self,
                system: &str,
                prompt: &str,
            ) -> microcoder_loop::models::Generated {
                match self {
                    Self::Unavailable(calls) => {
                        let first = calls.fetch_add(1, Ordering::Relaxed) == 0;
                        let mut generated = microcoder_loop::failover::refused_generation(
                            "local/model",
                            false,
                            "The login expired.",
                        );
                        if first {
                            generated.action =
                                microcoder_loop::models::parse_action(&fixture_action(
                                    vec!["printf 'once\\n' >> result.txt".into()],
                                    false,
                                ));
                        } else {
                            assert!(prompt.contains("result.txt"));
                        }
                        generated
                    }
                    Self::Gateway(gateway) => gateway.generate(system, prompt).await,
                }
            }
        }
        impl microcoder_loop::failover::Lane for FixtureProvider {
            fn refusal(&self) -> Option<microcoder_loop::capacity::Refusal> {
                match self {
                    Self::Unavailable(_) => None,
                    Self::Gateway(gateway) => gateway.refusal(),
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        if coder_boundary::Boundary::writing(dir.path())
            .build()
            .is_err()
        {
            return;
        }
        let local_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let fixture = Arc::new(GatewayFixture {
            calls: std::sync::atomic::AtomicUsize::new(0),
            replies: vec![fixture_action(vec![], true)],
        });
        let providers = coder::local_chain::GeneratorChain {
            providers: vec![
                FixtureProvider::Unavailable(Arc::clone(&local_calls)),
                FixtureProvider::Gateway(coder::cloud::CloudLane::new(Arc::clone(&fixture))),
            ],
            current: Cell::new(0),
            book: None,
            refused: std::cell::RefCell::new(vec![]),
        };
        let result = run_microcoder(
            "A scratch task",
            dir.path(),
            &providers,
            None,
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result["outcome"]["ending"]["reason"], "finished");
        assert_eq!(local_calls.load(Ordering::Relaxed), 2);
        assert_eq!(fixture.calls.load(Ordering::Relaxed), 1);
        assert_eq!(result["model"], "google/gemini-3.8-flash");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("result.txt")).unwrap(),
            "once\n"
        );
    }

    /// #11593: Microcoder's own door answers every step on Gemini 4
    /// Argon and tells the model which model it is; a wrong reply stays on
    /// the door; two calls in a row with no reply move the rest of the
    /// task to this computer's logins and say why.
    #[tokio::test]
    async fn microcoder_runs_on_argon_and_leaves_the_gateway_only_when_it_stops_answering() {
        use microcoder_loop::models::{Ask, Basis, Generated, NextAction};
        fn reply(model: &str, tokens: u32, action: Result<NextAction, String>) -> Generated {
            Generated {
                action,
                model: model.into(),
                prompt_tokens: tokens.into(),
                completion_tokens: 0,
                usd: Some(0.0),
                known_usd: 0.0,
                cost_unknown: None,
                usd_upper: Some(0.0),
                cost_basis: Basis::Billed,
                milliseconds: 1,
            }
        }
        fn done() -> NextAction {
            NextAction {
                rationale: "Done.".into(),
                commands: vec![],
                view: vec![],
                freeze_tests: false,
                expand: vec![],
                finished: true,
                reply: "ok".into(),
                ask: Ask::None,
            }
        }
        /// The gateway: its scripted answers in order, and the systems
        /// it was sent.
        struct Door(RefCell<Vec<Generated>>, RefCell<Vec<String>>);
        impl Generate for Door {
            async fn generate(&self, system: &str, _prompt: &str) -> Generated {
                self.1.borrow_mut().push(system.to_owned());
                self.0.borrow_mut().remove(0)
            }
        }
        struct Login;
        impl Generate for Login {
            async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
                reply("codex:gpt-6.1-sol", 5, Ok(done()))
            }
        }
        let argon = MICROCODER_MODEL;
        assert_eq!(argon, "google/gemini-4-argon");
        assert_eq!(crate::ui::rail_model_name(argon), "Gemini 4 Argon");
        let gateway = Door(
            RefCell::new(vec![
                reply(argon, 9, Err("next_action's arguments didn't parse".into())),
                reply(argon, 0, Err("HTTP 503".into())),
                reply(argon, 9, Ok(done())),
                reply(argon, 0, Err("connection refused".into())),
                reply(argon, 0, Err("connection refused".into())),
            ]),
            RefCell::new(vec![]),
        );
        let first = GatewayFirst::new(gateway, Box::new(|| Ok(Login)));
        // A wrong reply and one miss stay on the door.
        assert!(first.generate("SYSTEM", "p").await.action.is_err());
        assert!(first.generate("SYSTEM", "p").await.action.is_err());
        assert_eq!(first.generate("SYSTEM", "p").await.model, argon);
        assert!(first.left().is_none());
        assert!(
            first
                .gateway
                .1
                .borrow()
                .iter()
                .all(|system| system.starts_with("SYSTEM")
                    && system
                        .contains("You run on Gemini 4 Argon through the OpenAgents gateway."))
        );
        // Two misses in a row: the same step runs on the login.
        let moved = first.generate("SYSTEM", "p").await;
        assert!(first.generate("SYSTEM", "p").await.action.is_ok());
        assert!(moved.action.is_err(), "one miss is still the door's");
        let after = first.generate("SYSTEM", "p").await;
        assert_eq!(after.model, "codex:gpt-6.1-sol");
        assert_eq!(first.left().as_deref(), Some("connection refused"));
        assert!(first.fallback().is_some());
        assert_eq!(
            gateway_left("connection refused", Some("codex")),
            "The OpenAgents gateway couldn't be reached (connection refused), so Microcoder finished on this computer's Codex login instead of Gemini 4 Argon."
        );
    }

    /// #11593: the loop says who it is, and each door adds the model.
    #[test]
    fn microcoder_says_it_is_microcoder_on_the_model_it_runs() {
        assert!(
            microcoder_loop::run::SYSTEM.contains("You are Microcoder, OpenAgents' coding agent.")
        );
        assert!(microcoder_loop::run::SYSTEM.contains("never say you are ChatGPT"));
        assert!(!microcoder_loop::run::SYSTEM.contains("You are Coder"));
        assert_eq!(
            microcoder_loop::run::runs_on("Gemini 4 Argon through the OpenAgents gateway"),
            " You run on Gemini 4 Argon through the OpenAgents gateway. Asked which model runs you, say exactly that; never guess."
        );
    }

    /// #11593: a read-only task has a private scratch to work in: it can
    /// copy the project there and run its tests, cargo builds there, and
    /// the project itself still can't change.
    #[tokio::test]
    #[cfg(unix)]
    async fn a_read_only_task_tests_a_copy_in_its_scratch_and_cannot_change_the_project() {
        let _gate_lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use microcoder_loop::models::{Ask, Basis, Generated, NextAction};
        struct Copying(std::cell::Cell<usize>);
        impl Generate for Copying {
            async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
                let first = self.0.get() == 0;
                self.0.set(self.0.get() + 1);
                Generated {
                    action: Ok(NextAction {
                        rationale: "Test a copy.".into(),
                        commands: if first {
                            vec![
                                "cp -R . \"$TMPDIR/copy\" && cd \"$TMPDIR/copy\" && sh test.sh > result && cat result".into(),
                                "touch \"$CARGO_TARGET_DIR/built\" && mkdir -p \"$XDG_CACHE_HOME/tool\" && test -d \"$CARGO_HOME\" && echo cached".into(),
                                "echo changed > test.sh || echo refused".into(),
                            ]
                        } else {
                            vec![]
                        },
                        view: vec![],
                        freeze_tests: false,
                        expand: vec![],
                        finished: !first,
                        reply: "Done.".into(),
                        ask: Ask::None,
                    }),
                    model: "fixture/model".into(),
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    usd: Some(0.0),
                    known_usd: 0.0,
                    cost_unknown: None,
                    usd_upper: Some(0.0),
                    cost_basis: Basis::Billed,
                    milliseconds: 1,
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        if coder_boundary::Boundary::readonly()
            .protecting(dir.path())
            .build()
            .is_err()
        {
            return;
        }
        std::fs::write(dir.path().join("test.sh"), "echo tests passed\n").unwrap();
        let mut events = vec![];
        run_microcoder_with(
            "Run the tests",
            dir.path(),
            true,
            &Copying(std::cell::Cell::new(0)),
            None,
            &[],
            &Arc::new(AtomicBool::new(false)),
            &mut |event| events.push(event),
            None,
        )
        .await
        .unwrap();
        let shown = format!("{events:?}");
        assert!(shown.contains("tests passed"), "{shown}");
        assert!(shown.contains("cached"), "{shown}");
        assert!(shown.contains("refused"), "{shown}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("test.sh")).unwrap(),
            "echo tests passed\n"
        );
        assert!(!dir.path().join("result").exists());
        let vars = scratch_env(dir.path());
        let names: Vec<&str> = vars.iter().map(|(name, _)| *name).collect();
        assert!(names.starts_with(&["CARGO_TARGET_DIR", "CARGO_HOME", "XDG_CACHE_HOME"]));
        assert!(
            vars.iter()
                .all(|(name, path)| *name == "RUSTUP_HOME" || path.starts_with(dir.path()))
        );
    }
}
