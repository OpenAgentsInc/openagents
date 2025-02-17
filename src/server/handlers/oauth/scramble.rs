use super::{create_session_and_redirect, handle_oauth_error, OAuthCallback, OAuthState};
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::info;

pub async fn scramble_login(
    State(state): State<OAuthState>,
    Query(params): Query<ScrambleLoginParams>,
) -> impl IntoResponse {
    info!("Starting Scramble login flow for email: {}", params.email);

    // Generate authorization URL
    let (auth_url, _csrf_token) = state
        .scramble
        .authorization_url_for_login(&params.email);

    Redirect::temporary(&auth_url)
}

pub async fn scramble_signup(
    State(state): State<OAuthState>,
    Query(params): Query<ScrambleLoginParams>,
) -> impl IntoResponse {
    info!("Starting Scramble signup flow for email: {}", params.email);

    // Generate authorization URL
    let (auth_url, _csrf_token) = state
        .scramble
        .authorization_url_for_signup(&params.email);

    Redirect::temporary(&auth_url)
}

pub async fn scramble_callback(
    State(state): State<OAuthState>,
    Query(params): Query<OAuthCallback>,
) -> impl IntoResponse {
    info!("Handling Scramble callback");

    match params.error {
        Some(error) => {
            info!("Scramble auth error: {}", error);
            Redirect::temporary("/auth/error")
        }
        None => {
            let code = params.code;
            let is_signup = params
                .state
                .as_deref()
                .map(|s| s.contains("signup"))
                .unwrap_or(false);

            // Authenticate with Scramble
            match state.scramble.authenticate(code, is_signup).await {
                Ok(user) => {
                    match create_session_and_redirect(&user, None).await {
                        Ok(response) => response,
                        Err(error) => handle_oauth_error(error),
                    }
                }
                Err(error) => handle_oauth_error(error),
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ScrambleLoginParams {
    email: String,
}
