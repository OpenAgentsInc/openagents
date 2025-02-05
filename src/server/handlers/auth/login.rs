use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use tracing::{error, info};

use crate::server::config::AppState;

use super::{forms::LoginForm, ErrorResponse};

pub async fn login_page() -> Response {
    info!("Serving login page");
    super::session::render_login_template()
}

pub async fn handle_login(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Response {
    info!("Received login form: {:?}", form);

    // Validate form input
    if let Err(e) = form.validate() {
        error!("Login form validation failed: {}", e);
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e,
            }),
        )
            .into_response();
    }

    // Generate login URL with email
    match state.auth_state.service.authorization_url_for_login(&form.email) {
        Ok(auth_url) => {
            info!("Redirecting to login URL: {}", auth_url);
            Redirect::temporary(&auth_url).into_response()
        }
        Err(e) => {
            error!("Failed to generate auth URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Internal server error: {}", e),
                }),
            )
                .into_response()
        }
    }
}

pub async fn handle_login_callback(
    State(state): State<AppState>,
    code: String,
) -> Response {
    info!("Processing login callback with code length: {}", code.len());

    match state.auth_state.service.login(code).await {
        Ok(user) => {
            info!("Login successful for user: {:?}", user);
            super::session::create_session_and_redirect(user)
        }
        Err(e) => super::handle_auth_error(e),
    }
}