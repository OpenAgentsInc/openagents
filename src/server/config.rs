use axum::Router;
use sqlx::PgPool;

use crate::server::{
    handlers::auth::AuthState,
    services::auth::OIDCConfig,
};

#[derive(Clone)]
pub struct AppConfig {
    pub oidc_auth_url: String,
    pub oidc_token_url: String,
    pub oidc_client_id: String,
    pub oidc_client_secret: String,
    pub oidc_redirect_uri: String,
    pub database_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            oidc_auth_url: std::env::var("OIDC_AUTH_URL").expect("OIDC_AUTH_URL must be set"),
            oidc_token_url: std::env::var("OIDC_TOKEN_URL").expect("OIDC_TOKEN_URL must be set"),
            oidc_client_id: std::env::var("OIDC_CLIENT_ID").expect("OIDC_CLIENT_ID must be set"),
            oidc_client_secret: std::env::var("OIDC_CLIENT_SECRET").expect("OIDC_CLIENT_SECRET must be set"),
            oidc_redirect_uri: std::env::var("OIDC_REDIRECT_URI").expect("OIDC_REDIRECT_URI must be set"),
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
        }
    }
}

pub fn configure_app() -> Router {
    configure_app_with_config(None)
}

pub fn configure_app_with_config(config: Option<AppConfig>) -> Router {
    let config = config.unwrap_or_else(AppConfig::default);

    let pool = PgPool::connect_lazy(&config.database_url)
        .expect("Failed to create database pool");

    let oidc_config = OIDCConfig::new(
        config.oidc_client_id,
        config.oidc_client_secret,
        config.oidc_redirect_uri,
        config.oidc_auth_url,
        config.oidc_token_url,
    ).expect("Failed to create OIDC config");

    let auth_state = AuthState::new(oidc_config, pool.clone());

    Router::new()
        .route("/auth/login", axum::routing::get(crate::server::handlers::auth::login))
        .route("/auth/signup", axum::routing::get(crate::server::handlers::auth::signup))
        .route("/auth/callback", axum::routing::get(crate::server::handlers::auth::callback))
        .route("/auth/logout", axum::routing::get(crate::server::handlers::auth::logout))
        .with_state(auth_state)
}