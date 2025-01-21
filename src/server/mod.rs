pub mod admin;
pub mod config;
pub mod services;

use axum::{routing::get, Router};
use services::{
    solver::ws::{ws_handler, SolverWsState},
    solver::SolverService,
};
use std::sync::Arc;

pub fn app_router() -> Router {
    // Create shared solver service and WebSocket state
    let solver_service = Arc::new(SolverService::new());
    let ws_state = Arc::new(SolverWsState::new(solver_service.clone()));

    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(ws_state)
}
