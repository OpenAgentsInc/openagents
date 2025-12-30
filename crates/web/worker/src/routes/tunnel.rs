//! Tunnel relay routes
//!
//! Handles tunnel session registration and WebSocket routing to Durable Objects.

use crate::relay::TunnelSession;
use serde::{Deserialize, Serialize};
use worker::*;

/// Request to register a new tunnel session
#[derive(Deserialize)]
pub struct RegisterRequest {
    pub repo: String,
}

/// Response from tunnel registration
#[derive(Serialize)]
pub struct RegisterResponse {
    pub session_id: String,
    pub tunnel_token: String,
    pub tunnel_url: String,
    pub browser_url: String,
}

/// Register a new tunnel session
///
/// POST /api/tunnel/register
/// Body: { "repo": "owner/repo" }
///
/// Creates a session and returns tokens for both browser and tunnel to connect.
pub async fn register_with_origin(
    env: Env,
    user: crate::AuthenticatedUser,
    body: String,
    origin: String,
) -> Result<Response> {
    // Parse request
    let register_req: RegisterRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;

    // Create session
    let session = TunnelSession::new(&user.user_id, &register_req.repo);

    // Save to KV
    let kv = env.kv("SESSIONS")?;
    session.save(&kv).await?;

    // Build WebSocket URLs
    let ws_origin = if origin.starts_with("https://") {
        origin.replace("https://", "wss://")
    } else {
        origin.replace("http://", "ws://")
    };

    Response::from_json(&RegisterResponse {
        session_id: session.session_id.clone(),
        tunnel_token: session.tunnel_token.clone(),
        tunnel_url: format!(
            "{}/api/tunnel/ws/tunnel?session_id={}&token={}",
            ws_origin, session.session_id, session.tunnel_token
        ),
        browser_url: format!(
            "{}/api/tunnel/ws/browser?session_id={}",
            ws_origin, session.session_id
        ),
    })
}

/// Get tunnel session status
///
/// GET /api/tunnel/status/:session_id
pub async fn status(env: Env, session_id: String) -> Result<Response> {
    let kv = env.kv("SESSIONS")?;

    match TunnelSession::load(&kv, &session_id).await? {
        Some(session) => {
            // Get DO to check if tunnel is connected
            let namespace = env.durable_object("TUNNEL_RELAY")?;
            let stub = namespace.id_from_name(&session_id)?.get_stub()?;

            // Ping the DO to check status
            let status_req = Request::new(
                &format!("https://internal/status?session_id={}", session_id),
                Method::Get,
            )?;

            // For now, just return session info
            // TODO: Actually query DO for tunnel connection status
            Response::from_json(&serde_json::json!({
                "session_id": session.session_id,
                "repo": session.repo,
                "created_at": session.created_at,
                "tunnel_connected": false // Will be updated when we query DO
            }))
        }
        None => Response::error("Session not found", 404),
    }
}

/// Route WebSocket connection to Durable Object
///
/// GET /api/tunnel/ws/:type (browser or tunnel)
/// Upgrades to WebSocket and routes to session's Durable Object.
pub async fn websocket(req: Request, env: Env) -> Result<Response> {
    let url = req.url()?;
    let path = url.path();

    // Extract connection type from path
    let conn_type = if path.contains("/browser") {
        "browser"
    } else if path.contains("/tunnel") {
        "tunnel"
    } else {
        return Response::error("Invalid WebSocket path", 400);
    };

    // Get session ID
    let session_id = url
        .query_pairs()
        .find(|(k, _)| k == "session_id")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| Error::RustError("Missing session_id".to_string()))?;

    // For tunnel connections, validate token
    if conn_type == "tunnel" {
        let token = url
            .query_pairs()
            .find(|(k, _)| k == "token")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| Error::RustError("Missing token".to_string()))?;

        let kv = env.kv("SESSIONS")?;
        let valid_session = TunnelSession::validate_token(&kv, &token).await?;

        if valid_session.as_deref() != Some(&session_id) {
            return Response::error("Invalid tunnel token", 401);
        }
    }

    // Get the Durable Object for this session
    let namespace = env.durable_object("TUNNEL_RELAY")?;
    let id = namespace.id_from_name(&session_id)?;
    let stub = id.get_stub()?;

    // Forward the request to the Durable Object
    let do_url = format!("/ws/{}?session_id={}", conn_type, session_id);
    let mut new_req = Request::new(&do_url, Method::Get)?;

    // Copy headers (especially Upgrade header for WebSocket)
    for (key, value) in req.headers() {
        new_req.headers_mut()?.set(&key, &value)?;
    }

    stub.fetch_with_request(new_req).await
}
