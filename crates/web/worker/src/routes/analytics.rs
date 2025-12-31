//! Funnel analytics events for GTM.

use crate::middleware::auth::AuthenticatedUser;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::{Env, Error, Response, Result};

#[derive(Deserialize)]
struct FunnelEventRequest {
    event: String,
    repo: Option<String>,
}

const ALLOWED_EVENTS: [&str; 5] = [
    "landing_view",
    "github_connect_click",
    "repo_selected",
    "hud_share",
    "hud_embed",
];

pub async fn track_event(
    env: Env,
    user: Option<AuthenticatedUser>,
    body: String,
) -> Result<Response> {
    let payload: FunnelEventRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;

    if !ALLOWED_EVENTS.contains(&payload.event.as_str()) {
        return Response::error("Unsupported event", 400);
    }

    let db = env.d1("DB")?;
    let created_at = chrono::Utc::now().timestamp();
    let user_id = user.map(|u| JsValue::from(u.user_id)).unwrap_or(JsValue::NULL);
    let repo = payload
        .repo
        .map(JsValue::from)
        .unwrap_or(JsValue::NULL);

    db.prepare(
        "INSERT INTO funnel_events (event, user_id, repo, created_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&[
        payload.event.into(),
        user_id,
        repo,
        created_at.into(),
    ])?
    .run()
    .await?;

    Response::from_json(&serde_json::json!({ "ok": true }))
}
