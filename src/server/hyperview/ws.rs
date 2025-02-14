use std::sync::Arc;
use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade, Message},
        State,
    },
    response::IntoResponse,
    http::{Request, StatusCode},
};
use axum_extra::extract::cookie::CookieJar;
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;
use serde_json;

use crate::server::config::AppState;
use crate::server::ws::transport::WebSocketState;
use crate::server::ws::handlers::MessageHandler;

#[axum::debug_handler]
pub async fn hyperview_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
) -> impl IntoResponse {
    let jar = CookieJar::from_headers(request.headers());

    match state.ws_state.validate_session(&jar, request).await {
        Ok(user_id) => {
            info!("Hyperview WebSocket connection authenticated for user_id: {}", user_id);
            // Create handlers
            let (chat_handler, solver_handler) = state.ws_state.create_handlers();
            ws.on_upgrade(move |socket| handle_socket(socket, state.ws_state, chat_handler, solver_handler, user_id))
        }
        Err(e) => {
            warn!("Hyperview WebSocket authentication failed: {:?}", e);
            (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
        }
    }
}

async fn handle_socket(
    socket: WebSocket,
    state: Arc<WebSocketState>,
    chat_handler: Arc<crate::server::ws::handlers::chat::ChatHandler>,
    solver_handler: Arc<crate::server::ws::handlers::solver_json::SolverJsonHandler>,
    user_id: i32,
) {
    // Handle the WebSocket connection
    info!("New hyperview WebSocket connection established for user_id: {}", user_id);

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Generate unique connection ID
    let conn_id = Uuid::new_v4().to_string();

    // Store connection with user ID
    state.add_connection(&conn_id, user_id, tx.clone()).await;

    // Handle outgoing messages
    let ws_state = state.clone();
    let send_conn_id = conn_id.clone();
    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
        // Connection closed, remove from state
        ws_state.remove_connection(&send_conn_id).await;
    });

    // Handle incoming messages
    let receive_conn_id = conn_id.clone();
    let receive_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            if let Message::Text(text) = message {
                // Parse the message
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                    // Try to extract message type
                    if let Some(message_type) = data.get("type") {
                        match message_type.as_str() {
                            Some("chat") => {
                                if let Some(message) = data.get("message") {
                                    if let Ok(chat_msg) = serde_json::from_value(message.clone()) {
                                        if let Err(e) = chat_handler
                                            .handle_message(chat_msg, receive_conn_id.clone())
                                            .await
                                        {
                                            error!("Error handling chat message: {}", e);
                                        }
                                    }
                                }
                            }
                            Some("solver") => {
                                if let Ok(solver_msg) = serde_json::from_value(data.clone()) {
                                    if let Err(e) = solver_handler
                                        .handle_message(solver_msg, state.clone(), receive_conn_id.clone())
                                        .await
                                    {
                                        error!("Error handling solver message: {}", e);
                                    }
                                }
                            }
                            _ => {
                                error!("Unknown message type");
                            }
                        }
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = receive_task => {},
    }
}
