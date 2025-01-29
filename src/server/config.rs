use super::services::{deepseek::DeepSeekService, github_issue::GitHubService, repomap::RepomapService};
use crate::routes;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn configure_app() -> Router {
    let deepseek = Arc::new(DeepSeekService::new().unwrap());
    let github = Arc::new(GitHubService::new().unwrap());
    let repomap = Arc::new(RepomapService::new());

    Router::new()
        .route("/", get(routes::home))
        .route("/login", get(routes::login))
        .route("/signup", get(routes::signup))
        .route("/chat", get(routes::chat))
        .route("/onyx", get(routes::mobile_app))
        .route("/services", get(routes::business))
        .route("/video-series", get(routes::video_series))
        .route("/company", get(routes::company))
        .route("/coming-soon", get(routes::coming_soon))
        .route("/repomap", get(routes::repomap))
        .route("/repomap/generate", post(routes::generate_repomap))
        .with_state(deepseek)
        .with_state(github)
        .with_state(repomap)
}