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
    models::{
        CodexGenerator, Generate, JevJudge, Judge, Judgment, OpenRouterGenerator, QuestionSet,
    },
    run::{Event, Limits, Models, Observer},
    state::{CommandResult, State},
};
use model_access::ApiKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const TEXT_MAX: usize = 64 * 1024;
const ARGUMENT_MAX: usize = 128;
const ARGUMENT_BYTES: usize = 64 * 1024;
const RUN_SECONDS: u64 = 600;
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
            && (self.id != claude_print::ID || !self.arguments.is_empty() || self.mode.is_some())
        {
            return Err("The built-in Claude Code bridge requires the claude agent ID and does not accept ACP arguments or modes.".into());
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
            Self::Tokens(_) | Self::Progress { .. } => {}
            Self::Text(text) | Self::Model(text) => *text = redact_text(text, keys),
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
        "description":"Run the bundled OpenAgents CLI in the current working directory. Pass an argument array, never a shell command. Use [\"--help\"] to discover command groups, then [\"GROUP\",\"--help\"] for syntax. The CLI supports computers, tasks, issues, settings, knowledge, relays, plugins, worlds, and wallets; commands retain their own access checks. JSON output is added by the host. Call only for work the user requested; do not send messages, publish, pay, or delete data without their authorization.",
        "parameters":{"type":"object","properties":{"arguments":{"type":"array","items":{"type":"string"},"maxItems":128}},"required":["arguments"],"additionalProperties":false}
    }})
}

/// The `run` tool of an agent-driven chat: one shell command in the
/// working directory, under the approval gate.
pub fn run_tool_definition() -> Value {
    json!({"type":"function","function":{
        "name":"Run",
        "description":"Run one shell command in the working directory and return its exit status and output. Commands have full filesystem and network access by default. Follow the user's instructions. An explicitly gated host can require CONFIRM or REJECT for changes. Never start an interactive program or a pager.",
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
    let boundary = command_boundary(&directory)?;
    let sink = RefCell::new(emit);
    let checkout = Checkout {
        directory,
        boundary,
        cancel: Arc::clone(cancel),
        redaction_keys,
        events: Some(&sink),
    };
    let ran = checkout.run(script, Duration::from_secs(120)).await;
    Ok(json!({"exit":ran.exit,"output":ran.output,"timed_out":ran.timed_out,"seconds":ran.seconds}))
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
        "description":"Delegate a task to a registered local agent through ACP or the built-in native Codex bridge. Use exactly the agent the user names; never substitute another agent. If that agent is unavailable, report the reason and let the user choose. The host supplies the executable. Include the task, relevant context, and the result you need. This starts one child session, streams its work, and closes it when the task ends. Agents start with full permissions by default: native Codex has no sandbox or approval prompts, and ACP permission requests are approved. Explicit host approval policies still apply.",
        "parameters":{"type":"object","properties":{"agent":{"type":"string","enum":ids},"task":{"type":"string","minLength":1,"maxLength":65536},"model":{"type":"string","description":"The model the agent runs; only devin-cli takes one."}},"required":["agent","task"],"additionalProperties":false}
    }}))
}

pub fn microcoder_tool_definition() -> Value {
    json!({"type":"function","function":{
        "name":"microcoder",
        "description":"Hand a concrete coding task to the bundled Microcoder loop. The loop uses structured next actions, Jev judgments when configured, and commands bounded to writes in the current checkout. State the desired result and relevant constraints. The delegation runs until completion or cancellation and returns the reply, actual model, token usage, and ending.",
        "parameters":{"type":"object","properties":{"task":{"type":"string","minLength":1,"maxLength":65536}},"required":["task"],"additionalProperties":false}
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
    let program = cli_binary().ok_or_else(|| {
        format!(
            "The bundled OpenAgents CLI is missing. Reinstall Coder with `{}`.",
            crate::account::INSTALL_COMMAND
        )
    })?;
    cli_at(&program, arguments, cwd, cancel, emit).await
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
    if let crate::approval::Verdict::Refused(why) = crate::approval::check(&shown) {
        return Err(why);
    }
    let mut command = std::process::Command::new(program);
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
    let seconds = if cloud_job { 12 * 3600 + 1200 } else { 300 };
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

fn scrub_credentials(command: &mut std::process::Command) {
    for (name, _) in std::env::vars_os() {
        let text = name.to_string_lossy();
        if text.ends_with("_API_KEY") || text.ends_with("_TOKEN") || text.ends_with("_SECRET") {
            command.env_remove(name);
        }
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
    if !crate::approval::tools_allowed() {
        return Err(
            "The crew charter refuses ACP and Codex delegation, including read-only work.".into(),
        );
    }
    if model.is_some() && agent.id != "devin-cli" {
        return Err(format!(
            "A model names only the devin-cli agent, not {}.",
            agent.id
        ));
    }
    agent.validate()?;
    if !agent.enabled {
        return Err("This ACP agent is turned off.".into());
    }
    validate_task(task)?;
    let started = Instant::now();
    let canceled =
        || cancel.load(Ordering::Relaxed) || started.elapsed() >= Duration::from_secs(RUN_SECONDS);
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
    if agent.transport == AgentTransport::CodexCli {
        let sandbox = codex_sandbox()?;
        return codex_cli(&program, task, cwd, sandbox, cancel, emit).await;
    }
    if agent.transport == AgentTransport::ClaudeCli {
        return claude_print::run(&program, task, cwd, cancel, emit).await;
    }
    let cursor = agent.id == "cursor";
    let admitted = std::env::var("OA_CODER_CLOUD_CREDENTIAL_NAMES").unwrap_or_default();
    let environment: Vec<(String, String)> = std::env::vars()
        .filter(|(name, _)| {
            admitted.split(',').any(|allowed| allowed == name)
                || (cursor && acp_client::cursor::CREDENTIAL_VARS.contains(&name.as_str()))
                || !(name.ends_with("_API_KEY")
                    || name.ends_with("_TOKEN")
                    || name.ends_with("_SECRET"))
        })
        .collect();
    // Devin's session mode and sandbox come from the chat's gate, the same
    // access a `devin:` route's grant names: an ungated chat is full access,
    // so Devin runs `bypass`; a gated chat is the boundary, so it runs
    // `accept-edits` under `devin --sandbox` and every ask is refused
    // (#10929). An explicit mode on the agent still wins.
    let mut arguments = match (agent.id.as_str(), model) {
        ("devin-cli", Some(model)) => acp_client::devin::arguments(model),
        _ => agent.arguments.clone(),
    };
    let mode = agent.mode.clone().or_else(|| {
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
    if cursor {
        cursor_signed_in(&program, &environment, cwd).await?;
    }
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
    let mut session =
        Session::open(&opening, &canceled)
            .await
            .map_err(|failure| match &failure {
                _ if cursor && failure.unauthenticated() => acp_client::cursor::SIGN_IN.to_owned(),
                acp_client::Failure::Protocol {
                    error: acp_client::ClientError::Silent { method, .. },
                    ..
                } if cursor && method == acp_client::wire::method::AUTHENTICATE => {
                    acp_client::cursor::SILENT_SIGN_IN.to_owned()
                }
                _ => failure.to_string(),
            })?;
    let id = session.id().to_owned();
    let model = session.opened.model().map(str::to_owned);
    if let Some(model) = &model {
        emit(RuntimeEvent::Model(model.clone()));
    }
    let mut handler = AcpEvents {
        emit,
        text: String::new(),
        tools: BTreeMap::new(),
    };
    let result = session
        .prompt(
            task,
            Duration::from_secs(120),
            &canceled,
            Duration::from_secs(2),
            &mut handler,
        )
        .await;
    let text = handler.text;
    let group_clear = session.close(Duration::from_secs(2)).await;
    result.map(|reply| json!({"session":id,"reply":text,"model":model,"stop_reason":reply.stop_reason.as_str(),"usage":reply.usage,"group_clear":group_clear})).map_err(|error| format!("ACP task failed: {error}; process group cleared: {group_clear}."))
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

/// Drive Codex's native protocol without treating its executable as an ACP server.
async fn codex_cli(
    program: &Path,
    task: &str,
    cwd: &Path,
    sandbox: CodexSandbox,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    if !crate::approval::tools_allowed() {
        return Err("The crew charter refuses Codex delegation, including read-only work.".into());
    }
    let mut command = std::process::Command::new(program);
    command.args(["exec", "--json", "--skip-git-repo-check"]);
    let variable = |name| std::env::var(name).ok().filter(|value| !value.is_empty());
    let model = variable("CODER_CODEX_MODEL");
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
    command
        .args(["-c", "approval_policy=\"never\"", "-"])
        .current_dir(cwd);
    let (mark, value) = coder_delegate::delegate::Agent::Codex.engine_mark();
    command.env(mark, value);
    scrub_credentials(&mut command);
    let mut live = supervise::Job::from_command(command)
        .bounded(supervise::Limits::until_stopped().keeping(TEXT_MAX))
        .start(supervise::Input::Piped)?;
    if let Err(error) = live.send(task.as_bytes()).await {
        let stopped = live.stop().await;
        return Err(format!(
            "Codex could not read the task: {error}; process group cleared: {}.",
            stopped.group_clear
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
            "The Codex task was canceled; process group cleared: {}.",
            stopped.group_clear
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
        let limited = if coder_delegate::limit::says_limited(&reason) {
            "Codex is out of capacity, so continue without Codex. "
        } else {
            ""
        };
        return Err(format!(
            "{limited}Codex task failed: {reason}; process group cleared: {}.",
            stopped.group_clear
        ));
    }
    if !events.completed {
        return Err(format!(
            "Codex exited without completing the turn; process group cleared: {}.",
            stopped.group_clear
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
    tools: BTreeMap<String, (String, Value)>,
}

impl Handler for AcpEvents<'_> {
    fn permission(
        &mut self,
        request: &acp_client::wire::PermissionRequest,
    ) -> acp_client::wire::PermissionAnswer {
        let option = if !crate::approval::gated() {
            request.allow()
        } else {
            request.reject()
        };
        option.map_or(acp_client::wire::PermissionAnswer::Cancelled, |option| {
            acp_client::wire::PermissionAnswer::Selected(option.to_owned())
        })
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
        match update {
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
            } => {
                let (name, input) = self
                    .tools
                    .entry(id.clone())
                    .or_insert_with(|| (title.unwrap_or(id), Value::Null));
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

/// Local logins followed by the OpenAgents gateway, without a model API key.
pub struct LocalGenerator {
    providers: GeneratorChain<LocalProvider>,
}

enum LocalProvider {
    Codex(Box<CodexGenerator>),
    Claude(microcoder_loop::claude::ClaudeGenerator),
    Gateway(coder::cloud::CloudLane<coder::relay::RelayDoor>),
}

impl Generate for LocalProvider {
    async fn generate(&self, system: &str, prompt: &str) -> microcoder_loop::models::Generated {
        match self {
            Self::Codex(generator) => generator.generate(system, prompt).await,
            Self::Claude(generator) => generator.generate(system, prompt).await,
            Self::Gateway(generator) => generator.generate(system, &gateway_prompt(prompt)).await,
        }
    }
}

/// Regenerate a failed action before dispatch, preserving earlier commands.
struct GeneratorChain<G> {
    providers: Vec<G>,
    current: Cell<usize>,
}

impl<G: Generate> Generate for GeneratorChain<G> {
    async fn generate(&self, system: &str, prompt: &str) -> microcoder_loop::models::Generated {
        let mut spent = None;
        loop {
            let current = self.current.get();
            let generated = self.providers[current].generate(system, prompt).await;
            let retry_elsewhere = generated.action.is_err() && current + 1 < self.providers.len();
            let generated = microcoder_loop::failover::merge(spent.take(), generated);
            if !retry_elsewhere {
                return generated;
            }
            spent = Some(generated);
            self.current.set(current + 1);
        }
    }
}

impl Generate for LocalGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> microcoder_loop::models::Generated {
        self.providers.generate(system, prompt).await
    }
}

/// Keep the task and recent observations within the gateway's request size.
fn gateway_prompt(prompt: &str) -> String {
    const HEAD: usize = 12 * 1024;
    const TAIL: usize = 32 * 1024;
    if prompt.len() <= HEAD + TAIL {
        return prompt.to_owned();
    }
    let head = bounded(prompt, HEAD);
    let mut tail = prompt.len().saturating_sub(TAIL);
    while !prompt.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{head}\n\n[Earlier observations were compacted. Inspect current files to recover details; \
         do not repeat completed changes.]\n\n{}",
        &prompt[tail..]
    )
}

/// Resolve local model providers and the no-setup gateway fallback.
pub fn local_generator() -> Result<LocalGenerator, String> {
    use codex_transport::codex::{CodexTransport, Login};
    let session = format!("coder-new-{}", std::process::id());
    let mut providers = vec![];
    if let Some(path) = Login::default_path() {
        if let Ok(transport) = CodexTransport::new(path, &session) {
            providers.push(LocalProvider::Codex(Box::new(CodexGenerator {
                transport,
                model: std::env::var("CODER_CODEX_MODEL")
                    .ok()
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| microcoder_loop::MODEL.into()),
                effort: std::env::var("CODER_CODEX_REASONING")
                    .ok()
                    .filter(|v| !v.is_empty()),
                cache_key: session,
                images: vec![],
            })));
        }
    }
    if let Ok(generator) =
        microcoder_loop::claude::ClaudeGenerator::from_env(microcoder_loop::MODEL, None)
    {
        providers.push(LocalProvider::Claude(generator));
    }
    if std::env::var("CODER_CLOUD").as_deref() != Ok("off") {
        match coder::cloud::door(&|name| std::env::var(name).ok()) {
            Ok(door) => providers.push(LocalProvider::Gateway(coder::cloud::CloudLane::new(
                Arc::new(door),
            ))),
            Err(error) if providers.is_empty() => {
                return Err(format!(
                    "Cannot connect to the OpenAgents AI Gateway: {error}"
                ));
            }
            Err(_) => {}
        }
    }
    if providers.is_empty() {
        return Err("No Codex or Claude Code login is available, and the OpenAgents AI Gateway is turned off. Enable OpenRouter BYOK or unset CODER_CLOUD=off.".into());
    }
    Ok(LocalGenerator {
        providers: GeneratorChain {
            providers,
            current: Cell::new(0),
        },
    })
}

pub async fn microcoder_local(
    task: &str,
    cwd: &Path,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    validate_task(task)?;
    let generator = local_generator()?;
    run_microcoder(task, cwd, &generator, jev, redaction_keys, cancel, emit).await
}

pub async fn microcoder_openrouter(
    task: &str,
    cwd: &Path,
    client: openrouter::Client,
    model: String,
    effort: Option<String>,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    validate_task(task)?;
    let generator = OpenRouterGenerator {
        client,
        model,
        effort,
    };
    run_microcoder(task, cwd, &generator, jev, redaction_keys, cancel, emit).await
}

async fn run_microcoder<G: Generate>(
    task: &str,
    cwd: &Path,
    generator: &G,
    jev: Option<jev::Client>,
    redaction_keys: &[ApiKey],
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("The coding task was canceled before it started.".into());
    }
    let directory = cwd
        .canonicalize()
        .map_err(|_| "The working directory is unavailable.".to_string())?;
    let boundary = command_boundary(&directory)?;
    let sink = RefCell::new(emit);
    let environment = Checkout {
        directory: directory.clone(),
        boundary,
        cancel: Arc::clone(cancel),
        redaction_keys,
        events: Some(&sink),
    };
    let state = State {
        task: redact_text(task, redaction_keys),
        environment: format!(
            "Working directory: {}. Commands have full filesystem and network access unless the host explicitly installs an approval policy.",
            directory.display()
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
    let mut result = json!({"reply":observer.reply,"model":observer.model,"tokens":observer.tokens,"outcome":outcome});
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
            () = async { while !self.cancel.load(Ordering::Relaxed) { tokio::time::sleep(POLL).await; } } => microcoder_loop::models::Generated {
                action: Err("The user canceled this generation.".into()),
                model: String::new(), prompt_tokens: 0, completion_tokens: 0, usd: None, known_usd: 0.0,
                cost_unknown: Some("A generation was canceled after it may have reached the provider.".into()),
                usd_upper: None, cost_basis: microcoder_loop::models::Basis::ListPrice, milliseconds: 0,
            },
        };
        match &generated.action {
            Ok(action) => {
                self.failures.set(0);
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

fn command_boundary(directory: &Path) -> Result<Option<coder_boundary::Boundary>, String> {
    if !crate::approval::gated() {
        return Ok(None);
    }
    coder_boundary::Boundary::writing(directory)
        .owned_scratch_under(std::env::temp_dir())
        .build()
        .map(Some)
        .map_err(|error| format!("The host could not bound the command: {error}"))
}

struct Checkout<'a> {
    directory: PathBuf,
    boundary: Option<coder_boundary::Boundary>,
    cancel: Arc<AtomicBool>,
    redaction_keys: &'a [ApiKey],
    events: Option<&'a dyn crate::delegation_events::Sink>,
}

impl Env for Checkout<'_> {
    async fn run(&self, script: &str, deadline: Duration) -> CommandResult {
        let started = Instant::now();
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
            if let Some(scratch) = self
                .boundary
                .as_ref()
                .and_then(|boundary| boundary.scratch())
            {
                command.env("TMPDIR", scratch);
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
                self.events.map(|sink| (sink, script)),
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
                    text.push_str("\nThe command's process group did not clear.");
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
            command: redact_text(script, self.redaction_keys),
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
                if let Ok(action) = &generated.action {
                    if !action.reply.is_empty() {
                        self.reply =
                            bounded(&redact_text(&action.reply, self.redaction_keys), TEXT_MAX);
                    }
                    for command in &action.commands {
                        self.emit.emit(RuntimeEvent::Tool {
                            name: "Run".into(),
                            input: json!(redact_text(command, self.redaction_keys)),
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
                input: json!(result.command),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(unix)]
    async fn silent_run_reports_elapsed_and_silence() {
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
            directory: cwd.clone(),
            boundary: command_boundary(&cwd).unwrap(),
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
            emit: &mut emit,
            text: String::new(),
            tools: BTreeMap::new(),
        };
        assert_eq!(
            handler.permission(&request),
            acp_client::wire::PermissionAnswer::Selected("deny".into())
        );
        desk.close();
        assert_eq!(codex_sandbox().unwrap(), CodexSandbox::ReadOnly);
        assert!(desk.drain().is_empty());
    }

    #[tokio::test]
    async fn crew_tool_free_scope_refuses_native_fallback_reads_and_commands() {
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().canonicalize().unwrap();
        std::fs::write(directory.join("private.txt"), "private fixture").unwrap();
        let boundary = coder_boundary::Boundary::writing(&directory)
            .build()
            .unwrap();
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

    #[tokio::test]
    #[cfg(unix)]
    async fn native_codex_starts_with_full_permissions_and_streams_its_protocol() {
        use std::os::unix::fs::PermissionsExt;
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
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result["sandbox"], "workspace-write");
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.contains("--sandbox\nworkspace-write\n"));
        assert!(!args.contains("bypass"));

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
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(
            error.starts_with("Codex is out of capacity, so continue without Codex."),
            "{error}"
        );
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
        assert!(error.contains("process group cleared: true"));
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
        assert!(
            result
                .unwrap_err()
                .contains("canceled; process group cleared: true")
        );
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
            emit: &mut |event| events.push(event),
            text: String::new(),
            tools: BTreeMap::new(),
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
        });
        handler.update(Update::ToolCallUpdate {
            id: "read-1".into(),
            title: None,
            status: Some("completed".into()),
            text: Some("contents".into()),
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
        agent.name = "Devin".into();
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
        assert_eq!(
            acp_client::replay::arguments(dir.path()),
            vec!["acp", "--model", "swe-2-high"]
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
        let _ = acp(
            &devin_agent(program),
            "Write result.txt.",
            dir.path(),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            acp_client::replay::arguments(dir.path()),
            vec!["--sandbox", "acp"]
        );
        let sent = acp_client::replay::received(dir.path());
        let set_mode = sent
            .iter()
            .find(|frame| frame["method"] == "session/set_mode")
            .expect("a set_mode");
        assert_eq!(set_mode["params"]["modeId"], "accept-edits");
        desk.close();
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
        assert_eq!(result["tokens"], 35);
        assert_eq!(result["outcome"]["ending"]["reason"], "finished");
        assert!(
            events.iter().any(
                |event| matches!(event,RuntimeEvent::Model(model) if model == "fixture/model")
            )
        );
    }

    #[tokio::test]
    async fn canceling_generation_does_not_wait_for_the_provider() {
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
        use microcoder_loop::models::{Ask, Basis, Generated, NextAction};
        const MARKER: &str = "fixture-saved-provider-credential";
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

    #[test]
    fn gateway_context_keeps_the_task_and_latest_results_without_splitting_unicode() {
        let prompt = format!(
            "TASK: review these files\n{}\nLATEST: changed file",
            "界".repeat(25_000)
        );
        let compact = gateway_prompt(&prompt);
        assert!(compact.starts_with("TASK: review these files"));
        assert!(compact.ends_with("LATEST: changed file"));
        assert!(compact.contains("observations were compacted"));
        assert!(compact.len() < 48 * 1024);
        assert_eq!(gateway_prompt("A short prompt"), "A short prompt");
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
        let providers = GeneratorChain {
            providers: vec![
                FixtureProvider::Unavailable(Arc::clone(&local_calls)),
                FixtureProvider::Gateway(coder::cloud::CloudLane::new(Arc::clone(&fixture))),
            ],
            current: Cell::new(0),
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
}
