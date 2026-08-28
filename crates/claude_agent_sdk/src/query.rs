//! Query struct for executing prompts and streaming responses.

use crate::error::{Error, Result};
use crate::options::QueryOptions;
use crate::permissions::PermissionHandler;
use crate::protocol::{
    ControlRequestData, ControlRequestType, ControlResponseData, ControlResponseType,
    HookCallbackStub, InitializeRequest, PermissionMode, PermissionResult, SdkControlRequest,
    SdkControlResponse, SdkMessage, SdkUserMessage, SetMaxThinkingTokensRequest, SetModelRequest,
    SetPermissionModeRequest, StdinMessage, StdoutMessage, UserMessageType,
};
use crate::transport::ProcessTransport;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, trace, warn};

/// A query execution that streams messages from Claude.
pub struct Query {
    /// The process transport.
    transport: Arc<Mutex<ProcessTransport>>,
    /// Pending control requests waiting for responses.
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value>>>>>,
    /// Request ID counter.
    request_counter: AtomicU64,
    /// Permission handler for tool use requests.
    permission_handler: Option<Arc<dyn PermissionHandler>>,
    /// Channel to receive messages.
    message_rx: mpsc::Receiver<Result<SdkMessage>>,
    /// Session ID (available after first message).
    session_id: Option<String>,
    /// Payload from the `initialize` handshake, if it completed.
    initialization: Option<Value>,
    /// How long each control request may wait for a response.
    control_timeout: Duration,
    /// Whether the query has completed.
    completed: bool,
}

impl Query {
    /// Create a new query with a prompt.
    pub async fn new(
        prompt: impl Into<String>,
        options: QueryOptions,
        permission_handler: Option<Arc<dyn PermissionHandler>>,
    ) -> Result<Self> {
        let prompt = prompt.into();
        let args = options.build_args();

        let env = options.env.clone().map(|e| e.into_iter().collect());

        let mut transport =
            ProcessTransport::spawn(options.executable.clone(), args, options.cwd.clone(), env)
                .await?;

        // The reader task waits on stdout without holding the stdin lock,
        // otherwise initialize (and every later control write) deadlocks.
        let stdout_rx = transport.take_stdout_rx();
        let transport = Arc::new(Mutex::new(transport));
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));

        // Create message channel
        let (message_tx, message_rx) = mpsc::channel(256);

        // Spawn message processing task
        let transport_clone = transport.clone();
        let pending_clone = pending_requests.clone();
        let handler_clone = permission_handler.clone();

        tokio::spawn(async move {
            Self::process_messages(
                transport_clone,
                stdout_rx,
                pending_clone,
                handler_clone,
                message_tx,
            )
            .await;
        });

        let mut query = Self {
            transport,
            pending_requests,
            request_counter: AtomicU64::new(0),
            permission_handler,
            message_rx,
            session_id: None,
            initialization: None,
            control_timeout: options.control_timeout_or_default(),
            completed: false,
        };

        // Handshake first. The TS Query does the same: initialize, then the
        // user prompt. Sending the prompt first left session_id() empty and
        // made initializationResult() impossible.
        let init = query
            .send_control_request(ControlRequestData::Initialize(InitializeRequest::default()))
            .await
            .map_err(|error| match error {
                Error::InvalidMessage(message) => Error::InitializationFailed(message),
                other => other,
            })?;
        query.initialization = Some(init);

        query.send_prompt(&prompt).await?;

        Ok(query)
    }

    /// Send a prompt to the CLI.
    async fn send_prompt(&mut self, prompt: &str) -> Result<()> {
        let session_id = self.session_id.clone().unwrap_or_default();

        let message = SdkUserMessage {
            msg_type: UserMessageType::User,
            message: serde_json::json!({
                "role": "user",
                "content": prompt
            }),
            parent_tool_use_id: None,
            is_synthetic: None,
            tool_use_result: None,
            uuid: None,
            session_id,
            is_replay: None,
        };

        let mut transport = self.transport.lock().await;
        transport.send(&StdinMessage::UserMessage(message)).await
    }

    /// Process messages from the transport.
    async fn process_messages(
        transport: Arc<Mutex<ProcessTransport>>,
        mut stdout_rx: mpsc::Receiver<Result<StdoutMessage>>,
        pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value>>>>>,
        permission_handler: Option<Arc<dyn PermissionHandler>>,
        message_tx: mpsc::Sender<Result<SdkMessage>>,
    ) {
        loop {
            let msg = stdout_rx.recv().await;

            match msg {
                Some(Ok(stdout_msg)) => {
                    match stdout_msg {
                        StdoutMessage::Message(sdk_msg) => {
                            if message_tx.send(Ok(sdk_msg)).await.is_err() {
                                break;
                            }
                        }
                        StdoutMessage::ControlRequest(req) => {
                            // Handle control requests (e.g., permission checks)
                            Self::handle_control_request(&transport, &permission_handler, req)
                                .await;
                        }
                        StdoutMessage::ControlResponse(resp) => {
                            // Route response to waiting request
                            Self::handle_control_response(&pending_requests, resp).await;
                        }
                        StdoutMessage::KeepAlive(_) => {
                            trace!("Received keep-alive");
                        }
                    }
                }
                Some(Err(e)) => {
                    let fatal = !matches!(e, Error::UnrecognizedMessage { .. });
                    if message_tx.send(Err(e)).await.is_err() {
                        break;
                    }
                    if fatal {
                        break;
                    }
                }
                None => {
                    // Transport closed
                    break;
                }
            }
        }
    }

    /// Handle a control request from the CLI.
    async fn handle_control_request(
        transport: &Arc<Mutex<ProcessTransport>>,
        permission_handler: &Option<Arc<dyn PermissionHandler>>,
        request: SdkControlRequest,
    ) {
        debug!(request_id = %request.request_id, "Handling control request");

        let response = match request.request {
            ControlRequestData::CanUseTool(ref tool_req) => {
                // Handle permission request
                let result = if let Some(handler) = permission_handler {
                    handler
                        .can_use_tool(
                            &tool_req.tool_name,
                            &tool_req.input,
                            tool_req.permission_suggestions.clone(),
                            tool_req.blocked_path.clone(),
                            tool_req.decision_reason.clone(),
                            &tool_req.tool_use_id,
                            tool_req.agent_id.clone(),
                        )
                        .await
                } else {
                    // Default: allow all
                    Ok(PermissionResult::allow(tool_req.input.clone()))
                };

                match result {
                    Ok(perm_result) => SdkControlResponse {
                        msg_type: ControlResponseType::ControlResponse,
                        response: ControlResponseData::Success {
                            request_id: request.request_id.clone(),
                            response: Some(serde_json::to_value(perm_result).unwrap_or_default()),
                        },
                    },
                    Err(e) => SdkControlResponse {
                        msg_type: ControlResponseType::ControlResponse,
                        response: ControlResponseData::Error {
                            request_id: request.request_id.clone(),
                            error: e.to_string(),
                            pending_permission_requests: None,
                        },
                    },
                }
            }
            ControlRequestData::HookCallback(ref hook_req) => {
                // Typed continue stub. No host hook is invoked.
                let stub = HookCallbackStub::from_request(hook_req);
                debug!(
                    request_id = %request.request_id,
                    callback_id = %stub.callback_id,
                    hook_event_name = ?stub.hook_event_name,
                    hook_ran = stub.hook_ran,
                    "hook_callback stub continue"
                );
                SdkControlResponse {
                    msg_type: ControlResponseType::ControlResponse,
                    response: ControlResponseData::Success {
                        request_id: request.request_id.clone(),
                        response: Some(stub.response_value()),
                    },
                }
            }
            _ => {
                // Respond with success for other requests
                SdkControlResponse {
                    msg_type: ControlResponseType::ControlResponse,
                    response: ControlResponseData::Success {
                        request_id: request.request_id.clone(),
                        response: None,
                    },
                }
            }
        };

        // Send response
        let mut transport = transport.lock().await;
        if let Err(e) = transport
            .send(&StdinMessage::ControlResponse(response))
            .await
        {
            warn!(error = %e, "Failed to send control response");
        }
    }

    /// Handle a control response from the CLI.
    async fn handle_control_response(
        pending_requests: &Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value>>>>>,
        response: SdkControlResponse,
    ) {
        let (request_id, result) = match response.response {
            ControlResponseData::Success {
                request_id,
                response,
            } => (request_id, Ok(response.unwrap_or(Value::Null))),
            ControlResponseData::Error {
                request_id, error, ..
            } => (request_id, Err(Error::InvalidMessage(error))),
        };

        let mut pending = pending_requests.lock().await;
        if let Some(tx) = pending.remove(&request_id) {
            let _ = tx.send(result);
        }
    }

    /// Send a control request and wait for response.
    async fn send_control_request(&self, request: ControlRequestData) -> Result<Value> {
        let request_id = format!(
            "sdk-{}",
            self.request_counter.fetch_add(1, Ordering::SeqCst)
        );

        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        // Send request
        let control_req = SdkControlRequest {
            msg_type: ControlRequestType::ControlRequest,
            request_id: request_id.clone(),
            request,
        };

        {
            let mut transport = self.transport.lock().await;
            transport
                .send(&StdinMessage::ControlRequest(control_req))
                .await?;
        }

        match tokio::time::timeout(self.control_timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) | Err(_) => {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request_id);
                Err(Error::ControlTimeout)
            }
        }
    }

    /// Interrupt the current query execution.
    pub async fn interrupt(&self) -> Result<()> {
        self.send_control_request(ControlRequestData::Interrupt)
            .await?;
        Ok(())
    }

    /// Change the permission mode.
    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<()> {
        self.send_control_request(ControlRequestData::SetPermissionMode(
            SetPermissionModeRequest { mode },
        ))
        .await?;
        Ok(())
    }

    /// Change the model.
    pub async fn set_model(&self, model: Option<String>) -> Result<()> {
        self.send_control_request(ControlRequestData::SetModel(SetModelRequest { model }))
            .await?;
        Ok(())
    }

    /// Set maximum thinking tokens.
    pub async fn set_max_thinking_tokens(&self, max_tokens: Option<u32>) -> Result<()> {
        self.send_control_request(ControlRequestData::SetMaxThinkingTokens(
            SetMaxThinkingTokensRequest {
                max_thinking_tokens: max_tokens,
            },
        ))
        .await?;
        Ok(())
    }

    /// Get MCP server status.
    pub async fn mcp_server_status(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpStatus)
            .await
    }

    /// Rewind files to a specific user message.
    pub async fn rewind_files(&self, user_message_id: &str) -> Result<()> {
        self.send_control_request(ControlRequestData::RewindFiles(
            crate::protocol::RewindFilesRequest {
                user_message_id: user_message_id.to_string(),
            },
        ))
        .await?;
        Ok(())
    }

    /// Merge settings into the flag settings layer (TS `applyFlagSettings`).
    pub async fn apply_flag_settings(&self, settings: Value) -> Result<Value> {
        self.send_control_request(ControlRequestData::ApplyFlagSettings(
            crate::protocol::ApplyFlagSettingsRequest { settings },
        ))
        .await
    }

    /// Replace dynamically managed MCP servers (TS `setMcpServers`).
    pub async fn set_mcp_servers(&self, servers: Value) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpSetServers(
            crate::protocol::McpSetServersRequest { servers },
        ))
        .await
    }

    /// Stop a running task (TS `stopTask`).
    pub async fn stop_task(&self, task_id: &str) -> Result<()> {
        self.send_control_request(ControlRequestData::StopTask(
            crate::protocol::StopTaskRequest {
                task_id: task_id.to_string(),
            },
        ))
        .await?;
        Ok(())
    }

    /// Context-window usage by category (TS `getContextUsage`).
    pub async fn get_context_usage(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetContextUsage)
            .await
    }

    /// Background in-flight foreground tasks (TS `backgroundTasks`).
    pub async fn background_tasks(&self, tool_use_id: Option<&str>) -> Result<Value> {
        self.send_control_request(ControlRequestData::BackgroundTasks(
            crate::protocol::BackgroundTasksRequest {
                tool_use_id: tool_use_id.map(str::to_string),
            },
        ))
        .await
    }

    /// Drop a queued async user message (TS).
    pub async fn cancel_async_message(&self, message_uuid: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::CancelAsyncMessage(
            crate::protocol::CancelAsyncMessageRequest {
                message_uuid: message_uuid.to_string(),
            },
        ))
        .await
    }

    /// Session cost totals.
    pub async fn get_session_cost(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetSessionCost)
            .await
    }

    /// Structured `/usage` payload.
    pub async fn get_usage(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetUsage)
            .await
    }

    /// Remote CLI binary version.
    pub async fn get_binary_version(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetBinaryVersion)
            .await
    }

    /// At-mention file autocomplete.
    pub async fn file_suggestions(&self, query: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::FileSuggestions(
            crate::protocol::FileSuggestionsRequest {
                query: query.to_string(),
            },
        ))
        .await
    }

    /// Reload plugins, commands, and MCP status.
    pub async fn reload_plugins(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::ReloadPlugins)
            .await
    }

    /// Reload skills.
    pub async fn reload_skills(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::ReloadSkills)
            .await
    }

    /// Reconnect one MCP server.
    pub async fn reconnect_mcp_server(&self, server_name: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpReconnect(
            crate::protocol::McpReconnectRequest {
                server_name: server_name.to_string(),
            },
        ))
        .await
    }

    /// Enable or disable one MCP server.
    pub async fn toggle_mcp_server(&self, server_name: &str, enabled: bool) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpToggle(
            crate::protocol::McpToggleRequest {
                server_name: server_name.to_string(),
                enabled,
            },
        ))
        .await
    }

    /// Set the session title.
    pub async fn rename_session(&self, title: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::RenameSession(
            crate::protocol::RenameSessionRequest {
                title: title.to_string(),
            },
        ))
        .await
    }

    /// Get the session ID (available after receiving first message).
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Full `initialize` handshake payload (`commands`, `models`, `account`, …).
    ///
    /// Present after [`Query::new`] returns. Matches TS
    /// `Query.initializationResult()`.
    pub fn initialization_result(&self) -> Option<&Value> {
        self.initialization.as_ref()
    }

    /// Models advertised in the initialize handshake (TS `supportedModels`).
    ///
    /// There is no `list_models` control subtype; this is the handshake
    /// field. Absent until initialize completes.
    pub fn supported_models(&self) -> Option<&Value> {
        self.initialization.as_ref().map(|value| &value["models"])
    }

    /// Check if the query has completed.
    pub fn is_completed(&self) -> bool {
        self.completed
    }
}

impl Stream for Query {
    type Item = Result<SdkMessage>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.completed {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.message_rx).poll_recv(cx) {
            Poll::Ready(Some(result)) => {
                // Update session_id from messages
                if let Ok(ref msg) = result {
                    match msg {
                        SdkMessage::System(sys) => {
                            if let crate::protocol::SdkSystemMessage::Init(init) = sys {
                                self.session_id = Some(init.session_id.clone());
                            }
                        }
                        SdkMessage::Result(_) => {
                            self.completed = true;
                        }
                        _ => {}
                    }
                }
                Poll::Ready(Some(result))
            }
            Poll::Ready(None) => {
                self.completed = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{OutputFormat, SystemPromptConfig};
    use crate::transport::ExecutableConfig;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_query_options_build_args() {
        let options = QueryOptions::new()
            .model("claude-sonnet-4-5-20250929")
            .max_turns(10)
            .max_budget_usd(1.0);

        let args = options.build_args();

        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-5-20250929".to_string()));
        assert!(args.contains(&"--max-turns".to_string()));
        assert!(args.contains(&"10".to_string()));
        assert!(args.contains(&"--max-budget-usd".to_string()));
        assert!(args.contains(&"1".to_string()));
    }

    #[test]
    fn build_args_emits_auto_mode_fallback_model_and_plugin_dir() {
        let mut options = QueryOptions::new().permission_mode(PermissionMode::Auto);
        options.fallback_model = Some("sonnet".to_string());
        options.plugins = vec![crate::options::PluginConfig::Local {
            path: "/tmp/plug".to_string(),
        }];
        let args = options.build_args();
        assert!(args.windows(2).any(|w| w == ["--permission-mode", "auto"]));
        assert!(args.windows(2).any(|w| w == ["--fallback-model", "sonnet"]));
        assert!(args.windows(2).any(|w| w == ["--plugin-dir", "/tmp/plug"]));
    }

    fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.windows(2)
            .find_map(|w| (w[0] == flag).then(|| w[1].as_str()))
    }

    #[test]
    fn build_args_emits_remaining_query_options_as_cli_flags() {
        let mut options = QueryOptions::new();
        options.system_prompt = Some(SystemPromptConfig::Custom("be terse".into()));
        options.mcp_servers.insert(
            "docs".into(),
            crate::options::McpServerConfig::Stdio {
                command: "npx".into(),
                args: Some(vec!["-y".into(), "docs".into()]),
                env: None,
            },
        );
        options.agents.insert(
            "reviewer".into(),
            crate::options::AgentDefinition {
                description: "Reviews code".into(),
                prompt: "You are a reviewer".into(),
                tools: Some(vec!["Read".into()]),
                disallowed_tools: Some(vec!["Bash".into()]),
                model: Some(crate::options::AgentModel::Haiku),
            },
        );
        options.sandbox = Some(crate::options::SandboxSettings {
            enabled: Some(true),
            auto_allow_bash_if_sandboxed: Some(true),
            network: Some(crate::options::SandboxNetworkConfig {
                allow_local_binding: Some(true),
                allow_unix_sockets: Some(vec!["/tmp/sock".into()]),
            }),
        });
        options.output_format = Some(OutputFormat {
            format_type: "json_schema".into(),
            schema: serde_json::json!({
                "type": "object",
                "properties": { "ok": { "type": "boolean" } },
                "required": ["ok"]
            }),
        });
        options.tools = Some(crate::options::ToolsConfig::Names(vec![
            "Read".into(),
            "Bash".into(),
        ]));
        options.thinking = Some(crate::options::ThinkingConfig::Adaptive {
            display: Some(crate::options::ThinkingDisplay::Summarized),
        });
        options.effort = Some(crate::options::EffortLevel::High);
        options.max_thinking_tokens = Some(99);

        let args = options.build_args();

        assert!(
            args.windows(2)
                .any(|w| w == ["--output-format", "stream-json"]),
            "structured schema must not replace the stream-json transport: {args:?}"
        );
        assert!(!args.iter().any(|a| a == "--sandbox"));
        assert_eq!(flag_value(&args, "--system-prompt"), Some("be terse"));
        assert_eq!(flag_value(&args, "--tools"), Some("Read,Bash"));
        assert_eq!(flag_value(&args, "--thinking"), Some("adaptive"));
        assert_eq!(flag_value(&args, "--thinking-display"), Some("summarized"));
        assert_eq!(flag_value(&args, "--effort"), Some("high"));
        assert!(
            flag_value(&args, "--max-thinking-tokens").is_none(),
            "thinking takes precedence over max_thinking_tokens"
        );

        let mcp: Value =
            serde_json::from_str(flag_value(&args, "--mcp-config").expect("--mcp-config")).unwrap();
        assert_eq!(mcp["mcpServers"]["docs"]["type"], "stdio");
        assert_eq!(mcp["mcpServers"]["docs"]["command"], "npx");

        let agents: Value =
            serde_json::from_str(flag_value(&args, "--agents").expect("--agents")).unwrap();
        assert_eq!(agents["reviewer"]["description"], "Reviews code");
        assert_eq!(agents["reviewer"]["disallowedTools"][0], "Bash");
        assert_eq!(agents["reviewer"]["model"], "haiku");

        let settings: Value =
            serde_json::from_str(flag_value(&args, "--settings").expect("--settings")).unwrap();
        assert_eq!(settings["sandbox"]["enabled"], true);
        assert_eq!(settings["sandbox"]["autoAllowBashIfSandboxed"], true);
        assert_eq!(settings["sandbox"]["network"]["allowLocalBinding"], true);

        let schema: Value =
            serde_json::from_str(flag_value(&args, "--json-schema").expect("--json-schema"))
                .unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"][0], "ok");
    }

    #[test]
    fn build_args_emits_append_system_prompt_and_tools_default() {
        let mut options = QueryOptions::new();
        options.system_prompt = Some(SystemPromptConfig::Preset {
            append: Some("always cite files".into()),
        });
        options.tools = Some(crate::options::ToolsConfig::Default);
        options.thinking = Some(crate::options::ThinkingConfig::Disabled);
        let args = options.build_args();
        assert_eq!(
            flag_value(&args, "--append-system-prompt"),
            Some("always cite files")
        );
        assert!(flag_value(&args, "--system-prompt").is_none());
        assert_eq!(flag_value(&args, "--tools"), Some("default"));
        assert_eq!(flag_value(&args, "--thinking"), Some("disabled"));
    }

    #[test]
    fn build_args_emits_enabled_thinking_budget_as_max_thinking_tokens() {
        let mut options = QueryOptions::new();
        options.thinking = Some(crate::options::ThinkingConfig::Enabled {
            budget_tokens: Some(2048),
            display: Some(crate::options::ThinkingDisplay::Omitted),
        });
        let args = options.build_args();
        assert_eq!(flag_value(&args, "--max-thinking-tokens"), Some("2048"));
        assert!(flag_value(&args, "--thinking").is_none());
        assert_eq!(flag_value(&args, "--thinking-display"), Some("omitted"));
    }

    #[test]
    fn initialize_control_request_serializes_with_the_wire_subtype() {
        let request = SdkControlRequest {
            msg_type: ControlRequestType::ControlRequest,
            request_id: "sdk-0".to_string(),
            request: ControlRequestData::Initialize(InitializeRequest::default()),
        };
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["type"], "control_request");
        assert_eq!(value["request_id"], "sdk-0");
        assert_eq!(value["request"]["subtype"], "initialize");
    }

    fn unique_temp_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("claude-sdk-p2-{nanos}-{n}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_executable(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, body).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn options_for_fake(path: PathBuf, timeout: Duration) -> QueryOptions {
        let mut options = QueryOptions::new().control_timeout(timeout);
        options.executable = ExecutableConfig {
            path: Some(path),
            executable: None,
            executable_args: Vec::new(),
        };
        options
    }

    /// A fake CLI that answers `initialize` and records every stdin line.
    const FAKE_OK: &str = r#"#!/usr/bin/env node
const fs = require('fs');
const readline = require('readline');
const log = process.env.FAKE_LOG;
const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
  if (!line) return;
  if (log) fs.appendFileSync(log, line + '\n');
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.type === 'control_request' && msg.request) {
    const payload = msg.request.subtype === 'initialize'
      ? JSON.parse('{"commands":[{"name":"help","description":"help"}],"agents":[],"output_style":"default","available_output_styles":["default"],"models":[{"value":"sonnet","displayName":"Sonnet"}],"account":{"email":"t@example.com"}}')
      : { "echo": msg.request.subtype };
    process.stdout.write(JSON.stringify({
      type: 'control_response',
      response: {
        subtype: 'success',
        request_id: msg.request_id,
        response: payload
      }
    }) + '\n');
  }
});
"#;

    /// A fake CLI that returns an initialize error.
    const FAKE_INIT_ERROR: &str = r#"#!/usr/bin/env node
const readline = require('readline');
const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
  if (!line) return;
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.type === 'control_request' && msg.request && msg.request.subtype === 'initialize') {
    process.stdout.write(JSON.stringify({
      type: 'control_response',
      response: {
        subtype: 'error',
        request_id: msg.request_id,
        error: 'no session'
      }
    }) + '\n');
  }
});
"#;

    /// A fake CLI that never writes a control response.
    const FAKE_HANG: &str = r#"#!/usr/bin/env node
const readline = require('readline');
readline.createInterface({ input: process.stdin });
"#;

    /// A fake CLI that answers initialize, then sends a PreToolUse hook_callback.
    const FAKE_HOOK: &str = r#"#!/usr/bin/env node
const fs = require('fs');
const readline = require('readline');
const log = process.env.FAKE_LOG;
const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
  if (!line) return;
  if (log) fs.appendFileSync(log, line + '\n');
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.type === 'control_request' && msg.request && msg.request.subtype === 'initialize') {
    process.stdout.write(JSON.stringify({
      type: 'control_response',
      response: {
        subtype: 'success',
        request_id: msg.request_id,
        response: JSON.parse('{"commands":[],"agents":[],"output_style":"default","available_output_styles":["default"],"models":[],"account":{}}')
      }
    }) + '\n');
    process.stdout.write(JSON.stringify({
      type: 'control_request',
      request_id: 'cli-hook-1',
      request: {
        subtype: 'hook_callback',
        callback_id: 'cb-pre-1',
        tool_use_id: 'tu-1',
        input: {
          hook_event_name: 'PreToolUse',
          tool_name: 'Bash',
          tool_input: { command: 'ls' },
          tool_use_id: 'tu-1'
        }
      }
    }) + '\n');
  }
});
"#;

    #[tokio::test]
    async fn query_new_sends_initialize_before_the_user_prompt() {
        let dir = unique_temp_dir();
        let fake = write_executable(&dir, "fake-claude", FAKE_OK);
        let log = dir.join("stdin.jsonl");
        let mut options = options_for_fake(fake, Duration::from_secs(5));
        options.env = Some(
            [("FAKE_LOG".to_string(), log.display().to_string())]
                .into_iter()
                .collect(),
        );

        let query = Query::new("hello", options, None).await.unwrap();
        let init = query.initialization_result().expect("handshake payload");
        assert_eq!(init["output_style"], "default");
        assert_eq!(init["commands"][0]["name"], "help");
        assert_eq!(init["account"]["email"], "t@example.com");
        assert_eq!(query.supported_models().unwrap()[0]["value"], "sonnet");

        // Drop the query so the child exits and flushes the log.
        drop(query);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let recorded = fs::read_to_string(&log).unwrap();
        let lines: Vec<&str> = recorded.lines().filter(|line| !line.is_empty()).collect();
        assert!(
            lines.len() >= 2,
            "expected initialize then user prompt, got {recorded}"
        );
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        let second: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(first["type"], "control_request");
        assert_eq!(first["request"]["subtype"], "initialize");
        assert_eq!(second["type"], "user");
        assert_eq!(second["message"]["content"], "hello");
    }

    #[tokio::test]
    async fn p3_control_methods_round_trip_on_the_fake_cli() {
        let dir = unique_temp_dir();
        let fake = write_executable(&dir, "fake-claude", FAKE_OK);
        let log = dir.join("stdin.jsonl");
        let mut options = options_for_fake(fake, Duration::from_secs(5));
        options.env = Some(
            [("FAKE_LOG".to_string(), log.display().to_string())]
                .into_iter()
                .collect(),
        );

        let query = Query::new("hello", options, None).await.unwrap();
        query
            .apply_flag_settings(serde_json::json!({"permissions": {}}))
            .await
            .unwrap();
        query
            .set_mcp_servers(serde_json::json!({"docs": {"type": "stdio", "command": "npx"}}))
            .await
            .unwrap();
        query.stop_task("task-1").await.unwrap();
        query.get_context_usage().await.unwrap();
        drop(query);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let recorded = fs::read_to_string(&log).unwrap();
        let subtypes: Vec<String> = recorded
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|value| value["type"] == "control_request")
            .filter_map(|value| value["request"]["subtype"].as_str().map(str::to_string))
            .collect();
        assert_eq!(
            subtypes,
            vec![
                "initialize",
                "apply_flag_settings",
                "mcp_set_servers",
                "stop_task",
                "get_context_usage",
            ]
        );
    }

    #[test]
    fn p3_control_request_subtypes_serialize_on_the_wire() {
        let cases = [
            (
                ControlRequestData::ApplyFlagSettings(crate::protocol::ApplyFlagSettingsRequest {
                    settings: serde_json::json!({"a": 1}),
                }),
                "apply_flag_settings",
            ),
            (
                ControlRequestData::McpSetServers(crate::protocol::McpSetServersRequest {
                    servers: serde_json::json!({}),
                }),
                "mcp_set_servers",
            ),
            (
                ControlRequestData::StopTask(crate::protocol::StopTaskRequest {
                    task_id: "t1".into(),
                }),
                "stop_task",
            ),
            (ControlRequestData::GetContextUsage, "get_context_usage"),
            (
                ControlRequestData::BackgroundTasks(crate::protocol::BackgroundTasksRequest {
                    tool_use_id: None,
                }),
                "background_tasks",
            ),
            (
                ControlRequestData::CancelAsyncMessage(
                    crate::protocol::CancelAsyncMessageRequest {
                        message_uuid: "u1".into(),
                    },
                ),
                "cancel_async_message",
            ),
            (ControlRequestData::GetSessionCost, "get_session_cost"),
            (ControlRequestData::GetUsage, "get_usage"),
            (ControlRequestData::GetBinaryVersion, "get_binary_version"),
            (
                ControlRequestData::FileSuggestions(crate::protocol::FileSuggestionsRequest {
                    query: "src/".into(),
                }),
                "file_suggestions",
            ),
            (ControlRequestData::ReloadPlugins, "reload_plugins"),
            (ControlRequestData::ReloadSkills, "reload_skills"),
            (
                ControlRequestData::McpReconnect(crate::protocol::McpReconnectRequest {
                    server_name: "docs".into(),
                }),
                "mcp_reconnect",
            ),
            (
                ControlRequestData::McpToggle(crate::protocol::McpToggleRequest {
                    server_name: "docs".into(),
                    enabled: false,
                }),
                "mcp_toggle",
            ),
            (
                ControlRequestData::RenameSession(crate::protocol::RenameSessionRequest {
                    title: "t".into(),
                }),
                "rename_session",
            ),
        ];
        for (request, subtype) in cases {
            let value = serde_json::to_value(&request).unwrap();
            assert_eq!(value["subtype"], subtype);
        }
    }

    #[tokio::test]
    async fn a_failed_initialize_is_initialization_failed() {
        let dir = unique_temp_dir();
        let fake = write_executable(&dir, "fake-claude", FAKE_INIT_ERROR);
        let error = match Query::new(
            "hello",
            options_for_fake(fake, Duration::from_secs(5)),
            None,
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("expected initialize error"),
        };
        match error {
            Error::InitializationFailed(message) => assert_eq!(message, "no session"),
            other => panic!("expected InitializationFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_silent_control_channel_times_out() {
        let dir = unique_temp_dir();
        let fake = write_executable(&dir, "fake-claude", FAKE_HANG);
        let started = std::time::Instant::now();
        let error = match Query::new(
            "hello",
            options_for_fake(fake, Duration::from_millis(200)),
            None,
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("expected hung initialize"),
        };
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "timeout must not wait the default 60s"
        );
        match error {
            Error::ControlTimeout => {}
            other => panic!("expected ControlTimeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn hook_callback_replies_continue_without_running_a_hook() {
        let dir = unique_temp_dir();
        let fake = write_executable(&dir, "fake-claude", FAKE_HOOK);
        let log = dir.join("stdin.jsonl");
        let mut options = options_for_fake(fake, Duration::from_secs(5));
        options.env = Some(
            [("FAKE_LOG".to_string(), log.display().to_string())]
                .into_iter()
                .collect(),
        );

        let query = Query::new("hello", options, None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
        drop(query);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let recorded = fs::read_to_string(&log).unwrap();
        let hook_reply = recorded
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|value| {
                value["type"] == "control_response"
                    && value["response"]["request_id"] == "cli-hook-1"
            })
            .expect("expected a control_response for the CLI hook_callback");
        assert_eq!(hook_reply["response"]["subtype"], "success");
        assert_eq!(hook_reply["response"]["response"]["continue"], true);
        assert!(
            hook_reply["response"]["response"]
                .get("hookSpecificOutput")
                .is_none()
        );
        assert!(hook_reply["response"]["response"].get("decision").is_none());
    }
}
