//! Autopilot Container Service
//!
//! HTTP API wrapper for autopilot, designed to run in Cloudflare Containers.
//! Exposes endpoints for starting tasks, checking status, and streaming events via WebSocket.

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
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

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
}

#[derive(Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TaskStatus {
    Starting,
    Running,
    Completed,
    Failed,
}

/// Events streamed to clients
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TaskEvent {
    /// Text chunk from Claude
    Chunk { task_id: String, text: String },
    /// Tool execution started
    ToolStart {
        task_id: String,
        tool_name: String,
        tool_id: String,
    },
    /// Tool execution completed
    ToolDone {
        task_id: String,
        tool_id: String,
        output: String,
        is_error: bool,
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
    #[serde(default)]
    claude_api_key: Option<String>,
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
                .add_directive("autopilot_container=info".parse().unwrap()),
        )
        .init();

    info!("Starting autopilot container service");

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
            if t.status == TaskStatus::Running || t.status == TaskStatus::Starting {
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
        status: TaskStatus::Starting,
        started_at: chrono::Utc::now().to_rfc3339(),
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
            status: "starting".to_string(),
        }),
    )
        .into_response()
}

/// Get current task status
async fn get_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let task = state.task.read().await;
    Json(StatusResponse {
        task: task.clone(),
    })
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
                    // Future: handle input messages
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

/// Run the actual autopilot task
async fn run_autopilot_task(state: Arc<AppState>, task_id: String, repo: String, prompt: String) {
    info!("Starting task {} for repo {}", task_id, repo);

    // Update status to running
    {
        let mut task = state.task.write().await;
        if let Some(ref mut t) = *task {
            t.status = TaskStatus::Running;
        }
    }

    // TODO: Integrate with actual autopilot
    // For now, simulate some events
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let _ = state.events_tx.send(TaskEvent::Chunk {
        task_id: task_id.clone(),
        text: format!("Processing task for repo: {}\n", repo),
    });

    let _ = state.events_tx.send(TaskEvent::Chunk {
        task_id: task_id.clone(),
        text: format!("Prompt: {}\n", prompt),
    });

    // Simulate tool use
    let tool_id = uuid::Uuid::new_v4().to_string();
    let _ = state.events_tx.send(TaskEvent::ToolStart {
        task_id: task_id.clone(),
        tool_name: "Read".to_string(),
        tool_id: tool_id.clone(),
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let _ = state.events_tx.send(TaskEvent::ToolDone {
        task_id: task_id.clone(),
        tool_id,
        output: "File contents...".to_string(),
        is_error: false,
    });

    // Complete task
    let _ = state.events_tx.send(TaskEvent::Done {
        task_id: task_id.clone(),
        summary: "Task completed successfully".to_string(),
    });

    // Update status to completed
    {
        let mut task = state.task.write().await;
        if let Some(ref mut t) = *task {
            t.status = TaskStatus::Completed;
        }
    }

    info!("Task {} completed", task_id);
}
