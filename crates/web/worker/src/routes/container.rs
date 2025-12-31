//! Container task routes
//!
//! Handles starting and managing autopilot tasks in Cloudflare Containers.
//! This is the "paid tier" - users pay credits for cloud compute.

use serde::{Deserialize, Serialize};
use worker::*;

use crate::AuthenticatedUser;

/// Request to start a container task
#[derive(Deserialize)]
pub struct StartContainerRequest {
    pub repo: String,
    pub prompt: String,
}

/// Response from starting a container task
#[derive(Serialize)]
pub struct StartContainerResponse {
    pub session_id: String,
    pub status: String,
    pub ws_url: String,
}

/// Container session stored in KV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSession {
    pub session_id: String,
    pub user_id: String,
    pub repo: String,
    pub created_at: u64,
}

impl ContainerSession {
    pub fn new(user_id: &str, repo: &str) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            repo: repo.to_string(),
            created_at: js_sys::Date::now() as u64 / 1000,
        }
    }

    pub async fn save(&self, kv: &kv::KvStore) -> Result<()> {
        let json = serde_json::to_string(self)?;
        kv.put(&format!("container_session:{}", self.session_id), &json)?
            .expiration_ttl(86400) // 24 hours
            .execute()
            .await?;
        Ok(())
    }

    pub async fn load(kv: &kv::KvStore, session_id: &str) -> Result<Option<Self>> {
        let result = kv
            .get(&format!("container_session:{}", session_id))
            .text()
            .await
            .map_err(|e| Error::RustError(format!("KV error: {:?}", e)))?;
        match result {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }
}

/// Start a new container task
///
/// POST /api/container/start
/// Body: { "repo": "owner/repo", "prompt": "do something" }
pub async fn start_task(
    user: AuthenticatedUser,
    env: Env,
    body: String,
) -> Result<Response> {
    // Parse request
    let req: StartContainerRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;

    // TODO: Check user has credits
    // let db = env.d1("DB")?;
    // let balance = db::billing::get_balance(&db, &user.user_id).await?;
    // if balance <= 0 { return Response::error("Insufficient credits", 402); }

    // Create session
    let session = ContainerSession::new(&user.user_id, &req.repo);

    // Save to KV
    let kv = env.kv("SESSIONS")?;
    session.save(&kv).await?;

    // Get the Durable Object for this user
    let namespace = env.durable_object("AUTOPILOT_CONTAINER")?;
    let id = namespace.id_from_name(&user.user_id)?;
    let stub = id.get_stub()?;

    // Forward start request to container
    let start_body = serde_json::json!({
        "repo": req.repo,
        "prompt": req.prompt,
    });

    let start_req = Request::new_with_init(
        "http://internal/api/start",
        RequestInit::new()
            .with_method(Method::Post)
            .with_body(Some(start_body.to_string().into())),
    )?;

    // Send to DO (which forwards to container)
    let _resp = stub.fetch_with_request(start_req).await?;

    // Build WebSocket URL
    let ws_url = format!("/api/container/ws/{}", session.session_id);

    Response::from_json(&StartContainerResponse {
        session_id: session.session_id,
        status: "starting".to_string(),
        ws_url,
    })
}

/// Get container task status
///
/// GET /api/container/status
pub async fn get_status(user: AuthenticatedUser, env: Env) -> Result<Response> {
    // Get the Durable Object for this user
    let namespace = env.durable_object("AUTOPILOT_CONTAINER")?;
    let id = namespace.id_from_name(&user.user_id)?;
    let stub = id.get_stub()?;

    // Query status from DO
    let status_req = Request::new("http://internal/status", Method::Get)?;
    stub.fetch_with_request(status_req).await
}

/// WebSocket handler for streaming container events
///
/// GET /api/container/ws/:session_id
pub async fn websocket(req: Request, env: Env) -> Result<Response> {
    let url = req.url()?;
    let path = url.path();

    // Extract session_id from path
    let session_id = path
        .trim_start_matches("/api/container/ws/")
        .to_string();

    if session_id.is_empty() {
        return Response::error("Missing session_id", 400);
    }

    // Load session to get user_id
    let kv = env.kv("SESSIONS")?;
    let session = match ContainerSession::load(&kv, &session_id).await? {
        Some(s) => s,
        None => return Response::error("Session not found", 404),
    };

    // Get the Durable Object for the user
    let namespace = env.durable_object("AUTOPILOT_CONTAINER")?;
    let id = namespace.id_from_name(&session.user_id)?;
    let stub = id.get_stub()?;

    // Forward WebSocket request to DO
    let ws_req = Request::new_with_init(
        "http://internal/ws",
        RequestInit::new().with_method(Method::Get),
    )?;

    // Copy upgrade header
    let mut new_req = ws_req;
    for (key, value) in req.headers() {
        new_req.headers_mut()?.set(&key, &value)?;
    }

    stub.fetch_with_request(new_req).await
}
