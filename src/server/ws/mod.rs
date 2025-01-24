use std::sync::Arc;

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::IntoResponse,
};

use self::transport::WebSocketState;

pub mod handlers;
pub mod transport;
pub mod types;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebSocketState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<WebSocketState>) {
    // Create chat handler
    let chat_handler = WebSocketState::create_handlers(state.clone());

    // Handle socket
    state.handle_socket(socket, chat_handler).await;
}