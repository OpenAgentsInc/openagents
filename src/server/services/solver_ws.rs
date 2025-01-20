use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use bytes::Bytes;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};
use serde_json::Value;

fn format_stream_chunk(chunk: &str) -> Result<String, serde_json::Error> {
    let v: Value = serde_json::from_str(chunk)?;
    
    if let Some(choices) = v.get("choices").and_then(Value::as_array) {
        if let Some(first) = choices.first() {
            if let Some(delta) = first.get("delta") {
                if let Some(content) = delta.get("content").and_then(Value::as_str) {
                    return Ok(format!(
                        r#"<div id="solver-result" hx-swap-oob="true" hx-swap="beforeend">
                            {}</div>"#,
                        content
                    ));
                }
            }
        }
    }
    
    Ok(String::new()) // Return empty string for non-content chunks
}

use super::solver::SolverService;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct SolverWsState {
    solver_service: Arc<SolverService>,
    connections: Arc<tokio::sync::RwLock<HashMap<String, mpsc::Sender<Message>>>>,
    update_tx: broadcast::Sender<SolverUpdate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum SolverUpdate {
    Progress {
        stage: SolverStage,
        message: String,
        data: Option<serde_json::Value>,
    },
    Complete {
        result: serde_json::Value,
    },
    Error {
        message: String,
        details: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SolverStage {
    Init,
    Repomap,
    Analysis,
    Solution,
    PR,
}

impl SolverWsState {
    pub fn new(solver_service: Arc<SolverService>) -> Self {
        let (update_tx, _) = broadcast::channel(100);
        Self {
            solver_service,
            connections: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            update_tx,
        }
    }

    pub async fn broadcast_update(&self, update: SolverUpdate) {
        // Convert update to HTML fragment with hx-swap-oob
        let html = match &update {
            SolverUpdate::Progress { stage, message, data: _ } => {
                format!(
                    r#"<div id="solver-progress" hx-swap-oob="true">
                        <div class="progress-bar" style="width: {}%">
                            {}
                        </div>
                    </div>
                    <div id="solver-status" hx-swap-oob="true">
                        Stage {}: {}
                    </div>"#,
                    match stage {
                        SolverStage::Init => 0,
                        SolverStage::Repomap => 25,
                        SolverStage::Analysis => 50,
                        SolverStage::Solution => 75,
                        SolverStage::PR => 90,
                    },
                    message,
                    match stage {
                        SolverStage::Init => "1/5",
                        SolverStage::Repomap => "2/5", 
                        SolverStage::Analysis => "3/5",
                        SolverStage::Solution => "4/5",
                        SolverStage::PR => "5/5",
                    },
                    message
                )
            }
            SolverUpdate::Complete { result } => {
                format!(
                    r#"<div id="solver-progress" hx-swap-oob="true">
                        <div class="progress-bar" style="width: 100%">
                            Complete
                        </div>
                    </div>
                    <div id="solver-status" hx-swap-oob="true">
                        Solution complete
                    </div>
                    <div id="solver-result" hx-swap-oob="true">
                        {result}
                    </div>"#
                )
            }
            SolverUpdate::Error { message, details } => {
                format!(
                    r#"<div id="solver-status" hx-swap-oob="true">
                        <div class="error">
                            Error: {message}
                            {}</div>
                    </div>"#,
                    details.as_ref().map(|d| format!("<pre>{d}</pre>")).unwrap_or_default()
                )
            }
        };

        // Send HTML update to all connected clients
        let conns = self.connections.read().await;
        for tx in conns.values() {
            let _ = tx.send(Message::Text(html.clone().into())).await;
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<SolverWsState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<SolverWsState>) {
    let (tx, mut rx) = mpsc::channel(32);
    let (mut sender, mut receiver) = socket.split();
    let state_clone = state.clone();
    let tx_clone = tx.clone();

    // Add connection to state
    let conn_id = uuid::Uuid::new_v4().to_string();
    {
        let mut conns = state.connections.write().await;
        conns.insert(conn_id.clone(), tx.clone());
    }

    // Create broadcast subscription
    let mut update_rx = state.update_tx.subscribe();

    // Forward broadcast updates to this client
    let send_task = tokio::spawn(async move {
        let last_active = Instant::now();
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);

        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    if sender.send(msg).await.is_err() {
                        break;
                    }
                }
                Ok(update) = update_rx.recv() => {
                    // Convert update to HTML and send
                    let html = match update {
                        SolverUpdate::Progress { stage, message, data: _ } => {
                            format!(
                                r#"<div id="solver-progress" hx-swap-oob="true">
                                    <div class="progress-bar" style="width: {}%">
                                        {}
                                    </div>
                                </div>"#,
                                match stage {
                                    SolverStage::Init => 0,
                                    SolverStage::Repomap => 25,
                                    SolverStage::Analysis => 50,
                                    SolverStage::Solution => 75,
                                    SolverStage::PR => 90,
                                },
                                message
                            )
                        }
                        SolverUpdate::Complete { result } => {
                            format!(
                                r#"<div id="solver-progress" hx-swap-oob="true">
                                    <div class="progress-bar" style="width: 100%">
                                        Complete
                                    </div>
                                </div>
                                <div id="solver-status" hx-swap-oob="true">
                                    Solution complete
                                </div>
                                <div id="solver-result" hx-swap-oob="true">
                                    <pre class="solution-text" style="white-space: pre-wrap; word-wrap: break-word; max-width: 100%; padding: 1em; background: #1a1a1a; border-radius: 4px;">
                                        {}
                                    </pre>
                                </div>"#,
                                result["solution"]
                            )
                        }
                        SolverUpdate::Error { message, details } => {
                            format!(
                                r#"<div id="solver-status" hx-swap-oob="true">
                                    <div class="error">
                                        Error: {message}
                                        {}</div>
                                </div>"#,
                                details.as_ref().map(|d| format!("<pre>{d}</pre>")).unwrap_or_default()
                            )
                        }
                    };
                    if sender.send(Message::Text(html.into())).await.is_err() {
                        break;
                    }
                }
                _ = heartbeat_interval.tick() => {
                    if Instant::now().duration_since(last_active) > CLIENT_TIMEOUT {
                        break;
                    }
                    if sender.send(Message::Ping(Bytes::new())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Handle incoming messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    info!("Received message: {}", text);
                    // Parse HTMX form data
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(issue_url) = data.get("issue_url") {
                            if let Some(url) = issue_url.as_str() {
                                info!("Starting solver for issue: {}", url);
                                

                                match state_clone.solver_service.solve_issue_with_ws(
                                    url.to_string(),
                                    state_clone.update_tx.clone()
                                ).await {
                                    Ok(response) => {
                                        let html = format!(
                                            r#"<div id="solver-result" hx-swap-oob="true">
                                                <pre class="solution-text" style="white-space: pre-wrap; word-wrap: break-word; max-width: 100%; padding: 1em; background: #1a1a1a; border-radius: 4px;">
                                                    {}
                                                </pre>
                                            </div>"#,
                                            response.solution
                                        );
                                        // Ensure message is sent or log error
                                        if let Err(e) = tx_clone.send(Message::Text(html.into())).await {
                                            error!("Failed to send final solution: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Solver error: {}", e);
                                        let error_html = format!(
                                            r#"<div id="solver-error" hx-swap-oob="true">
                                                <div class="error">Error: {}</div>
                                            </div>"#,
                                            e
                                        );
                                        let _ = tx_clone.send(Message::Text(error_html.into())).await;
                                    }
                                }
                            }
                        }
                    }
                }
                Message::Ping(bytes) => {
                    let _ = tx_clone.send(Message::Pong(bytes)).await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for both tasks to complete
    let (send_result, recv_result) = tokio::join!(send_task, recv_task);
    
    info!("Send task completed: {:?}", send_result);
    info!("Receive task completed: {:?}", recv_result);

    // Remove connection from state
    let mut conns = state.connections.write().await;
    conns.remove(&conn_id);
}
