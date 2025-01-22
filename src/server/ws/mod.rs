use std::sync::Arc;
use axum::{
    extract::ws::WebSocket,
    response::IntoResponse,
};

pub mod handlers;
pub mod transport;
pub mod types;

use handlers::{
    chat::ChatHandler,
    solver::SolverHandler
};
use transport::WebSocketState;

pub async fn ws_handler(ws: WebSocket) -> impl IntoResponse {
    handle_socket(ws).await;
    axum::response::Response::new(axum::body::Body::empty())
}

async fn handle_socket(socket: WebSocket) {
    // Create WebSocketState first
    let ws_state = WebSocketState::new();
    
    // Create handlers using the state
    let (chat_handler, solver_handler) = WebSocketState::create_handlers(ws_state.clone());
    
    // Handle the socket with all components
    ws_state.handle_socket(socket, chat_handler, solver_handler).await;
}