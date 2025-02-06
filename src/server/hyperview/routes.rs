use axum::{
    Router,
    routing::get,
};
use crate::server::config::AppState;
use super::handlers;

pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route("/hyperview", get(handlers::hello_world))
}