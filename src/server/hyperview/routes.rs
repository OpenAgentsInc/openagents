use super::{handlers, ws};
use crate::server::config::AppState;
use axum::{routing::get, Router};

pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route("/hyperview/main", get(handlers::main_screen))
        .route("/hyperview/fragments/user-info", get(handlers::user_info))
        .route("/hyperview/fragments/github-repos", get(handlers::github_repos))
        .route(
            "/hyperview/fragments/connected-status",
            get(handlers::connected_status),
        )
        .route(
            "/hyperview/fragments/disconnected-status",
            get(handlers::disconnected_status),
        )
        .route("/hyperview/ws", get(ws::hyperview_ws_handler))
        .route("/hyperview/fragments/content", get(handlers::content))
}
