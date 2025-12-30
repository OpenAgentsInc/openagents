//! HUD routes for GTM - personal shareable HUD URLs

use crate::db::users;
use crate::middleware::auth::AuthenticatedUser;
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
}

#[derive(Deserialize)]
struct HudSettingsUpdate {
    repo: String,
    is_public: Option<bool>,
    embed_allowed: Option<bool>,
}

/// View a user's HUD: /hud/:username/:repo
/// For now, simplified to just serve the HUD without strict user lookup
pub async fn view_hud(
    env: Env,
    username: String,
    repo: String,
    maybe_user: Option<AuthenticatedUser>,
) -> Result<Response> {
    // Check if current user is the owner (by matching github username)
    let is_owner = maybe_user
        .as_ref()
        .map(|u| u.github_username.eq_ignore_ascii_case(&username))
        .unwrap_or(false);

    // Return HUD page
    let context = HudContext {
        username: username.clone(),
        repo: repo.clone(),
        is_owner,
        is_public: true, // Default public for now
        embed_mode: false,
    };

    serve_hud_html(&env, &context).await
}

/// Embeddable HUD view: /embed/:username/:repo
pub async fn embed_hud(env: Env, username: String, repo: String) -> Result<Response> {
    let db = env.d1("DB")?;

    // Look up the HUD owner
    let owner = users::get_by_github_username(&db, &username).await?;

    let owner = match owner {
        Some(u) => u,
        None => return Response::error("User not found", 404),
    };

    // Check HUD settings
    let settings = db
        .prepare(
            "SELECT is_public, embed_allowed FROM hud_settings
             WHERE user_id = ? AND repo = ?",
        )
        .bind(&[owner.user_id.clone().into(), repo.clone().into()])?
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

    let context = HudContext {
        username,
        repo,
        is_owner: false,
        is_public: true,
        embed_mode: true,
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
        update.is_public.map(|b| if b { 1i64 } else { 0 }.into()).unwrap_or(JsValue::NULL),
        update.embed_allowed.map(|b| if b { 1i64 } else { 0 }.into()).unwrap_or(JsValue::NULL),
    ])?
    .run()
    .await?;

    Response::from_json(&serde_json::json!({
        "success": true,
        "repo": update.repo
    }))
}

/// Serve the HUD HTML page with context
async fn serve_hud_html(_env: &Env, context: &HudContext) -> Result<Response> {
    // In a full implementation, we'd inject the context into the HTML
    // For now, we serve a simple HTML that fetches context via API

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Autopilot - {}/{}</title>
    <style>
        body {{
            margin: 0;
            padding: 0;
            background: #0a0a0a;
            color: #e0e0e0;
            font-family: 'Vera Mono', monospace;
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
