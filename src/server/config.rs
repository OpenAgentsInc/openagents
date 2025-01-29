use crate::routes;
use axum::{
    routing::{get, post},
    Router,
};
use std::{env, sync::Arc};

pub fn configure_app() -> Router {
    let github_token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
    let deepseek_key = env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set");

    let deepseek = Arc::new(crate::server::services::deepseek::DeepSeekService::new(deepseek_key).unwrap());
    let github = Arc::new(crate::server::services::github_issue::GitHubService::new(Some(github_token)).unwrap());

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
}