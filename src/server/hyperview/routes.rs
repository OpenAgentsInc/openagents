use super::{handlers, ws};
use crate::server::config::AppState;
use axum::{routing::get, Router};

pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route("/hyperview/main", get(handlers::main_page))
        .route("/hyperview/login", get(handlers::login_page))
        .route("/hyperview/mobile/logout", get(handlers::mobile_logout))
        .route("/hyperview/user", get(handlers::user_info))
        .route(
            "/hyperview/status/connected",
            get(handlers::connected_status),
        )
        .route(
            "/hyperview/status/disconnected",
            get(handlers::disconnected_status),
        )
        .route(
            "/hyperview/repo/{owner}/{repo}/repomap",
            get(handlers::generate_repomap),
        )
        .route(
            "/hyperview/repo/{owner}/{repo}/content/{*path}",
            get(handlers::content),
        )
        .route("/hyperview/repos", get(handlers::github_repos))
        .route(
            "/hyperview/repo/{owner}/{repo}/issues",
            get(handlers::github_issues),
        )
        .route(
            "/hyperview/repo/{owner}/{repo}/issues/{number}/analyze",
            get(handlers::analyze_issue),
        )
        .route(
            "/hyperview/solver/{solver_id}/status",
            get(handlers::solver_status),
        )
        .route("/hyperview/ws", get(ws::hyperview_ws_handler))
        .route("/hyperview/fragments/user-info", get(handlers::user_info))
        .route(
            "/hyperview/fragments/github-repos",
            get(handlers::github_repos),
        )
        .route("/hyperview/fragments/content", get(handlers::content))
        .route("/templates/pages/auth/login.xml", get(handlers::login_page))
        .route("/templates/pages/main.xml", get(handlers::main_page))
}
