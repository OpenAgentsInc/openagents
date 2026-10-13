//! Query struct for executing prompts and streaming responses.

use crate::callbacks::{ElicitationHandler, HookRegistry, UserDialogHandler};
use crate::error::{Error, Result};
use crate::mcp::SdkMcpServer;
use crate::options::QueryOptions;
use crate::permissions::PermissionHandler;
use crate::protocol::{
    ApplyFlagSettingsRequest, BackgroundTasksRequest, CancelAsyncMessageRequest,
    ControlRequestData, ControlRequestType, ControlResponseData, ElicitationResult,
    FileSuggestionsRequest, GetTaskOutputRequest, InterruptRequest, McpCallRequest,
    McpReadResourceRequest, McpReconnectRequest, McpSetServersRequest, McpToggleRequest,
    PermissionMode, ReadFileRequest, RegisterRepoRootRequest, RenameSessionRequest,
    RewindFilesRequest, SdkControlRequest, SdkControlResponse, SdkMessage, SdkUserMessage,
    SeedReadStateRequest, SetColorRequest, SetMaxThinkingTokensRequest, SetModelRequest,
    SetPermissionModeRequest, StdinMessage, StdoutMessage, StopTaskRequest, UpdateSettingsRequest,
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

type PendingRequests = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value>>>>>;

/// Subtypes meant for the machine serving the session's tools. The TS SDK
/// leaves them unanswered, and so does this crate.
const REMOTE_TOOL_SUBTYPES: &[&str] = &[
    "remote_tool_call",
    "remote_plumbing_call",
    "remote_tools_probe",
    "remote_tools_reannounce",
];

/// What answers the control requests the CLI sends.
struct ControlHandlers {
    transport: Arc<Mutex<ProcessTransport>>,
    permission_handler: Option<Arc<dyn PermissionHandler>>,
    hooks: HookRegistry,
    on_elicitation: Option<Arc<dyn ElicitationHandler>>,
    on_user_dialog: Option<Arc<dyn UserDialogHandler>>,
    /// SDK-hosted MCP servers, by name.
    sdk_mcp_servers: HashMap<String, Arc<SdkMcpServer>>,
    /// Inbound requests being answered, so a `control_cancel_request` can
    /// stop one and a duplicate delivery is skipped.
    in_flight: Mutex<HashMap<String, tokio::task::AbortHandle>>,
}

impl ControlHandlers {
    /// Answer a request on its own task, so a slow hook or permission
    /// prompt does not stall the stdout reader.
    async fn spawn(self: &Arc<Self>, request: SdkControlRequest) {
        let mut in_flight = self.in_flight.lock().await;
        if in_flight.contains_key(&request.request_id) {
            debug!(request_id = %request.request_id, "skipping a duplicate control request");
            return;
        }
        let request_id = request.request_id.clone();
        let handlers = self.clone();
        let task = tokio::spawn(async move {
            let request_id = request.request_id.clone();
            if let Some(response) = handlers.answer(request).await {
                handlers.send(response).await;
            }
            handlers.in_flight.lock().await.remove(&request_id);
        });
        in_flight.insert(request_id, task.abort_handle());
    }

    /// Stop answering a request the CLI withdrew.
    async fn cancel(&self, request_id: &str) {
        if let Some(handle) = self.in_flight.lock().await.remove(request_id) {
            handle.abort();
        }
    }

    async fn send(&self, response: SdkControlResponse) {
        let mut transport = self.transport.lock().await;
        if let Err(e) = transport
            .send(&StdinMessage::ControlResponse(response))
            .await
        {
            warn!(error = %e, "failed to send control response");
        }
    }

    /// The response to one inbound request, or `None` to stay silent.
    async fn answer(&self, request: SdkControlRequest) -> Option<SdkControlResponse> {
        let id = request.request_id;
        debug!(request_id = %id, subtype = ?request.request.subtype(), "handling control request");
        let result: Result<Option<Value>> = match request.request {
            ControlRequestData::CanUseTool(ref tool) => match &self.permission_handler {
                Some(handler) => handler
                    .can_use_tool_request(tool)
                    .await
                    .and_then(|r| Ok(Some(serde_json::to_value(r.within_limits())?))),
                None => Err(Error::InvalidMessage(
                    "canUseTool callback is not provided".into(),
                )),
            },
            ControlRequestData::HookCallback(ref hook) => self
                .hooks
                .run(hook)
                .await
                .and_then(|out| Ok(Some(serde_json::to_value(out)?))),
            ControlRequestData::McpMessage(ref message) => {
                match self.sdk_mcp_servers.get(&message.server_name) {
                    Some(server) => Ok(Some(serde_json::json!({
                        "mcp_response": server.handle_message(&message.message).await
                    }))),
                    None => Err(Error::McpError(format!(
                        "SDK MCP server not found: {}",
                        message.server_name
                    ))),
                }
            }
            ControlRequestData::Elicitation(ref elicitation) => match &self.on_elicitation {
                Some(handler) => match handler.elicit(elicitation).await {
                    Ok(Some(answer)) => serde_json::to_value(answer).map(Some).map_err(Error::from),
                    Ok(None) => return None,
                    Err(e) => Err(e),
                },
                None => serde_json::to_value(ElicitationResult::decline())
                    .map(Some)
                    .map_err(Error::from),
            },
            ControlRequestData::RequestUserDialog(ref dialog) => match &self.on_user_dialog {
                Some(handler) => match handler.dialog(dialog).await {
                    Ok(Some(answer)) => Ok(Some(answer)),
                    Ok(None) => return None,
                    Err(e) => Err(e),
                },
                None => {
                    debug!(
                        dialog_kind = %dialog.dialog_kind,
                        "no user dialog handler; leaving request_user_dialog unanswered"
                    );
                    return None;
                }
            },
            ControlRequestData::OauthTokenRefresh => Err(Error::InvalidMessage(
                "getOAuthToken callback is not provided".into(),
            )),
            ControlRequestData::HostAuthTokenRefresh => Err(Error::InvalidMessage(
                "getHostAuthToken callback is not provided".into(),
            )),
            ref other => Err(Error::InvalidMessage(format!(
                "unsupported control request subtype: {}",
                other.subtype().unwrap_or_default()
            ))),
        };
        Some(match result {
            Ok(response) => SdkControlResponse::success(id, response),
            Err(e) => SdkControlResponse::error(id, e.to_string()),
        })
    }
}

/// A query execution that streams messages from Claude.
///
/// A query made with [`Query::new`] sends one prompt and its stream ends
/// with the turn's `result`. A query made with [`Query::streaming`] keeps
/// the CLI's stdin open (TS `AsyncIterable` prompts and `streamInput`):
/// send user messages at any time with [`Query::send_user_message`],
/// [`Query::send_user_content`], or a cloned [`Query::input`] handle; each
/// turn ends with a `result`, and the stream goes on until
/// [`Query::end_input`] closes stdin and the CLI exits.
pub struct Query {
    /// The process transport.
    transport: Arc<Mutex<ProcessTransport>>,
    /// Channel to receive messages.
    message_rx: mpsc::Receiver<Result<SdkMessage>>,
    /// Session ID (available after first message).
    session_id: Option<String>,
    /// Payload from the `initialize` handshake, if it completed.
    initialization: Option<Value>,
    /// Writes user messages to stdin; shared with [`QueryInput`] handles.
    input: QueryInput,
    /// Streaming input: a `result` ends a turn, not the stream.
    streaming: bool,
    /// Whether the query has completed.
    completed: bool,
}

/// A cloneable handle that writes user messages to a query's CLI, so one
/// task can send while another reads the [`Query`] stream. Get one with
/// [`Query::input`].
#[derive(Clone)]
pub struct QueryInput {
    transport: Arc<Mutex<ProcessTransport>>,
    /// The session ID from `system/init`, empty until it arrives.
    session_id: Arc<std::sync::Mutex<String>>,
    /// Send messages with `client_composed: true`.
    verbatim_prompts: bool,
    /// Control requests waiting for responses, shared with the reader task.
    pending_requests: PendingRequests,
    /// Request ID counter, shared by every handle.
    request_counter: Arc<AtomicU64>,
    /// How long each control request may wait for a response.
    control_timeout: Duration,
}

impl QueryInput {
    /// Send a user message as it is (TS: yield an `SDKUserMessage` into
    /// `streamInput`). An empty `session_id` is filled with the session's,
    /// once known, and `verbatim_prompts` sets `client_composed` when the
    /// message leaves it unset.
    pub async fn send_message(&self, mut message: SdkUserMessage) -> Result<()> {
        if message.session_id.is_empty() {
            message.session_id = self
                .session_id
                .lock()
                .map(|id| id.clone())
                .unwrap_or_default();
        }
        if message.client_composed.is_none() && self.verbatim_prompts {
            message.client_composed = Some(true);
        }
        let mut transport = self.transport.lock().await;
        transport.send(&StdinMessage::UserMessage(message)).await
    }

    /// Send a plain-text user message. During a turn the CLI queues it
    /// and takes it up as it can; between turns it starts the next turn.
    pub async fn send_user_message(&self, text: impl Into<String>) -> Result<()> {
        self.send_message(SdkUserMessage::text(text)).await
    }

    /// Send a user message made of content blocks, for example
    /// [`crate::protocol::text_block`] and [`crate::protocol::image_block`].
    pub async fn send_user_content(&self, blocks: Vec<Value>) -> Result<()> {
        self.send_message(SdkUserMessage::blocks(blocks)).await
    }

    /// Close the CLI's stdin (TS `endInput`). The CLI finishes the current
    /// turn and its background work, then exits, which ends the stream.
    /// Later sends and control requests fail. A second call does nothing.
    pub async fn end_input(&self) -> Result<()> {
        self.transport.lock().await.end_input().await
    }

    /// Whether [`QueryInput::end_input`] closed stdin.
    pub async fn is_input_closed(&self) -> bool {
        self.transport.lock().await.is_input_closed()
    }

    /// Change the model (TS `setModel`) while another task reads the
    /// stream. `None` returns to the default model. The CLI applies it from
    /// its next model call on, the session and its history kept.
    pub async fn set_model(&self, model: Option<String>) -> Result<()> {
        self.send_control_request(ControlRequestData::SetModel(SetModelRequest { model }))
            .await
            .map(|_| ())
    }

    /// Send a control request and wait for response.
    async fn send_control_request(&self, request: ControlRequestData) -> Result<Value> {
        let request_id = format!(
            "sdk-{}",
            self.request_counter.fetch_add(1, Ordering::SeqCst)
        );

        let (tx, rx) = oneshot::channel();
        self.pending_requests
            .lock()
            .await
            .insert(request_id.clone(), tx);

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
                self.pending_requests.lock().await.remove(&request_id);
                Err(Error::ControlTimeout)
            }
        }
    }
}

impl Query {
    /// Create a new query with a prompt.
    ///
    /// With a `permission_handler`, the CLI sends permission prompts to it
    /// (`--permission-prompt-tool stdio`). Without one, the permission mode
    /// and rules decide, and a prompt the CLI would show is denied.
    pub async fn new(
        prompt: impl Into<String>,
        options: QueryOptions,
        permission_handler: Option<Arc<dyn PermissionHandler>>,
    ) -> Result<Self> {
        let prompt = prompt.into();
        let query = Self::start(options, permission_handler, false).await?;
        query.send_user_message(prompt).await?;
        Ok(query)
    }

    /// Start a query with streaming input and no prompt yet. The CLI runs
    /// the `initialize` handshake and then waits for user messages; send
    /// them with [`Query::send_user_message`], [`Query::send_user_content`],
    /// [`Query::send_message`], or a [`Query::input`] handle. Every turn
    /// ends with a `result` and the stream continues; it ends after
    /// [`Query::end_input`], once the CLI exits.
    ///
    /// Read the stream until it ends before dropping the query: dropping
    /// it kills the CLI's process group, background work included.
    pub async fn streaming(
        options: QueryOptions,
        permission_handler: Option<Arc<dyn PermissionHandler>>,
    ) -> Result<Self> {
        Self::start(options, permission_handler, true).await
    }

    /// Spawn the CLI and run the `initialize` handshake.
    async fn start(
        options: QueryOptions,
        permission_handler: Option<Arc<dyn PermissionHandler>>,
        streaming: bool,
    ) -> Result<Self> {
        options.validate(permission_handler.is_some())?;
        let args = options.build_args_for(permission_handler.is_some());
        let env = options.env_vars();
        let env = (!env.is_empty()).then_some(env);

        let mut transport = ProcessTransport::spawn_with(
            options.executable.clone(),
            args,
            options.cwd.clone(),
            env,
            &options.env_remove,
        )
        .await?;

        // The reader task waits on stdout without holding the stdin lock,
        // otherwise initialize (and every later control write) deadlocks.
        let stdout_rx = transport.take_stdout_rx();
        let transport = Arc::new(Mutex::new(transport));
        let pending_requests: PendingRequests = Arc::new(Mutex::new(HashMap::new()));

        let (hooks, hooks_payload) = HookRegistry::register(&options.hooks);
        let handlers = Arc::new(ControlHandlers {
            transport: transport.clone(),
            permission_handler,
            hooks,
            on_elicitation: options.on_elicitation.clone(),
            on_user_dialog: options.on_user_dialog.clone(),
            sdk_mcp_servers: options
                .sdk_mcp_servers
                .iter()
                .map(|s| (s.name().to_string(), s.clone()))
                .collect(),
            in_flight: Mutex::new(HashMap::new()),
        });

        let (message_tx, message_rx) = mpsc::channel(256);
        let pending_clone = pending_requests.clone();
        tokio::spawn(async move {
            Self::process_messages(handlers, stdout_rx, pending_clone, message_tx).await;
        });

        let input = QueryInput {
            transport: transport.clone(),
            session_id: Arc::new(std::sync::Mutex::new(String::new())),
            verbatim_prompts: options.verbatim_prompts,
            pending_requests,
            request_counter: Arc::new(AtomicU64::new(0)),
            control_timeout: options.control_timeout_or_default(),
        };
        let mut query = Self {
            transport,
            message_rx,
            session_id: None,
            initialization: None,
            input,
            streaming,
            completed: false,
        };

        // Handshake first, as the TS Query does: initialize, then the
        // user prompt.
        let init = query
            .send_control_request(ControlRequestData::Initialize(
                options.initialize_request(hooks_payload),
            ))
            .await
            .map_err(|error| match error {
                Error::InvalidMessage(message) => Error::InitializationFailed(message),
                other => other,
            })?;
        query.initialization = Some(init);

        Ok(query)
    }

    /// A cloneable handle that sends user messages and closes input while
    /// another task reads the stream.
    pub fn input(&self) -> QueryInput {
        self.input.clone()
    }

    /// Whether this query was started with [`Query::streaming`].
    pub fn is_streaming(&self) -> bool {
        self.streaming
    }

    /// Send a user message as it is. See [`QueryInput::send_message`].
    pub async fn send_message(&self, message: SdkUserMessage) -> Result<()> {
        self.input.send_message(message).await
    }

    /// Send a plain-text user message. See
    /// [`QueryInput::send_user_message`].
    pub async fn send_user_message(&self, text: impl Into<String>) -> Result<()> {
        self.input.send_user_message(text).await
    }

    /// Send a user message made of content blocks. See
    /// [`QueryInput::send_user_content`].
    pub async fn send_user_content(&self, blocks: Vec<Value>) -> Result<()> {
        self.input.send_user_content(blocks).await
    }

    /// Close the CLI's stdin so it exits once its work is done (TS
    /// `endInput`). See [`QueryInput::end_input`].
    pub async fn end_input(&self) -> Result<()> {
        self.input.end_input().await
    }

    /// Process messages from the transport.
    async fn process_messages(
        handlers: Arc<ControlHandlers>,
        mut stdout_rx: mpsc::Receiver<Result<StdoutMessage>>,
        pending_requests: PendingRequests,
        message_tx: mpsc::Sender<Result<SdkMessage>>,
    ) {
        while let Some(msg) = stdout_rx.recv().await {
            match msg {
                Ok(StdoutMessage::Message(sdk_msg)) => {
                    if message_tx.send(Ok(sdk_msg)).await.is_err() {
                        break;
                    }
                }
                Ok(StdoutMessage::ControlRequest(req)) => handlers.spawn(req).await,
                Ok(StdoutMessage::UnsupportedControlRequest(req)) => {
                    if REMOTE_TOOL_SUBTYPES.contains(&req.subtype.as_str()) {
                        debug!(subtype = %req.subtype, "leaving a remote tool request unanswered");
                    } else {
                        handlers
                            .send(SdkControlResponse::error(
                                req.request_id,
                                format!("unsupported control request subtype: {}", req.subtype),
                            ))
                            .await;
                    }
                }
                Ok(StdoutMessage::ControlCancelRequest(cancel)) => {
                    handlers.cancel(&cancel.request_id).await;
                }
                Ok(StdoutMessage::ControlResponse(resp)) => {
                    Self::handle_control_response(&handlers, &pending_requests, resp).await;
                }
                Ok(StdoutMessage::KeepAlive(_)) => trace!("received keep-alive"),
                Err(e) => {
                    let fatal = !matches!(e, Error::UnrecognizedMessage { .. });
                    if message_tx.send(Err(e)).await.is_err() || fatal {
                        break;
                    }
                }
            }
        }
    }

    /// Route a control response to its waiting request, and answer any
    /// permission or dialog requests it carries.
    async fn handle_control_response(
        handlers: &Arc<ControlHandlers>,
        pending_requests: &PendingRequests,
        response: SdkControlResponse,
    ) {
        let (request_id, result, carried) = match response.response {
            ControlResponseData::Success {
                request_id,
                response,
                pending_permission_requests,
                pending_user_dialog_requests,
            } => (
                request_id,
                Ok(response.unwrap_or(Value::Null)),
                [pending_permission_requests, pending_user_dialog_requests],
            ),
            ControlResponseData::Error {
                request_id,
                error,
                pending_permission_requests,
                pending_user_dialog_requests,
            } => (
                request_id,
                Err(Error::InvalidMessage(error)),
                [pending_permission_requests, pending_user_dialog_requests],
            ),
        };

        if let Some(tx) = pending_requests.lock().await.remove(&request_id) {
            let _ = tx.send(result);
        }
        for request in carried.into_iter().flatten().flatten() {
            if matches!(
                request.request,
                ControlRequestData::CanUseTool(_) | ControlRequestData::RequestUserDialog(_)
            ) {
                handlers.spawn(request).await;
            }
        }
    }

    /// Send a control request and wait for response.
    async fn send_control_request(&self, request: ControlRequestData) -> Result<Value> {
        self.input.send_control_request(request).await
    }

    /// Send a control request whose response carries nothing.
    async fn send_unit(&self, request: ControlRequestData) -> Result<()> {
        self.send_control_request(request).await.map(|_| ())
    }

    /// Interrupt the current turn.
    pub async fn interrupt(&self) -> Result<()> {
        self.send_unit(ControlRequestData::Interrupt(InterruptRequest::default()))
            .await
    }

    /// Interrupt the current turn and drop queued user messages.
    pub async fn interrupt_and_cancel_queued(&self) -> Result<()> {
        self.send_unit(ControlRequestData::Interrupt(InterruptRequest {
            cancel_queued: Some(true),
        }))
        .await
    }

    /// Change the permission mode.
    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<()> {
        self.send_unit(ControlRequestData::SetPermissionMode(
            SetPermissionModeRequest { mode },
        ))
        .await
    }

    /// Change the model. `None` returns to the default model.
    pub async fn set_model(&self, model: Option<String>) -> Result<()> {
        self.send_unit(ControlRequestData::SetModel(SetModelRequest { model }))
            .await
    }

    /// Set maximum thinking tokens.
    pub async fn set_max_thinking_tokens(&self, max_tokens: Option<u32>) -> Result<()> {
        self.send_unit(ControlRequestData::SetMaxThinkingTokens(
            SetMaxThinkingTokensRequest {
                max_thinking_tokens: max_tokens,
            },
        ))
        .await
    }

    /// Get MCP server status.
    pub async fn mcp_server_status(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpStatus)
            .await
    }

    /// Rewind files to a specific user message.
    pub async fn rewind_files(&self, user_message_id: &str) -> Result<()> {
        self.send_unit(ControlRequestData::RewindFiles(RewindFilesRequest {
            user_message_id: user_message_id.to_string(),
            dry_run: None,
        }))
        .await
    }

    /// Report what rewinding to a user message would change, without
    /// changing anything.
    pub async fn rewind_files_dry_run(&self, user_message_id: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::RewindFiles(RewindFilesRequest {
            user_message_id: user_message_id.to_string(),
            dry_run: Some(true),
        }))
        .await
    }

    /// Merge settings into the flag settings layer (TS `applyFlagSettings`).
    pub async fn apply_flag_settings(&self, settings: Value) -> Result<Value> {
        self.send_control_request(ControlRequestData::ApplyFlagSettings(
            ApplyFlagSettingsRequest { settings },
        ))
        .await
    }

    /// Effective settings (TS `getSettings`).
    pub async fn get_settings(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetSettings)
            .await
    }

    /// Write settings to the user or local settings file. `source` is
    /// `userSettings` or `localSettings`.
    pub async fn update_settings(&self, source: &str, settings: Value) -> Result<Value> {
        self.send_control_request(ControlRequestData::UpdateSettings(UpdateSettingsRequest {
            source: source.to_string(),
            settings,
        }))
        .await
    }

    /// Replace dynamically managed MCP servers (TS `setMcpServers`).
    pub async fn set_mcp_servers(&self, servers: Value) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpSetServers(McpSetServersRequest {
            servers,
        }))
        .await
    }

    /// Stop a running task (TS `stopTask`).
    pub async fn stop_task(&self, task_id: &str) -> Result<()> {
        self.send_unit(ControlRequestData::StopTask(StopTaskRequest {
            task_id: task_id.to_string(),
        }))
        .await
    }

    /// Output of a background task (TS `getTaskOutput`).
    pub async fn get_task_output(&self, task_id: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetTaskOutput(GetTaskOutputRequest {
            task_id: task_id.to_string(),
        }))
        .await
    }

    /// Context-window usage by category (TS `getContextUsage`).
    pub async fn get_context_usage(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetContextUsage)
            .await
    }

    /// Background in-flight foreground tasks (TS `backgroundTasks`).
    pub async fn background_tasks(&self, tool_use_id: Option<&str>) -> Result<Value> {
        self.send_control_request(ControlRequestData::BackgroundTasks(
            BackgroundTasksRequest {
                tool_use_id: tool_use_id.map(str::to_string),
            },
        ))
        .await
    }

    /// Drop a queued async user message.
    pub async fn cancel_async_message(&self, message_uuid: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::CancelAsyncMessage(
            CancelAsyncMessageRequest {
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

    /// CLI binary version.
    pub async fn get_binary_version(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetBinaryVersion)
            .await
    }

    /// Models the session can switch to (TS `supportedModels` over the
    /// `list_models` subtype).
    pub async fn list_models(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::ListModels)
            .await
    }

    /// Permission rules in force (TS `listPermissionRules`).
    pub async fn list_permission_rules(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::ListPermissionRules)
            .await
    }

    /// Configured hooks (TS `getHooksListing`).
    pub async fn get_hooks_listing(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::GetHooksListing)
            .await
    }

    /// At-mention file autocomplete.
    pub async fn file_suggestions(&self, query: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::FileSuggestions(
            FileSuggestionsRequest {
                query: query.to_string(),
            },
        ))
        .await
    }

    /// Read a file through the CLI.
    pub async fn read_file(&self, request: ReadFileRequest) -> Result<Value> {
        self.send_control_request(ControlRequestData::ReadFile(request))
            .await
    }

    /// Seed the CLI's read-file state for a path.
    pub async fn seed_read_state(&self, path: &str, mtime: f64) -> Result<Value> {
        self.send_control_request(ControlRequestData::SeedReadState(SeedReadStateRequest {
            path: path.to_string(),
            mtime,
        }))
        .await
    }

    /// Register an additional repository root.
    pub async fn register_repo_root(&self, request: RegisterRepoRootRequest) -> Result<Value> {
        self.send_control_request(ControlRequestData::RegisterRepoRoot(request))
            .await
    }

    /// Call an MCP tool through the CLI.
    pub async fn mcp_call(&self, request: McpCallRequest) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpCall(request))
            .await
    }

    /// Read an MCP resource (TS `readMcpResource`).
    pub async fn read_mcp_resource(&self, server_name: &str, uri: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpReadResource(
            McpReadResourceRequest {
                server_name: server_name.to_string(),
                uri: uri.to_string(),
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

    /// Reload output styles.
    pub async fn reload_output_styles(&self) -> Result<Value> {
        self.send_control_request(ControlRequestData::ReloadOutputStyles)
            .await
    }

    /// Reconnect one MCP server.
    pub async fn reconnect_mcp_server(&self, server_name: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpReconnect(McpReconnectRequest {
            server_name: server_name.to_string(),
        }))
        .await
    }

    /// Enable or disable one MCP server.
    pub async fn toggle_mcp_server(&self, server_name: &str, enabled: bool) -> Result<Value> {
        self.send_control_request(ControlRequestData::McpToggle(McpToggleRequest {
            server_name: server_name.to_string(),
            enabled,
        }))
        .await
    }

    /// Set the session title.
    pub async fn rename_session(&self, title: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::RenameSession(RenameSessionRequest {
            title: title.to_string(),
        }))
        .await
    }

    /// Set the session color.
    pub async fn set_color(&self, color: &str) -> Result<Value> {
        self.send_control_request(ControlRequestData::SetColor(SetColorRequest {
            color: color.to_string(),
        }))
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

    /// Claude Code version of the process that runs the session's turns,
    /// from the `initialize` response (0.3.296). `None` on CLIs that
    /// predate the field.
    pub fn claude_code_version(&self) -> Option<&str> {
        self.initialization
            .as_ref()
            .and_then(|value| value.get("claude_code_version"))
            .and_then(Value::as_str)
    }

    /// Models advertised in the initialize handshake. [`Query::list_models`]
    /// asks the CLI again.
    pub fn supported_models(&self) -> Option<&Value> {
        self.initialization.as_ref().map(|value| &value["models"])
    }

    /// Check if the query has completed.
    pub fn is_completed(&self) -> bool {
        self.completed
    }

    /// Stop the CLI: its whole process group on Unix, then wait for it to
    /// exit. Dropping the query also stops it, without waiting.
    pub async fn kill(&self) -> Result<()> {
        self.transport.lock().await.kill().await
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        // The control handlers hold the transport too, so dropping the
        // query alone would leave the CLI running.
        if let Ok(mut transport) = self.transport.try_lock() {
            transport.kill_group();
        }
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
                if let Ok(ref msg) = result {
                    match msg {
                        SdkMessage::System(crate::protocol::SdkSystemMessage::Init(init)) => {
                            let id = init.session_id.clone();
                            if let Ok(mut shared) = self.input.session_id.lock() {
                                shared.clone_from(&id);
                            }
                            self.session_id = Some(id);
                        }
                        SdkMessage::Result(_) if !self.streaming => {
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
        // 0.3.295: a named option's value rides in its flag's argument.
        assert!(args.contains(&"--model=claude-sonnet-4-5-20250929".to_string()));
        assert!(args.contains(&"--max-turns=10".to_string()));
        assert!(args.contains(&"--max-budget-usd=1".to_string()));
        assert!(!args.contains(&"--model".to_string()));
    }

    #[test]
    fn build_args_emits_auto_mode_fallback_model_and_plugin_dir() {
        let mut options = QueryOptions::new().permission_mode(PermissionMode::Auto);
        options.fallback_model = Some("sonnet".to_string());
        options.plugins = vec![crate::options::PluginConfig::Local {
            path: "/tmp/plug".to_string(),
            skip_mcp_discovery: None,
        }];
        let args = options.build_args();
        assert!(args.contains(&"--permission-mode=auto".to_string()));
        assert!(args.contains(&"--fallback-model=sonnet".to_string()));
        assert!(args.contains(&"--plugin-dir=/tmp/plug".to_string()));
    }

    /// The value of `flag`, sent as `flag=value` or as `flag value`.
    fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        let joined = format!("{flag}=");
        args.iter().enumerate().find_map(|(i, arg)| {
            if let Some(value) = arg.strip_prefix(&joined) {
                Some(value)
            } else if arg == flag {
                args.get(i + 1).map(String::as_str)
            } else {
                None
            }
        })
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
                auto_compact_window: Some(60_000),
            },
        );
        options.sandbox = Some(crate::options::SandboxSettings {
            enabled: Some(true),
            auto_allow_bash_if_sandboxed: Some(true),
            network: Some(crate::options::SandboxNetworkConfig {
                allow_local_binding: Some(true),
                allow_unix_sockets: Some(vec!["/tmp/sock".into()]),
                ..Default::default()
            }),
            ..Default::default()
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
            request: ControlRequestData::Initialize(crate::protocol::InitializeRequest::default()),
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
      ? JSON.parse('{"commands":[{"name":"help","description":"help"}],"agents":[],"output_style":"default","available_output_styles":["default"],"models":[{"value":"sonnet","displayName":"Sonnet"}],"account":{"email":"t@example.com"},"claude_code_version":"2.1.296"}')
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
        assert_eq!(query.claude_code_version(), Some("2.1.296"));

        // Wait for the fake to log both lines; dropping the query kills it.
        let mut recorded = String::new();
        for _ in 0..100 {
            recorded = fs::read_to_string(&log).unwrap_or_default();
            if recorded.lines().filter(|line| !line.is_empty()).count() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        drop(query);

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

    /// A fake CLI that answers `initialize`, then writes the frames in
    /// `FAKE_FRAMES` (one JSON object per line). `$HOOK0` in a frame becomes
    /// the first PreToolUse callback ID from the initialize request. Every
    /// stdin line is logged to `FAKE_LOG`.
    const FAKE_SCRIPTED: &str = r#"#!/usr/bin/env node
const fs = require('fs');
const readline = require('readline');
const log = process.env.FAKE_LOG;
const frames = process.env.FAKE_FRAMES ? fs.readFileSync(process.env.FAKE_FRAMES, 'utf8') : '';
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
    const hooks = msg.request.hooks || {};
    const pre = (hooks.PreToolUse || [])[0];
    const hook0 = pre ? pre.hookCallbackIds[0] : 'none';
    for (const frame of frames.split('\n')) {
      if (frame.trim()) process.stdout.write(frame.split('$HOOK0').join(hook0) + '\n');
    }
  }
});
"#;

    /// Run the scripted fake with `frames` and return every stdin line it
    /// logged once `settle` has passed.
    async fn run_scripted(frames: &[Value], options: QueryOptions, settle: Duration) -> Vec<Value> {
        run_scripted_with(frames, options, settle, None).await
    }

    async fn run_scripted_with(
        frames: &[Value],
        mut options: QueryOptions,
        settle: Duration,
        permission_handler: Option<Arc<dyn PermissionHandler>>,
    ) -> Vec<Value> {
        let dir = unique_temp_dir();
        let fake = write_executable(&dir, "fake-claude", FAKE_SCRIPTED);
        let log = dir.join("stdin.jsonl");
        let frames_path = dir.join("frames.jsonl");
        let body: Vec<String> = frames.iter().map(Value::to_string).collect();
        fs::write(&frames_path, body.join("\n")).unwrap();
        options.executable = ExecutableConfig {
            path: Some(fake),
            executable: None,
            executable_args: Vec::new(),
        };
        options.control_timeout = Some(Duration::from_secs(5));
        options.env = Some(
            [
                ("FAKE_LOG".to_string(), log.display().to_string()),
                ("FAKE_FRAMES".to_string(), frames_path.display().to_string()),
            ]
            .into_iter()
            .collect(),
        );
        let query = Query::new("hello", options, permission_handler)
            .await
            .unwrap();
        tokio::time::sleep(settle).await;
        drop(query);
        tokio::time::sleep(Duration::from_millis(50)).await;
        fs::read_to_string(&log)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect()
    }

    fn reply_to<'a>(lines: &'a [Value], request_id: &str) -> Option<&'a Value> {
        lines.iter().find(|value| {
            value["type"] == "control_response" && value["response"]["request_id"] == request_id
        })
    }

    #[tokio::test]
    async fn hook_callback_runs_the_registered_host_closure() {
        let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ran_in_hook = ran.clone();
        let hook = crate::callbacks::hook_fn(move |input: Value, tool_use_id| {
            let ran = ran_in_hook.clone();
            async move {
                ran.store(true, Ordering::SeqCst);
                assert_eq!(input["hook_event_name"], "PreToolUse");
                assert_eq!(tool_use_id.as_deref(), Some("tu-1"));
                Ok(crate::protocol::SyncHookJSONOutput {
                    hook_specific_output: Some(serde_json::json!({
                        "hookEventName": "PreToolUse",
                        "permissionDecision": "deny",
                        "permissionDecisionReason": "no shell"
                    })),
                    ..Default::default()
                }
                .into())
            }
        });
        let options = QueryOptions::new().hook(
            crate::protocol::HookEvent::PreToolUse,
            crate::callbacks::HookMatcher::new(Some("Bash"), hook),
        );
        let frames = [serde_json::json!({
            "type": "control_request",
            "request_id": "cli-hook-1",
            "request": {
                "subtype": "hook_callback",
                "callback_id": "$HOOK0",
                "tool_use_id": "tu-1",
                "input": {"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "ls"}}
            }
        })];
        let lines = run_scripted(&frames, options, Duration::from_millis(200)).await;

        let init = &lines[0];
        assert_eq!(init["request"]["subtype"], "initialize");
        let matcher = &init["request"]["hooks"]["PreToolUse"][0];
        assert_eq!(matcher["matcher"], "Bash");
        assert_eq!(matcher["hookCallbackIds"][0], "hook_0");

        assert!(ran.load(Ordering::SeqCst), "the host hook must run");
        let reply = reply_to(&lines, "cli-hook-1").expect("hook_callback reply");
        assert_eq!(reply["response"]["subtype"], "success");
        assert_eq!(
            reply["response"]["response"]["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
    }

    fn add_server() -> SdkMcpServer {
        SdkMcpServer::new("calc", "1.0.0").timeout_ms(30_000).tool(
            "add",
            "Add two numbers",
            serde_json::json!({
                "type": "object",
                "properties": {"a": {"type": "number"}, "b": {"type": "number"}},
                "required": ["a", "b"]
            }),
            |args| async move {
                let sum =
                    args["a"].as_f64().unwrap_or_default() + args["b"].as_f64().unwrap_or_default();
                Ok(crate::mcp::ToolResult::text(sum.to_string()))
            },
        )
    }

    fn mcp_frame(request_id: &str, server: &str, message: Value) -> Value {
        serde_json::json!({
            "type": "control_request",
            "request_id": request_id,
            "request": {"subtype": "mcp_message", "server_name": server, "message": message}
        })
    }

    #[tokio::test]
    async fn sdk_mcp_server_answers_list_and_call_over_mcp_message() {
        let options = QueryOptions::new().sdk_mcp_server(add_server());
        assert!(
            flag_value(&options.build_args(), "--mcp-config").is_none(),
            "SDK servers go in initialize, not --mcp-config"
        );
        let frames = [
            mcp_frame(
                "cli-mcp-0",
                "calc",
                serde_json::json!({"jsonrpc": "2.0", "id": 0, "method": "initialize",
                    "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                               "clientInfo": {"name": "claude-code", "version": "2.1.296"}}}),
            ),
            mcp_frame(
                "cli-mcp-1",
                "calc",
                serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            ),
            mcp_frame(
                "cli-mcp-2",
                "calc",
                serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
            ),
            mcp_frame(
                "cli-mcp-3",
                "calc",
                serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": {"name": "add", "arguments": {"a": 2, "b": 3}}}),
            ),
            mcp_frame(
                "cli-mcp-4",
                "nope",
                serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}),
            ),
        ];
        let lines = run_scripted(&frames, options, Duration::from_millis(250)).await;

        let init = &lines[0]["request"];
        assert_eq!(init["subtype"], "initialize");
        assert_eq!(init["sdkMcpServers"], serde_json::json!(["calc"]));
        assert_eq!(init["sdkMcpServerConfigs"]["calc"]["timeout"], 30_000);
        assert!(init.get("sdkMcpServerManifests").is_none());

        let mcp = |id: &str| {
            let reply = reply_to(&lines, id).unwrap_or_else(|| panic!("no reply to {id}"));
            assert_eq!(reply["response"]["subtype"], "success", "{reply}");
            reply["response"]["response"]["mcp_response"].clone()
        };
        let initialized = mcp("cli-mcp-0");
        assert_eq!(initialized["id"], 0);
        assert_eq!(initialized["result"]["serverInfo"]["name"], "calc");
        assert_eq!(
            mcp("cli-mcp-1"),
            serde_json::json!({"jsonrpc": "2.0", "result": {}, "id": 0})
        );
        let listed = mcp("cli-mcp-2");
        assert_eq!(listed["id"], 1);
        assert_eq!(listed["result"]["tools"][0]["name"], "add");
        assert_eq!(
            listed["result"]["tools"][0]["inputSchema"]["required"][0],
            "a"
        );
        let called = mcp("cli-mcp-3");
        assert_eq!(called["id"], 2);
        assert_eq!(called["result"]["content"][0]["text"], "5");

        let missing = reply_to(&lines, "cli-mcp-4").expect("reply for an unknown server");
        assert_eq!(missing["response"]["subtype"], "error");
        assert!(
            missing["response"]["error"]
                .as_str()
                .unwrap()
                .contains("nope")
        );
    }

    #[tokio::test]
    async fn sdk_mcp_manifests_ride_in_initialize_when_asked() {
        let options = QueryOptions::new()
            .sdk_mcp_server(add_server())
            .sdk_mcp_manifests(true);
        let lines = run_scripted(&[], options, Duration::from_millis(50)).await;
        let manifest = &lines[0]["request"]["sdkMcpServerManifests"]["calc"];
        assert_eq!(manifest["initializeResult"]["serverInfo"]["name"], "calc");
        assert_eq!(manifest["toolsListResult"]["tools"][0]["name"], "add");
    }

    #[test]
    fn sdk_mcp_server_name_cannot_shadow_a_process_server() {
        let options = QueryOptions::new()
            .mcp_server(
                "calc",
                crate::options::McpServerConfig::Stdio {
                    command: "calc".into(),
                    args: None,
                    env: None,
                },
            )
            .sdk_mcp_server(add_server());
        assert!(matches!(
            options.validate(false),
            Err(Error::InvalidOptions(_))
        ));
    }

    #[tokio::test]
    async fn unknown_hook_callback_id_is_an_error_reply() {
        let frames = [serde_json::json!({
            "type": "control_request",
            "request_id": "cli-hook-2",
            "request": {"subtype": "hook_callback", "callback_id": "hook_99", "input": {}}
        })];
        let lines = run_scripted(&frames, QueryOptions::new(), Duration::from_millis(150)).await;
        let reply = reply_to(&lines, "cli-hook-2").expect("hook_callback reply");
        assert_eq!(reply["response"]["subtype"], "error");
        assert!(
            reply["response"]["error"]
                .as_str()
                .unwrap()
                .contains("hook_99")
        );
    }

    #[tokio::test]
    async fn elicitation_without_a_handler_is_declined() {
        let frames = [serde_json::json!({
            "type": "control_request",
            "request_id": "cli-elicit-1",
            "request": {
                "subtype": "elicitation",
                "mcp_server_name": "docs",
                "message": "Pick a branch",
                "mode": "form",
                "requested_schema": {"type": "object"}
            }
        })];
        let lines = run_scripted(&frames, QueryOptions::new(), Duration::from_millis(150)).await;
        let reply = reply_to(&lines, "cli-elicit-1").expect("elicitation reply");
        assert_eq!(reply["response"]["subtype"], "success");
        assert_eq!(
            reply["response"]["response"],
            serde_json::json!({"action": "decline"})
        );
    }

    struct AcceptElicitation;

    #[async_trait::async_trait]
    impl crate::callbacks::ElicitationHandler for AcceptElicitation {
        async fn elicit(
            &self,
            request: &crate::protocol::ElicitationRequest,
        ) -> Result<Option<crate::protocol::ElicitationResult>> {
            assert_eq!(request.mcp_server_name, "docs");
            Ok(Some(crate::protocol::ElicitationResult {
                action: crate::protocol::ElicitationAction::Accept,
                content: Some(serde_json::json!({"branch": "main"})),
            }))
        }
    }

    #[tokio::test]
    async fn elicitation_handler_answer_is_the_reply() {
        let mut options = QueryOptions::new();
        options.on_elicitation = Some(Arc::new(AcceptElicitation));
        let frames = [serde_json::json!({
            "type": "control_request",
            "request_id": "cli-elicit-2",
            "request": {"subtype": "elicitation", "mcp_server_name": "docs", "message": "Pick"}
        })];
        let lines = run_scripted(&frames, options, Duration::from_millis(150)).await;
        let reply = reply_to(&lines, "cli-elicit-2").expect("elicitation reply");
        assert_eq!(reply["response"]["response"]["action"], "accept");
        assert_eq!(reply["response"]["response"]["content"]["branch"], "main");
    }

    #[tokio::test]
    async fn user_dialog_without_a_handler_gets_no_reply_and_unsupported_gets_an_error() {
        let frames = [
            serde_json::json!({
                "type": "control_request",
                "request_id": "cli-dialog-1",
                "request": {"subtype": "request_user_dialog", "dialog_kind": "confirm", "payload": {}}
            }),
            serde_json::json!({
                "type": "control_request",
                "request_id": "cli-future-1",
                "request": {"subtype": "some_future_subtype", "x": 1}
            }),
            serde_json::json!({
                "type": "control_request",
                "request_id": "cli-remote-1",
                "request": {"subtype": "remote_tool_call"}
            }),
            serde_json::json!({
                "type": "control_request",
                "request_id": "cli-perm-1",
                "request": {"subtype": "can_use_tool", "tool_name": "Bash", "input": {}, "tool_use_id": "tu-9"}
            }),
        ];
        let lines = run_scripted(&frames, QueryOptions::new(), Duration::from_millis(200)).await;
        assert!(reply_to(&lines, "cli-dialog-1").is_none());
        assert!(reply_to(&lines, "cli-remote-1").is_none());
        let future = reply_to(&lines, "cli-future-1").expect("unsupported reply");
        assert_eq!(future["response"]["subtype"], "error");
        assert!(
            future["response"]["error"]
                .as_str()
                .unwrap()
                .contains("some_future_subtype")
        );
        // Without a permission handler the TS SDK refuses can_use_tool.
        let perm = reply_to(&lines, "cli-perm-1").expect("can_use_tool reply");
        assert_eq!(perm["response"]["subtype"], "error");
    }

    #[tokio::test]
    async fn control_cancel_request_stops_a_slow_hook() {
        let hook = crate::callbacks::hook_fn(|_input, _tool_use_id| async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok(crate::protocol::SyncHookJSONOutput::continue_execution().into())
        });
        let options = QueryOptions::new().hook(
            crate::protocol::HookEvent::PreToolUse,
            crate::callbacks::HookMatcher::new(None, hook),
        );
        let frames = [
            serde_json::json!({
                "type": "control_request",
                "request_id": "cli-slow-1",
                "request": {"subtype": "hook_callback", "callback_id": "$HOOK0", "input": {}}
            }),
            serde_json::json!({"type": "control_cancel_request", "request_id": "cli-slow-1"}),
        ];
        let started = std::time::Instant::now();
        let lines = run_scripted(&frames, options, Duration::from_millis(200)).await;
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(reply_to(&lines, "cli-slow-1").is_none());
    }

    #[test]
    fn initialize_request_carries_0_3_289_fields_in_camel_case() {
        let mut options = QueryOptions::new();
        options.supported_dialog_kinds = Some(vec!["confirm".into()]);
        options.per_task_stop_affordance = true;
        options.prompt_suggestions = true;
        options.title = Some("smoke".into());
        options.plugin_delivery = crate::options::PluginDelivery::Initialize;
        options.plugins = vec![crate::options::PluginConfig::Local {
            path: "/tmp/plug".into(),
            skip_mcp_discovery: Some(true),
        }];
        let request = ControlRequestData::Initialize(options.initialize_request(None));
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["subtype"], "initialize");
        assert_eq!(value["supportedDialogKinds"][0], "confirm");
        assert_eq!(value["perTaskStopAffordance"], true);
        assert_eq!(value["promptSuggestions"], true);
        assert_eq!(value["title"], "smoke");
        assert_eq!(value["plugins"][0]["type"], "local");
        assert_eq!(value["plugins"][0]["skipMcpDiscovery"], true);
        assert!(value.get("hooks").is_none());

        let args = options.build_args();
        assert!(args.contains(&"--await-initialize".to_string()));
        assert!(!args.iter().any(|a| a.starts_with("--plugin-dir")));
    }

    #[test]
    fn build_args_match_the_0_3_289_flag_spellings() {
        let mut options = QueryOptions::new().no_session_persistence();
        options.allowed_tools = Some(vec!["Read".into(), "Bash(git *)".into()]);
        options.disallowed_tools = Some(vec!["Write".into()]);
        options.setting_sources = Some(vec![
            crate::options::SettingSource::User,
            crate::options::SettingSource::Project,
        ]);
        options.betas = vec!["a".into(), "b".into()];
        options.allow_dangerously_skip_permissions = true;
        options.resume = Some("sess-1".into());
        options.resume_session_at = Some("msg-1".into());
        options.session_id = Some("sess-2".into());
        options.task_budget = Some(5000);
        options.agent = Some("reviewer".into());
        options.permission_prompts = Some(crate::options::PermissionPrompts::None);
        options.strict_mcp_config = true;
        options.include_hook_events = true;
        options.project_config_root = Some(PathBuf::from("/repo"));
        options.settings = Some(serde_json::json!({"model": "haiku"}));
        options.sandbox = Some(crate::options::SandboxSettings {
            enabled: Some(true),
            ..Default::default()
        });
        options.plugins = vec![crate::options::PluginConfig::Local {
            path: "/p".into(),
            skip_mcp_discovery: Some(true),
        }];
        options.enable_file_checkpointing = true;

        let args = options.build_args();
        assert_eq!(
            flag_value(&args, "--allowedTools"),
            Some("Read,Bash(git *)")
        );
        assert_eq!(flag_value(&args, "--disallowedTools"), Some("Write"));
        assert_eq!(flag_value(&args, "--betas"), Some("a,b"));
        assert_eq!(flag_value(&args, "--task-budget"), Some("5000"));
        assert_eq!(flag_value(&args, "--agent"), Some("reviewer"));
        assert_eq!(flag_value(&args, "--permission-prompts"), Some("none"));
        assert_eq!(flag_value(&args, "--plugin-dir-no-mcp"), Some("/p"));
        for flag in [
            "--setting-sources=user,project",
            "--allow-dangerously-skip-permissions",
            "--resume=sess-1",
            "--resume-session-at=msg-1",
            "--session-id=sess-2",
            "--no-session-persistence",
            "--strict-mcp-config",
            "--include-hook-events",
            "--project-config-root=/repo",
        ] {
            assert!(args.iter().any(|a| a == flag), "missing {flag}: {args:?}");
        }
        for gone in [
            "--no-persist-session",
            "--dangerously-skip-permissions",
            "--enable-file-checkpointing",
            "--permission-prompt-tool",
        ] {
            assert!(
                !args
                    .iter()
                    .any(|a| a == gone || a.starts_with(&format!("{gone}="))),
                "unexpected {gone}"
            );
        }
        let settings: Value =
            serde_json::from_str(flag_value(&args, "--settings").unwrap()).unwrap();
        assert_eq!(settings["model"], "haiku");
        assert_eq!(settings["sandbox"]["enabled"], true);
        assert_eq!(settings["sandbox"]["failIfUnavailable"], true);
        assert!(options.env_vars().contains(&(
            "CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING".into(),
            "true".into()
        )));

        let with_handler = options.build_args_for(true);
        assert_eq!(
            flag_value(&with_handler, "--permission-prompt-tool"),
            Some("stdio")
        );
    }

    #[test]
    fn validate_rejects_what_the_ts_sdk_rejects() {
        let mut options = QueryOptions::new().model("sonnet");
        options.fallback_model = Some("sonnet".into());
        assert!(matches!(
            options.validate(false),
            Err(Error::InvalidOptions(_))
        ));

        let mut options = QueryOptions::new();
        options.permission_prompt_tool_name = Some("mcp__perm__ask".into());
        assert!(options.validate(false).is_ok());
        assert!(matches!(
            options.validate(true),
            Err(Error::InvalidOptions(_))
        ));
    }

    #[test]
    fn new_0_3_289_control_subtypes_serialize_on_the_wire() {
        let cases = [
            (ControlRequestData::ListModels, "list_models"),
            (ControlRequestData::GetHooksListing, "get_hooks_listing"),
            (
                ControlRequestData::ListPermissionRules,
                "list_permission_rules",
            ),
            (
                ControlRequestData::ReloadOutputStyles,
                "reload_output_styles",
            ),
            (ControlRequestData::GetSettings, "get_settings"),
            (
                ControlRequestData::GetTaskOutput(GetTaskOutputRequest {
                    task_id: "t1".into(),
                }),
                "get_task_output",
            ),
            (
                ControlRequestData::McpReadResource(McpReadResourceRequest {
                    server_name: "docs".into(),
                    uri: "file:///a".into(),
                }),
                "mcp_read_resource",
            ),
            (
                ControlRequestData::UpdateSettings(UpdateSettingsRequest {
                    source: "localSettings".into(),
                    settings: serde_json::json!({}),
                }),
                "update_settings",
            ),
            (
                ControlRequestData::Interrupt(InterruptRequest {
                    cancel_queued: Some(true),
                }),
                "interrupt",
            ),
        ];
        for (request, subtype) in cases {
            let value = serde_json::to_value(&request).unwrap();
            assert_eq!(value["subtype"], subtype);
            assert_eq!(request.subtype().as_deref(), Some(subtype));
        }
        let read = serde_json::to_value(ControlRequestData::McpReadResource(
            McpReadResourceRequest {
                server_name: "docs".into(),
                uri: "u".into(),
            },
        ))
        .unwrap();
        assert_eq!(read["serverName"], "docs");
    }

    #[test]
    fn build_args_send_values_in_the_flag_argument_as_0_3_295_does() {
        let mut options = QueryOptions::new().model("-odd-model");
        options.system_prompt = Some(SystemPromptConfig::Custom("-starts with a dash".into()));
        options.tools = Some(crate::options::ToolsConfig::Names(Vec::new()));
        options.resume = Some("sess".into());
        options.managed_settings = Some("{\"a\":1}".into());
        options.settings = Some(Value::String("/etc/claude.json".into()));
        options
            .extra_args
            .insert("note".into(), Some("-dashed".into()));
        options.extra_args.insert("plain".into(), Some("x".into()));
        options.mcp_servers.insert(
            "docs".into(),
            crate::options::McpServerConfig::Http {
                url: "https://docs".into(),
                headers: None,
            },
        );
        let args = options.build_args();
        for joined in [
            "--model=-odd-model",
            "--system-prompt=-starts with a dash",
            "--tools=",
            "--resume=sess",
            "--note=-dashed",
        ] {
            assert!(
                args.iter().any(|a| a == joined),
                "missing {joined}: {args:?}"
            );
        }
        // The TS SDK keeps `--flag value` for these unless the value
        // starts with a dash.
        for (flag, value) in [
            ("--managed-settings", r#"{"a":1}"#),
            ("--settings", "/etc/claude.json"),
            ("--plain", "x"),
        ] {
            assert!(
                args.windows(2).any(|w| w[0] == flag && w[1] == value),
                "expected {flag} {value}: {args:?}"
            );
        }
        assert!(args.windows(2).any(|w| w[0] == "--mcp-config"));
        // The stream-json transport flags stay as they are.
        assert!(
            args.windows(2)
                .any(|w| w == ["--output-format", "stream-json"])
        );
    }

    #[test]
    fn agent_definition_carries_auto_compact_window() {
        let mut options = QueryOptions::new();
        options.agents.insert(
            "small".into(),
            crate::options::AgentDefinition {
                description: "d".into(),
                prompt: "p".into(),
                auto_compact_window: Some(50_000),
                ..Default::default()
            },
        );
        let args = options.build_args();
        let agents: Value = serde_json::from_str(flag_value(&args, "--agents").unwrap()).unwrap();
        assert_eq!(agents["small"]["autoCompactWindow"], 50_000);
        assert!(agents["small"].get("tools").is_none());
    }

    fn settings_of(options: &QueryOptions) -> Value {
        serde_json::from_str(flag_value(&options.build_args(), "--settings").expect("--settings"))
            .unwrap()
    }

    #[test]
    fn sandbox_option_merges_into_an_inline_settings_sandbox_block() {
        let mut options = QueryOptions::new();
        options.settings = Some(serde_json::json!({
            "model": "haiku",
            "sandbox": {
                "enabled": false,
                "autoAllowBashIfSandboxed": true,
                "failIfUnavailable": false,
                "filesystem": {"denyRead": ["/secret"], "disabled": true},
                "network": {"deniedDomains": ["a.com"], "httpProxyPort": 8080,
                            "socksProxyPort": 1080, "allowLocalBinding": true},
                "credentials": {"envVars": [{"name": "TOKEN", "mode": "mask"}]},
                "ripgrep": {"command": "rg", "args": ["--x"]}
            }
        }));
        let mut extra = serde_json::Map::new();
        extra.insert("ripgrep".into(), serde_json::json!({"command": "/bin/rg"}));
        options.sandbox = Some(crate::options::SandboxSettings {
            enabled: Some(true),
            filesystem: Some(serde_json::json!({"denyRead": ["/keys", "/secret"]})),
            network: Some(crate::options::SandboxNetworkConfig {
                denied_domains: Some(vec!["b.com".into()]),
                ..Default::default()
            }),
            credentials: Some(serde_json::json!({"envVars": [{"name": "KEY", "mode": "deny"}]})),
            extra,
            ..Default::default()
        });
        let settings = settings_of(&options);
        assert_eq!(settings["model"], "haiku");
        let sandbox = &settings["sandbox"];
        // The option's values win; values it does not set are kept.
        assert_eq!(sandbox["enabled"], true);
        assert_eq!(sandbox["autoAllowBashIfSandboxed"], true);
        // The settings block set failIfUnavailable, so it is not defaulted.
        assert_eq!(sandbox["failIfUnavailable"], false);
        // Restriction lists combine.
        assert_eq!(
            sandbox["filesystem"]["denyRead"],
            serde_json::json!(["/secret", "/keys"])
        );
        assert_eq!(
            sandbox["network"]["deniedDomains"],
            serde_json::json!(["a.com", "b.com"])
        );
        assert_eq!(
            sandbox["credentials"]["envVars"].as_array().unwrap().len(),
            2
        );
        // A filesystem restriction drops `disabled: true`; a domain
        // restriction drops the proxy ports the option did not set.
        assert!(sandbox["filesystem"].get("disabled").is_none());
        assert!(sandbox["network"].get("httpProxyPort").is_none());
        assert!(sandbox["network"].get("socksProxyPort").is_none());
        assert_eq!(sandbox["network"]["allowLocalBinding"], true);
        // ripgrep is replaced whole.
        assert_eq!(
            sandbox["ripgrep"],
            serde_json::json!({"command": "/bin/rg"})
        );
    }

    #[test]
    fn sandbox_option_merges_into_inline_json_settings_text() {
        let mut options = QueryOptions::new();
        options.settings = Some(Value::String(
            r#" {"sandbox": {"network": {"httpProxyPort": 9}}} "#.into(),
        ));
        options.sandbox = Some(crate::options::SandboxSettings {
            enabled: Some(true),
            ..Default::default()
        });
        assert!(options.validate(false).is_ok());
        let sandbox = settings_of(&options)["sandbox"].clone();
        assert_eq!(sandbox["enabled"], true);
        assert_eq!(sandbox["failIfUnavailable"], true);
        // No domain restriction, so the proxy port stays.
        assert_eq!(sandbox["network"]["httpProxyPort"], 9);

        options.settings = Some(Value::String("/etc/claude.json".into()));
        assert!(matches!(
            options.validate(false),
            Err(Error::InvalidOptions(_))
        ));
    }

    /// A fake CLI with streaming input: it answers control requests,
    /// writes `system/init` after `initialize`, answers every user message
    /// with a `result` whose text is `<pid>:<content as JSON>`, and on
    /// stdin EOF writes one more `result` ("closed", standing in for
    /// background work finishing) and exits.
    const FAKE_STREAMING: &str = r#"#!/usr/bin/env node
const fs = require('fs');
const readline = require('readline');
const log = process.env.FAKE_LOG;
const out = (v) => process.stdout.write(JSON.stringify(v) + '\n');
const result = (text) => out({
  type: 'result', subtype: 'success', duration_ms: 1, duration_api_ms: 1,
  is_error: false, num_turns: 1, result: text, total_cost_usd: 0,
  usage: { input_tokens: 1, output_tokens: 1 }, modelUsage: {},
  permission_denials: [], uuid: 'u', session_id: 'sess-stream'
});
const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
  if (!line) return;
  if (log) fs.appendFileSync(log, line + '\n');
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.type === 'control_request') {
    out({ type: 'control_response', response: { subtype: 'success', request_id: msg.request_id,
      response: { echo: msg.request.subtype } } });
    if (msg.request.subtype === 'initialize') {
      out({ type: 'system', subtype: 'init', apiKeySource: 'user', claude_code_version: '2.1.296',
        cwd: '/tmp', tools: [], mcp_servers: [], model: 'sonnet', permissionMode: 'default',
        slash_commands: [], output_style: 'default', skills: [], plugins: [],
        uuid: 'init-u', session_id: 'sess-stream' });
    }
  } else if (msg.type === 'user') {
    result(process.pid + ':' + JSON.stringify(msg.message.content));
  }
});
rl.on('close', () => { result('closed'); process.exit(0); });
"#;

    fn streaming_options(dir: &std::path::Path) -> (QueryOptions, PathBuf) {
        let fake = write_executable(dir, "fake-claude", FAKE_STREAMING);
        let log = dir.join("stdin.jsonl");
        let mut options = options_for_fake(fake, Duration::from_secs(5));
        options.env = Some(
            [("FAKE_LOG".to_string(), log.display().to_string())]
                .into_iter()
                .collect(),
        );
        (options, log)
    }

    /// Read the stream up to and including the next `result`; return its
    /// text.
    async fn next_result(query: &mut Query) -> String {
        use futures::StreamExt;
        let wait = async {
            while let Some(message) = query.next().await {
                if let SdkMessage::Result(crate::protocol::SdkResultMessage::Success(success)) =
                    message.unwrap()
                {
                    return success.result;
                }
            }
            panic!("stream ended before a result");
        };
        tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .expect("result in time")
    }

    #[tokio::test]
    async fn streaming_query_runs_many_turns_on_one_process_until_input_ends() {
        use futures::StreamExt;
        let dir = unique_temp_dir();
        let (options, log) = streaming_options(&dir);
        let mut query = Query::streaming(options, None).await.unwrap();
        assert!(query.is_streaming());

        query.send_user_message("one").await.unwrap();
        let first = next_result(&mut query).await;
        let (pid, content) = first.split_once(':').unwrap();
        assert_eq!(content, "\"one\"");
        assert!(
            !query.is_completed(),
            "a result ends the turn, not the stream"
        );
        assert_eq!(query.session_id(), Some("sess-stream"));

        // Content blocks, sent from a cloned handle on another task.
        let input = query.input();
        tokio::spawn(async move {
            input
                .send_user_content(vec![
                    crate::protocol::text_block("what is this?"),
                    crate::protocol::image_block("image/png", "iVBORw0KGgo="),
                ])
                .await
                .unwrap();
        });
        let second = next_result(&mut query).await;
        let (pid2, content) = second.split_once(':').unwrap();
        assert_eq!(pid2, pid, "every turn runs on the same CLI process");
        let blocks: Value = serde_json::from_str(content).unwrap();
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "image");
        assert_eq!(blocks[1]["source"]["media_type"], "image/png");
        assert_eq!(blocks[1]["source"]["data"], "iVBORw0KGgo=");

        // Control requests still work on a streaming query.
        query.interrupt().await.unwrap();

        query.end_input().await.unwrap();
        query.end_input().await.unwrap();
        assert!(query.input().is_input_closed().await);
        assert!(matches!(
            query.send_user_message("late").await,
            Err(Error::StdinWrite(_))
        ));
        // The CLI finishes its work after input closes, then the stream ends.
        assert_eq!(next_result(&mut query).await, "closed");
        let end = tokio::time::timeout(Duration::from_secs(5), query.next())
            .await
            .expect("stream end in time");
        assert!(end.is_none());
        assert!(query.is_completed());

        let lines: Vec<Value> = fs::read_to_string(&log)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let users: Vec<&Value> = lines.iter().filter(|v| v["type"] == "user").collect();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0]["message"]["role"], "user");
        assert_eq!(users[0]["parent_tool_use_id"], Value::Null);
        // Once system/init has named the session, messages carry its ID.
        assert_eq!(users[1]["session_id"], "sess-stream");
        assert_eq!(lines[0]["request"]["subtype"], "initialize");
    }

    #[tokio::test]
    async fn streaming_query_sends_verbatim_and_priority_fields() {
        let dir = unique_temp_dir();
        let (mut options, log) = streaming_options(&dir);
        options.verbatim_prompts = true;
        let mut query = Query::streaming(options, None).await.unwrap();
        query
            .send_message(
                SdkUserMessage::text("queued")
                    .with_uuid("m-1")
                    .with_priority(crate::protocol::UserMessagePriority::Next),
            )
            .await
            .unwrap();
        next_result(&mut query).await;
        let user: Value = fs::read_to_string(&log)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|v| v["type"] == "user")
            .unwrap();
        assert_eq!(user["client_composed"], true);
        assert_eq!(user["priority"], "next");
        assert_eq!(user["uuid"], "m-1");
        assert!(user.get("shouldQuery").is_none());
    }

    #[tokio::test]
    async fn a_prompt_query_still_ends_at_its_first_result() {
        use futures::StreamExt;
        let dir = unique_temp_dir();
        let (options, _log) = streaming_options(&dir);
        let mut query = Query::new("hi", options, None).await.unwrap();
        assert!(!query.is_streaming());
        assert!(next_result(&mut query).await.ends_with(":\"hi\""));
        assert!(query.is_completed());
        assert!(query.next().await.is_none());
    }

    #[tokio::test]
    async fn an_oversized_permission_answer_goes_out_as_a_deny() {
        let frames = vec![serde_json::json!({
            "type": "control_request",
            "request_id": "cli-perm-big",
            "request": {"subtype": "can_use_tool", "tool_name": "Bash", "input": {}, "tool_use_id": "tu-big"}
        })];
        let handler = crate::permissions::permission_handler(|_request| async {
            Ok::<_, Error>(crate::protocol::PermissionResult::Allow {
                updated_input: serde_json::json!({}),
                updated_permissions: Some(vec![
                    crate::protocol::PermissionUpdate::AddDirectories {
                        directories: (0..crate::protocol::MAX_UPDATED_PERMISSIONS)
                            .map(|i| format!("/d{i}"))
                            .collect(),
                        destination: "session".into(),
                    },
                ]),
                tool_use_id: None,
                decision_classification: None,
            })
        });
        let lines = run_scripted_with(
            &frames,
            QueryOptions::new(),
            Duration::from_millis(400),
            Some(handler),
        )
        .await;
        let reply = reply_to(&lines, "cli-perm-big").expect("can_use_tool reply");
        assert_eq!(reply["response"]["response"]["behavior"], "deny");
        assert!(
            reply["response"]["response"]["message"]
                .as_str()
                .unwrap()
                .contains("4097")
        );
    }
}
