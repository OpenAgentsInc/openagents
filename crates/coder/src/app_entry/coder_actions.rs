use std::sync::Arc;

use futures::StreamExt;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::mpsc::error::TryRecvError;

use claude_agent_sdk::permissions::{CallbackPermissionHandler, PermissionRequest};
use claude_agent_sdk::protocol::{PermissionMode, PermissionResult};
use claude_agent_sdk::{query_with_permissions, QueryOptions, SdkMessage};

use crate::app::catalog::build_hook_map;
use crate::app::chat::{ChatMessage, MessageRole};
use crate::app::dvm::{DvmEvent, DvmStatus};
use crate::app::gateway::GatewayEvent;
use crate::app::lm_router::LmRouterEvent;
use crate::app::nexus::NexusEvent;
use crate::app::spark_wallet::SparkWalletEvent;
use crate::app::events::{CommandAction, ModalState, QueryControl, ResponseEvent};
use crate::app::nip28::{Nip28ConnectionStatus, Nip28Event, Nip28Message};
use crate::app::nip90::{Nip90ConnectionStatus, Nip90Event};
use crate::app::parsing::expand_prompt_text;
use crate::app::permissions::{
    coder_mode_default_allow, extract_bash_command, is_read_only_tool, parse_coder_mode,
    pattern_matches, PermissionPending,
};
use crate::app::session::SessionInfo;
use crate::app::tools::tool_result_output;
use crate::app::ui::render_app;
use crate::app::CoderMode;
use crate::commands::Command;

use super::CoderApp;
use super::command_palette_ids;
use super::commands::handle_command;
use super::settings::parse_mcp_status;

impl CoderApp {
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

        // Check selected backend
        use crate::app::config::AgentKindConfig;
        if matches!(state.agent_selection.agent, AgentKindConfig::Codex) {
            tracing::info!("Using Codex backend");
            self.submit_codex_prompt(prompt);
            return;
        }
        tracing::info!("Using Claude backend");

        let cwd = std::env::current_dir().unwrap_or_default();
        let active_agent = state.catalogs.active_agent.clone();
        let expanded_prompt = match expand_prompt_text(&prompt, &cwd) {
            Ok(result) => result,
            Err(err) => {
                state.push_system_message(err);
                state.window.request_redraw();
                return;
            }
        };
        let expanded_prompt = if let Some(agent) = active_agent.as_ref() {
            format!("Use the {} subagent for this request.\n\n{}", agent, expanded_prompt)
        } else {
            expanded_prompt
        };

        // Create channel for receiving responses
        let (tx, rx) = mpsc::unbounded_channel();
        let (control_tx, mut control_rx) = mpsc::unbounded_channel();
        let (permission_tx, permission_rx) = mpsc::unbounded_channel();
        let (permission_action_tx, permission_action_rx) = mpsc::unbounded_channel();
        state.chat.response_rx = Some(rx);
        state.chat.query_control_tx = Some(control_tx);
        state.permissions.permission_requests_rx = Some(permission_rx);
        state.permissions.permission_action_tx = Some(permission_action_tx.clone());
        state.permissions.permission_action_rx = Some(permission_action_rx);
        state.permissions.permission_queue.clear();
        state.permissions.permission_pending = None;
        state.permissions.permission_dialog = None;
        state.chat.is_thinking = true;
        state.chat.streaming_markdown.reset();
        state.catalogs.refresh_agent_cards(state.chat.is_thinking);

        // Get window handle for triggering redraws from async task
        let window = state.window.clone();
        let model_id = state.settings.selected_model.model_id().to_string();
        let resume_session = state.session.pending_resume_session
            .take()
            .or_else(|| {
                if state.session.session_info.session_id.trim().is_empty() {
                    None
                } else {
                    Some(state.session.session_info.session_id.clone())
                }
            });
        let fork_session = state.session.pending_fork_session;
        state.session.pending_fork_session = false;
        let permission_mode = Some(state.permissions.coder_mode.to_sdk_permission_mode());
        let output_style = state.permissions.output_style.clone();
        let allowed_tools = state.permissions.tools_allowed.clone();
        let disallowed_tools = state.permissions.tools_disallowed.clone();
        let permission_allow_tools = state.permissions.permission_allow_tools.clone();
        let permission_deny_tools = state.permissions.permission_deny_tools.clone();
        let permission_allow_bash_patterns = state.permissions.permission_allow_bash_patterns.clone();
        let permission_deny_bash_patterns = state.permissions.permission_deny_bash_patterns.clone();
        let permission_default_allow = state.permissions.permission_default_allow;
        let mcp_servers = state.catalogs.merged_mcp_servers();
        let agent_definitions = state.agent_definitions_for_query();
        let setting_sources = state.setting_sources_for_query();
        let hook_config = state.catalogs.hook_config.clone();
        let hook_scripts = state.catalogs.hook_scripts.clone();
        let max_thinking_tokens = state.settings.coder_settings.max_thinking_tokens;
        let persist_session = state.settings.coder_settings.session_auto_save;

        // Spawn async query task
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            let hook_cwd = cwd.clone();
            let mut options = QueryOptions::new()
                .cwd(cwd)
                .include_partial_messages(true) // Enable streaming deltas
                .model(&model_id);

            options.max_thinking_tokens = max_thinking_tokens;
            options.persist_session = persist_session;

            if let Some(mode) = permission_mode.clone() {
                options = options.permission_mode(mode);
            }
            if let Some(resume_id) = resume_session {
                options = options.resume(resume_id);
            }
            if fork_session {
                options = options.fork_session(true);
            }
            if !allowed_tools.is_empty() {
                options.allowed_tools = Some(allowed_tools);
            }
            if !disallowed_tools.is_empty() {
                options.disallowed_tools = Some(disallowed_tools);
            }
            if let Some(style) = output_style {
                options
                    .extra_args
                    .insert("output-style".to_string(), Some(style));
            }
            if !mcp_servers.is_empty() {
                options.mcp_servers = mcp_servers;
            }
            if !agent_definitions.is_empty() {
                options.agents = agent_definitions;
            }
            if !setting_sources.is_empty() {
                options.setting_sources = setting_sources;
            }
            if let Some(hooks) = build_hook_map(hook_cwd, hook_config, hook_scripts, tx.clone()) {
                options = options.hooks(hooks);
            }

            let permission_window = window.clone();
            let permissions = Arc::new(CallbackPermissionHandler::new(move |request: PermissionRequest| {
                let permission_tx = permission_tx.clone();
                let permission_window = permission_window.clone();
                let permission_mode = permission_mode.clone();
                let permission_allow_tools = permission_allow_tools.clone();
                let permission_deny_tools = permission_deny_tools.clone();
                let permission_allow_bash_patterns = permission_allow_bash_patterns.clone();
                let permission_deny_bash_patterns = permission_deny_bash_patterns.clone();
                async move {
                    let tool_name = request.tool_name.clone();

                    if tool_name == "Bash" {
                        if let Some(command) = extract_bash_command(&request.input) {
                            if permission_deny_bash_patterns
                                .iter()
                                .any(|pattern| pattern_matches(pattern, &command))
                            {
                                return Ok(PermissionResult::Deny {
                                    message: format!("Bash command denied by rule: {}", command),
                                    interrupt: None,
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            if permission_allow_bash_patterns
                                .iter()
                                .any(|pattern| pattern_matches(pattern, &command))
                            {
                                return Ok(PermissionResult::Allow {
                                    updated_input: request.input.clone(),
                                    updated_permissions: request.suggestions.clone(),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                        }
                    }

                    if permission_deny_tools.iter().any(|tool| tool == &tool_name) {
                        return Ok(PermissionResult::Deny {
                            message: format!("Tool {} is denied by rule.", tool_name),
                            interrupt: None,
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }
                    if permission_allow_tools.iter().any(|tool| tool == &tool_name) {
                        return Ok(PermissionResult::Allow {
                            updated_input: request.input.clone(),
                            updated_permissions: request.suggestions.clone(),
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }

                    if let Some(mode) = permission_mode.as_ref() {
                        match mode {
                            PermissionMode::BypassPermissions | PermissionMode::AcceptEdits => {
                                return Ok(PermissionResult::Allow {
                                    updated_input: request.input.clone(),
                                    updated_permissions: request.suggestions.clone(),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::DontAsk => {
                                return Ok(PermissionResult::Deny {
                                    message: format!("Permission denied for tool {}.", tool_name),
                                    interrupt: Some(true),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::Plan => {
                                if is_read_only_tool(&tool_name) {
                                    return Ok(PermissionResult::Allow {
                                        updated_input: request.input.clone(),
                                        updated_permissions: None,
                                        tool_use_id: Some(request.tool_use_id.clone()),
                                    });
                                }
                                return Ok(PermissionResult::Deny {
                                    message: format!("Plan mode denies tool {}.", tool_name),
                                    interrupt: Some(true),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::Default => {}
                        }
                    }

                    if permission_default_allow {
                        return Ok(PermissionResult::Allow {
                            updated_input: request.input.clone(),
                            updated_permissions: request.suggestions.clone(),
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }

                    let (respond_to, response_rx) = oneshot::channel();
                    let pending = PermissionPending { request, respond_to };
                    if permission_tx.send(pending).is_err() {
                        return Ok(PermissionResult::deny_and_interrupt(
                            "Permission prompt unavailable.",
                        ));
                    }
                    permission_window.request_redraw();
                    match response_rx.await {
                        Ok(result) => Ok(result),
                        Err(_) => Ok(PermissionResult::deny_and_interrupt(
                            "Permission prompt interrupted.",
                        )),
                    }
                }
            }));

            tracing::info!("Starting query...");

            match query_with_permissions(&expanded_prompt, options, permissions).await {
                Ok(mut stream) => {
                    tracing::info!("Query stream started");
                    let mut interrupt_requested = false;

                    loop {
                        if interrupt_requested {
                            if let Err(e) = stream.interrupt().await {
                                tracing::error!("Interrupt failed: {}", e);
                                let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                window.request_redraw();
                                break;
                            }
                            interrupt_requested = false;
                        }

                        tokio::select! {
                            Some(control) = control_rx.recv() => {
                                match control {
                                    QueryControl::Interrupt => {
                                        interrupt_requested = true;
                                    }
                                    QueryControl::RewindFiles { user_message_id } => {
                                        match stream.rewind_files(&user_message_id).await {
                                            Ok(()) => {
                                                let _ = tx.send(ResponseEvent::SystemMessage(
                                                    "Checkpoint restore requested.".to_string(),
                                                ));
                                            }
                                            Err(err) => {
                                                let _ = tx.send(ResponseEvent::SystemMessage(
                                                    format!("Checkpoint restore failed: {}", err),
                                                ));
                                            }
                                        }
                                        window.request_redraw();
                                    }
                                    QueryControl::Abort => {
                                        if let Err(e) = stream.abort().await {
                                            tracing::error!("Abort failed: {}", e);
                                            let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                        } else {
                                            let _ = tx.send(ResponseEvent::Error("Request aborted.".to_string()));
                                        }
                                        window.request_redraw();
                                        break;
                                    }
                                    QueryControl::FetchMcpStatus => {
                                        match stream.mcp_server_status().await {
                                            Ok(value) => {
                                                match parse_mcp_status(&value) {
                                                    Ok(servers) => {
                                                        let _ = tx.send(ResponseEvent::McpStatus {
                                                            servers,
                                                            error: None,
                                                        });
                                                    }
                                                    Err(err) => {
                                                        let _ = tx.send(ResponseEvent::McpStatus {
                                                            servers: Vec::new(),
                                                            error: Some(err),
                                                        });
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                let _ = tx.send(ResponseEvent::McpStatus {
                                                    servers: Vec::new(),
                                                    error: Some(err.to_string()),
                                                });
                                            }
                                        }
                                        window.request_redraw();
                                    }
                                }
                            }
                            msg = stream.next() => {
                                match msg {
                                    Some(Ok(SdkMessage::Assistant(m))) => {
                                        // Don't extract text here - we get it from STREAM_EVENT deltas
                                        // The ASSISTANT message contains the full text which would duplicate
                                        tracing::trace!("ASSISTANT: (skipping text extraction, using stream events)");
                                        tracing::trace!("  full message: {:?}", m.message);
                                    }
                                    Some(Ok(SdkMessage::StreamEvent(e))) => {
                                        tracing::trace!("STREAM_EVENT: {:?}", e.event);
                                        // Check for tool call start
                                        if let Some((tool_name, tool_id)) = extract_tool_call_start(&e.event) {
                                            tracing::debug!("  -> tool call start: {}", tool_name);
                                            let _ = tx.send(ResponseEvent::ToolCallStart {
                                                name: tool_name,
                                                tool_use_id: tool_id,
                                            });
                                            window.request_redraw();
                                        }
                                        // Check for tool input delta
                                        else if let Some(json) = extract_tool_input_delta(&e.event) {
                                            let _ = tx.send(ResponseEvent::ToolCallInput { json });
                                            window.request_redraw();
                                        }
                                        // Check for content_block_stop (tool call end)
                                        else if e.event.get("type").and_then(|t| t.as_str()) == Some("content_block_stop") {
                                            let _ = tx.send(ResponseEvent::ToolCallEnd);
                                            window.request_redraw();
                                        }
                                        // Extract streaming text delta
                                        else if let Some(text) = extract_stream_text(&e.event) {
                                            tracing::trace!("  -> stream text: {}", text);
                                            if tx.send(ResponseEvent::Chunk(text)).is_err() {
                                                break;
                                            }
                                            window.request_redraw();
                                        }
                                    }
                                    Some(Ok(SdkMessage::System(s))) => {
                                        tracing::debug!("SYSTEM: {:?}", s);
                                        // Extract init info
                                        if let claude_agent_sdk::SdkSystemMessage::Init(init) = s {
                                    let _ = tx.send(ResponseEvent::SystemInit {
                                        model: init.model.clone(),
                                        permission_mode: init.permission_mode.clone(),
                                        session_id: init.session_id.clone(),
                                        tool_count: init.tools.len(),
                                        tools: init.tools.clone(),
                                        output_style: init.output_style.clone(),
                                        slash_commands: init.slash_commands.clone(),
                                        mcp_servers: init.mcp_servers.clone(),
                                    });
                                    window.request_redraw();
                                }
                                    }
                                    Some(Ok(SdkMessage::User(u))) => {
                                        tracing::trace!("USER message received (tool result)");
                                        if let Some(uuid) = u.uuid.clone() {
                                            let _ = tx.send(ResponseEvent::UserMessageId { uuid });
                                            window.request_redraw();
                                        }
                                        // Extract tool results from USER messages
                                        if let Some(content) = u.message.get("content").and_then(|c| c.as_array()) {
                                            for item in content {
                                                if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                                    let tool_use_id = item
                                                        .get("tool_use_id")
                                                        .or_else(|| item.get("toolUseId"))
                                                        .or_else(|| item.get("toolUseID"))
                                                        .and_then(|v| v.as_str())
                                                        .map(|v| v.to_string())
                                                        .or_else(|| u.parent_tool_use_id.clone());
                                                    let is_error = item
                                                        .get("is_error")
                                                        .or_else(|| item.get("isError"))
                                                        .and_then(|e| e.as_bool())
                                                        .unwrap_or(false);
                                                    let content_value = item.get("content").cloned().unwrap_or(Value::Null);
                                                    let (result_content, exit_code, output_value) =
                                                        tool_result_output(&content_value, u.tool_use_result.as_ref());
                                                    let _ = tx.send(ResponseEvent::ToolResult {
                                                        content: result_content,
                                                        is_error,
                                                        tool_use_id,
                                                        exit_code,
                                                        output_value,
                                                    });
                                                    window.request_redraw();
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(SdkMessage::ToolProgress(tp))) => {
                                        tracing::trace!(
                                            "TOOL_PROGRESS: {} - {:.1}s",
                                            tp.tool_name,
                                            tp.elapsed_time_seconds
                                        );
                                        let _ = tx.send(ResponseEvent::ToolProgress {
                                            tool_use_id: tp.tool_use_id.clone(),
                                            tool_name: tp.tool_name.clone(),
                                            elapsed_secs: tp.elapsed_time_seconds,
                                        });
                                        window.request_redraw();
                                    }
                                    Some(Ok(SdkMessage::AuthStatus(a))) => {
                                        tracing::debug!("AUTH_STATUS: {:?}", a);
                                    }
                                    Some(Ok(SdkMessage::Result(_r))) => {
                                        tracing::debug!("RESULT received");
                                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                                        window.request_redraw();
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        tracing::error!("ERROR: {}", e);
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
                    tracing::info!("Query stream ended");
                }
                Err(e) => {
                    tracing::error!("Query failed to start: {}", e);
                    let _ = tx.send(ResponseEvent::Error(e.to_string()));
                    window.request_redraw();
                }
            }
        });
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
        // Don't pass Claude model to Codex - Codex uses its own OAuth and default model
        let resume_session = state.session.pending_resume_session
            .take()
            .or_else(|| {
                if state.session.session_info.session_id.trim().is_empty() {
                    None
                } else {
                    Some(state.session.session_info.session_id.clone())
                }
            });
        let permission_mode = Some(state.permissions.coder_mode.to_sdk_permission_mode());

        // Spawn async Codex query task
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions, ThreadEvent, ThreadItemDetails, SandboxMode, ApprovalMode};

            let codex = Codex::new();

            // Build thread options - let Codex use its default model
            let mut thread_options = ThreadOptions::new()
                .working_directory(&cwd)
                .skip_git_repo_check(true);

            // Map permission mode to sandbox mode
            if let Some(ref mode) = permission_mode {
                use claude_agent_sdk::protocol::PermissionMode;
                let sandbox = match mode {
                    PermissionMode::BypassPermissions => SandboxMode::DangerFullAccess,
                    PermissionMode::Plan => SandboxMode::ReadOnly,
                    _ => SandboxMode::WorkspaceWrite,
                };
                thread_options = thread_options.sandbox_mode(sandbox);

                let approval = match mode {
                    PermissionMode::BypassPermissions => ApprovalMode::Never,
                    PermissionMode::Plan => ApprovalMode::Never,
                    _ => ApprovalMode::OnRequest,
                };
                thread_options = thread_options.approval_policy(approval);
            }

            // Create or resume thread
            let mut thread = if let Some(ref resume_id) = resume_session {
                codex.resume_thread(resume_id, thread_options)
            } else {
                codex.start_thread(thread_options)
            };

            // Start streaming turn
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
                                                let mode_str = permission_mode.as_ref()
                                                    .map(|m| match m {
                                                        claude_agent_sdk::protocol::PermissionMode::Default => "default",
                                                        claude_agent_sdk::protocol::PermissionMode::AcceptEdits => "acceptEdits",
                                                        claude_agent_sdk::protocol::PermissionMode::BypassPermissions => "bypassPermissions",
                                                        claude_agent_sdk::protocol::PermissionMode::Plan => "plan",
                                                        claude_agent_sdk::protocol::PermissionMode::DontAsk => "dontAsk",
                                                    }.to_string())
                                                    .unwrap_or_default();
                                                let _ = tx.send(ResponseEvent::SystemInit {
                                                    model: "codex".to_string(),
                                                    permission_mode: mode_str,
                                                    session_id: started.thread_id,
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
                            // Cost estimation: ~$3/M input, ~$15/M output for Claude Opus
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
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;

        let mut session_events = Vec::new();
        if let Some(rx) = &mut state.session.session_action_rx {
            while let Ok(event) = rx.try_recv() {
                session_events.push(event);
            }
        }
        for event in session_events {
            state.handle_session_card_action(event.action, event.session_id);
            needs_redraw = true;
        }

        let mut checkpoint_events = Vec::new();
        if let Some(rx) = &mut state.session.checkpoint_action_rx {
            while let Ok(index) = rx.try_recv() {
                checkpoint_events.push(index);
            }
        }
        for index in checkpoint_events {
            state.handle_checkpoint_restore(index);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
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

fn extract_tool_call_start(event: &Value) -> Option<(String, String)> {
    if event.get("type").and_then(|t| t.as_str()) != Some("content_block_start") {
        return None;
    }
    let block = event.get("content_block")?;
    if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
        return None;
    }
    let tool_name = block.get("name").and_then(|v| v.as_str())?.to_string();
    let tool_id = block.get("id").and_then(|v| v.as_str())?.to_string();
    Some((tool_name, tool_id))
}

fn extract_tool_input_delta(event: &Value) -> Option<String> {
    if event.get("type").and_then(|t| t.as_str()) != Some("content_block_delta") {
        return None;
    }
    let delta = event.get("delta")?;
    if delta.get("type").and_then(|t| t.as_str()) != Some("input_json_delta") {
        return None;
    }
    delta
        .get("partial_json")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_stream_text(event: &Value) -> Option<String> {
    if event.get("type").and_then(|t| t.as_str()) != Some("content_block_delta") {
        return None;
    }
    let delta = event.get("delta")?;
    if delta.get("type").and_then(|t| t.as_str()) != Some("text_delta") {
        return None;
    }
    delta
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}
