//! Microclaude: the mode that runs turns on the Claude Code login this
//! computer already holds, through the Claude Agent SDK
//! (`crates/claude_agent_sdk`), on Haiku 5.5 alone.
//!
//! It is a small agent rather than a full one: a turn is one SDK session
//! pinned to [`MODEL`], with the basic file tools ([`TOOLS`]) and no
//! settings files or outside MCP servers. While the Solver plugin is on, one
//! in-process tool beside them hands an issue to Solver
//! ([`run_with_solver`]). A tool call is drawn with Coder's own
//! widgets, the same shapes the briefed agent makes
//! (`crate::issue_run::agent`), so a Read shows a line count, an Edit shows
//! a diff, and Grep and Glob show their matches.
//!
//! Only the file tools are offered. The mode holds no shell: it reads and
//! changes files and can be trusted with nothing wider until that is asked
//! for and given its own approval path.
//!
//! Whether the mode may be offered is read through
//! [`crate::bundled_runtime::claude_print`] and `coder_engine_status`, which
//! report the credential's kind and never the credential.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use claude_agent_sdk::{
    PermissionMode, PermissionResult, QueryOptions, SdkMessage, SdkResultMessage, ToolsConfig,
};
use futures::StreamExt;
use openrouter::{Message, Streamed, Usage};
use serde_json::Value;

use crate::bundled_runtime::RuntimeEvent;

/// The mode's name, in the rail and in the conversation.
pub const LABEL: &str = "microclaude";

/// What Shift+Tab says on a computer with no Claude account login.
pub const NEEDS_LOGIN: &str = "microclaude needs Claude Code signed in on this computer.";

/// The one model this mode runs: Haiku 5.5, and nothing else.
pub const MODEL: &str = "claude-haiku-5-5";

/// What the mode calls its model where a person reads it.
pub const MODEL_LABEL: &str = "Haiku 5.5";

/// The basic file tools a microclaude turn has, the same names Coder's own
/// file tools use.
pub const TOOLS: [&str; 5] = ["Read", "Write", "Edit", "Glob", "Grep"];

/// What a message with a picture says: the mode reads text only.
pub const TEXT_ONLY: &str = "Microclaude reads text only, so your picture was left out.";

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

/// Whether `tool` is one of the mode's [`TOOLS`].
#[must_use]
pub fn allows(tool: &str) -> bool {
    TOOLS.contains(&tool)
}

/// The prompt one turn sends: Coder's own identity note, then the
/// conversation, within [`TASK_LIMIT`].
#[must_use]
pub fn task(messages: &[Message]) -> String {
    task_with(messages, false)
}

/// [`task`], telling the model about the solver tool when `solver` is on.
#[must_use]
pub fn task_with(messages: &[Message], solver: bool) -> String {
    let tools = if solver {
        crate::solver_plugin::instructions()
    } else {
        ""
    };
    let prefix = format!(
        "{}\n{tools}The conversation:\n",
        crate::identity::prompt(MODEL)
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

/// One running tool call, kept between its `tool_use` and its `tool_result`.
struct Call {
    coder_name: String,
    coder_input: Value,
    sdk_input: Value,
}

/// Runs one microclaude turn to its end, emitting the reply's text, the model
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
    run_with(prompt, cwd, &TOOLS, None, cancel, emit).await
}

/// Runs one Haiku turn like [`run`], with `tools` from Claude Code's own
/// set and, beside them, the tools of an in-process `server` (the solver's
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
    run_inner(prompt, cwd, tools, server, None, cancel, emit).await
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
        &TOOLS,
        Some(server),
        Some(delegations),
        cancel,
        emit,
    )
    .await
}

/// The options and the tool names one turn may use.
///
/// `allowed_tools` is deliberately left unset. Claude Code's `--allowedTools`
/// pre-approves the tool for every call, so the CLI never sends
/// `can_use_tool` and the permission handler below never runs; a tool named
/// there could read or write any path. `--tools` alone narrows the offered
/// tools without pre-approving them, so every call still comes to the
/// handler, which is what holds the checkout boundary.
fn turn_options(
    cwd: &Path,
    tools: &[&str],
    server: Option<claude_agent_sdk::SdkMcpServer>,
) -> (QueryOptions, Vec<String>) {
    let mut options = QueryOptions::new()
        .cwd(cwd)
        .model(MODEL)
        .permission_mode(PermissionMode::Default)
        .no_session_persistence();
    let mut allowed: Vec<String> = tools.iter().map(|t| (*t).to_owned()).collect();
    options.tools = Some(ToolsConfig::Names(allowed.clone()));
    if let Some(server) = server {
        allowed.extend(server.allowed_tool_names());
        options = options.sdk_mcp_server(server);
    }
    options.setting_sources = Some(Vec::new());
    options.strict_mcp_config = true;
    options.env_remove = REMOVED_ENV.iter().map(|name| (*name).to_owned()).collect();
    (options, allowed)
}

/// Delegation events a tool server sends while the turn waits on it, and
/// the flag that stops what it runs when the turn ends.
struct Delegations {
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
    tools: &[&str],
    server: Option<claude_agent_sdk::SdkMcpServer>,
    mut delegations: Option<Delegations>,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Streamed, String> {
    if !crate::approval::tools_allowed() {
        return Err("This host refuses every tool, so microclaude cannot work here.".into());
    }
    let (options, allowed) = turn_options(cwd, tools, server);
    let root = cwd.to_owned();
    let handler = claude_agent_sdk::permission_handler(move |request| {
        let decision = if !allowed.iter().any(|name| *name == request.tool_name) {
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
    let started = Instant::now();
    let mut reply = Streamed::default();
    let mut calls: HashMap<String, Call> = HashMap::new();
    let mut first_text_ms = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = query.kill().await;
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
        match message {
            Err(claude_agent_sdk::Error::UnrecognizedMessage { .. }) => continue,
            Err(error) => {
                let _ = query.kill().await;
                return Err(error.to_string());
            }
            Ok(SdkMessage::Assistant(assistant)) => {
                if let Some(model) = assistant.message["model"]
                    .as_str()
                    .filter(|m| !m.is_empty())
                {
                    reply.model = model.to_owned();
                    emit(RuntimeEvent::Model(model.to_owned()));
                }
                for block in assistant.message["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    match block["type"].as_str() {
                        Some("text") => {
                            let text = block["text"].as_str().unwrap_or_default();
                            if !text.is_empty() {
                                if first_text_ms.is_none() {
                                    first_text_ms = Some(elapsed_ms(started));
                                }
                                reply.text.push_str(text);
                                emit(RuntimeEvent::Text(text.to_owned()));
                            }
                        }
                        Some("tool_use") => {
                            let name = block["name"].as_str().unwrap_or_default();
                            // The delegation's own entry stands for this call.
                            if crate::solver_plugin::is_mcp_tool(name) {
                                continue;
                            }
                            let sdk_input = block["input"].clone();
                            let (coder_name, coder_input) =
                                crate::issue_run::agent::coder_call(name, &sdk_input, cwd, &[]);
                            emit(RuntimeEvent::Tool {
                                name: coder_name.clone(),
                                input: coder_input.clone(),
                                output: Value::Null,
                                running: true,
                            });
                            if let Some(id) = block["id"].as_str() {
                                calls.insert(
                                    id.to_owned(),
                                    Call {
                                        coder_name,
                                        coder_input,
                                        sdk_input,
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
                    let Some(call) = block["tool_use_id"]
                        .as_str()
                        .and_then(|id| calls.remove(id))
                    else {
                        continue;
                    };
                    let structured = user.tool_use_result.as_ref().filter(|v| v.is_object());
                    let output = crate::issue_run::agent::coder_output(
                        &call.coder_name,
                        &call.sdk_input,
                        &call.coder_input,
                        &crate::issue_run::agent::result_text(block),
                        structured,
                        block["is_error"].as_bool().unwrap_or(false),
                    );
                    emit(RuntimeEvent::Tool {
                        name: call.coder_name,
                        input: call.coder_input,
                        output,
                        running: false,
                    });
                }
            }
            Ok(SdkMessage::Result(result)) => {
                let _ = query.kill().await;
                if let Some(delegations) = delegations.as_mut() {
                    while let Ok(event) = delegations.receiver.try_recv() {
                        emit(event);
                    }
                }
                let mut done = finish(result)?;
                done.milliseconds = elapsed_ms(started);
                done.first_text_ms = first_text_ms;
                return Ok(done);
            }
            Ok(SdkMessage::Unknown { type_name, raw }) if type_name == "result" => {
                let _ = query.kill().await;
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

    #[test]
    fn the_mode_runs_haiku_5_5_and_only_the_basic_file_tools() {
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
    fn a_turn_never_pre_approves_a_tool() {
        let (options, allowed) = turn_options(Path::new("/work/tree"), &TOOLS, None);
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

    /// A run must not fall back to an API key or a cloud provider: the login
    /// this computer holds is the only credential it may use.
    #[test]
    fn a_run_strips_api_keys_and_cloud_provider_switches() {
        let (options, _) = turn_options(Path::new("/work/tree"), &TOOLS, None);
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

    /// One real turn on this computer's Claude login, proving the checkout
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
        let text = match run(&prompt, dir.path(), &cancel, &mut emit).await {
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
