use crate::server::{
    models::user::User,
    services::oauth::{github::GitHubOAuth, scramble::ScrambleOAuth, OAuthConfig, OAuthError},
};
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{error, info};

pub mod github;
pub mod scramble;

const SESSION_COOKIE_NAME: &str = "session";
const SESSION_DURATION_DAYS: i64 = 30;

#[derive(Debug, Clone)]
pub struct OAuthState {
    pub github: GitHubOAuth,
    pub scramble: ScrambleOAuth,
    pub pool: PgPool,
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallback {
    code: String,
    state: Option<String>,
    error: Option<String>,
}

impl OAuthState {
    pub fn new(
        pool: PgPool,
        github_config: OAuthConfig,
        scramble_config: OAuthConfig,
    ) -> Result<Self, OAuthError> {
        Ok(Self {
            github: GitHubOAuth::new(pool.clone(), github_config)?,
            scramble: ScrambleOAuth::new(pool.clone(), scramble_config)?,
            pool,
        })
    }
}

pub async fn create_session_and_redirect(
    user: &User,
    platform: Option<String>,
) -> Result<impl IntoResponse, OAuthError> {
    info!("Creating session for user ID: {}", user.id);

    // Create JWT token or session ID here
    // For now, just use user ID as session token
    let session_token = user.id.to_string();

    // Handle mobile platform
    if let Some(platform) = platform {
        if platform == "mobile" {
            let mobile_url = format!("openagents://auth?token={}", session_token);
            info!("Redirecting to mobile URL: {}", mobile_url);
            return Ok(Redirect::temporary(&mobile_url));
        }
    }

    // Default to web redirect
    info!("Redirecting to web chat interface");
    Ok(Redirect::temporary("/chat"))
}

pub fn handle_oauth_error(error: OAuthError) -> impl IntoResponse {
    error!("OAuth error: {}", error);

    // Log detailed error info
    match &error {
        OAuthError::InvalidConfig(msg) => error!("Invalid config: {}", msg),
        OAuthError::AuthenticationFailed(msg) => error!("Auth failed: {}", msg),
        OAuthError::TokenExchangeFailed(msg) => error!("Token exchange failed: {}", msg),
        OAuthError::DatabaseError(msg) => error!("Database error: {}", msg),
        OAuthError::UserCreationError(msg) => error!("User creation error: {}", msg),
    }

    // Redirect to error page
    Redirect::temporary("/auth/error")
}
