pub mod admin;
pub mod config;
pub mod services;
pub mod ws;

use axum::{routing::get, Router};
use services::solver::SolverService;
use std::sync::Arc;
use ws::{ws_handler, init_websocket_state};

pub fn app_router() -> Router {
    // Create shared services
    let solver_service = Arc::new(SolverService::new());
    
    // Initialize WebSocket state with all handlers
    let ws_state = init_websocket_state();

    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(ws_state)
}