use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::CookieJar;
use tracing::info;

use crate::server::config::AppState;

pub async fn require_auth(
    State(state): State<AppState>,
    cookies: CookieJar,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    info!("Validating auth for request");

    // Get session cookie
    let session_cookie = cookies
        .get("session")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate session
    let session_token = session_cookie.value();
    let user_id = state
        .auth_state
        .service
        .validate_session(session_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Add user_id to request extensions
    request.extensions().insert(user_id);

    // Continue with the request
    Ok(next.run(request).await)
}

pub fn auth_middleware<B>() -> axum::middleware::from_fn_with_state<AppState, B, fn(State<AppState>, CookieJar, Request, Next) -> _> 
where
    B: Send + 'static,
{
    axum::middleware::from_fn_with_state(require_auth)
}