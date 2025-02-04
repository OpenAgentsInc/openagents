use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use tower_http::trace::TraceLayer;

use crate::server::{
    handlers::{callback, login, logout, signup, AppState},
    services::auth::OIDCConfig,
};

pub fn configure_app(pool: PgPool, oidc_config: OIDCConfig) -> Router {
    let state = AppState::new(oidc_config, pool);

    Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/auth/login", get(login))
        .route("/auth/signup", post(signup))
        .route("/auth/callback", get(callback))
        .route("/auth/logout", get(logout))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}