use super::services::{
    deepseek::DeepSeekService,
    github_issue::GitHubService,
    oauth::{github::GitHubOAuth, scramble::ScrambleOAuth, OAuthConfig},
    openrouter::OpenRouterService,
    solver::SolverService,
};
use super::tools::create_tools;
use super::ws::transport::WebSocketState;
use crate::{routes, server};
use axum::{
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
    Router,
};
use sqlx::PgPool;
use std::{env, sync::Arc};
use tower_http::services::ServeDir;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub ws_state: Arc<WebSocketState>,
    pub github_oauth: Arc<GitHubOAuth>,
    pub scramble_oauth: Arc<ScrambleOAuth>,
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
            scramble_auth_url: env::var("SCRAMBLE_AUTH_URL")
                .expect("SCRAMBLE_AUTH_URL must be set"),
            scramble_token_url: env::var("SCRAMBLE_TOKEN_URL")
                .expect("SCRAMBLE_TOKEN_URL must be set"),
            scramble_client_id: env::var("SCRAMBLE_CLIENT_ID")
                .expect("SCRAMBLE_CLIENT_ID must be set"),
            scramble_client_secret: env::var("SCRAMBLE_CLIENT_SECRET")
                .expect("SCRAMBLE_CLIENT_SECRET must be set"),
            scramble_redirect_uri: env::var("SCRAMBLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8000/auth/scramble/callback".to_string()),
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
        redirect_url: config.github_redirect_uri,
        auth_url: "https://github.com/login/oauth/authorize".to_string(),
        token_url: "https://github.com/login/oauth/access_token".to_string(),
    };

    let scramble_config = OAuthConfig {
        client_id: config.scramble_client_id,
        client_secret: config.scramble_client_secret,
        redirect_url: config.scramble_redirect_uri,
        auth_url: config.scramble_auth_url,
        token_url: config.scramble_token_url,
    };

    // Initialize OAuth services
    let github_oauth = Arc::new(
        GitHubOAuth::new(pool.clone(), github_config)
            .expect("Failed to create GitHub OAuth service"),
    );

    let scramble_oauth = Arc::new(
        ScrambleOAuth::new(pool.clone(), scramble_config)
            .expect("Failed to create Scramble OAuth service"),
    );

    // Create shared app state
    let app_state = AppState {
        ws_state,
        github_oauth,
        scramble_oauth,
        pool: pool.clone(),
    };

    // Create the main router
    Router::new()
        // API routes first to prevent conflicts
        .route("/api/user", get(routes::get_user_info))
        .route("/health", get(routes::health_check))
        .route("/ws", get(server::ws::ws_handler))
        // Merge auth router
        .merge(app_router(app_state.clone()))
        // Static assets
        .nest_service(
            "/assets",
            ServeDir::new("./client/assets").precompressed_gzip(),
        )
        // Serve index.html for all other routes (SPA)
        .fallback_service(tower_http::services::fs::ServeFile::new(
            "./client/index.html",
        ))
        .with_state(app_state)
}

async fn log_request(req: Request<Body>, next: Next) -> impl IntoResponse {
    info!(
        "Incoming request: {} {} query_params: {:?}",
        req.method(),
        req.uri(),
        req.uri().query()
    );
    next.run(req).await
}

pub fn app_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/auth/logout",
            get(server::handlers::oauth::session::clear_session_and_redirect),
        )
        // Add root callback route for Scramble
        .route(
            "/auth/callback",
            get(server::handlers::oauth::scramble::scramble_callback),
        )
        .nest(
            "/auth/github",
            Router::new()
                .route("/login", get(server::handlers::oauth::github::github_login))
                .route(
                    "/callback",
                    get(server::handlers::oauth::github::github_callback),
                )
                .with_state(state.clone()),
        )
        .nest(
            "/auth/scramble",
            Router::new()
                .route(
                    "/login",
                    get(server::handlers::oauth::scramble::scramble_login)
                        .post(server::handlers::oauth::scramble::scramble_login),
                )
                .route(
                    "/signup",
                    get(server::handlers::oauth::scramble::scramble_signup)
                        .post(server::handlers::oauth::scramble::scramble_signup),
                )
                .route(
                    "/callback",
                    get(server::handlers::oauth::scramble::scramble_callback),
                )
                .with_state(state.clone()),
        )
        .layer(middleware::from_fn(log_request))
        .with_state(state)
}
