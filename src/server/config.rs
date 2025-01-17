use std::env;
use tower_http::services::ServeDir;

use super::{admin::middleware::admin_auth, services::RepomapService};

pub fn configure_app() -> axum::Router {
    // Initialize repomap service
    let aider_api_key = env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
    let repomap_service = RepomapService::new(aider_api_key);

    // Create the main router with state
    axum::Router::with_state(repomap_service)
        // Admin routes with authentication
        .nest(
            "/admin",
            super::admin::routes::admin_routes().layer(axum::middleware::from_fn(admin_auth)),
        )
        // Static files
        .nest_service(
            "/static",
            ServeDir::new("./static").precompressed_gzip(),
        )
        // Template files
        .nest_service(
            "/templates",
            ServeDir::new("./templates").precompressed_gzip(),
        )
}