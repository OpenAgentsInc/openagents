//! Microclaude: the mode that runs turns on the Claude Code login this
//! computer already holds, through the Claude Agent SDK
//! (`crates/claude_agent_sdk`), on Haiku 5.5 alone.
//!
//! It is a small agent rather than a full one: a turn is one SDK session
//! pinned to [`MODEL`], with the basic file tools only ([`TOOLS`]) and no
//! settings files or MCP servers. A tool call is drawn with Coder's own
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
use std::path::Path;
use std::sync::OnceLock;
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

/// Credentials the CLI must not inherit, so the session uses the Claude Code
/// login this computer holds rather than an API key.
const REMOVED_ENV: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

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
    let prefix = format!("{}\nThe conversation:\n", crate::identity::prompt(MODEL));
    let allowance = TASK_LIMIT.saturating_sub(prefix.len());
    format!(
        "{prefix}{}",
        crate::live::local_conversation(messages, allowance)
    )
}

static AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Whether this computer has Claude Code on a Claude account login: the
/// `claude` program is present, and it is signed in to a Claude account
/// rather than with an API key or a cloud credential.
///
/// The answer is read once per process, because it runs `claude auth
/// status`, which is the command that names the credential's kind. A chat
/// here runs interactively on the person's plan, not on the plan's
/// automated-run book, so it never takes a plan slot.
#[must_use]
pub fn available() -> bool {
    *AVAILABLE.get_or_init(|| {
        let Some(agent) = crate::bundled_runtime::claude_print::agent() else {
            return false;
        };
        let status =
            coder_engine_status::claude::check(&agent.program, None, atif::now_ms() / 1_000);
        matches!(
            status.state,
            coder_engine_status::State::SignedIn | coder_engine_status::State::Expiring
        ) && matches!(
            status.method,
            Some(coder_engine_status::Method::ClaudeAi | coder_engine_status::Method::OauthToken)
        )
    })
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
    if !crate::approval::tools_allowed() {
        return Err("This host refuses every tool, so microclaude cannot work here.".into());
    }
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
    options.allowed_tools = Some(allowed.clone());
    options.setting_sources = Some(Vec::new());
    options.strict_mcp_config = true;
    options.env_remove = REMOVED_ENV.iter().map(|name| (*name).to_owned()).collect();
    let handler = claude_agent_sdk::permission_handler(move |request| {
        let decision = if allowed.iter().any(|name| *name == request.tool_name) {
            PermissionResult::allow(request.input.clone())
        } else {
            PermissionResult::deny("That tool is not available in this mode.")
        };
        async move { Ok::<_, claude_agent_sdk::Error>(decision) }
    });
    let mut query = claude_agent_sdk::query_with_permissions(prompt, options, handler)
        .await
        .map_err(|error| format!("Claude Code did not start: {error}"))?;
    let started = Instant::now();
    let mut reply = Streamed::default();
    let mut calls: HashMap<String, Call> = HashMap::new();
    let mut first_text_ms = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = query.kill().await;
            return Err(STOPPED.into());
        }
        let Some(message) = query.next().await else {
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
                let mut done = finish(result)?;
                done.milliseconds = elapsed_ms(started);
                done.first_text_ms = first_text_ms;
                return Ok(done);
            }
            Ok(SdkMessage::Unknown { type_name, raw }) if type_name == "result" => {
                let _ = query.kill().await;
                return Err(raw["result"]
                    .as_str()
                    .filter(|text| !text.is_empty())
                    .unwrap_or("The turn ended without a reply.")
                    .to_owned());
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
    let prompt_tokens = usage.input_tokens;
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

/// The model the engine reported for a turn, from its per-model usage.
fn first_model(usage: &std::collections::HashMap<String, claude_agent_sdk::ModelUsage>) -> String {
    usage.keys().next().cloned().unwrap_or_default()
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
        assert!(prompt.len() <= TASK_LIMIT + 4096, "{}", prompt.len());
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
}
