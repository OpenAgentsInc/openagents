//! Authentication middleware

use crate::db::sessions::{extract_session_token, Session};
use worker::*;

/// Authenticated user extracted from session
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub github_username: String,
    pub session_token: String,
}

/// Authenticate a request using session cookie
pub async fn authenticate(req: &Request, env: &Env) -> Result<AuthenticatedUser> {
    // Get cookie header
    let cookie_header = req
        .headers()
        .get("cookie")?
        .ok_or_else(|| Error::RustError("No cookies".to_string()))?;

    // Extract session token
    let token = extract_session_token(&cookie_header)
        .ok_or_else(|| Error::RustError("No session cookie".to_string()))?;

    // Get session from KV
    let kv = env.kv("SESSIONS")?;
    let session = Session::get(&kv, &token)
        .await?
        .ok_or_else(|| Error::RustError("Invalid session".to_string()))?;

    // Touch session to refresh TTL
    session.touch(&kv, &token).await?;

    Ok(AuthenticatedUser {
        user_id: session.user_id,
        github_username: session.github_username,
        session_token: token,
    })
}

/// Check if a request is authenticated (without requiring it)
pub async fn check_auth(req: &Request, env: &Env) -> Option<AuthenticatedUser> {
    authenticate(req, env).await.ok()
}
