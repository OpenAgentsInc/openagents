//! Claude OAuth authentication routes
//!
//! Handles OAuth 2.0 with PKCE for Claude Pro/Max subscription users.
//! Users can authenticate with their Claude subscription instead of API keys.

use crate::services::claude::{self, generate_pkce, AUTHORIZE_URL};
use serde::{Deserialize, Serialize};
use worker::*;

/// PKCE state stored in KV during OAuth flow
#[derive(Serialize, Deserialize)]
struct OAuthPkceState {
    verifier: String,
    redirect_uri: String,
}

/// Start Claude OAuth flow with PKCE
///
/// GET /api/auth/claude/start
/// Redirects user to claude.ai to authorize
pub async fn claude_start(req: Request, env: Env) -> Result<Response> {
    // Get Claude client ID from environment
    let client_id = match env.var("CLAUDE_CLIENT_ID") {
        Ok(v) => v.to_string(),
        Err(_) => {
            return Response::error(
                "Claude OAuth not configured. Set CLAUDE_CLIENT_ID.",
                500,
            )
        }
    };

    // Get redirect URI from request origin
    let url = req.url()?;
    let origin = url.origin().ascii_serialization();
    let redirect_uri = format!("{}/api/auth/claude/callback", origin);

    // Generate PKCE challenge
    let pkce = generate_pkce();

    // Generate state token for CSRF protection
    let state = uuid::Uuid::new_v4().to_string();

    // Store PKCE verifier + redirect URI in KV (needed for token exchange)
    let kv = env.kv("SESSIONS")?;
    let pkce_state = OAuthPkceState {
        verifier: pkce.verifier,
        redirect_uri: redirect_uri.clone(),
    };
    kv.put(
        &format!("claude_oauth:{}", state),
        serde_json::to_string(&pkce_state)?,
    )?
    .expiration_ttl(600) // 10 minutes
    .execute()
    .await?;

    // Build authorization URL with PKCE
    // Scopes: openid for basic auth, org:read for organization access
    let scope = "openid";
    let authorize_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&state={}&scope={}&code_challenge={}&code_challenge_method=S256",
        AUTHORIZE_URL,
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&state),
        urlencoding::encode(scope),
        urlencoding::encode(&pkce.challenge)
    );

    Response::redirect(Url::parse(&authorize_url)?)
}

/// Handle Claude OAuth callback
///
/// GET /api/auth/claude/callback?code=...&state=...
/// Exchanges authorization code for tokens using PKCE verifier
pub async fn claude_callback(req: Request, env: Env) -> Result<Response> {
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

    // Check for error response
    if let Some((_, error)) = url.query_pairs().find(|(k, _)| k == "error") {
        let error_desc = url
            .query_pairs()
            .find(|(k, _)| k == "error_description")
            .map(|(_, v)| v.to_string())
            .unwrap_or_default();
        return Response::error(format!("OAuth error: {} - {}", error, error_desc), 400);
    }

    // Retrieve PKCE state from KV
    let kv = env.kv("SESSIONS")?;
    let pkce_state: OAuthPkceState = match kv.get(&format!("claude_oauth:{}", state)).text().await? {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| Error::RustError(format!("Invalid PKCE state: {}", e)))?,
        None => return Response::error("Invalid or expired OAuth state", 400),
    };

    // Delete used state
    kv.delete(&format!("claude_oauth:{}", state)).await?;

    // Get client ID
    let client_id = env.var("CLAUDE_CLIENT_ID")?.to_string();

    // Exchange code for tokens using PKCE verifier
    let token_response = claude::exchange_code(
        &code,
        &pkce_state.verifier,
        &pkce_state.redirect_uri,
        &client_id,
    )
    .await?;

    // Store Claude tokens in user's session
    // For now, we'll store them in a separate KV entry linked to the user
    let origin = url.origin().ascii_serialization();

    // Check if user is already logged in with GitHub
    if let Some(cookie) = req.headers().get("cookie")? {
        if let Some(session_token) = crate::db::sessions::extract_session_token(&cookie) {
            // User is logged in - store Claude tokens with their session
            let tokens_json = serde_json::to_string(&token_response)?;

            // Store Claude tokens linked to session
            kv.put(
                &format!("claude_tokens:{}", session_token),
                &tokens_json,
            )?
            .expiration_ttl(token_response.expires_in)
            .execute()
            .await?;

            // Redirect back to app with success
            let redirect_url = format!("{}?claude_connected=true", origin);
            return Response::redirect(Url::parse(&redirect_url)?);
        }
    }

    // User is not logged in - create a temporary token and redirect to login
    // They'll need to connect GitHub first, then Claude
    let temp_token = uuid::Uuid::new_v4().to_string();
    let tokens_json = serde_json::to_string(&token_response)?;

    // Store tokens temporarily (10 minutes)
    kv.put(&format!("claude_pending:{}", temp_token), &tokens_json)?
        .expiration_ttl(600)
        .execute()
        .await?;

    // Redirect to app with pending token - user needs to login with GitHub
    let redirect_url = format!("{}?claude_pending={}", origin, temp_token);
    Response::redirect(Url::parse(&redirect_url)?)
}

/// Get Claude connection status for current user
///
/// GET /api/auth/claude/status
/// Returns whether user has connected their Claude account
pub async fn claude_status(req: Request, env: Env) -> Result<Response> {
    // Get session token
    let cookie = req.headers().get("cookie")?.unwrap_or_default();
    let session_token = match crate::db::sessions::extract_session_token(&cookie) {
        Some(t) => t,
        None => {
            return Response::from_json(&serde_json::json!({
                "connected": false,
                "reason": "not_logged_in"
            }))
        }
    };

    // Check if Claude tokens exist for this session
    let kv = env.kv("SESSIONS")?;
    let has_tokens = kv
        .get(&format!("claude_tokens:{}", session_token))
        .text()
        .await?
        .is_some();

    Response::from_json(&serde_json::json!({
        "connected": has_tokens
    }))
}

/// Disconnect Claude account
///
/// POST /api/auth/claude/disconnect
/// Removes Claude tokens from user's session
pub async fn claude_disconnect(req: Request, env: Env) -> Result<Response> {
    // Get session token
    let cookie = req.headers().get("cookie")?.unwrap_or_default();
    let session_token = match crate::db::sessions::extract_session_token(&cookie) {
        Some(t) => t,
        None => return Response::error("Not logged in", 401),
    };

    // Delete Claude tokens
    let kv = env.kv("SESSIONS")?;
    kv.delete(&format!("claude_tokens:{}", session_token)).await?;

    Response::ok("Claude account disconnected")
}

/// Link pending Claude tokens after GitHub login
///
/// POST /api/auth/claude/link
/// Body: { "pending_token": "..." }
pub async fn claude_link(req: Request, env: Env, body: String) -> Result<Response> {
    // Get session token (user must be logged in now)
    let cookie = req.headers().get("cookie")?.unwrap_or_default();
    let session_token = match crate::db::sessions::extract_session_token(&cookie) {
        Some(t) => t,
        None => return Response::error("Not logged in", 401),
    };

    // Parse request body
    #[derive(Deserialize)]
    struct LinkRequest {
        pending_token: String,
    }

    let link_req: LinkRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request body: {}", e)))?;

    // Get pending Claude tokens
    let kv = env.kv("SESSIONS")?;
    let tokens_json = match kv
        .get(&format!("claude_pending:{}", link_req.pending_token))
        .text()
        .await?
    {
        Some(json) => json,
        None => return Response::error("Invalid or expired pending token", 400),
    };

    // Delete pending token
    kv.delete(&format!("claude_pending:{}", link_req.pending_token))
        .await?;

    // Parse to get expiration
    let token_response: claude::TokenResponse = serde_json::from_str(&tokens_json)
        .map_err(|e| Error::RustError(format!("Invalid token data: {}", e)))?;

    // Store with user's session
    kv.put(&format!("claude_tokens:{}", session_token), &tokens_json)?
        .expiration_ttl(token_response.expires_in)
        .execute()
        .await?;

    Response::ok("Claude account linked successfully")
}
