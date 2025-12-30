//! Autopilot Container Service
//!
//! HTTP API wrapper for autopilot, designed to run in Cloudflare Containers.
//! Exposes endpoints for starting tasks, checking status, and streaming events via WebSocket.
//!
//! ## Architecture
//!
//! 1. User starts task via POST /api/start with repo URL and prompt
//! 2. Service clones repo to /workspace
//! 3. Service runs Claude SDK queries against the repo
//! 4. Results stream to connected WebSocket clients
//! 5. Task completes, repo is cleaned up

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use claude_agent_sdk::{
    AllowAllPermissions, QueryOptions, SdkMessage, SdkResultMessage,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

/// Workspace directory for cloned repos
const WORKSPACE_DIR: &str = "/workspace";

/// Application state shared across handlers
struct AppState {
    /// Current task status
    task: RwLock<Option<TaskState>>,
    /// Broadcast channel for streaming events
    events_tx: broadcast::Sender<TaskEvent>,
}

/// Current task state
#[derive(Clone, Serialize)]
struct TaskState {
    task_id: String,
    repo: String,
    prompt: String,
    status: TaskStatus,
    started_at: String,
    working_dir: Option<String>,
}

#[derive(Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TaskStatus {
    Cloning,
    Running,
    Completed,
    Failed,
}

/// Events streamed to clients
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskEvent {
    /// Status update
    Status { task_id: String, status: String },
    /// Text chunk from Claude
    Chunk { task_id: String, text: String },
    /// Tool execution started
    ToolStart {
        task_id: String,
        tool_name: String,
        tool_id: String,
        params: serde_json::Value,
    },
    /// Tool execution completed
    ToolDone {
        task_id: String,
        tool_id: String,
        output: String,
        is_error: bool,
    },
    /// Tool progress update
    ToolProgress {
        task_id: String,
        tool_id: String,
        elapsed_secs: f32,
    },
    /// Usage statistics
    Usage {
        task_id: String,
        input_tokens: u64,
        output_tokens: u64,
        total_cost_usd: f64,
    },
    /// Task completed
    Done { task_id: String, summary: String },
    /// Task failed
    Error { task_id: String, error: String },
}

/// Request to start a new task
#[derive(Deserialize)]
struct StartTaskRequest {
    repo: String,
    prompt: String,
}

/// Response from starting a task
#[derive(Serialize)]
struct StartTaskResponse {
    task_id: String,
    status: String,
}

/// Response from status endpoint
#[derive(Serialize)]
struct StatusResponse {
    task: Option<TaskState>,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("autopilot_container=info".parse().unwrap())
                .add_directive("claude_agent_sdk=info".parse().unwrap()),
        )
        .init();

    info!("Starting autopilot container service");

    // Create workspace directory
    std::fs::create_dir_all(WORKSPACE_DIR).ok();

    // Create broadcast channel for events (capacity 1000)
    let (events_tx, _) = broadcast::channel(1000);

    // Create shared state
    let state = Arc::new(AppState {
        task: RwLock::new(None),
        events_tx,
    });

    // Build router
    let app = Router::new()
        .route("/ping", get(ping))
        .route("/api/start", post(start_task))
        .route("/api/status", get(get_status))
        .route("/ws", get(ws_handler))
        .with_state(state);

    // Bind to all interfaces on port 8080
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Listening on 0.0.0.0:8080");

    axum::serve(listener, app).await.unwrap();
}

/// Health check endpoint
async fn ping() -> &'static str {
    "ok"
}

/// Start a new task
async fn start_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartTaskRequest>,
) -> Response {
    // Check if a task is already running
    {
        let task = state.task.read().await;
        if let Some(ref t) = *task {
            if t.status == TaskStatus::Running || t.status == TaskStatus::Cloning {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "A task is already running",
                        "task_id": t.task_id
                    })),
                )
                    .into_response();
            }
        }
    }

    // Create new task
    let task_id = uuid::Uuid::new_v4().to_string();
    let task_state = TaskState {
        task_id: task_id.clone(),
        repo: req.repo.clone(),
        prompt: req.prompt.clone(),
        status: TaskStatus::Cloning,
        started_at: chrono::Utc::now().to_rfc3339(),
        working_dir: None,
    };

    // Store task state
    {
        let mut task = state.task.write().await;
        *task = Some(task_state);
    }

    // Spawn task execution in background
    let state_clone = state.clone();
    let repo = req.repo.clone();
    let prompt = req.prompt.clone();
    let task_id_clone = task_id.clone();

    tokio::spawn(async move {
        run_autopilot_task(state_clone, task_id_clone, repo, prompt).await;
    });

    (
        StatusCode::OK,
        Json(StartTaskResponse {
            task_id,
            status: "cloning".to_string(),
        }),
    )
        .into_response()
}

/// Get current task status
async fn get_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let task = state.task.read().await;
    Json(StatusResponse { task: task.clone() })
}

/// WebSocket handler for streaming events
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to events
    let mut events_rx = state.events_tx.subscribe();

    // Spawn task to forward events to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(event) = events_rx.recv().await {
            let msg = serde_json::to_string(&event).unwrap();
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages (for future input support)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    info!("Received from client: {}", text);
                    // Future: handle input messages like user responses
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }
}

/// Clone a git repository to the workspace
fn clone_repo(repo_url: &str, task_id: &str) -> Result<PathBuf, String> {
    let repo_name = repo_url
        .split('/')
        .last()
        .unwrap_or("repo")
        .trim_end_matches(".git");

    let target_dir = PathBuf::from(WORKSPACE_DIR).join(format!("{}_{}", repo_name, task_id));

    // Use git CLI for simplicity (handles auth better)
    let status = Command::new("git")
        .args(["clone", "--depth", "1", repo_url])
        .arg(&target_dir)
        .status()
        .map_err(|e| format!("Failed to spawn git: {}", e))?;

    if status.success() {
        Ok(target_dir)
    } else {
        Err(format!("git clone failed with status: {}", status))
    }
}

/// Clean up a cloned repository
fn cleanup_repo(dir: &PathBuf) {
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            warn!("Failed to cleanup repo dir: {}", e);
        }
    }
}

/// Run the actual autopilot task
async fn run_autopilot_task(state: Arc<AppState>, task_id: String, repo: String, prompt: String) {
    info!("Starting task {} for repo {}", task_id, repo);

    // Send status update
    let _ = state.events_tx.send(TaskEvent::Status {
        task_id: task_id.clone(),
        status: "cloning".to_string(),
    });

    // Clone the repository
    let working_dir = match clone_repo(&repo, &task_id) {
        Ok(dir) => {
            info!("Cloned repo to {:?}", dir);
            dir
        }
        Err(e) => {
            error!("Failed to clone repo: {}", e);
            let _ = state.events_tx.send(TaskEvent::Error {
                task_id: task_id.clone(),
                error: format!("Failed to clone repository: {}", e),
            });

            // Update task status to failed
            let mut task = state.task.write().await;
            if let Some(ref mut t) = *task {
                t.status = TaskStatus::Failed;
            }
            return;
        }
    };

    // Update task state with working directory
    {
        let mut task = state.task.write().await;
        if let Some(ref mut t) = *task {
            t.status = TaskStatus::Running;
            t.working_dir = Some(working_dir.to_string_lossy().to_string());
        }
    }

    let _ = state.events_tx.send(TaskEvent::Status {
        task_id: task_id.clone(),
        status: "running".to_string(),
    });

    // Run Claude SDK query
    let result = run_claude_query(&state, &task_id, &working_dir, &prompt).await;

    // Clean up repo
    cleanup_repo(&working_dir);

    // Update final status
    let mut task = state.task.write().await;
    match result {
        Ok(summary) => {
            let _ = state.events_tx.send(TaskEvent::Done {
                task_id: task_id.clone(),
                summary: summary.clone(),
            });
            if let Some(ref mut t) = *task {
                t.status = TaskStatus::Completed;
            }
            info!("Task {} completed: {}", task_id, summary);
        }
        Err(e) => {
            let _ = state.events_tx.send(TaskEvent::Error {
                task_id: task_id.clone(),
                error: e.clone(),
            });
            if let Some(ref mut t) = *task {
                t.status = TaskStatus::Failed;
            }
            error!("Task {} failed: {}", task_id, e);
        }
    }
}

/// Run a Claude SDK query and stream results
async fn run_claude_query(
    state: &Arc<AppState>,
    task_id: &str,
    working_dir: &PathBuf,
    prompt: &str,
) -> Result<String, String> {
    use std::sync::Arc as StdArc;

    // Change to working directory for the query
    let original_dir = std::env::current_dir().ok();
    if let Err(e) = std::env::set_current_dir(working_dir) {
        return Err(format!("Failed to change to working dir: {}", e));
    }

    // Create query options
    let options = QueryOptions::new()
        .model("claude-sonnet-4-5-20250929")
        .max_turns(50)
        .max_budget_usd(10.0)
        .include_partial_messages(true);

    // Run the query
    let query_result = claude_agent_sdk::query_with_permissions(
        prompt,
        options,
        StdArc::new(AllowAllPermissions),
    )
    .await;

    // Restore original directory
    if let Some(dir) = original_dir {
        let _ = std::env::set_current_dir(dir);
    }

    let mut stream = match query_result {
        Ok(s) => s,
        Err(e) => return Err(format!("Failed to start Claude query: {}", e)),
    };

    let mut final_summary = String::new();
    let mut current_tool_id: Option<String> = None;

    // Process stream
    while let Some(msg_result) = stream.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                warn!("Stream error: {}", e);
                continue;
            }
        };

        match msg {
            SdkMessage::Assistant(assistant_msg) => {
                // Stream text content - message is serde_json::Value
                if let Some(content) = assistant_msg.message.get("content").and_then(|c| c.as_array()) {
                    for block in content {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            let _ = state.events_tx.send(TaskEvent::Chunk {
                                task_id: task_id.to_string(),
                                text: text.to_string(),
                            });
                            final_summary.push_str(text);
                        }
                    }
                }
            }
            SdkMessage::StreamEvent(stream_event) => {
                // SdkStreamEvent has event: Value - access everything through JSON
                let event = &stream_event.event;

                // Handle content_block_start with tool_use
                if let Some(content_block) = event.get("content_block") {
                    if content_block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let tool_id = content_block
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let tool_name = content_block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();

                        current_tool_id = Some(tool_id.clone());

                        let _ = state.events_tx.send(TaskEvent::ToolStart {
                            task_id: task_id.to_string(),
                            tool_name,
                            tool_id,
                            params: content_block.clone(),
                        });
                    }
                }

                // Handle text delta
                if let Some(delta) = event.get("delta") {
                    if let Some(text_delta) = delta.get("text").and_then(|t| t.as_str()) {
                        let _ = state.events_tx.send(TaskEvent::Chunk {
                            task_id: task_id.to_string(),
                            text: text_delta.to_string(),
                        });
                    }
                }
            }
            SdkMessage::User(user_msg) => {
                // Handle tool results from user messages
                if let Some(tool_result) = &user_msg.tool_use_result {
                    if let Some(tool_id) = current_tool_id.take() {
                        let output = tool_result
                            .get("content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string();
                        let is_error = tool_result
                            .get("is_error")
                            .and_then(|e| e.as_bool())
                            .unwrap_or(false);

                        let _ = state.events_tx.send(TaskEvent::ToolDone {
                            task_id: task_id.to_string(),
                            tool_id,
                            output,
                            is_error,
                        });
                    }
                }
            }
            SdkMessage::ToolProgress(progress) => {
                let _ = state.events_tx.send(TaskEvent::ToolProgress {
                    task_id: task_id.to_string(),
                    tool_id: progress.tool_use_id.clone(),
                    elapsed_secs: progress.elapsed_time_seconds as f32,
                });
            }
            SdkMessage::Result(result) => {
                match result {
                    SdkResultMessage::Success(success) => {
                        // Send usage stats - total_cost_usd is on ResultSuccess, not Usage
                        let _ = state.events_tx.send(TaskEvent::Usage {
                            task_id: task_id.to_string(),
                            input_tokens: success.usage.input_tokens,
                            output_tokens: success.usage.output_tokens,
                            total_cost_usd: success.total_cost_usd,
                        });

                        if final_summary.is_empty() {
                            final_summary = success.result.clone();
                        }
                    }
                    SdkResultMessage::ErrorDuringExecution(err)
                    | SdkResultMessage::ErrorMaxTurns(err)
                    | SdkResultMessage::ErrorMaxBudget(err)
                    | SdkResultMessage::ErrorMaxStructuredOutputRetries(err) => {
                        return Err(format!("Claude error: {:?}", err.errors));
                    }
                }
            }
            _ => {}
        }
    }

    // Truncate summary if too long
    if final_summary.len() > 500 {
        final_summary = format!("{}...", &final_summary[..500]);
    }

    Ok(final_summary)
}
