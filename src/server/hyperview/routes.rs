use super::{handlers, ws};
use crate::server::{config::AppState, handlers::auth::session};
use axum::{routing::get, Router};

pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route("/hyperview", get(handlers::hello_world))
        .route("/hyperview/main", get(handlers::main_screen))
        .route(
            "/hyperview/fragments/connected-status",
            get(handlers::connected_status),
        )
        .route(
            "/hyperview/fragments/disconnected-status",
            get(handlers::disconnected_status),
        )
        .route("/hyperview/ws", get(ws::hyperview_ws_handler))
        .route("/auth/logout", get(session::clear_session_and_redirect))
}