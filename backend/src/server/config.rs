use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::server::{
    handlers::{
        chat::{send_message, start_repo_chat},
        oauth::{
            github::{github_callback, github_login},
            scramble::{scramble_callback, scramble_login, scramble_signup},
        },
        user::{check_email, create_user, get_user},
    },
    services::{
        auth::AuthService,
        chat_database::ChatDatabaseService,
        github_auth::GitHubAuthService,
        github_issue::GitHubIssueService,
        github_repos::GitHubReposService,
        model_router::ModelRouterService,
        oauth::{github::GitHubOAuthService, scramble::ScrambleOAuthService},
        repomap::RepomapService,
        solver::SolverService,
    },
    ws::transport::WebSocketTransport,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub auth: Arc<AuthService>,
    pub github_auth: Arc<GitHubAuthService>,
    pub github_oauth: Arc<GitHubOAuthService>,
    pub scramble_oauth: Arc<ScrambleOAuthService>,
    pub github_issue: Arc<GitHubIssueService>,
    pub github_repos: Arc<GitHubReposService>,
    pub chat_db: Arc<ChatDatabaseService>,
    pub model_router: Arc<ModelRouterService>,
    pub repomap: Arc<RepomapService>,
    pub solver: Arc<SolverService>,
    pub ws_transport: Arc<WebSocketTransport>,
}

impl Default for AppState {
    fn default() -> Self {
        unimplemented!("AppState requires initialization with services")
    }
}

pub async fn configure_app(pool: PgPool) -> Router {
    configure_app_with_config(pool).await
}

pub async fn configure_app_with_config(pool: PgPool) -> Router {
    // Initialize services
    let auth = Arc::new(AuthService::new(pool.clone()));
    let github_auth = Arc::new(GitHubAuthService::new());
    let github_oauth = Arc::new(GitHubOAuthService::new());
    let scramble_oauth = Arc::new(ScrambleOAuthService::new());
    let github_issue = Arc::new(GitHubIssueService::new());
    let github_repos = Arc::new(GitHubReposService::new());
    let chat_db = Arc::new(ChatDatabaseService::new(pool.clone()));
    let model_router = Arc::new(ModelRouterService::new());
    let repomap = Arc::new(RepomapService::new());
    let solver = Arc::new(SolverService::new());
    let ws_transport = Arc::new(WebSocketTransport::new());

    let state = AppState {
        pool,
        auth,
        github_auth,
        github_oauth,
        scramble_oauth,
        github_issue,
        github_repos,
        chat_db,
        model_router,
        repomap,
        solver,
        ws_transport,
    };

    app_router(state)
}

async fn log_request<B>(request: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    info!("{} {}", request.method(), request.uri().path());
    Ok(next.run(request).await)
}

fn app_router(state: AppState) -> Router {
    Router::new()
        .route("/api/user", get(get_user))
        .route("/api/check-email", post(check_email))
        .route("/api/create-user", post(create_user))
        .route("/api/github/login", get(github_login))
        .route("/api/github/callback", get(github_callback))
        .route("/api/scramble/login", get(scramble_login))
        .route("/api/scramble/signup", get(scramble_signup))
        .route("/api/scramble/callback", get(scramble_callback))
        .route("/api/start-repo-chat", post(start_repo_chat))
        .route("/api/send-message", post(send_message))
        .layer(middleware::from_fn(log_request))
        .layer(CorsLayer::permissive())
        .with_state(state)
}