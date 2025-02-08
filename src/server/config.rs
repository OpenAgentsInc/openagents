use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

pub fn create_router() -> Router {
    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_credentials(true);

    Router::new()
        .route("/auth/logout", get(handlers::auth::clear_session_and_redirect))
        .route("/auth/github/login", get(handlers::auth::handle_github_login))
        .route("/auth/github/callback", get(handlers::auth::handle_github_callback))
        .layer(cors)
}