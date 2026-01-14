use std::process::Command as ProcessCommand;
use std::sync::atomic::Ordering;

use adjutant::{
    AcpChannelOutput, Adjutant, AdjutantError, DSPY_META_KEY, Task as AdjutantTask,
    generate_session_id,
};
use agent_client_protocol_schema as acp;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::app::chat::MessageMetadata;
use crate::app::config::AgentKindConfig;
use crate::app::events::ResponseEvent;
use crate::autopilot_loop::{AutopilotConfig, AutopilotLoop, AutopilotResult, DspyStage};

pub(crate) fn submit_autopilot_prompt(
    runtime_handle: &tokio::runtime::Handle,
    state: &mut AppState,
    prompt: String,
) {
    tracing::info!("Autopilot mode: starting autonomous loop");

    // Load .env.local for OPENAI_API_KEY and other local secrets if present.
    load_env_local();

    // Autopilot always routes execution to Adjutant; backend selection is for chat only.
    let selected_backend = state.agent_selection.agent;
    let use_codex = false;
    tracing::info!(
        "Autopilot: forcing Adjutant execution loop (selected backend={:?})",
        selected_backend
    );

    // Check which LM provider will be used (for logging only)
    let provider = if use_codex {
        adjutant::dspy::lm_config::detect_provider()
    } else {
        adjutant::dspy::lm_config::detect_provider_skip_codex()
    };
    tracing::info!("Autopilot: will use LM provider: {:?}", provider);

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

        // Get model name for metadata
        let model_name = adjutant::dspy::lm_config::detect_provider()
            .map(|p| p.short_name().to_string());

        // Get workspace root for verification commands
        let workspace_root = manifest
            .workspace
            .as_ref()
            .map(|w| w.root.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // EARLY BRANCH: If Codex is selected, skip dsrs/Adjutant and use CodexRuntime
        if use_codex {
            tracing::info!("Autopilot: Codex selected, routing to app-server");

            // Build enriched prompt with OANIX context
            let full_prompt = build_autopilot_context(&manifest, &workspace_root, &prompt_clone);

            tracing::info!(
                "Autopilot: context gathered for Codex (directives: {}, issues: {})",
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

            // Signal to UI to submit this prompt via Codex
            let _ = tx.send(ResponseEvent::CodexPromptReady(full_prompt));
            window.request_redraw();
            return;
        }

        // dsrs/Adjutant path (for local models like llama.cpp, Ollama, Pylon, etc.)
        match Adjutant::new(manifest.clone()) {
            Ok(adjutant) => {
                tracing::info!("Autopilot: Adjutant initialized, starting autonomous loop");

                // Configure dsrs global settings with user-selected backend preference
                if let Err(e) = adjutant::dspy::lm_config::configure_dsrs_with_preference(use_codex).await {
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

                // Build rich context from OANIX manifest
                let mut context_parts = Vec::new();

                // Add active directive info
                if let Some(workspace) = &manifest.workspace {
                    // Find active directive
                    if let Some(active_id) = &workspace.active_directive {
                        if let Some(directive) =
                            workspace.directives.iter().find(|d| &d.id == active_id)
                        {
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
                    .current_dir(&workspace_root)
                    .output()
                {
                    if output.status.success() {
                        let commits = String::from_utf8_lossy(&output.stdout);
                        if !commits.trim().is_empty() {
                            context_parts
                                .push(format!("## Recent Commits\n{}", commits.trim()));
                        }
                    }
                }

                // Build the full prompt with context
                let full_prompt = if context_parts.is_empty() {
                    prompt_clone.clone()
                } else {
                    format!(
                        "{}\n\n---\n\n## User Request\n{}\n\n\
                        Use the tools to complete the task. Read relevant files, make changes, run tests.",
                        context_parts.join("\n\n"),
                        prompt_clone
                    )
                };

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

fn load_env_local() {
    let path = std::path::Path::new(".env.local");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        if std::env::var(key).is_ok() {
            continue;
        }
        let mut value = value.trim().to_string();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }
        if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }
        std::env::set_var(key, value);
    }
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
}
