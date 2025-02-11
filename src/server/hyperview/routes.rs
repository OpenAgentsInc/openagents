use axum::{
    routing::get,
    Router,
};

use crate::server::config::AppState;

use super::{handlers, ws::hyperview_ws_handler};

pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route("/hyperview", get(handlers::hello_world))
        .route("/hyperview/main", get(handlers::hello_world))
        .route("/hyperview/repositories", get(handlers::repositories_screen))
        .route("/hyperview/ws", get(hyperview_ws_handler))
}