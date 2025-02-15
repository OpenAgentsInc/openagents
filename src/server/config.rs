use super::services::{
    deepseek::DeepSeekService,
    github_auth::{GitHubAuthService, GitHubConfig},
    github_issue::GitHubService,
    openrouter::OpenRouterService,
    solver::SolverService,
};
use super::tools::create_tools;
use super::ws::transport::WebSocketState;
use crate::{routes, server};
use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use std::{env, sync::Arc};
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub ws_state: Arc<WebSocketState>,
    pub auth_state: Arc<server::handlers::auth::AuthState>,
    pub github_auth: Arc<GitHubAuthService>,
    pub pool: PgPool,
}

#[derive(Clone)]
pub struct AppConfig {
    pub oidc_auth_url: String,
    pub oidc_token_url: String,
    pub oidc_client_id: String,
    pub oidc_client_secret: String,
    pub oidc_redirect_uri: String,
    pub database_url: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub github_redirect_uri: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            oidc_auth_url: env::var("OIDC_AUTH_URL").expect("OIDC_AUTH_URL must be set"),
            oidc_token_url: env::var("OIDC_TOKEN_URL").expect("OIDC_TOKEN_URL must be set"),
            oidc_client_id: env::var("OIDC_CLIENT_ID").expect("OIDC_CLIENT_ID must be set"),
            oidc_client_secret: env::var("OIDC_CLIENT_SECRET")
                .expect("OIDC_CLIENT_SECRET must be set"),
            oidc_redirect_uri: env::var("OIDC_REDIRECT_URI")
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

    // Create auth state with OIDC config
    let oidc_config = server::services::auth::OIDCConfig::new(
        config.oidc_client_id,
        config.oidc_client_secret,
        config.oidc_redirect_uri,
        config.oidc_auth_url,
        config.oidc_token_url,
    )
    .expect("Failed to create OIDC config");

    let auth_state = Arc::new(server::handlers::auth::AuthState::new(
        oidc_config,
        pool.clone(),
    ));

    // Create GitHub auth service
    let github_config = GitHubConfig {
        client_id: config.github_client_id,
        client_secret: config.github_client_secret,
        redirect_uri: config.github_redirect_uri,
    };
    let github_auth = Arc::new(GitHubAuthService::new(pool.clone(), github_config));

    // Create shared app state
    let app_state = AppState {
        ws_state,
        auth_state,
        github_auth,
        pool: pool.clone(),
    };

    // Create the main router
    Router::new()
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
        .route("/cota", get(routes::cota))
        // Auth pages
        .route("/login", get(server::handlers::login_page))
        .route("/signup", get(server::handlers::signup_page))
        // Auth routes
        .route("/auth/login", post(server::handlers::handle_login))
        .route("/auth/signup", post(server::handlers::handle_signup))
        .route("/auth/callback", get(server::handlers::callback))
        .route(
            "/auth/logout",
            get(server::handlers::auth::clear_session_and_redirect),
        )
        // GitHub auth routes
        .route(
            "/auth/github",
            get(server::handlers::auth::github_login_page),
        )
        .route(
            "/auth/github/login",
            get(server::handlers::auth::handle_github_login),
        )
        .route(
            "/auth/github/callback",
            get(server::handlers::auth::handle_github_callback),
        )
        // Hyperview routes
        .merge(server::hyperview::hyperview_routes())
        // Static files
        .nest_service("/assets", ServeDir::new("./assets").precompressed_gzip())
        .nest_service(
            "/templates",
            ServeDir::new("./templates").precompressed_gzip(),
        )
        // Serve Expo web build static files
        .nest_service(
            "/chat/assets",
            ServeDir::new("./chat/web-build/assets").precompressed_gzip(),
        )
        .nest_service(
            "/chat/static",
            ServeDir::new("./chat/web-build/static").precompressed_gzip(),
        )
        // State
        .with_state(app_state)
}
