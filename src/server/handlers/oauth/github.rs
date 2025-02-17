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
    platform: Option<String>,
}

pub async fn github_login(
    State(state): State<AppState>,
    Query(_params): Query<LoginParams>,
) -> Response {
    info!("Handling GitHub login request");

    let (url, _csrf_token, _pkce_verifier) =
        state.oauth_state.github.authorization_url_for_login(""); // Email not used for GitHub

    axum::response::Redirect::temporary(&url).into_response()
}

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
    state: Option<String>,
    error: Option<String>,
}

pub async fn github_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Response {
    info!("Processing GitHub callback");

    if let Some(error) = params.error {
        info!("GitHub OAuth error: {}", error);
        return handle_oauth_error(error.into()).into_response();
    }

    let is_mobile = params
        .state
        .as_deref()
        .map(|s| s.contains("mobile"))
        .unwrap_or(false);

    match state
        .oauth_state
        .github
        .authenticate(params.code, false)
        .await
    {
        Ok(user) => {
            info!("Successfully authenticated GitHub user: {:?}", user);
            match create_session_and_redirect(&user, Some(is_mobile)).await {
                Ok(response) => response,
                Err(error) => handle_oauth_error(error).into_response(),
            }
        }
        Err(error) => handle_oauth_error(error).into_response(),
    }
}
