//! Claude tunnel client for Pylon.
//!
//! Connects to the Cloudflare relay WebSocket and serves Claude Agent SDK sessions
//! on the user's machine. Messages are relayed via the openagents-relay protocol.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use claude_agent_sdk::permissions::{CallbackPermissionHandler, PermissionRequest};
use claude_agent_sdk::protocol::{PermissionResult, SdkMessage, SdkResultMessage, SdkSystemMessage};
use claude_agent_sdk::{ExecutableConfig, Query, QueryOptions, ToolsConfig};
use futures_util::{SinkExt, StreamExt};
use openagents_relay::{
    ClaudeChunk, ClaudeRequest, ClaudeSessionAutonomy, ClaudeUsage, ChunkType, RelayMessage,
    ToolChunk,
};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;

#[derive(Clone, Debug)]
pub struct ClaudeTunnelConfig {
    pub model: String,
    pub autonomy: ClaudeSessionAutonomy,
    pub approval_required_tools: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub blocked_tools: Vec<String>,
    pub max_cost_usd: Option<u64>,
    pub cwd: Option<PathBuf>,
    pub executable_path: Option<PathBuf>,
}

#[derive(Clone)]
struct SessionHandle {
    prompt_tx: mpsc::Sender<String>,
    control_tx: mpsc::Sender<ControlCommand>,
    pending_approval: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
}

#[derive(Clone, Copy)]
enum ControlCommand {
    Stop,
    Pause,
    Resume,
}

#[derive(Clone)]
struct ToolPolicy {
    autonomy: ClaudeSessionAutonomy,
    approval_required: Vec<String>,
    allowed: Vec<String>,
    blocked: Vec<String>,
}

pub async fn run_tunnel_client(
    tunnel_url: String,
    config: ClaudeTunnelConfig,
) -> anyhow::Result<()> {
    tracing::info!("Connecting Claude tunnel client to {tunnel_url}");

    let (ws_stream, _) =
        tokio_tungstenite::connect_async(&tunnel_url).await.context("WebSocket connect failed")?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<RelayMessage>(256);
    let sessions: Arc<RwLock<HashMap<String, SessionHandle>>> = Arc::new(RwLock::new(HashMap::new()));

    let writer = tokio::spawn(async move {
        while let Some(msg) = outgoing_rx.recv().await {
            let payload = msg.to_json();
            if ws_write.send(WsMessage::Text(payload)).await.is_err() {
                break;
            }
        }
    });

    let version = env!("CARGO_PKG_VERSION").to_string();
    let _ = outgoing_tx
        .send(RelayMessage::TunnelConnected {
            version,
            capabilities: vec!["claude".to_string()],
        })
        .await;

    while let Some(msg) = ws_read.next().await {
        let msg = match msg {
            Ok(WsMessage::Text(text)) => RelayMessage::from_json(&text).ok(),
            Ok(WsMessage::Binary(bin)) => {
                RelayMessage::from_json(&String::from_utf8_lossy(&bin)).ok()
            }
            _ => None,
        };

        let Some(msg) = msg else { continue; };

        match msg {
            RelayMessage::Ping { timestamp } => {
                let _ = outgoing_tx.send(RelayMessage::Pong { timestamp }).await;
            }
            RelayMessage::ClaudeCreateSession { session_id, request } => {
                let handle = spawn_claude_session(
                    session_id.clone(),
                    request,
                    config.clone(),
                    outgoing_tx.clone(),
                )
                .await;

                match handle {
                    Ok(handle) => {
                        sessions.write().await.insert(session_id, handle);
                    }
                    Err(err) => {
                        let _ = outgoing_tx
                            .send(RelayMessage::ClaudeError {
                                session_id,
                                error: err.to_string(),
                            })
                            .await;
                    }
                }
            }
            RelayMessage::ClaudePrompt { session_id, content } => {
                if let Some(handle) = sessions.read().await.get(&session_id) {
                    let _ = handle.prompt_tx.send(content).await;
                }
            }
            RelayMessage::ClaudeToolApprovalResponse { session_id, approved } => {
                if let Some(handle) = sessions.read().await.get(&session_id) {
                    if let Some(tx) = handle
                        .pending_approval
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take()
                    {
                        let _ = tx.send(approved);
                    }
                }
            }
            RelayMessage::ClaudeStop { session_id } => {
                if let Some(handle) = sessions.read().await.get(&session_id) {
                    let _ = handle.control_tx.send(ControlCommand::Stop).await;
                }
            }
            RelayMessage::ClaudePause { session_id } => {
                if let Some(handle) = sessions.read().await.get(&session_id) {
                    let _ = handle.control_tx.send(ControlCommand::Pause).await;
                }
            }
            RelayMessage::ClaudeResume { session_id } => {
                if let Some(handle) = sessions.read().await.get(&session_id) {
                    let _ = handle.control_tx.send(ControlCommand::Resume).await;
                }
            }
            _ => {}
        }
    }

    writer.abort();
    Ok(())
}

async fn spawn_claude_session(
    session_id: String,
    request: ClaudeRequest,
    config: ClaudeTunnelConfig,
    outgoing_tx: mpsc::Sender<RelayMessage>,
) -> anyhow::Result<SessionHandle> {
    let (prompt_tx, prompt_rx) = mpsc::channel(128);
    let (control_tx, mut control_rx) = mpsc::channel(32);
    let pending_approval: Arc<Mutex<Option<oneshot::Sender<bool>>>> = Arc::new(Mutex::new(None));

    let policy = ToolPolicy {
        autonomy: request
            .autonomy
            .clone()
            .unwrap_or_else(|| config.autonomy.clone()),
        approval_required: request
            .approval_required_tools
            .clone()
            .unwrap_or_else(|| config.approval_required_tools.clone()),
        allowed: config.allowed_tools.clone(),
        blocked: config.blocked_tools.clone(),
    };

    let handler = build_permission_handler(
        session_id.clone(),
        policy,
        pending_approval.clone(),
        outgoing_tx.clone(),
    );

    let mut options = QueryOptions::new();
    options.model = Some(if request.model.is_empty() {
        config.model.clone()
    } else {
        request.model.clone()
    });
    if let Some(system_prompt) = request.system_prompt.clone() {
        options.system_prompt = Some(claude_agent_sdk::options::SystemPromptConfig::Custom(
            system_prompt,
        ));
    }
    if let Some(max_cost) = request.max_cost_usd.or(config.max_cost_usd) {
        options.max_budget_usd = Some(max_cost as f64 / 1_000_000.0);
    }
    if !request.tools.is_empty() {
        let tools: Vec<String> = request.tools.iter().map(|t| t.name.clone()).collect();
        options.tools = Some(ToolsConfig::list(tools));
    }
    if !config.allowed_tools.is_empty() {
        options.allowed_tools = Some(config.allowed_tools.clone());
    }
    if !config.blocked_tools.is_empty() {
        options.disallowed_tools = Some(config.blocked_tools.clone());
    }
    options.include_partial_messages = true;
    options.cwd = config.cwd.clone();
    if let Some(path) = config.executable_path.clone() {
        options.executable = ExecutableConfig {
            path: Some(path),
            ..ExecutableConfig::default()
        };
    }

    let initial_prompt = request.initial_prompt.clone().unwrap_or_default();
    let mut query = Query::new(initial_prompt, options, Some(handler))
        .await
        .context("Failed to start Claude session")?;

    let _ = query
        .stream_input(ReceiverStream::new(prompt_rx))
        .await
        .context("Failed to stream input")?;

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(cmd) = control_rx.recv() => {
                    match cmd {
                        ControlCommand::Stop => {
                            let _ = query.interrupt().await;
                            let _ = outgoing_tx
                                .send(RelayMessage::ClaudeError { session_id: session_id.clone(), error: "stopped".to_string() })
                                .await;
                            break;
                        }
                        ControlCommand::Pause => {
                            let _ = query.interrupt().await;
                        }
                        ControlCommand::Resume => {}
                    }
                }
                msg = query.next() => {
                    let Some(msg) = msg else { break; };
                    match msg {
                        Ok(SdkMessage::System(SdkSystemMessage::Init(_init))) => {
                            let _ = outgoing_tx.send(RelayMessage::ClaudeSessionCreated { session_id: session_id.clone() }).await;
                        }
                        Ok(SdkMessage::StreamEvent(event)) => {
                            if let Some(delta) = extract_delta_from_event(&event.event) {
                                let chunk = ClaudeChunk {
                                    session_id: session_id.clone(),
                                    chunk_type: ChunkType::Text,
                                    delta: Some(delta),
                                    tool: None,
                                    usage: None,
                                };
                                let _ = outgoing_tx.send(RelayMessage::ClaudeChunk { chunk }).await;
                            }
                        }
                        Ok(SdkMessage::Assistant(assistant)) => {
                            if let Some(text) = extract_text_from_value(&assistant.message) {
                                let chunk = ClaudeChunk {
                                    session_id: session_id.clone(),
                                    chunk_type: ChunkType::Text,
                                    delta: Some(text),
                                    tool: None,
                                    usage: None,
                                };
                                let _ = outgoing_tx.send(RelayMessage::ClaudeChunk { chunk }).await;
                            }
                        }
                        Ok(SdkMessage::ToolProgress(progress)) => {
                            let chunk = ClaudeChunk {
                                session_id: session_id.clone(),
                                chunk_type: ChunkType::ToolStart,
                                delta: None,
                                tool: Some(ToolChunk { name: progress.tool_name.clone(), params: None, result: None, error: None }),
                                usage: None,
                            };
                            let _ = outgoing_tx.send(RelayMessage::ClaudeChunk { chunk }).await;
                        }
                        Ok(SdkMessage::Result(result)) => {
                            let (usage, cost) = map_result_usage(&result);
                            let (chunk_type, delta) = match &result {
                                SdkResultMessage::Success(success) => {
                                    if success.is_error {
                                        (ChunkType::Error, Some("execution error".to_string()))
                                    } else {
                                        (ChunkType::Done, Some(success.result.clone()))
                                    }
                                }
                                SdkResultMessage::ErrorDuringExecution(err)
                                | SdkResultMessage::ErrorMaxTurns(err)
                                | SdkResultMessage::ErrorMaxBudget(err)
                                | SdkResultMessage::ErrorMaxStructuredOutputRetries(err) => {
                                    let message = if err.errors.is_empty() {
                                        "execution error".to_string()
                                    } else {
                                        err.errors.join("; ")
                                    };
                                    (ChunkType::Error, Some(message))
                                }
                            };

                            let chunk = ClaudeChunk {
                                session_id: session_id.clone(),
                                chunk_type,
                                delta,
                                tool: None,
                                usage: usage.clone(),
                            };
                            let _ = outgoing_tx.send(RelayMessage::ClaudeChunk { chunk }).await;

                            if cost.is_some() && matches!(chunk_type, ChunkType::Error) {
                                let _ = outgoing_tx
                                    .send(RelayMessage::ClaudeError {
                                        session_id: session_id.clone(),
                                        error: "session failed".to_string(),
                                    })
                                    .await;
                            }

                            break;
                        }
                        Ok(_) => {}
                        Err(err) => {
                            let _ = outgoing_tx
                                .send(RelayMessage::ClaudeError {
                                    session_id: session_id.clone(),
                                    error: err.to_string(),
                                })
                                .await;
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(SessionHandle {
        prompt_tx,
        control_tx,
        pending_approval,
    })
}

fn build_permission_handler(
    session_id: String,
    policy: ToolPolicy,
    pending_approval: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
    outgoing_tx: mpsc::Sender<RelayMessage>,
) -> Arc<dyn claude_agent_sdk::permissions::PermissionHandler> {
    let handler = move |request: PermissionRequest| {
        let policy = policy.clone();
        let pending_approval = pending_approval.clone();
        let outgoing_tx = outgoing_tx.clone();
        let session_id = session_id.clone();
        async move {
            let tool_name = request.tool_name.clone();
            let params = request.input.clone();

            if !policy.allowed.is_empty() && !policy.allowed.iter().any(|t| t == &tool_name) {
                return Ok(PermissionResult::deny("tool not allowed"));
            }

            if policy.blocked.iter().any(|t| t == &tool_name) {
                return Ok(PermissionResult::deny("tool blocked"));
            }

            if policy.autonomy == ClaudeSessionAutonomy::ReadOnly {
                return Ok(PermissionResult::deny("read-only"));
            }

            let requires_approval = matches!(policy.autonomy, ClaudeSessionAutonomy::Restricted)
                || (policy.autonomy == ClaudeSessionAutonomy::Supervised
                    && policy.approval_required.iter().any(|t| t == &tool_name));

            if requires_approval {
                let (tx, rx) = oneshot::channel();
                {
                    let mut guard = pending_approval.lock().unwrap_or_else(|e| e.into_inner());
                    if guard.is_some() {
                        return Ok(PermissionResult::deny("approval already pending"));
                    }
                    *guard = Some(tx);
                }

                let _ = outgoing_tx
                    .send(RelayMessage::ClaudeToolApproval {
                        session_id,
                        tool: tool_name.clone(),
                        params: params.clone(),
                    })
                    .await;

                let approved = rx.await.unwrap_or(false);
                if approved {
                    Ok(PermissionResult::allow(request.input.clone()))
                } else {
                    Ok(PermissionResult::deny_and_interrupt("tool denied"))
                }
            } else {
                Ok(PermissionResult::allow(request.input.clone()))
            }
        }
    };

    Arc::new(CallbackPermissionHandler::new(handler))
}

fn usd_to_microusd(cost: f64) -> u64 {
    if cost <= 0.0 {
        return 0;
    }
    (cost * 1_000_000.0).round() as u64
}

fn map_result_usage(result: &SdkResultMessage) -> (Option<ClaudeUsage>, Option<u64>) {
    match result {
        SdkResultMessage::Success(success) => {
            let usage = ClaudeUsage {
                input_tokens: success.usage.input_tokens,
                output_tokens: success.usage.output_tokens,
                cache_read_tokens: success.usage.cache_read_input_tokens.unwrap_or(0),
                cache_write_tokens: success.usage.cache_creation_input_tokens.unwrap_or(0),
                total_tokens: success.usage.input_tokens + success.usage.output_tokens,
            };
            (Some(usage), Some(usd_to_microusd(success.total_cost_usd)))
        }
        SdkResultMessage::ErrorDuringExecution(err)
        | SdkResultMessage::ErrorMaxTurns(err)
        | SdkResultMessage::ErrorMaxBudget(err)
        | SdkResultMessage::ErrorMaxStructuredOutputRetries(err) => {
            let usage = ClaudeUsage {
                input_tokens: err.usage.input_tokens,
                output_tokens: err.usage.output_tokens,
                cache_read_tokens: err.usage.cache_read_input_tokens.unwrap_or(0),
                cache_write_tokens: err.usage.cache_creation_input_tokens.unwrap_or(0),
                total_tokens: err.usage.input_tokens + err.usage.output_tokens,
            };
            (Some(usage), Some(usd_to_microusd(err.total_cost_usd)))
        }
    }
}

fn extract_text_from_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Object(map) => {
            if let Some(content) = map.get("content") {
                if let Some(text) = extract_text_from_value(content) {
                    return Some(text);
                }
            }
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
            None
        }
        serde_json::Value::Array(items) => {
            let mut out = String::new();
            for item in items {
                if let Some(text) = extract_text_from_value(item) {
                    out.push_str(&text);
                }
            }
            if out.is_empty() { None } else { Some(out) }
        }
        _ => None,
    }
}

fn extract_delta_from_event(event: &serde_json::Value) -> Option<String> {
    if let Some(delta) = event.get("delta") {
        if let Some(text) = extract_text_from_value(delta) {
            return Some(text);
        }
    }
    if let Some(text) = extract_text_from_value(event) {
        return Some(text);
    }
    None
}
