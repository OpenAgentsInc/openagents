use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::info;

use crate::server::{
    config::AppState,
    handlers::auth::session::{create_session_and_redirect, render_login_template},
    services::github_auth::GitHubAuthError,
};

#[derive(Debug, Deserialize)]
pub struct GitHubCallback {
    code: String,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubLoginParams {
    #[serde(default)]
    platform: Option<String>,
}

pub async fn github_login_page() -> Response {
    render_login_template().await
}

#[axum::debug_handler]
pub async fn handle_github_login(
    State(state): State<AppState>,
    Query(params): Query<GitHubLoginParams>,
) -> Response {
    info!("Handling GitHub login request");
    
    // Get authorization URL with platform in state
    let url = match state.github_auth.authorization_url(params.platform) {
        Ok(url) => url,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, url)
        .body(axum::body::Body::empty())
        .unwrap()
}

#[axum::debug_handler]
pub async fn handle_github_callback(
    State(state): State<AppState>,
    Query(callback): Query<GitHubCallback>,
) -> Response {
    info!("Handling GitHub callback");
    info!("Code length: {}", callback.code.len());
    info!("Platform: {:?}", callback.platform);
    info!("State: {:?}", callback.state);

    // Check if request is from mobile app (either from platform param or state)
    let is_mobile = callback.platform.as_deref() == Some("mobile") 
        || callback.state.as_deref() == Some("mobile");

    match state.github_auth.authenticate(callback.code).await {
        Ok(user) => create_session_and_redirect(user, is_mobile).await,
        Err(GitHubAuthError::UserAlreadyExists(user)) => {
            create_session_and_redirect(user, is_mobile).await
        }
        Err(e) => {
            info!("GitHub auth error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}