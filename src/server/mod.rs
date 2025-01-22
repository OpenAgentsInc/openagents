pub mod admin;
pub mod config;
pub mod routes;
pub mod services;
pub mod tools;
pub mod ws;

use axum::{routing::get, Router};

pub fn app_router() -> Router {
    // Create base router
    Router::new()
        .route("/ws", get(ws::ws_handler))
        .merge(routes::routes())
}