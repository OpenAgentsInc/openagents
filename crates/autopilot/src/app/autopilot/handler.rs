use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::atomic::Ordering;

use adjutant::{
    AcpChannelOutput, Adjutant, AdjutantError, DSPY_META_KEY, Task as AdjutantTask,
    generate_session_id,
};
use agent_client_protocol_schema as acp;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::app::chat::MessageMetadata;
use crate::app::codex_runtime::{CodexRuntime, CodexRuntimeConfig};
use crate::app::events::ResponseEvent;
use crate::autopilot_loop::{AutopilotConfig, AutopilotLoop, AutopilotResult, DspyStage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutopilotDispatchMode {
    LocalFirst,
    LocalOnly,
    RemoteOnly,
}

impl AutopilotDispatchMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::LocalFirst => "local_first",
            Self::LocalOnly => "local_only",
            Self::RemoteOnly => "remote_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutopilotExecutionLane {
    LocalCodex,
    RemoteFallback,
}

impl AutopilotExecutionLane {
    fn as_str(self) -> &'static str {
        match self {
            Self::LocalCodex => "local_codex",
            Self::RemoteFallback => "remote_fallback",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalCodexProbe {
    available: bool,
    healthy: bool,
    reason: Option<String>,
}

impl LocalCodexProbe {
    fn healthy() -> Self {
        Self {
            available: true,
            healthy: true,
            reason: None,
        }
    }

    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            healthy: false,
            reason: Some(reason.into()),
        }
    }

    fn unhealthy(reason: impl Into<String>) -> Self {
        Self {
            available: true,
            healthy: false,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutopilotDispatchDecision {
    mode: AutopilotDispatchMode,
    lane: AutopilotExecutionLane,
    reason: String,
}

#[derive(Debug, Serialize)]
struct LocalReplayQueueEntry {
    timestamp_ms: u64,
    dispatch_mode: String,
    lane: String,
    reason: String,
    workspace_root: String,
    prompt_sha256: String,
    local_codex_available: bool,
    local_codex_healthy: bool,
}

fn parse_autopilot_dispatch_mode(value: Option<&str>) -> AutopilotDispatchMode {
    match value
        .unwrap_or("local_first")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "local_only" => AutopilotDispatchMode::LocalOnly,
        "remote_only" => AutopilotDispatchMode::RemoteOnly,
        _ => AutopilotDispatchMode::LocalFirst,
    }
}

fn resolve_autopilot_dispatch_mode() -> AutopilotDispatchMode {
    parse_autopilot_dispatch_mode(std::env::var("OA_AUTOPILOT_DISPATCH_MODE").ok().as_deref())
}

async fn probe_local_codex_health(cwd: &Path) -> LocalCodexProbe {
    if !CodexRuntime::is_available() {
        return LocalCodexProbe::unavailable("codex_binary_not_found");
    }

    let runtime = match CodexRuntime::spawn(CodexRuntimeConfig {
        cwd: Some(cwd.to_path_buf()),
        wire_log: None,
    })
    .await
    {
        Ok(runtime) => runtime,
        Err(err) => {
            return LocalCodexProbe::unhealthy(format!("spawn_failed: {}", err));
        }
    };

    match runtime.shutdown().await {
        Ok(()) => LocalCodexProbe::healthy(),
        Err(err) => LocalCodexProbe::unhealthy(format!("shutdown_failed: {}", err)),
    }
}

fn select_autopilot_dispatch_lane(
    mode: AutopilotDispatchMode,
    probe: &LocalCodexProbe,
) -> Result<AutopilotDispatchDecision, String> {
    match mode {
        AutopilotDispatchMode::RemoteOnly => Ok(AutopilotDispatchDecision {
            mode,
            lane: AutopilotExecutionLane::RemoteFallback,
            reason: "dispatch_mode_remote_only".to_string(),
        }),
        AutopilotDispatchMode::LocalFirst => {
            if probe.available && probe.healthy {
                Ok(AutopilotDispatchDecision {
                    mode,
                    lane: AutopilotExecutionLane::LocalCodex,
                    reason: "local_codex_healthy".to_string(),
                })
            } else {
                Ok(AutopilotDispatchDecision {
                    mode,
                    lane: AutopilotExecutionLane::RemoteFallback,
                    reason: probe
                        .reason
                        .clone()
                        .unwrap_or_else(|| "local_codex_unavailable".to_string()),
                })
            }
        }
        AutopilotDispatchMode::LocalOnly => {
            if probe.available && probe.healthy {
                Ok(AutopilotDispatchDecision {
                    mode,
                    lane: AutopilotExecutionLane::LocalCodex,
                    reason: "local_codex_healthy".to_string(),
                })
            } else {
                Err(probe
                    .reason
                    .clone()
                    .unwrap_or_else(|| "local_codex_unavailable".to_string()))
            }
        }
    }
}

fn local_replay_queue_path() -> PathBuf {
    if let Ok(path) = std::env::var("OA_AUTOPILOT_LOCAL_REPLAY_QUEUE_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    crate::app::config::sessions_dir().join("autopilot-local-replay-queue.jsonl")
}

fn append_local_replay_queue_entry(entry: &LocalReplayQueueEntry) {
    let path = local_replay_queue_path();
    append_local_replay_queue_entry_at(&path, entry);
}

fn append_local_replay_queue_entry_at(path: &Path, entry: &LocalReplayQueueEntry) {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            tracing::warn!(error = %err, path = %parent.display(), "Failed to create replay queue directory");
            return;
        }
    }

    let mut file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => file,
        Err(err) => {
            tracing::warn!(error = %err, path = %path.display(), "Failed to open local replay queue");
            return;
        }
    };

    let line = match serde_json::to_string(entry) {
        Ok(line) => line,
        Err(err) => {
            tracing::warn!(error = %err, "Failed to encode local replay queue entry");
            return;
        }
    };

    if let Err(err) = writeln!(file, "{}", line) {
        tracing::warn!(error = %err, path = %path.display(), "Failed to append local replay queue entry");
    }
}

fn prompt_sha256_hex(prompt: &str) -> String {
    format!("{:x}", Sha256::digest(prompt.as_bytes()))
}

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn submit_autopilot_prompt(
    runtime_handle: &tokio::runtime::Handle,
    state: &mut AppState,
    prompt: String,
) {
    tracing::info!("Autopilot mode: starting autonomous loop");

    // Backend selection controls chat UX; Autopilot lane is selected by dispatch policy.
    let selected_backend = state.agent_selection.agent;
    tracing::info!(
        "Autopilot: preparing execution lane (selected backend={:?})",
        selected_backend
    );

    // Create channels for receiving responses.
    let (tx, rx) = mpsc::unbounded_channel();
    state.chat.response_rx = Some(rx);
    state.chat.is_thinking = true;
    state.chat.streaming_markdown.reset();
    state.chat.streaming_thought.reset();

    // Reset interrupt flag for new loop
    state
        .autopilot
        .autopilot_interrupt_flag
        .store(false, Ordering::Relaxed);
    state.autopilot.autopilot_loop_iteration = 0;
    let interrupt_flag = state.autopilot.autopilot_interrupt_flag.clone();
    let max_iterations = state.autopilot.autopilot_max_iterations;

    let window = state.window.clone();
    let prompt_clone = prompt.clone();

    // Use cached manifest, wait for pending boot, or start new boot
    let cached_manifest = state.autopilot.oanix_manifest.clone();
    let pending_rx = state.autopilot.oanix_manifest_rx.take(); // Take pending boot rx if any

    // Create channel to send manifest back for caching (if we might get a new manifest)
    let manifest_tx = if cached_manifest.is_none() {
        let (mtx, mrx) = mpsc::unbounded_channel();
        state.autopilot.oanix_manifest_rx = Some(mrx);
        Some(mtx)
    } else {
        None
    };

    runtime_handle.spawn(async move {
        // Get manifest: cached > pending boot > new boot
        let manifest = if let Some(m) = cached_manifest {
            tracing::info!("Autopilot: using cached OANIX manifest");
            m
        } else if let Some(mut rx) = pending_rx {
            tracing::info!("Autopilot: waiting for startup OANIX boot...");
            match rx.recv().await {
                Some(m) => {
                    tracing::info!("Autopilot: received OANIX manifest from startup boot");
                    // Send manifest back for caching
                    if let Some(mtx) = &manifest_tx {
                        let _ = mtx.send(m.clone());
                    }
                    m
                }
                None => {
                    tracing::error!("Autopilot: startup OANIX boot channel closed");
                    let _ = tx.send(ResponseEvent::Error("OANIX boot failed".to_string()));
                    window.request_redraw();
                    return;
                }
            }
        } else {
            tracing::info!("Autopilot: booting OANIX...");
            match adjutant::boot().await {
                Ok(m) => {
                    tracing::info!(
                        "Autopilot: OANIX booted, workspace: {:?}",
                        m.workspace.as_ref().map(|w| &w.root)
                    );
                    // Send manifest back for caching
                    if let Some(mtx) = &manifest_tx {
                        let _ = mtx.send(m.clone());
                    }
                    m
                }
                Err(e) => {
                    tracing::error!("Autopilot: OANIX boot failed: {}", e);
                    let _ = tx.send(ResponseEvent::Error(format!("OANIX boot failed: {}", e)));
                    window.request_redraw();
                    return;
                }
            }
        };

        // Get workspace root for verification commands
        let workspace_root = manifest
            .workspace
            .as_ref()
            .map(|w| w.root.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let dispatch_mode = resolve_autopilot_dispatch_mode();
        let local_codex_probe = if matches!(dispatch_mode, AutopilotDispatchMode::RemoteOnly) {
            LocalCodexProbe::unavailable("dispatch_mode_remote_only")
        } else {
            probe_local_codex_health(&workspace_root).await
        };
        let dispatch_decision =
            match select_autopilot_dispatch_lane(dispatch_mode, &local_codex_probe) {
                Ok(decision) => decision,
                Err(reason) => {
                    let _ = tx.send(ResponseEvent::Error(format!(
                        "Autopilot local-only mode requires healthy local Codex: {}",
                        reason
                    )));
                    window.request_redraw();
                    return;
                }
            };

        tracing::info!(
            mode = dispatch_decision.mode.as_str(),
            lane = dispatch_decision.lane.as_str(),
            reason = %dispatch_decision.reason,
            local_codex_available = local_codex_probe.available,
            local_codex_healthy = local_codex_probe.healthy,
            "Autopilot dispatch lane selected"
        );

        append_local_replay_queue_entry(&LocalReplayQueueEntry {
            timestamp_ms: current_timestamp_ms(),
            dispatch_mode: dispatch_decision.mode.as_str().to_string(),
            lane: dispatch_decision.lane.as_str().to_string(),
            reason: dispatch_decision.reason.clone(),
            workspace_root: workspace_root.display().to_string(),
            prompt_sha256: prompt_sha256_hex(&prompt_clone),
            local_codex_available: local_codex_probe.available,
            local_codex_healthy: local_codex_probe.healthy,
        });

        if matches!(dispatch_decision.lane, AutopilotExecutionLane::LocalCodex) {
            tracing::info!("Autopilot: routing to local Codex app-server lane");

            // Build enriched prompt with OANIX context
            let full_prompt = build_autopilot_context(&manifest, &workspace_root, &prompt_clone);

            tracing::info!(
                "Autopilot: context gathered for local Codex (directives: {}, issues: {})",
                manifest
                    .workspace
                    .as_ref()
                    .map(|w| w.directives.len())
                    .unwrap_or(0),
                manifest
                    .workspace
                    .as_ref()
                    .map(|w| w.issues.len())
                    .unwrap_or(0)
            );

            let _ = tx.send(ResponseEvent::SystemMessage(format!(
                "Autopilot lane: local Codex ({})",
                dispatch_decision.reason
            )));
            // Signal to UI to submit this prompt via Codex
            let _ = tx.send(ResponseEvent::CodexPromptReady(full_prompt));
            window.request_redraw();
            return;
        }

        let _ = tx.send(ResponseEvent::SystemMessage(format!(
            "Autopilot lane: remote fallback ({})",
            dispatch_decision.reason
        )));

        let provider = adjutant::dspy::lm_config::detect_provider_skip_codex();
        tracing::info!("Autopilot: using fallback provider {:?}", provider);
        let model_name = provider.map(|p| p.short_name().to_string());

        // dsrs/Adjutant path (for local models like llama.cpp, Ollama, Pylon, etc.)
        match Adjutant::new(manifest.clone()) {
            Ok(adjutant) => {
                tracing::info!("Autopilot: Adjutant initialized, starting autonomous loop");

                // Configure dsrs global settings with user-selected backend preference
                if let Err(e) = adjutant::dspy::lm_config::configure_dsrs_with_preference(false).await {
                    tracing::error!("Autopilot: failed to configure dsrs: {}", e);
                    let _ = tx.send(ResponseEvent::Error(format!(
                        "Failed to configure LM: {}",
                        e
                    )));
                    window.request_redraw();
                    return;
                }
                tracing::info!("Autopilot: dsrs configured");

                // Note: Issue suggestions are already run during bootloader (coder_actions.rs)
                // when the OANIX manifest arrives, so we don't duplicate them here.

                // Build enriched prompt with OANIX context
                let full_prompt = build_autopilot_context(&manifest, &workspace_root, &prompt_clone);

                tracing::info!(
                    "Autopilot: context gathered (directives: {}, issues: {})",
                    manifest
                        .workspace
                        .as_ref()
                        .map(|w| w.directives.len())
                        .unwrap_or(0),
                    manifest
                        .workspace
                        .as_ref()
                        .map(|w| w.issues.len())
                        .unwrap_or(0)
                );

                // Create task with enriched prompt
                let task = AdjutantTask::new("autopilot", "User Request", &full_prompt);

                // Create channel for streaming ACP notifications to UI
                let (acp_tx, mut acp_rx) =
                    tokio::sync::mpsc::unbounded_channel::<acp::SessionNotification>();
                let session_id = acp::SessionId::new(generate_session_id());

                // Spawn task to forward ACP notifications to the response channel
                let tx_clone = tx.clone();
                let window_clone = window.clone();
                tokio::spawn(async move {
                    while let Some(notification) = acp_rx.recv().await {
                        let events = acp_notification_to_response(notification);
                        for event in events {
                            let _ = tx_clone.send(event);
                        }
                        window_clone.request_redraw();
                    }
                });

                // Configure the autopilot loop
                let config = AutopilotConfig {
                    max_iterations,
                    workspace_root,
                    verify_completion: true,
                };

                // Create and run the autopilot loop
                let channel_output = AcpChannelOutput::new(session_id, acp_tx);
                let autopilot_loop =
                    AutopilotLoop::new(adjutant, task, config, channel_output, interrupt_flag);

                let start_time = std::time::Instant::now();
                let result = autopilot_loop.run().await;
                let duration_ms = start_time.elapsed().as_millis() as u64;

                // Handle loop result
                match result {
                    AutopilotResult::Success(task_result) => {
                        tracing::info!("Autopilot: task completed successfully");
                        if !task_result.modified_files.is_empty() {
                            let files = task_result.modified_files.join(", ");
                            let _ = tx.send(ResponseEvent::Chunk(format!(
                                "\n\n**Modified files:** {}",
                                files
                            )));
                        }
                        let metadata = Some(MessageMetadata {
                            model: model_name.clone(),
                            duration_ms: Some(duration_ms),
                            ..Default::default()
                        });
                        let _ = tx.send(ResponseEvent::Complete { metadata });
                    }
                    AutopilotResult::Failed(task_result) => {
                        tracing::warn!("Autopilot: task failed definitively");
                        let error_msg =
                            task_result.error.unwrap_or_else(|| "Unknown error".to_string());
                        let _ = tx.send(ResponseEvent::Chunk(format!(
                            "\n\n**Task failed:** {}",
                            error_msg
                        )));
                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                    }
                    AutopilotResult::MaxIterationsReached {
                        iterations,
                        last_result,
                    } => {
                        tracing::warn!(
                            "Autopilot: max iterations ({}) reached",
                            iterations
                        );
                        let _ = tx.send(ResponseEvent::Chunk(format!(
                            "\n\n**Max iterations ({}) reached.** Send another message to continue.",
                            iterations
                        )));
                        if let Some(result) = last_result {
                            if !result.modified_files.is_empty() {
                                let files = result.modified_files.join(", ");
                                let _ = tx.send(ResponseEvent::Chunk(format!(
                                    "\n\n**Modified files so far:** {}",
                                    files
                                )));
                            }
                        }
                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                    }
                    AutopilotResult::UserInterrupted { iterations } => {
                        tracing::info!(
                            "Autopilot: interrupted by user after {} iterations",
                            iterations
                        );
                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                    }
                    AutopilotResult::Error(error) => {
                        tracing::error!("Autopilot: error during execution: {}", error);
                        let _ = tx.send(ResponseEvent::Error(format!(
                            "Autopilot error: {}",
                            error
                        )));
                    }
                }
            }
            Err(AdjutantError::NoWorkspace) => {
                tracing::warn!("Autopilot: no workspace found");
                let _ = tx.send(ResponseEvent::Chunk(
                    "Autopilot requires an OpenAgents workspace.\n\n\
                     Run `oanix init` in your project directory to create one."
                        .to_string(),
                ));
                let _ = tx.send(ResponseEvent::Complete { metadata: None });
            }
            Err(e) => {
                tracing::error!("Autopilot: failed to initialize Adjutant: {}", e);
                let _ = tx.send(ResponseEvent::Error(format!(
                    "Failed to initialize Adjutant: {}",
                    e
                )));
            }
        }
        window.request_redraw();
    });

    state.window.request_redraw();
}

pub(crate) fn acp_notification_to_response(
    notification: acp::SessionNotification,
) -> Vec<ResponseEvent> {
    match notification.update {
        acp::SessionUpdate::AgentMessageChunk(chunk) => {
            acp_chunk_to_events(&chunk, ResponseEvent::Chunk)
        }
        acp::SessionUpdate::AgentThoughtChunk(chunk) => {
            acp_chunk_to_events(&chunk, ResponseEvent::ThoughtChunk)
        }
        acp::SessionUpdate::Plan(plan) => vec![ResponseEvent::DspyStage(plan_to_todo_stage(&plan))],
        acp::SessionUpdate::ToolCall(tool_call) => {
            let mut events = Vec::new();
            let tool_use_id = tool_call.tool_call_id.to_string();
            events.push(ResponseEvent::ToolCallStart {
                name: tool_call.title,
                tool_use_id: tool_use_id.clone(),
            });
            if let Some(raw_input) = tool_call.raw_input {
                events.push(ResponseEvent::ToolCallInput {
                    json: raw_input.to_string(),
                });
            }
            events
        }
        acp::SessionUpdate::ToolCallUpdate(update) => {
            let mut events = Vec::new();
            if let Some(status) = update.fields.status {
                if matches!(
                    status,
                    acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed
                ) {
                    let raw_output = update.fields.raw_output.clone();
                    let is_error = matches!(status, acp::ToolCallStatus::Failed);
                    let content = raw_output
                        .as_ref()
                        .and_then(format_tool_output)
                        .unwrap_or_else(|| {
                            if is_error {
                                "Tool failed.".to_string()
                            } else {
                                "Tool completed.".to_string()
                            }
                        });
                    events.push(ResponseEvent::ToolResult {
                        content,
                        is_error,
                        tool_use_id: Some(update.tool_call_id.to_string()),
                        exit_code: None,
                        output_value: raw_output,
                    });
                }
            }
            events
        }
        acp::SessionUpdate::UserMessageChunk(_)
        | acp::SessionUpdate::AvailableCommandsUpdate(_)
        | acp::SessionUpdate::CurrentModeUpdate(_) => Vec::new(),
        _ => Vec::new(),
    }
}

fn acp_chunk_to_events(
    chunk: &acp::ContentChunk,
    text_event: fn(String) -> ResponseEvent,
) -> Vec<ResponseEvent> {
    let mut events = Vec::new();
    if let Some(stage) = acp_content_dspy_stage(&chunk.content) {
        events.push(ResponseEvent::DspyStage(stage));
    }
    if let Some(text) = acp_content_text(&chunk.content) {
        events.push(text_event(text));
    }
    events
}

fn acp_content_dspy_stage(content: &acp::ContentBlock) -> Option<DspyStage> {
    let meta = match content {
        acp::ContentBlock::Text(text) => text.meta.as_ref(),
        _ => None,
    }?;
    let stage_value = meta.get(DSPY_META_KEY)?;
    serde_json::from_value(stage_value.clone()).ok()
}

fn acp_content_text(content: &acp::ContentBlock) -> Option<String> {
    match content {
        acp::ContentBlock::Text(text) => Some(text.text.clone()),
        _ => None,
    }
}

fn plan_to_todo_stage(plan: &acp::Plan) -> DspyStage {
    let tasks = plan
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let (index, description) = parse_plan_entry(&entry.content, idx + 1);
            let status = match entry.status {
                acp::PlanEntryStatus::Pending => crate::autopilot_loop::TodoStatus::Pending,
                acp::PlanEntryStatus::InProgress => crate::autopilot_loop::TodoStatus::InProgress,
                acp::PlanEntryStatus::Completed => crate::autopilot_loop::TodoStatus::Complete,
                _ => crate::autopilot_loop::TodoStatus::Pending,
            };
            crate::autopilot_loop::TodoTask {
                index,
                description,
                status,
            }
        })
        .collect();
    DspyStage::TodoList { tasks }
}

fn parse_plan_entry(content: &str, fallback_index: usize) -> (usize, String) {
    if let Some((prefix, rest)) = content.split_once(". ") {
        if let Ok(index) = prefix.parse::<usize>() {
            let description = if rest.is_empty() {
                content.to_string()
            } else {
                rest.to_string()
            };
            return (index, description);
        }
    }
    (fallback_index, content.to_string())
}

fn format_tool_output(value: &serde_json::Value) -> Option<String> {
    if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
        return Some(content.to_string());
    }
    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Some(format!("Error: {}", error));
    }
    let stdout = value.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
    let stderr = value.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
    if !stdout.is_empty() || !stderr.is_empty() {
        if !stdout.is_empty() && !stderr.is_empty() {
            return Some(format!("{}\n\nstderr:\n{}", stdout, stderr));
        }
        if !stdout.is_empty() {
            return Some(stdout.to_string());
        }
        if !stderr.is_empty() {
            return Some(stderr.to_string());
        }
    }
    Some(value.to_string())
}

/// Build enriched prompt with OANIX context (issues, directives, recent commits).
/// Used for both Codex and dsrs/Adjutant paths.
fn build_autopilot_context(
    manifest: &adjutant::OanixManifest,
    workspace_root: &std::path::Path,
    user_prompt: &str,
) -> String {
    let mut context_parts = Vec::new();

    // Add active directive info
    if let Some(workspace) = &manifest.workspace {
        // Find active directive
        if let Some(active_id) = &workspace.active_directive {
            if let Some(directive) = workspace.directives.iter().find(|d| &d.id == active_id) {
                let priority = directive.priority.as_deref().unwrap_or("unknown");
                let progress = directive.progress_pct.unwrap_or(0);
                context_parts.push(format!(
                    "## Active Directive\n{}: {} (Priority: {}, Progress: {}%)",
                    directive.id, directive.title, priority, progress
                ));
            }
        }

        // Add open issues
        let open_issues: Vec<_> = workspace
            .issues
            .iter()
            .filter(|i| i.status == "open" && !i.is_blocked)
            .take(5)
            .collect();

        if !open_issues.is_empty() {
            let mut issues_text = String::from("## Open Issues\n");
            for issue in open_issues {
                issues_text.push_str(&format!(
                    "- #{}: {} (Priority: {})\n",
                    issue.number, issue.title, issue.priority
                ));
            }
            context_parts.push(issues_text);
        }
    }

    // Get recent commits (quick git log)
    if let Ok(output) = ProcessCommand::new("git")
        .args(["log", "--oneline", "-5", "--no-decorate"])
        .current_dir(workspace_root)
        .output()
    {
        if output.status.success() {
            let commits = String::from_utf8_lossy(&output.stdout);
            if !commits.trim().is_empty() {
                context_parts.push(format!("## Recent Commits\n{}", commits.trim()));
            }
        }
    }

    // Build the full prompt with context
    if context_parts.is_empty() {
        user_prompt.to_string()
    } else {
        format!(
            "{}\n\n---\n\n## User Request\n{}\n\n\
            Use the tools to complete the task. Read relevant files, make changes, run tests.",
            context_parts.join("\n\n"),
            user_prompt
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acp_message_chunk_maps_to_response() {
        let notification = acp::SessionNotification::new(
            acp::SessionId::new("test"),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("hello"),
            ))),
        );
        let events = acp_notification_to_response(notification);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ResponseEvent::Chunk(text) => assert_eq!(text, "hello"),
            _ => panic!("expected chunk event"),
        }
    }

    #[test]
    fn acp_plan_maps_to_todo_stage() {
        let plan = acp::Plan::new(vec![acp::PlanEntry::new(
            "Do the thing",
            acp::PlanEntryPriority::Medium,
            acp::PlanEntryStatus::Pending,
        )]);
        let notification = acp::SessionNotification::new(
            acp::SessionId::new("test"),
            acp::SessionUpdate::Plan(plan),
        );
        let events = acp_notification_to_response(notification);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ResponseEvent::DspyStage(DspyStage::TodoList { tasks }) => {
                assert_eq!(tasks.len(), 1);
                assert_eq!(tasks[0].description, "Do the thing");
            }
            _ => panic!("expected todo list stage"),
        }
    }

    #[test]
    fn acp_dspy_meta_maps_to_stage() {
        let stage = DspyStage::Complete {
            total_tasks: 2,
            successful: 2,
            failed: 0,
        };
        let mut meta = acp::Meta::new();
        meta.insert(
            DSPY_META_KEY.to_string(),
            serde_json::to_value(&stage).expect("stage"),
        );
        let chunk = acp::ContentChunk::new(acp::ContentBlock::Text(
            acp::TextContent::new("done").meta(meta),
        ));
        let notification = acp::SessionNotification::new(
            acp::SessionId::new("test"),
            acp::SessionUpdate::AgentThoughtChunk(chunk),
        );
        let events = acp_notification_to_response(notification);
        assert_eq!(events.len(), 2);
        match &events[0] {
            ResponseEvent::DspyStage(DspyStage::Complete { total_tasks, .. }) => {
                assert_eq!(*total_tasks, 2);
            }
            _ => panic!("expected dspy stage event"),
        }
        match &events[1] {
            ResponseEvent::ThoughtChunk(text) => assert_eq!(text, "done"),
            _ => panic!("expected thought chunk event"),
        }
    }

    #[test]
    fn acp_thought_chunk_maps_to_thought_event() {
        let notification = acp::SessionNotification::new(
            acp::SessionId::new("test"),
            acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("thinking"),
            ))),
        );
        let events = acp_notification_to_response(notification);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ResponseEvent::ThoughtChunk(text) => assert_eq!(text, "thinking"),
            _ => panic!("expected thought chunk event"),
        }
    }

    #[test]
    fn acp_tool_call_update_maps_to_tool_result() {
        let tool_call_id = acp::ToolCallId::new("tool-1");
        let tool_call =
            acp::ToolCall::new(tool_call_id.clone(), "Read").raw_input(serde_json::json!({
                "path": "README.md"
            }));
        let start = acp::SessionNotification::new(
            acp::SessionId::new("test"),
            acp::SessionUpdate::ToolCall(tool_call),
        );
        let fields = acp::ToolCallUpdateFields::new()
            .status(acp::ToolCallStatus::Completed)
            .raw_output(serde_json::json!({
                "content": "ok"
            }));
        let update = acp::ToolCallUpdate::new(tool_call_id.clone(), fields);
        let done = acp::SessionNotification::new(
            acp::SessionId::new("test"),
            acp::SessionUpdate::ToolCallUpdate(update),
        );

        let start_events = acp_notification_to_response(start);
        assert!(
            start_events
                .iter()
                .any(|event| matches!(event, ResponseEvent::ToolCallStart { .. }))
        );
        assert!(
            start_events
                .iter()
                .any(|event| matches!(event, ResponseEvent::ToolCallInput { .. }))
        );

        let done_events = acp_notification_to_response(done);
        assert!(
            done_events
                .iter()
                .any(|event| matches!(event, ResponseEvent::ToolResult { .. }))
        );
    }

    #[test]
    fn dispatch_prefers_local_codex_when_healthy() {
        let probe = LocalCodexProbe::healthy();
        let decision =
            select_autopilot_dispatch_lane(AutopilotDispatchMode::LocalFirst, &probe).unwrap();
        assert_eq!(decision.lane, AutopilotExecutionLane::LocalCodex);
        assert_eq!(decision.reason, "local_codex_healthy");
    }

    #[test]
    fn dispatch_falls_back_when_local_codex_is_degraded() {
        let probe = LocalCodexProbe::unhealthy("spawn_failed: timeout");
        let decision =
            select_autopilot_dispatch_lane(AutopilotDispatchMode::LocalFirst, &probe).unwrap();
        assert_eq!(decision.lane, AutopilotExecutionLane::RemoteFallback);
        assert_eq!(decision.reason, "spawn_failed: timeout");
    }

    #[test]
    fn dispatch_remote_only_mode_forces_fallback_lane() {
        let probe = LocalCodexProbe::healthy();
        let decision =
            select_autopilot_dispatch_lane(AutopilotDispatchMode::RemoteOnly, &probe).unwrap();
        assert_eq!(decision.lane, AutopilotExecutionLane::RemoteFallback);
        assert_eq!(decision.reason, "dispatch_mode_remote_only");
    }

    #[test]
    fn replay_queue_entry_writes_jsonl() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let queue_path = temp_dir.path().join("autopilot-local-replay-queue.jsonl");
        let entry = LocalReplayQueueEntry {
            timestamp_ms: 123,
            dispatch_mode: "local_first".to_string(),
            lane: "local_codex".to_string(),
            reason: "local_codex_healthy".to_string(),
            workspace_root: "/tmp/workspace".to_string(),
            prompt_sha256: "abc123".to_string(),
            local_codex_available: true,
            local_codex_healthy: true,
        };
        append_local_replay_queue_entry_at(&queue_path, &entry);

        let data = std::fs::read_to_string(&queue_path).expect("queue data");
        assert!(data.contains("\"lane\":\"local_codex\""));
        assert!(data.contains("\"prompt_sha256\":\"abc123\""));
    }
}
