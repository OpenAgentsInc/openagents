use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{Request, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::cookie::CookieJar;
use futures_util::{SinkExt, StreamExt};
use serde_json;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

use self::handlers::MessageHandler;
use self::transport::WebSocketState;
use self::types::ChatMessage;
use crate::server::config::AppState;

pub mod handlers;
pub mod transport;
pub mod types;

// Add the debug handler attribute to improve error messages
#[axum::debug_handler]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
) -> impl IntoResponse {
    let jar = CookieJar::from_headers(request.headers());

    match state.ws_state.validate_session(&jar, request).await {
        Ok(user_id) => {
            info!(
                "WebSocket connection authenticated for user_id: {}",
                user_id
            );
            // Create handlers
            let (chat_handler, solver_handler) = state.ws_state.create_handlers();
            ws.on_upgrade(move |socket| {
                handle_socket(
                    socket,
                    state.ws_state,
                    chat_handler,
                    solver_handler,
                    user_id,
                )
            })
        }
        Err(e) => {
            warn!("WebSocket authentication failed: {:?}", e);
            (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
        }
    }
}

async fn handle_socket(
    socket: WebSocket,
    state: Arc<WebSocketState>,
    chat_handler: Arc<handlers::chat::ChatHandler>,
    solver_handler: Arc<handlers::solver_json::SolverJsonHandler>,
    user_id: i32,
) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Generate unique connection ID
    let conn_id = Uuid::new_v4().to_string();
    info!("New WebSocket connection established: {}", conn_id);

    // Store connection with user ID
    state.add_connection(&conn_id, user_id, tx.clone()).await;
    info!("Connection stored for user {}: {}", user_id, conn_id);

    // Handle outgoing messages
    let ws_state = state.clone();
    let send_conn_id = conn_id.clone();
    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            info!("Sending message to client {}: {:?}", send_conn_id, message);
            if sender.send(message).await.is_err() {
                error!("Failed to send message to client {}", send_conn_id);
                break;
            }
        }
        // Connection closed, remove from state
        ws_state.remove_connection(&send_conn_id).await;
        info!("Connection removed: {}", send_conn_id);
    });

    // Handle incoming messages
    let receive_conn_id = conn_id.clone();
    let receive_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            if let Message::Text(text) = message {
                info!("Raw WebSocket message received: {}", text);

                // Parse the message
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                    info!("Parsed message: {:?}", data);

                    // Try to extract message type
                    if let Some(message_type) = data.get("type") {
                        match message_type.as_str() {
                            Some("chat") => {
                                info!("Processing chat message");
                                if let Some(message) = data.get("message") {
                                    if let Ok(chat_msg) = serde_json::from_value(message.clone()) {
                                        info!("Parsed chat message: {:?}", chat_msg);
                                        if let Err(e) = chat_handler
                                            .handle_message(chat_msg, receive_conn_id.clone())
                                            .await
                                        {
                                            error!("Error handling chat message: {}", e);
                                        }
                                    }
                                }
                            }
                            Some("solve_demo_repo") | Some("solve_repo") | Some("solver") => {
                                info!("Processing solver message");
                                if let Ok(solver_msg) = serde_json::from_value(data.clone()) {
                                    info!("Parsed solver message: {:?}", solver_msg);
                                    if let Err(e) = solver_handler
                                        .handle_message(
                                            solver_msg,
                                            state.clone(),
                                            receive_conn_id.clone(),
                                        )
                                        .await
                                    {
                                        error!("Error handling solver message: {}", e);
                                    }
                                }
                            }
                            _ => {
                                error!("Unknown message type: {:?}", message_type);
                            }
                        }
                    } else {
                        // Try to extract content directly if message type is missing
                        if let Some(content) = data.get("content") {
                            info!("Found direct content: {:?}", content);
                            if let Some(content_str) = content.as_str() {
                                // Create a chat message manually
                                let chat_msg = ChatMessage::UserMessage {
                                    content: content_str.to_string(),
                                };
                                info!("Created chat message: {:?}", chat_msg);
                                if let Err(e) = chat_handler
                                    .handle_message(chat_msg, receive_conn_id.clone())
                                    .await
                                {
                                    error!("Error handling chat message: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    let final_conn_id = conn_id.clone();
    tokio::select! {
        _ = send_task => {
            info!("Send task completed for {}", final_conn_id);
        },
        _ = receive_task => {
            info!("Receive task completed for {}", final_conn_id);
        },
    }
}
