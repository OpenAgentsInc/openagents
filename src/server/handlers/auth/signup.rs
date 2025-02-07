use axum::{
    extract::{Query, State},
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
    #[serde(default)]
    pub platform: Option<String>,
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

#[derive(Debug, Deserialize)]
pub struct SignupCallbackRequest {
    pub code: String,
    #[serde(default)]
    pub platform: Option<String>,
}

pub async fn handle_signup_callback(
    State(state): State<AppState>,
    Query(request): Query<SignupCallbackRequest>,
) -> Response {
    info!(
        "Processing signup callback with code length: {}",
        request.code.len()
    );

    // Check if request is from mobile app
    let is_mobile = request.platform.as_deref() == Some("mobile");

    match state.auth_state.service.signup(request.code).await {
        Ok(user) => {
            info!("Successfully signed up user: {:?}", user);
            super::session::create_session_and_redirect(user, is_mobile).await
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
