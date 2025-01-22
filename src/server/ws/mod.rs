use axum::{
    extract::WebSocketUpgrade,
    response::IntoResponse,
};

pub mod handlers;
pub mod transport;
pub mod types;

use transport::WebSocketState;

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: axum::extract::ws::WebSocket) {
    // Create WebSocketState first
    let ws_state = WebSocketState::new();
    
    // Create handlers using the state
    let (chat_handler, solver_handler) = WebSocketState::create_handlers(ws_state.clone());
    
    // Handle the socket with all components
    ws_state.handle_socket(socket, chat_handler, solver_handler).await;
}