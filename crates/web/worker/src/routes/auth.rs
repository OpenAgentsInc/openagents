//! GitHub OAuth authentication routes

use crate::db::sessions::{clear_session_cookie, session_cookie, Session};
use crate::db::users;
use crate::services::github;
use worker::*;

/// Start GitHub OAuth flow
pub async fn github_start(req: Request, env: Env) -> Result<Response> {
    let client_id = env.var("GITHUB_CLIENT_ID")?.to_string();

    // Get or determine redirect URI
    let url = req.url()?;
    let origin = url.origin().ascii_serialization();
    let redirect_uri = format!("{}/api/auth/github/callback", origin);

    // Generate state token
    let state = uuid::Uuid::new_v4().to_string();

    // Store state in a temporary KV entry
    let kv = env.kv("SESSIONS")?;
    kv.put(&format!("oauth_state:{}", state), "1")?
        .expiration_ttl(600) // 10 minutes
        .execute()
        .await?;

    // Scopes: read user, email, orgs, and repo access
    let scope = "read:user user:email read:org repo";

    let authorize_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&state={}&scope={}",
        client_id,
        urlencoding::encode(&redirect_uri),
        state,
        urlencoding::encode(scope)
    );

    Response::redirect(Url::parse(&authorize_url)?)
}

/// Handle GitHub OAuth callback
pub async fn github_callback(req: Request, env: Env) -> Result<Response> {
    let url = req.url()?;

    // Parse query parameters
    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| Error::RustError("Missing code parameter".to_string()))?;

    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| Error::RustError("Missing state parameter".to_string()))?;

    // Verify state
    let kv = env.kv("SESSIONS")?;
    let state_valid = kv
        .get(&format!("oauth_state:{}", state))
        .text()
        .await?
        .is_some();

    if !state_valid {
        return Response::error("Invalid OAuth state", 400);
    }

    // Delete used state
    kv.delete(&format!("oauth_state:{}", state)).await?;

    // Exchange code for access token
    let client_id = env.var("GITHUB_CLIENT_ID")?.to_string();
    let client_secret = env.secret("GITHUB_CLIENT_SECRET")?.to_string();

    let token_response = github::exchange_code(&client_id, &client_secret, &code).await?;

    // Get user info from GitHub
    let github_user = github::get_user(&token_response.access_token).await?;

    // Get primary email
    let emails = github::get_emails(&token_response.access_token).await?;
    let primary_email = emails
        .iter()
        .find(|e| e.primary)
        .map(|e| e.email.as_str());

    // Upsert user in D1
    let db = env.d1("DB")?;
    let session_secret = env.secret("SESSION_SECRET")?.to_string();
    let user = users::upsert_from_github(
        &db,
        &github_user.id.to_string(),
        &github_user.login,
        primary_email,
        &token_response.access_token,
        &session_secret,
    )
    .await?;

    // Create session
    let session_token = Session::create(&kv, &user.user_id, &user.github_username).await?;

    // Redirect to home with session cookie
    let origin = url.origin().ascii_serialization();
    let is_secure = origin.starts_with("https");

    let mut headers = Headers::new();
    headers.set("Location", &origin)?;
    headers.set("Set-Cookie", &session_cookie(&session_token, is_secure))?;

    Ok(Response::empty()?
        .with_status(302)
        .with_headers(headers))
}

/// Logout - clear session
pub async fn logout(req: Request, env: Env) -> Result<Response> {
    // Get session token
    if let Some(cookie) = req.headers().get("cookie")? {
        if let Some(token) = crate::db::sessions::extract_session_token(&cookie) {
            let kv = env.kv("SESSIONS")?;
            Session::delete(&kv, &token).await?;
        }
    }

    let mut headers = Headers::new();
    headers.set("Set-Cookie", &clear_session_cookie())?;

    Ok(Response::ok("Logged out")?.with_headers(headers))
}
