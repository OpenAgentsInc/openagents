pub mod admin;
pub mod config;
pub mod services;
pub mod ws;

use axum::Router;
use services::solver::SolverService;
use std::sync::Arc;
use ws::{init_websocket_state, ws_routes};

pub fn app_router() -> Router {
    // Create shared services
    let solver_service = Arc::new(SolverService::new());
    
    // Initialize WebSocket state with all handlers
    let ws_state = init_websocket_state();

    // Create base router
    Router::new()
        .merge(ws_routes())
        .with_state(ws_state)
}