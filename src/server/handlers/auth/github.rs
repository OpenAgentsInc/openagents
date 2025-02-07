use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
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
}

pub async fn github_login_page() -> Response {
    render_login_template().await
}

#[axum::debug_handler]
pub async fn handle_github_login(
    State(state): State<AppState>,
    Query(params): Query<GitHubCallback>,
) -> Response {
    info!("Handling GitHub login request");
    
    // Add platform to authorization URL if provided
    let mut url = match state.github_auth.authorization_url() {
        Ok(url) => url,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    // Append platform parameter if provided
    if let Some(platform) = params.platform {
        url.push_str(&format!("&platform={}", platform));
    }

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

    // Check if request is from mobile app
    let is_mobile = callback.platform.as_deref() == Some("mobile");

    match state.github_auth.authenticate(callback.code).await {
        Ok(user) => create_session_and_redirect(user, is_mobile).await,
        Err(GitHubAuthError::UserAlreadyExists(user)) => {
            create_session_and_redirect(user, is_mobile).await
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}