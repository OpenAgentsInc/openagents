use axum::{
    extract::State,
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

pub async fn handle_login_callback(
    State(state): State<AppState>,
    Json(code): Json<String>,
) -> Response {
    info!("Processing login callback with code length: {}", code.len());

    match state.auth_state.service.login(code).await {
        Ok(user) => {
            info!("Successfully logged in user: {:?}", user);
            super::session::create_session_and_redirect(user).await
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