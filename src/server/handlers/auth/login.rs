use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::server::config::AppState;

pub async fn login_page() -> Response {
    super::session::render_login_template().await
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    #[serde(default)]
    pub platform: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub url: String,
}

pub async fn handle_login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Response {
    info!("Processing login request for email: {}", request.email);

    match state
        .auth_state
        .service
        .authorization_url_for_login(&request.email)
    {
        Ok(url) => {
            info!("Generated login URL: {}", url);
            Json(LoginResponse { url }).into_response()
        }
        Err(e) => {
            info!("Failed to generate login URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to generate login URL: {}", e),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginCallbackRequest {
    pub code: String,
    #[serde(default)]
    pub platform: Option<String>,
}

pub async fn handle_login_callback(
    State(state): State<AppState>,
    Query(request): Query<LoginCallbackRequest>,
) -> Response {
    info!("Processing login callback with code length: {}", request.code.len());

    // Check if request is from mobile app
    let is_mobile = request.platform.as_deref() == Some("mobile");

    match state.auth_state.service.login(request.code).await {
        Ok(user) => {
            info!("Successfully logged in user: {:?}", user);
            super::session::create_session_and_redirect(user, is_mobile).await
        }
        Err(e) => {
            info!("Login failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Login failed: {}", e),
            )
                .into_response()
        }
    }
}