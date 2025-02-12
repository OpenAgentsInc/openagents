use crate::server::config::AppState;
use crate::server::hyperview::handlers::{generate_repomap, github_repos};
use axum::{routing::get, Router};

pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/hyperview/fragments/github-repos",
            get(github_repos),
        )
        .route(
            "/hyperview/repo/:owner/:repo/repomap",
            get(generate_repomap),
        )
}