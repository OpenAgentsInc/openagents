use std::process::Command as ProcessCommand;
use std::sync::atomic::Ordering;

use agent_client_protocol_schema as acp;
use adjutant::{AcpChannelOutput, Adjutant, AdjutantError, Task as AdjutantTask, DSPY_META_KEY};
use tokio::sync::mpsc;

use crate::autopilot_loop::{AutopilotConfig, AutopilotLoop, AutopilotResult, DspyStage};
use crate::app::chat::MessageMetadata;
use crate::app::events::ResponseEvent;
use crate::app::{now_timestamp, AppState};

pub(crate) fn submit_autopilot_prompt(
    runtime_handle: &tokio::runtime::Handle,
    state: &mut AppState,
    prompt: String,
) {
    tracing::info!("Autopilot mode: starting autonomous loop");

    // Check which LM provider will be used
    let provider = adjutant::dspy::lm_config::detect_provider();
    tracing::info!("Autopilot: detected LM provider: {:?}", provider);

    // Create channels for receiving responses (same pattern as Claude)
    let (tx, rx) = mpsc::unbounded_channel();
    state.chat.response_rx = Some(rx);
    state.chat.is_thinking = true;
    state.chat.streaming_markdown.reset();

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
            match oanix::boot().await {
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

        match Adjutant::new(manifest.clone()) {
            Ok(adjutant) => {
                tracing::info!("Autopilot: Adjutant initialized, starting autonomous loop");

                // Configure dsrs global settings with detected LM provider
                if let Err(e) = adjutant::dspy::lm_config::configure_dsrs().await {
                    tracing::error!("Autopilot: failed to configure dsrs: {}", e);
                    let _ = tx.send(ResponseEvent::Error(format!(
                        "Failed to configure LM: {}",
                        e
                    )));
                    window.request_redraw();
                    return;
                }
                tracing::info!("Autopilot: dsrs configured");

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
                let session_id = acp::SessionId::new(format!("autopilot-{}", now_timestamp()));

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
        acp::SessionUpdate::AgentMessageChunk(chunk) => acp_chunk_to_events(&chunk),
        acp::SessionUpdate::AgentThoughtChunk(chunk) => acp_chunk_to_events(&chunk),
        acp::SessionUpdate::Plan(plan) => vec![ResponseEvent::Chunk(format_plan_update(&plan))],
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

fn acp_chunk_to_events(chunk: &acp::ContentChunk) -> Vec<ResponseEvent> {
    let mut events = Vec::new();
    if let Some(stage) = acp_content_dspy_stage(&chunk.content) {
        events.push(ResponseEvent::DspyStage(stage));
    }
    if let Some(text) = acp_content_text(&chunk.content) {
        events.push(ResponseEvent::Chunk(text));
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

fn format_plan_update(plan: &acp::Plan) -> String {
    if plan.entries.is_empty() {
        return "Plan updated (no entries).".to_string();
    }
    let mut output = String::from("Plan updated:\n");
    for (idx, entry) in plan.entries.iter().enumerate() {
        let status = match entry.status {
            acp::PlanEntryStatus::Pending => "pending",
            acp::PlanEntryStatus::InProgress => "in_progress",
            acp::PlanEntryStatus::Completed => "completed",
            _ => "unknown",
        };
        output.push_str(&format!(
            "{}. [{}] {}\n",
            idx + 1,
            status,
            entry.content
        ));
    }
    output.trim_end().to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acp_message_chunk_maps_to_response() {
        let notification = acp::SessionNotification::new(
            acp::SessionId::new("test"),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                acp::ContentBlock::Text(acp::TextContent::new("hello")),
            )),
        );
        let events = acp_notification_to_response(notification);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ResponseEvent::Chunk(text) => assert_eq!(text, "hello"),
            _ => panic!("expected chunk event"),
        }
    }

    #[test]
    fn acp_plan_maps_to_chunk() {
        let plan = acp::Plan::new(vec![acp::PlanEntry::new(
            "Do the thing",
            acp::PlanEntryPriority::Medium,
            acp::PlanEntryStatus::Pending,
        )]);
        let notification =
            acp::SessionNotification::new(acp::SessionId::new("test"), acp::SessionUpdate::Plan(plan));
        let events = acp_notification_to_response(notification);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ResponseEvent::Chunk(text) => {
                assert!(text.contains("Plan updated"));
                assert!(text.contains("Do the thing"));
            }
            _ => panic!("expected chunk event"),
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
            ResponseEvent::Chunk(text) => assert_eq!(text, "done"),
            _ => panic!("expected chunk event"),
        }
    }

    #[test]
    fn acp_tool_call_update_maps_to_tool_result() {
        let tool_call_id = acp::ToolCallId::new("tool-1");
        let tool_call = acp::ToolCall::new(tool_call_id.clone(), "Read").raw_input(
            serde_json::json!({
                "path": "README.md"
            }),
        );
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
        assert!(start_events
            .iter()
            .any(|event| matches!(event, ResponseEvent::ToolCallStart { .. })));
        assert!(start_events
            .iter()
            .any(|event| matches!(event, ResponseEvent::ToolCallInput { .. })));

        let done_events = acp_notification_to_response(done);
        assert!(done_events
            .iter()
            .any(|event| matches!(event, ResponseEvent::ToolResult { .. })));
    }
}
