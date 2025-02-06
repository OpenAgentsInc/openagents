use axum::{
    extract::State,
    http::StatusCode,
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
}

pub async fn github_login_page() -> Response {
    render_login_template()
}

#[axum::debug_handler]
pub async fn handle_github_login(State(state): State<AppState>) -> Response {
    info!("Handling GitHub login request");
    
    match state.github_auth.authorization_url() {
        Ok(url) => Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header("Location", url)
            .body(axum::body::Body::empty())
            .unwrap(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[axum::debug_handler]
pub async fn handle_github_callback(
    State(state): State<AppState>,
    Json(callback): Json<GitHubCallback>,
) -> Response {
    info!("Handling GitHub callback");

    match state.github_auth.authenticate(callback.code).await {
        Ok(user) => create_session_and_redirect(user),
        Err(GitHubAuthError::UserAlreadyExists(user)) => {
            create_session_and_redirect(user)
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}