use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{error, info};

use crate::server::config::AppState;

pub async fn require_auth(
    State(state): State<AppState>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    info!("Validating auth for request to: {}", request.uri());

    // Get session cookie
    let session_cookie = match cookies.get("session") {
        Some(cookie) => {
            info!("Found session cookie");
            cookie
        }
        None => {
            error!("No session cookie found");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Validate session
    let session_token = session_cookie.value();
    info!("Validating session token");
    
    let user_id = match state.auth_state.service.validate_session(session_token).await {
        Ok(id) => {
            info!("Session validated for user: {}", id);
            id
        }
        Err(e) => {
            error!("Session validation failed: {}", e);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Add user_id to request extensions
    request.extensions_mut().insert(user_id);
    info!("Added user_id to request extensions");

    // Continue with the request
    Ok(next.run(request).await)
}

pub fn with_auth<B>(
    state: AppState,
) -> axum::middleware::from_fn_with_state<AppState, B, fn(State<AppState>, CookieJar, Request, Next) -> Result<Response, StatusCode>>
where
    B: Send + 'static,
{
    axum::middleware::from_fn_with_state(state, require_auth)
}