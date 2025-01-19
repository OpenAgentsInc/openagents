use axum::{
    extract::ws::{Message, WebSocket},
    http::StatusCode,
};
use futures::{SinkExt, StreamExt};
use openagents::server::services::{
    solver::SolverService,
    solver_ws::{SolverStage, SolverUpdate, SolverWsState},
};
use std::{env, sync::Arc};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message as WsMessage;

#[tokio::test]
async fn test_solver_ws_updates() {
    // Set up environment
    env::set_var("AIDER_API_KEY", "test_key");
    env::set_var("OPENROUTER_API_KEY", "test_key");
    env::set_var("GITHUB_TOKEN", "test_key");

    // Create solver service and state
    let solver_service = Arc::new(SolverService::new());
    let ws_state = Arc::new(SolverWsState::new(solver_service.clone()));

    // Create a test WebSocket connection
    let (client_socket, server_socket) = tokio::io::duplex(64);
    let client_socket = tokio_tungstenite::WebSocketStream::from_raw_socket(
        client_socket,
        tokio_tungstenite::tungstenite::protocol::Role::Client,
        None,
    )
    .await;
    let server_socket = tokio_tungstenite::WebSocketStream::from_raw_socket(
        server_socket,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    )
    .await;

    // Start WebSocket handler
    let ws_state_clone = ws_state.clone();
    tokio::spawn(async move {
        handle_test_socket(server_socket, ws_state_clone).await;
    });

    // Send a test message
    let test_msg = serde_json::json!({
        "action": "start",
        "issue_url": "https://github.com/test/repo/issues/1"
    });
    client_socket
        .send(WsMessage::Text(test_msg.to_string()))
        .await
        .unwrap();

    // Wait for updates
    let mut messages = Vec::new();
    while let Some(msg) = client_socket.next().await {
        let msg = msg.unwrap();
        if let WsMessage::Text(text) = msg {
            messages.push(text);
            if text.contains("Complete") || text.contains("Error") {
                break;
            }
        }
    }

    // Verify we got the expected sequence of updates
    assert!(messages.iter().any(|m| m.contains("Init")));
    assert!(messages.iter().any(|m| m.contains("Repomap")));
    assert!(messages.iter().any(|m| m.contains("Analysis")));
    assert!(messages.iter().any(|m| m.contains("Solution")));
}

#[tokio::test]
async fn test_solver_ws_error_handling() {
    // Set up environment with invalid keys to trigger errors
    env::set_var("AIDER_API_KEY", "invalid_key");
    env::set_var("OPENROUTER_API_KEY", "invalid_key");
    env::set_var("GITHUB_TOKEN", "invalid_key");

    // Create solver service and state
    let solver_service = Arc::new(SolverService::new());
    let ws_state = Arc::new(SolverWsState::new(solver_service.clone()));

    // Create a test WebSocket connection
    let (client_socket, server_socket) = tokio::io::duplex(64);
    let client_socket = tokio_tungstenite::WebSocketStream::from_raw_socket(
        client_socket,
        tokio_tungstenite::tungstenite::protocol::Role::Client,
        None,
    )
    .await;
    let server_socket = tokio_tungstenite::WebSocketStream::from_raw_socket(
        server_socket,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    )
    .await;

    // Start WebSocket handler
    let ws_state_clone = ws_state.clone();
    tokio::spawn(async move {
        handle_test_socket(server_socket, ws_state_clone).await;
    });

    // Send a test message with invalid URL
    let test_msg = serde_json::json!({
        "action": "start",
        "issue_url": "invalid_url"
    });
    client_socket
        .send(WsMessage::Text(test_msg.to_string()))
        .await
        .unwrap();

    // Wait for error message
    let mut received_error = false;
    while let Some(msg) = client_socket.next().await {
        let msg = msg.unwrap();
        if let WsMessage::Text(text) = msg {
            if text.contains("Error") {
                received_error = true;
                break;
            }
        }
    }

    assert!(received_error);
}

#[tokio::test]
async fn test_solver_ws_broadcast() {
    // Create solver service and state
    let solver_service = Arc::new(SolverService::new());
    let ws_state = Arc::new(SolverWsState::new(solver_service.clone()));

    // Create multiple test connections
    let mut client_sockets = Vec::new();
    let mut server_sockets = Vec::new();
    for _ in 0..3 {
        let (client_socket, server_socket) = tokio::io::duplex(64);
        let client_socket = tokio_tungstenite::WebSocketStream::from_raw_socket(
            client_socket,
            tokio_tungstenite::tungstenite::protocol::Role::Client,
            None,
        )
        .await;
        let server_socket = tokio_tungstenite::WebSocketStream::from_raw_socket(
            server_socket,
            tokio_tungstenite::tungstenite::protocol::Role::Server,
            None,
        )
        .await;

        client_sockets.push(client_socket);
        server_sockets.push(server_socket);
    }

    // Start handlers for each connection
    for server_socket in server_sockets {
        let ws_state_clone = ws_state.clone();
        tokio::spawn(async move {
            handle_test_socket(server_socket, ws_state_clone).await;
        });
    }

    // Broadcast a test update
    ws_state
        .broadcast_update(SolverUpdate::Progress {
            stage: SolverStage::Init,
            message: "Test broadcast".into(),
            data: None,
        })
        .await;

    // Verify all clients received the update
    for mut client_socket in client_sockets {
        let mut received_broadcast = false;
        while let Some(msg) = client_socket.next().await {
            let msg = msg.unwrap();
            if let WsMessage::Text(text) = msg {
                if text.contains("Test broadcast") {
                    received_broadcast = true;
                    break;
                }
            }
        }
        assert!(received_broadcast);
    }
}

async fn handle_test_socket(socket: impl StreamExt<Item = Result<WsMessage, std::io::Error>> + SinkExt<WsMessage>, state: Arc<SolverWsState>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let (mut sender, mut receiver) = socket.split();

    // Forward messages from channel to socket
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            WsMessage::Text(text) => {
                if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(action) = cmd.get("action") {
                        if action == "start" {
                            if let Some(issue_url) = cmd.get("issue_url").and_then(|v| v.as_str()) {
                                state
                                    .solver_service
                                    .solve_issue_with_ws(issue_url.to_string(), state.update_tx.clone())
                                    .await
                                    .ok();
                            }
                        }
                    }
                }
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }
}