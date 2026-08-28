use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use openagents_coder_contract::{
    ApiErrorBody, Credit, CreditEnvelope, MintedGrant, GRANT_REVOKED, MODEL_NOT_SERVED,
    MODEL_UNAVAILABLE,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::catalog;
use crate::config::Config;
use crate::proxy;
use crate::store::{mint_token, Store, ThreadRow};

#[derive(Clone)]
pub struct App {
    pub config: Config,
    pub store: Arc<Store>,
}

pub fn router(app: App) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/models", get(models))
        .route("/api/v1/credit", get(credit))
        .route("/api/v1/user", get(user))
        .route("/api/v1/threads", get(list_threads).post(open_thread))
        .route(
            "/api/v1/threads/{thread_id}",
            get(show_thread).delete(cancel_thread),
        )
        .route(
            "/api/v1/threads/{thread_id}/events",
            get(list_events).post(record_events),
        )
        .route("/api/v1/threads/{thread_id}/report", post(report_thread))
        .route("/api/v1/threads/{thread_id}/grants", post(remint))
        .route("/api/inference/proxy", post(inference_proxy))
        .layer(middleware::from_fn_with_state(app.clone(), require_bearer))
        .with_state(app)
}

async fn require_bearer(
    State(app): State<App>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if path == "/health" || !app.config.require_bearer {
        return next.run(request).await;
    }
    if bearer(request.headers()).is_none() {
        return fail(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "bearer token required",
        );
    }
    next.run(request).await
}

async fn health(State(app): State<App>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "service": "openagents-coder-api",
        "providers": {
            "vercel_gateway": app.config.vercel_configured(),
            "openrouter": app.config.openrouter_configured(),
        }
    }))
}

async fn user(headers: HeaderMap) -> Response {
    if bearer(&headers).is_none() {
        return fail(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "bearer token required",
        );
    }
    Json(json!({
        "id": 0,
        "login": "local",
        "token_expires_at": "9999-12-31T23:59:59Z",
        "namespaces": []
    }))
    .into_response()
}

async fn list_threads(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = principal(&headers);
    match app.store.list_threads(&owner) {
        Ok(threads) => Json(json!({
            "threads": threads.iter().map(thread_view).collect::<Vec<_>>()
        }))
        .into_response(),
        Err(error) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store",
            &error.to_string(),
        ),
    }
}

async fn models(State(app): State<App>) -> Json<Value> {
    Json(serde_json::to_value(catalog::project(&app.config)).unwrap_or(json!({})))
}

async fn credit(State(app): State<App>, headers: HeaderMap) -> Json<CreditEnvelope> {
    let owner = principal(&headers);
    let allowance = app.config.credit_allowance_microusd;
    let (tokens, unpriced, complete) = app.store.credit(&owner, allowance).unwrap_or((0, 0, true));
    let _ = tokens;
    Json(CreditEnvelope {
        credit: Credit {
            allowance_microusd: allowance,
            spent_microusd: 0,
            remaining_microusd: allowance,
            unpriced_calls: unpriced,
            complete,
        },
    })
}

async fn open_thread(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let owner = principal(&headers);
    let objective = body
        .get("objective")
        .and_then(Value::as_str)
        .unwrap_or("Coding assistant session");
    let lane = body.get("lane").and_then(Value::as_str).unwrap_or("thread");
    let requested = body
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if lane == "local" {
        let thread = match app.store.open_thread(&owner, objective, "local", requested) {
            Ok(thread) => thread,
            Err(error) => {
                return fail(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "store",
                    &error.to_string(),
                )
            }
        };
        return (
            StatusCode::CREATED,
            Json(json!({"thread": thread_view(&thread)})),
        )
            .into_response();
    }

    let model_id = requested.unwrap_or(catalog::default_id());
    let Some(model) = catalog::fetch(&app.config, model_id) else {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            MODEL_NOT_SERVED,
            &format!("`{model_id}` is not served here"),
        );
    };
    if !catalog::available(&app.config, &model) {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            MODEL_UNAVAILABLE,
            &format!("`{model_id}` is in the catalog but its provider is not configured"),
        );
    }

    let thread = match app
        .store
        .open_thread(&owner, objective, "thread", Some(&model.id))
    {
        Ok(thread) => thread,
        Err(error) => {
            return fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "store",
                &error.to_string(),
            )
        }
    };
    let token = mint_token();
    if let Err(error) = app.store.mint_grant(&thread, &model.id, &token) {
        return fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store",
            &error.to_string(),
        );
    }
    let grant = MintedGrant {
        token,
        url: format!(
            "{}/api/inference/proxy",
            app.config.public_origin.trim_end_matches('/')
        ),
        model: model.id,
        expires_at: None,
    };
    (
        StatusCode::CREATED,
        Json(json!({
            "thread": thread_view(&thread),
            "grant": grant
        })),
    )
        .into_response()
}

async fn show_thread(
    State(app): State<App>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
) -> Response {
    let owner = principal(&headers);
    match app.store.get_thread(&thread_id, &owner) {
        Ok(Some(thread)) => Json(json!({"thread": thread_view(&thread)})).into_response(),
        Ok(None) => fail(StatusCode::NOT_FOUND, "not_found", "thread not found"),
        Err(error) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store",
            &error.to_string(),
        ),
    }
}

async fn cancel_thread(
    State(app): State<App>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
) -> Response {
    let owner = principal(&headers);
    match app.store.get_thread(&thread_id, &owner) {
        Ok(Some(_)) => {
            let _ = app.store.finish(
                &thread_id,
                "cancelled",
                "The thread was cancelled before it reported.",
                Some("cancelled"),
            );
            Json(json!({"thread": {"id": thread_id, "status": "cancelled"}})).into_response()
        }
        Ok(None) => fail(StatusCode::NOT_FOUND, "not_found", "thread not found"),
        Err(error) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store",
            &error.to_string(),
        ),
    }
}

async fn report_thread(
    State(app): State<App>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let owner = principal(&headers);
    match app.store.get_thread(&thread_id, &owner) {
        Ok(Some(_)) => {
            let status = body
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("succeeded");
            let report = body.get("report").and_then(Value::as_str).unwrap_or("");
            let error_code = body.get("error_code").and_then(Value::as_str);
            let _ = app.store.finish(&thread_id, status, report, error_code);
            Json(json!({"thread": {"id": thread_id, "status": status}})).into_response()
        }
        Ok(None) => fail(StatusCode::NOT_FOUND, "not_found", "thread not found"),
        Err(error) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store",
            &error.to_string(),
        ),
    }
}

async fn remint(
    State(app): State<App>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
) -> Response {
    let owner = principal(&headers);
    let token = mint_token();
    match app.store.bump_and_mint(&thread_id, &owner, &token) {
        Ok(Some((thread, grant))) => Json(json!({
            "thread": thread_view(&thread),
            "grant": MintedGrant {
                token,
                url: format!("{}/api/inference/proxy", app.config.public_origin.trim_end_matches('/')),
                model: grant.model,
                expires_at: None,
            }
        }))
        .into_response(),
        Ok(None) => fail(StatusCode::NOT_FOUND, "not_found", "thread not found"),
        Err(error) => fail(StatusCode::INTERNAL_SERVER_ERROR, "store", &error.to_string()),
    }
}

async fn record_events(
    State(app): State<App>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let owner = principal(&headers);
    if app
        .store
        .get_thread(&thread_id, &owner)
        .ok()
        .flatten()
        .is_none()
    {
        return fail(StatusCode::NOT_FOUND, "not_found", "thread not found");
    }
    let mut entries = Vec::new();
    if let Some(events) = body.get("events").and_then(Value::as_array) {
        for event in events {
            let event_type = event
                .get("event_type")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let payload = event.get("payload").cloned().unwrap_or(json!({}));
            entries.push((event_type, payload));
        }
    } else if let Some(event_type) = body.get("event_type").and_then(Value::as_str) {
        entries.push((
            event_type.to_string(),
            body.get("payload").cloned().unwrap_or(json!({})),
        ));
    }
    let _ = app.store.append_events(&thread_id, &entries);
    Json(json!({"ok": true, "count": entries.len()})).into_response()
}

async fn list_events(
    State(app): State<App>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
) -> Response {
    let owner = principal(&headers);
    if app
        .store
        .get_thread(&thread_id, &owner)
        .ok()
        .flatten()
        .is_none()
    {
        return fail(StatusCode::NOT_FOUND, "not_found", "thread not found");
    }
    let events = app.store.list_events(&thread_id).unwrap_or_default();
    Json(json!({
        "thread_id": thread_id,
        "event_count": events.len(),
        "events": events.into_iter().map(|(event_type, payload)| json!({
            "event_type": event_type,
            "payload": payload
        })).collect::<Vec<_>>()
    }))
    .into_response()
}

async fn inference_proxy(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let Some(token) = bearer(&headers) else {
        return fail(
            StatusCode::UNAUTHORIZED,
            GRANT_REVOKED,
            "grant token required",
        );
    };
    let grant = match app.store.grant_by_token(&token) {
        Ok(Some(grant)) if grant.status == "active" => grant,
        Ok(Some(_)) => return fail(StatusCode::UNAUTHORIZED, GRANT_REVOKED, "grant is revoked"),
        Ok(None) => return fail(StatusCode::UNAUTHORIZED, GRANT_REVOKED, "unknown grant"),
        Err(error) => {
            return fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "store",
                &error.to_string(),
            )
        }
    };

    let Some(model) = catalog::fetch(&app.config, &grant.model) else {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            MODEL_UNAVAILABLE,
            "grant model is gone",
        );
    };
    if !catalog::available(&app.config, &model) {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            MODEL_UNAVAILABLE,
            "grant model is not configured",
        );
    }
    if let Some(requested) = body.get("model").and_then(Value::as_str) {
        if requested != model.id && requested != model.provider_model {
            return fail(
                StatusCode::UNPROCESSABLE_ENTITY,
                openagents_coder_contract::MODEL_MISMATCH,
                &format!("body named `{requested}`, grant is `{}`", model.id),
            );
        }
    }

    // Default Flash grants open on GLM. A greeting still arrives with that
    // grant from packaged CLIs (rc8). Route the call to Gemini when the last
    // user turn is trivial; keep the Explore pin (gemini grants) intact.
    let model = reroute_simple_flash(&app.config, &grant.model, &body, model);

    match proxy::stream_completion(&app.config, &model, body, true).await {
        Ok(response) => {
            let _ = app.store.record_usage(&grant.token_digest, 0, 0, 0);
            response
        }
        Err((status, message)) => {
            fail(status, openagents_coder_contract::PROVIDER_FAILED, &message)
        }
    }
}

fn reroute_simple_flash(
    config: &Config,
    grant_model: &str,
    body: &Value,
    granted: crate::catalog::Model,
) -> crate::catalog::Model {
    let Some(text) = openagents_coder_contract::classify::last_user_text(
        body.get("messages").unwrap_or(&Value::Null),
    ) else {
        return granted;
    };
    let Some(simple_id) =
        openagents_coder_contract::classify::maybe_simple_flash(grant_model, &text)
    else {
        return granted;
    };
    catalog::fetch(config, simple_id)
        .filter(|candidate| catalog::available(config, candidate))
        .unwrap_or(granted)
}

fn thread_view(thread: &ThreadRow) -> Value {
    json!({
        "id": thread.id,
        "status": thread.status,
        "objective": thread.objective,
        "generation": thread.generation,
        "model": thread.model,
    })
}

fn principal(headers: &HeaderMap) -> String {
    match bearer(headers) {
        Some(token) => {
            let mut hasher = Sha256::new();
            hasher.update(token.as_bytes());
            format!("local_{}", hex::encode(&hasher.finalize()[..8]))
        }
        None => "local".to_string(),
    }
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
}

fn fail(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(ApiErrorBody {
            code: code.to_string(),
            message: message.to_string(),
            errors: None,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use crate::config::Config;
    use std::path::PathBuf;

    fn cfg(gateway: bool) -> Config {
        Config {
            bind: "127.0.0.1:0".into(),
            db_path: PathBuf::from("/tmp/unused.sqlite"),
            public_origin: "http://127.0.0.1".into(),
            ai_gateway_api_key: gateway.then(|| "k".into()),
            openrouter_api_key: None,
            credit_allowance_microusd: 0,
            require_bearer: false,
        }
    }

    #[test]
    fn a_glm_grant_reroutes_hey_to_gemini() {
        let config = cfg(true);
        let granted = catalog::fetch(&config, "glm-5.3-flash").unwrap();
        let body = json!({"messages":[{"role":"user","content":"hey"}]});
        let out = reroute_simple_flash(&config, "glm-5.3-flash", &body, granted);
        assert_eq!(out.id, "gemini-3.7-flash");
    }

    #[test]
    fn a_glm_grant_keeps_coding_work_on_glm() {
        let config = cfg(true);
        let granted = catalog::fetch(&config, "glm-5.3-flash").unwrap();
        let body = json!({
            "messages":[{"role":"user","content":"implement the proxy classifier"}]
        });
        let out = reroute_simple_flash(&config, "glm-5.3-flash", &body, granted);
        assert_eq!(out.id, "glm-5.3-flash");
    }

    #[test]
    fn a_gemini_grant_is_not_rewritten() {
        let config = cfg(true);
        let granted = catalog::fetch(&config, "gemini-3.7-flash").unwrap();
        let body = json!({"messages":[{"role":"user","content":"hey"}]});
        let out = reroute_simple_flash(&config, "gemini-3.7-flash", &body, granted);
        assert_eq!(out.id, "gemini-3.7-flash");
    }
}
