use super::services::{
    deepseek::DeepSeekService,
    github_issue::GitHubService,
    oauth::{OAuthConfig, OAuthState},
    openrouter::OpenRouterService,
    solver::SolverService,
};
use super::tools::create_tools;
use super::ws::transport::WebSocketState;
use crate::{routes, server};
use axum::{
    routing::get,
    Router,
};
use sqlx::PgPool;
use std::{env, sync::Arc};
use tower_http::services::ServeDir;
use crate::server::{
    handlers::{
        auth::{login, signup},
        oauth::{github, scramble},
    },
    services::oauth::{OAuthConfig, OAuthState},
};

#[derive(Clone)]
pub struct AppState {
    pub ws_state: Arc<WebSocketState>,
    pub oauth_state: Arc<OAuthState>,
    pub pool: PgPool,
}

#[derive(Clone)]
pub struct AppConfig {
    pub scramble_auth_url: String,
    pub scramble_token_url: String,
    pub scramble_client_id: String,
    pub scramble_client_secret: String,
    pub scramble_redirect_uri: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub github_redirect_uri: String,
    pub database_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            scramble_auth_url: env::var("SCRAMBLE_AUTH_URL").expect("SCRAMBLE_AUTH_URL must be set"),
            scramble_token_url: env::var("SCRAMBLE_TOKEN_URL").expect("SCRAMBLE_TOKEN_URL must be set"),
            scramble_client_id: env::var("SCRAMBLE_CLIENT_ID").expect("SCRAMBLE_CLIENT_ID must be set"),
            scramble_client_secret: env::var("SCRAMBLE_CLIENT_SECRET")
                .expect("SCRAMBLE_CLIENT_SECRET must be set"),
            scramble_redirect_uri: env::var("SCRAMBLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8000/auth/callback".to_string()),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            github_client_id: env::var("GITHUB_CLIENT_ID").expect("GITHUB_CLIENT_ID must be set"),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .expect("GITHUB_CLIENT_SECRET must be set"),
            github_redirect_uri: env::var("GITHUB_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8000/auth/github/callback".to_string()),
        }
    }
}

pub fn configure_app(pool: PgPool) -> Router {
    // Use default config
    configure_app_with_config(pool, None)
}

pub fn configure_app_with_config(pool: PgPool, config: Option<AppConfig>) -> Router {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize services with proper configuration
    let openrouter = OpenRouterService::new(
        env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set"),
    );

    let github_service = Arc::new(
        GitHubService::new(Some(
            env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set"),
        ))
        .expect("Failed to create GitHub service"),
    );

    let solver_service = Arc::new(SolverService::new(pool.clone(), openrouter.clone()));

    let api_key = env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set");
    let base_url =
        env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let tool_model = Arc::new(DeepSeekService::with_base_url(
        api_key.clone(),
        base_url.clone(),
    ));

    let chat_model = Arc::new(DeepSeekService::with_base_url(api_key, base_url));

    let tools = create_tools();

    let ws_state = Arc::new(WebSocketState::new(
        tool_model,
        chat_model,
        github_service.clone(),
        solver_service.clone(),
        tools,
    ));

    // Use provided config or default
    let config = config.unwrap_or_default();

    // Create OAuth configs
    let github_config = OAuthConfig {
        client_id: config.github_client_id,
        client_secret: config.github_client_secret,
        redirect_uri: config.github_redirect_uri,
        auth_url: "https://github.com/login/oauth/authorize".to_string(),
        token_url: "https://github.com/login/oauth/access_token".to_string(),
    };

    let scramble_config = OAuthConfig {
        client_id: config.scramble_client_id,
        client_secret: config.scramble_client_secret,
        redirect_uri: config.scramble_redirect_uri,
        auth_url: config.scramble_auth_url,
        token_url: config.scramble_token_url,
    };

    // Create OAuth state
    let oauth_state = Arc::new(
        OAuthState::new(pool.clone(), github_config, scramble_config)
            .expect("Failed to create OAuth state"),
    );

    // Create shared app state
    let app_state = AppState {
        ws_state,
        oauth_state,
        pool: pool.clone(),
    };

    // Create the main router
    Router::new()
        // Main routes
        .route("/", get(routes::home))
        .route("/ws", get(server::ws::ws_handler))
        .route("/onyx", get(routes::mobile_app))
        .route("/services", get(routes::business))
        .route("/video-series", get(routes::video_series))
        .route("/company", get(routes::company))
        .route("/coming-soon", get(routes::coming_soon))
        .route("/health", get(routes::health_check))
        .route("/cota", get(routes::cota))
        // Auth pages
        .route("/login", get(login::login_page))
        .route("/signup", get(signup::signup_page))
        // OAuth routes
        .route(
            "/auth/github/login",
            get(server::handlers::oauth::github::github_login),
        )
        .route(
            "/auth/github/callback",
            get(server::handlers::oauth::github::github_callback),
        )
        .route(
            "/auth/scramble/login",
            get(server::handlers::oauth::scramble::scramble_login),
        )
        .route(
            "/auth/scramble/signup",
            get(server::handlers::oauth::scramble::scramble_signup),
        )
        .route(
            "/auth/scramble/callback",
            get(server::handlers::oauth::scramble::scramble_callback),
        )
        .route(
            "/auth/logout",
            get(server::handlers::auth::clear_session_and_redirect),
        )
        // Hyperview routes
        .merge(server::hyperview::hyperview_routes())
        // Static files
        .nest_service("/assets", ServeDir::new("./assets").precompressed_gzip())
        .nest_service(
            "/templates",
            ServeDir::new("./templates").precompressed_gzip(),
        )
        // Serve all Vite files from dist
        .nest_service(
            "/chat",
            tower_http::services::fs::ServeFile::new("./chat/dist/index.html"),
        )
        .nest_service(
            "/chat/assets",
            ServeDir::new("./chat/dist/assets").precompressed_gzip(),
        )
        // State
        .with_state(app_state)
}

pub fn app_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login::login_page))
        .route("/auth/signup", get(signup::signup_page))
        .route("/auth/logout", get(server::handlers::auth::clear_session_and_redirect))
        .nest(
            "/auth/github",
            Router::new()
                .route("/login", get(github::github_login))
                .route("/callback", get(github::github_callback))
                .with_state(state.clone()),
        )
        .nest(
            "/auth/scramble",
            Router::new()
                .route("/login", get(scramble::scramble_login))
                .route("/signup", get(scramble::scramble_signup))
                .route("/callback", get(scramble::scramble_callback))
                .with_state(state.clone()),
        )
        .with_state(state)
}
