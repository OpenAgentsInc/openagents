use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response, Redirect},
};
use serde::Deserialize;
use tracing::{info, error};

use crate::server::{
    config::AppState,
    handlers::auth::session::{create_session_and_redirect, render_login_template},
    services::auth::AuthError,
};

#[derive(Debug, Deserialize)]
pub struct GitHubCallback {
    code: String,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubLoginParams {
    #[serde(default)]
    platform: Option<String>,
}

pub async fn github_login_page() -> Response {
    render_login_template().await
}

#[axum::debug_handler]
pub async fn handle_github_login(
    State(state): State<AppState>,
    Query(params): Query<GitHubLoginParams>,
) -> Response {
    info!(
        "Handling GitHub login request with platform: {:?}",
        params.platform
    );

    // Get authorization URL with platform in state
    let url = match state.github_auth.authorization_url(params.platform.clone()) {
        Ok(url) => url,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, url)
        .body(axum::body::Body::empty())
        .unwrap()
}

#[axum::debug_handler]
pub async fn handle_github_callback(
    State(state): State<AppState>,
    Query(callback): Query<GitHubCallback>,
) -> Response {
    info!("Handling GitHub callback");
    info!("Code length: {}", callback.code.len());
    info!("Platform: {:?}", callback.platform);
    info!("State: {:?}", callback.state);

    // Check if request is from mobile app
    let is_mobile = callback.platform.as_deref() == Some("mobile")
        || callback.state.as_deref() == Some("mobile")
        || callback.state.as_deref() == Some("\"mobile\"");

    info!("Is mobile: {}", is_mobile);

    match state.github_auth.process_auth_code(&callback.code).await {
        Ok(user) => {
            info!("Authentication successful, redirecting with is_mobile: {}", is_mobile);
            if is_mobile {
                // Redirect to main screen with auth token
                let token = format!("github_{}", user.github_id.unwrap_or_default());
                let redirect_url = format!("onyx://?token={}&screen=main", token);
                Redirect::to(&redirect_url).into_response()
            } else {
                create_session_and_redirect(user, false).await
            }
        }
        Err(AuthError::NotAuthenticated) => {
            // Clean redirect back to login
            if is_mobile {
                Redirect::to("onyx://?screen=login").into_response()
            } else {
                render_login_template().await
            }
        }
        Err(e) => {
            error!("GitHub auth error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
