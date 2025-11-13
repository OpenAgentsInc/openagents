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

        ControlCommand::UpdateThread { thread_id, updates } => {
            handle_update_thread(state, &thread_id, updates).await?;
        }

        ControlCommand::CreateProject { project } => {
            handle_create_project(state, project).await?;
        }

        ControlCommand::UpdateProject { project_id, updates } => {
            handle_update_project(state, &project_id, updates).await?;
        }

        ControlCommand::DeleteProject { project_id } => {
            handle_delete_project(state, &project_id).await?;
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

        "projects.list" => {
            let projects = state.tinyvex.list_projects()?;
            json!({
                "type": "tinyvex.query_result",
                "name": "projects.list",
                "rows": projects,
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

/// Handle update thread command (rename, archive, etc.)
async fn handle_update_thread(
    state: &Arc<TinyvexState>,
    thread_id: &str,
    updates: serde_json::Value,
) -> anyhow::Result<()> {
    info!("Update thread: thread_id={}, updates={}", thread_id, updates);

    // Fetch current thread from database
    let mut thread = state.tinyvex.get_thread(thread_id)?
        .ok_or_else(|| anyhow::anyhow!("Thread not found: {}", thread_id))?;

    // Apply updates
    if let Some(title) = updates.get("title").and_then(|t| t.as_str()) {
        thread.title = title.to_string();
    }

    if let Some(archived) = updates.get("archived").and_then(|a| a.as_bool()) {
        thread.archived = if archived { 1 } else { 0 };
    }

    // Update timestamp
    thread.updated_at = chrono::Utc::now().timestamp_millis();

    // Save updated thread
    state.tinyvex.upsert_thread(&thread)?;

    // Broadcast update notification
    let notification = tinyvex::WriterNotification::ThreadsUpsert { row: thread };
    broadcast_writer_notification(state, &notification).await;

    Ok(())
}

/// Handle create project command
async fn handle_create_project(
    state: &Arc<TinyvexState>,
    project: serde_json::Value,
) -> anyhow::Result<()> {
    use tinyvex::ProjectRow;

    info!("Create project: project={}", project);

    let id = project.get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing project id"))?;

    let name = project.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing project name"))?;

    let path = project.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing project path"))?;

    let now = chrono::Utc::now().timestamp_millis();

    let project_row = ProjectRow {
        id: id.to_string(),
        name: name.to_string(),
        path: path.to_string(),
        description: project.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
        icon: project.get("icon").and_then(|v| v.as_str()).map(|s| s.to_string()),
        color: project.get("color").and_then(|v| v.as_str()).map(|s| s.to_string()),
        starred: project.get("starred").and_then(|v| v.as_bool()).map(|b| if b { 1 } else { 0 }).unwrap_or(0),
        archived: 0,
        created_at: now,
        updated_at: now,
    };

    // Save project to database
    state.tinyvex.upsert_project(&project_row)?;

    // Broadcast creation notification
    let response = json!({
        "type": "tinyvex.update",
        "stream": "projects",
        "row": project_row,
    });

    state.broadcast(response.to_string()).await;
    Ok(())
}

/// Handle update project command
async fn handle_update_project(
    state: &Arc<TinyvexState>,
    project_id: &str,
    updates: serde_json::Value,
) -> anyhow::Result<()> {
    info!("Update project: project_id={}, updates={}", project_id, updates);

    // Fetch current project from database
    let mut project = state.tinyvex.get_project(project_id)?
        .ok_or_else(|| anyhow::anyhow!("Project not found: {}", project_id))?;

    // Apply updates
    if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
        project.name = name.to_string();
    }

    if let Some(path) = updates.get("path").and_then(|v| v.as_str()) {
        project.path = path.to_string();
    }

    if let Some(description) = updates.get("description").and_then(|v| v.as_str()) {
        project.description = Some(description.to_string());
    }

    if let Some(icon) = updates.get("icon").and_then(|v| v.as_str()) {
        project.icon = Some(icon.to_string());
    }

    if let Some(color) = updates.get("color").and_then(|v| v.as_str()) {
        project.color = Some(color.to_string());
    }

    if let Some(starred) = updates.get("starred").and_then(|v| v.as_bool()) {
        project.starred = if starred { 1 } else { 0 };
    }

    if let Some(archived) = updates.get("archived").and_then(|v| v.as_bool()) {
        project.archived = if archived { 1 } else { 0 };
    }

    // Update timestamp
    project.updated_at = chrono::Utc::now().timestamp_millis();

    // Save updated project
    state.tinyvex.upsert_project(&project)?;

    // Broadcast update notification
    let response = json!({
        "type": "tinyvex.update",
        "stream": "projects",
        "row": project,
    });

    state.broadcast(response.to_string()).await;
    Ok(())
}

/// Handle delete project command (archives the project)
async fn handle_delete_project(
    state: &Arc<TinyvexState>,
    project_id: &str,
) -> anyhow::Result<()> {
    info!("Delete project: project_id={}", project_id);

    // Soft delete by archiving
    state.tinyvex.delete_project(project_id)?;

    // Broadcast deletion notification
    let response = json!({
        "type": "tinyvex.update",
        "stream": "projects",
        "projectId": project_id,
        "archived": true,
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
