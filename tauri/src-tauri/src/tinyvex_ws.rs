//! WebSocket server for tinyvex - simplified for Tauri integration.
//!
//! Provides WebSocket endpoint for real-time ACP event streaming and tinyvex queries.
//! Clients can subscribe to updates and query the database via control messages.

use std::sync::Arc;

use axum::{
    extract::{ws::{Message, WebSocket}, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tracing::{info, error, warn};

use crate::tinyvex_controls::{parse_control_command, ControlCommand};
use crate::tinyvex_state::TinyvexState;

/// Create the WebSocket router with the /ws endpoint
pub fn create_router(state: Arc<TinyvexState>) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

/// WebSocket upgrade handler
async fn ws_handler(
    State(state): State<Arc<TinyvexState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    info!("WebSocket connection request");
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, state: Arc<TinyvexState>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast channel for updates
    let mut rx = state.tx.subscribe();

    // Send history replay to new client
    let history = state.get_history().await;
    for msg in history {
        if let Err(e) = sender.send(Message::Text(msg.into())).await {
            error!("Failed to send history: {}", e);
            return;
        }
    }

    // Spawn task to forward broadcast messages to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from client
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = handle_control_message(&text, &state).await {
                        error!("Error handling control message: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket closed by client");
                    break;
                }
                Err(e) => {
                    warn!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("WebSocket connection closed");
}

/// Handle a control message from the client
async fn handle_control_message(text: &str, state: &Arc<TinyvexState>) -> anyhow::Result<()> {
    let cmd = parse_control_command(text)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse control command"))?;

    match cmd {
        ControlCommand::Echo { payload, tag } => {
            handle_echo(state, payload, tag).await;
        }

        ControlCommand::TvxQuery { name, args } => {
            handle_tvx_query(state, &name, args).await?;
        }

        ControlCommand::TvxSubscribe { stream, thread_id } => {
            handle_tvx_subscribe(state, &stream, thread_id.as_deref()).await?;
        }

        ControlCommand::RunSubmit { thread_id, text } => {
            handle_run_submit(state, &thread_id, &text).await?;
        }
    }

    Ok(())
}

/// Handle echo command (connection testing)
async fn handle_echo(state: &Arc<TinyvexState>, payload: Option<String>, tag: Option<String>) {
    let response = json!({
        "type": "echo",
        "payload": payload,
        "tag": tag,
        "timestamp": chrono::Utc::now().timestamp_millis(),
    });

    state.broadcast(response.to_string()).await;
}

/// Handle tinyvex query command
async fn handle_tvx_query(
    state: &Arc<TinyvexState>,
    name: &str,
    args: serde_json::Value,
) -> anyhow::Result<()> {
    let result = match name {
        "threads.list" => {
            let limit = args.get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(50);

            let threads = state.tinyvex.list_threads(limit)?;
            json!({
                "type": "tinyvex.query_result",
                "name": "threads.list",
                "rows": threads,
            })
        }

        "messages.list" => {
            let thread_id = args.get("threadId")
                .or_else(|| args.get("thread_id"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing threadId"))?;

            let limit = args.get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(50);

            let messages = state.tinyvex.list_messages(thread_id, limit)?;
            json!({
                "type": "tinyvex.query_result",
                "name": "messages.list",
                "threadId": thread_id,
                "rows": messages,
            })
        }

        "tool_calls.list" => {
            let thread_id = args.get("threadId")
                .or_else(|| args.get("thread_id"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing threadId"))?;

            let limit = args.get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(50);

            let tool_calls = state.tinyvex.list_tool_calls(thread_id, limit)?;
            json!({
                "type": "tinyvex.query_result",
                "name": "tool_calls.list",
                "threadId": thread_id,
                "rows": tool_calls,
            })
        }

        _ => {
            return Err(anyhow::anyhow!("Unknown query: {}", name));
        }
    };

    state.broadcast(result.to_string()).await;
    Ok(())
}

/// Handle tinyvex subscribe command (returns initial snapshot)
async fn handle_tvx_subscribe(
    state: &Arc<TinyvexState>,
    stream: &str,
    thread_id: Option<&str>,
) -> anyhow::Result<()> {
    match stream {
        "threads" => {
            let threads = state.tinyvex.list_threads(100)?;
            let snapshot = json!({
                "type": "tinyvex.snapshot",
                "stream": "threads",
                "rows": threads,
            });
            state.broadcast(snapshot.to_string()).await;
        }

        "messages" => {
            let thread_id = thread_id
                .ok_or_else(|| anyhow::anyhow!("Missing thread_id for messages subscription"))?;

            let messages = state.tinyvex.list_messages(thread_id, 100)?;
            let snapshot = json!({
                "type": "tinyvex.snapshot",
                "stream": "messages",
                "threadId": thread_id,
                "rows": messages,
            });
            state.broadcast(snapshot.to_string()).await;
        }

        _ => {
            return Err(anyhow::anyhow!("Unknown stream: {}", stream));
        }
    }

    Ok(())
}

/// Handle run submit command (start/continue conversation)
async fn handle_run_submit(
    state: &Arc<TinyvexState>,
    thread_id: &str,
    text: &str,
) -> anyhow::Result<()> {
    info!("Run submit: thread_id={}, text_len={}", thread_id, text.len());

    // For now, just acknowledge receipt
    // TODO: Wire this to ACP session manager to actually submit the prompt
    let response = json!({
        "type": "run.submitted",
        "threadId": thread_id,
        "timestamp": chrono::Utc::now().timestamp_millis(),
    });

    state.broadcast(response.to_string()).await;
    Ok(())
}

/// Broadcast a tinyvex writer notification to all connected clients
pub async fn broadcast_writer_notification(
    state: &Arc<TinyvexState>,
    notification: &tinyvex::WriterNotification,
) {
    let msg = match notification {
        tinyvex::WriterNotification::ThreadsUpsert { row } => {
            json!({
                "type": "tinyvex.update",
                "stream": "threads",
                "row": row,
            })
        }

        tinyvex::WriterNotification::MessagesUpsert {
            thread_id,
            item_id,
            kind,
            role,
            seq,
            text_len,
        } => {
            json!({
                "type": "tinyvex.update",
                "stream": "messages",
                "threadId": thread_id,
                "itemId": item_id,
                "kind": kind,
                "role": role,
                "seq": seq,
                "textLen": text_len,
            })
        }

        tinyvex::WriterNotification::MessagesFinalize {
            thread_id,
            item_id,
            kind,
            text_len,
        } => {
            json!({
                "type": "tinyvex.finalize",
                "stream": "messages",
                "threadId": thread_id,
                "itemId": item_id,
                "kind": kind,
                "textLen": text_len,
            })
        }

        tinyvex::WriterNotification::ToolCallUpsert { thread_id, tool_call_id } => {
            json!({
                "type": "tinyvex.update",
                "stream": "tool_calls",
                "threadId": thread_id,
                "toolCallId": tool_call_id,
            })
        }

        tinyvex::WriterNotification::ToolCallUpdate { thread_id, tool_call_id } => {
            json!({
                "type": "tinyvex.update",
                "stream": "tool_calls",
                "threadId": thread_id,
                "toolCallId": tool_call_id,
            })
        }

        tinyvex::WriterNotification::PlanUpsert { thread_id } => {
            json!({
                "type": "tinyvex.update",
                "stream": "plan",
                "threadId": thread_id,
            })
        }

        tinyvex::WriterNotification::StateUpsert { thread_id } => {
            json!({
                "type": "tinyvex.update",
                "stream": "state",
                "threadId": thread_id,
            })
        }
    };

    state.broadcast(msg.to_string()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_websocket_echo() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let tvx = Arc::new(tinyvex::Tinyvex::open(&db_path).unwrap());
        let writer = Arc::new(tinyvex::Writer::new(tvx.clone()));
        let state = Arc::new(TinyvexState::new(tvx, writer));

        // Test echo command parsing and handling
        let echo_cmd = r#"{"control":"echo","payload":"test","tag":"t1"}"#;
        handle_control_message(echo_cmd, &state).await.unwrap();

        // Verify broadcast
        let mut rx = state.tx.subscribe();
        tokio::select! {
            Ok(msg) = rx.recv() => {
                let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
                assert_eq!(v["type"], "echo");
                assert_eq!(v["payload"], "test");
                assert_eq!(v["tag"], "t1");
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                panic!("No message received");
            }
        }
    }
}
