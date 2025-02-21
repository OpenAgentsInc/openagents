use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
};
use tracing::{error, info};

use crate::server::config::AppState;

pub mod handlers;
pub mod transport;
pub mod types;

use transport::WebSocketTransport;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    info!("WebSocket upgrade request received");
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    // Create transport with existing ws_state
    let transport = WebSocketTransport::new(state.ws_state.clone(), state);

    if let Err(e) = transport
        .handle_socket(socket, "anonymous".to_string())
        .await
    {
        error!("Error handling WebSocket connection: {:?}", e);
    }
}