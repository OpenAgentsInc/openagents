use crate::server::{
    config::AppState,
    handlers::auth::{create_session_and_redirect, handle_oauth_error},
};
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct LoginParams {
    email: String,
}

pub async fn scramble_login(
    State(state): State<AppState>,
    Query(params): Query<LoginParams>,
) -> Response {
    info!(
        "Handling Scramble login request for email: {}",
        params.email
    );

    let (url, _csrf_token, _pkce_verifier) = state
        .oauth_state
        .scramble
        .authorization_url_for_login(&params.email);

    axum::response::Redirect::temporary(&url).into_response()
}

pub async fn scramble_signup(
    State(state): State<AppState>,
    Query(params): Query<LoginParams>,
) -> Response {
    info!(
        "Handling Scramble signup request for email: {}",
        params.email
    );

    let (url, _csrf_token, _pkce_verifier) = state
        .oauth_state
        .scramble
        .authorization_url_for_signup(&params.email);

    axum::response::Redirect::temporary(&url).into_response()
}

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
    state: Option<String>,
    error: Option<String>,
}

pub async fn scramble_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Response {
    info!("Processing Scramble callback");

    if let Some(error) = params.error {
        info!("Scramble OAuth error: {}", error);
        return handle_oauth_error(error.into()).into_response();
    }

    let is_signup = params
        .state
        .as_deref()
        .map(|s| s.contains("signup"))
        .unwrap_or(false);

    match state
        .oauth_state
        .scramble
        .authenticate(params.code, is_signup)
        .await
    {
        Ok(user) => {
            info!("Successfully authenticated Scramble user: {:?}", user);
            match create_session_and_redirect(&user, None).await {
                Ok(response) => response,
                Err(error) => handle_oauth_error(error).into_response(),
            }
        }
        Err(error) => handle_oauth_error(error).into_response(),
    }
}
