use super::services::{deepseek::DeepSeekService, github_issue::GitHubService, RepomapService};
use super::tools::create_tools;
use super::ws::transport::WebSocketState;
use crate::{routes, server};
use axum::{
    routing::{get, post},
    Router,
};
use std::{env, sync::Arc};
use tower_http::services::ServeDir;
use tracing::{error, info};

pub fn configure_app() -> Router {
    info!("Configuring application router");

    // Load environment variables
    dotenvy::dotenv().ok();
    info!("Environment variables loaded");

    // Create shared services
    let tool_model = Arc::new(DeepSeekService::new(
        env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set"),
    ));

    let chat_model = Arc::new(DeepSeekService::new(
        env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set"),
    ));

    let github_service = Arc::new(
        GitHubService::new(Some(
            env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set"),
        ))
        .expect("Failed to create GitHub service"),
    );

    info!("Core services created");

    // Create available tools
    let tools = create_tools();
    info!("Tools created");

    // Create WebSocket state with services
    let ws_state = WebSocketState::new(tool_model, chat_model, github_service.clone(), tools);
    info!("WebSocket state initialized");

    // Initialize repomap service
    let aider_api_key = env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
    let repomap_service = Arc::new(RepomapService::new(aider_api_key));
    info!("Repomap service initialized");

    // Create auth state with OIDC config
    info!("Creating OIDC config");
    let oidc_config = match server::services::auth::OIDCConfig::new(
        env::var("OIDC_CLIENT_ID").expect("OIDC_CLIENT_ID must be set"),
        env::var("OIDC_CLIENT_SECRET").expect("OIDC_CLIENT_SECRET must be set"),
        env::var("OIDC_REDIRECT_URI")
            .unwrap_or_else(|_| "http://localhost:8000/auth/callback".to_string()),
        env::var("OIDC_AUTH_URL").expect("OIDC_AUTH_URL must be set"),
        env::var("OIDC_TOKEN_URL").expect("OIDC_TOKEN_URL must be set"),
    ) {
        Ok(config) => {
            info!("OIDC config created successfully");
            config
        }
        Err(e) => {
            error!("Failed to create OIDC config: {}", e);
            panic!("Failed to create OIDC config: {}", e);
        }
    };

    info!("Creating database pool");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    info!("Database URL: {}", database_url);
    let pool = sqlx::PgPool::connect_lazy(&database_url).expect("Failed to create database pool");
    info!("Database pool created successfully");

    let auth_state = server::handlers::AuthState::new(oidc_config, pool);
    info!("Auth state initialized");

    // Create the main router with WebSocket state
    info!("Creating main router");
    let app = Router::new()
        // Main routes
        .route("/", get(routes::home))
        .route("/chat", get(routes::chat))
        .route("/ws", get(server::ws::ws_handler))
        .route("/onyx", get(routes::mobile_app))
        .route("/services", get(routes::business))
        .route("/video-series", get(routes::video_series))
        .route("/company", get(routes::company))
        .route("/coming-soon", get(routes::coming_soon))
        .route("/health", get(routes::health_check))
        .route("/repomap", get(routes::repomap))
        .route("/cota", get(routes::cota))
        // Auth pages
        .route("/login", get(routes::login))
        .route("/signup", get(routes::signup))
        .with_state(ws_state);

    // Create auth router with auth state
    info!("Creating auth router");
    let auth_router = Router::new()
        .route("/auth/login", get(server::handlers::auth::login))
        .route("/auth/signup", post(server::handlers::auth::handle_signup))
        .route("/auth/callback", get(server::handlers::auth::callback))
        .route("/auth/logout", get(server::handlers::auth::logout))
        .with_state(auth_state.clone());

    // Create repomap router with repomap state
    info!("Creating repomap router");
    let repomap_router = Router::new()
        .route("/repomap/generate", post(routes::generate_repomap))
        .with_state(repomap_service);

    // Merge all routers
    info!("Merging routers");
    let app = app
        .merge(auth_router)
        .merge(repomap_router)
        .with_state(auth_state);

    // Static files
    info!("Adding static file services");
    app.nest_service("/assets", ServeDir::new("./assets").precompressed_gzip())
        .nest_service(
            "/templates",
            ServeDir::new("./templates").precompressed_gzip(),
        )
}