use super::services::RepomapService;
use std::env;
use tower_http::services::ServeDir;

pub fn configure_app() -> axum::Router {
    // Initialize repomap service
    let aider_api_key = env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
    let _repomap_service = RepomapService::new(aider_api_key);

    // Create the main router with state
    axum::Router::new()
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
