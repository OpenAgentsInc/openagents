use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub mod routes;
pub mod services;
pub mod tools;
pub mod ws;

pub fn app() -> Router {
    let cors = CorsLayer::permissive();

    Router::new()
        .merge(routes::routes())
        .layer(cors)
}