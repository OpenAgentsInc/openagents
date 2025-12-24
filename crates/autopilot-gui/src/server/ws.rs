//! WebSocket handler for real-time message streaming

use crate::server::state::AppState;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::{Message as WsMessage, Session};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

/// WebSocket upgrade endpoint
pub async fn websocket(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    // Subscribe to broadcast channel
    let mut broadcast_rx = state.ws_tx.subscribe();

    // Spawn WebSocket handler
    actix_web::rt::spawn(async move {
        info!("WebSocket connection established");

        loop {
            tokio::select! {
                // Handle incoming WebSocket messages from client
                msg = msg_stream.next() => {
                    match msg {
                        Some(Ok(WsMessage::Text(text))) => {
                            debug!("Received message: {}", text);

                            // Parse incoming message
                            match serde_json::from_str::<ClientMessage>(&text) {
                                Ok(client_msg) => {
                                    handle_client_message(client_msg, &mut session).await;
                                }
                                Err(e) => {
                                    error!("Failed to parse message: {}", e);
                                }
                            }
                        }
                        Some(Ok(WsMessage::Ping(msg))) => {
                            let _ = session.pong(&msg).await;
                        }
                        Some(Ok(WsMessage::Close(reason))) => {
                            info!("WebSocket closed: {:?}", reason);
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            break;
                        }
                        _ => {}
                    }
                }
                // Handle broadcast messages from state
                broadcast_msg = broadcast_rx.recv() => {
                    match broadcast_msg {
                        Ok(msg) => {
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if let Err(e) = session.text(json).await {
                                    error!("Failed to send broadcast: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Broadcast channel error: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        info!("WebSocket connection closed");
    });

    Ok(response)
}

/// Client -> Server message types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "prompt")]
    Prompt { text: String },

    #[serde(rename = "abort")]
    Abort,

    #[serde(rename = "permission_response")]
    PermissionResponse {
        request_id: String,
        action: String,
        pattern: Option<String>,
        persistent: bool,
    },
}

/// Server -> Client message types
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum ServerMessage {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: String,
    },

    #[serde(rename = "status")]
    Status {
        status: String,
    },

    #[serde(rename = "error")]
    Error {
        message: String,
    },

    #[serde(rename = "tool_call")]
    ToolCall {
        tool: String,
        input: serde_json::Value,
        status: String,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool: String,
        output: String,
        elapsed_ms: Option<u64>,
    },

    #[serde(rename = "permission_request")]
    PermissionRequest {
        id: String,
        tool: String,
        input: serde_json::Value,
        description: Option<String>,
        timestamp: String,
    },

    #[serde(rename = "session_started")]
    SessionStarted {
        session_id: String,
        timestamp: String,
        model: String,
        prompt: String,
    },

    #[serde(rename = "session_updated")]
    SessionUpdated {
        session_id: String,
        tokens_in: i64,
        tokens_out: i64,
        tool_calls: i64,
        tool_errors: i64,
    },

    #[serde(rename = "session_completed")]
    SessionCompleted {
        session_id: String,
        duration_seconds: f64,
        final_status: String,
        issues_completed: i64,
        cost_usd: f64,
    },

    #[serde(rename = "stats_updated")]
    StatsUpdated {
        sessions_today: i64,
        success_rate: f64,
        total_tokens: i64,
        total_cost: f64,
        avg_duration: f64,
    },

    #[serde(rename = "apm_updated")]
    APMUpdated {
        avg_apm: f64,
        session_apm: Option<f64>,
    },
}

async fn handle_client_message(msg: ClientMessage, session: &mut Session) {
    match msg {
        ClientMessage::Prompt { text } => {
            debug!("Prompt received (not implemented): {}", text);

            // === BLOCKED: Claude Agent SDK integration required (d-009) ===
            // This requires bidirectional communication between the GUI WebSocket
            // and the Claude Agent SDK session. Currently there's no integration.
            //
            // When implementing:
            // 1. Create channel between WebSocket handler and SDK session
            // 2. Forward prompt to SDK via channel
            // 3. Stream back SDK responses (tool calls, results, messages)
            // 4. Handle session lifecycle (start, continue, abort)
            //
            // See d-009 Phase 3 for full specification.

            let response = ServerMessage::Error {
                message: "Agent SDK integration not yet implemented. This GUI is under development per d-009.".to_string(),
            };
            send_message(session, response).await;
        }
        ClientMessage::Abort => {
            debug!("Abort requested");

            // === BLOCKED: Agent SDK integration required (d-009) ===
            // Abort requires sending interrupt signal to active SDK session.
            // No session management is currently implemented.

            let response = ServerMessage::Error {
                message: "Abort requires Agent SDK session integration (d-009 pending)".to_string(),
            };
            send_message(session, response).await;
        }
        ClientMessage::PermissionResponse {
            request_id,
            action,
            pattern,
            persistent,
        } => {
            debug!(
                "Permission response: {} -> {} (persistent: {})",
                request_id, action, persistent
            );

            // === BLOCKED: Agent SDK integration required (d-009) ===
            // Permission response integration requires bidirectional channel
            // between GUI WebSocket and autopilot permission handler.
            // This is experimental UI (d-009) - full integration pending.

            let response = ServerMessage::Error {
                message: "Permission handling requires Agent SDK integration (d-009 pending)".to_string(),
            };
            send_message(session, response).await;

            let _ = (request_id, action, pattern, persistent);
        }
    }
}

async fn send_message(session: &mut Session, msg: ServerMessage) {
    match serde_json::to_string(&msg) {
        Ok(json) => {
            if let Err(e) = session.text(json).await {
                error!("Failed to send message: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to serialize message: {}", e);
        }
    }
}
