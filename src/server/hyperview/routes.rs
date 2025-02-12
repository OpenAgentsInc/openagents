use super::handlers;
use crate::server::config::AppState;
use axum::{routing::get, Router};

pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/hyperview/fragments/github-repos",
            get(handlers::github_repos),
        )
        .route(
            "/hyperview/repo/:owner/:repo/repomap",
            get(handlers::generate_repomap),
        )
}