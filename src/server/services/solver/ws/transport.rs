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
                let progress_html = super::html_formatting::render_progress_bar(stage, message);

                // Send content updates if any
                if let Some(data) = data {
                    if let Some(files_list) = data.get("files_list") {
                        let content_html = super::html_formatting::render_files_list(files_list);
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(content_html.clone().into())).await;
                        }
                    }
                    if let Some(files_reasoning) = data.get("files_reasoning") {
                        let reasoning_html = super::html_formatting::render_files_reasoning(files_reasoning);
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(reasoning_html.clone().into())).await;
                        }
                    }
                    if let Some(solution_text) = data.get("solution_text") {
                        let solution_html = super::html_formatting::render_solution(solution_text);
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(solution_html.clone().into())).await;
                        }
                    }
                    if let Some(solution_reasoning) = data.get("solution_reasoning") {
                        let reasoning_html = super::html_formatting::render_solution_reasoning(solution_reasoning);
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
                let html = super::html_formatting::render_complete(result);
                for tx in conns.values() {
                    let _ = tx.send(Message::Text(html.clone().into())).await;
                }
            }
            SolverUpdate::Error { message, details } => {
                let html = super::html_formatting::render_error(message, details);
                for tx in conns.values() {
                    let _ = tx.send(Message::Text(html.clone().into())).await;
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
                    // Convert update to HTML and send
                    let html = match update {
                        SolverUpdate::Progress { stage, message, data: _ } => {
                            super::html_formatting::render_progress_bar(&stage, &message)
                        }
                        SolverUpdate::Complete { result } => {
                            super::html_formatting::render_complete(&result)
                        }
                        SolverUpdate::Error { message, details } => {
                            super::html_formatting::render_error(&message, &details)
                        }
                    };
                    if sender.send(Message::Text(html.into())).await.is_err() {
                        break;
                    }
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

                                let solve_result = state_clone
                                    .solver_service
                                    .solve_issue_with_ws(
                                        url.to_string(),
                                        state_clone.update_tx.clone(),
                                    )
                                    .await;

                                match solve_result {
                                    Ok(response) => {
                                        let html = super::html_formatting::render_final_solution(&response.solution);

                                        // Try multiple times to send the final message
                                        for _ in 0..3 {
                                            match tx_clone
                                                .send(Message::Text(html.clone().into()))
                                                .await
                                            {
                                                Ok(_) => {
                                                    info!("Successfully sent final solution");
                                                    break;
                                                }
                                                Err(e) => {
                                                    error!("Failed to send final solution: {}", e);
                                                    tokio::time::sleep(
                                                        Duration::from_millis(100),
                                                    )
                                                    .await;
                                                }
                                            }
                                        }

                                        // Wait a bit before closing to ensure all messages are sent
                                        tokio::time::sleep(Duration::from_secs(1)).await;
                                    }
                                    Err(e) => {
                                        error!("Solver error: {}", e);
                                        let error_html = super::html_formatting::render_error("Solver error", &Some(e.to_string()));
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