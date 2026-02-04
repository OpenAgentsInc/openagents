use serde::Serialize;
use worker::{Env, Method, Request, Response, RouteContext, Result};

use crate::openclaw::billing;
use crate::openclaw::cf;
use crate::openclaw::convex;
use crate::openclaw::runtime_client::{RuntimeClient, RuntimeResult};
use crate::openclaw::{INTERNAL_KEY_HEADER, USER_ID_HEADER};
use crate::ApiResponse;

#[derive(Debug, Serialize)]
struct InstanceSummary {
    status: String,
    runtime_name: Option<String>,
    created_at: i64,
    updated_at: i64,
    last_ready_at: Option<i64>,
}

fn instance_summary(instance: &convex::OpenclawInstance) -> InstanceSummary {
    InstanceSummary {
        status: instance.status.clone(),
        runtime_name: instance.runtime_name.clone(),
        created_at: instance.created_at,
        updated_at: instance.updated_at,
        last_ready_at: instance.last_ready_at,
    }
}

fn internal_key(env: &Env) -> Option<String> {
    env.var("OA_INTERNAL_KEY")
        .ok()
        .map(|v| v.to_string())
        .filter(|v| !v.trim().is_empty())
}

fn service_token_from_env(env: &Env) -> Option<String> {
    env.var("OPENCLAW_SERVICE_TOKEN")
        .ok()
        .map(|v| v.to_string())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            env.var("OPENAGENTS_SERVICE_TOKEN")
                .ok()
                .map(|v| v.to_string())
                .filter(|v| !v.trim().is_empty())
        })
}

fn resolve_internal_user(req: &Request, env: &Env) -> std::result::Result<Option<String>, Response> {
    let provided = req.headers().get(INTERNAL_KEY_HEADER).ok().flatten();
    let Some(provided) = provided else {
        return Ok(None);
    };

    let expected = match internal_key(env) {
        Some(value) => value,
        None => {
            let response = crate::json_error("OA_INTERNAL_KEY not configured", 500)
                .unwrap_or_else(|_| Response::error("OA_INTERNAL_KEY not configured", 500).unwrap());
            return Err(response);
        }
    };

    if provided.trim() != expected {
        return Err(crate::json_unauthorized("unauthorized"));
    }

    let user_id = req
        .headers()
        .get(USER_ID_HEADER)
        .ok()
        .flatten()
        .unwrap_or_default();
    if user_id.trim().is_empty() {
        let response = crate::json_error("missing user id", 400)
            .unwrap_or_else(|_| Response::error("missing user id", 400).unwrap());
        return Err(response);
    }

    Ok(Some(user_id))
}

async fn require_openclaw_user(req: &Request, env: &Env) -> std::result::Result<String, Response> {
    match resolve_internal_user(req, env) {
        Ok(Some(user_id)) => return Ok(user_id),
        Ok(None) => {}
        Err(response) => return Err(response),
    }

    let token = match crate::api_token_from_request(req) {
        Some(value) => value,
        None => return Err(crate::json_unauthorized("missing api token")),
    };

    match crate::resolve_api_token(env, &token).await {
        Ok(Some(resolved)) => Ok(resolved.user_id),
        Ok(None) => Err(crate::json_unauthorized("invalid api token")),
        Err(err) => {
            let response = crate::json_error(&err.to_string(), 502)
                .unwrap_or_else(|_| Response::error("auth resolution failed", 502).unwrap());
            Err(response)
        }
    }
}

fn json_ok<T: Serialize>(data: Option<T>) -> Result<Response> {
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data,
        error: None,
    })?;
    crate::apply_cors(&mut response)?;
    Ok(response)
}

fn json_runtime_result(result: RuntimeResult<serde_json::Value>) -> Result<Response> {
    if result.envelope.ok {
        return json_ok(result.envelope.data);
    }

    let message = result
        .envelope
        .error
        .map(|err| err.message)
        .unwrap_or_else(|| "runtime error".to_string());
    let status = if (400..=599).contains(&result.status) {
        result.status
    } else {
        502
    };
    crate::json_error(&message, status)
}

fn runtime_setup_error(err: worker::Error) -> Result<Response> {
    let message = err.to_string();
    let status = if message.contains("instance not found") {
        404
    } else if message.contains("service token not set") || message.contains("runtime url not set") {
        409
    } else {
        500
    };
    crate::json_error(&message, status)
}

fn forward_gateway_headers(req: &Request) -> Vec<(String, String)> {
    let allowlist = [
        "x-openclaw-session-key",
        "x-openclaw-agent-id",
        "x-openclaw-message-channel",
        "x-openclaw-account-id",
    ];
    let mut collected = Vec::new();
    for (name, value) in req.headers().entries() {
        let normalized = name.to_lowercase();
        if !allowlist.contains(&normalized.as_str()) {
            continue;
        }
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        collected.push((normalized, trimmed.to_string()));
    }
    collected
}

async fn runtime_client_for_user(env: &Env, user_id: &str) -> Result<RuntimeClient> {
    let instance = convex::get_instance(env, user_id)
        .await?
        .ok_or_else(|| worker::Error::RustError("instance not found".to_string()))?;
    let runtime_url = instance
        .runtime_url
        .ok_or_else(|| worker::Error::RustError("runtime url not set".to_string()))?;
    let service_token = convex::get_secret(env, user_id, "service_token")
        .await?
        .ok_or_else(|| worker::Error::RustError("service token not set".to_string()))?;
    Ok(RuntimeClient::new(runtime_url, service_token))
}

pub async fn handle_instance_get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let instance = match convex::get_instance(&ctx.env, &user_id).await {
        Ok(opt) => opt,
        Err(err) => {
            let msg = format!("openclaw getInstance: {}", err);
            return crate::json_error(&msg, 500);
        }
    };
    let summary = instance.as_ref().map(instance_summary);
    json_ok(summary)
}

pub async fn handle_instance_post(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let existing = match convex::get_instance(&ctx.env, &user_id).await {
        Ok(opt) => opt,
        Err(err) => {
            let msg = format!("openclaw createInstance get_instance: {}", err);
            return crate::json_error(&msg, 500);
        }
    };
    if let Some(ex) = existing {
        if ex.status != "error" && ex.status != "deleted" {
            return json_ok(Some(instance_summary(&ex)));
        }
    }

    if let Err(err) = convex::upsert_instance(
        &ctx.env,
        serde_json::json!({
            "user_id": user_id,
            "status": "provisioning",
        }),
    )
    .await
    {
        let msg = format!("openclaw createInstance upsert (provisioning): {}", err);
        return crate::json_error(&msg, 500);
    }

    let provisioned = match cf::provision_instance(&ctx.env, &user_id).await {
        Ok(value) => value,
        Err(err) => {
            let _ = convex::set_status(&ctx.env, &user_id, "error").await;
            let msg = format!("openclaw createInstance provision_instance: {}", err);
            return crate::json_error(&msg, 500);
        }
    };

    let service_token = service_token_from_env(&ctx.env)
        .unwrap_or_else(|| crate::random_token("oa_svc_", 32));
    if let Err(err) = convex::store_secret(&ctx.env, &user_id, "service_token", &service_token).await {
        let msg = format!("openclaw createInstance store_secret: {}", err);
        return crate::json_error(&msg, 500);
    }

    let updated = match convex::upsert_instance(
        &ctx.env,
        serde_json::json!({
            "user_id": user_id,
            "status": "ready",
            "runtime_url": provisioned.runtime_url,
            "runtime_name": provisioned.runtime_name,
            "cf_account_id": provisioned.cf_account_id,
            "cf_worker_name": provisioned.cf_worker_name,
            "cf_worker_id": provisioned.cf_worker_id,
            "cf_container_app_id": provisioned.cf_container_app_id,
            "cf_container_app_name": provisioned.cf_container_app_name,
            "r2_bucket_name": provisioned.r2_bucket_name,
        }),
    )
    .await
    {
        Ok(u) => u,
        Err(err) => {
            let msg = format!("openclaw createInstance upsert (ready): {}", err);
            return crate::json_error(&msg, 500);
        }
    };

    json_ok(Some(instance_summary(&updated)))
}

pub async fn handle_instance_delete(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let result = match convex::delete_instance(&ctx.env, &user_id).await {
        Ok(res) => res,
        Err(err) => {
            let msg = format!("openclaw deleteInstance: {}", err);
            return crate::json_error(&msg, 500);
        }
    };

    json_ok(Some(result))
}

pub async fn handle_runtime_status(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.status().await?;
    json_runtime_result(result)
}

pub async fn handle_runtime_devices(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.devices().await?;
    json_runtime_result(result)
}

pub async fn handle_runtime_device_approve(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let request_id = ctx
        .param("requestId")
        .map(|value| value.as_str())
        .unwrap_or("");
    if request_id.trim().is_empty() {
        return crate::json_error("missing request id", 400);
    }

    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.approve_device(request_id).await?;
    json_runtime_result(result)
}

pub async fn handle_runtime_pairing_list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let channel = ctx.param("channel").map(|value| value.as_str()).unwrap_or("");
    if channel.trim().is_empty() {
        return crate::json_error("missing channel", 400);
    }

    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.pairing_list(channel).await?;
    json_runtime_result(result)
}

pub async fn handle_runtime_pairing_approve(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let channel = ctx.param("channel").map(|value| value.as_str()).unwrap_or("");
    if channel.trim().is_empty() {
        return crate::json_error("missing channel", 400);
    }

    let body: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let code = body
        .get("code")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if code.is_empty() {
        return crate::json_error("missing code", 400);
    }
    let notify = body.get("notify").and_then(|value| value.as_bool());

    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.pairing_approve(channel, &code, notify).await?;
    json_runtime_result(result)
}

pub async fn handle_runtime_backup(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.backup().await?;
    json_runtime_result(result)
}

pub async fn handle_runtime_restart(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.restart().await?;
    json_runtime_result(result)
}

pub async fn handle_tools_invoke(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let body: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.tools_invoke(body).await?;
    json_runtime_result(result)
}

pub async fn handle_sessions_list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let query = req.url()?.query().map(|value| value.to_string());
    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.sessions_list(query.as_deref()).await?;
    json_runtime_result(result)
}

pub async fn handle_sessions_history(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let session_key = ctx.param("key").map(|value| value.as_str()).unwrap_or("");
    if session_key.trim().is_empty() {
        return crate::json_error("missing session key", 400);
    }

    let query = req.url()?.query().map(|value| value.to_string());
    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.sessions_history(session_key, query.as_deref()).await?;
    json_runtime_result(result)
}

pub async fn handle_sessions_send(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let session_key = ctx.param("key").map(|value| value.as_str()).unwrap_or("");
    if session_key.trim().is_empty() {
        return crate::json_error("missing session key", 400);
    }

    let body: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };
    let result = client.sessions_send(session_key, body).await?;
    json_runtime_result(result)
}

pub async fn handle_openclaw_chat(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let body_text = req.text().await.unwrap_or_default();
    if body_text.trim().is_empty() {
        return crate::json_error("missing request body", 400);
    }

    let extra_headers = forward_gateway_headers(&req);
    let client = match runtime_client_for_user(&ctx.env, &user_id).await {
        Ok(client) => client,
        Err(err) => return runtime_setup_error(err),
    };

    let mut upstream = client.responses_stream(body_text, &extra_headers).await?;
    let status = upstream.status_code();
    if status >= 400 {
        let message = upstream.text().await.unwrap_or_else(|_| "runtime error".to_string());
        return crate::json_error(&message, status);
    }

    let body = upstream.body().clone();
    let mut response = Response::from_body(body)?.with_status(status);
    response
        .headers_mut()
        .set("content-type", "text/event-stream; charset=utf-8")?;
    response.headers_mut().set("cache-control", "no-cache")?;
    response.headers_mut().set("connection", "keep-alive")?;
    crate::apply_cors(&mut response)?;
    Ok(response)
}

pub async fn handle_billing_summary(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let user_id = match require_openclaw_user(&req, &ctx.env).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };

    let summary = billing::get_summary(&ctx.env, &user_id).await?;
    json_ok(Some(summary))
}

pub async fn handle_openclaw_index(req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    if req.method() != Method::Get {
        return crate::json_error("method not allowed", 405);
    }
    let mut response = Response::from_json(&ApiResponse::<serde_json::Value> {
        ok: true,
        data: Some(serde_json::json!({
            "instance": "/api/openclaw/instance",
            "runtime_status": "/api/openclaw/runtime/status",
            "runtime_devices": "/api/openclaw/runtime/devices",
            "runtime_backup": "/api/openclaw/runtime/backup",
            "runtime_restart": "/api/openclaw/runtime/restart",
            "instance_delete": "/api/openclaw/instance",
            "tools_invoke": "/api/openclaw/tools/invoke",
            "sessions": "/api/openclaw/sessions",
            "sessions_history": "/api/openclaw/sessions/:key/history",
            "sessions_send": "/api/openclaw/sessions/:key/send",
            "chat": "/api/openclaw/chat",
            "billing_summary": "/api/openclaw/billing/summary"
        })),
        error: None,
    })?;
    crate::apply_cors(&mut response)?;
    Ok(response)
}
