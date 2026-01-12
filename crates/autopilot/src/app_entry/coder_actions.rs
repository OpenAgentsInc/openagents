use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::oneshot;

use crate::app::agents::AgentBackendsEvent;
use crate::app::chat::{ChatMessage, MessageRole};
use crate::app::dvm::{DvmEvent, DvmStatus};
use crate::app::gateway::GatewayEvent;
use crate::app::lm_router::LmRouterEvent;
use crate::app::nexus::NexusEvent;
use crate::app::spark_wallet::SparkWalletEvent;
use crate::app::codex_app_server as app_server;
use crate::app::events::{CommandAction, ModalState, QueryControl, ResponseEvent};
use crate::app::nip28::{Nip28ConnectionStatus, Nip28Event, Nip28Message};
use crate::app::nip90::{Nip90ConnectionStatus, Nip90Event};
use crate::app::parsing::expand_prompt_text;
use crate::app::permissions::{coder_mode_default_allow, parse_coder_mode, PermissionPending};
use crate::app::permissions::request::{PermissionRequest, PermissionResult};
use crate::app::session::{SessionInfo, SessionUpdate};
use crate::app::ui::render_app;
use crate::app::CoderMode;
use crate::autopilot_loop::{DspyStage, TodoStatus, TodoTask};
use crate::commands::{Command, ReviewCommand, ReviewDelivery, ReviewTarget};

use super::AutopilotApp;
use super::command_palette_ids;
use super::COMMAND_PALETTE_ENABLED;
use super::commands::handle_command;

impl AutopilotApp {
    pub(super) fn submit_prompt(&mut self, prompt: String) {
        let Some(state) = &mut self.state else {
            return;
        };

        tracing::info!("Submitted prompt: {}", prompt);

        // Add user message to history
        state.chat.messages.push(ChatMessage {
            role: MessageRole::User,
            content: prompt.clone(),
            document: None,
            uuid: None,
            metadata: None,
        });

        if matches!(state.permissions.coder_mode, CoderMode::Autopilot) {
            crate::app::autopilot::submit_autopilot_prompt(&self.runtime_handle, state, prompt);
            return;
        }

        tracing::info!("Using Codex backend");
        self.submit_codex_prompt(prompt);
    }

    pub(super) fn start_review(&mut self, review: ReviewCommand) {
        let Some(state) = &mut self.state else {
            return;
        };

        if state.chat.is_thinking {
            state.push_system_message("Cannot start review during an active request.".to_string());
            return;
        }

        if !use_app_server_transport() {
            state.push_system_message("Review requires Codex app-server transport.".to_string());
            return;
        }

        let thread_id = state
            .session
            .session_info
            .codex_thread_id
            .clone()
            .or_else(|| {
                let session_id = state.session.session_info.session_id.trim();
                if session_id.starts_with("thr_") {
                    Some(session_id.to_string())
                } else {
                    None
                }
            });
        let Some(thread_id) = thread_id else {
            state.push_system_message("No active Codex session to review.".to_string());
            return;
        };

        let cwd = std::env::current_dir().unwrap_or_default();

        let (tx, rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        state.chat.response_rx = Some(rx);
        state.chat.query_control_tx = Some(control_tx);
        state.chat.is_thinking = true;
        state.chat.streaming_markdown.reset();
        state.catalogs.refresh_agent_cards(state.chat.is_thinking);

        let window = state.window.clone();
        let coder_mode = state.permissions.coder_mode;
        let (permission_tx, permission_rx) = mpsc::unbounded_channel();
        let (permission_action_tx, permission_action_rx) = mpsc::unbounded_channel();
        state.permissions.permission_requests_rx = Some(permission_rx);
        state.permissions.permission_action_tx = Some(permission_action_tx);
        state.permissions.permission_action_rx = Some(permission_action_rx);

        self.submit_codex_review_app_server(
            review,
            thread_id,
            cwd,
            coder_mode,
            tx,
            permission_tx,
            control_rx,
            window,
        );
    }

    /// Submit a prompt to the Codex backend.
    fn submit_codex_prompt(&mut self, prompt: String) {
        let Some(state) = &mut self.state else {
            return;
        };

        let cwd = std::env::current_dir().unwrap_or_default();
        let expanded_prompt = match expand_prompt_text(&prompt, &cwd) {
            Ok(result) => result,
            Err(err) => {
                state.push_system_message(err);
                state.window.request_redraw();
                return;
            }
        };

        // Create channel for receiving responses
        let (tx, rx) = mpsc::unbounded_channel();
        let (control_tx, mut control_rx) = mpsc::unbounded_channel();
        state.chat.response_rx = Some(rx);
        state.chat.query_control_tx = Some(control_tx);
        state.chat.is_thinking = true;
        state.chat.streaming_markdown.reset();
        state.catalogs.refresh_agent_cards(state.chat.is_thinking);

        // Get window handle for triggering redraws from async task
        let window = state.window.clone();
        // Resolve the pending session to a Codex thread id when possible; otherwise start fresh.
        let coder_mode = state.permissions.coder_mode;
        let pending_resume = state.session.pending_resume_session.take();
        let pending_fork = state.session.pending_fork_session;
        state.session.pending_fork_session = false;

        if use_app_server_transport() {
            tracing::info!("Using Codex app-server transport");
            let (permission_tx, permission_rx) = mpsc::unbounded_channel();
            let (permission_action_tx, permission_action_rx) = mpsc::unbounded_channel();
            state.permissions.permission_requests_rx = Some(permission_rx);
            state.permissions.permission_action_tx = Some(permission_action_tx);
            state.permissions.permission_action_rx = Some(permission_action_rx);
            let resume_thread_id = pending_resume
                .as_ref()
                .and_then(|session_id| resolve_codex_thread_id(&state.session, session_id));
            self.submit_codex_prompt_app_server(
                expanded_prompt,
                cwd,
                coder_mode,
                resume_thread_id,
                pending_fork,
                tx,
                permission_tx,
                control_rx,
                window,
            );
            return;
        }

        // Spawn async Codex query task
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions, ThreadEvent, ThreadItemDetails};

            let codex = Codex::new();

            // Build thread options - let Codex use its default model
            let mut thread_options = ThreadOptions::new()
                .working_directory(&cwd)
                .skip_git_repo_check(true);

            // Map permission mode to sandbox mode
            thread_options = thread_options
                .sandbox_mode(coder_mode.sandbox_mode())
                .approval_policy(coder_mode.approval_mode());

            // Create new thread (resume not supported yet for Codex)
            let mut thread = codex.start_thread(thread_options);

            // Start streaming turn
            tracing::info!("Starting Codex query");
            let turn_options = TurnOptions::default();
            match thread.run_streamed(expanded_prompt.as_str(), turn_options).await {
                Ok(mut streamed) => {
                    tracing::info!("Codex query stream started");

                    // Track text accumulation for agent messages
                    let mut current_message_text = String::new();

                    loop {
                        tokio::select! {
                            Some(control) = control_rx.recv() => {
                                match control {
                                    QueryControl::Interrupt | QueryControl::Abort => {
                                        let _ = tx.send(ResponseEvent::Error("Request interrupted.".to_string()));
                                        window.request_redraw();
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            event = streamed.next() => {
                                match event {
                                    Some(Ok(ev)) => {
                                        match ev {
                                            ThreadEvent::ThreadStarted(started) => {
                                                let mode_str = coder_mode.mode_label().to_string();
                                                let thread_id = started.thread_id;
                                                let _ = tx.send(ResponseEvent::SystemInit {
                                                    model: "codex".to_string(),
                                                    permission_mode: mode_str,
                                                    session_id: thread_id.clone(),
                                                    codex_thread_id: Some(thread_id),
                                                    tool_count: 0,
                                                    tools: vec![],
                                                    output_style: String::new(),
                                                    slash_commands: vec![],
                                                    mcp_servers: vec![],
                                                });
                                                window.request_redraw();
                                            }
                                            ThreadEvent::TurnStarted(_) => {
                                                tracing::debug!("Codex turn started");
                                            }
                                            ThreadEvent::ItemStarted(item) => {
                                                match &item.item.details {
                                                    ThreadItemDetails::AgentMessage(_) => {
                                                        current_message_text.clear();
                                                    }
                                                    ThreadItemDetails::CommandExecution(cmd) => {
                                                        let _ = tx.send(ResponseEvent::ToolCallStart {
                                                            name: "Bash".to_string(),
                                                            tool_use_id: item.item.id.clone(),
                                                        });
                                                        let _ = tx.send(ResponseEvent::ToolCallInput {
                                                            json: serde_json::json!({"command": cmd.command}).to_string(),
                                                        });
                                                        let _ = tx.send(ResponseEvent::ToolCallEnd);
                                                        window.request_redraw();
                                                    }
                                                    ThreadItemDetails::FileChange(fc) => {
                                                        let _ = tx.send(ResponseEvent::ToolCallStart {
                                                            name: "Edit".to_string(),
                                                            tool_use_id: item.item.id.clone(),
                                                        });
                                                        let paths: Vec<&str> = fc.changes.iter()
                                                            .map(|c| c.path.as_str())
                                                            .collect();
                                                        let _ = tx.send(ResponseEvent::ToolCallInput {
                                                            json: serde_json::json!({"files": paths}).to_string(),
                                                        });
                                                        let _ = tx.send(ResponseEvent::ToolCallEnd);
                                                        window.request_redraw();
                                                    }
                                                    ThreadItemDetails::McpToolCall(tool) => {
                                                        let _ = tx.send(ResponseEvent::ToolCallStart {
                                                            name: format!("mcp__{}__{}", tool.server, tool.tool),
                                                            tool_use_id: item.item.id.clone(),
                                                        });
                                                        let _ = tx.send(ResponseEvent::ToolCallInput {
                                                            json: tool.arguments.to_string(),
                                                        });
                                                        let _ = tx.send(ResponseEvent::ToolCallEnd);
                                                        window.request_redraw();
                                                    }
                                                    ThreadItemDetails::Reasoning(r) => {
                                                        let _ = tx.send(ResponseEvent::Chunk(
                                                            format!("*Thinking:* {}\n\n", r.text)
                                                        ));
                                                        window.request_redraw();
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            ThreadEvent::ItemUpdated(item) => {
                                                if let ThreadItemDetails::AgentMessage(msg) = &item.item.details {
                                                    // Send delta (new text since last update)
                                                    if msg.text.len() > current_message_text.len() {
                                                        let delta = &msg.text[current_message_text.len()..];
                                                        if tx.send(ResponseEvent::Chunk(delta.to_string())).is_err() {
                                                            break;
                                                        }
                                                        current_message_text = msg.text.clone();
                                                        window.request_redraw();
                                                    }
                                                }
                                            }
                                            ThreadEvent::ItemCompleted(item) => {
                                                match &item.item.details {
                                                    ThreadItemDetails::AgentMessage(msg) => {
                                                        // Send any remaining text
                                                        if msg.text.len() > current_message_text.len() {
                                                            let delta = &msg.text[current_message_text.len()..];
                                                            let _ = tx.send(ResponseEvent::Chunk(delta.to_string()));
                                                            window.request_redraw();
                                                        }
                                                        current_message_text.clear();
                                                    }
                                                    ThreadItemDetails::CommandExecution(cmd) => {
                                                        let _ = tx.send(ResponseEvent::ToolResult {
                                                            content: cmd.aggregated_output.clone(),
                                                            is_error: matches!(cmd.status, codex_agent_sdk::CommandExecutionStatus::Failed),
                                                            tool_use_id: Some(item.item.id.clone()),
                                                            exit_code: cmd.exit_code,
                                                            output_value: None,
                                                        });
                                                        window.request_redraw();
                                                    }
                                                    ThreadItemDetails::FileChange(fc) => {
                                                        let paths: Vec<&str> = fc.changes.iter()
                                                            .map(|c| c.path.as_str())
                                                            .collect();
                                                        let content = format!("Modified files: {}", paths.join(", "));
                                                        let _ = tx.send(ResponseEvent::ToolResult {
                                                            content,
                                                            is_error: matches!(fc.status, codex_agent_sdk::PatchApplyStatus::Failed),
                                                            tool_use_id: Some(item.item.id.clone()),
                                                            exit_code: None,
                                                            output_value: None,
                                                        });
                                                        window.request_redraw();
                                                    }
                                                    ThreadItemDetails::McpToolCall(tool) => {
                                                        let content = if let Some(result) = &tool.result {
                                                            serde_json::to_string_pretty(&result.content)
                                                                .unwrap_or_default()
                                                        } else if let Some(err) = &tool.error {
                                                            err.message.clone()
                                                        } else {
                                                            String::new()
                                                        };
                                                        let _ = tx.send(ResponseEvent::ToolResult {
                                                            content,
                                                            is_error: tool.error.is_some(),
                                                            tool_use_id: Some(item.item.id.clone()),
                                                            exit_code: None,
                                                            output_value: None,
                                                        });
                                                        window.request_redraw();
                                                    }
                                                    ThreadItemDetails::Error(err) => {
                                                        let _ = tx.send(ResponseEvent::SystemMessage(
                                                            format!("Codex error: {}", err.message)
                                                        ));
                                                        window.request_redraw();
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            ThreadEvent::TurnCompleted(tc) => {
                                                let _ = tx.send(ResponseEvent::Complete {
                                                    metadata: Some(crate::app::chat::MessageMetadata {
                                                        model: Some("codex".to_string()),
                                                        input_tokens: Some(tc.usage.input_tokens as u64),
                                                        output_tokens: Some(tc.usage.output_tokens as u64),
                                                        duration_ms: None,
                                                        cost_msats: None,
                                                    }),
                                                });
                                                window.request_redraw();
                                                break;
                                            }
                                            ThreadEvent::TurnFailed(failed) => {
                                                let _ = tx.send(ResponseEvent::Error(failed.error.message));
                                                window.request_redraw();
                                                break;
                                            }
                                            ThreadEvent::Error(err) => {
                                                let _ = tx.send(ResponseEvent::Error(err.message));
                                                window.request_redraw();
                                                break;
                                            }
                                        }
                                    }
                                    Some(Err(e)) => {
                                        tracing::error!("Codex stream error: {}", e);
                                        let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                        window.request_redraw();
                                        break;
                                    }
                                    None => {
                                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                                        window.request_redraw();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    tracing::info!("Codex query stream ended");
                }
                Err(e) => {
                    tracing::error!("Codex query failed to start: {}", e);
                    let _ = tx.send(ResponseEvent::Error(e.to_string()));
                    window.request_redraw();
                }
            }
        });
    }

    fn submit_codex_prompt_app_server(
        &self,
        prompt: String,
        cwd: PathBuf,
        coder_mode: CoderMode,
        resume_thread_id: Option<String>,
        fork_session: bool,
        tx: mpsc::UnboundedSender<ResponseEvent>,
        permission_tx: mpsc::UnboundedSender<PermissionPending>,
        control_rx: mpsc::UnboundedReceiver<QueryControl>,
        window: Arc<winit::window::Window>,
    ) {
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            let wire_log = app_server::AppServerWireLog::new();
            let trace_log = TraceLogger::new();
            let (client, channels) = match app_server::AppServerClient::spawn(
                app_server::AppServerConfig {
                    cwd: Some(cwd.clone()),
                    wire_log: Some(wire_log.clone()),
                },
            )
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    let _ = tx.send(ResponseEvent::Error(format!(
                        "Failed to start codex app-server: {}",
                        err
                    )));
                    window.request_redraw();
                    return;
                }
            };
            let notification_rx = channels.notifications;
            let request_rx = channels.requests;
            let emitter = ResponseEmitter::new(tx.clone(), Some(trace_log.clone()));

            let client_info = app_server::ClientInfo {
                name: "autopilot".to_string(),
                title: Some("Autopilot".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            };

            if let Err(err) = client.initialize(client_info).await {
                let _ = tx.send(ResponseEvent::Error(format!(
                    "Failed to initialize codex app-server: {}",
                    err
                )));
                window.request_redraw();
                let _ = client.shutdown().await;
                return;
            }

            let (thread_id, model_name) = if let Some(resume_id) = resume_thread_id.clone()
                .filter(|_| !fork_session)
            {
                let resume_params = app_server::ThreadResumeParams {
                    thread_id: resume_id,
                    model: None,
                    model_provider: None,
                    cwd: Some(cwd.to_string_lossy().to_string()),
                    approval_policy: Some(approval_policy_for_mode(coder_mode)),
                    sandbox: Some(sandbox_mode_for_mode(coder_mode)),
                };
                let response = match client.thread_resume(resume_params).await {
                    Ok(response) => response,
                    Err(err) => {
                        let _ = emitter.send(ResponseEvent::Error(format!(
                            "Failed to resume codex thread: {}",
                            err
                        )));
                        window.request_redraw();
                        let _ = client.shutdown().await;
                        return;
                    }
                };
                let thread_id = response.thread.id.clone();
                let model_name = if response.model.is_empty() {
                    "codex".to_string()
                } else {
                    response.model.clone()
                };
                (thread_id, model_name)
            } else {
                let thread_params = app_server::ThreadStartParams {
                    model: None,
                    model_provider: None,
                    cwd: Some(cwd.to_string_lossy().to_string()),
                    approval_policy: Some(approval_policy_for_mode(coder_mode)),
                    sandbox: Some(sandbox_mode_for_mode(coder_mode)),
                };

                let response = match client.thread_start(thread_params).await {
                    Ok(response) => response,
                    Err(err) => {
                        let _ = emitter.send(ResponseEvent::Error(format!(
                            "Failed to start codex thread: {}",
                            err
                        )));
                        window.request_redraw();
                        let _ = client.shutdown().await;
                        return;
                    }
                };
                let thread_id = response.thread.id.clone();
                let model_name = if response.model.is_empty() {
                    "codex".to_string()
                } else {
                    response.model.clone()
                };
                (thread_id, model_name)
            };

            let run_id = current_timestamp_ms();
            let session_dir = crate::app::config::session_messages_dir(&thread_id);
            wire_log.set_path(session_dir.join(format!("wire-{}.jsonl", run_id)));
            trace_log.set_path(session_dir.join(format!("trace-{}.jsonl", run_id)));

            emitter.send(ResponseEvent::SystemInit {
                model: model_name.clone(),
                permission_mode: coder_mode.mode_label().to_string(),
                session_id: thread_id.clone(),
                codex_thread_id: Some(thread_id.clone()),
                tool_count: 0,
                tools: vec![],
                output_style: String::new(),
                slash_commands: vec![],
                mcp_servers: vec![],
            });
            window.request_redraw();

            let turn_params = app_server::TurnStartParams {
                thread_id: thread_id.clone(),
                input: vec![app_server::UserInput::Text { text: prompt }],
            };

            let turn_id = match client.turn_start(turn_params).await {
                Ok(response) => Some(response.turn.id),
                Err(err) => {
                    let _ = emitter.send(ResponseEvent::Error(format!(
                        "Failed to start codex turn: {}",
                        err
                    )));
                    window.request_redraw();
                    let _ = client.shutdown().await;
                    return;
                }
            };
            run_app_server_turn_loop(
                client,
                notification_rx,
                request_rx,
                emitter,
                coder_mode,
                permission_tx,
                control_rx,
                window,
                thread_id,
                model_name,
                turn_id,
            )
            .await;
        });
    }

    fn submit_codex_review_app_server(
        &self,
        review: ReviewCommand,
        thread_id: String,
        cwd: PathBuf,
        coder_mode: CoderMode,
        tx: mpsc::UnboundedSender<ResponseEvent>,
        permission_tx: mpsc::UnboundedSender<PermissionPending>,
        control_rx: mpsc::UnboundedReceiver<QueryControl>,
        window: Arc<winit::window::Window>,
    ) {
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            let wire_log = app_server::AppServerWireLog::new();
            let trace_log = TraceLogger::new();
            let (client, channels) = match app_server::AppServerClient::spawn(
                app_server::AppServerConfig {
                    cwd: Some(cwd.clone()),
                    wire_log: Some(wire_log.clone()),
                },
            )
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    let _ = tx.send(ResponseEvent::Error(format!(
                        "Failed to start codex app-server: {}",
                        err
                    )));
                    window.request_redraw();
                    return;
                }
            };

            let emitter = ResponseEmitter::new(tx.clone(), Some(trace_log.clone()));
            let notification_rx = channels.notifications;
            let request_rx = channels.requests;

            let client_info = app_server::ClientInfo {
                name: "autopilot".to_string(),
                title: Some("Autopilot".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            };

            if let Err(err) = client.initialize(client_info).await {
                let _ = emitter.send(ResponseEvent::Error(format!(
                    "Failed to initialize codex app-server: {}",
                    err
                )));
                window.request_redraw();
                let _ = client.shutdown().await;
                return;
            }

            let resume_params = app_server::ThreadResumeParams {
                thread_id: thread_id.clone(),
                model: None,
                model_provider: None,
                cwd: Some(cwd.to_string_lossy().to_string()),
                approval_policy: Some(approval_policy_for_mode(coder_mode)),
                sandbox: Some(sandbox_mode_for_mode(coder_mode)),
            };

            let resume_response = match client.thread_resume(resume_params).await {
                Ok(response) => response,
                Err(err) => {
                    let _ = emitter.send(ResponseEvent::Error(format!(
                        "Failed to resume codex thread: {}",
                        err
                    )));
                    window.request_redraw();
                    let _ = client.shutdown().await;
                    return;
                }
            };

            let model_name = if resume_response.model.is_empty() {
                "codex".to_string()
            } else {
                resume_response.model.clone()
            };

            let ReviewCommand { delivery, target } = review;
            let target = match target {
                ReviewTarget::UncommittedChanges => app_server::ReviewTarget::UncommittedChanges,
                ReviewTarget::BaseBranch { branch } => {
                    app_server::ReviewTarget::BaseBranch { branch }
                }
                ReviewTarget::Commit { sha, title } => {
                    app_server::ReviewTarget::Commit { sha, title }
                }
                ReviewTarget::Custom { instructions } => {
                    app_server::ReviewTarget::Custom { instructions }
                }
            };
            let delivery = match delivery {
                ReviewDelivery::Inline => None,
                ReviewDelivery::Detached => Some(app_server::ReviewDelivery::Detached),
            };

            let review_params = app_server::ReviewStartParams {
                thread_id: thread_id.clone(),
                target,
                delivery,
            };

            let review_response = match client.review_start(review_params).await {
                Ok(response) => response,
                Err(err) => {
                    let _ = emitter.send(ResponseEvent::Error(format!(
                        "Failed to start codex review: {}",
                        err
                    )));
                    window.request_redraw();
                    let _ = client.shutdown().await;
                    return;
                }
            };

            let active_thread_id = review_response.review_thread_id.clone();
            let run_id = current_timestamp_ms();
            let session_dir = crate::app::config::session_messages_dir(&active_thread_id);
            wire_log.set_path(session_dir.join(format!("wire-{}.jsonl", run_id)));
            trace_log.set_path(session_dir.join(format!("trace-{}.jsonl", run_id)));

            if active_thread_id != thread_id {
                emitter.send(ResponseEvent::SystemInit {
                    model: model_name.clone(),
                    permission_mode: coder_mode.mode_label().to_string(),
                    session_id: active_thread_id.clone(),
                    codex_thread_id: Some(active_thread_id.clone()),
                    tool_count: 0,
                    tools: vec![],
                    output_style: String::new(),
                    slash_commands: vec![],
                    mcp_servers: vec![],
                });
                let _ = emitter.send(ResponseEvent::SystemMessage(format!(
                    "Review started in detached thread {}.",
                    active_thread_id
                )));
            }

            run_app_server_turn_loop(
                client,
                notification_rx,
                request_rx,
                emitter,
                coder_mode,
                permission_tx,
                control_rx,
                window,
                active_thread_id,
                model_name,
                Some(review_response.turn.id),
            )
            .await;
        });
    }

    pub(super) fn poll_responses(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut events = Vec::new();
        if let Some(rx) = &mut state.chat.response_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        } else {
            return;
        }

        let mut needs_redraw = false;

        for event in events {
            match event {
                ResponseEvent::Chunk(text) => {
                    state.chat.streaming_markdown.append(&text);
                    state.chat.streaming_markdown.tick();
                    needs_redraw = true;
                }
                ResponseEvent::ThoughtChunk(text) => {
                    if let Some(last) = state.chat.messages.last_mut() {
                        if last.role == MessageRole::AssistantThought {
                            // Add newline between thought chunks for proper formatting
                            if !last.content.is_empty() && !last.content.ends_with('\n') {
                                last.content.push('\n');
                            }
                            last.content.push_str(&text);
                        } else {
                            state.chat.messages.push(ChatMessage {
                                role: MessageRole::AssistantThought,
                                content: text,
                                document: None,
                                uuid: None,
                                metadata: None,
                            });
                        }
                    } else {
                        state.chat.messages.push(ChatMessage {
                            role: MessageRole::AssistantThought,
                            content: text,
                            document: None,
                            uuid: None,
                            metadata: None,
                        });
                    }
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallStart { name, tool_use_id } => {
                    // During streaming, associate with the NEXT message (the one being streamed)
                    // After completion, associate with the last message
                    let message_index = if state.chat.is_thinking {
                        state.chat.messages.len() // Index of the message being streamed
                    } else {
                        state.chat.messages.len().saturating_sub(1)
                    };
                    tracing::debug!("Tool call start: {} (message_index={})", name, message_index);
                    state
                        .tools
                        .start_tool_call(name, tool_use_id, message_index);
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallInput { json } => {
                    state.tools.current_tool_input.push_str(&json);
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallEnd => {
                    state.tools.finalize_tool_input();
                    needs_redraw = true;
                }
                ResponseEvent::ToolResult {
                    content,
                    is_error,
                    tool_use_id,
                    exit_code,
                    output_value,
                } => {
                    tracing::debug!(
                        "Tool result: tool_use_id={:?}, is_error={}, exit_code={:?}, content_len={}",
                        tool_use_id,
                        is_error,
                        exit_code,
                        content.len()
                    );
                    state.tools.apply_tool_result(
                        tool_use_id,
                        content,
                        is_error,
                        exit_code,
                        output_value,
                    );
                    needs_redraw = true;
                }
                ResponseEvent::ToolProgress {
                    tool_use_id,
                    tool_name,
                    elapsed_secs,
                } => {
                    // During streaming, associate with the NEXT message (the one being streamed)
                    let message_index = if state.chat.is_thinking {
                        state.chat.messages.len()
                    } else {
                        state.chat.messages.len().saturating_sub(1)
                    };
                    tracing::debug!(
                        "Tool progress: {} - {:.1}s (message_index={})",
                        tool_name,
                        elapsed_secs,
                        message_index
                    );
                    state.tools.update_tool_progress(
                        tool_use_id,
                        tool_name,
                        elapsed_secs,
                        message_index,
                    );
                    needs_redraw = true;
                }
                ResponseEvent::UserMessageId { uuid } => {
                    state.attach_user_message_id(uuid);
                    needs_redraw = true;
                }
                ResponseEvent::SystemMessage(message) => {
                    state.push_system_message(message);
                    needs_redraw = true;
                }
                ResponseEvent::Complete { metadata } => {
                    // Complete and move to messages
                    state.chat.streaming_markdown.complete();
                    let source = state.chat.streaming_markdown.source().to_string();
                    if !source.is_empty() {
                        // Aggregate into session usage
                        if let Some(ref meta) = metadata {
                            if let Some(input) = meta.input_tokens {
                                state.session.session_usage.input_tokens += input;
                            }
                            if let Some(output) = meta.output_tokens {
                                state.session.session_usage.output_tokens += output;
                            }
                            if let Some(ms) = meta.duration_ms {
                                state.session.session_usage.duration_ms += ms;
                            }
                            // Cost estimation: placeholder for model pricing
                            let cost = (meta.input_tokens.unwrap_or(0) as f64 * 3.0 / 1_000_000.0)
                                     + (meta.output_tokens.unwrap_or(0) as f64 * 15.0 / 1_000_000.0);
                            state.session.session_usage.total_cost_usd += cost;
                        }
                        state.session.session_usage.num_turns += 1;

                        let doc = state.chat.streaming_markdown.document().clone();
                        state.chat.messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: source,
                            document: Some(doc),
                            uuid: None,
                            metadata,
                        });
                    }
                    state.chat.streaming_markdown.reset();
                    state.session.record_session(
                        &state.settings.coder_settings,
                        &state.chat.messages,
                        state.chat.is_thinking,
                    );
                    state.tools.cancel_running_tools();
                    state.chat.is_thinking = false;
                    state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    state.chat.response_rx = None;
                    state.chat.query_control_tx = None;
                    state.permissions.permission_requests_rx = None;
                    state.permissions.permission_action_tx = None;
                    state.permissions.permission_action_rx = None;
                    state.permissions.permission_dialog = None;
                    state.permissions.permission_pending = None;
                    state.permissions.permission_queue.clear();
                    state.tools.current_tool_name = None;
                    state.tools.current_tool_input.clear();
                    state.tools.current_tool_use_id = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::Error(e) => {
                    state.chat.messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: format!("Error: {}", e),
                        document: None,
                        uuid: None,
                        metadata: None,
                    });
                    state.chat.streaming_markdown.reset();
                    state.session.record_session(
                        &state.settings.coder_settings,
                        &state.chat.messages,
                        state.chat.is_thinking,
                    );
                    state.tools.cancel_running_tools();
                    state.chat.is_thinking = false;
                    state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    state.chat.response_rx = None;
                    state.chat.query_control_tx = None;
                    state.permissions.permission_requests_rx = None;
                    state.permissions.permission_action_tx = None;
                    state.permissions.permission_action_rx = None;
                    state.permissions.permission_dialog = None;
                    state.permissions.permission_pending = None;
                    state.permissions.permission_queue.clear();
                    state.tools.current_tool_name = None;
                    state.tools.current_tool_input.clear();
                    state.tools.current_tool_use_id = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::SystemInit {
                    model,
                    permission_mode,
                    session_id,
                    codex_thread_id,
                    tool_count,
                    tools,
                    output_style,
                    slash_commands,
                    mcp_servers,
                } => {
                    state.session.session_info = SessionInfo {
                        model,
                        permission_mode,
                        session_id,
                        codex_thread_id,
                        tool_count,
                        tools,
                        output_style,
                        slash_commands,
                    };
                    state.catalogs.update_mcp_status(mcp_servers, None);
                    if let Some(parsed_mode) = parse_coder_mode(&state.session.session_info.permission_mode)
                    {
                        state.permissions.coder_mode = parsed_mode;
                        state.permissions.permission_default_allow =
                            coder_mode_default_allow(parsed_mode, state.permissions.permission_default_allow);
                    }
                    state.session.refresh_session_cards(state.chat.is_thinking);
                    needs_redraw = true;
                }
                ResponseEvent::McpStatus { servers, error } => {
                    state.catalogs.update_mcp_status(servers, error);
                    needs_redraw = true;
                }
                ResponseEvent::HookLog(entry) => {
                    state.push_hook_log(entry);
                    needs_redraw = true;
                }
                ResponseEvent::DspyStage(stage) => {
                    let message_index = state.chat.messages.len().saturating_sub(1);
                    state.tools.push_dspy_stage(stage, message_index);
                    needs_redraw = true;
                }
            }
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_permissions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;

        let mut pending_requests = Vec::new();
        if let Some(rx) = &mut state.permissions.permission_requests_rx {
            while let Ok(pending) = rx.try_recv() {
                pending_requests.push(pending);
            }
        }
        for pending in pending_requests {
            state.permissions.enqueue_permission_prompt(pending);
            needs_redraw = true;
        }

        let mut pending_actions = Vec::new();
        if let Some(rx) = &mut state.permissions.permission_action_rx {
            while let Ok(action) = rx.try_recv() {
                pending_actions.push(action);
            }
        }
        for action in pending_actions {
            state.permissions.handle_permission_action(action);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_command_palette_actions(&mut self) {
        if !COMMAND_PALETTE_ENABLED {
            return;
        }
        let actions = {
            let Some(state) = &mut self.state else {
                return;
            };
            let mut actions = Vec::new();
            if let Some(rx) = &mut state.command_palette_action_rx {
                while let Ok(action) = rx.try_recv() {
                    actions.push(action);
                }
            }
            actions
        };

        if actions.is_empty() {
            return;
        }

        for action in actions {
            if let Some(prompt) = self.execute_command_palette_action(&action) {
                self.submit_prompt(prompt);
            }
        }

        if let Some(state) = &mut self.state {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_session_actions(&mut self) {
        let (session_events, session_updates, checkpoint_events) = {
            let Some(state) = &mut self.state else {
                return;
            };

            let mut session_events = Vec::new();
            if let Some(rx) = &mut state.session.session_action_rx {
                while let Ok(event) = rx.try_recv() {
                    session_events.push(event);
                }
            }

            let mut session_updates = Vec::new();
            if let Some(rx) = &mut state.session.session_update_rx {
                while let Ok(update) = rx.try_recv() {
                    session_updates.push(update);
                }
            }

            let mut checkpoint_events = Vec::new();
            if let Some(rx) = &mut state.session.checkpoint_action_rx {
                while let Ok(index) = rx.try_recv() {
                    checkpoint_events.push(index);
                }
            }

            (session_events, session_updates, checkpoint_events)
        };

        let mut needs_redraw = false;

        for event in session_events {
            if matches!(event.action, wgpui::components::molecules::SessionAction::Delete) {
                self.archive_session(event.session_id);
            } else if let Some(state) = &mut self.state {
                state.handle_session_card_action(event.action, event.session_id);
            }
            needs_redraw = true;
        }

        for update in session_updates {
            if let Some(state) = &mut self.state {
                match update {
                    SessionUpdate::MergeEntries(entries) => {
                        state.session.merge_session_entries(
                            entries,
                            &state.settings.coder_settings,
                            state.chat.is_thinking,
                        );
                        needs_redraw = true;
                    }
                    SessionUpdate::Remove { session_id } => {
                        state.session.remove_session_entry(&session_id, state.chat.is_thinking);
                        needs_redraw = true;
                    }
                    SessionUpdate::Error(message) => {
                        state.push_system_message(message);
                        needs_redraw = true;
                    }
                }
            }
        }

        for index in checkpoint_events {
            if let Some(state) = &mut self.state {
                state.handle_checkpoint_restore(index);
                needs_redraw = true;
            }
        }

        if needs_redraw {
            if let Some(state) = &mut self.state {
                state.window.request_redraw();
            }
        }
    }

    fn archive_session(&mut self, session_id: String) {
        let Some(state) = &mut self.state else {
            return;
        };

        if state.chat.is_thinking {
            state.push_system_message("Cannot delete sessions during an active request.".to_string());
            return;
        }

        if session_id == state.session.session_info.session_id {
            state.push_system_message("Cannot delete the active session.".to_string());
            return;
        }

        let codex_thread_id = lookup_codex_thread_id(&state.session, &session_id);
        if let Some(thread_id) = codex_thread_id {
            let update_tx = state.session.session_update_tx.clone();
            let handle = self.runtime_handle.clone();
            let cwd = std::env::current_dir().unwrap_or_default();
            let session_id_for_update = session_id.clone();
            handle.spawn(async move {
                let (client, _channels) = match app_server::AppServerClient::spawn(
                    app_server::AppServerConfig {
                        cwd: Some(cwd),
                        wire_log: None,
                    },
                )
                .await
                {
                    Ok(result) => result,
                    Err(err) => {
                        if let Some(tx) = update_tx {
                            let _ = tx.send(SessionUpdate::Error(format!(
                                "Failed to start codex app-server: {}",
                                err
                            )));
                        }
                        return;
                    }
                };

                let client_info = app_server::ClientInfo {
                    name: "autopilot".to_string(),
                    title: Some("Autopilot".to_string()),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                };
                if let Err(err) = client.initialize(client_info).await {
                    if let Some(tx) = update_tx {
                        let _ = tx.send(SessionUpdate::Error(format!(
                            "Failed to initialize codex app-server: {}",
                            err
                        )));
                    }
                    let _ = client.shutdown().await;
                    return;
                }

                let archive_result = client
                    .thread_archive(app_server::ThreadArchiveParams { thread_id })
                    .await;
                if let Some(tx) = update_tx {
                    match archive_result {
                        Ok(_) => {
                            let _ = tx.send(SessionUpdate::Remove {
                                session_id: session_id_for_update,
                            });
                        }
                        Err(err) => {
                            let _ = tx.send(SessionUpdate::Error(format!(
                                "Failed to archive session: {}",
                                err
                            )));
                        }
                    }
                }
                let _ = client.shutdown().await;
            });
        } else {
            state.session.remove_session_entry(&session_id, state.chat.is_thinking);
        }
    }

    pub(super) fn poll_agent_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut agent_events = Vec::new();
        if let Some(rx) = &mut state.catalogs.agent_action_rx {
            while let Ok(event) = rx.try_recv() {
                agent_events.push(event);
            }
        }
        for event in agent_events {
            state.handle_agent_card_action(event.action, event.agent_id);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_skill_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut skill_events = Vec::new();
        if let Some(rx) = &mut state.catalogs.skill_action_rx {
            while let Ok(event) = rx.try_recv() {
                skill_events.push(event);
            }
        }
        for event in skill_events {
            state.handle_skill_card_action(event.action, event.skill_id);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_hook_inspector_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut views = Vec::new();
        if let Some(rx) = &mut state.catalogs.hook_inspector_action_rx {
            while let Ok(view) = rx.try_recv() {
                views.push(view);
            }
        }
        for view in views {
            state.catalogs.hook_inspector_view = view;
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_oanix_manifest(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;

        // Check if we received a manifest to cache
        if let Some(rx) = &mut state.autopilot.oanix_manifest_rx {
            match rx.try_recv() {
                Ok(manifest) => {
                    tracing::info!("Autopilot: cached OANIX manifest");
                    state.autopilot.oanix_manifest = Some(manifest);
                    state.wallet.refresh(state.autopilot.oanix_manifest.as_ref());
                    if matches!(state.modal_state, ModalState::AutopilotIssues) {
                        state.refresh_issue_tracker();
                    }
                    state.autopilot.oanix_manifest_rx = None; // Done receiving
                    needs_redraw = true;
                }
                Err(TryRecvError::Disconnected) => {
                    tracing::warn!("Autopilot: OANIX manifest channel closed");
                    state.autopilot.oanix_manifest_rx = None;
                    needs_redraw = true;
                }
                Err(TryRecvError::Empty) => {}
            }
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_nip28_events(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut should_redraw = false;
        loop {
            let event = match state.nip28.runtime.event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state.nip28.status = Nip28ConnectionStatus::Error(
                        "NIP-28 runtime disconnected".to_string(),
                    );
                    should_redraw = true;
                    break;
                }
            };

            match event {
                Nip28Event::Connected => {
                    state.nip28.status = Nip28ConnectionStatus::Connected;
                    state.nip28.status_message = Some("Connected to relay".to_string());
                    state.nip28.request_channel_setup();
                    should_redraw = true;
                }
                Nip28Event::ConnectionFailed(error) => {
                    state.nip28.status = Nip28ConnectionStatus::Error(error.clone());
                    state.nip28.status_message = Some(error);
                    should_redraw = true;
                }
                Nip28Event::AuthChallenge(challenge) => {
                    state.nip28.authenticate(&challenge);
                    state.nip28.status_message = Some("Authenticating...".to_string());
                    should_redraw = true;
                }
                Nip28Event::Authenticated => {
                    state.nip28.status = Nip28ConnectionStatus::Authenticated;
                    state.nip28.status_message = Some("Authenticated".to_string());
                    state.nip28.request_channel_setup();
                    should_redraw = true;
                }
                Nip28Event::ChatMessage {
                    id,
                    pubkey,
                    content,
                    created_at,
                } => {
                    state.nip28.push_message(Nip28Message {
                        _id: id,
                        pubkey,
                        content,
                        created_at,
                    });
                    should_redraw = true;
                }
                Nip28Event::Published { _event_id: _ } => {
                    state.nip28.status_message = Some("Message published".to_string());
                    should_redraw = true;
                }
                Nip28Event::PublishFailed { error } => {
                    state.nip28.status_message = Some(format!("Publish failed: {}", error));
                    should_redraw = true;
                }
                Nip28Event::ChannelFound { channel_id, _name } => {
                    state.nip28.mark_channel_ready(channel_id);
                    state.nip28.status_message = Some("Channel ready".to_string());
                    state.nip28.request_channel_setup();
                    should_redraw = true;
                }
            }
        }

        if should_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_nip90_events(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut should_redraw = false;
        loop {
            let event = match state.nip90.runtime.event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state.nip90.status = Nip90ConnectionStatus::Error(
                        "NIP-90 runtime disconnected".to_string(),
                    );
                    should_redraw = true;
                    break;
                }
            };

            match event {
                Nip90Event::Connected => {
                    state.nip90.status = Nip90ConnectionStatus::Connected;
                    state.nip90.status_message = Some("Connected to relay".to_string());
                    state.nip90.request_subscription();
                    should_redraw = true;
                }
                Nip90Event::ConnectionFailed(error) => {
                    state.nip90.status = Nip90ConnectionStatus::Error(error.clone());
                    state.nip90.status_message = Some(error);
                    should_redraw = true;
                }
                Nip90Event::AuthChallenge(challenge) => {
                    state.nip90.status = Nip90ConnectionStatus::Authenticating;
                    state.nip90.runtime.authenticate(&challenge);
                    state.nip90.status_message = Some("Authenticating...".to_string());
                    should_redraw = true;
                }
                Nip90Event::Authenticated => {
                    state.nip90.status = Nip90ConnectionStatus::Authenticated;
                    state.nip90.status_message = Some("Authenticated".to_string());
                    state.nip90.request_subscription();
                    should_redraw = true;
                }
                Nip90Event::JobMessage(message) => {
                    state.nip90.push_message(message);
                    should_redraw = true;
                }
                Nip90Event::Notice(message) => {
                    state.nip90.status_message = Some(message);
                    should_redraw = true;
                }
            }
        }

        if should_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_dvm_events(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut should_redraw = false;
        loop {
            let event = match state.dvm.runtime.event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state.dvm.status = DvmStatus::Error("DVM runtime disconnected".to_string());
                    should_redraw = true;
                    break;
                }
            };

            match event {
                DvmEvent::Providers(providers) => {
                    state.dvm.set_providers(providers);
                    state.dvm.status_message = Some("Providers loaded".to_string());
                    should_redraw = true;
                }
                DvmEvent::Error(message) => {
                    state.dvm.status = DvmStatus::Error(message.clone());
                    state.dvm.status_message = Some(message);
                    should_redraw = true;
                }
            }
        }

        if should_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_gateway_events(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut should_redraw = false;
        loop {
            let event = match state.gateway.runtime.event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state
                        .gateway
                        .set_error("Gateway runtime disconnected".to_string());
                    should_redraw = true;
                    break;
                }
            };

            match event {
                GatewayEvent::Snapshot(snapshot) => {
                    state.gateway.set_snapshot(snapshot);
                    should_redraw = true;
                }
                GatewayEvent::NotConfigured(message) => {
                    state.gateway.set_not_configured(message);
                    should_redraw = true;
                }
                GatewayEvent::Error(message) => {
                    state.gateway.set_error(message);
                    should_redraw = true;
                }
            }
        }

        if should_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_lm_router_events(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut should_redraw = false;
        loop {
            let event = match state.lm_router.runtime.event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state
                        .lm_router
                        .set_error("LM router runtime disconnected".to_string());
                    should_redraw = true;
                    break;
                }
            };

            match event {
                LmRouterEvent::Snapshot(snapshot) => {
                    state.lm_router.set_snapshot(snapshot);
                    should_redraw = true;
                }
                LmRouterEvent::NoBackends(message) => {
                    state.lm_router.set_no_backends(message);
                    should_redraw = true;
                }
                LmRouterEvent::Error(message) => {
                    state.lm_router.set_error(message);
                    should_redraw = true;
                }
            }
        }

        if should_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_nexus_events(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut should_redraw = false;
        loop {
            let event = match state.nexus.runtime.event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state
                        .nexus
                        .set_error("Nexus runtime disconnected".to_string());
                    should_redraw = true;
                    break;
                }
            };

            match event {
                NexusEvent::Snapshot(snapshot) => {
                    state.nexus.set_snapshot(snapshot);
                    should_redraw = true;
                }
                NexusEvent::Error(message) => {
                    state.nexus.set_error(message);
                    should_redraw = true;
                }
            }
        }

        if should_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_spark_wallet_events(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut should_redraw = false;
        loop {
            let event = match state.spark_wallet.runtime.event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state
                        .spark_wallet
                        .set_error("Spark wallet runtime disconnected".to_string());
                    should_redraw = true;
                    break;
                }
            };

            match event {
                SparkWalletEvent::Snapshot(snapshot) => {
                    state.spark_wallet.set_snapshot(snapshot);
                    should_redraw = true;
                }
                SparkWalletEvent::NotConfigured(message) => {
                    state.spark_wallet.set_not_configured(message);
                    should_redraw = true;
                }
                SparkWalletEvent::Error(message) => {
                    state.spark_wallet.set_error(message);
                    should_redraw = true;
                }
            }
        }

        if should_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_agent_backends_events(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut should_redraw = false;
        loop {
            let event = match state.agent_backends.runtime.event_rx.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    state
                        .agent_backends
                        .set_error("Agent backend runtime disconnected".to_string());
                    should_redraw = true;
                    break;
                }
            };

            match event {
                AgentBackendsEvent::Snapshot(snapshot) => {
                    state.agent_backends.set_snapshot(snapshot);
                    should_redraw = true;
                }
            }
        }

        if should_redraw {
            state.window.request_redraw();
        }
    }

    pub(super) fn poll_autopilot_history(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Check if we received updated conversation history from Adjutant
        if let Some(rx) = &mut state.autopilot.autopilot_history_rx {
            if let Ok(updated_history) = rx.try_recv() {
                tracing::info!("Autopilot: updated conversation history ({} turns)", updated_history.len());
                state.autopilot.autopilot_history = updated_history;
                state.autopilot.autopilot_history_rx = None; // Done receiving
            }
        }
    }

    pub(super) fn poll_rate_limits(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Check if we received rate limits
        if let Some(rx) = &mut state.session.rate_limit_rx {
            if let Ok(limits) = rx.try_recv() {
                state.session.rate_limits = limits;
                state.session.rate_limit_rx = None; // Done receiving (one-shot)
                state.window.request_redraw();
            }
        }
    }

    fn execute_command_palette_action(&mut self, command_id: &str) -> Option<String> {
        let Some(state) = &mut self.state else {
            return None;
        };

        let command_action = match command_id {
            command_palette_ids::HELP => {
                state.open_help();
                None
            }
            command_palette_ids::SETTINGS => Some(handle_command(state, Command::Config)),
            command_palette_ids::MODEL_PICKER => Some(handle_command(state, Command::Model)),
            command_palette_ids::SESSION_LIST => Some(handle_command(state, Command::SessionList)),
            command_palette_ids::SESSION_FORK => Some(handle_command(state, Command::SessionFork)),
            command_palette_ids::SESSION_EXPORT => Some(handle_command(state, Command::SessionExport)),
            command_palette_ids::CLEAR_CONVERSATION => Some(handle_command(state, Command::Clear)),
            command_palette_ids::UNDO_LAST => Some(handle_command(state, Command::Undo)),
            command_palette_ids::COMPACT_CONTEXT => Some(handle_command(state, Command::Compact)),
            command_palette_ids::INTERRUPT_REQUEST => {
                state.interrupt_query();
                None
            }
            command_palette_ids::PERMISSION_RULES => Some(handle_command(state, Command::PermissionRules)),
            command_palette_ids::MODE_CYCLE => {
                state
                    .permissions
                    .cycle_coder_mode(&mut state.session.session_info);
                None
            }
            command_palette_ids::MODE_BYPASS => {
                state.permissions.set_coder_mode(
                    CoderMode::BypassPermissions,
                    &mut state.session.session_info,
                );
                None
            }
            command_palette_ids::MODE_PLAN => {
                state
                    .permissions
                    .set_coder_mode(CoderMode::Plan, &mut state.session.session_info);
                None
            }
            command_palette_ids::MODE_AUTOPILOT => {
                state.permissions.set_coder_mode(
                    CoderMode::Autopilot,
                    &mut state.session.session_info,
                );
                None
            }
            command_palette_ids::TOOLS_LIST => Some(handle_command(state, Command::ToolsList)),
            command_palette_ids::MCP_CONFIG => Some(handle_command(state, Command::Mcp)),
            command_palette_ids::MCP_RELOAD => Some(handle_command(state, Command::McpReload)),
            command_palette_ids::MCP_STATUS => Some(handle_command(state, Command::McpStatus)),
            command_palette_ids::AGENTS_LIST => Some(handle_command(state, Command::Agents)),
            command_palette_ids::AGENT_BACKENDS_OPEN => {
                Some(handle_command(state, Command::AgentBackends))
            }
            command_palette_ids::AGENT_CLEAR => Some(handle_command(state, Command::AgentClear)),
            command_palette_ids::AGENT_RELOAD => Some(handle_command(state, Command::AgentReload)),
            command_palette_ids::WALLET_OPEN => {
                state.open_wallet();
                None
            }
            command_palette_ids::DVM_OPEN => {
                state.open_dvm();
                None
            }
            command_palette_ids::GATEWAY_OPEN => {
                state.open_gateway();
                None
            }
            command_palette_ids::LM_ROUTER_OPEN => {
                state.open_lm_router();
                None
            }
            command_palette_ids::NEXUS_OPEN => {
                state.open_nexus();
                None
            }
            command_palette_ids::SPARK_WALLET_OPEN => {
                state.open_spark_wallet();
                None
            }
            command_palette_ids::NIP90_OPEN => {
                state.open_nip90();
                None
            }
            command_palette_ids::OANIX_OPEN => {
                state.open_oanix();
                None
            }
            command_palette_ids::DIRECTIVES_OPEN => {
                state.open_directives();
                None
            }
            command_palette_ids::ISSUES_OPEN => {
                state.open_issues();
                None
            }
            command_palette_ids::ISSUE_TRACKER_OPEN => {
                state.open_issue_tracker();
                None
            }
            command_palette_ids::RLM_OPEN => {
                state.open_rlm();
                None
            }
            command_palette_ids::RLM_TRACE_OPEN => {
                state.open_rlm_trace(None);
                None
            }
            command_palette_ids::PYLON_EARNINGS_OPEN => {
                state.open_pylon_earnings();
                None
            }
            command_palette_ids::PYLON_JOBS_OPEN => {
                state.open_pylon_jobs();
                None
            }
            command_palette_ids::DSPY_OPEN => {
                state.open_dspy();
                None
            }
            command_palette_ids::NIP28_OPEN => {
                state.open_nip28();
                None
            }
            command_palette_ids::SKILLS_LIST => Some(handle_command(state, Command::Skills)),
            command_palette_ids::SKILLS_RELOAD => Some(handle_command(state, Command::SkillsReload)),
            command_palette_ids::HOOKS_OPEN => Some(handle_command(state, Command::Hooks)),
            command_palette_ids::HOOKS_RELOAD => Some(handle_command(state, Command::HooksReload)),
            command_palette_ids::SIDEBAR_LEFT => {
                state.toggle_left_sidebar();
                None
            }
            command_palette_ids::SIDEBAR_RIGHT => {
                state.toggle_right_sidebar();
                None
            }
            command_palette_ids::SIDEBAR_TOGGLE => {
                state.toggle_sidebars();
                None
            }
            command_palette_ids::BUG_REPORT => Some(handle_command(state, Command::Bug)),
            command_palette_ids::KITCHEN_SINK => {
                state.show_kitchen_sink = true;
                None
            }
            _ => None,
        };

        match command_action {
            Some(CommandAction::SubmitPrompt(prompt)) => Some(prompt),
            Some(CommandAction::StartReview(review)) => {
                self.start_review(review);
                None
            }
            _ => None,
        }
    }

    pub(super) fn render(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };
        render_app(state);
    }
}

fn use_app_server_transport() -> bool {
    match std::env::var("AUTOPILOT_CODEX_TRANSPORT") {
        Ok(value) => matches!(
            value.to_ascii_lowercase().as_str(),
            "app-server" | "appserver" | "app_server"
        ),
        Err(_) => false,
    }
}

fn approval_policy_for_mode(mode: CoderMode) -> app_server::AskForApproval {
    match mode {
        CoderMode::BypassPermissions => app_server::AskForApproval::Never,
        CoderMode::Plan => app_server::AskForApproval::Never,
        CoderMode::Autopilot => app_server::AskForApproval::OnRequest,
    }
}

fn sandbox_mode_for_mode(mode: CoderMode) -> app_server::SandboxMode {
    match mode {
        CoderMode::BypassPermissions => app_server::SandboxMode::DangerFullAccess,
        CoderMode::Plan => app_server::SandboxMode::ReadOnly,
        CoderMode::Autopilot => app_server::SandboxMode::WorkspaceWrite,
    }
}

async fn run_app_server_turn_loop(
    client: app_server::AppServerClient,
    mut notification_rx: mpsc::Receiver<app_server::AppServerNotification>,
    mut request_rx: mpsc::Receiver<app_server::AppServerRequest>,
    emitter: ResponseEmitter,
    coder_mode: CoderMode,
    permission_tx: mpsc::UnboundedSender<PermissionPending>,
    mut control_rx: mpsc::UnboundedReceiver<QueryControl>,
    window: Arc<winit::window::Window>,
    thread_id: String,
    model_name: String,
    mut turn_id: Option<String>,
) {
    let mut command_outputs: HashMap<String, String> = HashMap::new();
    let mut file_change_outputs: HashMap<String, String> = HashMap::new();
    let mut agent_message_with_delta: HashSet<String> = HashSet::new();
    let mut reasoning_with_delta: HashSet<String> = HashSet::new();
    let mut diff_tool_ids: HashSet<String> = HashSet::new();
    let mut latest_token_usage: Option<app_server::ThreadTokenUsage> = None;
    let mut approval_items: HashMap<String, Value> = HashMap::new();

    let mut completed = false;
    while !completed {
        tokio::select! {
            Some(control) = control_rx.recv() => {
                match control {
                    QueryControl::Interrupt | QueryControl::Abort => {
                        if let Some(active_turn) = turn_id.clone() {
                            let _ = client.turn_interrupt(app_server::TurnInterruptParams {
                                thread_id: thread_id.clone(),
                                turn_id: active_turn,
                            }).await;
                        } else {
                            let _ = emitter.send(ResponseEvent::Error("Request interrupted.".to_string()));
                        }
                        window.request_redraw();
                    }
                    _ => {}
                }
            }
            Some(notification) = notification_rx.recv() => {
                match notification.method.as_str() {
                    "thread/started" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::ThreadStartedNotification>(params) {
                                tracing::debug!("App-server thread started: {}", event.thread.id);
                            }
                        }
                    }
                    "turn/started" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::TurnStartedNotification>(params) {
                                let id = event.turn.id;
                                turn_id = Some(id.clone());
                                tracing::debug!("App-server turn started: {}", id);
                            }
                        }
                    }
                    "item/agentMessage/delta" => {
                        if let Some(params) = notification.params {
                            match serde_json::from_value::<app_server::AgentMessageDeltaNotification>(params) {
                                Ok(event) => {
                                    agent_message_with_delta.insert(event.item_id);
                                    let _ = emitter.send(ResponseEvent::Chunk(event.delta));
                                    window.request_redraw();
                                }
                                Err(err) => {
                                    tracing::warn!(error = %err, "Failed to parse agent message delta");
                                }
                            }
                        }
                    }
                    "item/reasoning/summaryTextDelta" => {
                        if let Some(params) = notification.params {
                            match serde_json::from_value::<app_server::ReasoningSummaryTextDeltaNotification>(params) {
                                Ok(event) => {
                                    reasoning_with_delta.insert(event.item_id);
                                    let _ = emitter.send(ResponseEvent::ThoughtChunk(event.delta));
                                    window.request_redraw();
                                }
                                Err(err) => {
                                    tracing::warn!(error = %err, "Failed to parse reasoning summary delta");
                                }
                            }
                        }
                    }
                    "item/reasoning/textDelta" => {
                        if let Some(params) = notification.params {
                            match serde_json::from_value::<app_server::ReasoningTextDeltaNotification>(params) {
                                Ok(event) => {
                                    reasoning_with_delta.insert(event.item_id);
                                    let _ = emitter.send(ResponseEvent::ThoughtChunk(event.delta));
                                    window.request_redraw();
                                }
                                Err(err) => {
                                    tracing::warn!(error = %err, "Failed to parse reasoning text delta");
                                }
                            }
                        }
                    }
                    "item/commandExecution/outputDelta" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::CommandExecutionOutputDeltaNotification>(params) {
                                append_tool_output(&mut command_outputs, &event.item_id, &event.delta);
                            }
                        }
                    }
                    "item/fileChange/outputDelta" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::FileChangeOutputDeltaNotification>(params) {
                                append_tool_output(&mut file_change_outputs, &event.item_id, &event.delta);
                            }
                        }
                    }
                    "item/started" => {
                        if let Some(params) = notification.params {
                            match serde_json::from_value::<app_server::ItemStartedNotification>(params) {
                                Ok(event) => {
                                    handle_app_server_item_started(
                                        &event.item,
                                        &emitter,
                                        &mut command_outputs,
                                        &mut file_change_outputs,
                                        &mut approval_items,
                                        &window,
                                    );
                                }
                                Err(err) => {
                                    tracing::warn!(error = %err, "Failed to parse item started notification");
                                }
                            }
                        }
                    }
                    "item/completed" => {
                        if let Some(params) = notification.params {
                            match serde_json::from_value::<app_server::ItemCompletedNotification>(params) {
                                Ok(event) => {
                                    handle_app_server_item_completed(
                                        &event.item,
                                        &emitter,
                                        &mut command_outputs,
                                        &mut file_change_outputs,
                                        &mut agent_message_with_delta,
                                        &mut reasoning_with_delta,
                                        &mut approval_items,
                                        &window,
                                    );
                                }
                                Err(err) => {
                                    tracing::warn!(error = %err, "Failed to parse item completed notification");
                                }
                            }
                        }
                    }
                    "turn/diff/updated" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::TurnDiffUpdatedNotification>(params) {
                                handle_app_server_turn_diff(
                                    &event,
                                    &mut diff_tool_ids,
                                    &emitter,
                                    &window,
                                );
                            }
                        }
                    }
                    "turn/plan/updated" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::TurnPlanUpdatedNotification>(params) {
                                if let Some(stage) = plan_steps_to_dspy_stage(&event.plan) {
                                    let _ = emitter.send(ResponseEvent::DspyStage(stage));
                                }
                                if let Some(explanation) = event.explanation.as_ref() {
                                    let explanation = explanation.trim();
                                    if !explanation.is_empty() {
                                        let _ = emitter.send(ResponseEvent::SystemMessage(format!(
                                            "Plan update: {}",
                                            explanation
                                        )));
                                    }
                                }
                                window.request_redraw();
                            }
                        }
                    }
                    "thread/tokenUsage/updated" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::ThreadTokenUsageUpdatedNotification>(params) {
                                latest_token_usage = Some(event.token_usage.clone());
                            }
                        }
                    }
                    "thread/compacted" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::ContextCompactedNotification>(params) {
                                let message = format!(
                                    "Context compacted for thread {} (turn {}).",
                                    event.thread_id,
                                    event.turn_id
                                );
                                let _ = emitter.send(ResponseEvent::SystemMessage(message));
                                window.request_redraw();
                            }
                        }
                    }
                    "turn/completed" => {
                        if let Some(params) = notification.params {
                            if let Ok(event) = serde_json::from_value::<app_server::TurnCompletedNotification>(params) {
                                tracing::debug!("App-server turn completed: {}", event.turn.id);
                            }
                        }
                        let metadata = latest_token_usage.as_ref().map(|usage| {
                            let input_total =
                                usage.input_tokens.saturating_add(usage.cached_input_tokens);
                            let input_tokens = u64::try_from(input_total).unwrap_or(0);
                            let output_tokens = u64::try_from(usage.output_tokens).unwrap_or(0);
                            crate::app::chat::MessageMetadata {
                                model: Some(model_name.clone()),
                                input_tokens: Some(input_tokens),
                                output_tokens: Some(output_tokens),
                                duration_ms: None,
                                cost_msats: None,
                            }
                        });
                        let _ = emitter.send(ResponseEvent::Complete { metadata });
                        window.request_redraw();
                        completed = true;
                    }
                    "error" => {
                        if let Some(params) = notification.params {
                            match serde_json::from_value::<app_server::ErrorNotification>(params) {
                                Ok(event) => {
                                    if event.will_retry {
                                        let _ = emitter.send(ResponseEvent::SystemMessage(format!(
                                            "Codex error (retrying): {}",
                                            event.error.message
                                        )));
                                    } else {
                                        let _ = emitter.send(ResponseEvent::Error(event.error.message));
                                        completed = true;
                                    }
                                    window.request_redraw();
                                }
                                Err(err) => {
                                    tracing::warn!(error = %err, "Failed to parse app-server error event");
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some(request) = request_rx.recv() => {
                handle_app_server_request(
                    &client,
                    &emitter,
                    coder_mode,
                    &permission_tx,
                    &approval_items,
                    request,
                    &window,
                )
                .await;
            }
            else => {
                completed = true;
            }
        }
    }

    let _ = client.shutdown().await;
}

#[derive(Clone, Copy)]
enum ApprovalKind {
    CommandExecution,
    FileChange,
}

async fn handle_app_server_request(
    client: &app_server::AppServerClient,
    emitter: &ResponseEmitter,
    coder_mode: CoderMode,
    permission_tx: &mpsc::UnboundedSender<PermissionPending>,
    approval_items: &HashMap<String, Value>,
    request: app_server::AppServerRequest,
    window: &Arc<winit::window::Window>,
) {
    let app_server::AppServerRequest { id, method, params } = request;
    let params_value = params.clone();

    let parsed = match method.as_str() {
        "item/commandExecution/requestApproval" => {
            params.and_then(|value| {
                serde_json::from_value::<app_server::CommandExecutionRequestApprovalParams>(value)
                    .ok()
            })
            .map(|parsed| (ApprovalKind::CommandExecution, parsed.item_id, parsed.reason))
        }
        "item/fileChange/requestApproval" => params
            .and_then(|value| {
                serde_json::from_value::<app_server::FileChangeRequestApprovalParams>(value).ok()
            })
            .map(|parsed| (ApprovalKind::FileChange, parsed.item_id, parsed.reason)),
        _ => {
            tracing::warn!("Unhandled app-server request: {}", method);
            None
        }
    };

    let Some((kind, item_id, reason)) = parsed else {
        let _ = emitter.send(ResponseEvent::SystemMessage(
            "Approval request unsupported.".to_string(),
        ));
        let _ = client
            .respond(
                id,
                &app_server::ApprovalResponse {
                    decision: app_server::ApprovalDecision::Decline,
                    accept_settings: None,
                },
            )
            .await;
        window.request_redraw();
        return;
    };

    let approval_item = approval_items.get(&item_id).cloned();
    let permission_request = build_permission_request(
        kind,
        item_id.clone(),
        reason.clone(),
        approval_item.as_ref(),
        params_value.as_ref(),
    );

    let decision_result = if let Some(request) = permission_request {
        let (response_tx, response_rx) = oneshot::channel();
        if permission_tx
            .send(PermissionPending {
                request,
                respond_to: response_tx,
            })
            .is_err()
        {
            None
        } else {
            window.request_redraw();
            response_rx.await.ok()
        }
    } else {
        None
    };

    let (decision, accept_settings, decline_reason) = match decision_result {
        Some(PermissionResult::Allow {
            accept_for_session,
            ..
        }) => {
            let accept_settings = if matches!(kind, ApprovalKind::CommandExecution) {
                accept_for_session.map(|for_session| app_server::ApprovalAcceptSettings {
                    for_session,
                })
            } else {
                None
            };
            (app_server::ApprovalDecision::Accept, accept_settings, None)
        }
        Some(PermissionResult::Deny { message, .. }) => {
            (app_server::ApprovalDecision::Decline, None, Some(message))
        }
        None => {
            let decision = if coder_mode.auto_approves_all() {
                app_server::ApprovalDecision::Accept
            } else {
                app_server::ApprovalDecision::Decline
            };
            let decline_reason = if matches!(decision, app_server::ApprovalDecision::Decline) {
                reason.clone()
            } else {
                None
            };
            (decision, None, decline_reason)
        }
    };

    if matches!(decision, app_server::ApprovalDecision::Decline) {
        let label = decline_reason.unwrap_or_else(|| "No reason provided.".to_string());
        let _ = emitter.send(ResponseEvent::SystemMessage(format!(
            "Approval declined: {}",
            label
        )));
    }

    if let Err(err) = client
        .respond(
            id,
            &app_server::ApprovalResponse {
                decision,
                accept_settings,
            },
        )
        .await
    {
        let _ = emitter.send(ResponseEvent::Error(format!(
            "Failed to respond to approval request: {}",
            err
        )));
    }

    window.request_redraw();
}

fn build_permission_request(
    kind: ApprovalKind,
    item_id: String,
    reason: Option<String>,
    approval_item: Option<&Value>,
    params: Option<&Value>,
) -> Option<PermissionRequest> {
    match kind {
        ApprovalKind::CommandExecution => {
            let command = params
                .and_then(command_string_from_params)
                .or_else(|| approval_item.and_then(command_string_from_item));
            let mut input = serde_json::json!({});
            if let Some(command) = command {
                input["command"] = Value::String(command);
            }
            Some(PermissionRequest {
                tool_name: "Bash".to_string(),
                tool_use_id: item_id,
                input,
                suggestions: None,
                blocked_path: None,
                decision_reason: reason,
            })
        }
        ApprovalKind::FileChange => {
            let (paths, first_path, _) = approval_item
                .map(extract_file_changes)
                .unwrap_or_default();
            let mut input = serde_json::json!({ "files": paths });
            if let Some(path) = first_path.clone() {
                input["file_path"] = Value::String(path);
            }
            Some(PermissionRequest {
                tool_name: "Edit".to_string(),
                tool_use_id: item_id,
                input,
                suggestions: None,
                blocked_path: first_path,
                decision_reason: reason,
            })
        }
    }
}

fn command_string_from_params(params: &Value) -> Option<String> {
    params
        .get("parsedCmd")
        .and_then(value_to_command_string)
        .or_else(|| params.get("command").and_then(value_to_command_string))
}

fn command_string_from_item(item: &Value) -> Option<String> {
    item.get("command").and_then(value_to_command_string)
}

fn value_to_command_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        Value::Array(parts) => {
            let items: Vec<&str> = parts.iter().filter_map(|val| val.as_str()).collect();
            if items.is_empty() {
                None
            } else {
                Some(items.join(" "))
            }
        }
        _ => None,
    }
}

fn append_tool_output(buffers: &mut HashMap<String, String>, item_id: &str, delta: &str) {
    if delta.is_empty() {
        return;
    }
    buffers
        .entry(item_id.to_string())
        .or_default()
        .push_str(delta);
}

fn handle_app_server_turn_diff(
    event: &app_server::TurnDiffUpdatedNotification,
    diff_tool_ids: &mut HashSet<String>,
    emitter: &ResponseEmitter,
    window: &Arc<winit::window::Window>,
) {
    let tool_use_id = format!("turn-diff-{}", event.turn_id);
    if diff_tool_ids.insert(tool_use_id.clone()) {
        let _ = emitter.send(ResponseEvent::ToolCallStart {
            name: "Diff".to_string(),
            tool_use_id: tool_use_id.clone(),
        });
        let _ = emitter.send(ResponseEvent::ToolCallInput {
            json: serde_json::json!({ "file_path": "turn diff" }).to_string(),
        });
        let _ = emitter.send(ResponseEvent::ToolCallEnd);
    }
    let _ = emitter.send(ResponseEvent::ToolResult {
        content: String::new(),
        is_error: false,
        tool_use_id: Some(tool_use_id),
        exit_code: None,
        output_value: Some(serde_json::json!({ "diff": event.diff })),
    });
    window.request_redraw();
}

fn handle_app_server_item_started(
    item: &Value,
    emitter: &ResponseEmitter,
    command_outputs: &mut HashMap<String, String>,
    file_change_outputs: &mut HashMap<String, String>,
    approval_items: &mut HashMap<String, Value>,
    window: &Arc<winit::window::Window>,
) {
    let Some(item_type) = item.get("type").and_then(Value::as_str) else {
        return;
    };
    match item_type {
        "commandExecution" => {
            let Some(id) = item_id(item) else {
                return;
            };
            approval_items.insert(id.clone(), item.clone());
            let command = command_string_from_item(item).unwrap_or_default();
            command_outputs.entry(id.clone()).or_default();
            let _ = emitter.send(ResponseEvent::ToolCallStart {
                name: "Bash".to_string(),
                tool_use_id: id.clone(),
            });
            let _ = emitter.send(ResponseEvent::ToolCallInput {
                json: serde_json::json!({ "command": command }).to_string(),
            });
            let _ = emitter.send(ResponseEvent::ToolCallEnd);
            window.request_redraw();
        }
        "fileChange" => {
            let Some(id) = item_id(item) else {
                return;
            };
            approval_items.insert(id.clone(), item.clone());
            let (paths, first_path, _) = extract_file_changes(item);
            let mut input = serde_json::json!({ "files": paths });
            if let Some(path) = first_path {
                input["file_path"] = Value::String(path);
            }
            file_change_outputs.entry(id.clone()).or_default();
            let _ = emitter.send(ResponseEvent::ToolCallStart {
                name: "Edit".to_string(),
                tool_use_id: id.clone(),
            });
            let _ = emitter.send(ResponseEvent::ToolCallInput {
                json: input.to_string(),
            });
            let _ = emitter.send(ResponseEvent::ToolCallEnd);
            window.request_redraw();
        }
        "mcpToolCall" => {
            let Some(id) = item_id(item) else {
                return;
            };
            let server = item_string(item, "server").unwrap_or_else(|| "server".to_string());
            let tool = item_string(item, "tool").unwrap_or_else(|| "tool".to_string());
            let name = format!("mcp__{}__{}", server, tool);
            let args = item
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Null);
            let _ = emitter.send(ResponseEvent::ToolCallStart {
                name,
                tool_use_id: id.clone(),
            });
            let _ = emitter.send(ResponseEvent::ToolCallInput {
                json: serde_json::to_string(&args).unwrap_or_default(),
            });
            let _ = emitter.send(ResponseEvent::ToolCallEnd);
            window.request_redraw();
        }
        "webSearch" => {
            let query = item_string(item, "query").unwrap_or_else(|| "unknown".to_string());
            let _ = emitter.send(ResponseEvent::SystemMessage(format!(
                "Web search requested: {}",
                query
            )));
            window.request_redraw();
        }
        "enteredReviewMode" => {
            let review = item_string(item, "review").unwrap_or_else(|| "review".to_string());
            let _ = emitter.send(ResponseEvent::SystemMessage(format!(
                "Review started: {}",
                review
            )));
            window.request_redraw();
        }
        "imageView" => {
            let path = item_string(item, "path").unwrap_or_else(|| "unknown".to_string());
            let _ = emitter.send(ResponseEvent::SystemMessage(format!(
                "Image view requested: {}",
                path
            )));
            window.request_redraw();
        }
        _ => {}
    }
}

fn handle_app_server_item_completed(
    item: &Value,
    emitter: &ResponseEmitter,
    command_outputs: &mut HashMap<String, String>,
    file_change_outputs: &mut HashMap<String, String>,
    agent_message_with_delta: &mut HashSet<String>,
    reasoning_with_delta: &mut HashSet<String>,
    approval_items: &mut HashMap<String, Value>,
    window: &Arc<winit::window::Window>,
) {
    let Some(item_type) = item.get("type").and_then(Value::as_str) else {
        return;
    };
    match item_type {
        "agentMessage" => {
            let Some(id) = item_id(item) else {
                return;
            };
            if agent_message_with_delta.remove(&id) {
                return;
            }
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                let _ = emitter.send(ResponseEvent::Chunk(text.to_string()));
                window.request_redraw();
            }
        }
        "reasoning" => {
            let Some(id) = item_id(item) else {
                return;
            };
            if reasoning_with_delta.remove(&id) {
                return;
            }
            let summary = item
                .get("summary")
                .and_then(Value::as_array)
                .map(|parts| join_string_array(parts));
            let content = item
                .get("content")
                .and_then(Value::as_array)
                .map(|parts| join_string_array(parts));
            let text = summary
                .filter(|value| !value.trim().is_empty())
                .or_else(|| content.filter(|value| !value.trim().is_empty()));
            if let Some(text) = text {
                let _ = emitter.send(ResponseEvent::ThoughtChunk(text));
                window.request_redraw();
            }
        }
        "commandExecution" => {
            let Some(id) = item_id(item) else {
                return;
            };
            approval_items.remove(&id);
            let status = item
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            let aggregated = item
                .get("aggregatedOutput")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let buffered = command_outputs.remove(&id).unwrap_or_default();
            let output = if !aggregated.trim().is_empty() {
                aggregated
            } else {
                buffered
            };
            let exit_code = item
                .get("exitCode")
                .and_then(Value::as_i64)
                .map(|code| code as i32);
            let is_error = matches!(status.as_str(), "failed" | "declined");
            let _ = emitter.send(ResponseEvent::ToolResult {
                content: output,
                is_error,
                tool_use_id: Some(id),
                exit_code,
                output_value: None,
            });
            window.request_redraw();
        }
        "fileChange" => {
            let Some(id) = item_id(item) else {
                return;
            };
            approval_items.remove(&id);
            let status = item
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            let (paths, _first_path, diff) = extract_file_changes(item);
            let buffered = file_change_outputs.remove(&id).unwrap_or_default();
            let diff_text = diff
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    if buffered.trim().is_empty() {
                        None
                    } else {
                        Some(buffered)
                    }
                });
            let output_value = diff_text.map(|text| serde_json::json!({ "diff": text }));
            let content = if paths.is_empty() {
                "File change completed.".to_string()
            } else {
                format!("Modified files: {}", paths.join(", "))
            };
            let is_error = matches!(status.as_str(), "failed" | "declined");
            let _ = emitter.send(ResponseEvent::ToolResult {
                content,
                is_error,
                tool_use_id: Some(id),
                exit_code: None,
                output_value,
            });
            window.request_redraw();
        }
        "mcpToolCall" => {
            let Some(id) = item_id(item) else {
                return;
            };
            let mut is_error = false;
            let mut output_value = None;
            let content = if let Some(error) = item.get("error") {
                is_error = true;
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("MCP tool error")
                    .to_string()
            } else if let Some(result) = item.get("result") {
                output_value = Some(result.clone());
                serde_json::to_string_pretty(result).unwrap_or_default()
            } else {
                String::new()
            };
            let _ = emitter.send(ResponseEvent::ToolResult {
                content,
                is_error,
                tool_use_id: Some(id),
                exit_code: None,
                output_value,
            });
            window.request_redraw();
        }
        "webSearch" => {
            let query = item_string(item, "query").unwrap_or_else(|| "unknown".to_string());
            let _ = emitter.send(ResponseEvent::SystemMessage(format!(
                "Web search completed: {}",
                query
            )));
            window.request_redraw();
        }
        "exitedReviewMode" => {
            if let Some(review) = item.get("review").and_then(Value::as_str) {
                let _ = emitter.send(ResponseEvent::Chunk(review.to_string()));
                window.request_redraw();
            }
        }
        "imageView" => {
            let path = item_string(item, "path").unwrap_or_else(|| "unknown".to_string());
            let _ = emitter.send(ResponseEvent::SystemMessage(format!(
                "Image view completed: {}",
                path
            )));
            window.request_redraw();
        }
        _ => {}
    }
}

fn item_id(item: &Value) -> Option<String> {
    item.get("id").and_then(Value::as_str).map(|s| s.to_string())
}

fn item_string(item: &Value, key: &str) -> Option<String> {
    item.get(key).and_then(Value::as_str).map(|s| s.to_string())
}

fn join_string_array(parts: &[Value]) -> String {
    parts
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_file_changes(item: &Value) -> (Vec<String>, Option<String>, Option<String>) {
    let mut paths = Vec::new();
    let mut first_path = None;
    let mut first_diff = None;
    if let Some(changes) = item.get("changes").and_then(Value::as_array) {
        for change in changes {
            if let Some(path) = change.get("path").and_then(Value::as_str) {
                if first_path.is_none() {
                    first_path = Some(path.to_string());
                }
                paths.push(path.to_string());
            }
            if first_diff.is_none() {
                if let Some(diff) = change.get("diff").and_then(Value::as_str) {
                    first_diff = Some(diff.to_string());
                }
            }
        }
    }
    (paths, first_path, first_diff)
}

fn plan_steps_to_dspy_stage(steps: &[app_server::TurnPlanStep]) -> Option<DspyStage> {
    if steps.is_empty() {
        return None;
    }
    let tasks = steps
        .iter()
        .enumerate()
        .map(|(idx, step)| TodoTask {
            index: idx + 1,
            description: step.step.clone(),
            status: plan_status_to_todo_status(&step.status),
        })
        .collect();
    Some(DspyStage::TodoList { tasks })
}

fn plan_status_to_todo_status(status: &str) -> TodoStatus {
    match status.trim().to_ascii_lowercase().as_str() {
        "pending" => TodoStatus::Pending,
        "inprogress" | "in_progress" | "in-progress" => TodoStatus::InProgress,
        "completed" | "complete" => TodoStatus::Complete,
        "failed" | "error" => TodoStatus::Failed,
        _ => TodoStatus::Pending,
    }
}

#[derive(Clone)]
struct ResponseEmitter {
    tx: mpsc::UnboundedSender<ResponseEvent>,
    trace: Option<TraceLogger>,
}

impl ResponseEmitter {
    fn new(tx: mpsc::UnboundedSender<ResponseEvent>, trace: Option<TraceLogger>) -> Self {
        Self { tx, trace }
    }

    fn send(&self, event: ResponseEvent) {
        if let Some(trace) = &self.trace {
            trace.log_response_event(&event);
        }
        let _ = self.tx.send(event);
    }
}

#[derive(Clone)]
struct TraceLogger {
    tx: mpsc::UnboundedSender<TraceLogCommand>,
}

enum TraceLogCommand {
    SetPath(PathBuf),
    Entry(Value),
}

impl TraceLogger {
    fn new() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut file: Option<tokio::fs::File> = None;
            let mut buffer: Vec<String> = Vec::new();

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    TraceLogCommand::SetPath(path) => {
                        if let Some(parent) = path.parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }
                        match tokio::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&path)
                            .await
                        {
                            Ok(mut opened) => {
                                for line in buffer.drain(..) {
                                    let _ = opened.write_all(line.as_bytes()).await;
                                    let _ = opened.write_all(b"\n").await;
                                }
                                file = Some(opened);
                            }
                            Err(err) => {
                                tracing::warn!(
                                    error = %err,
                                    path = %path.display(),
                                    "Failed to open app-server trace log"
                                );
                            }
                        }
                    }
                    TraceLogCommand::Entry(value) => {
                        let line =
                            serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string());
                        if let Some(file) = file.as_mut() {
                            let _ = file.write_all(line.as_bytes()).await;
                            let _ = file.write_all(b"\n").await;
                        } else {
                            buffer.push(line);
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    fn set_path(&self, path: PathBuf) {
        let _ = self.tx.send(TraceLogCommand::SetPath(path));
    }

    fn log_event(&self, kind: &str, data: Value) {
        let entry = serde_json::json!({
            "timestamp_ms": current_timestamp_ms(),
            "kind": kind,
            "data": data,
        });
        let _ = self.tx.send(TraceLogCommand::Entry(entry));
    }

    fn log_response_event(&self, event: &ResponseEvent) {
        match event {
            ResponseEvent::Chunk(text) => {
                self.log_event("chunk", serde_json::json!({ "text": text }));
            }
            ResponseEvent::ThoughtChunk(text) => {
                self.log_event("thought_chunk", serde_json::json!({ "text": text }));
            }
            ResponseEvent::ToolCallStart { name, tool_use_id } => {
                self.log_event(
                    "tool_call_start",
                    serde_json::json!({ "name": name, "tool_use_id": tool_use_id }),
                );
            }
            ResponseEvent::ToolCallInput { json } => {
                self.log_event("tool_call_input", serde_json::json!({ "json": json }));
            }
            ResponseEvent::ToolCallEnd => {
                self.log_event("tool_call_end", serde_json::json!({}));
            }
            ResponseEvent::ToolResult {
                content,
                is_error,
                tool_use_id,
                exit_code,
                output_value,
            } => {
                self.log_event(
                    "tool_result",
                    serde_json::json!({
                        "content": content,
                        "is_error": is_error,
                        "tool_use_id": tool_use_id,
                        "exit_code": exit_code,
                        "output_value": output_value,
                    }),
                );
            }
            ResponseEvent::ToolProgress {
                tool_use_id,
                tool_name,
                elapsed_secs,
            } => {
                self.log_event(
                    "tool_progress",
                    serde_json::json!({
                        "tool_use_id": tool_use_id,
                        "tool_name": tool_name,
                        "elapsed_secs": elapsed_secs,
                    }),
                );
            }
            ResponseEvent::UserMessageId { uuid } => {
                self.log_event("user_message_id", serde_json::json!({ "uuid": uuid }));
            }
            ResponseEvent::SystemMessage(message) => {
                self.log_event("system_message", serde_json::json!({ "message": message }));
            }
            ResponseEvent::Complete { metadata } => {
                let meta = metadata.as_ref().map(|meta| {
                    serde_json::json!({
                        "model": meta.model.clone(),
                        "input_tokens": meta.input_tokens,
                        "output_tokens": meta.output_tokens,
                        "duration_ms": meta.duration_ms,
                        "cost_msats": meta.cost_msats,
                    })
                });
                self.log_event("complete", serde_json::json!({ "metadata": meta }));
            }
            ResponseEvent::Error(message) => {
                self.log_event("error", serde_json::json!({ "message": message }));
            }
            ResponseEvent::SystemInit {
                model,
                permission_mode,
                session_id,
                codex_thread_id,
                tool_count,
                tools,
                output_style,
                slash_commands,
                mcp_servers,
            } => {
                let server_values = mcp_servers
                    .iter()
                    .map(|server| {
                        serde_json::json!({
                            "name": &server.name,
                            "status": &server.status,
                        })
                    })
                    .collect::<Vec<_>>();
                self.log_event(
                    "system_init",
                    serde_json::json!({
                        "model": model,
                        "permission_mode": permission_mode,
                        "session_id": session_id,
                        "codex_thread_id": codex_thread_id,
                        "tool_count": tool_count,
                        "tools": tools,
                        "output_style": output_style,
                        "slash_commands": slash_commands,
                        "mcp_servers": server_values,
                    }),
                );
            }
            ResponseEvent::McpStatus { servers, error } => {
                let server_values = servers
                    .iter()
                    .map(|server| {
                        serde_json::json!({
                            "name": &server.name,
                            "status": &server.status,
                        })
                    })
                    .collect::<Vec<_>>();
                self.log_event(
                    "mcp_status",
                    serde_json::json!({ "servers": server_values, "error": error }),
                );
            }
            ResponseEvent::HookLog(entry) => {
                self.log_event(
                    "hook_log",
                    serde_json::json!({
                        "id": &entry.id,
                        "event": entry.event,
                        "summary": &entry.summary,
                        "tool_name": entry.tool_name.clone(),
                        "error": entry.error.clone(),
                    }),
                );
            }
            ResponseEvent::DspyStage(stage) => {
                self.log_event(
                    "dspy_stage",
                    serde_json::json!({ "stage": format!("{:?}", stage) }),
                );
            }
        }
    }
}

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn resolve_codex_thread_id(
    session: &crate::app::session::SessionState,
    session_id: &str,
) -> Option<String> {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(entry) = session
        .session_index
        .iter()
        .find(|entry| entry.id == trimmed)
    {
        if let Some(thread_id) = entry.codex_thread_id.clone() {
            return Some(thread_id);
        }
        if entry.id.starts_with("thr_") {
            return Some(entry.id.clone());
        }
        return None;
    }
    if trimmed.starts_with("thr_") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn lookup_codex_thread_id(
    session: &crate::app::session::SessionState,
    session_id: &str,
) -> Option<String> {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(entry) = session
        .session_index
        .iter()
        .find(|entry| entry.id == trimmed)
    {
        if let Some(thread_id) = entry.codex_thread_id.clone() {
            return Some(thread_id);
        }
        if entry.id.starts_with("thr_") {
            return Some(entry.id.clone());
        }
        return None;
    }
    if trimmed.starts_with("thr_") {
        Some(trimmed.to_string())
    } else {
        None
    }
}
