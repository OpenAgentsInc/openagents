use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use tracing::{error, info};

use crate::server::config::AppState;

use super::{forms::SignupForm, ErrorResponse};

pub async fn signup_page() -> Response {
    info!("Serving signup page");
    super::session::render_signup_template()
}

pub async fn handle_signup(
    State(state): State<AppState>,
    Form(form): Form<SignupForm>,
) -> Response {
    info!("Received signup form: {:?}", form);

    // Basic validation
    if !form.terms_accepted {
        error!("Terms not accepted");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Terms must be accepted".to_string(),
            }),
        )
            .into_response();
    }
    if form.password != form.password_confirmation {
        error!("Passwords do not match");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Passwords do not match".to_string(),
            }),
        )
            .into_response();
    }

    // Generate signup URL with prompt=create and email
    match state
        .auth_state
        .service
        .authorization_url_for_signup(&form.email)
    {
        Ok(auth_url) => {
            info!("Redirecting to signup URL: {}", auth_url);
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

pub async fn handle_signup_callback(
    State(state): State<AppState>,
    code: String,
) -> Response {
    info!("Processing signup callback with code length: {}", code.len());

    match state.auth_state.service.signup(code).await {
        Ok(user) => {
            info!("Signup successful for user: {:?}", user);
            super::session::create_session_and_redirect(user)
        }
        Err(e) => super::handle_auth_error(e),
    }
}