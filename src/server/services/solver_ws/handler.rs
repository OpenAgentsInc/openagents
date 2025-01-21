use std::{sync::Arc, time::Duration};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::mpsc;
use tracing::{error, info};
use bytes::Bytes;

use super::state::SolverWsState;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

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
                            super::html::render_progress_bar(&stage, &message)
                        }
                        SolverUpdate::Complete { result } => {
                            super::html::render_complete(&result)
                        }
                        SolverUpdate::Error { message, details } => {
                            super::html::render_error(&message, &details)
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
                                        let html = super::html::render_final_solution(&response.solution);

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
                                        let error_html = super::html::render_error("Solver error", &Some(e.to_string()));
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