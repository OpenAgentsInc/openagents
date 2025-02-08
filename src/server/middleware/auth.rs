use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tower_cookies::Cookies;

use crate::server::config::AppState;

pub fn require_auth() -> axum::middleware::from_fn_with_state<AppState, RequireAuth> {
    axum::middleware::from_fn_with_state(require_auth_middleware)
}

async fn require_auth_middleware<B>(
    State(state): State<AppState>,
    cookies: Cookies,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // Get session cookie
    let session_cookie = cookies
        .get("session")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate session
    let session_token = session_cookie.value();
    let user_id = state
        .auth_state
        .validate_session(session_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Add user_id to request extensions
    req.extensions_mut().insert(user_id);

    // Continue with the request
    Ok(next.run(req).await)
}

pub struct RequireAuth;

impl<B> axum::middleware::FromFnWithState<AppState, B> for RequireAuth
where
    B: Send + 'static,
{
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>;

    fn call<F>(
        self,
        state: State<AppState>,
        cookies: Cookies,
        req: Request<B>,
        next: Next<B>,
        middleware: F,
    ) -> Self::Future
    where
        F: FnOnce(State<AppState>, Cookies, Request<B>, Next<B>) -> Self::Future + Send + 'static,
    {
        Box::pin(async move {
            match middleware(state, cookies, req, next).await {
                Ok(response) => response,
                Err(status) => status.into_response(),
            }
        })
    }
}