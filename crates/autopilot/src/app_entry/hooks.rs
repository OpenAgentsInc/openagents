use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use claude_agent_sdk::error::Result as SdkResult;
use claude_agent_sdk::{
    BaseHookInput, HookCallback, HookDecision, HookEvent, HookInput, HookOutput, HookSpecificOutput,
    PostToolUseSpecificOutput, SessionStartSpecificOutput, SyncHookOutput,
    UserPromptSubmitSpecificOutput,
};

use crate::app::catalog::{HookConfig, HookRuntimeConfig, HookScriptEntry, HookScriptSource};
use crate::app::events::ResponseEvent;
use crate::app::parsing::{build_context_injection, build_todo_context};
use crate::app::permissions::extract_bash_command;
use crate::app::{
    hook_event_label, now_timestamp, truncate_bytes, truncate_preview, HookLogEntry,
    HOOK_SCRIPT_TIMEOUT_SECS,
};
use wgpui::components::organisms::{EventData, TagData};

const HOOK_OUTPUT_TRUNCATE: usize = 2000;
const HOOK_BLOCK_PATTERNS: [&str; 3] = ["rm -rf /", "sudo", "> /dev/"];

#[derive(Clone, Debug)]
pub(crate) enum HookCallbackKind {
    ToolBlocker,
    ToolLogger,
    OutputTruncator,
    ContextEnforcer,
    Script(HookScriptEntry),
}

pub(crate) struct CoderHookCallback {
    kind: HookCallbackKind,
    runtime: Arc<HookRuntimeConfig>,
}

impl CoderHookCallback {
    pub(crate) fn new(kind: HookCallbackKind, runtime: Arc<HookRuntimeConfig>) -> Self {
        Self { kind, runtime }
    }
}

#[async_trait]
impl HookCallback for CoderHookCallback {
    async fn call(&self, input: HookInput, tool_use_id: Option<String>) -> SdkResult<HookOutput> {
        let event = hook_event_from_input(&input);
        let tool_name = hook_tool_name(&input);
        let matcher = match &self.kind {
            HookCallbackKind::Script(entry) => entry.matcher.clone(),
            _ => None,
        };

        let summary: String;
        let mut error = None;
        let mut sources = Vec::new();
        let mut output = HookOutput::Sync(SyncHookOutput::continue_execution());
        let mut log_output = true;

        match &self.kind {
            HookCallbackKind::ToolBlocker => {
                sources.push("builtin:tool_blocker".to_string());
                let (next_output, next_summary) = hook_tool_blocker(&input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::ToolLogger => {
                sources.push("builtin:tool_logger".to_string());
                summary = hook_tool_logger_summary(&input);
            }
            HookCallbackKind::OutputTruncator => {
                sources.push("builtin:output_truncator".to_string());
                let (next_output, next_summary) = hook_output_truncator(&input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::ContextEnforcer => {
                sources.extend(hook_context_sources(&self.runtime.config));
                let (next_output, next_summary) =
                    hook_context_enforcer(&self.runtime, &input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::Script(entry) => {
                sources.push(hook_script_source_label(entry));
                match run_hook_script(entry, &input, tool_use_id.as_deref(), &self.runtime).await {
                    Ok(next_output) => {
                        output = next_output;
                        summary = format!("Script {} completed.", entry.path.display());
                    }
                    Err(err) => {
                        summary = format!("Script {} failed.", entry.path.display());
                        error = Some(err);
                        log_output = false;
                    }
                }
            }
        }

        let output_ref = if log_output { Some(&output) } else { None };
        log_hook_event(
            &self.runtime,
            event,
            summary,
            tool_name,
            matcher,
            &input,
            output_ref,
            error,
            sources,
        );

        Ok(output)
    }
}
fn hook_event_from_input(input: &HookInput) -> HookEvent {
    match input {
        HookInput::PreToolUse(_) => HookEvent::PreToolUse,
        HookInput::PostToolUse(_) => HookEvent::PostToolUse,
        HookInput::PostToolUseFailure(_) => HookEvent::PostToolUseFailure,
        HookInput::Notification(_) => HookEvent::Notification,
        HookInput::UserPromptSubmit(_) => HookEvent::UserPromptSubmit,
        HookInput::SessionStart(_) => HookEvent::SessionStart,
        HookInput::SessionEnd(_) => HookEvent::SessionEnd,
        HookInput::Stop(_) => HookEvent::Stop,
        HookInput::SubagentStart(_) => HookEvent::SubagentStart,
        HookInput::SubagentStop(_) => HookEvent::SubagentStop,
        HookInput::PreCompact(_) => HookEvent::PreCompact,
        HookInput::PermissionRequest(_) => HookEvent::PermissionRequest,
    }
}

fn hook_base_input(input: &HookInput) -> &BaseHookInput {
    match input {
        HookInput::PreToolUse(hook) => &hook.base,
        HookInput::PostToolUse(hook) => &hook.base,
        HookInput::PostToolUseFailure(hook) => &hook.base,
        HookInput::Notification(hook) => &hook.base,
        HookInput::UserPromptSubmit(hook) => &hook.base,
        HookInput::SessionStart(hook) => &hook.base,
        HookInput::SessionEnd(hook) => &hook.base,
        HookInput::Stop(hook) => &hook.base,
        HookInput::SubagentStart(hook) => &hook.base,
        HookInput::SubagentStop(hook) => &hook.base,
        HookInput::PreCompact(hook) => &hook.base,
        HookInput::PermissionRequest(hook) => &hook.base,
    }
}

fn hook_tool_name(input: &HookInput) -> Option<String> {
    match input {
        HookInput::PreToolUse(hook) => Some(hook.tool_name.clone()),
        HookInput::PostToolUse(hook) => Some(hook.tool_name.clone()),
        HookInput::PostToolUseFailure(hook) => Some(hook.tool_name.clone()),
        HookInput::PermissionRequest(hook) => Some(hook.tool_name.clone()),
        _ => None,
    }
}

fn hook_tool_input(input: &HookInput) -> Option<&Value> {
    match input {
        HookInput::PreToolUse(hook) => Some(&hook.tool_input),
        HookInput::PostToolUse(hook) => Some(&hook.tool_input),
        HookInput::PostToolUseFailure(hook) => Some(&hook.tool_input),
        HookInput::PermissionRequest(hook) => Some(&hook.tool_input),
        _ => None,
    }
}

fn hook_tool_response(input: &HookInput) -> Option<&Value> {
    match input {
        HookInput::PostToolUse(hook) => Some(&hook.tool_response),
        _ => None,
    }
}

fn hook_tool_error(input: &HookInput) -> Option<&str> {
    match input {
        HookInput::PostToolUseFailure(hook) => Some(hook.error.as_str()),
        _ => None,
    }
}

fn hook_tool_blocker(input: &HookInput) -> (HookOutput, String) {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    let mut summary = format!("ToolBlocker allowed {}.", tool_name);
    let mut sync = SyncHookOutput::continue_execution();

    let is_bash = tool_name.eq_ignore_ascii_case("bash");
    if !is_bash {
        return (HookOutput::Sync(sync), summary);
    }

    let Some(tool_input) = hook_tool_input(input) else {
        return (HookOutput::Sync(sync), summary);
    };
    let Some(command) = extract_bash_command(tool_input) else {
        return (HookOutput::Sync(sync), summary);
    };

    let lowered = command.to_ascii_lowercase();
    for pattern in HOOK_BLOCK_PATTERNS {
        if lowered.contains(&pattern.to_ascii_lowercase()) {
            let reason = format!(
                "Blocked dangerous command: {}",
                truncate_preview(&command, 160)
            );
            sync = SyncHookOutput {
                continue_execution: Some(false),
                decision: Some(HookDecision::Block),
                reason: Some(reason),
                ..Default::default()
            };
            summary = format!("ToolBlocker blocked {}.", tool_name);
            break;
        }
    }

    (HookOutput::Sync(sync), summary)
}

fn hook_tool_logger_summary(input: &HookInput) -> String {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    match hook_event_from_input(input) {
        HookEvent::PreToolUse => format!("ToolLogger pre {}.", tool_name),
        HookEvent::PostToolUse => format!("ToolLogger post {}.", tool_name),
        HookEvent::PostToolUseFailure => {
            if let Some(error) = hook_tool_error(input) {
                format!(
                    "ToolLogger failure {}: {}",
                    tool_name,
                    truncate_preview(error, 120)
                )
            } else {
                format!("ToolLogger failure {}.", tool_name)
            }
        }
        event => format!("ToolLogger {}.", hook_event_label(event)),
    }
}

fn hook_output_truncator(input: &HookInput) -> (HookOutput, String) {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    let Some(tool_response) = hook_tool_response(input) else {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            format!("OutputTruncator skipped {}.", tool_name),
        );
    };

    let response_text =
        serde_json::to_string(tool_response).unwrap_or_else(|_| tool_response.to_string());
    let response_len = response_text.len();
    if response_len <= HOOK_OUTPUT_TRUNCATE {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            format!("OutputTruncator ok for {}.", tool_name),
        );
    }

    let truncated = truncate_bytes(response_text, HOOK_OUTPUT_TRUNCATE);
    let mut sync = SyncHookOutput::continue_execution();
    sync.suppress_output = Some(true);
    sync.hook_specific_output = Some(HookSpecificOutput::PostToolUse(
        PostToolUseSpecificOutput {
            hook_event_name: HookEvent::PostToolUse.as_str().to_string(),
            additional_context: Some(format!(
                "Tool output truncated ({} bytes):\n{}",
                response_len, truncated
            )),
            updated_mcp_tool_output: None,
        },
    ));

    (
        HookOutput::Sync(sync),
        format!("OutputTruncator truncated {} output.", tool_name),
    )
}

fn hook_context_sources(config: &HookConfig) -> Vec<String> {
    let mut sources = Vec::new();
    if config.context_injection {
        sources.push("builtin:context_injection".to_string());
    }
    if config.todo_enforcer {
        sources.push("builtin:todo_enforcer".to_string());
    }
    sources
}

fn hook_context_enforcer(
    runtime: &HookRuntimeConfig,
    input: &HookInput,
) -> (HookOutput, String) {
    let event = hook_event_from_input(input);
    let mut sections = Vec::new();

    if runtime.config.context_injection {
        if let Some(context) = build_context_injection(&runtime.cwd) {
            sections.push(context);
        }
    }
    if runtime.config.todo_enforcer {
        if let Some(todo) = build_todo_context(&runtime.cwd) {
            sections.push(todo);
        }
    }

    if sections.is_empty() {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            "ContextEnforcer no context.".to_string(),
        );
    }

    let combined = sections.join("\n\n");
    let combined_len = combined.len();
    let hook_specific_output = match event {
        HookEvent::UserPromptSubmit => HookSpecificOutput::UserPromptSubmit(
            UserPromptSubmitSpecificOutput {
                hook_event_name: HookEvent::UserPromptSubmit.as_str().to_string(),
                additional_context: Some(combined),
            },
        ),
        HookEvent::SessionStart => HookSpecificOutput::SessionStart(SessionStartSpecificOutput {
            hook_event_name: HookEvent::SessionStart.as_str().to_string(),
            additional_context: Some(combined),
        }),
        _ => {
            return (
                HookOutput::Sync(SyncHookOutput::continue_execution()),
                "ContextEnforcer skipped.".to_string(),
            )
        }
    };

    let mut sync = SyncHookOutput::continue_execution();
    sync.hook_specific_output = Some(hook_specific_output);
    (
        HookOutput::Sync(sync),
        format!("ContextEnforcer injected {} bytes.", combined_len),
    )
}

fn hook_script_source_label(entry: &HookScriptEntry) -> String {
    let source = match entry.source {
        HookScriptSource::Project => "project",
        HookScriptSource::User => "user",
    };
    format!("script:{}:{}", source, entry.path.display())
}

fn hook_script_env(input: &HookInput, tool_use_id: Option<&str>) -> Vec<(String, String)> {
    let base = hook_base_input(input);
    let event = hook_event_from_input(input);
    let mut envs = vec![
        (
            "CLAUDE_HOOK_EVENT".to_string(),
            hook_event_label(event).to_string(),
        ),
        ("CLAUDE_SESSION_ID".to_string(), base.session_id.clone()),
        (
            "CLAUDE_TRANSCRIPT_PATH".to_string(),
            base.transcript_path.clone(),
        ),
        ("CLAUDE_CWD".to_string(), base.cwd.clone()),
    ];
    if let Some(mode) = &base.permission_mode {
        envs.push(("CLAUDE_PERMISSION_MODE".to_string(), mode.clone()));
    }
    if let Some(tool_name) = hook_tool_name(input) {
        envs.push(("CLAUDE_TOOL_NAME".to_string(), tool_name));
    }
    if let Some(tool_use_id) = tool_use_id {
        envs.push(("CLAUDE_TOOL_USE_ID".to_string(), tool_use_id.to_string()));
    }
    envs
}

async fn run_hook_script(
    entry: &HookScriptEntry,
    input: &HookInput,
    tool_use_id: Option<&str>,
    runtime: &HookRuntimeConfig,
) -> Result<HookOutput, String> {
    let mut command = TokioCommand::new(&entry.path);
    command
        .current_dir(&runtime.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in hook_script_env(input, tool_use_id) {
        command.env(key, value);
    }

    let mut child = command.spawn().map_err(|err| {
        format!(
            "Failed to spawn hook script {}: {}",
            entry.path.display(),
            err
        )
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(input)
            .map_err(|err| format!("Failed to serialize hook input: {}", err))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|err| format!("Failed to write hook input: {}", err))?;
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(mut stdout) = stdout {
            let _ = stdout.read_to_end(&mut buffer).await;
        }
        buffer
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_end(&mut buffer).await;
        }
        buffer
    });

    let status = match timeout(Duration::from_secs(HOOK_SCRIPT_TIMEOUT_SECS), child.wait()).await {
        Ok(status) => status.map_err(|err| format!("Hook script failed: {}", err))?,
        Err(_) => {
            let _ = child.kill().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(format!(
                "Hook script {} timed out after {}s.",
                entry.path.display(),
                HOOK_SCRIPT_TIMEOUT_SECS
            ));
        }
    };

    let stdout_bytes = stdout_task
        .await
        .unwrap_or_default();
    let stderr_bytes = stderr_task
        .await
        .unwrap_or_default();

    let stdout_text = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    if !status.success() {
        let mut message = format!("Hook script exited with status {}", status);
        if !stderr_text.is_empty() {
            message.push_str(": ");
            message.push_str(&stderr_text);
        }
        return Err(message);
    }

    if stdout_text.is_empty() {
        return Ok(HookOutput::Sync(SyncHookOutput::continue_execution()));
    }

    serde_json::from_str::<HookOutput>(&stdout_text).map_err(|err| {
        format!(
            "Failed to parse hook output: {} (stdout: {})",
            err,
            truncate_preview(&stdout_text, 160)
        )
    })
}

fn truncate_hook_value(value: Value, max_bytes: usize) -> Value {
    match value {
        Value::String(text) => {
            if text.len() <= max_bytes {
                Value::String(text)
            } else {
                Value::String(truncate_bytes(text, max_bytes))
            }
        }
        other => {
            let raw = serde_json::to_string(&other).unwrap_or_else(|_| other.to_string());
            if raw.len() <= max_bytes {
                other
            } else {
                Value::String(truncate_bytes(raw, max_bytes))
            }
        }
    }
}

fn serialize_hook_value<T: Serialize>(value: &T, max_bytes: usize) -> Value {
    let serialized = serde_json::to_value(value).unwrap_or(Value::Null);
    truncate_hook_value(serialized, max_bytes)
}

fn log_hook_event(
    runtime: &HookRuntimeConfig,
    event: HookEvent,
    summary: String,
    tool_name: Option<String>,
    matcher: Option<String>,
    input: &HookInput,
    output: Option<&HookOutput>,
    error: Option<String>,
    sources: Vec<String>,
) {
    let id = format!(
        "hook-{}-{}",
        hook_event_label(event).to_ascii_lowercase(),
        runtime.counter.fetch_add(1, Ordering::SeqCst)
    );
    let entry = HookLogEntry {
        id,
        event,
        timestamp: now_timestamp(),
        summary,
        tool_name,
        matcher,
        input: serialize_hook_value(input, HOOK_OUTPUT_TRUNCATE),
        output: output.map(|value| serialize_hook_value(value, HOOK_OUTPUT_TRUNCATE)),
        error,
        sources,
    };
    let _ = runtime.log_tx.send(ResponseEvent::HookLog(entry));
}

fn hook_event_kind(event: HookEvent) -> u32 {
    match event {
        HookEvent::PreToolUse => 61001,
        HookEvent::PostToolUse => 61002,
        HookEvent::PostToolUseFailure => 61003,
        HookEvent::Notification => 61004,
        HookEvent::UserPromptSubmit => 61005,
        HookEvent::SessionStart => 61006,
        HookEvent::SessionEnd => 61007,
        HookEvent::Stop => 61008,
        HookEvent::SubagentStart => 61009,
        HookEvent::SubagentStop => 61010,
        HookEvent::PreCompact => 61011,
        HookEvent::PermissionRequest => 61012,
    }
}

fn value_preview(value: &Value, max_chars: usize) -> String {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    truncate_preview(&text, max_chars)
}

pub(super) fn hook_log_event_data(entry: &HookLogEntry) -> EventData {
    let mut tags = Vec::new();
    tags.push(TagData::new(
        "event",
        vec![hook_event_label(entry.event).to_string()],
    ));
    if let Some(tool) = &entry.tool_name {
        tags.push(TagData::new("tool", vec![tool.clone()]));
    }
    if let Some(matcher) = &entry.matcher {
        tags.push(TagData::new("matcher", vec![matcher.clone()]));
    }
    if !entry.sources.is_empty() {
        tags.push(TagData::new("sources", entry.sources.clone()));
    }
    if let Some(error) = &entry.error {
        tags.push(TagData::new("error", vec![error.clone()]));
    }
    tags.push(TagData::new(
        "input",
        vec![value_preview(&entry.input, 180)],
    ));
    if let Some(output) = &entry.output {
        tags.push(TagData::new(
            "output",
            vec![value_preview(output, 180)],
        ));
    }

    let mut content = entry.summary.clone();
    if let Some(error) = &entry.error {
        if !error.trim().is_empty() {
            content.push_str("\n");
            content.push_str(error);
        }
    }

    EventData::new(&entry.id, "hooks", hook_event_kind(entry.event))
        .content(content)
        .created_at(entry.timestamp)
        .tags(tags)
        .sig("")
        .verified(false)
}
