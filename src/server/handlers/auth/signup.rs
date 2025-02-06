use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::server::config::AppState;

pub async fn signup_page() -> Response {
    super::session::render_signup_template().await
}

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct SignupResponse {
    pub url: String,
}

pub async fn handle_signup(
    State(state): State<AppState>,
    Json(request): Json<SignupRequest>,
) -> Response {
    info!("Processing signup request for email: {}", request.email);

    match state
        .auth_state
        .service
        .authorization_url_for_signup(&request.email)
    {
        Ok(url) => {
            info!("Generated signup URL: {}", url);
            Json(SignupResponse { url }).into_response()
        }
        Err(e) => {
            info!("Failed to generate signup URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to generate signup URL: {}", e),
            )
                .into_response()
        }
    }
}

pub async fn handle_signup_callback(
    State(state): State<AppState>,
    Json(code): Json<String>,
) -> Response {
    info!("Processing signup callback with code length: {}", code.len());

    match state.auth_state.service.signup(code).await {
        Ok(user) => {
            info!("Successfully signed up user: {:?}", user);
            super::session::create_session_and_redirect(user).await
        }
        Err(e) => {
            info!("Signup failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Signup failed: {}", e),
            )
                .into_response()
        }
    }
}