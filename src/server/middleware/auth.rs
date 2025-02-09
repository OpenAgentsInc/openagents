use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{error, info};

use crate::server::config::AppState;

pub async fn require_auth(
    State(state): State<AppState>,
    cookies: CookieJar,
    request: Request,
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
    request.extensions().insert(user_id);
    info!("Added user_id to request extensions");

    // Continue with the request
    Ok(next.run(request).await)
}

pub fn auth_middleware<B>() -> axum::middleware::from_fn_with_state<AppState, B, fn(State<AppState>, CookieJar, Request, Next) -> _> 
where
    B: Send + 'static,
{
    axum::middleware::from_fn_with_state(require_auth)
}