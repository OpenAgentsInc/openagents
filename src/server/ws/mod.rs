use axum::{extract::WebSocketUpgrade, response::IntoResponse};
use std::sync::Arc;

pub mod handlers;
pub mod transport;
pub mod types;

use crate::server::services::DeepSeekService;
use transport::WebSocketState;

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: axum::extract::ws::WebSocket) {
    // Initialize DeepSeek service
    let deepseek_api_key = std::env::var("DEEPSEEK_API_KEY")
        .expect("DEEPSEEK_API_KEY must be set");
    let deepseek_service = Arc::new(DeepSeekService::new(deepseek_api_key));

    // Create WebSocketState with DeepSeek service
    let ws_state = WebSocketState::new(deepseek_service);

    // Create handlers using the state
    let (chat_handler, solver_handler) = WebSocketState::create_handlers(ws_state.clone());

    // Handle the socket with all components
    ws_state
        .handle_socket(socket, chat_handler, solver_handler)
        .await;
}