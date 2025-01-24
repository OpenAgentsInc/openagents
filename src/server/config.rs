use super::services::{DeepSeekService, GitHubService, RepomapService};
use super::ws::handlers::chat::ChatHandler;
use super::ws::transport::WebSocketState;
use axum::Router;
use std::{env, sync::Arc};
use tower_http::services::ServeDir;

pub fn configure_app() -> Router {
    // Create shared services
    let deepseek_service = Arc::new(DeepSeekService::new(
        env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set"),
    ));

    let github_service = Arc::new(GitHubService::new(Some(
        env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set"),
    )));

    let repomap_service = Arc::new(RepomapService::new(
        env::var("FIRECRAWL_API_KEY").expect("FIRECRAWL_API_KEY must be set"),
    ));

    // Create WebSocket state with services
    let ws_state = WebSocketState::new(
        deepseek_service.clone(),
        github_service.clone(),
    );
    let ws_state = Arc::new(ws_state);

    // Create chat handler
    let chat_handler = ChatHandler::new(
        ws_state,
        deepseek_service.clone(),
        github_service.clone(),
    );

    // Create the main router
    Router::new()
        .route("/", axum::routing::get(|| async { "Hello, World!" }))
        // Static files
        .nest_service("/static", ServeDir::new("./static").precompressed_gzip())
        // Template files
        .nest_service(
            "/templates",
            ServeDir::new("./templates").precompressed_gzip(),
        )
        .with_state(())
}