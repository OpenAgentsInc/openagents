use axum::{
    extract::ws::WebSocketUpgrade,
    response::Response,
};
use tracing::{error, info};

use crate::server::config::AppState;

pub mod handlers;
pub mod transport;
pub mod types;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
    info!("WebSocket upgrade request received");
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = transport::WebSocketTransport::new(state.ws_state.clone(), state)
            .handle_socket(socket, "anonymous".to_string())
            .await
        {
            error!("WebSocket error: {:?}", e);
        }
    })
}