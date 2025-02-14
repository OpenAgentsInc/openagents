use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    http::Request,
    response::IntoResponse,
};
use axum_extra::extract::cookie::CookieJar;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tracing::{error, info};
use std::collections::HashMap;
use url::form_urlencoded;

use crate::server::config::AppState;
use crate::server::ws::{handlers::MessageHandler, transport::WebSocketState, types::ChatMessage};

#[axum::debug_handler]
pub async fn hyperview_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
) -> impl IntoResponse {
    info!("Handling WebSocket upgrade request");

    // Log headers for debugging
    info!("Request headers: {:?}", request.headers());

    // Get session token from either cookie or query param
    let session_token: Option<String> = {
        // First try cookie
        let jar = CookieJar::from_headers(request.headers());
        let cookie_token = jar.get("session").map(|c| c.value().to_string());
        if let Some(ref _token) = cookie_token {
            info!("Found session token in cookie");
        } else {
            info!("No session token found in cookie, checking query params");
        }

        // If no cookie, try query param
        if cookie_token.is_none() {
            let query_params = request.uri().query().unwrap_or("");
            info!("Query parameters: {}", query_params);

            let params: HashMap<String, String> = form_urlencoded::parse(query_params.as_bytes())
                .into_iter()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();

            info!("Parsed query parameters: {:?}", params);
            let token = params.get("session").cloned();
            if token.is_some() {
                info!("Found session token in query parameters");
            } else {
                info!("No session token found in query parameters");
            }
            token
        } else {
            cookie_token
        }
    };

    info!("Session token extraction result: {}", session_token.is_some());

    // Validate session token and get user_id
    match session_token {
        Some(token) => {
            info!("Attempting to validate session token");
            match state.ws_state.validate_session_token(&token).await {
                Ok(user_id) => {
                    info!(
                        "Hyperview WebSocket connection authenticated for user {}",
                        user_id
                    );

                    // Create chat handler
                    let chat_handler = state.ws_state.create_handlers();
                    info!("Created chat handler for user {}", user_id);

                    // Upgrade connection with user_id
                    info!("Upgrading WebSocket connection for user {}", user_id);
                    ws.on_upgrade(move |socket| {
                        handle_socket(socket, state.ws_state, chat_handler, user_id)
                    })
                }
                Err(e) => {
                    error!("Hyperview WebSocket token validation failed: {}", e);
                    error!("Token validation error details: {:?}", e);
                    (axum::http::StatusCode::UNAUTHORIZED, "Invalid session token").into_response()
                }
            }
        }
        None => {
            error!("No session token found in cookie or query params");
            error!("Request URI: {}", request.uri());
            error!("Available cookies: {:?}", CookieJar::from_headers(request.headers()));
            (axum::http::StatusCode::UNAUTHORIZED, "No session token provided").into_response()
        }
    }
}

async fn handle_socket(
    socket: WebSocket,
    state: Arc<WebSocketState>,
    chat_handler: Arc<crate::server::ws::handlers::chat::ChatHandler>,
    user_id: i32,
) {
    info!(
        "Handling Hyperview WebSocket connection for user {}",
        user_id
    );

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Generate unique connection ID
    let conn_id = uuid::Uuid::new_v4().to_string();
    info!(
        "New Hyperview WebSocket connection established: {}",
        conn_id
    );

    // Store connection with user ID
    state.add_connection(&conn_id, user_id, tx.clone()).await;
    info!("Connection stored for user {}: {}", user_id, conn_id);

    // Send connected status
    let connected_xml = r###"<?xml version="1.0" encoding="UTF-8"?>
<view xmlns="https://hyperview.org/hyperview" style="message">
  <text style="messageText">Connected to chat server</text>
</view>"###;

    if let Err(e) = tx.send(axum::extract::ws::Message::Text(connected_xml.into())) {
        error!("Failed to send connected status: {}", e);
    }

    // Handle incoming messages
    let receive_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            if let axum::extract::ws::Message::Text(text) = message {
                info!("Received message from {}: {}", conn_id, text);

                // Parse the message
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(message_type) = data.get("type") {
                        match message_type.as_str() {
                            Some("solve_demo_repo") => {
                                info!("Handling solve_demo_repo request");

                                // Send initial status
                                let status_xml = r###"<?xml version="1.0" encoding="UTF-8"?>
<view xmlns="https://hyperview.org/hyperview" style="message">
  <text style="messageText">Starting demo repo analysis...</text>
</view>"###;

                                if let Err(e) = tx.send(axum::extract::ws::Message::Text(status_xml.into())) {
                                    error!("Failed to send status message: {}", e);
                                }

                                // TODO: Implement actual demo repo solving logic
                                // For now, just send a mock response
                                let result_xml = r###"<?xml version="1.0" encoding="UTF-8"?>
<view xmlns="https://hyperview.org/hyperview" style="message">
  <text style="messageText">Demo repo analysis complete!</text>
  <text style="messageText">Found 3 issues to solve.</text>
</view>"###;

                                if let Err(e) = tx.send(axum::extract::ws::Message::Text(result_xml.into())) {
                                    error!("Failed to send result message: {}", e);
                                }
                            }
                            _ => {
                                if let Some(content) = data.get("content") {
                                    if let Some(content_str) = content.as_str() {
                                        // Create chat message
                                        let chat_msg = ChatMessage::UserMessage {
                                            content: content_str.to_string(),
                                        };

                                        // Handle the message
                                        if let Err(e) =
                                            chat_handler.handle_message(chat_msg, conn_id.clone()).await
                                        {
                                            error!("Error handling chat message: {}", e);

                                            // Send error response as HXML
                                            let error_xml = format!(
                                                r###"<?xml version="1.0" encoding="UTF-8"?>
<view xmlns="https://hyperview.org/hyperview" style="message">
  <text style="messageText">Error: {}</text>
</view>"###,
                                                e
                                            );

                                            if let Err(e) =
                                                tx.send(axum::extract::ws::Message::Text(error_xml.into()))
                                            {
                                                error!("Failed to send error message: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        info!("WebSocket connection closed: {}", conn_id);
    });

    // Handle outgoing messages
    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Err(e) = sender.send(message).await {
                error!("Failed to send message: {}", e);
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = receive_task => info!("Receive task completed"),
        _ = send_task => info!("Send task completed"),
    }
}
