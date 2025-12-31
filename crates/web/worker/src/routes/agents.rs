//! Agent CRUD and proxy routes

use serde::{Deserialize, Serialize};
use worker::*;

use crate::db::agents;
use crate::AuthenticatedUser;

#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentResponse {
    pub agent_id: i64,
    pub name: Option<String>,
    pub status: String,
    pub nostr_npub: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// GET /api/agents
pub async fn list(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let db = env.d1("DB")?;
    let records = agents::list_for_user(&db, &user.user_id).await?;
    let response: Vec<AgentResponse> = records.into_iter().map(to_response).collect();
    Response::from_json(&response)
}

/// POST /api/agents
pub async fn create(user: AuthenticatedUser, env: Env, body: String) -> Result<Response> {
    let req = if body.trim().is_empty() {
        CreateAgentRequest { name: None }
    } else {
        serde_json::from_str(&body)
            .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?
    };

    let db = env.d1("DB")?;
    let record = agents::create(&db, &user.user_id, req.name.as_deref()).await?;
    init_agent_do(&env, &record.user_id, record.agent_id, record.name.as_deref()).await?;

    Response::from_json(&to_response(record))
}

/// GET /api/agents/:id
pub async fn get(user: AuthenticatedUser, env: Env, agent_id: i64) -> Result<Response> {
    let db = env.d1("DB")?;
    let record = agents::get_by_id(&db, &user.user_id, agent_id).await?;
    match record {
        Some(record) => Response::from_json(&to_response(record)),
        None => Response::error("Agent not found", 404),
    }
}

/// DELETE /api/agents/:id
pub async fn delete(user: AuthenticatedUser, env: Env, agent_id: i64) -> Result<Response> {
    let db = env.d1("DB")?;
    let existing = agents::get_by_id(&db, &user.user_id, agent_id).await?;
    if existing.is_none() {
        return Response::error("Agent not found", 404);
    }

    agents::soft_delete(&db, &user.user_id, agent_id).await?;
    Response::from_json(&serde_json::json!({ "ok": true }))
}

/// GET /api/agents/:id/do/status
pub async fn do_status(user: AuthenticatedUser, env: Env, agent_id: i64) -> Result<Response> {
    if !agent_exists(&env, &user.user_id, agent_id).await? {
        return Response::error("Agent not found", 404);
    }
    proxy_to_agent_do(&env, &user.user_id, agent_id, Method::Get, "/status").await
}

/// POST /api/agents/:id/do/tick
pub async fn do_tick(user: AuthenticatedUser, env: Env, agent_id: i64) -> Result<Response> {
    if !agent_exists(&env, &user.user_id, agent_id).await? {
        return Response::error("Agent not found", 404);
    }
    proxy_to_agent_do(&env, &user.user_id, agent_id, Method::Post, "/tick").await
}

fn to_response(record: agents::AgentRecord) -> AgentResponse {
    AgentResponse {
        agent_id: record.agent_id,
        name: record.name,
        status: record.status,
        nostr_npub: record.nostr_npub,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

async fn agent_exists(env: &Env, user_id: &str, agent_id: i64) -> Result<bool> {
    let db = env.d1("DB")?;
    let record = agents::get_by_id(&db, user_id, agent_id).await?;
    Ok(record.is_some())
}

async fn init_agent_do(
    env: &Env,
    user_id: &str,
    agent_id: i64,
    name: Option<&str>,
) -> Result<()> {
    let namespace = env.durable_object("AGENT_DO")?;
    let do_name = format!("agent:{}:{}", user_id, agent_id);
    let id = namespace.id_from_name(&do_name)?;
    let stub = id.get_stub()?;

    let payload = serde_json::json!({
        "user_id": user_id,
        "agent_id": agent_id,
        "name": name,
    });
    let init_req = Request::new_with_init(
        "http://internal/init",
        RequestInit::new()
            .with_method(Method::Post)
            .with_body(Some(payload.to_string().into())),
    )?;

    let resp = stub.fetch_with_request(init_req).await?;
    if resp.status_code() >= 400 {
        return Err(Error::RustError("Agent DO init failed".to_string()));
    }

    Ok(())
}

async fn proxy_to_agent_do(
    env: &Env,
    user_id: &str,
    agent_id: i64,
    method: Method,
    path: &str,
) -> Result<Response> {
    let namespace = env.durable_object("AGENT_DO")?;
    let do_name = format!("agent:{}:{}", user_id, agent_id);
    let id = namespace.id_from_name(&do_name)?;
    let stub = id.get_stub()?;

    let req = Request::new_with_init(
        &format!("http://internal{}", path),
        RequestInit::new().with_method(method),
    )?;

    stub.fetch_with_request(req).await
}
