use crate::server::{
    config::AppState,
    models::user::User,
    services::oauth::OAuthError,
};
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tracing::info;

pub mod login;
pub mod session;
pub mod signup;

pub use login::{handle_login, login_page};
pub use signup::{handle_signup, signup_page};

#[derive(Debug, Deserialize)]
pub struct AuthParams {
    error: Option<String>,
}

pub async fn handle_auth_error(
    State(_state): State<AppState>,
    Query(params): Query<AuthParams>,
) -> Response {
    info!("Handling auth error: {:?}", params.error);

    if let Some(error) = params.error {
        // Handle the error appropriately
        Redirect::temporary(&format!("/login?error={}", error)).into_response()
    } else {
        // No error, redirect to login
        Redirect::temporary("/login").into_response()
    }
}

pub async fn create_session_and_redirect(
    user: &User,
    is_mobile: bool,
) -> Response {
    session::create_session_and_redirect(user, is_mobile).await
}
