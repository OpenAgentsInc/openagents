use super::handlers;
use crate::server::config::AppState;
use axum::{routing::get, Router};

pub fn hyperview_routes() -> Router<AppState> {
    Router::new().route("/hyperview", get(handlers::hello_world))
}
