use std::process::Command as ProcessCommand;
use std::sync::atomic::Ordering;

use adjutant::{Adjutant, AdjutantError, Task as AdjutantTask};
use tokio::sync::mpsc;

use crate::autopilot_loop::{
    AutopilotConfig, AutopilotLoop, AutopilotResult, ChannelOutput, DspyStage,
};
use crate::app::chat::MessageMetadata;
use crate::app::events::ResponseEvent;
use crate::app::AppState;

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

                // Create channel for streaming tokens to UI
                let (token_tx, mut token_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                // Register UI callback for dsrs events so user sees LLM call progress
                let ui_callback = super::UiDspyCallback::new(token_tx.clone());
                dsrs::set_callback(ui_callback);

                // Spawn task to forward tokens to the response channel
                let tx_clone = tx.clone();
                let window_clone = window.clone();
                tokio::spawn(async move {
                    let mut buffer = String::new();
                    while let Some(token) = token_rx.recv().await {
                        buffer.push_str(&token);

                        // Check for complete DSPY_STAGE markers
                        while let Some(start) = buffer.find("<<DSPY_STAGE:") {
                            if let Some(end) = buffer[start..].find(":DSPY_STAGE>>") {
                                // Extract text before the marker
                                if start > 0 {
                                    let before = buffer[..start].to_string();
                                    if !before.trim().is_empty() {
                                        let _ = tx_clone.send(ResponseEvent::Chunk(before));
                                    }
                                }

                                // Extract and parse the JSON
                                let json_start = start + "<<DSPY_STAGE:".len();
                                let json_end = start + end;
                                let json_str = &buffer[json_start..json_end];

                                match serde_json::from_str::<DspyStage>(json_str) {
                                    Ok(stage) => {
                                        let _ = tx_clone.send(ResponseEvent::DspyStage(stage));
                                    }
                                    Err(e) => {
                                        // Log with more context: error reason and truncated JSON
                                        let preview = if json_str.len() > 200 {
                                            format!("{}...", &json_str[..200])
                                        } else {
                                            json_str.to_string()
                                        };
                                        tracing::warn!("Failed to parse DSPY_STAGE: {} | JSON: {}", e, preview);
                                    }
                                }

                                // Remove processed content from buffer
                                let after_marker = start + end + ":DSPY_STAGE>>".len();
                                buffer = buffer[after_marker..].to_string();
                            } else {
                                // Incomplete marker, wait for more data
                                break;
                            }
                        }

                        // Send any remaining text that doesn't contain a partial marker
                        if !buffer.contains("<<DSPY_STAGE:") && !buffer.is_empty() {
                            let _ = tx_clone.send(ResponseEvent::Chunk(buffer.clone()));
                            buffer.clear();
                        }

                        window_clone.request_redraw();
                    }

                    // Send any remaining buffer content
                    if !buffer.is_empty() {
                        let _ = tx_clone.send(ResponseEvent::Chunk(buffer));
                    }
                });

                // Configure the autopilot loop
                let config = AutopilotConfig {
                    max_iterations,
                    workspace_root,
                    verify_completion: true,
                };

                // Create and run the autopilot loop
                let channel_output = ChannelOutput::new(token_tx);
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
