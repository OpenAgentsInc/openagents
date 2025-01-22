pub mod admin;
pub mod config;
pub mod services;
pub mod ws;

use axum::Router;
use ws::{init_websocket_state, ws_routes};

pub fn app_router() -> Router {
    // Initialize WebSocket state with all handlers
    let ws_state = init_websocket_state();

    // Create base router
    Router::new()
        .merge(ws_routes())
        .with_state(ws_state)
}
