use std::{sync::Arc, time::Duration};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::{broadcast, mpsc, RwLock};
use std::collections::HashMap;
use bytes::Bytes;
use tracing::{error, info};

use crate::server::services::solver::SolverService;
use super::types::SolverUpdate;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct SolverWsState {
    solver_service: Arc<SolverService>,
    connections: Arc<RwLock<HashMap<String, mpsc::Sender<Message>>>>,
    update_tx: broadcast::Sender<SolverUpdate>,
}

impl SolverWsState {
    pub fn new(solver_service: Arc<SolverService>) -> Self {
        let (update_tx, _) = broadcast::channel(100);
        Self {
            solver_service,
            connections: Arc::new(RwLock::new(HashMap::new())),
            update_tx,
        }
    }

    pub async fn broadcast_update(&self, update: SolverUpdate) {
        let conns = self.connections.read().await;
        
        match &update {
            SolverUpdate::Progress { stage, message, data } => {
                // Send stage update (progress bar)
                let progress_html = format!(
                    "<div id='progress-bar' hx-swap-oob='true'>{}</div>
                     <div id='solver-status' hx-swap-oob='true'>{}</div>",
                    super::html_formatting::render_progress_bar(stage),
                    message
                );

                // Send content updates if any
                if let Some(data) = data {
                    // Files list (replace)
                    if let Some(files_list) = data.get("files_list") {
                        let content_html = format!(
                            "<div id='files-list' hx-swap-oob='true'>{}</div>",
                            super::html_formatting::render_files_list(files_list)
                        );
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(content_html.clone().into())).await;
                        }
                    }

                    // Files reasoning (replace)
                    if let Some(files_reasoning) = data.get("files_reasoning") {
                        let reasoning_html = format!(
                            "<div id='files-reasoning' hx-swap-oob='true'>{}</div>",
                            super::html_formatting::render_files_reasoning(files_reasoning)
                        );
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(reasoning_html.clone().into())).await;
                        }
                    }

                    // Solution code (replace)
                    if let Some(solution_text) = data.get("solution_text") {
                        let solution_html = format!(
                            "<div id='solution-code' hx-swap-oob='true'>{}</div>",
                            super::html_formatting::render_solution(solution_text)
                        );
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(solution_html.clone().into())).await;
                        }
                    }

                    // Solution reasoning (replace)
                    if let Some(solution_reasoning) = data.get("solution_reasoning") {
                        let reasoning_html = format!(
                            "<div id='solution-reasoning' hx-swap-oob='true'>{}</div>",
                            super::html_formatting::render_solution_reasoning(solution_reasoning)
                        );
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(reasoning_html.clone().into())).await;
                        }
                    }
                }

                // Send progress bar update
                for tx in conns.values() {
                    let _ = tx.send(Message::Text(progress_html.clone().into())).await;
                }
            }
            SolverUpdate::Complete { result } => {
                // Send completion message
                let complete_html = format!(
                    "<div id='solver-status' hx-swap-oob='true'>Complete</div>
                     <div id='solution-code' hx-swap-oob='true'>{}</div>",
                    super::html_formatting::render_complete(result)
                );
                
                // Send completion message
                let complete_msg = serde_json::json!({
                    "type": "complete",
                    "message": "Solution complete"
                }).to_string();

                for tx in conns.values() {
                    let _ = tx.send(Message::Text(complete_html.clone().into())).await;
                    let _ = tx.send(Message::Text(complete_msg.clone().into())).await;
                }
            }
            SolverUpdate::Error { message, details } => {
                // Show error in error section
                let error_html = format!(
                    "<div id='error-section' hx-swap-oob='true' class='mt-4 p-4 bg-red-900/20 border border-red-500/20 rounded'>
                        <div id='error-message' class='text-sm text-red-400'>{}: {}</div>
                    </div>",
                    message,
                    details.as_deref().unwrap_or("")
                );

                // Send error message
                let error_msg = serde_json::json!({
                    "type": "error",
                    "message": message,
                    "details": details
                }).to_string();

                for tx in conns.values() {
                    let _ = tx.send(Message::Text(error_html.clone().into())).await;
                    let _ = tx.send(Message::Text(error_msg.clone().into())).await;
                }
            }
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
    let (tx, mut rx) = mpsc::channel(100); // Increase buffer size
    let (mut sender, mut receiver) = socket.split();
    let state_clone = Arc::clone(&state);
    let state_clone2 = Arc::clone(&state);
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
        let last_active = std::time::Instant::now();
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);

        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    if sender.send(msg).await.is_err() {
                        break;
                    }
                }
                Ok(update) = update_rx.recv() => {
                    // Let broadcast_update handle the HTML formatting
                    state_clone.broadcast_update(update).await;
                }
                _ = heartbeat_interval.tick() => {
                    if std::time::Instant::now().duration_since(last_active) > CLIENT_TIMEOUT {
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

                                // Reset UI state
                                let reset_html = r#"
                                    <div id="files-list" hx-swap-oob="true"></div>
                                    <div id="files-reasoning" hx-swap-oob="true"></div>
                                    <div id="solution-reasoning" hx-swap-oob="true"></div>
                                    <div id="solution-code" hx-swap-oob="true"></div>
                                    <div id="error-section" hx-swap-oob="true" class="hidden"></div>
                                "#;
                                let _ = tx_clone.send(Message::Text(reset_html.into())).await;

                                let solve_result = state_clone2
                                    .solver_service
                                    .solve_issue_with_ws(
                                        url.to_string(),
                                        state_clone2.update_tx.clone(),
                                    )
                                    .await;

                                match solve_result {
                                    Ok(response) => {
                                        // Final solution is handled by broadcast_update
                                        let complete_update = SolverUpdate::Complete {
                                            result: serde_json::json!({
                                                "solution": response.solution
                                            }),
                                        };
                                        state_clone2.broadcast_update(complete_update).await;
                                    }
                                    Err(e) => {
                                        error!("Solver error: {}", e);
                                        let error_update = SolverUpdate::Error {
                                            message: "Solver error".into(),
                                            details: Some(e.to_string()),
                                        };
                                        state_clone2.broadcast_update(error_update).await;
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