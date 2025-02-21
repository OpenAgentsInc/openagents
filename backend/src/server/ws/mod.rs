use axum::{
    extract::{
        ws::WebSocket,
        State, WebSocketUpgrade,
    },
    response::Response,
};
use std::sync::Arc;
use tracing::error;

use crate::server::config::AppState;

pub mod handlers;
pub mod transport;
pub mod types;

use transport::WebSocketTransport;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let transport = WebSocketTransport::new(Arc::new(transport::WebSocketState::new(
        Arc::new(state.ws_state.github_service.clone()),
        Arc::new(state.ws_state.model_router.clone()),
    )), state);

    if let Err(e) = transport.handle_socket(socket, "anonymous".to_string()).await {
        error!("Error handling WebSocket connection: {:?}", e);
    }
}