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
    let transport = WebSocketTransport::new(state.ws_state.clone(), state);
    
    match transport.handle_socket(socket, "anonymous".to_string()).await {
        Ok(_) => {
            info!("WebSocket connection closed normally");
        }
        Err(e) => {
            error!("WebSocket error: {:?}", e);
            // Error is already handled in transport layer
            // Including sending error message to client and cleaning up state
        }
    }
}