use super::services::{
    deepseek::DeepSeekService,
    github_auth::{GitHubAuthService, GitHubConfig},
    github_issue::GitHubService,
    RepomapService,
};
use super::tools::create_tools;
use super::ws::transport::WebSocketState;
use crate::{routes, server};
use axum::{
    routing::{get, post},
    Router,
    http::{Method, HeaderName, HeaderValue},
};
use std::{env, sync::Arc};
use tower_http::{
    services::ServeDir,
    cors::{CorsLayer},
};

// ... [previous AppState and AppConfig code remains the same] ...

pub fn configure_app_with_config(config: Option<AppConfig>) -> Router {
    // ... [previous service setup code remains the same] ...

    // Configure CORS with specific origins
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
            "http://localhost:8000".parse::<HeaderValue>().unwrap(),
            "https://openagents.com".parse::<HeaderValue>().unwrap(),
            "onyx://".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::OPTIONS,
        ])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("accept"),
        ])
        .allow_credentials(true);

    // Create the main router
    Router::new()
        // ... [rest of the router configuration remains the same] ...
        .layer(cors)
        .with_state(app_state)
}