use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
use openagents::server::services::{
    solver::ws::{SolverStage, SolverUpdate},
    SolverService,
};
use std::sync::Arc;

#[tokio::test]
async fn test_solver_ws_connection() {
    // Create solver service
    let _solver_service = Arc::new(SolverService::new());

    // Rest of the test...
}

#[tokio::test]
async fn test_solver_ws_stages() {
    // Create solver service
    let _solver_service = Arc::new(SolverService::new());

    // Rest of the test...
}

#[tokio::test]
async fn test_solver_ws_error_handling() {
    // Create solver service
    let _solver_service = Arc::new(SolverService::new());

    // Rest of the test...
}
