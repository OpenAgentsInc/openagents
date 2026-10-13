//! The `claude` mode: turns run on the Claude Code login this
//! computer already holds, through the Claude Agent SDK
//! (`crates/claude_agent_sdk`).
//!
//! The mode's own turns are full Claude Code inside Coder (#11434, #11440):
//! the CLI's whole default toolset, every call approved (the
//! `--dangerously-skip-permissions` equivalent, owner 2026-10-12), and the
//! person's and the repository's settings, CLAUDE.md, skills, hooks, agents
//! and MCP servers loaded without a trust prompt. The model is the one the
//! person picked in /models ([`choice`]), Haiku 5.5 until they pick. Beside
//! Claude Code's tools, one in-process server offers Coder's own: background
//! agents, Solver, cloud jobs and the rest a live chat can hand work to
//! ([`crate::claude_tools`]). A tool call is drawn with Coder's own
//! widgets, the same shapes the briefed agent makes
//! (`crate::issue_run::agent`), so a Read shows a line count, an Edit shows
//! a diff, and Grep and Glob show their matches.
//!
//! [`run_with`] stays narrow: Solver's conversational Intaker asks for a
//! few file tools on Haiku, and its handler keeps every call inside the
//! checkout.
//!
//! Whether the mode may be offered is read through
//! [`crate::bundled_runtime::claude_print`] and `coder_engine_status`, which
//! report the credential's kind and never the credential.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use claude_agent_sdk::{
    PermissionMode, PermissionResult, QueryOptions, SdkMessage, SdkResultMessage, SdkSystemMessage,
    SettingSource, SystemPromptConfig, ThinkingConfig, ThinkingDisplay, ToolsConfig,
};
use futures::StreamExt;
use openrouter::{Message, Streamed, Usage};
use serde_json::Value;

use crate::bundled_runtime::RuntimeEvent;

/// The mode's name, in the rail and in the conversation.
pub const LABEL: &str = "claude";

/// The plugin that offers the mode, in /plugins and `bundled-plugins.json`.
/// It turns on only on a computer signed in to Claude Code, and new chats
/// run on the Claude Code harness only while it is on.
pub const PLUGIN: &str = "claude-code";

/// What /plugins says when the plugin cannot turn on here.
pub const SIGN_IN_FIRST: &str = "Sign in to Claude Code first";

/// What `coder chat --mode claude` says on a computer with no Claude
/// account login.
pub const NEEDS_LOGIN: &str = "claude needs Claude Code signed in on this computer.";

/// The model a claude turn runs before the person picks one, and the one
/// [`run_with`] always runs: Haiku 5.5.
pub const MODEL: &str = "claude-haiku-5-5";

/// What the mode calls [`MODEL`] where a person reads it.
pub const MODEL_LABEL: &str = "Haiku 5.5";

/// The picker rows' plugin name, so /models knows a claude row from an
/// OpenRouter one.
pub const PICKER: &str = "claude";

/// The models /models offers in this mode, [`MODEL`] first: id, name, and
/// what the row says.
pub const MODELS: [(&str, &str, &str); 4] = [
    (
        MODEL,
        MODEL_LABEL,
        "Fastest; the default while the mode is tested",
    ),
    (
        "claude-sonnet-5-5",
        "Sonnet 5.5",
        "Balanced speed and intelligence",
    ),
    (
        "claude-opus-5-5",
        "Opus 5.5",
        "Most capable for coding and agents",
    ),
    ("claude-fable-5-1", "Fable 5.1", "Anthropic's largest model"),
];

/// The efforts a picked model may run at, as Claude Code's `--effort`
/// names them.
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// Where the person's pick is kept, beside Coder's other settings.
const CHOICE_FILE: &str = "claude-model.json";

/// The file tools [`run_with`] may offer, the same names Coder's own file
/// tools use.
pub const TOOLS: [&str; 5] = ["Read", "Write", "Edit", "Glob", "Grep"];

/// What a stopped turn says.
pub const STOPPED: &str = "The turn was stopped.";

/// Credentials and provider switches the CLI must not inherit, so the session
/// uses the Claude Code login this computer holds rather than an API key or a
/// cloud provider. The three `CLAUDE_CODE_USE_*` names are the ones
/// [`crate::claude_plan::plan_login`] treats as leaving the plan.
const REMOVED_ENV: [&str; 5] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

/// The most of a conversation one turn carries into its prompt.
const TASK_LIMIT: usize = 48 * 1024;

/// The model and effort the mode's own turns run, as the person picked
/// them in /models.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub model: String,
    /// Claude Code's `--effort`, or `None` for the model's own default.
    pub effort: Option<String>,
}

impl Default for Choice {
    fn default() -> Self {
        Self {
            model: MODEL.into(),
            effort: None,
        }
    }
}

/// The pick, and where it is saved once Coder's settings have loaded.
static CHOICE: std::sync::Mutex<(Option<Choice>, Option<PathBuf>)> =
    std::sync::Mutex::new((None, None));

#[derive(serde::Deserialize, serde::Serialize)]
struct ChoiceFile {
    version: u32,
    model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    effort: Option<String>,
}

/// The model and effort the next claude turn runs.
#[must_use]
pub fn choice() -> Choice {
    CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .0
        .clone()
        .unwrap_or_default()
}

/// A new chat starts on the default model, not an earlier chat's pick
/// (#11556); the saved pick stays on disk.
pub fn reset_choice() {
    CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .0 = None;
}

/// Reads the saved pick under `root`, Coder's settings folder. A missing
/// file, or one naming a model the mode does not offer, leaves Haiku 5.5.
pub fn load_choice(root: &Path) {
    let path = root.join(CHOICE_FILE);
    let saved = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ChoiceFile>(&bytes).ok())
        .filter(|file| file.version == 1 && offered(&file.model))
        .map(|file| Choice {
            model: file.model,
            effort: file
                .effort
                .filter(|effort| EFFORTS.contains(&effort.as_str())),
        });
    *CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = (saved, Some(path));
}

/// Makes `model` at `effort` the mode's pick and saves it.
///
/// # Errors
/// The mode does not offer `model`, or the pick could not be saved; the
/// earlier pick stays.
pub fn choose(model: &str, effort: Option<String>) -> Result<(), String> {
    if !offered(model) {
        return Err(format!("claude does not offer {model}."));
    }
    let effort = effort.filter(|effort| EFFORTS.contains(&effort.as_str()));
    let mut held = CHOICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(path) = &held.1 {
        let file = ChoiceFile {
            version: 1,
            model: model.into(),
            effort: effort.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&file).map_err(|error| error.to_string())?;
        std::fs::write(path, bytes)
            .map_err(|_| "Cannot save the claude model. The earlier one stays.".to_string())?;
    }
    held.0 = Some(Choice {
        model: model.into(),
        effort,
    });
    Ok(())
}

/// The model a chat turn runs on: a pin's `model` (#11475), else the
/// person's current pick in /models.
#[must_use]
pub fn turn_model(pinned: Option<String>) -> String {
    pinned.unwrap_or_else(|| choice().model)
}

/// What a person reads for `model`: its name in /models ("Opus 5.5"), or
/// the id's own short form for a model the mode does not list.
#[must_use]
pub fn name(model: &str) -> String {
    MODELS.iter().find(|(id, _, _)| *id == model).map_or_else(
        || crate::models::label(model),
        |(_, name, _)| (*name).to_owned(),
    )
}

/// Whether /models offers `model` in this mode.
fn offered(model: &str) -> bool {
    MODELS.iter().any(|(id, _, _)| *id == model)
}

/// What /models lists in this mode.
#[must_use]
pub fn catalog() -> Vec<crate::models::Model> {
    MODELS
        .iter()
        .map(|(id, name, description)| crate::models::Model {
            plugin: PICKER.into(),
            provider: "Anthropic".into(),
            id: (*id).into(),
            name: (*name).into(),
            description: (*description).into(),
            context_length: None,
            max_output_tokens: None,
            efforts: EFFORTS.iter().map(|effort| (*effort).to_owned()).collect(),
            default_effort: None,
            supports_output_limit: false,
            group: None,
        })
        .collect()
}

/// Whether `tool` is one of the mode's [`TOOLS`].
#[must_use]
pub fn allows(tool: &str) -> bool {
    TOOLS.contains(&tool)
}

/// The prompt one turn sends: Coder's own identity note, then the
/// conversation, within [`TASK_LIMIT`].
#[must_use]
pub fn task(messages: &[Message]) -> String {
    task_with(
        messages,
        &crate::claude_tools::Tools::default(),
        &choice().model,
    )
}

/// [`task`], telling the model about Coder's own `tools`
/// ([`crate::claude_tools`]), for a turn on `model`.
#[must_use]
pub fn task_with(messages: &[Message], tools: &crate::claude_tools::Tools, model: &str) -> String {
    let tools = crate::claude_tools::instructions(tools);
    let prefix = format!(
        "{}\n{tools}The conversation:\n",
        crate::identity::prompt(model)
    );
    let allowance = TASK_LIMIT.saturating_sub(prefix.len());
    let mut conversation = crate::live::local_conversation(messages, allowance);
    // A newest message larger than the allowance fits no row at all, so the
    // model would never see the request. Its head and tail carry it instead.
    if conversation.is_empty()
        && let Some(last) = messages.last()
    {
        let role = format!("{}: ", last.role);
        conversation = format!(
            "{role}{}",
            clip(&last.content, allowance.saturating_sub(role.len()))
        );
    }
    format!("{prefix}{conversation}")
}

/// `text` within `limit` bytes: its head and tail with a marker between,
/// cut on character boundaries.
fn clip(text: &str, limit: usize) -> String {
    const MARKER: &str = "\n[...]\n";
    if text.len() <= limit {
        return text.to_owned();
    }
    // Too tight for the marker to fit: the head alone stands in.
    if limit <= MARKER.len() {
        let head = head_within(text, limit);
        return text[..head].to_owned();
    }
    let kept = limit.saturating_sub(MARKER.len());
    let head = head_within(text, kept / 2);
    let mut tail = text.len() - (kept - kept / 2);
    while !text.is_char_boundary(tail) {
        tail += 1;
    }
    format!("{}{MARKER}{}", &text[..head], &text[tail..])
}

/// The largest index no greater than `limit` that is a character boundary
/// of `text`.
fn head_within(text: &str, limit: usize) -> usize {
    let mut head = limit.min(text.len());
    while !text.is_char_boundary(head) {
        head -= 1;
    }
    head
}

/// Whether the file tools of one call stay inside `root`: the path each
/// names resolves under it, symlinks included, and a Glob pattern names
/// nothing above it.
///
/// The check runs when the call is admitted, before the tool executes, so a
/// symlink swapped in between the two is not caught. The window is small and
/// the CLI sandbox is the wider guard; this is the mode's own boundary.
fn stays_in(root: &Path, tool: &str, input: &Value) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let paths_inside = ["file_path", "path"]
        .iter()
        .filter_map(|key| input[*key].as_str())
        .all(|path| inside(&root, Path::new(path)));
    let pattern_inside = tool != "Glob"
        || input["pattern"]
            .as_str()
            .is_none_or(|pattern| !Path::new(pattern).is_absolute() && !names_a_parent(pattern));
    paths_inside && pattern_inside
}

/// Whether a Glob pattern walks up a directory, `..` written as a whole
/// path segment (`../x`, `a/../x`) rather than inside a name (`foo..bar`).
fn names_a_parent(pattern: &str) -> bool {
    pattern.split(['/', '\\']).any(|segment| segment == "..")
}

/// Whether `path`, read from `root` when relative, lands under the canonical
/// `root`. `..` is folded away first, then the longest part that exists is
/// resolved through the disk so a symlink cannot lead out of the checkout.
fn inside(root: &Path, path: &Path) -> bool {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let mut lexical = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                lexical.pop();
            }
            Component::CurDir => {}
            other => lexical.push(other.as_os_str()),
        }
    }
    let mut existing = lexical.as_path();
    let mut missing = Vec::new();
    while existing.symlink_metadata().is_err() {
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_owned());
                existing = parent;
            }
            _ => return false,
        }
    }
    let Ok(mut real) = existing.canonicalize() else {
        return false;
    };
    for name in missing.iter().rev() {
        real.push(name);
    }
    real.starts_with(root)
}

static AVAILABLE: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

/// Whether this computer has Claude Code on a Claude account login: the
/// `claude` program is present, and it is signed in to a Claude account
/// rather than with an API key or a cloud credential.
///
/// The answer is read once and kept, because the check runs `claude auth
/// status`, which is the command that names the credential's kind. A chat
/// here runs interactively on the person's plan, not on the plan's
/// automated-run book, so it never takes a plan slot. A turn that will not
/// start calls [`recheck`], so a sign-out after startup stops the mode from
/// being offered rather than failing every turn.
#[must_use]
pub fn available() -> bool {
    let mut cache = AVAILABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(known) = *cache {
        return known;
    }
    let known = probe();
    *cache = Some(known);
    known
}

/// What [`available`] already found this run, without asking the
/// computer or waiting for a check under way: `None` before it has.
#[must_use]
pub fn known() -> Option<bool> {
    match AVAILABLE.try_lock() {
        Ok(cache) => *cache,
        Err(std::sync::TryLockError::Poisoned(cache)) => *cache.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => None,
    }
}

/// Forget the cached answer, so the next [`available`] asks the computer
/// again.
pub fn recheck() {
    *AVAILABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// Ask the computer whether the login is there, without touching the cache.
fn probe() -> bool {
    let Some(agent) = crate::bundled_runtime::claude_print::agent() else {
        return false;
    };
    let status = coder_engine_status::claude::check(&agent.program, None, atif::now_ms() / 1_000);
    matches!(
        status.state,
        coder_engine_status::State::SignedIn | coder_engine_status::State::Expiring
    ) && matches!(
        status.method,
        Some(coder_engine_status::Method::ClaudeAi | coder_engine_status::Method::OauthToken)
    )
}

/// The saved session `id` a chat's Claude Code process continues.
/// `note` is Coder's identity note, appended to Claude Code's own system
/// prompt so the model still knows whose agent it is.
pub(crate) struct Resume {
    pub(crate) id: String,
    pub(crate) note: String,
}

/// Whether Claude Code still keeps session `id` on this computer: a
/// `<id>.jsonl` under one of its project folders (`~/.claude/projects`, or
/// `CLAUDE_CONFIG_DIR`). A resume of a session it lost waits out the start
/// before failing, so a chat whose session is gone starts anew at once.
#[must_use]
pub fn kept(id: &str) -> bool {
    let root = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")));
    let Some(projects) = root.map(|root| root.join("projects")) else {
        return false;
    };
    let file = format!("{id}.jsonl");
    // An id is a UUID; anything that could name a path is not one.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return false;
    }
    std::fs::read_dir(projects)
        .into_iter()
        .flatten()
        .flatten()
        .any(|project| project.path().join(&file).is_file())
}

/// Coder's identity note for a claude chat on `model`, with what it is
/// told about Coder's own `tools`.
pub(crate) fn note(tools: &crate::claude_tools::Tools, model: &str) -> String {
    let tools = crate::claude_tools::instructions(tools);
    format!("{}\n{tools}", crate::identity::prompt(model))
}

/// A chat's Claude Code process on streaming input, ready for its first
/// message, and the Solver delegations its tool server reports.
pub(crate) struct Opened {
    pub(crate) query: claude_agent_sdk::Query,
    pub(crate) delegations: Option<Delegations>,
}

/// Starts the long-lived Claude Code process of one claude chat
/// ([`crate::claude_chat`]): Claude Code's whole toolset, the session saved,
/// continuing `resume` when the chat holds one, and Coder's own `tools`
/// beside the rest ([`crate::claude_tools`]).
///
/// # Errors
/// Claude Code did not start, or this host refuses every tool.
pub(crate) async fn open(
    cwd: &Path,
    tools: &crate::claude_tools::Tools,
    resume: Option<Resume>,
    asks: crate::claude_ask::Asks,
    briefing: Option<&crate::context::Briefing>,
    model: Option<&str>,
) -> Result<Opened, String> {
    if !crate::approval::tools_allowed() {
        return Err("This host refuses every tool, so claude cannot work here.".into());
    }
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let (server, delegations) = match crate::claude_tools::server(tools, sender, stop.clone()) {
        Some(server) => (Some(server), Some(Delegations { receiver, stop })),
        None => (None, None),
    };
    let (mut options, _) = turn_options(cwd, Access::Full, server);
    // A pin's model (#11475), else the person's pick in /models.
    if let Some(model) = model {
        options = options.model(model);
    }
    let options = briefed(with_session(options, resume), briefing);
    let query = claude_agent_sdk::Query::streaming(options, Some(full_handler(asks)))
        .await
        .map_err(|error| {
            recheck();
            format!("Claude Code did not start: {error}")
        })?;
    Ok(Opened { query, delegations })
}

/// What a chat's permission handler decides: every call approved, and a
/// question for the person waits for their answer on the choice screen
/// ([`crate::claude_ask`], #11449), which goes back as the tool's updated
/// input.
fn full_handler(
    asks: crate::claude_ask::Asks,
) -> std::sync::Arc<dyn claude_agent_sdk::PermissionHandler> {
    claude_agent_sdk::permission_handler(move |request| {
        let asks = asks.clone();
        async move {
            Ok::<_, claude_agent_sdk::Error>(if request.tool_name == ASKS_PERSON {
                match asks.ask(&request.input).await {
                    Some(answered) => PermissionResult::allow(answered),
                    None => PermissionResult::deny(
                        "The person chose to answer in the chat. Ask in your reply instead.",
                    ),
                }
            } else {
                PermissionResult::allow(request.input.clone())
            })
        }
    })
}

/// A message as the content blocks Claude Code reads: the text, then each
/// image or PDF its attachment note lines name (`[Image #1: /path]`), so a
/// pasted picture reaches the model rather than only its path.
#[must_use]
pub fn content(text: &str) -> Vec<Value> {
    let mut blocks = vec![claude_agent_sdk::text_block(text)];
    for path in crate::attachments::noted(text) {
        let Ok(attachment) = crate::attachments::from_path(&path) else {
            continue;
        };
        let data = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &attachment.bytes,
        );
        blocks.push(if attachment.is_image() {
            claude_agent_sdk::image_block(attachment.media_type, data)
        } else {
            serde_json::json!({
                "type": "document",
                "source": {"type": "base64", "media_type": attachment.media_type, "data": data},
            })
        });
    }
    blocks
}

/// One running tool call, kept between its `tool_use` and its `tool_result`.
struct Call {
    /// The subagent call this one runs under, when a subagent made it.
    parent: Option<String>,
    coder_name: String,
    coder_input: Value,
    sdk_input: Value,
    /// When the call reached Coder: a Bash call's file changes count only
    /// from then (`crate::shell_changes::Snapshot::changes_during`).
    started: std::time::SystemTime,
}

/// Runs one claude turn to its end, emitting the reply's text, the model
/// the engine reported, and each tool call as Coder's own shapes.
///
/// # Errors
/// The turn failed: Claude Code did not start, the login is missing or
/// limited, or the turn was stopped.
pub async fn run(
    prompt: &str,
    cwd: &Path,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Streamed, String> {
    run_inner(prompt, cwd, Access::Full, None, None, None, cancel, emit).await
}

/// Runs one narrow Haiku turn, with `tools` from [`TOOLS`] alone and,
/// beside them, the tools of an in-process `server` (the solver's
/// conversational Intaker gives it a read-only `gh`).
///
/// # Panics
/// If `tools` names anything outside [`TOOLS`]. The mode offers only file
/// tools; a caller that asked for a shell here would widen what the mode can
/// do without its own approval path. The check is a developer guard, not a
/// runtime condition.
///
/// # Errors
/// As [`run`].
pub async fn run_with(
    prompt: &str,
    cwd: &Path,
    tools: &[&str],
    server: Option<claude_agent_sdk::SdkMcpServer>,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Streamed, String> {
    assert!(
        tools.iter().all(|tool| allows(tool)),
        "run_with may take only the file tools, got {tools:?}"
    );
    run_inner(
        prompt,
        cwd,
        Access::Files(tools),
        server,
        None,
        None,
        cancel,
        emit,
    )
    .await
}

/// Runs one turn like [`run`], with the Solver plugin's tool beside the
/// file tools: an issue the model hands it runs as a delegation whose
/// progress reaches `emit` while the turn waits, and stopping the turn
/// stops the run ([`crate::solver_plugin`]).
///
/// # Errors
/// As [`run`].
pub async fn run_with_solver(
    prompt: &str,
    cwd: &Path,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Streamed, String> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let server = crate::solver_plugin::mcp_server(cwd.to_owned(), sender, stop.clone());
    let delegations = Delegations { receiver, stop };
    run_inner(
        prompt,
        cwd,
        Access::Full,
        Some(server),
        Some(delegations),
        None,
        cancel,
        emit,
    )
    .await
}

/// Runs a delegated task (#11511): the child's briefing as one turn, on
/// `model` when the call named one, under the session's hooks and floors.
/// A read-only task gets only the reading tools, each call held inside the
/// folder; another gets Claude Code's whole toolset in its worktree.
///
/// # Errors
/// As [`run`].
pub async fn run_delegated(
    prompt: &str,
    cwd: &Path,
    model: Option<&str>,
    read_only: bool,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Streamed, String> {
    const READING: [&str; 3] = ["Read", "Glob", "Grep"];
    let access = if read_only {
        Access::Files(&READING)
    } else {
        Access::Full
    };
    run_inner(prompt, cwd, access, None, None, model, cancel, emit).await
}

/// What one turn may do.
#[derive(Clone, Copy)]
pub(crate) enum Access<'a> {
    /// The mode's own turns: Claude Code's whole default toolset, every
    /// call approved, the person's and the repository's settings loaded, on
    /// the person's pick ([`choice`]).
    Full,
    /// Solver's Intaker: only these file tools, each call inside the
    /// checkout, on Haiku 5.5.
    Files(&'a [&'a str]),
}

/// The tool that asks the person: a chat answers it on the choice screen
/// ([`crate::claude_ask`]); a one-off turn, with nobody to answer, refuses
/// it so the model asks in its reply instead.
const ASKS_PERSON: &str = "AskUserQuestion";

/// The options and the tool names one turn may use.
///
/// A narrow turn leaves `allowed_tools` unset on purpose. Claude Code's
/// `--allowedTools` pre-approves the tool for every call, so the CLI never
/// sends `can_use_tool` and the permission handler never runs; a tool named
/// there could read or write any path. `--tools` alone narrows the offered
/// tools without pre-approving them, so every call still comes to the
/// handler, which is what holds the checkout boundary.
pub(crate) fn turn_options(
    cwd: &Path,
    access: Access<'_>,
    server: Option<claude_agent_sdk::SdkMcpServer>,
) -> (QueryOptions, Vec<String>) {
    let mut options = QueryOptions::new().cwd(cwd).no_session_persistence();
    let mut allowed = Vec::new();
    match access {
        Access::Full => {
            let picked = choice();
            // The box, through the Coder SDK (#11496): the person's model
            // and effort, Claude Code's own tools and system prompt, and
            // the person's and the repository's settings.
            options = coder_sdk::claude::SessionBox {
                model: Some(picked.model),
                effort: picked.effort,
                setting_sources: Some(vec![
                    SettingSource::User,
                    SettingSource::Project,
                    SettingSource::Local,
                ]),
                ..coder_sdk::claude::SessionBox::default()
            }
            .apply(options)
            .permission_mode(PermissionMode::BypassPermissions);
            options.allow_dangerously_skip_permissions = true;
            options.tools = Some(ToolsConfig::Default);
            // The reply's words as they arrive, and the model's thinking in
            // words, so the conversation shows both (#11438).
            options.include_partial_messages = true;
            options.thinking = Some(ThinkingConfig::Adaptive {
                display: Some(ThinkingDisplay::Summarized),
            });
        }
        Access::Files(tools) => {
            options = options
                .model(MODEL)
                .permission_mode(PermissionMode::Default);
            allowed = tools.iter().map(|t| (*t).to_owned()).collect();
            options.tools = Some(ToolsConfig::Names(allowed.clone()));
            options.setting_sources = Some(Vec::new());
            options.strict_mcp_config = true;
        }
    }
    if let Some(server) = server {
        allowed.extend(server.allowed_tool_names());
        options = options.sdk_mcp_server(server);
    }
    // The session's Coder SDK hooks (#11496), composed by one runner: the
    // floors (#11475) hold a production deploy, a merge, a secret or a
    // payment for the person before the tool runs, since under bypass
    // Claude Code never asks; on a full turn the token savers' squeezer
    // (#11486, #11494) wraps a noisy build or test command first, and the
    // floors judge the wrapped command; the gate (#11489) can only add a
    // deny, and the first touch of a directory adds its instruction files
    // (#11490), all in the same registry (#11531).
    options = crate::sdk_hooks::claude(cwd, matches!(access, Access::Full))
        .install(options, coder_sdk::claude::PreTool::Hook);
    options.env_remove = REMOVED_ENV.iter().map(|name| (*name).to_owned()).collect();
    (options, allowed)
}

/// Delegation events a tool server sends while the turn waits on it, and
/// the flag that stops what it runs when the turn ends.
pub(crate) struct Delegations {
    receiver: tokio::sync::mpsc::UnboundedReceiver<RuntimeEvent>,
    stop: std::sync::Arc<AtomicBool>,
}

impl Drop for Delegations {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

async fn run_inner(
    prompt: &str,
    cwd: &Path,
    access: Access<'_>,
    server: Option<claude_agent_sdk::SdkMcpServer>,
    mut delegations: Option<Delegations>,
    model: Option<&str>,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Streamed, String> {
    if !crate::approval::tools_allowed() {
        return Err("This host refuses every tool, so claude cannot work here.".into());
    }
    let (mut options, allowed) = turn_options(cwd, access, server);
    if let Some(model) = model {
        options = options.model(model);
    }
    // A delegated child's place in the tree (#11511) and the variables
    // this thread's children get.
    let vars = crate::bundled_runtime::child_env();
    if !vars.is_empty() {
        let env = options.env.get_or_insert_with(Default::default);
        for (name, value) in vars {
            env.insert(
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            );
        }
    }
    let root = cwd.to_owned();
    let full = matches!(access, Access::Full);
    let handler = claude_agent_sdk::permission_handler(move |request| {
        let decision = if full && request.tool_name == ASKS_PERSON {
            PermissionResult::deny("Ask the person in your reply instead; they answer there.")
        } else if full {
            PermissionResult::allow(request.input.clone())
        } else if !allowed.iter().any(|name| *name == request.tool_name) {
            PermissionResult::deny("That tool is not available in this mode.")
        } else if !stays_in(&root, &request.tool_name, &request.input) {
            PermissionResult::deny("That path is outside this checkout.")
        } else {
            PermissionResult::allow(request.input.clone())
        };
        async move { Ok::<_, claude_agent_sdk::Error>(decision) }
    });
    let mut query = claude_agent_sdk::query_with_permissions(prompt, options, handler)
        .await
        .map_err(|error| {
            // A start that fails is often a login that went away after
            // startup, so the next look at availability asks again rather
            // than trusting the answer read at startup.
            recheck();
            format!("Claude Code did not start: {error}")
        })?;
    read_turn(
        &mut query,
        cwd,
        &mut delegations,
        false,
        cancel,
        &mut |_| {},
        emit,
    )
    .await
}

/// Reads one turn of `query` to its `result`, emitting the reply's text, the
/// model the engine reported, and each tool call as Coder's own shapes.
/// `report` hears the session id with the turn's first message.
///
/// A one-off turn (`keep` false) ends its process with the turn. A chat's
/// long-lived process (`keep`, [`crate::claude_chat`]) stays up after the
/// result, and a stopped turn is interrupted rather than ended.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn read_turn(
    query: &mut claude_agent_sdk::Query,
    cwd: &Path,
    delegations: &mut Option<Delegations>,
    keep: bool,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(&str),
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Streamed, String> {
    let mut started = Instant::now();
    let mut waiting = keep;
    let mut reply = Streamed::default();
    let mut calls: HashMap<String, Call> = HashMap::new();
    let mut first_text_ms = None;
    let mut reported: Option<String> = None;
    let mut stream = crate::claude_stream::Stream::default();
    // The checkout's changed files, so a Bash call that edits a file shows
    // its diff (`crate::shell_changes`).
    let mut changed: Option<crate::shell_changes::Snapshot> = None;
    let mut snapshot = Some(tokio::task::spawn_blocking({
        let cwd = cwd.to_path_buf();
        move || crate::shell_changes::Snapshot::take(&cwd)
    }));
    loop {
        if cancel.load(Ordering::Relaxed) {
            stop(query, keep).await;
            return Err(STOPPED.into());
        }
        let next = match delegations.as_mut() {
            None => tokio::select! {
                message = query.next() => message,
                // Look at `cancel` while the model is quiet, as with a delegation.
                () = tokio::time::sleep(std::time::Duration::from_millis(250)) => continue,
            },
            Some(delegations) => tokio::select! {
                biased;
                Some(event) = delegations.receiver.recv() => {
                    emit(event);
                    continue;
                }
                message = query.next() => message,
                // Look at `cancel` while a delegation works quietly.
                () = tokio::time::sleep(std::time::Duration::from_millis(250)) => continue,
            },
        };
        let Some(message) = next else {
            break;
        };
        // A long-lived process waits between turns; the turn's time starts
        // with its first message, not with the wait.
        if waiting {
            waiting = false;
            started = Instant::now();
        }
        // The session id arrives with Claude Code's first message. It is
        // reported at once, so a turn stopped part way still leaves the chat
        // holding its session.
        if let Some(id) = query.session_id()
            && reported.as_deref() != Some(id)
        {
            reported = Some(id.to_owned());
            report(id);
        }
        match message {
            Err(claude_agent_sdk::Error::UnrecognizedMessage { .. }) => continue,
            Err(error) => {
                let _ = query.kill().await;
                return Err(error.to_string());
            }
            // Where the Claude plan's 5-hour and weekly windows stand, for
            // the line under the input ([`crate::claude_plan::status_text`]).
            Ok(SdkMessage::RateLimitEvent(event)) => {
                if let Ok(info) = serde_json::to_value(&event.rate_limit_info) {
                    crate::claude_plan::observe_rate_limit(&info, coder_delegate::limit::now());
                }
                if let Some(usage) = crate::claude_plan::plan_usage() {
                    emit(RuntimeEvent::Plan(usage));
                }
            }
            Ok(SdkMessage::StreamEvent(event)) => {
                if let Some(thinking) = stream.thinking(&event) {
                    emit(RuntimeEvent::Thinking(thinking));
                }
                for thought in stream.thought(&event) {
                    emit(thought);
                }
                if let Some(text) = stream.partial(&event) {
                    if first_text_ms.is_none() {
                        first_text_ms = Some(elapsed_ms(started));
                    }
                    emit(RuntimeEvent::Text(text));
                }
            }
            // A subagent's progress and end, background ones included,
            // which settle after the call that started them returned
            // (#11482).
            Ok(SdkMessage::System(SdkSystemMessage::TaskProgress(progress))) => {
                if let Some(id) = progress.tool_use_id.as_deref() {
                    emit(stream.agent_progress(id, progress.usage.total_tokens));
                }
            }
            Ok(SdkMessage::System(SdkSystemMessage::TaskNotification(settled))) => {
                if let Some(id) = settled.tool_use_id.as_deref() {
                    emit(stream.agent_settled(
                        id,
                        &settled.status,
                        &settled.summary,
                        settled.usage.as_ref().map_or(0, |usage| usage.total_tokens),
                    ));
                }
            }
            Ok(SdkMessage::System(SdkSystemMessage::ApiRetry(retry))) => {
                emit(crate::claude_stream::retry(&retry));
            }
            Ok(SdkMessage::System(SdkSystemMessage::Init(init))) => {
                // What failed to load flashes on the input's rail, never as
                // a transcript row.
                crate::claude_stream::note_load_problems(&init);
            }
            Ok(SdkMessage::Assistant(assistant)) => {
                // A subagent's message goes to its own delegation.
                let parent = assistant.parent_tool_use_id.as_deref();
                if parent.is_none() {
                    if let Some(model) = assistant.message["model"]
                        .as_str()
                        .filter(|m| !m.is_empty())
                    {
                        reply.model = model.to_owned();
                        emit(RuntimeEvent::Model(model.to_owned()));
                    }
                    stream.note_usage(&assistant.message);
                }
                // Words that already arrived one by one are not shown again.
                let streamed = parent.is_none() && stream.already_streamed(&assistant.message);
                for block in assistant.message["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    match block["type"].as_str() {
                        Some("text") => {
                            let text = block["text"].as_str().unwrap_or_default();
                            if text.is_empty() {
                                continue;
                            }
                            if parent.is_some() {
                                emit(stream.under(parent, RuntimeEvent::Text(text.to_owned())));
                                continue;
                            }
                            if first_text_ms.is_none() {
                                first_text_ms = Some(elapsed_ms(started));
                            }
                            reply.text.push_str(text);
                            if !streamed {
                                emit(RuntimeEvent::Text(text.to_owned()));
                            }
                        }
                        Some("thinking") if parent.is_none() => {
                            if let Some(thinking) = stream.thoughts.block(block) {
                                emit(thinking);
                            }
                        }
                        Some("thinking") => {
                            if let Some(thinking) = crate::claude_stream::thinking(block) {
                                emit(stream.under(parent, thinking));
                            }
                        }
                        Some("tool_use") => {
                            let name = block["name"].as_str().unwrap_or_default();
                            // The delegation's own entry stands for this call.
                            if crate::claude_tools::draws_itself(name) {
                                continue;
                            }
                            // A subagent shows as a delegation of its own.
                            if crate::claude_stream::is_agent_tool(name)
                                && let Some(id) = block["id"].as_str()
                            {
                                emit(stream.start_agent(id, &block["input"]));
                                continue;
                            }
                            let sdk_input = block["input"].clone();
                            let (coder_name, coder_input) =
                                crate::issue_run::agent::coder_call(name, &sdk_input, cwd, &[]);
                            // An MCP tool reads by its plain name, never
                            // `mcp__server__tool`.
                            let coder_name = crate::claude_tools::display_name(&coder_name);
                            emit(stream.under(
                                parent,
                                RuntimeEvent::Tool {
                                    name: coder_name.clone(),
                                    input: coder_input.clone(),
                                    output: Value::Null,
                                    running: true,
                                },
                            ));
                            if let Some(id) = block["id"].as_str() {
                                calls.insert(
                                    id.to_owned(),
                                    Call {
                                        parent: parent.map(str::to_owned),
                                        coder_name,
                                        coder_input,
                                        sdk_input,
                                        started: std::time::SystemTime::now(),
                                    },
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(SdkMessage::User(user)) => {
                for block in user.message["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|block| block["type"] == "tool_result")
                {
                    let Some(id) = block["tool_use_id"].as_str() else {
                        continue;
                    };
                    let failed = block["is_error"].as_bool().unwrap_or(false);
                    if let Some(end) = stream.end_agent(
                        id,
                        &crate::issue_run::agent::result_text(block),
                        failed,
                        user.tool_use_result.as_ref(),
                    ) {
                        emit(end);
                        continue;
                    }
                    let Some(call) = calls.remove(id) else {
                        continue;
                    };
                    let structured = user.tool_use_result.as_ref().filter(|v| v.is_object());
                    let output = crate::issue_run::agent::coder_output(
                        &call.coder_name,
                        &call.sdk_input,
                        &call.coder_input,
                        &crate::issue_run::agent::result_text(block),
                        structured,
                        failed,
                    );
                    if let Some(task) = snapshot.take() {
                        changed = task.await.ok().flatten();
                    }
                    let edited = matches!(call.coder_name.as_str(), "Edit" | "Write");
                    let shell = call.coder_name == "Bash";
                    emit(stream.under(
                        call.parent.as_deref(),
                        RuntimeEvent::Tool {
                            name: call.coder_name,
                            input: call.coder_input,
                            output,
                            running: false,
                        },
                    ));
                    // An Edit or Write drew its own diff; a Bash call's
                    // file changes show as diffs of their own.
                    if edited
                        && let Some(before) = changed.as_mut()
                        && let Some(path) = crate::shell_changes::edited_path(&call.sdk_input)
                    {
                        before.refresh(&path);
                    }
                    if shell && let Some(before) = changed.take() {
                        // Only what changed while the command ran is its
                        // work; the checkout is shared with other agents.
                        let (from, to) = (call.started, std::time::SystemTime::now());
                        let (changes, now) =
                            tokio::task::spawn_blocking(move || before.changes_during(from, to))
                                .await
                                .unwrap_or_default();
                        changed = now;
                        for change in changes {
                            emit(stream.under(call.parent.as_deref(), change.event()));
                        }
                    }
                }
            }
            Ok(SdkMessage::Result(result)) => {
                for thought in stream.thoughts.end() {
                    emit(thought);
                }
                if !keep {
                    let _ = query.kill().await;
                }
                if let Some(delegations) = delegations.as_mut() {
                    while let Ok(event) = delegations.receiver.try_recv() {
                        emit(event);
                    }
                }
                if let SdkResultMessage::Success(success) = &result {
                    stream.record_context(query.session_id(), &reply.model, &success.model_usage);
                }
                let mut done = finish(result)?;
                // The model that wrote this turn's reply. A long-lived
                // session's usage counts every model it ran, so after a
                // switch it still lists the first one (#11480).
                if !reply.model.is_empty() {
                    done.model.clone_from(&reply.model);
                }
                done.milliseconds = elapsed_ms(started);
                done.first_text_ms = first_text_ms;
                return Ok(done);
            }
            Ok(SdkMessage::Unknown { type_name, raw }) if type_name == "result" => {
                if !keep {
                    let _ = query.kill().await;
                }
                if let Some(delegations) = delegations.as_mut() {
                    while let Ok(event) = delegations.receiver.try_recv() {
                        emit(event);
                    }
                }
                // A result the SDK did not model still ends the turn. It says
                // what happened when it carries a reason; otherwise its text is
                // the reply, and an empty one is no reply.
                if raw["is_error"].as_bool().unwrap_or(false) {
                    return Err(said_or_ended(raw["result"].as_str())?);
                }
                return Ok(Streamed {
                    text: said_or_ended(raw["result"].as_str())?,
                    milliseconds: elapsed_ms(started),
                    first_text_ms,
                    ..Streamed::default()
                });
            }
            Ok(_) => {}
        }
    }
    let _ = query.kill().await;
    if reply.text.trim().is_empty() {
        return Err("The turn ended without a reply.".into());
    }
    reply.milliseconds = elapsed_ms(started);
    reply.first_text_ms = first_text_ms;
    Ok(reply)
}

/// `options` for a session Claude Code saves, continuing `resume` when the
/// chat already holds one.
fn with_session(mut options: QueryOptions, resume: Option<Resume>) -> QueryOptions {
    options.persist_session = true;
    if let Some(Resume { id, note }) = resume {
        options = options.resume(id);
        options.system_prompt = Some(SystemPromptConfig::Preset { append: Some(note) });
    }
    options
}

/// `options` carrying the chat's briefing (#11473): appended to Claude
/// Code's system prompt, after Coder's note on a resumed session, with
/// Claude Code's own loading of the instruction files the briefing was
/// chosen from switched off (`claudeMdExcludes`), so nothing is read twice.
/// Its settings, skills, hooks and other `CLAUDE.md` files still load.
pub(crate) fn briefed(
    mut options: QueryOptions,
    briefing: Option<&crate::context::Briefing>,
) -> QueryOptions {
    let Some(briefing) = briefing else {
        // A new process: no message briefing to add back after a
        // compaction (#11490).
        if let Some(cwd) = &options.cwd {
            crate::context_hook::note_message_briefing(cwd, None);
        }
        return options;
    };
    let text = briefing.render(false);
    let append = match options.system_prompt.take() {
        Some(SystemPromptConfig::Preset { append: Some(note) }) => format!("{note}\n\n{text}"),
        _ => text,
    };
    options.system_prompt = Some(SystemPromptConfig::Preset {
        append: Some(append),
    });
    options.settings = Some(serde_json::json!({ "claudeMdExcludes": excluded(briefing) }));
    if let Some(cwd) = &options.cwd {
        crate::context_hook::note_briefed(cwd, &briefing.files);
    }
    options
}

/// The instruction files Claude Code leaves to the briefing: each file the
/// briefing was chosen from, and the `CLAUDE.md` beside it, which Coder read
/// too (or found to be the same file).
#[must_use]
pub fn excluded(briefing: &crate::context::Briefing) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for file in &briefing.files {
        let mut add = |path: &Path| {
            let text = path.display().to_string();
            if !out.contains(&text) {
                out.push(text);
            }
        };
        add(file);
        if let Ok(real) = file.canonicalize() {
            add(&real);
        }
        if let Some(dir) = file.parent() {
            add(&dir.join("CLAUDE.md"));
        }
    }
    out
}

/// A message to a running chat that starts a new piece of work: its
/// briefing first, then the message.
#[must_use]
pub fn with_briefing(briefing: &crate::context::Briefing, message: &str) -> String {
    format!(
        "<briefing>\nFor this next piece of work, follow this briefing in place of the one you started with.\n\n{}\n</briefing>\n\n{message}",
        briefing.render(false).trim_end()
    )
}

/// `message` led by a note that the person switched the chat to `model`
/// (#11480). The session's opening note named the model it started on, and
/// a resumed session keeps that note, so the switch is told with the
/// message the new model first answers.
#[must_use]
pub fn with_switch(model: &str, message: &str) -> String {
    let name = name(model);
    format!(
        "<model-switch>\nThe person switched this chat to {name}. From this message on you run on {name}: when asked which model runs you, name {name}.\n</model-switch>\n\n{message}"
    )
}

/// Stop a running turn. A chat's long-lived process (`keep`) is
/// interrupted, so Claude Code ends the turn itself and the process and its
/// session stay for the next message; it is ended only when the interrupt is
/// refused or the turn does not report its end. A one-off turn is ended.
async fn stop(query: &mut claude_agent_sdk::Query, keep: bool) {
    if keep && query.interrupt().await.is_ok() {
        let ended = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Some(message) = query.next().await {
                if message.as_ref().is_ok_and(is_result) {
                    return true;
                }
            }
            false
        })
        .await;
        if ended == Ok(true) {
            return;
        }
    }
    let _ = query.kill().await;
}

/// Whether `message` is a turn's `result`, modeled or not.
fn is_result(message: &SdkMessage) -> bool {
    match message {
        SdkMessage::Result(_) => true,
        SdkMessage::Unknown { type_name, .. } => type_name == "result",
        _ => false,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

/// The sentence a result carries, or the no-reply sentence when it carries
/// none. Both the modeled and the unmodeled result paths end here, so an
/// empty result reads the same either way.
fn said_or_ended(text: Option<&str>) -> Result<String, String> {
    match text.filter(|text| !text.trim().is_empty()) {
        Some(text) => Ok(text.to_owned()),
        None => Err("The turn ended without a reply.".into()),
    }
}

/// The answer a finished turn gives, or the sentence it fails with.
fn finish(result: SdkResultMessage) -> Result<Streamed, String> {
    let (usage, model, text, cost, is_error, errors) = match result {
        SdkResultMessage::Success(success) => (
            success.usage,
            first_model(&success.model_usage),
            success.result,
            success.total_cost_usd,
            success.is_error,
            Vec::new(),
        ),
        SdkResultMessage::ErrorDuringExecution(error) => (
            error.usage,
            first_model(&error.model_usage),
            String::new(),
            error.total_cost_usd,
            true,
            error.errors,
        ),
        SdkResultMessage::ErrorMaxTurns(error) => (
            error.usage,
            first_model(&error.model_usage),
            String::new(),
            error.total_cost_usd,
            true,
            error.errors,
        ),
        SdkResultMessage::ErrorMaxBudget(error) => (
            error.usage,
            first_model(&error.model_usage),
            String::new(),
            error.total_cost_usd,
            true,
            error.errors,
        ),
        SdkResultMessage::ErrorMaxStructuredOutputRetries(error) => (
            error.usage,
            first_model(&error.model_usage),
            String::new(),
            error.total_cost_usd,
            true,
            error.errors,
        ),
    };
    if is_error {
        // A usage limit arrives as a success whose `result` is the excuse,
        // so the engine's own words are shown before a bare failure.
        return Err(errors
            .into_iter()
            .find(|error| !error.trim().is_empty())
            .or_else(|| (!text.trim().is_empty()).then_some(text))
            .unwrap_or_else(|| "The turn ended on an error.".to_owned()));
    }
    // An empty success is no reply, the same as a turn that streamed none.
    let text = said_or_ended(Some(&text))?;
    let prompt_tokens = counted_input(&usage);
    let completion_tokens = usage.output_tokens;
    Ok(Streamed {
        text,
        model,
        finish_reason: Some("stop".into()),
        usage: Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.saturating_add(completion_tokens),
            cost: (cost > 0.0).then_some(cost),
            completion_tokens_details: None,
        },
        first_text_ms: None,
        milliseconds: 0,
    })
}

/// The input a turn used: fresh input plus the cached prompt it was read
/// against. Claude Code reports them apart, and most of a long conversation's
/// input arrives from the cache, so counting `input_tokens` alone undercounts
/// every turn after the first.
fn counted_input(usage: &claude_agent_sdk::Usage) -> u64 {
    usage
        .input_tokens
        .saturating_add(usage.cache_read_input_tokens.unwrap_or(0))
        .saturating_add(usage.cache_creation_input_tokens.unwrap_or(0))
}

/// The model the engine reported for a turn, from its per-model usage.
///
/// A turn can name more than one model (a subagent, for instance), and the
/// map has no order, so the pinned [`MODEL`] wins when it appears and any
/// other reported name is the fallback rather than an arbitrary one.
fn first_model(usage: &std::collections::HashMap<String, claude_agent_sdk::ModelUsage>) -> String {
    if usage.contains_key(MODEL) {
        return MODEL.to_owned();
    }
    usage.keys().min().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #11480: a switch is told with the message the new model answers,
    /// by name, and a pin outranks the pick.
    #[test]
    fn a_switch_names_the_new_model_with_the_message() {
        let text = with_switch("claude-opus-5-5", "which model are you?");
        assert!(text.contains("switched this chat to Opus 5.5"), "{text}");
        assert!(text.ends_with("\n\nwhich model are you?"), "{text}");
        assert_eq!(
            turn_model(Some("claude-sonnet-5-5".into())),
            "claude-sonnet-5-5"
        );
        assert_eq!(name("claude-fable-5-1"), "Fable 5.1");
        let note = crate::identity::prompt("claude-opus-5-5");
        assert!(
            note.contains("the person picked Opus 5.5 for this chat"),
            "{note}"
        );
        assert!(note.contains("name the newest one"), "{note}");
    }

    #[test]
    fn a_narrow_turn_runs_haiku_5_5_and_only_the_basic_file_tools() {
        assert_eq!(MODEL, "claude-haiku-5-5");
        assert_eq!(MODEL_LABEL, "Haiku 5.5");
        assert_eq!(TOOLS, ["Read", "Write", "Edit", "Glob", "Grep"]);
        for tool in TOOLS {
            assert!(allows(tool), "{tool}");
        }
        for tool in ["Bash", "WebFetch", "Task", "NotebookEdit", "MultiEdit"] {
            assert!(!allows(tool), "{tool}");
        }
    }

    /// `--allowedTools` pre-approves a tool for every call and stops the CLI
    /// sending `can_use_tool`, so the checkout boundary in the handler would
    /// never run. A turn must narrow its tools with `--tools` alone.
    #[test]
    fn a_narrow_turn_never_pre_approves_a_tool() {
        let (options, allowed) = turn_options(Path::new("/work/tree"), Access::Files(&TOOLS), None);
        assert_eq!(options.model.as_deref(), Some(MODEL));
        assert_eq!(options.permission_mode, Some(PermissionMode::Default));
        assert!(options.strict_mcp_config);
        assert_eq!(options.allowed_tools, None);
        assert_eq!(
            options.tools,
            Some(ToolsConfig::Names(
                TOOLS.iter().map(|t| (*t).to_owned()).collect()
            ))
        );
        assert_eq!(allowed, TOOLS.map(str::to_owned).to_vec());
        // The narrower set and the handler's list agree, so nothing the CLI
        // offers is missing from the check that holds the boundary.
        if let Some(ToolsConfig::Names(named)) = &options.tools {
            assert_eq!(named, &allowed);
        }
    }

    #[test]
    fn the_prompt_carries_coders_own_name_and_the_conversation() {
        let messages = vec![
            Message::user("fix the test".to_owned()),
            Message {
                role: "assistant".into(),
                content: "on it".into(),
            },
        ];
        let prompt = task(&messages);
        assert!(
            prompt.starts_with("You are Coder, OpenAgents' coding agent."),
            "{prompt}"
        );
        assert!(prompt.contains("user: fix the test"), "{prompt}");
        assert!(prompt.contains("assistant: on it"), "{prompt}");
        assert!(prompt.len() <= TASK_LIMIT, "{}", prompt.len());
    }

    #[test]
    fn a_newest_message_larger_than_the_limit_still_reaches_the_prompt() {
        let big = "x".repeat(TASK_LIMIT * 2);
        let messages = vec![Message::user(format!("start {big} end"))];
        let prompt = task(&messages);
        assert!(prompt.len() <= TASK_LIMIT, "{}", prompt.len());
        assert!(
            prompt.contains("user: start "),
            "{}",
            &prompt[prompt.len() - 200..]
        );
        assert!(
            prompt.ends_with(" end"),
            "{}",
            &prompt[prompt.len() - 200..]
        );
        assert!(prompt.contains("[...]"));
    }

    #[test]
    fn clipping_keeps_the_head_and_tail_on_character_boundaries() {
        let text = "é".repeat(100);
        let clipped = clip(&text, 41);
        assert!(clipped.len() <= 41, "{}", clipped.len());
        assert!(
            clipped.starts_with('é') && clipped.ends_with('é'),
            "{clipped}"
        );
        assert_eq!(clip("short", 41), "short");
    }

    #[test]
    fn clipping_never_exceeds_the_limit_even_below_the_marker_size() {
        let text = "é".repeat(100);
        for limit in 0..=12 {
            let clipped = clip(&text, limit);
            assert!(clipped.len() <= limit, "{limit}: {}", clipped.len());
            // Without a marker the whole result is the head of the text.
            if !clipped.contains("[...]") {
                assert!(text.starts_with(&clipped), "{limit}: {clipped:?}");
            }
        }
        assert_eq!(clip("ééé", 3), "é");
        assert_eq!(clip("ééé", 0), "");
    }

    #[test]
    fn only_a_whole_segment_walks_a_glob_up_a_directory() {
        assert!(names_a_parent("../secret"));
        assert!(names_a_parent("src/../secret"));
        assert!(names_a_parent("a\\..\\b"));
        assert!(!names_a_parent("foo..bar"));
        assert!(!names_a_parent("src/**/*.rs"));
        assert!(!names_a_parent("..."));
    }

    #[test]
    #[cfg(unix)]
    fn the_file_tools_stay_inside_the_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "fn main() {}").unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("link")).unwrap();

        let inside_call = |tool: &str, input: Value| stays_in(root, tool, &input);
        assert!(inside_call(
            "Read",
            serde_json::json!({"file_path": "src/lib.rs"})
        ));
        assert!(inside_call(
            "Write",
            serde_json::json!({"file_path": "src/new.rs"})
        ));
        assert!(inside_call(
            "Grep",
            serde_json::json!({"pattern": "x", "path": "src"})
        ));
        assert!(inside_call(
            "Glob",
            serde_json::json!({"pattern": "**/*.rs"})
        ));
        assert!(!inside_call(
            "Read",
            serde_json::json!({"file_path": "../secret"})
        ));
        assert!(!inside_call(
            "Read",
            serde_json::json!({"file_path": "src/../../secret"})
        ));
        assert!(!inside_call(
            "Write",
            serde_json::json!({"file_path": "/etc/hosts"})
        ));
        assert!(!inside_call(
            "Edit",
            serde_json::json!({"file_path": "link/rc"})
        ));
        assert!(!inside_call(
            "Glob",
            serde_json::json!({"pattern": "../**"})
        ));
        assert!(!inside_call(
            "Glob",
            serde_json::json!({"pattern": "/etc/*"})
        ));
        assert!(inside_call(
            "Glob",
            serde_json::json!({"pattern": "src/foo..bar.rs"})
        ));
        assert!(!inside_call(
            "Grep",
            serde_json::json!({"pattern": "x", "path": "/"})
        ));
    }

    #[test]
    fn an_empty_success_is_no_reply() {
        let empty = SdkResultMessage::Success(
            serde_json::from_value(serde_json::json!({
                "duration_ms": 1, "duration_api_ms": 1, "is_error": false, "num_turns": 1,
                "result": "  ", "total_cost_usd": 0.0,
                "usage": {"input_tokens": 1, "output_tokens": 0},
                "modelUsage": {}, "permission_denials": [],
                "uuid": "u", "session_id": "s"
            }))
            .unwrap(),
        );
        assert_eq!(
            finish(empty).unwrap_err(),
            "The turn ended without a reply."
        );
    }

    #[test]
    fn an_error_result_never_reads_as_a_reply() {
        let error = SdkResultMessage::ErrorDuringExecution(
            serde_json::from_value(serde_json::json!({
                "duration_ms": 1, "duration_api_ms": 1, "is_error": true, "num_turns": 1,
                "total_cost_usd": 0.0,
                "usage": {"input_tokens": 1, "output_tokens": 1},
                "modelUsage": {}, "permission_denials": [],
                "errors": ["You've hit your session limit"], "uuid": "u", "session_id": "s"
            }))
            .unwrap(),
        );
        assert_eq!(finish(error).unwrap_err(), "You've hit your session limit");
    }

    /// A usage limit arrives as a success whose `result` is the excuse; the
    /// engine's own words are what the person sees.
    #[test]
    fn a_usage_limit_success_says_what_the_engine_said() {
        let limited = SdkResultMessage::Success(
            serde_json::from_value(serde_json::json!({
                "duration_ms": 1, "duration_api_ms": 1, "is_error": true, "num_turns": 1,
                "result": "You've hit your session limit · resets 1:10pm",
                "total_cost_usd": 0.0,
                "usage": {"input_tokens": 0, "output_tokens": 0},
                "modelUsage": {}, "permission_denials": [],
                "uuid": "u", "session_id": "s"
            }))
            .unwrap(),
        );
        assert_eq!(
            finish(limited).unwrap_err(),
            "You've hit your session limit · resets 1:10pm"
        );
    }

    #[test]
    fn a_successful_result_carries_its_text_model_tokens_and_cost() {
        let success = SdkResultMessage::Success(
            serde_json::from_value(serde_json::json!({
                "duration_ms": 5, "duration_api_ms": 4, "is_error": false, "num_turns": 2,
                "result": "done", "total_cost_usd": 0.0125,
                "usage": {"input_tokens": 30, "output_tokens": 4},
                "modelUsage": {"claude-haiku-5-5": {"inputTokens": 30, "outputTokens": 4,
                    "cacheReadInputTokens": 0, "cacheCreationInputTokens": 0,
                    "webSearchRequests": 0, "costUSD": 0.0125, "contextWindow": 200000}},
                "permission_denials": [],
                "uuid": "u", "session_id": "s"
            }))
            .unwrap(),
        );
        let reply = finish(success).unwrap();
        assert_eq!(reply.text, "done");
        assert_eq!(reply.model, "claude-haiku-5-5");
        assert_eq!(reply.usage.total_tokens, 34);
        assert_eq!(reply.usage.cost, Some(0.0125));
    }

    /// Cached input is input the model read, and Claude Code reports it apart
    /// from `input_tokens`. Counting only the fresh tokens undercounts every
    /// turn after the first, so all three are added.
    #[test]
    fn cached_input_counts_toward_the_prompt_tokens() {
        let success = SdkResultMessage::Success(
            serde_json::from_value(serde_json::json!({
                "duration_ms": 5, "duration_api_ms": 4, "is_error": false, "num_turns": 1,
                "result": "done", "total_cost_usd": 0.02,
                "usage": {"input_tokens": 3, "output_tokens": 10,
                    "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 500},
                "modelUsage": {}, "permission_denials": [],
                "uuid": "u", "session_id": "s"
            }))
            .unwrap(),
        );
        let reply = finish(success).unwrap();
        assert_eq!(reply.usage.prompt_tokens, 1503);
        assert_eq!(reply.usage.completion_tokens, 10);
        assert_eq!(reply.usage.total_tokens, 1513);
    }

    /// A turn can report more than one model; the map has no order, so the
    /// pinned one is chosen rather than whichever the map happened to yield.
    #[test]
    fn the_reported_model_is_the_pinned_one_when_it_appears() {
        let mut usage = std::collections::HashMap::new();
        usage.insert("claude-opus-5-5".to_owned(), model_usage());
        usage.insert(MODEL.to_owned(), model_usage());
        assert_eq!(first_model(&usage), MODEL);
        // Only one model, and not the pinned one: that name is the answer.
        let mut other = std::collections::HashMap::new();
        other.insert("claude-opus-5-5".to_owned(), model_usage());
        assert_eq!(first_model(&other), "claude-opus-5-5");
        assert_eq!(first_model(&std::collections::HashMap::new()), "");
    }

    fn model_usage() -> claude_agent_sdk::ModelUsage {
        serde_json::from_value(serde_json::json!({
            "inputTokens": 1, "outputTokens": 1, "cacheReadInputTokens": 0,
            "cacheCreationInputTokens": 0, "webSearchRequests": 0, "costUSD": 0.0,
            "contextWindow": 200000
        }))
        .unwrap()
    }

    /// A result the SDK did not model still ends the turn, and it reads the
    /// same way the modeled one does: text is the reply, empty is no reply.
    #[test]
    fn an_unmodeled_result_reads_the_same_as_a_modeled_one() {
        assert_eq!(said_or_ended(Some("hello")).unwrap(), "hello");
        assert_eq!(
            said_or_ended(Some("   ")).unwrap_err(),
            "The turn ended without a reply."
        );
        assert_eq!(
            said_or_ended(None).unwrap_err(),
            "The turn ended without a reply."
        );
    }

    /// A full Coder chat turn on Claude Code runs Token savers' gate and
    /// conditional context in the session's one registry (#11531): one
    /// PreToolUse hook (no raw gate beside it) holds a merge for the
    /// person, and the first touch of a folder adds its `AGENTS.md` through
    /// the same composition.
    #[tokio::test]
    async fn a_chat_turns_levers_run_through_the_session_registry() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/AGENTS.md"), "Tabs in sub, never spaces.\n").unwrap();
        std::fs::write(root.join("sub/x.rs"), "fn x() {}\n").unwrap();
        let (options, _) = turn_options(&root, Access::Full, None);
        let pre = &options.hooks[&claude_agent_sdk::HookEvent::PreToolUse];
        assert_eq!(pre.len(), 1, "one PreToolUse composition");
        let merge = serde_json::json!({
            "hook_event_name": "PreToolUse", "session_id": "s1", "cwd": root,
            "tool_name": "Bash", "tool_input": {"command": "gh pr merge 5 --squash"},
        });
        let out = serde_json::to_value(pre[0].hooks[0].call(merge, None).await.unwrap()).unwrap();
        assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(
            out["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .starts_with("Held for the person"),
            "{out}"
        );
        let post = &options.hooks[&claude_agent_sdk::HookEvent::PostToolUse];
        assert_eq!(post.len(), 1, "one PostToolUse composition");
        let read = serde_json::json!({
            "hook_event_name": "PostToolUse", "session_id": "s1", "cwd": root,
            "tool_name": "Read", "tool_input": {"file_path": root.join("sub/x.rs")},
            "tool_response": "fn x() {}",
        });
        let out = serde_json::to_value(post[0].hooks[0].call(read, None).await.unwrap()).unwrap();
        assert!(
            out["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .is_some_and(|context| context.contains("Tabs in sub")),
            "{out}"
        );
    }

    /// A chat turn is saved by Claude Code, and a later one continues the
    /// saved session with Coder's identity note beside Claude Code's own
    /// system prompt (#11435). Solver's Intaker stays a one-off.
    #[test]
    fn a_chat_turn_saves_and_resumes_its_session() {
        let base = || turn_options(Path::new("/work/tree"), Access::Full, None).0;
        let off = base();
        assert!(
            off.build_args()
                .contains(&"--no-session-persistence".to_owned())
        );
        assert_eq!(off.resume, None);

        let fresh = with_session(base(), None);
        assert!(
            !fresh
                .build_args()
                .contains(&"--no-session-persistence".to_owned())
        );
        assert_eq!(fresh.resume, None);

        let note = format!("{}\n", crate::identity::prompt(MODEL));
        let resumed = with_session(
            base(),
            Some(Resume {
                id: "5b0c1d2e".into(),
                note: note.clone(),
            }),
        );
        let args = resumed.build_args();
        assert!(!args.contains(&"--no-session-persistence".to_owned()));
        assert_eq!(resumed.resume.as_deref(), Some("5b0c1d2e"));
        assert!(args.iter().any(|arg| arg.contains("5b0c1d2e")), "{args:?}");
        assert!(
            matches!(
                &resumed.system_prompt,
                Some(SystemPromptConfig::Preset { append: Some(appended) }) if *appended == note
            ),
            "{:?}",
            resumed.system_prompt
        );
    }

    /// A run must not fall back to an API key or a cloud provider: the login
    /// this computer holds is the only credential it may use.
    #[test]
    fn a_run_strips_api_keys_and_cloud_provider_switches() {
        for access in [Access::Full, Access::Files(&TOOLS)] {
            let (options, _) = turn_options(Path::new("/work/tree"), access, None);
            for name in [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "CLAUDE_CODE_USE_BEDROCK",
                "CLAUDE_CODE_USE_VERTEX",
                "CLAUDE_CODE_USE_FOUNDRY",
            ] {
                assert!(options.env_remove.contains(&name.to_owned()), "{name}");
            }
        }
    }

    /// The mode's own turns are full Claude Code: every tool, every call
    /// approved, and the person's and the repository's settings (#11440).
    #[test]
    fn a_full_turn_is_claude_code_with_every_call_approved() {
        let (options, _) = turn_options(Path::new("/work/tree"), Access::Full, None);
        assert_eq!(
            options.permission_mode,
            Some(PermissionMode::BypassPermissions)
        );
        assert!(options.allow_dangerously_skip_permissions);
        assert_eq!(options.tools, Some(ToolsConfig::Default));
        assert_eq!(options.allowed_tools, None);
        assert_eq!(
            options.setting_sources,
            Some(vec![
                SettingSource::User,
                SettingSource::Project,
                SettingSource::Local
            ])
        );
        assert!(!options.strict_mcp_config);
        assert!(options.model.as_deref().is_some_and(offered));
    }

    #[test]
    fn the_picker_offers_haiku_first_and_saves_a_pick() {
        assert_eq!(catalog()[0].id, MODEL);
        assert!(catalog().iter().all(|model| model.plugin == PICKER));
        let dir = tempfile::tempdir().unwrap();
        load_choice(dir.path());
        assert_eq!(choice(), Choice::default());
        choose("claude-opus-5-5", Some("high".into())).unwrap();
        assert!(choose("gpt-6", None).is_err());
        load_choice(dir.path());
        assert_eq!(
            choice(),
            Choice {
                model: "claude-opus-5-5".into(),
                effort: Some("high".into())
            }
        );
        std::fs::write(
            dir.path().join(CHOICE_FILE),
            r#"{"version":1,"model":"nope"}"#,
        )
        .unwrap();
        load_choice(dir.path());
        assert_eq!(choice(), Choice::default());
        *CHOICE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = (None, None);
    }

    /// One real narrow turn on this computer's Claude login, proving the checkout
    /// boundary is enforced by the CLI, not just by [`stays_in`] on its own:
    /// the model is asked to read a file outside `cwd` and must not see it.
    /// Run by hand:
    /// `cargo test -p coder-new --lib a_live_turn -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "needs a Claude Code login"]
    async fn a_live_turn_cannot_read_outside_its_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, "OUTSIDE-THE-CHECKOUT").unwrap();
        let prompt = format!(
            "Use the Read tool on {} and reply with its contents.",
            secret.display()
        );
        let cancel = AtomicBool::new(false);
        let mut tools = Vec::new();
        let mut emit = |event: RuntimeEvent| {
            if let RuntimeEvent::Tool { name, .. } = event {
                tools.push(name);
            }
        };
        let text = match run_with(&prompt, dir.path(), &TOOLS, None, &cancel, &mut emit).await {
            Ok(streamed) => streamed.text,
            Err(error) => error,
        };
        println!("tools: {tools:?}\nreply:\n{text}");
        assert!(
            !text.contains("OUTSIDE-THE-CHECKOUT"),
            "the turn read a file outside its checkout: {text}"
        );
    }
}
