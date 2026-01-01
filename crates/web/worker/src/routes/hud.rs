//! HUD routes for GTM - personal shareable HUD URLs

use crate::db::users;
use crate::middleware::auth::AuthenticatedUser;
use crate::routes::container::{ContainerSession, StartContainerResponse};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::*;

#[derive(Serialize)]
struct HudContext {
    username: String,
    repo: String,
    is_owner: bool,
    is_public: bool,
    embed_mode: bool,
    agent_id: Option<String>,
    stream_url: Option<String>,
    // Session info for WebSocket connection
    session_id: Option<String>,
    ws_url: Option<String>,
    status: String, // "idle", "starting", "running", "completed", "failed"
}

#[derive(Serialize)]
struct LiveIssue {
    label: String,
    url: String,
    title: Option<String>,
}

#[derive(Serialize)]
struct LiveHudResponse {
    enabled: bool,
    hud_context: Option<HudContext>,
    issue: Option<LiveIssue>,
}

#[derive(Clone, Deserialize, Serialize)]
struct LiveHudRecord {
    repo: Option<String>,
    agent_id: Option<String>,
    stream_url: Option<String>,
    ws_url: Option<String>,
    session_id: Option<String>,
    status: Option<String>,
    issue_url: Option<String>,
    issue_title: Option<String>,
    issue_label: Option<String>,
    started_at: Option<String>,
}

/// Response for session query
#[derive(Serialize)]
pub struct SessionResponse {
    pub status: String,
    pub session_id: Option<String>,
    pub ws_url: Option<String>,
    pub can_start: bool,
}

/// Request to start a HUD session
#[derive(Deserialize)]
pub struct StartSessionRequest {
    pub repo: String,
    pub prompt: String,
    #[serde(default)]
    pub issue_url: Option<String>,
    #[serde(default)]
    pub issue_title: Option<String>,
    #[serde(default)]
    pub issue_label: Option<String>,
}

#[derive(Deserialize)]
struct HudSettingsUpdate {
    repo: String,
    is_public: Option<bool>,
    embed_allowed: Option<bool>,
}

/// View a user's HUD: /repo/:username/:repo
/// For now, simplified to just serve the HUD without strict user lookup
pub async fn view_hud(
    env: Env,
    username: String,
    repo: String,
    maybe_user: Option<AuthenticatedUser>,
    agent_id: Option<String>,
    stream_override: Option<String>,
) -> Result<Response> {
    let db = env.d1("DB")?;
    let owner = users::get_by_github_username(&db, &username).await?;

    // Check if current user is the owner (by matching github username)
    let is_owner = maybe_user
        .as_ref()
        .map(|u| u.github_username.eq_ignore_ascii_case(&username))
        .unwrap_or(false);

    let full_repo = format!("{}/{}", username, repo);

    // Look up settings if owner exists, otherwise use defaults
    let (is_public, owner_user_id) = if let Some(ref owner) = owner {
        let settings = db
            .prepare(
                "SELECT is_public, embed_allowed FROM hud_settings
                 WHERE user_id = ? AND repo = ?",
            )
            .bind(&[owner.user_id.clone().into(), full_repo.clone().into()])?
            .first::<serde_json::Value>(None)
            .await?;

        let is_public = settings
            .as_ref()
            .and_then(|s| s.get("is_public"))
            .and_then(|v| v.as_i64())
            .map(|v| v == 1)
            .unwrap_or(true);

        (is_public, Some(owner.user_id.clone()))
    } else {
        // No registered owner - default to public
        (true, None)
    };

    if !is_owner && !is_public {
        return Response::error("This HUD is private", 403);
    }

    // Check for active session if we have an owner
    let (session_id, ws_url, status) = if let Some(ref user_id) = owner_user_id {
        match get_active_session(&env, user_id, &full_repo).await {
            Ok(Some(session)) => {
                let ws = format!("/api/container/ws/{}", session.session_id);
                (Some(session.session_id), Some(ws), "running".to_string())
            }
            _ => (None, None, "idle".to_string()),
        }
    } else {
        (None, None, "idle".to_string())
    };

    // Return HUD page
    let stream_url = stream_override.or_else(|| {
        agent_id
            .as_ref()
            .map(|id| format!("/agents/{}/hud/stream?watch=1", id))
    });

    let context = HudContext {
        username: username.clone(),
        repo: repo.clone(),
        is_owner,
        is_public,
        embed_mode: false,
        agent_id,
        stream_url,
        session_id,
        ws_url,
        status,
    };

    serve_hud_html(&env, &context).await
}

/// Get active session for a user/repo combination
async fn get_active_session(
    env: &Env,
    user_id: &str,
    repo: &str,
) -> Result<Option<ContainerSession>> {
    let kv = env.kv("SESSIONS")?;

    // Look up session by user+repo key
    let key = format!("hud_session:{}:{}", user_id, repo);
    let result = kv.get(&key).text().await
        .map_err(|e| Error::RustError(format!("KV error: {:?}", e)))?;

    match result {
        Some(session_id) => {
            // Load the actual session
            ContainerSession::load(&kv, &session_id).await
        }
        None => Ok(None),
    }
}

/// Get session status for a repo
/// GET /api/hud/session?repo=owner/repo
pub async fn get_session(
    user: AuthenticatedUser,
    env: Env,
    repo: String,
) -> Result<Response> {
    match get_active_session(&env, &user.user_id, &repo).await? {
        Some(session) => {
            let ws_url = format!("/api/container/ws/{}", session.session_id);
            Response::from_json(&SessionResponse {
                status: "running".to_string(),
                session_id: Some(session.session_id),
                ws_url: Some(ws_url),
                can_start: false,
            })
        }
        None => {
            Response::from_json(&SessionResponse {
                status: "idle".to_string(),
                session_id: None,
                ws_url: None,
                can_start: true,
            })
        }
    }
}

/// Start a new HUD session
/// POST /api/hud/start
pub async fn start_session(
    user: AuthenticatedUser,
    env: Env,
    body: String,
) -> Result<Response> {
    let req: StartSessionRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;

    // Create container session
    let session = ContainerSession::new(&user.user_id, &req.repo);

    // Save to KV with both session key and user+repo lookup key
    let kv = env.kv("SESSIONS")?;
    session.save(&kv).await?;

    // Also save user+repo -> session_id mapping for lookup
    let lookup_key = format!("hud_session:{}:{}", user.user_id, req.repo);
    kv.put(&lookup_key, &session.session_id)?
        .expiration_ttl(86400) // 24 hours
        .execute()
        .await?;

    // Get the Durable Object for this user
    let namespace = env.durable_object("AUTOPILOT_CONTAINER")?;
    let id = namespace.id_from_name(&user.user_id)?;
    let stub = id.get_stub()?;

    // Forward start request to container
    let start_body = serde_json::json!({
        "repo": format!("https://github.com/{}", req.repo),
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

    // Build WebSocket URL (relative for same-origin)
    let ws_url = format!("/api/container/ws/{}", session.session_id);

    if let Some(live_repo) = env_var_non_empty(&env, "LIVE_HUD_REPO") {
        if live_repo.eq_ignore_ascii_case(&req.repo) {
            let live_record = LiveHudRecord {
                repo: Some(req.repo.clone()),
                agent_id: None,
                stream_url: None,
                ws_url: Some(ws_url.clone()),
                session_id: Some(session.session_id.clone()),
                status: Some("live".to_string()),
                issue_url: req.issue_url.clone(),
                issue_title: req.issue_title.clone(),
                issue_label: req.issue_label.clone(),
                started_at: Some(chrono::Utc::now().to_rfc3339()),
            };
            if let Ok(payload) = serde_json::to_string(&live_record) {
                if let Ok(put) = kv.put("live_hud:active", &payload) {
                    let _ = put.expiration_ttl(21600).execute().await;
                }
            }
        }
    }

    Response::from_json(&StartContainerResponse {
        session_id: session.session_id,
        status: "starting".to_string(),
        ws_url,
    })
}

/// Embeddable HUD view: /repo/:username/:repo/embed
pub async fn embed_hud(
    env: Env,
    username: String,
    repo: String,
    agent_id: Option<String>,
    stream_override: Option<String>,
) -> Result<Response> {
    let db = env.d1("DB")?;

    // Look up the HUD owner
    let owner = users::get_by_github_username(&db, &username).await?;

    let owner = match owner {
        Some(u) => u,
        None => return Response::error("User not found", 404),
    };

    let full_repo = format!("{}/{}", username, repo);

    // Check HUD settings
    let settings = db
        .prepare(
            "SELECT is_public, embed_allowed FROM hud_settings
             WHERE user_id = ? AND repo = ?",
        )
        .bind(&[owner.user_id.clone().into(), full_repo.clone().into()])?
        .first::<serde_json::Value>(None)
        .await?;

    let is_public = settings
        .as_ref()
        .and_then(|s| s.get("is_public"))
        .and_then(|v| v.as_i64())
        .map(|v| v == 1)
        .unwrap_or(true);

    let embed_allowed = settings
        .as_ref()
        .and_then(|s| s.get("embed_allowed"))
        .and_then(|v| v.as_i64())
        .map(|v| v == 1)
        .unwrap_or(true);

    if !is_public {
        return Response::error("This HUD is private", 403);
    }

    if !embed_allowed {
        return Response::error("Embedding is disabled for this HUD", 403);
    }

    let (session_id, ws_url, status) = match get_active_session(&env, &owner.user_id, &full_repo).await {
        Ok(Some(session)) => {
            let ws = format!("/api/container/ws/{}", session.session_id);
            (Some(session.session_id), Some(ws), "running".to_string())
        }
        _ => (None, None, "idle".to_string()),
    };

    let stream_url = stream_override.or_else(|| {
        agent_id
            .as_ref()
            .map(|id| format!("/agents/{}/hud/stream?watch=1", id))
    });

    let context = HudContext {
        username,
        repo,
        is_owner: false,
        is_public,
        embed_mode: true,
        agent_id,
        stream_url,
        session_id,
        ws_url,
        status,
    };

    serve_hud_html(&env, &context).await
}

/// Update HUD settings
pub async fn update_settings(
    user: AuthenticatedUser,
    env: Env,
    body: String,
) -> Result<Response> {
    let update: HudSettingsUpdate = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;

    let db = env.d1("DB")?;
    let now = chrono::Utc::now().to_rfc3339();

    // Upsert settings
    db.prepare(
        "INSERT INTO hud_settings (user_id, repo, is_public, embed_allowed, created_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, repo) DO UPDATE SET
            is_public = COALESCE(?, is_public),
            embed_allowed = COALESCE(?, embed_allowed)",
    )
    .bind(&[
        user.user_id.into(),
        update.repo.clone().into(),
        update.is_public.map(|b| if b { 1i64 } else { 0 }).unwrap_or(1).into(),
        update.embed_allowed.map(|b| if b { 1i64 } else { 0 }).unwrap_or(1).into(),
        now.into(),
        update
            .is_public
            .map(|b| JsValue::from(if b { 1i64 } else { 0 }))
            .unwrap_or(JsValue::NULL),
        update
            .embed_allowed
            .map(|b| JsValue::from(if b { 1i64 } else { 0 }))
            .unwrap_or(JsValue::NULL),
    ])?
    .run()
    .await?;

    Response::from_json(&serde_json::json!({
        "success": true,
        "repo": update.repo
    }))
}

/// Get HUD settings for a repo (owner only).
pub async fn get_settings(
    user: AuthenticatedUser,
    env: Env,
    repo: String,
) -> Result<Response> {
    let db = env.d1("DB")?;
    let settings = db
        .prepare(
            "SELECT is_public, embed_allowed FROM hud_settings
             WHERE user_id = ? AND repo = ?",
        )
        .bind(&[user.user_id.into(), repo.clone().into()])?
        .first::<serde_json::Value>(None)
        .await?;

    let is_public = settings
        .as_ref()
        .and_then(|s| s.get("is_public"))
        .and_then(|v| v.as_i64())
        .map(|v| v == 1)
        .unwrap_or(true);
    let embed_allowed = settings
        .as_ref()
        .and_then(|s| s.get("embed_allowed"))
        .and_then(|v| v.as_i64())
        .map(|v| v == 1)
        .unwrap_or(true);

    Response::from_json(&serde_json::json!({
        "repo": repo,
        "is_public": is_public,
        "embed_allowed": embed_allowed
    }))
}

/// Live HUD configuration for landing page.
pub async fn live_hud(env: Env) -> Result<Response> {
    let live_record = load_live_hud_record(&env).await?;
    let repo = live_record
        .as_ref()
        .and_then(|record| record.repo.clone())
        .or_else(|| env_var_non_empty(&env, "LIVE_HUD_REPO"));
    let repo = match repo {
        Some(value) => value,
        None => {
            return Response::from_json(&LiveHudResponse {
                enabled: false,
                hud_context: None,
                issue: None,
            })
        }
    };

    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() < 2 {
        return Response::from_json(&LiveHudResponse {
            enabled: false,
            hud_context: None,
            issue: None,
        });
    }

    let username = parts[0].to_string();
    let repo_name = parts[1..].join("/");
    let agent_id = live_record
        .as_ref()
        .and_then(|record| record.agent_id.clone())
        .or_else(|| env_var_non_empty(&env, "LIVE_HUD_AGENT_ID"));
    let stream_url = live_record
        .as_ref()
        .and_then(|record| record.stream_url.clone())
        .or_else(|| env_var_non_empty(&env, "LIVE_HUD_STREAM_URL"));
    let ws_url = live_record
        .as_ref()
        .and_then(|record| record.ws_url.clone());
    let session_id = live_record
        .as_ref()
        .and_then(|record| record.session_id.clone());
    if agent_id.is_none() && stream_url.is_none() && ws_url.is_none() {
        return Response::from_json(&LiveHudResponse {
            enabled: false,
            hud_context: None,
            issue: None,
        });
    }

    let status = live_record
        .as_ref()
        .and_then(|record| record.status.clone())
        .or_else(|| env_var_non_empty(&env, "LIVE_HUD_STATUS"))
        .unwrap_or_else(|| "live".to_string());

    let issue_url = live_record
        .as_ref()
        .and_then(|record| record.issue_url.clone())
        .or_else(|| env_var_non_empty(&env, "LIVE_HUD_ISSUE_URL"));
    let issue_title = live_record
        .as_ref()
        .and_then(|record| record.issue_title.clone())
        .or_else(|| env_var_non_empty(&env, "LIVE_HUD_ISSUE_TITLE"));
    let issue_label = live_record
        .as_ref()
        .and_then(|record| record.issue_label.clone())
        .or_else(|| env_var_non_empty(&env, "LIVE_HUD_ISSUE_LABEL"))
        .or_else(|| {
            issue_url.as_ref().and_then(|url| {
            let trimmed = url.trim_end_matches('/');
            let last = trimmed.rsplit('/').next()?;
            if last.chars().all(|c| c.is_ascii_digit()) {
                Some(format!("issue #{}", last))
            } else {
                None
            }
        })
    });

    let issue = match (issue_label, issue_url) {
        (Some(label), Some(url)) => Some(LiveIssue {
            label,
            url,
            title: issue_title,
        }),
        _ => None,
    };

    let context = HudContext {
        username,
        repo: repo_name,
        is_owner: false,
        is_public: true,
        embed_mode: false,
        agent_id,
        stream_url,
        session_id,
        ws_url,
        status,
    };

    Response::from_json(&LiveHudResponse {
        enabled: true,
        hud_context: Some(context),
        issue,
    })
}

fn env_var_non_empty(env: &Env, key: &str) -> Option<String> {
    env.var(key)
        .ok()
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
}

async fn load_live_hud_record(env: &Env) -> Result<Option<LiveHudRecord>> {
    let kv = env.kv("SESSIONS")?;
    let result = kv
        .get("live_hud:active")
        .text()
        .await
        .map_err(|e| Error::RustError(format!("KV error: {:?}", e)))?;
    let Some(json) = result else {
        return Ok(None);
    };
    Ok(serde_json::from_str(&json).ok())
}

/// Serve the HUD HTML page with context
async fn serve_hud_html(_env: &Env, context: &HudContext) -> Result<Response> {
    // Inject the HUD context for the WASM client.

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Autopilot - {}/{}</title>
    <link rel="stylesheet" href="/static/MyWebfontsKit.css">
    <style>
        body {{
            margin: 0;
            padding: 0;
            background: #0a0a0a;
            color: #e0e0e0;
            font-family: 'Square721StdRoman', sans-serif;
        }}
        #hud-container {{
            width: 100vw;
            height: 100vh;
        }}
        canvas {{
            width: 100%;
            height: 100%;
        }}
    </style>
</head>
<body>
    <div id="hud-container">
        <canvas id="canvas"></canvas>
    </div>
    <script type="module">
        window.HUD_CONTEXT = {context_json};

        import init, {{ start_demo }} from '/pkg/openagents_web_client.js';

        async function run() {{
            await init();
            await start_demo('canvas');
        }}

        run().catch(console.error);
    </script>
</body>
</html>"#,
        context.username,
        context.repo,
        context_json = serde_json::to_string(context).unwrap_or_default()
    );

    let mut headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("Cross-Origin-Opener-Policy", "same-origin")?;
    headers.set("Cross-Origin-Embedder-Policy", "require-corp")?;

    // Allow embedding if in embed mode
    if context.embed_mode {
        headers.set("X-Frame-Options", "ALLOWALL")?;
    } else {
        headers.set("X-Frame-Options", "SAMEORIGIN")?;
    }

    Ok(Response::ok(html)?.with_headers(headers))
}
