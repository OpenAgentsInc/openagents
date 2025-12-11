//! Query struct for executing prompts and streaming responses.

use crate::error::{Error, Result};
use crate::options::QueryOptions;
use crate::permissions::PermissionHandler;
use crate::protocol::{
    AccountInfo, ControlRequestData, ControlRequestType, ControlResponseData, ControlResponseType,
    ModelInfo, PermissionMode, PermissionResult, SdkControlRequest, SdkControlResponse, SdkMessage,
    SdkUserMessageOutgoing, SetMaxThinkingTokensRequest, SetModelRequest, SetPermissionModeRequest,
    SlashCommand, StdinMessage, StdoutMessage, UserMessageType,
};
use crate::transport::ProcessTransport;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::{mpsc, oneshot, Mutex};
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
        eprintln!("[SDK::Query] new() called");
        let prompt = prompt.into();
        let args = options.build_args();
        eprintln!("[SDK::Query] args built: {:?}", args);

        let env = options.env.clone().map(|e| e.into_iter().collect());

        eprintln!("[SDK::Query] calling ProcessTransport::spawn()...");
        let transport =
            ProcessTransport::spawn(options.executable.clone(), args, options.cwd.clone(), env)
                .await?;
        eprintln!("[SDK::Query] ProcessTransport::spawn() returned");

        let transport = Arc::new(Mutex::new(transport));
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));

        // Create message channel
        let (message_tx, message_rx) = mpsc::channel(256);

        // Spawn message processing task
        let transport_clone = transport.clone();
        let pending_clone = pending_requests.clone();
        let handler_clone = permission_handler.clone();

        eprintln!("[SDK::Query] spawning message processing task");
        tokio::spawn(async move {
            Self::process_messages(transport_clone, pending_clone, handler_clone, message_tx).await;
        });

        let mut query = Self {
            transport,
            pending_requests,
            request_counter: AtomicU64::new(0),
            permission_handler,
            message_rx,
            session_id: None,
            completed: false,
        };

        // Send initial prompt
        eprintln!("[SDK::Query] sending initial prompt...");
        query.send_prompt(&prompt).await?;
        eprintln!("[SDK::Query] initial prompt sent, returning Query");

        Ok(query)
    }

    /// Send a prompt to the CLI.
    async fn send_prompt(&mut self, prompt: &str) -> Result<()> {
        let session_id = self.session_id.clone().unwrap_or_default();

        let message = SdkUserMessageOutgoing {
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
        pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value>>>>>,
        permission_handler: Option<Arc<dyn PermissionHandler>>,
        message_tx: mpsc::Sender<Result<SdkMessage>>,
    ) {
        loop {
            let msg = {
                let mut transport = transport.lock().await;
                transport.recv().await
            };

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
                            Self::handle_control_request(
                                &transport,
                                &permission_handler,
                                req,
                            )
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
                    let _ = message_tx.send(Err(e)).await;
                    break;
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
            ControlRequestData::HookCallback(ref _hook_req) => {
                // TODO: Implement hook callbacks
                SdkControlResponse {
                    msg_type: ControlResponseType::ControlResponse,
                    response: ControlResponseData::Success {
                        request_id: request.request_id.clone(),
                        response: Some(serde_json::json!({ "continue": true })),
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
        let request_id = format!("sdk-{}", self.request_counter.fetch_add(1, Ordering::SeqCst));

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

        // Wait for response
        rx.await
            .map_err(|_| Error::ControlTimeout)?
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

    /// Get available slash commands.
    ///
    /// Returns a list of slash commands that can be used in prompts.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use claude_agent_sdk::{query, QueryOptions};
    /// # async fn example() -> Result<(), claude_agent_sdk::Error> {
    /// let stream = query("hello", QueryOptions::new()).await?;
    /// let commands = stream.supported_commands().await?;
    /// for cmd in commands {
    ///     println!("/{} - {} ({})", cmd.name, cmd.description, cmd.argument_hint);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn supported_commands(&self) -> Result<Vec<SlashCommand>> {
        let response = self
            .send_control_request(ControlRequestData::SupportedCommands)
            .await?;

        // Parse the response - should be an array of SlashCommand
        let commands: Vec<SlashCommand> = serde_json::from_value(response).map_err(|e| {
            Error::InvalidMessage(format!("Failed to parse supported commands: {}", e))
        })?;

        Ok(commands)
    }

    /// Get available models.
    ///
    /// Returns a list of models that can be used for queries.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use claude_agent_sdk::{query, QueryOptions};
    /// # async fn example() -> Result<(), claude_agent_sdk::Error> {
    /// let stream = query("hello", QueryOptions::new()).await?;
    /// let models = stream.supported_models().await?;
    /// for model in models {
    ///     println!("{}: {} - {}", model.value, model.display_name, model.description);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn supported_models(&self) -> Result<Vec<ModelInfo>> {
        let response = self
            .send_control_request(ControlRequestData::SupportedModels)
            .await?;

        // Parse the response - should be an array of ModelInfo
        let models: Vec<ModelInfo> = serde_json::from_value(response).map_err(|e| {
            Error::InvalidMessage(format!("Failed to parse supported models: {}", e))
        })?;

        Ok(models)
    }

    /// Get account information for the authenticated user.
    ///
    /// Returns details about the current authentication, including email,
    /// organization, and subscription type.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use claude_agent_sdk::{query, QueryOptions};
    /// # async fn example() -> Result<(), claude_agent_sdk::Error> {
    /// let stream = query("hello", QueryOptions::new()).await?;
    /// let account = stream.account_info().await?;
    /// if let Some(email) = account.email {
    ///     println!("Logged in as: {}", email);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn account_info(&self) -> Result<AccountInfo> {
        let response = self
            .send_control_request(ControlRequestData::AccountInfo)
            .await?;

        // Parse the response - should be an AccountInfo object
        let account: AccountInfo = serde_json::from_value(response).map_err(|e| {
            Error::InvalidMessage(format!("Failed to parse account info: {}", e))
        })?;

        Ok(account)
    }

    /// Stream additional user messages into the query.
    ///
    /// This allows sending multiple messages as a stream during an ongoing conversation.
    /// Messages will be forwarded to the CLI as they arrive from the stream.
    ///
    /// # Arguments
    /// * `stream` - A stream of user messages to send
    ///
    /// # Example
    /// ```rust,no_run
    /// # use claude_agent_sdk::{query, QueryOptions};
    /// # use futures::stream;
    /// # async fn example() -> Result<(), claude_agent_sdk::Error> {
    /// let mut query_stream = query("Start a task", QueryOptions::new()).await?;
    ///
    /// // Stream additional messages
    /// let messages = stream::iter(vec![
    ///     "Continue with step 2",
    ///     "And then step 3",
    /// ].into_iter().map(|s| s.to_string()));
    ///
    /// query_stream.stream_input(messages).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stream_input<S>(&self, mut stream: S) -> Result<()>
    where
        S: futures::Stream<Item = String> + Send + Unpin + 'static,
    {
        use futures::StreamExt;

        let transport = self.transport.clone();
        let session_id = self.session_id.clone().unwrap_or_default();

        // Spawn task to forward messages from the stream
        tokio::spawn(async move {
            while let Some(content) = stream.next().await {
                let message = SdkUserMessageOutgoing {
                    msg_type: UserMessageType::User,
                    message: serde_json::json!({
                        "role": "user",
                        "content": content
                    }),
                    parent_tool_use_id: None,
                    is_synthetic: None,
                    tool_use_result: None,
                    uuid: None,
                    session_id: session_id.clone(),
                    is_replay: None,
                };

                let mut transport = transport.lock().await;
                if transport.send(&StdinMessage::UserMessage(message)).await.is_err() {
                    break;
                }
            }
        });

        Ok(())
    }

    /// Send a single user message to the query.
    ///
    /// This is used internally by the Session API.
    pub(crate) async fn send_message(&self, content: impl Into<String>) -> Result<()> {
        let content = content.into();
        let session_id = self.session_id.clone().unwrap_or_default();

        let message = SdkUserMessageOutgoing {
            msg_type: UserMessageType::User,
            message: serde_json::json!({
                "role": "user",
                "content": content
            }),
            parent_tool_use_id: None,
            is_synthetic: None,
            tool_use_result: None,
            uuid: None,
            session_id,
            is_replay: None,
        };

        let mut transport = self.transport.lock().await;
        transport
            .send(&StdinMessage::UserMessage(message))
            .await?;
        Ok(())
    }

    /// Get the session ID (available after receiving first message).
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
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
}
