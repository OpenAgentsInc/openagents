//! OpenAgents API — Cloudflare Worker (workers-rs).
//!
//! Run locally: `npx wrangler dev`
//! Deploy: `npx wrangler deploy`

use include_dir::{include_dir, Dir};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::path::Path;
use url::form_urlencoded;
use wasm_bindgen::JsValue;
use worker::*;

const MOLTBOOK_SITE_DEFAULT: &str = "https://www.moltbook.com";
const MOLTBOOK_API_DEFAULT: &str = "https://www.moltbook.com/api/v1";
const INDEX_LIMIT_DEFAULT: usize = 100;
const INDEX_LIMIT_MAX: usize = 500;
const WATCH_SEEN_CAP: usize = 2000;
const CONVEX_CONTROL_HEADER: &str = "x-oa-control-key";

static MOLTBOOK_DOCS: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../crates/moltbook/docs");

static DOC_INDEX: Lazy<Vec<DocEntry>> = Lazy::new(build_doc_index);
static DOC_CATEGORIES: Lazy<Vec<CategorySummary>> = Lazy::new(|| summarize_categories(&DOC_INDEX));

#[derive(Debug, Deserialize, Serialize)]
struct ApiResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthData {
    status: &'static str,
    service: &'static str,
}

#[derive(Debug, Serialize, Clone)]
struct DocEntry {
    path: String,
    category: String,
    title: Option<String>,
    summary: Option<String>,
    bytes: usize,
    extension: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct CategorySummary {
    category: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct IndexPayload {
    entries: Vec<DocEntry>,
    total: usize,
    matched: usize,
    categories: Vec<CategorySummary>,
}

#[derive(Debug, Serialize)]
struct DocPayload {
    path: String,
    content: String,
    content_type: String,
    bytes: usize,
    entry: Option<DocEntry>,
}

#[derive(Debug, Serialize)]
struct WatchPayload {
    source: String,
    sort: String,
    limit: u32,
    submolt: Option<String>,
    total: usize,
    new_posts: Vec<serde_json::Value>,
    seen: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterRequestBody {
    name: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateProfileBody {
    description: Option<String>,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CreatePostBody {
    submolt: String,
    title: String,
    content: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCommentBody {
    content: String,
    parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateSubmoltBody {
    name: String,
    display_name: String,
    description: String,
}


#[derive(Debug, Deserialize)]
struct ModeratorBody {
    agent_name: String,
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubmoltSettingsBody {
    description: Option<String>,
    banner_color: Option<String>,
    theme_color: Option<String>,
}

/// Strip /api path prefix so the worker works at openagents.com/api/*.
fn strip_api_prefix(req: &Request) -> Result<Option<url::Url>> {
    let mut url = req.url()?.clone();
    let path = url.path().to_string();
    let new_path = if path == "/api" || path == "/api/" {
        "/".to_string()
    } else if path.starts_with("/api/") {
        path[4..].to_string()
    } else {
        return Ok(None);
    };
    url.set_path(&new_path);
    Ok(Some(url))
}

#[event(fetch)]
async fn main(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let req = if let Some(url) = strip_api_prefix(&req)? {
        let original_url = req.url()?.to_string();
        let mut init = RequestInit::new();
        init.with_method(req.method().clone());
        let headers = Headers::new();
        for (name, value) in req.headers().entries() {
            let _ = headers.append(&name, &value);
        }
        let _ = headers.set("x-oa-original-url", &original_url);
        init.with_headers(headers);
        if req.method() != Method::Get && req.method() != Method::Head {
            let bytes = req.bytes().await?;
            if !bytes.is_empty() {
                init.with_body(Some(js_sys::Uint8Array::from(bytes.as_slice()).into()));
            }
        }
        Request::new_with_init(url.as_str(), &init)?
    } else {
        req
    };

    Router::new()
        .get_async("/", handle_root)
        .get_async("/health", handle_health)
        .get_async("/openclaw/invoice", handle_openclaw_invoice_get)
        .post_async("/openclaw/invoice", handle_openclaw_invoice_post)
        .post_async("/register", handle_control_register)
        .get_async("/projects", handle_control_projects)
        .post_async("/projects", handle_control_projects)
        .get_async("/organizations", handle_control_organizations)
        .post_async("/organizations", handle_control_organizations)
        .get_async("/issues", handle_control_issues)
        .post_async("/issues", handle_control_issues)
        .patch_async("/issues", handle_control_issues)
        .delete_async("/issues", handle_control_issues)
        .get_async("/repos", handle_control_repos)
        .post_async("/repos", handle_control_repos)
        .delete_async("/repos", handle_control_repos)
        .get_async("/tokens", handle_control_tokens)
        .post_async("/tokens", handle_control_tokens)
        .delete_async("/tokens", handle_control_tokens)
        .get_async("/nostr", handle_control_nostr)
        .post_async("/nostr/verify", handle_control_nostr_verify)
        .get_async("/agents/wallet-onboarding", handle_agents_wallet_onboarding)
        .post_async("/agents", handle_agents_create)
        .get_async("/agents/:id", handle_agents_get)
        .post_async("/agents/:id/wallet", handle_agents_wallet_register)
        .get_async("/agents/:id/wallet", handle_agents_wallet_get)
        .get_async("/agents/:id/balance", handle_agents_balance)
        .post_async("/payments/invoice", handle_payments_invoice)
        .post_async("/payments/pay", handle_payments_pay)
        .get_async("/social/v1", handle_social_root)
        .get_async("/social/v1/", handle_social_root)
        .on_async("/social/v1/*path", handle_social_router)
        .get_async("/posts", handle_social_api_router)
        .get_async("/feed", handle_social_api_router)
        .get_async("/search", handle_social_api_router)
        .post_async("/agents/me/identity-token", handle_social_api_router)
        .get_async("/agents/me", handle_social_api_router)
        .post_async("/agents/register", handle_social_api_router)
        .post_async("/agents/verify-identity", handle_social_api_router)
        .get_async("/agents/status", handle_social_api_router)
        .get_async("/agents/profile", handle_social_api_router)
        .post_async("/agents/:name/follow", handle_social_api_router)
        .delete_async("/agents/:name/follow", handle_social_api_router)
        .post_async("/agents/me/avatar", handle_social_api_router)
        .delete_async("/agents/me/avatar", handle_social_api_router)
        .patch_async("/agents/me", handle_social_api_router)
        .get_async("/agents/me/wallet", handle_social_api_router)
        .post_async("/agents/me/wallet", handle_social_api_router)
        .get_async("/agents/me/balance", handle_social_api_router)
        .post_async("/posts", handle_social_api_router)
        .get_async("/posts/:id", handle_social_api_router)
        .delete_async("/posts/:id", handle_social_api_router)
        .post_async("/posts/:id/upvote", handle_social_api_router)
        .post_async("/posts/:id/downvote", handle_social_api_router)
        .post_async("/posts/:id/comments", handle_social_api_router)
        .get_async("/posts/:id/comments", handle_social_api_router)
        .post_async("/posts/:id/pin", handle_social_api_router)
        .delete_async("/posts/:id/pin", handle_social_api_router)
        .post_async("/comments/:id/upvote", handle_social_api_router)
        .get_async("/submolts", handle_social_api_router)
        .post_async("/submolts", handle_social_api_router)
        .get_async("/submolts/:name", handle_social_api_router)
        .get_async("/submolts/:name/feed", handle_social_api_router)
        .post_async("/submolts/:name/subscribe", handle_social_api_router)
        .delete_async("/submolts/:name/subscribe", handle_social_api_router)
        .patch_async("/submolts/:name/settings", handle_social_api_router)
        .post_async("/submolts/:name/settings", handle_social_api_router)
        .get_async("/submolts/:name/moderators", handle_social_api_router)
        .post_async("/submolts/:name/moderators", handle_social_api_router)
        .delete_async("/submolts/:name/moderators", handle_social_api_router)
        .get_async("/media/*key", handle_social_api_router)
        .get_async("/claim/:token", handle_social_claim_get)
        .post_async("/claim/:token", handle_social_claim_post)
        .get_async("/moltbook", handle_moltbook_root)
        .get_async("/moltbook/", handle_moltbook_root)
        .on_async("/moltbook/*path", handle_moltbook_router)
        .on_async("/*path", handle_site_fallback_proxy)
        .run(req, env)
        .await
}

async fn handle_root(req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    if wants_html(&req) {
        let mut url = req.url()?;
        url.set_path("/moltbook/");
        url.set_query(None);
        url.set_fragment(None);
        return Response::redirect(url);
    }
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "name": "openagents-api",
            "docs": "/moltbook",
            "social_api": "/",
            "moltbook_proxy": "/moltbook/site/",
            "moltbook_api": "/moltbook/api/",
            "moltbook_index": "/moltbook/index",
            "moltbook_indexer": "/api/indexer",
            "agents_wallet_onboarding": "/api/agents/wallet-onboarding"
        })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_health(_: Request, _: RouteContext<()>) -> Result<Response> {
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(HealthData {
            status: "ok",
            service: "openagents-api",
        }),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_control_register(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if req.method() != Method::Post {
        return json_error("method not allowed", 405);
    }

    let body: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let user_id = body
        .get("user_id")
        .or_else(|| body.get("userId"))
        .or_else(|| body.get("subject"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if user_id.is_empty() {
        return json_error("user_id required", 400);
    }

    let body_text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string());
    let response = forward_convex_control(
        &ctx.env,
        Method::Post,
        "control/register",
        Some(body_text),
        None,
        None,
    )
    .await?;
    Ok(response)
}

fn control_api_key_from_request(req: &Request) -> Option<String> {
    social_api_key_from_request(req)
        .or_else(|| {
            req.url()
                .ok()
                .and_then(|url| url.query_pairs().find_map(|(k, v)| {
                    if k == "api_key" || k == "moltbook_api_key" {
                        Some(v.to_string())
                    } else {
                        None
                    }
                }))
        })
        .filter(|k| !k.trim().is_empty())
}

fn nostr_auth_from_request(req: &Request) -> Option<String> {
    if let Ok(Some(token)) = req.headers().get("x-nostr-auth") {
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    if let Ok(Some(token)) = req.headers().get("x-oa-nostr-auth") {
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    if let Ok(Some(auth)) = req.headers().get("authorization") {
        let trimmed = auth.trim();
        if trimmed.to_lowercase().starts_with("nostr ") {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn control_query_string(url: &url::Url) -> String {
    let mut query_pairs: Vec<(String, String)> = Vec::new();
    for (k, v) in url.query_pairs() {
        if k == "api_key" || k == "moltbook_api_key" {
            continue;
        }
        query_pairs.push((k.to_string(), v.to_string()));
    }
    build_query_string(&query_pairs)
}

fn normalize_json_body(value: serde_json::Value) -> serde_json::Value {
    if value.is_null() {
        serde_json::json!({})
    } else {
        value
    }
}

async fn forward_control_get(
    req: Request,
    ctx: RouteContext<()>,
    path: &str,
) -> Result<Response> {
    let Some(api_key) = control_api_key_from_request(&req) else {
        return Ok(json_unauthorized("missing api key"));
    };

    let url = req.url()?;
    let query = control_query_string(&url);
    let path = if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{query}")
    };

    forward_convex_control(&ctx.env, Method::Get, &path, None, Some(api_key), None).await
}

async fn forward_control_with_body(
    mut req: Request,
    ctx: RouteContext<()>,
    method: Method,
    path: &str,
) -> Result<Response> {
    let Some(api_key) = control_api_key_from_request(&req) else {
        return Ok(json_unauthorized("missing api key"));
    };

    let body_value: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let body_value = normalize_json_body(body_value);
    let body_text = serde_json::to_string(&body_value).unwrap_or_else(|_| "{}".to_string());

    forward_convex_control(&ctx.env, method, path, Some(body_text), Some(api_key), None).await
}

async fn handle_control_projects(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let method = req.method().clone();
    if method != Method::Get && method != Method::Post {
        return json_error("method not allowed", 405);
    }

    if method == Method::Get {
        return forward_control_get(req, ctx, "control/projects").await;
    }

    forward_control_with_body(req, ctx, Method::Post, "control/projects").await
}

async fn handle_control_organizations(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let method = req.method().clone();
    if method == Method::Get {
        return forward_control_get(req, ctx, "control/organizations").await;
    }
    if method == Method::Post {
        return forward_control_with_body(req, ctx, Method::Post, "control/organizations").await;
    }
    json_error("method not allowed", 405)
}

async fn handle_control_issues(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let method = req.method().clone();
    match method {
        Method::Get => forward_control_get(req, ctx, "control/issues").await,
        Method::Post => forward_control_with_body(req, ctx, Method::Post, "control/issues").await,
        Method::Patch => forward_control_with_body(req, ctx, Method::Patch, "control/issues").await,
        Method::Delete => {
            forward_control_with_body(req, ctx, Method::Delete, "control/issues").await
        }
        _ => json_error("method not allowed", 405),
    }
}

async fn handle_control_repos(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let method = req.method().clone();
    match method {
        Method::Get => forward_control_get(req, ctx, "control/repos").await,
        Method::Post => forward_control_with_body(req, ctx, Method::Post, "control/repos").await,
        Method::Delete => {
            forward_control_with_body(req, ctx, Method::Delete, "control/repos").await
        }
        _ => json_error("method not allowed", 405),
    }
}

async fn handle_control_tokens(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let method = req.method().clone();
    match method {
        Method::Get => forward_control_get(req, ctx, "control/tokens").await,
        Method::Post => forward_control_with_body(req, ctx, Method::Post, "control/tokens").await,
        Method::Delete => {
            forward_control_with_body(req, ctx, Method::Delete, "control/tokens").await
        }
        _ => json_error("method not allowed", 405),
    }
}

async fn handle_control_nostr(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if req.method() != Method::Get {
        return json_error("method not allowed", 405);
    }
    forward_control_get(req, ctx, "control/nostr").await
}

async fn handle_control_nostr_verify(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if req.method() != Method::Post {
        return json_error("method not allowed", 405);
    }

    let Some(api_key) = control_api_key_from_request(&req) else {
        return Ok(json_unauthorized("missing api key"));
    };

    let Some(nostr_auth) = nostr_auth_from_request(&req) else {
        return Ok(json_unauthorized("missing nostr auth"));
    };

    let body_value: serde_json::Value = req.json().await.unwrap_or(serde_json::Value::Null);
    let body_value = normalize_json_body(body_value);
    let body_text = serde_json::to_string(&body_value).unwrap_or_else(|_| "{}".to_string());

    let mut extra_headers = Vec::new();
    extra_headers.push(("x-oa-nostr-auth".to_string(), nostr_auth));
    if let Ok(Some(original_url)) = req.headers().get("x-oa-original-url") {
        let trimmed = original_url.trim();
        if !trimmed.is_empty() {
            extra_headers.push(("x-oa-original-url".to_string(), trimmed.to_string()));
        }
    }

    forward_convex_control(
        &ctx.env,
        Method::Post,
        "control/nostr/verify",
        Some(body_text),
        Some(api_key),
        Some(extra_headers),
    )
    .await
}

// --- Social API (Moltbook parity, OpenAgents storage) ---

#[derive(Debug, Deserialize)]
struct SocialFeedQuery {
    sort: Option<String>,
    limit: Option<u32>,
    submolt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SocialCommentsQuery {
    sort: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SocialSearchQuery {
    q: Option<String>,
    #[serde(rename = "type")]
    r#type: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SocialProfileQuery {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SocialPostRow {
    id: String,
    created_at: Option<String>,
    submolt: Option<String>,
    title: Option<String>,
    content: Option<String>,
    url: Option<String>,
    author_name: Option<String>,
    author_id: Option<String>,
    score: Option<i64>,
    comment_count: Option<i64>,
    is_pinned: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SocialCommentRow {
    id: String,
    post_id: String,
    parent_id: Option<String>,
    created_at: Option<String>,
    author_name: Option<String>,
    author_id: Option<String>,
    content: Option<String>,
    score: Option<i64>,
}


fn social_limit(limit: Option<u32>, default_limit: u32, max_limit: u32) -> u32 {
    limit.unwrap_or(default_limit).min(max_limit).max(1)
}

fn social_sort(sort: Option<String>) -> String {
    sort.unwrap_or_else(|| "new".to_string())
}

fn parse_iso_seconds(value: &Option<String>) -> f64 {
    value
        .as_ref()
        .map(|s| js_sys::Date::new(&JsValue::from_str(s)).get_time() / 1000.0)
        .unwrap_or(0.0)
}

fn hot_score(score: i64, created_at: &Option<String>) -> f64 {
    let s = score.max(0) as f64;
    let order = (s.max(1.0)).log10();
    let seconds = parse_iso_seconds(created_at);
    order + seconds / 45_000.0
}

fn rising_score(score: i64, created_at: &Option<String>) -> f64 {
    let seconds = parse_iso_seconds(created_at);
    let age_hours = ((js_sys::Date::now() / 1000.0) - seconds) / 3600.0;
    let denom = (age_hours + 2.0).powf(1.5);
    (score as f64) / denom
}

fn post_row_to_value(row: SocialPostRow) -> serde_json::Value {
    let author = row.author_name.map(|name| {
        serde_json::json!({
            "name": name,
            "id": row.author_id
        })
    });
    serde_json::json!({
        "id": row.id,
        "submolt": row.submolt,
        "title": row.title,
        "content": row.content,
        "url": row.url,
        "author": author,
        "score": row.score,
        "commentCount": row.comment_count,
        "createdAt": row.created_at,
        "isPinned": row.is_pinned.map(|v| v != 0).unwrap_or(false)
    })
}

fn comment_row_to_value(row: SocialCommentRow) -> serde_json::Value {
    let author = row.author_name.map(|name| {
        serde_json::json!({
            "name": name,
            "id": row.author_id
        })
    });
    serde_json::json!({
        "id": row.id,
        "post_id": row.post_id,
        "parent_id": row.parent_id,
        "content": row.content,
        "author": author,
        "score": row.score,
        "created_at": row.created_at
    })
}

fn now_iso() -> String {
    js_sys::Date::new_0().to_string().into()
}

fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

fn iso_from_ms(ms: i64) -> String {
    js_sys::Date::new(&JsValue::from_f64(ms as f64))
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| now_iso())
}

fn parse_epoch_ms(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(raw) = trimmed.parse::<i64>() {
        if raw <= 0 {
            return None;
        }
        let ms = if raw < 1_000_000_000_000 { raw.saturating_mul(1000) } else { raw };
        return Some(ms);
    }
    let ms = js_sys::Date::new(&JsValue::from_str(trimmed)).get_time();
    if ms.is_finite() {
        return Some(ms as i64);
    }
    None
}

fn random_token(prefix: &str, len: usize) -> String {
    let mut out = String::with_capacity(prefix.len() + len);
    out.push_str(prefix);
    while out.len() < prefix.len() + len {
        let v = (js_sys::Math::random() * 36.0).floor() as u32;
        let ch = std::char::from_digit(v, 36).unwrap_or('a');
        out.push(ch);
    }
    out
}

fn social_api_key_from_request(req: &Request) -> Option<String> {
    if let Ok(Some(auth)) = req.headers().get("authorization") {
        let trimmed = auth.trim();
        if let Some(rest) = trimmed.strip_prefix("Bearer ") {
            if !rest.trim().is_empty() {
                return Some(rest.trim().to_string());
            }
        }
    }
    for header in ["x-api-key", "x-moltbook-api-key", "x-oa-moltbook-api-key"] {
        if let Ok(Some(value)) = req.headers().get(header) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    if let Ok(url) = req.url() {
        for (key, value) in url.query_pairs() {
            if matches!(
                key.as_ref(),
                "api_key" | "moltbook_api_key" | "oa_moltbook_api_key"
            ) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn openclaw_invoice_token(env: &Env) -> Result<String> {
    if let Ok(var) = env.var("OPENCLAW_INVOICE_TOKEN") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }
    Ok("".to_string())
}

fn openclaw_token_from_request(req: &Request) -> Option<String> {
    if let Ok(Some(auth)) = req.headers().get("authorization") {
        let trimmed = auth.trim();
        if let Some(rest) = trimmed.strip_prefix("Bearer ") {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn convex_site_base(env: &Env) -> Result<String> {
    if let Ok(var) = env.var("CONVEX_SITE_URL") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }
    Ok("".to_string())
}

fn convex_control_key(env: &Env) -> Result<String> {
    if let Ok(var) = env.var("CONVEX_CONTROL_KEY") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }
    Ok("".to_string())
}

async fn forward_convex_control(
    env: &Env,
    method: Method,
    path: &str,
    body: Option<String>,
    api_key: Option<String>,
    extra_headers: Option<Vec<(String, String)>>,
) -> Result<Response> {
    let base = convex_site_base(env)?;
    if base.trim().is_empty() {
        return json_error("CONVEX_SITE_URL not configured", 500);
    }
    let control_key = convex_control_key(env)?;
    if control_key.trim().is_empty() {
        return json_error("CONVEX_CONTROL_KEY not configured", 500);
    }

    let url = join_url(&base, path, "");
    let mut init = RequestInit::new();
    init.with_method(method);
    let headers = Headers::new();
    headers.set("content-type", "application/json")?;
    headers.set(CONVEX_CONTROL_HEADER, &control_key)?;
    if let Some(key) = api_key {
        headers.set("authorization", &format!("Bearer {key}"))?;
    }
    if let Some(extra) = extra_headers {
        for (name, value) in extra {
            let _ = headers.set(&name, &value);
        }
    }
    init.with_headers(headers);
    if let Some(body) = body {
        if !body.is_empty() {
            init.with_body(Some(JsValue::from_str(&body)));
        }
    }

    let outbound = match Request::new_with_init(&url, &init) {
        Ok(req) => req,
        Err(err) => {
            return json_error(&format!("convex request error: {err}"), 500);
        }
    };
    let mut response = match Fetch::Request(outbound).send().await {
        Ok(resp) => resp,
        Err(err) => {
            return json_error(&format!("convex fetch error: {err}"), 502);
        }
    };

    let status = response.status_code();
    let content_type = response.headers().get("content-type").ok().flatten();
    let bytes = response.bytes().await.unwrap_or_default();
    let mut out = Response::from_bytes(bytes)?.with_status(status);
    if let Some(ct) = content_type {
        let _ = out.headers_mut().set("content-type", &ct);
    }
    apply_cors(&mut out)?;
    Ok(out)
}

/// Returns (api_key, agent_name) or a 401 Response. Callers should return Ok(err) on Err.
async fn social_auth(req: &Request, ctx: &RouteContext<()>) -> std::result::Result<(String, String), Response> {
    let api_key = match social_api_key_from_request(req) {
        Some(k) => k,
        None => return Err(json_unauthorized("missing api key")),
    };
    let db = ctx.d1("SOCIAL_DB").map_err(|_| json_unauthorized("invalid api key"))?;
    let stmt = db
        .prepare("SELECT agent_name, status FROM social_api_keys WHERE api_key = ?1")
        .bind(&[JsValue::from_str(&api_key)])
        .map_err(|_| json_unauthorized("invalid api key"))?;
    let row: Option<serde_json::Value> = stmt
        .first(None)
        .await
        .map_err(|_| json_unauthorized("invalid api key"))?;
    let agent_name = row
        .and_then(|r| r.get("agent_name").cloned())
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| json_unauthorized("invalid api key"))?;
    Ok((api_key, agent_name))
}

async fn social_rate_limit(
    ctx: &RouteContext<()>,
    api_key: &str,
    action: &str,
    window_seconds: i64,
    max_count: i64,
) -> Result<Option<i64>> {
    let now = now_iso();
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT window_start, count FROM social_rate_limits WHERE api_key = ?1 AND action = ?2")
        .bind(&[JsValue::from_str(api_key), JsValue::from_str(action)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    if let Some(row) = row {
        let window_start = row.get("window_start").and_then(|v| v.as_str()).unwrap_or("");
        let count = row.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        let window_start_ms = js_sys::Date::new(&JsValue::from_str(window_start)).get_time();
        let now_ms = js_sys::Date::new(&JsValue::from_str(&now)).get_time();
        if now_ms - window_start_ms < (window_seconds as f64 * 1000.0) {
            if count >= max_count {
                let retry = ((window_seconds as f64 * 1000.0 - (now_ms - window_start_ms)) / 60000.0)
                    .ceil() as i64;
                return Ok(Some(retry.max(1)));
            }
            let _ = db
                .prepare("UPDATE social_rate_limits SET count = ?3 WHERE api_key = ?1 AND action = ?2")
                .bind(&[
                    JsValue::from_str(api_key),
                    JsValue::from_str(action),
                    JsValue::from_f64((count + 1) as f64),
                ])?
                .run()
                .await?;
            return Ok(None);
        }
    }
    let _ = db
        .prepare("INSERT OR REPLACE INTO social_rate_limits (api_key, action, window_start, count) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[
            JsValue::from_str(api_key),
            JsValue::from_str(action),
            JsValue::from_str(&now),
            JsValue::from_f64(1.0),
        ])?
        .run()
        .await?;
    Ok(None)
}

async fn handle_social_root(_: Request, _: RouteContext<()>) -> Result<Response> {
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "base": "/",
            "docs": "/docs/social-api",
            "endpoints": {
                "agents": [
                    "GET /agents/profile?name=",
                    "GET /agents/me",
                    "PATCH /agents/me",
                    "POST /agents/register",
                    "GET /agents/me/wallet",
                    "POST /agents/me/wallet",
                    "GET /agents/me/balance",
                    "POST /agents/me/avatar",
                    "DELETE /agents/me/avatar"
                ],
                "posts": [
                    "GET /posts?sort=&limit=&submolt=",
                    "GET /posts/{id}",
                    "GET /posts/{id}/comments",
                    "POST /posts",
                    "DELETE /posts/{id}",
                    "POST /posts/{id}/upvote",
                    "POST /posts/{id}/downvote",
                    "POST /posts/{id}/pin",
                    "DELETE /posts/{id}/pin"
                ],
                "submolts": [
                    "GET /submolts",
                    "GET /submolts/{name}",
                    "GET /submolts/{name}/feed",
                    "PATCH /submolts/{name}/settings",
                    "POST /submolts/{name}/settings",
                    "POST /submolts/{name}/subscribe",
                    "DELETE /submolts/{name}/subscribe",
                    "GET /submolts/{name}/moderators",
                    "POST /submolts/{name}/moderators",
                    "DELETE /submolts/{name}/moderators"
                ],
                "search": [
                    "GET /search?q=&type=&limit="
                ],
                "media": [
                    "GET /media/{key}"
                ]
            }
        })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

fn social_segments_from_path(path: &str) -> Vec<&str> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

async fn handle_social_router(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let path = ctx.param("path").map(|v| v.to_string()).unwrap_or_default();
    let segments = social_segments_from_path(&path);
    if segments.is_empty() {
        return handle_social_root(req, ctx).await;
    }
    handle_social_dispatch(req, ctx, segments).await
}

async fn handle_social_api_router(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // General rate limit: 100 req/min per API key (Moltbook parity). Only when key present.
    if let Some(api_key) = social_api_key_from_request(&req) {
        if let Ok(Some(retry)) = social_rate_limit(&ctx, &api_key, "request", 60, 100).await {
            return rate_limit_429(retry);
        }
    }
    let path = req.url()?.path().to_string();
    let segments = social_segments_from_path(&path);
    if segments.is_empty() {
        return handle_social_root(req, ctx).await;
    }
    handle_social_dispatch(req, ctx, segments).await
}

async fn handle_social_dispatch(
    req: Request,
    ctx: RouteContext<()>,
    segments: Vec<&str>,
) -> Result<Response> {
    match segments.as_slice() {
        ["agents", "register"] if req.method() == Method::Post => {
            handle_social_agents_register(req, ctx).await
        }
        ["agents", "me", "identity-token"] if req.method() == Method::Post => {
            handle_social_identity_token(req, ctx).await
        }
        ["agents", "me", "wallet"] if req.method() == Method::Get => {
            handle_social_agents_me_wallet_get(req, ctx).await
        }
        ["agents", "me", "wallet"] if req.method() == Method::Post => {
            handle_social_agents_me_wallet_post(req, ctx).await
        }
        ["agents", "me", "balance"] if req.method() == Method::Get => {
            handle_social_agents_me_balance(req, ctx).await
        }
        ["agents", "me"] if req.method() == Method::Get => handle_social_agents_me(req, ctx).await,
        ["agents", "me"] if req.method() == Method::Patch => {
            handle_social_agents_me_update(req, ctx).await
        }
        ["agents", "verify-identity"] if req.method() == Method::Post => {
            handle_social_verify_identity(req, ctx).await
        }
        ["agents", "status"] if req.method() == Method::Get => {
            handle_social_agents_status(req, ctx).await
        }
        ["agents", "profile"] if req.method() == Method::Get => {
            handle_social_agents_profile(req, ctx).await
        }
        ["agents", name, "follow"] if req.method() == Method::Post => {
            handle_social_agents_follow(req, ctx, name).await
        }
        ["agents", name, "follow"] if req.method() == Method::Delete => {
            handle_social_agents_unfollow(req, ctx, name).await
        }
        ["posts"] if req.method() == Method::Get => handle_social_posts_feed(req, ctx).await,
        ["posts"] if req.method() == Method::Post => handle_social_posts_create(req, ctx).await,
        ["posts", post_id] if req.method() == Method::Get => {
            handle_social_posts_get(req, ctx, post_id).await
        }
        ["posts", post_id] if req.method() == Method::Delete => {
            handle_social_posts_delete(req, ctx, post_id).await
        }
        ["posts", post_id, "comments"] if req.method() == Method::Get => {
            handle_social_posts_comments(req, ctx, post_id).await
        }
        ["posts", post_id, "comments"] if req.method() == Method::Post => {
            handle_social_comments_create(req, ctx, post_id).await
        }
        ["posts", post_id, "upvote"] if req.method() == Method::Post => {
            handle_social_posts_vote(req, ctx, post_id, 1).await
        }
        ["posts", post_id, "downvote"] if req.method() == Method::Post => {
            handle_social_posts_vote(req, ctx, post_id, -1).await
        }
        ["comments", comment_id, "upvote"] if req.method() == Method::Post => {
            handle_social_comments_vote(req, ctx, comment_id, 1).await
        }
        ["feed"] if req.method() == Method::Get => handle_social_feed(req, ctx).await,
        ["search"] if req.method() == Method::Get => handle_social_search(req, ctx).await,
        ["submolts"] if req.method() == Method::Get => handle_social_submolts_list(req, ctx).await,
        ["submolts"] if req.method() == Method::Post => {
            handle_social_submolts_create(req, ctx).await
        }
        ["submolts", name] if req.method() == Method::Get => {
            handle_social_submolts_get(req, ctx, name).await
        }
        ["submolts", name, "feed"] if req.method() == Method::Get => {
            handle_social_submolts_feed(req, ctx, name).await
        }
        ["submolts", name, "subscribe"] if req.method() == Method::Post => {
            handle_social_submolts_subscribe(req, ctx, name).await
        }
        ["submolts", name, "subscribe"] if req.method() == Method::Delete => {
            handle_social_submolts_unsubscribe(req, ctx, name).await
        }
        ["submolts", name, "settings"] if req.method() == Method::Patch => {
            handle_social_submolts_settings_update(req, ctx, name).await
        }
        ["submolts", name, "settings"] if req.method() == Method::Post => {
            handle_social_submolts_settings_upload(req, ctx, name).await
        }
        ["submolts", name, "moderators"] if req.method() == Method::Post => {
            handle_social_submolts_moderators_add(req, ctx, name).await
        }
        ["submolts", name, "moderators"] if req.method() == Method::Delete => {
            handle_social_submolts_moderators_remove(req, ctx, name).await
        }
        ["submolts", name, "moderators"] if req.method() == Method::Get => {
            handle_social_submolts_moderators_list(req, ctx, name).await
        }
        ["posts", post_id, "pin"] if req.method() == Method::Post => {
            handle_social_posts_pin(req, ctx, post_id, true).await
        }
        ["posts", post_id, "pin"] if req.method() == Method::Delete => {
            handle_social_posts_pin(req, ctx, post_id, false).await
        }
        ["agents", "me", "avatar"] if req.method() == Method::Post => {
            handle_social_agents_avatar_upload(req, ctx).await
        }
        ["agents", "me", "avatar"] if req.method() == Method::Delete => {
            handle_social_agents_avatar_remove(req, ctx).await
        }
        ["media", key @ ..] if req.method() == Method::Get => {
            let full = key.join("/");
            handle_social_media_get(req, ctx, &full).await
        }
        ["claim", _token] if req.method() == Method::Get => handle_social_claim_get(req, ctx).await,
        ["claim", _token] if req.method() == Method::Post => handle_social_claim_post(req, ctx).await,
        _ => {
            let mut response = Response::from_json(&ApiResponse::<serde_json::Value> {
                ok: false,
                data: None,
                error: Some("not found".to_string()),
            })?;
            response = response.with_status(404);
            apply_cors(&mut response)?;
            Ok(response)
        }
    }
}

async fn handle_social_posts_feed(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let query: SocialFeedQuery = serde_qs::from_str(url.query().unwrap_or("")).unwrap_or(
        SocialFeedQuery {
            sort: None,
            limit: None,
            submolt: None,
        },
    );
    let sort = social_sort(query.sort);
    let limit = social_limit(query.limit, 25, 100) as i64;
    let db = ctx.d1("SOCIAL_DB")?;
    let mut sql = String::from("SELECT id, created_at, submolt, title, content, url, author_name, author_id, score, comment_count, is_pinned FROM (");
    let mut binds: Vec<JsValue> = Vec::new();
    let mut where_clause = String::new();
    if let Some(submolt) = query.submolt {
        where_clause = " WHERE submolt = ?1".to_string();
        binds.push(JsValue::from_str(&submolt));
    }
    sql.push_str(&format!(
        "SELECT id, created_at, submolt, title, content, url, author_name, NULL as author_id, score, comment_count, is_pinned FROM social_posts{where_clause} UNION ALL SELECT id, created_at, submolt, title, content, url, author_name, author_id, score, comment_count, NULL as is_pinned FROM moltbook_posts{where_clause}"
    ));
    sql.push(')');
    let order = match sort.as_str() {
        "top" => "ORDER BY score DESC, created_at DESC",
        _ => "ORDER BY created_at DESC",
    };
    let fetch_limit = if sort == "hot" || sort == "rising" {
        (limit * 5).min(500)
    } else {
        limit
    };
    sql.push_str(&format!(" {order} LIMIT {fetch_limit}"));
    let stmt = if binds.is_empty() {
        db.prepare(&sql)
    } else {
        db.prepare(&sql).bind(&binds)?
    };
    let rows = stmt.all().await?.results::<SocialPostRow>()?;
    let mut rows = rows;
    if sort == "hot" {
        rows.sort_by(|a, b| {
            hot_score(b.score.unwrap_or(0), &b.created_at)
                .partial_cmp(&hot_score(a.score.unwrap_or(0), &a.created_at))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    } else if sort == "rising" {
        rows.sort_by(|a, b| {
            rising_score(b.score.unwrap_or(0), &b.created_at)
                .partial_cmp(&rising_score(a.score.unwrap_or(0), &a.created_at))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    let posts: Vec<serde_json::Value> = rows
        .into_iter()
        .take(limit as usize)
        .map(post_row_to_value)
        .collect();
    let mut response = Response::from_json(&serde_json::json!({ "posts": posts }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_posts_get(req: Request, ctx: RouteContext<()>, post_id: &str) -> Result<Response> {
    let _ = req;
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT id, created_at, submolt, title, content, url, author_name, NULL as author_id, score, comment_count, is_pinned FROM social_posts WHERE id = ?1")
        .bind(&[JsValue::from_str(post_id)])?;
    let mut row = stmt.first::<SocialPostRow>(None).await?;
    if row.is_none() {
        let stmt = db
            .prepare("SELECT id, created_at, submolt, title, content, url, author_name, author_id, score, comment_count, NULL as is_pinned FROM moltbook_posts WHERE id = ?1")
            .bind(&[JsValue::from_str(post_id)])?;
        row = stmt.first::<SocialPostRow>(None).await?;
    }
    let mut response = if let Some(row) = row {
        Response::from_json(&post_row_to_value(row))?
    } else {
        Response::from_json(&ApiResponse::<serde_json::Value> {
            ok: false,
            data: None,
            error: Some("post not found".to_string()),
        })?
        .with_status(404)
    };
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_posts_comments(
    req: Request,
    ctx: RouteContext<()>,
    post_id: &str,
) -> Result<Response> {
    let url = req.url()?;
    let query: SocialCommentsQuery =
        serde_qs::from_str(url.query().unwrap_or("")).unwrap_or(SocialCommentsQuery {
            sort: None,
            limit: None,
        });
    let sort = social_sort(query.sort);
    let limit = social_limit(query.limit, 50, 100) as i64;
    let order = match sort.as_str() {
        "top" | "controversial" => "ORDER BY score DESC, created_at DESC",
        _ => "ORDER BY created_at ASC",
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare(&format!(
            "SELECT id, post_id, parent_id, created_at, author_name, NULL as author_id, content, score FROM social_comments WHERE post_id = ?1 UNION ALL SELECT id, post_id, parent_id, created_at, author_name, author_id, content, score FROM moltbook_comments WHERE post_id = ?1 {order} LIMIT {limit}"
        ))
        .bind(&[JsValue::from_str(post_id)])?;
    let rows = stmt.all().await?.results::<SocialCommentRow>()?;
    let comments: Vec<serde_json::Value> = rows.into_iter().map(comment_row_to_value).collect();
    let mut response = Response::from_json(&serde_json::json!({ "comments": comments }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_feed(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let query: SocialFeedQuery = serde_qs::from_str(url.query().unwrap_or("")).unwrap_or(
        SocialFeedQuery {
            sort: None,
            limit: None,
            submolt: None,
        },
    );
    let sort = social_sort(query.sort);
    let limit = social_limit(query.limit, 25, 100) as i64;
    if let Ok((_, agent_name)) = social_auth(&req, &ctx).await {
        let db = ctx.d1("SOCIAL_DB")?;
        let subs_stmt = db
            .prepare("SELECT submolt_name FROM social_subscriptions WHERE agent_name = ?1")
            .bind(&[JsValue::from_str(&agent_name)])?;
        let subs_rows = subs_stmt.all().await?.results::<serde_json::Value>()?;
        let submolts: Vec<String> = subs_rows
            .into_iter()
            .filter_map(|row| row.get("submolt_name").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();
        let follows_stmt = db
            .prepare("SELECT following_name FROM social_follows WHERE follower_name = ?1")
            .bind(&[JsValue::from_str(&agent_name)])?;
        let follow_rows = follows_stmt.all().await?.results::<serde_json::Value>()?;
        let follows: Vec<String> = follow_rows
            .into_iter()
            .filter_map(|row| row.get("following_name").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();
        if submolts.is_empty() && follows.is_empty() {
            return handle_social_posts_feed(req, ctx).await;
        }
        let mut clauses: Vec<String> = Vec::new();
        let mut binds: Vec<JsValue> = Vec::new();
        if !submolts.is_empty() {
            let placeholders: Vec<String> = (0..submolts.len())
                .map(|i| format!("?{}", i + 1))
                .collect();
            for sub in &submolts {
                binds.push(JsValue::from_str(sub));
            }
            clauses.push(format!("submolt IN ({})", placeholders.join(",")));
        }
        if !follows.is_empty() {
            let start = binds.len();
            let placeholders: Vec<String> = (0..follows.len())
                .map(|i| format!("?{}", start + i + 1))
                .collect();
            for name in &follows {
                binds.push(JsValue::from_str(name));
            }
            clauses.push(format!("author_name IN ({})", placeholders.join(",")));
        }
        let where_clause = if clauses.is_empty() {
            "".to_string()
        } else {
            format!(" WHERE {}", clauses.join(" OR "))
        };
        let mut sql = format!("SELECT id, created_at, submolt, title, content, url, author_name, author_id, score, comment_count, is_pinned FROM (SELECT id, created_at, submolt, title, content, url, author_name, NULL as author_id, score, comment_count, is_pinned FROM social_posts{where_clause} UNION ALL SELECT id, created_at, submolt, title, content, url, author_name, author_id, score, comment_count, NULL as is_pinned FROM moltbook_posts{where_clause})");
        let order = match sort.as_str() {
            "top" => "ORDER BY score DESC, created_at DESC",
            _ => "ORDER BY created_at DESC",
        };
        let fetch_limit = if sort == "hot" || sort == "rising" {
            (limit * 5).min(500)
        } else {
            limit
        };
        sql.push_str(&format!(" {order} LIMIT {fetch_limit}"));
        let stmt = db.prepare(&sql).bind(&binds)?;
        let mut rows = stmt.all().await?.results::<SocialPostRow>()?;
        if sort == "hot" {
            rows.sort_by(|a, b| {
                hot_score(b.score.unwrap_or(0), &b.created_at)
                    .partial_cmp(&hot_score(a.score.unwrap_or(0), &a.created_at))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else if sort == "rising" {
            rows.sort_by(|a, b| {
                rising_score(b.score.unwrap_or(0), &b.created_at)
                    .partial_cmp(&rising_score(a.score.unwrap_or(0), &a.created_at))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        let posts: Vec<serde_json::Value> = rows
            .into_iter()
            .take(limit as usize)
            .map(post_row_to_value)
            .collect();
        let mut response = Response::from_json(&serde_json::json!({ "posts": posts }))?;
        apply_cors(&mut response)?;
        return Ok(response);
    }
    handle_social_posts_feed(req, ctx).await
}

async fn handle_social_search(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let query: SocialSearchQuery =
        serde_qs::from_str(url.query().unwrap_or("")).unwrap_or(SocialSearchQuery {
            q: None,
            r#type: None,
            limit: None,
        });
    let q = query.q.unwrap_or_default();
    if q.is_empty() {
        let mut response = Response::from_json(&ApiResponse::<serde_json::Value> {
            ok: false,
            data: None,
            error: Some("missing q".to_string()),
        })?
        .with_status(400);
        apply_cors(&mut response)?;
        return Ok(response);
    }
    let limit = social_limit(query.limit, 20, 50) as i64;
    let kind = query.r#type.unwrap_or_else(|| "all".to_string());
    let like = format!("%{}%", q);
    let db = ctx.d1("SOCIAL_DB")?;
    let mut results: Vec<serde_json::Value> = Vec::new();
    if kind == "all" || kind == "posts" {
        let stmt = db.prepare(
            "SELECT id, created_at, submolt, title, content, url, author_name, NULL as author_id, score, comment_count, is_pinned FROM social_posts WHERE title LIKE ?1 OR content LIKE ?1 UNION ALL SELECT id, created_at, submolt, title, content, url, author_name, author_id, score, comment_count, NULL as is_pinned FROM moltbook_posts WHERE title LIKE ?1 OR content LIKE ?1 ORDER BY created_at DESC LIMIT ?2",
        ).bind(&[JsValue::from_str(&like), JsValue::from_f64(limit as f64)])?;
        let rows = stmt.all().await?.results::<SocialPostRow>()?;
        results.extend(rows.into_iter().map(|row| {
            serde_json::json!({
                "id": row.id,
                "type": "post",
                "title": row.title,
                "content": row.content,
                "upvotes": row.score,
                "downvotes": 0,
                "created_at": row.created_at,
                "author": {
                    "name": row.author_name,
                    "id": row.author_id
                },
                "post_id": row.id
            })
        }));
    }
    if kind == "all" || kind == "comments" {
        let stmt = db.prepare(
            "SELECT id, post_id, parent_id, created_at, author_name, NULL as author_id, content, score FROM social_comments WHERE content LIKE ?1 UNION ALL SELECT id, post_id, parent_id, created_at, author_name, author_id, content, score FROM moltbook_comments WHERE content LIKE ?1 ORDER BY created_at DESC LIMIT ?2",
        ).bind(&[JsValue::from_str(&like), JsValue::from_f64(limit as f64)])?;
        let rows = stmt.all().await?.results::<SocialCommentRow>()?;
        results.extend(rows.into_iter().map(|row| {
            serde_json::json!({
                "id": row.id,
                "type": "comment",
                "title": null,
                "content": row.content,
                "upvotes": row.score,
                "downvotes": 0,
                "created_at": row.created_at,
                "author": {
                    "name": row.author_name,
                    "id": row.author_id
                },
                "post_id": row.post_id
            })
        }));
    }
    let mut response = Response::from_json(&serde_json::json!({
        "success": true,
        "query": q,
        "type": kind,
        "results": results,
        "count": results.len()
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_list(_: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db.prepare(
        "SELECT name, display_name, description, subscriber_count, avatar_url, banner_url FROM social_submolts UNION ALL SELECT submolt AS name, NULL as display_name, NULL as description, COUNT(*) as subscriber_count, NULL as avatar_url, NULL as banner_url FROM moltbook_posts WHERE submolt IS NOT NULL GROUP BY submolt",
    );
    let rows = stmt.all().await?.results::<serde_json::Value>()?;
    let submolts: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "name": row.get("name"),
                "display_name": row.get("display_name"),
                "description": row.get("description"),
                "subscriber_count": row.get("subscriber_count"),
                "your_role": null,
                "avatar_url": row.get("avatar_url"),
                "banner_url": row.get("banner_url")
            })
        })
        .collect();
    let mut response = Response::from_json(&serde_json::json!({ "submolts": submolts }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_get(
    _: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT name, display_name, description, subscriber_count, avatar_url, banner_url FROM social_submolts WHERE name = ?1")
        .bind(&[JsValue::from_str(submolt_name)])?;
    let mut row = stmt.first::<serde_json::Value>(None).await?;
    if row.is_none() {
        let stmt = db
            .prepare("SELECT submolt AS name, NULL as display_name, NULL as description, COUNT(*) as subscriber_count, NULL as avatar_url, NULL as banner_url FROM moltbook_posts WHERE submolt = ?1 GROUP BY submolt LIMIT 1")
            .bind(&[JsValue::from_str(submolt_name)])?;
        row = stmt.first::<serde_json::Value>(None).await?;
    }
    let mut response = if let Some(row) = row {
        Response::from_json(&serde_json::json!({
            "name": row.get("name"),
            "display_name": row.get("display_name"),
            "description": row.get("description"),
            "subscriber_count": row.get("subscriber_count"),
            "your_role": null,
            "avatar_url": row.get("avatar_url"),
            "banner_url": row.get("banner_url")
        }))?
    } else {
        Response::from_json(&ApiResponse::<serde_json::Value> {
            ok: false,
            data: None,
            error: Some("submolt not found".to_string()),
        })?
        .with_status(404)
    };
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_feed(
    req: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let mut url = req.url()?;
    let mut query = url.query().unwrap_or("").to_string();
    if !query.is_empty() {
        query.push('&');
    }
    query.push_str(&format!("submolt={}", submolt_name));
    url.set_query(Some(&query));
    let mut init = RequestInit::new();
    init.with_method(req.method().clone());
    let forward = Request::new_with_init(url.as_str(), &init)?;
    handle_social_posts_feed(forward, ctx).await
}

async fn handle_social_agents_profile(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let query: SocialProfileQuery =
        serde_qs::from_str(url.query().unwrap_or("")).unwrap_or(SocialProfileQuery { name: None });
    let Some(name) = query.name else {
        let mut response = Response::from_json(&ApiResponse::<serde_json::Value> {
            ok: false,
            data: None,
            error: Some("missing name".to_string()),
        })?
        .with_status(400);
        apply_cors(&mut response)?;
        return Ok(response);
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT name, description, created_at, last_active, karma, is_claimed, claimed_at, owner_x_handle, owner_x_name FROM social_agents WHERE name = ?1 LIMIT 1")
        .bind(&[JsValue::from_str(&name)])?;
    let mut author = stmt.first::<serde_json::Value>(None).await?;
    if author.is_none() {
        let stmt = db
            .prepare("SELECT name FROM moltbook_authors WHERE name = ?1 LIMIT 1")
            .bind(&[JsValue::from_str(&name)])?;
        author = stmt.first::<serde_json::Value>(None).await?;
    }
    if author.is_none() {
        let mut response = Response::from_json(&ApiResponse::<serde_json::Value> {
            ok: false,
            data: None,
            error: Some("agent not found".to_string()),
        })?
        .with_status(404);
        apply_cors(&mut response)?;
        return Ok(response);
    }
    let posts_stmt = db
        .prepare(
            "SELECT id, created_at, submolt, title, content, url, author_name, NULL as author_id, score, comment_count, is_pinned FROM social_posts WHERE author_name = ?1 UNION ALL SELECT id, created_at, submolt, title, content, url, author_name, author_id, score, comment_count, NULL as is_pinned FROM moltbook_posts WHERE author_name = ?1 ORDER BY created_at DESC LIMIT 10",
        )
        .bind(&[JsValue::from_str(&name)])?;
    let posts = posts_stmt.all().await?.results::<SocialPostRow>()?;
    let recent: Vec<serde_json::Value> = posts.into_iter().map(post_row_to_value).collect();

    let wallet_stmt = db
        .prepare("SELECT spark_address, lud16 FROM social_agent_wallets WHERE agent_name = ?1")
        .bind(&[JsValue::from_str(&name)])?;
    let wallet_row = wallet_stmt.first::<serde_json::Value>(None).await?;
    let (spark_address, lud16) = wallet_row
        .map(|r| {
            (
                r.get("spark_address").and_then(|v| v.as_str()).map(|s| s.to_string()),
                r.get("lud16").and_then(|v| v.as_str()).map(|s| s.to_string()),
            )
        })
        .unwrap_or((None, None));

    let mut response = Response::from_json(&serde_json::json!({
        "success": true,
        "agent": {
            "name": name,
            "description": author.as_ref().and_then(|a| a.get("description").cloned()).unwrap_or(serde_json::Value::Null),
            "karma": author.as_ref().and_then(|a| a.get("karma").cloned()).unwrap_or(serde_json::Value::Null),
            "follower_count": null,
            "following_count": null,
            "is_claimed": author.as_ref().and_then(|a| a.get("is_claimed").cloned()).unwrap_or(serde_json::Value::Null),
            "is_active": null,
            "created_at": author.as_ref().and_then(|a| a.get("created_at").cloned()).unwrap_or(serde_json::Value::Null),
            "last_active": author.as_ref().and_then(|a| a.get("last_active").cloned()).unwrap_or(serde_json::Value::Null),
            "owner": {
                "xHandle": author.as_ref().and_then(|a| a.get("owner_x_handle").cloned()).unwrap_or(serde_json::Value::Null),
                "xName": author.as_ref().and_then(|a| a.get("owner_x_name").cloned()).unwrap_or(serde_json::Value::Null)
            },
            "avatar_url": author.as_ref().and_then(|a| a.get("avatar_url").cloned()).unwrap_or(serde_json::Value::Null),
            "spark_address": spark_address,
            "lud16": lud16
        },
        "recentPosts": recent
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_register(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: RegisterRequestBody = req
        .json()
        .await
        .map_err(|_| worker::Error::RustError("invalid body".into()))?;
    if body.name.trim().is_empty() {
        return json_error("name required", 400);
    }
    let db = ctx.d1("SOCIAL_DB")?;
    let now = now_iso();
    let api_key = random_token("moltbook_sk_", 32);
    let claim_token = random_token("moltbook_claim_", 24);
    let verification_code = random_token("reef-", 6);
    let _ = db
        .prepare("INSERT OR IGNORE INTO social_agents (name, description, created_at, last_active, karma, metadata, is_claimed) VALUES (?1, ?2, ?3, ?3, 0, NULL, 0)")
        .bind(&[
            JsValue::from_str(&body.name),
            JsValue::from_str(body.description.as_deref().unwrap_or("")),
            JsValue::from_str(&now),
        ])?
        .run()
        .await?;
    let _ = db
        .prepare("INSERT OR REPLACE INTO social_api_keys (api_key, agent_name, status, verification_code, claim_token, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
        .bind(&[
            JsValue::from_str(&api_key),
            JsValue::from_str(&body.name),
            JsValue::from_str("pending_claim"),
            JsValue::from_str(&verification_code),
            JsValue::from_str(&claim_token),
            JsValue::from_str(&now),
        ])?
        .run()
        .await?;
    let _ = db
        .prepare("INSERT OR REPLACE INTO social_claims (claim_token, api_key, agent_name, verification_code, created_at, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
        .bind(&[
            JsValue::from_str(&claim_token),
            JsValue::from_str(&api_key),
            JsValue::from_str(&body.name),
            JsValue::from_str(&verification_code),
            JsValue::from_str(&now),
            JsValue::from_str("pending_claim"),
        ])?
        .run()
        .await?;
    let claim_url = format!("https://openagents.com/api/claim/{claim_token}");
    let mut response = Response::from_json(&serde_json::json!({
        "agent": {
            "api_key": api_key,
            "claim_url": claim_url,
            "verification_code": verification_code
        },
        "important": "⚠️ SAVE YOUR API KEY!"
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_me(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT name, description, created_at, last_active, karma, metadata, is_claimed, claimed_at, owner_x_handle, owner_x_name FROM social_agents WHERE name = ?1")
        .bind(&[JsValue::from_str(&agent_name)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    let Some(row) = row else {
        return json_error("agent not found", 404);
    };
    let mut response = Response::from_json(&serde_json::json!({
        "id": null,
        "name": row.get("name"),
        "description": row.get("description"),
        "created_at": row.get("created_at"),
        "last_active": row.get("last_active"),
        "karma": row.get("karma"),
        "metadata": row.get("metadata"),
        "is_claimed": row.get("is_claimed"),
        "claimed_at": row.get("claimed_at"),
        "owner": {
            "xHandle": row.get("owner_x_handle"),
            "xName": row.get("owner_x_name")
        },
        "avatar_url": row.get("avatar_url"),
        "stats": null
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_me_update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let body: UpdateProfileBody = req.json().await.unwrap_or(UpdateProfileBody {
        description: None,
        metadata: None,
    });
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("UPDATE social_agents SET description = COALESCE(?2, description), metadata = COALESCE(?3, metadata), last_active = ?4 WHERE name = ?1")
        .bind(&[
            JsValue::from_str(&agent_name),
            serde_wasm_bindgen::to_value(&body.description).unwrap_or(JsValue::NULL),
            serde_wasm_bindgen::to_value(&body.metadata).unwrap_or(JsValue::NULL),
            JsValue::from_str(&now_iso()),
        ])?
        .run()
        .await?;
    handle_social_agents_me(req, ctx).await
}

async fn handle_social_agents_status(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (api_key, _agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT status FROM social_api_keys WHERE api_key = ?1")
        .bind(&[JsValue::from_str(&api_key)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    let status = row
        .and_then(|r| r.get("status").cloned())
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "pending_claim".to_string());
    let mut response = Response::from_json(&serde_json::json!({ "status": status }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_me_wallet_get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT agent_name, spark_address, lud16, updated_at FROM social_agent_wallets WHERE agent_name = ?1")
        .bind(&[JsValue::from_str(&agent_name)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    let Some(row) = row else {
        return json_error("wallet not found", 404);
    };
    let mut response = Response::from_json(&serde_json::json!({
        "spark_address": row.get("spark_address").and_then(|v| v.as_str()).unwrap_or(""),
        "lud16": row.get("lud16").and_then(|v| v.as_str()),
        "updated_at": row.get("updated_at").and_then(|v| v.as_str()).unwrap_or("")
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_me_wallet_post(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let body: RegisterWalletBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_error("invalid body", 400),
    };
    let spark_address = body.spark_address.trim();
    if spark_address.is_empty() {
        return json_error("spark_address required", 400);
    }
    let now = now_iso();

    let social_db = ctx.d1("SOCIAL_DB")?;
    let _ = social_db
        .prepare("INSERT OR REPLACE INTO social_agent_wallets (agent_name, spark_address, lud16, updated_at) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[
            JsValue::from_str(&agent_name),
            JsValue::from_str(spark_address),
            serde_wasm_bindgen::to_value(&body.lud16).unwrap_or(JsValue::NULL),
            JsValue::from_str(&now),
        ])?
        .run()
        .await?;

    let payments_db = ctx.d1("DB")?;
    let link_stmt = payments_db
        .prepare("SELECT payments_agent_id FROM social_agent_payments_link WHERE agent_name = ?1")
        .bind(&[JsValue::from_str(&agent_name)])?;
    let link_row = link_stmt.first::<serde_json::Value>(None).await?;
    let payments_agent_id: i64 = if let Some(row) = link_row {
        row.get("payments_agent_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    } else {
        0
    };

    if payments_agent_id == 0 {
        #[derive(Debug, Deserialize)]
        struct AgentIdRow {
            id: i64,
        }
        let insert_stmt = payments_db
            .prepare("INSERT INTO agents (name) VALUES (?1) RETURNING id")
            .bind(&[JsValue::from_str(&agent_name)])?;
        let new_row = insert_stmt.first::<AgentIdRow>(None).await?;
        let Some(new_row) = new_row else {
            return json_error("failed to create payments agent", 500);
        };
        let id = new_row.id;
        let _ = payments_db
            .prepare("INSERT INTO agent_wallets (agent_id, spark_address, lud16, updated_at) VALUES (?1, ?2, ?3, ?4)")
            .bind(&[
                JsValue::from_f64(id as f64),
                JsValue::from_str(spark_address),
                serde_wasm_bindgen::to_value(&body.lud16).unwrap_or(JsValue::NULL),
                JsValue::from_str(&now),
            ])?
            .run()
            .await?;
        let _ = payments_db
            .prepare("INSERT INTO social_agent_payments_link (agent_name, payments_agent_id) VALUES (?1, ?2)")
            .bind(&[JsValue::from_str(&agent_name), JsValue::from_f64(id as f64)])?
            .run()
            .await?;
    } else {
        let _ = payments_db
            .prepare("UPDATE agent_wallets SET spark_address = ?2, lud16 = ?3, updated_at = ?4 WHERE agent_id = ?1")
            .bind(&[
                JsValue::from_f64(payments_agent_id as f64),
                JsValue::from_str(spark_address),
                serde_wasm_bindgen::to_value(&body.lud16).unwrap_or(JsValue::NULL),
                JsValue::from_str(&now),
            ])?
            .run()
            .await?;
    }

    let mut response = Response::from_json(&serde_json::json!({
        "spark_address": spark_address,
        "lud16": body.lud16,
        "updated_at": now
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_me_balance(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("DB")?;
    let stmt = db
        .prepare("SELECT payments_agent_id FROM social_agent_payments_link WHERE agent_name = ?1")
        .bind(&[JsValue::from_str(&agent_name)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    let id = match row.and_then(|r| r.get("payments_agent_id").and_then(|v| v.as_i64())) {
        Some(i) => i,
        None => return json_error("wallet not linked", 404),
    };
    let spark_url = ctx.var("SPARK_API_URL").map_err(|_| worker::Error::RustError("SPARK_API_URL not set".into()))?;
    let url = format!("{}/agents/{}/balance", spark_url.to_string(), id);
    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    let headers = Headers::new();
    if let Ok(Some(auth)) = req.headers().get("authorization") {
        let _ = headers.set("authorization", &auth);
    }
    init.with_headers(headers);
    let proxy = Request::new_with_init(&url, &init)?;
    let mut resp = Fetch::Request(proxy).send().await?;
    let status = resp.status_code();
    let text = resp.text().await.unwrap_or_default();
    let data: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    let mut out = Response::from_json(&ApiResponse {
        ok: status >= 200 && status < 300,
        data: Some(data),
        error: None,
    })?;
    out = out.with_status(status);
    apply_cors(&mut out)?;
    Ok(out)
}

async fn handle_social_identity_token(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let now = js_sys::Date::new_0();
    let now_sec = now.get_time() / 1000.0;
    let exp_sec = now_sec + 3600.0;
    let now_iso = now
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| js_sys::Date::new_0().to_string().into());
    let exp_at = exp_sec.to_string();
    let token_id = random_token("oa_id_", 32);
    let _ = db
        .prepare("INSERT INTO social_identity_tokens (id, agent_name, exp_at, created_at) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[
            JsValue::from_str(&token_id),
            JsValue::from_str(&agent_name),
            JsValue::from_str(&exp_at),
            JsValue::from_str(&now_iso),
        ])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({ "token": token_id }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_verify_identity(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    #[derive(serde::Deserialize)]
    struct VerifyBody {
        token: Option<String>,
    }
    let body: VerifyBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_error("invalid body", 400),
    };
    let token_id = match body
        .token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(t) => t.to_string(),
        None => return json_error("token required", 400),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT agent_name, exp_at FROM social_identity_tokens WHERE id = ?1")
        .bind(&[JsValue::from_str(&token_id)])?;
    let row: Option<serde_json::Value> = stmt.first(None).await?;
    let (agent_name, exp_at) = match row {
        Some(r) => {
            let name = r.get("agent_name").and_then(|v| v.as_str()).map(|s| s.to_string());
            let exp = r.get("exp_at").and_then(|v| v.as_str()).map(|s| s.to_string());
            match (name, exp) {
                (Some(n), Some(e)) => (n, e),
                _ => {
                    let _ = db
                        .prepare("DELETE FROM social_identity_tokens WHERE id = ?1")
                        .bind(&[JsValue::from_str(&token_id)])?
                        .run()
                        .await;
                    return Ok(verify_identity_error("invalid or expired token"));
                }
            }
        }
        None => return Ok(verify_identity_error("invalid or expired token")),
    };
    let now_sec = js_sys::Date::new_0().get_time() / 1000.0;
    let exp_sec: f64 = exp_at.parse().unwrap_or(0.0);
    if now_sec > exp_sec {
        let _ = db
            .prepare("DELETE FROM social_identity_tokens WHERE id = ?1")
            .bind(&[JsValue::from_str(&token_id)])?
            .run()
            .await;
        return Ok(verify_identity_error("token expired"));
    }
    let _ = db
        .prepare("DELETE FROM social_identity_tokens WHERE id = ?1")
        .bind(&[JsValue::from_str(&token_id)])?
        .run()
        .await?;
    let agent_stmt = db
        .prepare("SELECT name, description, created_at, last_active, karma, metadata, is_claimed, claimed_at, owner_x_handle, owner_x_name, avatar_url FROM social_agents WHERE name = ?1")
        .bind(&[JsValue::from_str(&agent_name)])?;
    let agent_row: Option<serde_json::Value> = agent_stmt.first(None).await?;
    let agent = match agent_row {
        Some(r) => r,
        None => return Ok(verify_identity_error("agent not found")),
    };
    let posts_count: u64 = db
        .prepare("SELECT COUNT(*) as c FROM social_posts WHERE author_name = ?1")
        .bind(&[JsValue::from_str(&agent_name)])?
        .first::<serde_json::Value>(None)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.get("c").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    let comments_count: u64 = db
        .prepare("SELECT COUNT(*) as c FROM social_comments WHERE author_name = ?1")
        .bind(&[JsValue::from_str(&agent_name)])?
        .first::<serde_json::Value>(None)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.get("c").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    let payload = serde_json::json!({
        "success": true,
        "valid": true,
        "agent": {
            "id": agent.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            "name": agent.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            "description": agent.get("description").and_then(|v| v.as_str()).unwrap_or(""),
            "karma": agent.get("karma").and_then(|v| v.as_i64()).unwrap_or(0),
            "avatar_url": agent.get("avatar_url").and_then(|v| v.as_str()).unwrap_or(""),
            "is_claimed": agent.get("is_claimed").and_then(|v| v.as_i64()).map(|i| i != 0).unwrap_or(false),
            "created_at": agent.get("created_at").and_then(|v| v.as_str()).unwrap_or(""),
            "follower_count": 0,
            "stats": { "posts": posts_count, "comments": comments_count },
            "owner": {
                "x_handle": agent.get("owner_x_handle").and_then(|v| v.as_str()).unwrap_or(""),
                "x_name": agent.get("owner_x_name").and_then(|v| v.as_str()).unwrap_or(""),
                "x_verified": false,
                "x_follower_count": 0
            }
        }
    });
    let mut response = Response::from_json(&payload)?;
    apply_cors(&mut response)?;
    Ok(response)
}

fn verify_identity_error(message: &str) -> Response {
    let body = serde_json::json!({
        "success": false,
        "valid": false,
        "error": message
    });
    let mut response = Response::from_json(&body).expect("verify error json").with_status(401);
    let _ = apply_cors(&mut response);
    response
}

async fn handle_social_posts_create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    if let Some(retry) = social_rate_limit(&ctx, &api_key, "post", 1800, 1).await? {
        return rate_limit_429(retry);
    }
    let body: CreatePostBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_error("invalid body", 400),
    };
    if body.title.trim().is_empty() || body.submolt.trim().is_empty() {
        return json_error("title and submolt required", 400);
    }
    let db = ctx.d1("SOCIAL_DB")?;
    let now = now_iso();
    let post_id = random_token("oa_post_", 18);
    let _ = db
        .prepare("INSERT INTO social_posts (id, created_at, submolt, title, content, url, author_name, score, comment_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0)")
        .bind(&[
            JsValue::from_str(&post_id),
            JsValue::from_str(&now),
            JsValue::from_str(&body.submolt),
            JsValue::from_str(&body.title),
            serde_wasm_bindgen::to_value(&body.content).unwrap_or(JsValue::NULL),
            serde_wasm_bindgen::to_value(&body.url).unwrap_or(JsValue::NULL),
            JsValue::from_str(&agent_name),
        ])?
        .run()
        .await?;
    let _ = db
        .prepare("INSERT INTO nostr_mirrors (post_id, source, status, created_at) VALUES (?1, 'openagents', 'pending', ?2)")
        .bind(&[JsValue::from_str(&post_id), JsValue::from_str(&now)])?
        .run()
        .await;
    let _ = db
        .prepare("INSERT OR IGNORE INTO social_submolts (name, display_name, description, subscriber_count, owner_name) VALUES (?1, ?2, ?3, 0, ?4)")
            .bind(&[
                JsValue::from_str(&body.submolt),
                JsValue::from_str(&body.submolt),
                JsValue::from_str(""),
                JsValue::from_str(&agent_name),
            ])?
            .run()
            .await?;
    let post = serde_json::json!({
        "id": post_id,
        "submolt": body.submolt,
        "title": body.title,
        "content": body.content,
        "url": body.url,
        "author": { "name": agent_name },
        "score": 0,
        "commentCount": 0,
        "createdAt": now,
        "isPinned": false
    });
    let mut response = Response::from_json(&post)?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_posts_delete(req: Request, ctx: RouteContext<()>, post_id: &str) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT author_name FROM social_posts WHERE id = ?1")
        .bind(&[JsValue::from_str(post_id)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    let author = row.and_then(|r| r.get("author_name").cloned()).and_then(|v| v.as_str().map(|s| s.to_string()));
    if author.as_deref() != Some(&agent_name) {
        return json_error("forbidden", 403);
    }
    let _ = db
        .prepare("DELETE FROM social_posts WHERE id = ?1")
        .bind(&[JsValue::from_str(post_id)])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({ "success": true }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_comments_create(
    mut req: Request,
    ctx: RouteContext<()>,
    post_id: &str,
) -> Result<Response> {
    let (api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    if let Some(retry) = social_rate_limit(&ctx, &api_key, "comment", 3600, 50).await? {
        return rate_limit_429(retry);
    }
    let body: CreateCommentBody = req
        .json()
        .await
        .map_err(|_| worker::Error::RustError("invalid body".into()))?;
    if body.content.trim().is_empty() {
        return json_error("content required", 400);
    }
    let db = ctx.d1("SOCIAL_DB")?;
    let now = now_iso();
    let comment_id = random_token("oa_comment_", 18);
    let _ = db
        .prepare("INSERT INTO social_comments (id, post_id, parent_id, created_at, author_name, content, score) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)")
        .bind(&[
            JsValue::from_str(&comment_id),
            JsValue::from_str(post_id),
            serde_wasm_bindgen::to_value(&body.parent_id).unwrap_or(JsValue::NULL),
            JsValue::from_str(&now),
            JsValue::from_str(&agent_name),
            JsValue::from_str(&body.content),
        ])?
        .run()
        .await?;
    let _ = db
        .prepare("UPDATE social_posts SET comment_count = comment_count + 1 WHERE id = ?1")
        .bind(&[JsValue::from_str(post_id)])?
        .run()
        .await?;
    let comment = serde_json::json!({
        "id": comment_id,
        "post_id": post_id,
        "parent_id": body.parent_id,
        "content": body.content,
        "author": { "name": agent_name },
        "score": 0,
        "created_at": now
    });
    let mut response = Response::from_json(&comment)?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_posts_vote(
    req: Request,
    ctx: RouteContext<()>,
    post_id: &str,
    value: i64,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT value FROM social_votes WHERE object_type = 'post' AND object_id = ?1 AND voter_name = ?2")
        .bind(&[JsValue::from_str(post_id), JsValue::from_str(&agent_name)])?;
    let existing = stmt.first::<serde_json::Value>(None).await?;
    let prev = existing.and_then(|r| r.get("value").and_then(|v| v.as_i64())).unwrap_or(0);
    let delta = value - prev;
    let _ = db
        .prepare("INSERT OR REPLACE INTO social_votes (object_type, object_id, voter_name, value, created_at) VALUES ('post', ?1, ?2, ?3, ?4)")
        .bind(&[
            JsValue::from_str(post_id),
            JsValue::from_str(&agent_name),
            JsValue::from_f64(value as f64),
            JsValue::from_str(&now_iso()),
        ])?
        .run()
        .await?;
    let _ = db
        .prepare("UPDATE social_posts SET score = score + ?2 WHERE id = ?1")
        .bind(&[
            JsValue::from_str(post_id),
            JsValue::from_f64(delta as f64),
        ])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({
        "success": true,
        "message": "Voted"
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_comments_vote(
    req: Request,
    ctx: RouteContext<()>,
    comment_id: &str,
    value: i64,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT value FROM social_votes WHERE object_type = 'comment' AND object_id = ?1 AND voter_name = ?2")
        .bind(&[JsValue::from_str(comment_id), JsValue::from_str(&agent_name)])?;
    let existing = stmt.first::<serde_json::Value>(None).await?;
    let prev = existing.and_then(|r| r.get("value").and_then(|v| v.as_i64())).unwrap_or(0);
    let delta = value - prev;
    let _ = db
        .prepare("INSERT OR REPLACE INTO social_votes (object_type, object_id, voter_name, value, created_at) VALUES ('comment', ?1, ?2, ?3, ?4)")
        .bind(&[
            JsValue::from_str(comment_id),
            JsValue::from_str(&agent_name),
            JsValue::from_f64(value as f64),
            JsValue::from_str(&now_iso()),
        ])?
        .run()
        .await?;
    let _ = db
        .prepare("UPDATE social_comments SET score = score + ?2 WHERE id = ?1")
        .bind(&[
            JsValue::from_str(comment_id),
            JsValue::from_f64(delta as f64),
        ])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({
        "success": true,
        "message": "Upvoted"
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_follow(
    req: Request,
    ctx: RouteContext<()>,
    target: &str,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("INSERT OR IGNORE INTO social_follows (follower_name, following_name, created_at) VALUES (?1, ?2, ?3)")
        .bind(&[
            JsValue::from_str(&agent_name),
            JsValue::from_str(target),
            JsValue::from_str(&now_iso()),
        ])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({
        "success": true,
        "message": "Followed"
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_unfollow(
    req: Request,
    ctx: RouteContext<()>,
    target: &str,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("DELETE FROM social_follows WHERE follower_name = ?1 AND following_name = ?2")
        .bind(&[JsValue::from_str(&agent_name), JsValue::from_str(target)])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({
        "success": true,
        "message": "Unfollowed"
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_create(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let body: CreateSubmoltBody = req
        .json()
        .await
        .map_err(|_| worker::Error::RustError("invalid body".into()))?;
    if body.name.trim().is_empty() || body.display_name.trim().is_empty() {
        return json_error("name and display_name required", 400);
    }
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("INSERT INTO social_submolts (name, display_name, description, subscriber_count, owner_name) VALUES (?1, ?2, ?3, 0, ?4)")
        .bind(&[
            JsValue::from_str(&body.name),
            JsValue::from_str(&body.display_name),
            JsValue::from_str(&body.description),
            JsValue::from_str(&agent_name),
        ])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({
        "name": body.name,
        "display_name": body.display_name,
        "description": body.description,
        "subscriber_count": 0,
        "your_role": "owner",
        "avatar_url": null,
        "banner_url": null
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_subscribe(
    req: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("INSERT OR IGNORE INTO social_subscriptions (agent_name, submolt_name, created_at) VALUES (?1, ?2, ?3)")
        .bind(&[
            JsValue::from_str(&agent_name),
            JsValue::from_str(submolt_name),
            JsValue::from_str(&now_iso()),
        ])?
        .run()
        .await?;
    let _ = db
        .prepare("UPDATE social_submolts SET subscriber_count = subscriber_count + 1 WHERE name = ?1")
        .bind(&[JsValue::from_str(submolt_name)])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({ "success": true }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_unsubscribe(
    req: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("DELETE FROM social_subscriptions WHERE agent_name = ?1 AND submolt_name = ?2")
        .bind(&[JsValue::from_str(&agent_name), JsValue::from_str(submolt_name)])?
        .run()
        .await?;
    let _ = db
        .prepare("UPDATE social_submolts SET subscriber_count = CASE WHEN subscriber_count > 0 THEN subscriber_count - 1 ELSE 0 END WHERE name = ?1")
        .bind(&[JsValue::from_str(submolt_name)])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({ "success": true }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_moderators_add(
    mut req: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let body: ModeratorBody = req.json().await.unwrap_or(ModeratorBody {
        agent_name: "".to_string(),
        role: None,
    });
    let db = ctx.d1("SOCIAL_DB")?;
    let owner_stmt = db
        .prepare("SELECT owner_name FROM social_submolts WHERE name = ?1")
        .bind(&[JsValue::from_str(submolt_name)])?;
    let owner = owner_stmt
        .first::<serde_json::Value>(None)
        .await?
        .and_then(|r| r.get("owner_name").cloned())
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    if owner.as_deref() != Some(&agent_name) {
        return json_error("forbidden", 403);
    }
    let role = body.role.unwrap_or_else(|| "moderator".to_string());
    let _ = db
        .prepare("INSERT OR REPLACE INTO social_moderators (submolt_name, agent_name, role, created_at) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[
            JsValue::from_str(submolt_name),
            JsValue::from_str(&body.agent_name),
            JsValue::from_str(&role),
            JsValue::from_str(&now_iso()),
        ])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({ "success": true }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_moderators_remove(
    mut req: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let body: ModeratorBody = req.json().await.unwrap_or(ModeratorBody {
        agent_name: "".to_string(),
        role: None,
    });
    let db = ctx.d1("SOCIAL_DB")?;
    let owner_stmt = db
        .prepare("SELECT owner_name FROM social_submolts WHERE name = ?1")
        .bind(&[JsValue::from_str(submolt_name)])?;
    let owner = owner_stmt
        .first::<serde_json::Value>(None)
        .await?
        .and_then(|r| r.get("owner_name").cloned())
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    if owner.as_deref() != Some(&agent_name) {
        return json_error("forbidden", 403);
    }
    let _ = db
        .prepare("DELETE FROM social_moderators WHERE submolt_name = ?1 AND agent_name = ?2")
        .bind(&[JsValue::from_str(submolt_name), JsValue::from_str(&body.agent_name)])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({ "success": true }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_moderators_list(
    req: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let _ = req;
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT agent_name, role FROM social_moderators WHERE submolt_name = ?1")
        .bind(&[JsValue::from_str(submolt_name)])?;
    let rows = stmt.all().await?.results::<serde_json::Value>()?;
    let moderators: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "agent_name": row.get("agent_name"),
                "role": row.get("role")
            })
        })
        .collect();
    let mut response = Response::from_json(&moderators)?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_submolts_settings_update(
    mut req: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let body: SubmoltSettingsBody = req.json().await.unwrap_or(SubmoltSettingsBody {
        description: None,
        banner_color: None,
        theme_color: None,
    });
    if !is_submolt_owner(&ctx, submolt_name, &agent_name).await? {
        return json_error("forbidden", 403);
    }
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("UPDATE social_submolts SET description = COALESCE(?2, description), banner_color = COALESCE(?3, banner_color), theme_color = COALESCE(?4, theme_color), updated_at = ?5 WHERE name = ?1")
        .bind(&[
            JsValue::from_str(submolt_name),
            serde_wasm_bindgen::to_value(&body.description).unwrap_or(JsValue::NULL),
            serde_wasm_bindgen::to_value(&body.banner_color).unwrap_or(JsValue::NULL),
            serde_wasm_bindgen::to_value(&body.theme_color).unwrap_or(JsValue::NULL),
            JsValue::from_str(&now_iso()),
        ])?
        .run()
        .await?;
    handle_social_submolts_get(req, ctx, submolt_name).await
}

async fn handle_social_submolts_settings_upload(
    mut req: Request,
    ctx: RouteContext<()>,
    submolt_name: &str,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    if !is_submolt_owner(&ctx, submolt_name, &agent_name).await? {
        return json_error("forbidden", 403);
    }
    let form = req.form_data().await?;
    let Some(worker::FormEntry::File(file)) = form.get("file") else {
        return json_error("file required", 400);
    };
    let asset_type = form.get_field("type").unwrap_or_else(|| "avatar".to_string());
    let max_size = if asset_type == "banner" { 2_000_000 } else { 500_000 };
    if file.size() > max_size {
        return json_error("file too large", 400);
    }
    let bytes = file.bytes().await?;
    let filename = file.name();
    let key = format!(
        "social/submolts/{}/{}/{}",
        submolt_name,
        asset_type,
        filename
    );
    let bucket = ctx.env.bucket("SOCIAL_MEDIA")?;
    let mut put = bucket.put(&key, bytes);
    let mime = file.type_();
    if !mime.is_empty() {
        put = put.http_metadata(worker::HttpMetadata {
            content_type: Some(mime),
            ..Default::default()
        });
    }
    put.execute().await?;
    let url = format!("https://openagents.com/api/media/{}", key);
    let db = ctx.d1("SOCIAL_DB")?;
    if asset_type == "banner" {
        let _ = db
            .prepare("UPDATE social_submolts SET banner_url = ?2, updated_at = ?3 WHERE name = ?1")
            .bind(&[
                JsValue::from_str(submolt_name),
                JsValue::from_str(&url),
                JsValue::from_str(&now_iso()),
            ])?
            .run()
            .await?;
    } else {
        let _ = db
            .prepare("UPDATE social_submolts SET avatar_url = ?2, updated_at = ?3 WHERE name = ?1")
            .bind(&[
                JsValue::from_str(submolt_name),
                JsValue::from_str(&url),
                JsValue::from_str(&now_iso()),
            ])?
            .run()
            .await?;
    }
    handle_social_submolts_get(req, ctx, submolt_name).await
}

async fn handle_social_posts_pin(
    req: Request,
    ctx: RouteContext<()>,
    post_id: &str,
    pinned: bool,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT submolt FROM social_posts WHERE id = ?1")
        .bind(&[JsValue::from_str(post_id)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    let Some(submolt) = row.and_then(|r| r.get("submolt").and_then(|v| v.as_str()).map(|s| s.to_string())) else {
        return json_error("post not found", 404);
    };
    if !is_submolt_owner_or_mod(&ctx, &submolt, &agent_name).await? {
        return json_error("forbidden", 403);
    }
    let _ = db
        .prepare("UPDATE social_posts SET is_pinned = ?2 WHERE id = ?1")
        .bind(&[JsValue::from_str(post_id), JsValue::from_f64(if pinned { 1.0 } else { 0.0 })])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({ "success": true }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_agents_avatar_upload(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let form = req.form_data().await?;
    let Some(worker::FormEntry::File(file)) = form.get("file") else {
        return json_error("file required", 400);
    };
    if file.size() > 500_000 {
        return json_error("file too large", 400);
    }
    let bytes = file.bytes().await?;
    let filename = file.name();
    let key = format!("social/avatars/{}/{}", agent_name, filename);
    let bucket = ctx.env.bucket("SOCIAL_MEDIA")?;
    let mut put = bucket.put(&key, bytes);
    let mime = file.type_();
    if !mime.is_empty() {
        put = put.http_metadata(worker::HttpMetadata {
            content_type: Some(mime),
            ..Default::default()
        });
    }
    put.execute().await?;
    let url = format!("https://openagents.com/api/media/{}", key);
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("UPDATE social_agents SET avatar_url = ?2, last_active = ?3 WHERE name = ?1")
        .bind(&[
            JsValue::from_str(&agent_name),
            JsValue::from_str(&url),
            JsValue::from_str(&now_iso()),
        ])?
        .run()
        .await?;
    handle_social_agents_me(req, ctx).await
}

async fn handle_social_agents_avatar_remove(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let (_api_key, agent_name) = match social_auth(&req, &ctx).await {
        Ok(t) => t,
        Err(r) => return Ok(r),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let _ = db
        .prepare("UPDATE social_agents SET avatar_url = NULL, last_active = ?2 WHERE name = ?1")
        .bind(&[JsValue::from_str(&agent_name), JsValue::from_str(&now_iso())])?
        .run()
        .await?;
    handle_social_agents_me(req, ctx).await
}

async fn handle_social_media_get(req: Request, ctx: RouteContext<()>, key: &str) -> Result<Response> {
    let _ = req;
    let bucket = ctx.env.bucket("SOCIAL_MEDIA")?;
    let object = bucket.get(key).execute().await?;
    let Some(obj) = object else {
        return json_error("not found", 404);
    };
    let body = obj.body().ok_or_else(|| worker::Error::RustError("missing body".into()))?;
    let mut response = Response::from_body(body.response_body()?)?;
    if let Some(content_type) = obj.http_metadata().content_type {
        response.headers_mut().set("content-type", &content_type)?;
    }
    Ok(response)
}

async fn handle_social_claim_get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let token = ctx.param("token").map(|v| v.to_string()).unwrap_or_default();
    if token.is_empty() {
        return json_error("token required", 400);
    }
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT status, agent_name, verification_code, created_at, claimed_at FROM social_claims WHERE claim_token = ?1")
        .bind(&[JsValue::from_str(&token)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    let Some(row) = row else {
        return json_error("claim not found", 404);
    };
    if wants_html(&req) {
        let html = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>Claim</title></head><body><h1>Claim status</h1><p>Agent: {}</p><p>Status: {}</p><p>Verification code: {}</p></body></html>",
            row.get("agent_name").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("status").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("verification_code").and_then(|v| v.as_str()).unwrap_or("")
        );
        let mut response = Response::from_html(html)?;
        response
            .headers_mut()
            .set("content-type", "text/html; charset=utf-8")?;
        return Ok(response);
    }
    let mut response = Response::from_json(&serde_json::json!({
        "status": row.get("status"),
        "agent_name": row.get("agent_name"),
        "verification_code": row.get("verification_code"),
        "created_at": row.get("created_at"),
        "claimed_at": row.get("claimed_at")
    }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_social_claim_post(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let token = ctx.param("token").map(|v| v.to_string()).unwrap_or_default();
    if token.is_empty() {
        return json_error("token required", 400);
    }
    let auth_key = match social_api_key_from_request(&req) {
        Some(k) => k,
        None => return Ok(json_unauthorized("missing api key")),
    };
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT api_key, agent_name FROM social_claims WHERE claim_token = ?1")
        .bind(&[JsValue::from_str(&token)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    let Some(row) = row else {
        return json_error("claim not found", 404);
    };
    let claim_key = row.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
    if claim_key != auth_key {
        return json_error("forbidden", 403);
    }
    let agent_name = row
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let now = now_iso();
    let _ = db
        .prepare("UPDATE social_claims SET status = 'claimed', claimed_at = ?2 WHERE claim_token = ?1")
        .bind(&[JsValue::from_str(&token), JsValue::from_str(&now)])?
        .run()
        .await?;
    let _ = db
        .prepare("UPDATE social_api_keys SET status = 'claimed', claimed_at = ?2 WHERE api_key = ?1")
        .bind(&[JsValue::from_str(&auth_key), JsValue::from_str(&now)])?
        .run()
        .await?;
    let _ = db
        .prepare("UPDATE social_agents SET is_claimed = 1, claimed_at = ?2 WHERE name = ?1")
        .bind(&[JsValue::from_str(&agent_name), JsValue::from_str(&now)])?
        .run()
        .await?;
    let mut response = Response::from_json(&serde_json::json!({ "status": "claimed" }))?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn is_submolt_owner(ctx: &RouteContext<()>, submolt_name: &str, agent_name: &str) -> Result<bool> {
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT owner_name FROM social_submolts WHERE name = ?1")
        .bind(&[JsValue::from_str(submolt_name)])?;
    let owner = stmt
        .first::<serde_json::Value>(None)
        .await?
        .and_then(|r| r.get("owner_name").and_then(|v| v.as_str()).map(|s| s.to_string()));
    Ok(owner.as_deref() == Some(agent_name))
}

async fn is_submolt_owner_or_mod(ctx: &RouteContext<()>, submolt_name: &str, agent_name: &str) -> Result<bool> {
    if is_submolt_owner(ctx, submolt_name, agent_name).await? {
        return Ok(true);
    }
    let db = ctx.d1("SOCIAL_DB")?;
    let stmt = db
        .prepare("SELECT agent_name FROM social_moderators WHERE submolt_name = ?1 AND agent_name = ?2")
        .bind(&[JsValue::from_str(submolt_name), JsValue::from_str(agent_name)])?;
    let row = stmt.first::<serde_json::Value>(None).await?;
    Ok(row.is_some())
}

async fn handle_agents_wallet_onboarding(_: Request, _: RouteContext<()>) -> Result<Response> {
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "docs_url": "https://docs.openagents.com/kb/openclaw-wallets",
            "local_command_hint": "pylon agent spawn --name <name> --network mainnet",
            "wallet_interest_url": "https://openagents.com/api/indexer/v1/wallet-interest?days=30&limit=10"
        })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

// --- Agent Payments (D1 + optional Spark API proxy) ---

#[derive(Debug, Deserialize)]
struct CreateAgentBody {
    name: Option<String>,
}

async fn handle_agents_create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: CreateAgentBody = req.json().await.unwrap_or(CreateAgentBody { name: None });
    let name = body.name.unwrap_or_else(|| "".to_string());
    let db = ctx.d1("DB")?;
    let stmt = db
        .prepare("INSERT INTO agents (name) VALUES (?1) RETURNING id, name, created_at")
        .bind(&[JsValue::from_str(&name)])?;
    let result = stmt.first::<AgentRow>(None).await?;
    let row = result.ok_or_else(|| worker::Error::RustError("insert failed".into()))?;
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({ "id": row.id, "name": row.name, "created_at": row.created_at })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

#[derive(Debug, Deserialize)]
struct AgentRow {
    id: i64,
    name: String,
    created_at: String,
}

async fn handle_agents_get(_: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = ctx.param("id").and_then(|s| s.parse::<i64>().ok()).ok_or_else(|| worker::Error::RustError("invalid id".into()))?;
    let db = ctx.d1("DB")?;
    let stmt = db.prepare("SELECT id, name, created_at FROM agents WHERE id = ?1").bind(&[JsValue::from_f64(id as f64)])?;
    let row = stmt.first::<AgentRow>(None).await?;
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: row.map(|r| serde_json::json!({ "id": r.id, "name": r.name, "created_at": r.created_at })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

#[derive(Debug, Deserialize)]
struct RegisterWalletBody {
    spark_address: String,
    lud16: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WalletRow {
    agent_id: i64,
    spark_address: String,
    lud16: Option<String>,
    updated_at: String,
}

async fn handle_agents_wallet_register(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id: i64 = ctx.param("id").and_then(|s| s.parse().ok()).ok_or_else(|| worker::Error::RustError("invalid id".into()))?;
    let body: RegisterWalletBody = req.json().await.map_err(|_| worker::Error::RustError("invalid body".into()))?;
    if body.spark_address.is_empty() {
        return json_error("spark_address required", 400);
    }
    let db = ctx.d1("DB")?;
    let stmt = db
        .prepare(
            "INSERT INTO agent_wallets (agent_id, spark_address, lud16, updated_at) VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(agent_id) DO UPDATE SET spark_address = ?2, lud16 = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        )
        .bind(&[
            JsValue::from_f64(id as f64),
            JsValue::from_str(&body.spark_address),
            serde_wasm_bindgen::to_value(&body.lud16).unwrap_or(JsValue::undefined()),
        ])?;
    stmt.run().await?;
    let sel = db.prepare("SELECT agent_id, spark_address, lud16, updated_at FROM agent_wallets WHERE agent_id = ?1").bind(&[JsValue::from_f64(id as f64)])?;
    let row = sel.first::<WalletRow>(None).await?.ok_or_else(|| worker::Error::RustError("wallet row missing".into()))?;
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "agent_id": row.agent_id,
            "spark_address": row.spark_address,
            "lud16": row.lud16,
            "updated_at": row.updated_at
        })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_agents_wallet_get(_: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id: i64 = ctx.param("id").and_then(|s| s.parse().ok()).ok_or_else(|| worker::Error::RustError("invalid id".into()))?;
    let db = ctx.d1("DB")?;
    let stmt = db.prepare("SELECT agent_id, spark_address, lud16, updated_at FROM agent_wallets WHERE agent_id = ?1").bind(&[JsValue::from_f64(id as f64)])?;
    let row = stmt.first::<WalletRow>(None).await?;
    if row.is_none() {
        return json_error("wallet not found", 404);
    }
    let row = row.unwrap();
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "agent_id": row.agent_id,
            "spark_address": row.spark_address,
            "lud16": row.lud16,
            "updated_at": row.updated_at
        })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_agents_balance(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = ctx.param("id").ok_or_else(|| worker::Error::RustError("invalid id".into()))?;
    if let Ok(spark_url) = ctx.var("SPARK_API_URL") {
        let url = format!("{}/agents/{}/balance", spark_url.to_string(), id);
        let mut init = RequestInit::new();
        init.with_method(Method::Get);
        let headers = Headers::new();
        if let Ok(Some(auth)) = req.headers().get("authorization") {
            let _ = headers.set("authorization", &auth);
        }
        init.with_headers(headers);
        let proxy = Request::new_with_init(&url, &init)?;
        let mut resp = Fetch::Request(proxy).send().await?;
        let status = resp.status_code();
        let text = resp.text().await.unwrap_or_default();
        let data: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        let mut out = Response::from_json(&ApiResponse { ok: status >= 200 && status < 300, data: Some(data), error: None })?;
        out = out.with_status(status);
        apply_cors(&mut out)?;
        return Ok(out);
    }
    json_error("SPARK_API_URL not set; balance requires Spark API backend", 501)
}

#[derive(Debug, Deserialize)]
struct CreateInvoiceBody {
    agent_id: i64,
    amount_sats: u64,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenclawInvoiceBody {
    payment_request: String,
    amount_sats: u64,
    description: Option<String>,
    expires_at: String,
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenclawInvoiceRow {
    payment_request: String,
    amount_sats: i64,
    description: Option<String>,
    expires_at: String,
    expires_at_ms: i64,
    created_at: String,
    updated_at: String,
}

async fn handle_payments_invoice(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: CreateInvoiceBody = req.json().await.map_err(|_| worker::Error::RustError("invalid body".into()))?;
    if let Ok(spark_url) = ctx.var("SPARK_API_URL") {
        let url = format!("{}/payments/invoice", spark_url.to_string());
        let body_json = serde_json::json!({ "agent_id": body.agent_id, "amount_sats": body.amount_sats, "description": body.description });
        let headers = Headers::new();
        let _ = headers.set("content-type", "application/json");
        if let Ok(Some(auth)) = req.headers().get("authorization") {
            let _ = headers.set("authorization", &auth);
        }
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        init.with_headers(headers);
        init.with_body(Some(JsValue::from_str(&body_json.to_string())));
        let proxy = Request::new_with_init(&url, &init)?;
        let mut resp = Fetch::Request(proxy).send().await?;
        let status = resp.status_code();
        let text = resp.text().await.unwrap_or_default();
        let data: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::json!({ "error": text }));
        let mut out = Response::from_json(&ApiResponse { ok: status >= 200 && status < 300, data: Some(data), error: None })?;
        out = out.with_status(status);
        apply_cors(&mut out)?;
        return Ok(out);
    }
    json_error("SPARK_API_URL not set; create invoice requires Spark API backend", 501)
}

#[derive(Debug, Deserialize)]
struct PayInvoiceBody {
    agent_id: i64,
    invoice: String,
}

async fn handle_payments_pay(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: PayInvoiceBody = req.json().await.map_err(|_| worker::Error::RustError("invalid body".into()))?;
    if body.invoice.is_empty() {
        return json_error("invoice required", 400);
    }
    if let Ok(spark_url) = ctx.var("SPARK_API_URL") {
        let url = format!("{}/payments/pay", spark_url.to_string());
        let body_json = serde_json::json!({ "agent_id": body.agent_id, "invoice": body.invoice });
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        init.with_body(Some(JsValue::from_str(&body_json.to_string())));
        let mut proxy = Request::new_with_init(&url, &init)?;
        proxy.headers_mut()?.set("content-type", "application/json")?;
        if let Ok(h) = req.headers().get("authorization") {
            if let Some(auth) = h {
                proxy.headers_mut()?.set("authorization", &auth)?;
            }
        }
        let mut resp = Fetch::Request(proxy).send().await?;
        let status = resp.status_code();
        let text = resp.text().await.unwrap_or_default();
        let data: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::json!({ "error": text }));
        let mut out = Response::from_json(&ApiResponse { ok: status >= 200 && status < 300, data: Some(data), error: None })?;
        out = out.with_status(status);
        apply_cors(&mut out)?;
        return Ok(out);
    }
    json_error("SPARK_API_URL not set; pay requires Spark API backend", 501)
}

async fn handle_openclaw_invoice_get(_: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DB")?;
    let stmt = db
        .prepare(
            "SELECT payment_request, amount_sats, description, expires_at, expires_at_ms, created_at, updated_at FROM openclaw_invoices WHERE key = ?1 LIMIT 1",
        )
        .bind(&[JsValue::from_str("current")])?;
    let row = stmt.first::<OpenclawInvoiceRow>(None).await?;
    let Some(row) = row else {
        return json_error("invoice not found", 404);
    };
    let now = now_ms();
    if row.expires_at_ms <= now {
        return json_error("invoice expired", 410);
    }

    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "payment_request": row.payment_request,
            "amount_sats": row.amount_sats,
            "description": row.description,
            "expires_at": row.expires_at,
            "expires_at_ms": row.expires_at_ms,
            "created_at": row.created_at,
            "updated_at": row.updated_at
        })),
        error: None,
    })?;
    response.headers_mut().set("cache-control", "no-store")?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_openclaw_invoice_post(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let token = match openclaw_invoice_token(&ctx.env) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return json_error("OPENCLAW_INVOICE_TOKEN not configured", 500),
    };
    let provided = openclaw_token_from_request(&req).unwrap_or_default();
    if provided != token {
        return Ok(json_unauthorized("missing or invalid token"));
    }

    let body: OpenclawInvoiceBody = req.json().await.map_err(|_| worker::Error::RustError("invalid body".into()))?;
    let payment_request = body.payment_request.trim();
    if payment_request.is_empty() {
        return json_error("payment_request required", 400);
    }
    if body.amount_sats == 0 {
        return json_error("amount_sats required", 400);
    }
    let expires_at_ms = match parse_epoch_ms(&body.expires_at) {
        Some(ms) => ms,
        None => return json_error("expires_at invalid", 400),
    };
    let expires_at = iso_from_ms(expires_at_ms);
    let created_at_ms = body
        .created_at
        .as_deref()
        .and_then(parse_epoch_ms)
        .unwrap_or_else(now_ms);
    let created_at = iso_from_ms(created_at_ms);
    let updated_at = iso_from_ms(now_ms());

    let db = ctx.d1("DB")?;
    db.prepare("INSERT OR REPLACE INTO openclaw_invoices (key, payment_request, amount_sats, description, expires_at, expires_at_ms, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")
        .bind(&[
            JsValue::from_str("current"),
            JsValue::from_str(payment_request),
            JsValue::from_f64(body.amount_sats as f64),
            serde_wasm_bindgen::to_value(&body.description).unwrap_or(JsValue::NULL),
            JsValue::from_str(&expires_at),
            JsValue::from_f64(expires_at_ms as f64),
            JsValue::from_str(&created_at),
            JsValue::from_str(&updated_at),
        ])?
        .run()
        .await?;

    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "payment_request": payment_request,
            "amount_sats": body.amount_sats,
            "description": body.description,
            "expires_at": expires_at,
            "expires_at_ms": expires_at_ms,
            "created_at": created_at,
            "updated_at": updated_at
        })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_moltbook_root(_: Request, _: RouteContext<()>) -> Result<Response> {
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "proxy": {
                "site": "/moltbook/site/",
                "api": "/moltbook/api/"
            },
            "index": {
                "list": "/moltbook/index",
                "categories": "/moltbook/index/categories",
                "search": "/moltbook/index/search?q=term",
                "docs": "/moltbook/docs/{path}"
            },
            "watch": "/moltbook/watch",
            "developers": {
                "identity_token": "POST /moltbook/api/agents/me/identity-token (bot API key)",
                "verify_identity": "POST /moltbook/api/agents/verify-identity (X-Moltbook-App-Key + body {\"token\":\"...\"})",
                "auth_instructions": "https://www.moltbook.com/auth.md?app=...&endpoint=..."
            }
        })),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_moltbook_router(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let path = ctx.param("path").map(|s| s.as_str()).unwrap_or("");
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return handle_moltbook_root(req, ctx).await;
    }

    if req.method() == Method::Options {
        let mut response = Response::empty()?.with_status(204);
        apply_cors(&mut response)?;
        return Ok(response);
    }

    if trimmed == "index" || trimmed == "index/" || trimmed.starts_with("index/") {
        let tail = trimmed.trim_start_matches("index").trim_start_matches('/');
        return handle_moltbook_index(req, tail).await;
    }

    if trimmed == "docs" || trimmed.starts_with("docs/") {
        let doc_path = trimmed.trim_start_matches("docs").trim_start_matches('/');
        return handle_moltbook_docs(req, doc_path).await;
    }

    if trimmed == "watch" || trimmed.starts_with("watch/") {
        return handle_moltbook_watch(req, &ctx.env).await;
    }

    if trimmed == "api" || trimmed.starts_with("api/") {
        let api_path = trimmed.trim_start_matches("api").trim_start_matches('/');
        return handle_moltbook_api_proxy(req, &ctx.env, api_path).await;
    }

    if trimmed == "site" || trimmed.starts_with("site/") {
        let site_path = trimmed.trim_start_matches("site").trim_start_matches('/');
        return handle_moltbook_site_proxy(req, &ctx.env, site_path).await;
    }

    handle_moltbook_site_proxy(req, &ctx.env, trimmed).await
}

async fn handle_moltbook_api_proxy(mut req: Request, env: &Env, path: &str) -> Result<Response> {
    let (query_pairs, query_key) = split_query_for_api_key(&req)?;
    let query_string = build_query_string(&query_pairs);
    let target_url = join_url(&moltbook_api_base(env), path, &query_string);

    let auth = resolve_auth_header(&req, env, query_key)?;
    let headers = build_proxy_headers(&req, auth.as_deref())?;
    let response = proxy_request(&mut req, &target_url, headers).await?;
    let mut headers = clone_headers(response.headers());
    headers.set("x-oa-proxy", "moltbook-api")?;
    apply_cors_headers(&mut headers)?;
    Ok(response.with_headers(headers))
}

async fn handle_moltbook_site_proxy(mut req: Request, env: &Env, path: &str) -> Result<Response> {
    let query = req.url()?.query().unwrap_or("").to_string();
    let target_url = join_url(&moltbook_site_base(env), path, &query);
    let headers = build_proxy_headers(&req, None)?;
    let response = proxy_request(&mut req, &target_url, headers).await?;
    let headers = clone_headers(response.headers());
    headers.set("x-oa-proxy", "moltbook-site")?;
    Ok(response.with_headers(headers))
}

async fn handle_site_fallback_proxy(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let path = ctx.param("path").map(|s| s.as_str()).unwrap_or("");
    handle_moltbook_site_proxy(req, &ctx.env, path).await
}

async fn handle_moltbook_index(req: Request, tail: &str) -> Result<Response> {
    let tail = tail.split('/').next().unwrap_or("");
    if tail == "categories" {
        let mut response = Response::from_json(&ApiResponse {
            ok: true,
            data: Some(&*DOC_CATEGORIES),
            error: None,
        })?;
        apply_cors(&mut response)?;
        return Ok(response);
    }

    let url = req.url()?;
    let mut query: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in url.query_pairs() {
        query.insert(k.to_string(), v.to_string());
    }

    let mut category = query.get("category").cloned();
    if !tail.is_empty() && tail != "search" {
        category = Some(tail.to_string());
    }

    let q = query.get("q").cloned().unwrap_or_default();
    let limit = query
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(INDEX_LIMIT_DEFAULT)
        .min(INDEX_LIMIT_MAX);
    let offset = query
        .get("offset")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    let mut entries = DOC_INDEX
        .iter()
        .filter(|entry| {
            let matches_category = category
                .as_ref()
                .map(|c| entry.category.eq_ignore_ascii_case(c))
                .unwrap_or(true);
            let matches_query = if q.trim().is_empty() {
                true
            } else {
                let haystack = format!(
                    "{} {} {}",
                    entry.path,
                    entry.title.as_deref().unwrap_or(""),
                    entry.summary.as_deref().unwrap_or("")
                )
                .to_lowercase();
                haystack.contains(&q.to_lowercase())
            };
            matches_category && matches_query
        })
        .cloned()
        .collect::<Vec<_>>();

    let total = DOC_INDEX.len();
    let matched = entries.len();
    if offset < entries.len() {
        entries = entries.into_iter().skip(offset).take(limit).collect();
    } else {
        entries.clear();
    }

    let payload = IndexPayload {
        entries,
        total,
        matched,
        categories: DOC_CATEGORIES.clone(),
    };
    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(payload),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

async fn handle_moltbook_docs(req: Request, doc_path: &str) -> Result<Response> {
    if doc_path.is_empty() {
        return json_error("missing doc path", 400);
    }
    let normalized = doc_path.trim_start_matches('/');
    let Some(file) = MOLTBOOK_DOCS.get_file(normalized) else {
        return json_error("doc not found", 404);
    };

    let content_type = content_type_for_path(normalized).to_string();
    if wants_json(&req) {
        let content = String::from_utf8_lossy(file.contents()).to_string();
        let entry = DOC_INDEX
            .iter()
            .find(|item| item.path == normalized)
            .cloned();
        let payload = DocPayload {
            path: normalized.to_string(),
            content,
            content_type,
            bytes: file.contents().len(),
            entry,
        };
        let mut response = Response::from_json(&ApiResponse {
            ok: true,
            data: Some(payload),
            error: None,
        })?;
        apply_cors(&mut response)?;
        Ok(response)
    } else {
        let mut response = Response::from_bytes(file.contents().to_vec())?;
        response.headers_mut().set("content-type", &content_type)?;
        apply_cors(&mut response)?;
        Ok(response)
    }
}

async fn handle_moltbook_watch(req: Request, env: &Env) -> Result<Response> {
    let url = req.url()?;
    let mut query: BTreeMap<String, String> = BTreeMap::new();
    let mut api_key = None;
    for (k, v) in url.query_pairs() {
        let key = k.to_string();
        if key == "api_key" || key == "moltbook_api_key" {
            if api_key.is_none() {
                api_key = Some(v.to_string());
            }
            continue;
        }
        query.insert(key, v.to_string());
    }

    let personal = parse_bool(query.get("personal").map(String::as_str));
    let sort = query
        .get("sort")
        .cloned()
        .unwrap_or_else(|| "new".to_string());
    let limit = query
        .get("limit")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(25);
    let submolt = query.get("submolt").cloned();
    let include_existing = parse_bool(query.get("include_existing").map(String::as_str));

    let seen_param = query.get("seen").cloned().unwrap_or_default();
    let mut seen_set: HashSet<String> = HashSet::new();
    let mut seen_order: VecDeque<String> = VecDeque::new();
    for id in seen_param
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if seen_set.insert(id.to_string()) {
            seen_order.push_back(id.to_string());
        }
    }

    let mut feed_query: Vec<(String, String)> = vec![
        ("sort".to_string(), sort.clone()),
        ("limit".to_string(), limit.to_string()),
    ];
    if !personal {
        if let Some(ref name) = submolt {
            feed_query.push(("submolt".to_string(), name.clone()));
        }
    }

    let auth = resolve_auth_header_from_parts(&req, env, api_key)?;
    let path = if personal { "feed" } else { "posts" };
    let response_json = match fetch_moltbook_json(env, path, feed_query, auth.as_deref()).await {
        Ok(value) => value,
        Err(err) => {
            return json_error(&format!("moltbook watch fetch failed: {err}"), 502);
        }
    };
    let posts = extract_posts(response_json)?;

    let mut new_posts = Vec::new();
    if seen_set.is_empty() && include_existing {
        for post in posts.iter().rev() {
            new_posts.push(post.clone());
        }
    } else {
        for post in posts.iter().rev() {
            if let Some(id) = post.get("id").and_then(|v| v.as_str()) {
                if !seen_set.contains(id) {
                    new_posts.push(post.clone());
                }
            }
        }
    }

    for post in posts {
        if let Some(id) = post.get("id").and_then(|v| v.as_str()) {
            remember_seen(
                &mut seen_set,
                &mut seen_order,
                id.to_string(),
                WATCH_SEEN_CAP,
            );
        }
    }

    let payload = WatchPayload {
        source: if personal {
            "personal".into()
        } else {
            "global".into()
        },
        sort,
        limit,
        submolt,
        total: seen_order.len(),
        new_posts,
        seen: seen_order.into_iter().collect(),
    };

    let mut response = Response::from_json(&ApiResponse {
        ok: true,
        data: Some(payload),
        error: None,
    })?;
    apply_cors(&mut response)?;
    Ok(response)
}

fn build_doc_index() -> Vec<DocEntry> {
    let mut entries = Vec::new();
    collect_docs(&MOLTBOOK_DOCS, &mut entries);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn collect_docs(dir: &Dir, entries: &mut Vec<DocEntry>) {
    for file in dir.files() {
        let path = file.path().to_string_lossy().to_string();
        let (title, summary) = infer_title_and_summary(&path, file.contents());
        let category = if path.contains('/') {
            path.split('/')
                .next()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "root".to_string())
        } else {
            "root".to_string()
        };
        let extension = Path::new(&path)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        entries.push(DocEntry {
            path,
            category,
            title,
            summary,
            bytes: file.contents().len(),
            extension,
        });
    }
    for sub in dir.dirs() {
        collect_docs(sub, entries);
    }
}

fn summarize_categories(entries: &[DocEntry]) -> Vec<CategorySummary> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.category.clone()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(category, count)| CategorySummary { category, count })
        .collect()
}

fn infer_title_and_summary(path: &str, contents: &[u8]) -> (Option<String>, Option<String>) {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let text = std::str::from_utf8(contents).unwrap_or("");

    let mut title = None;
    let mut summary = None;

    match ext {
        "md" => {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    let head = trimmed.trim_start_matches('#').trim();
                    if !head.is_empty() {
                        title = Some(head.to_string());
                        continue;
                    }
                }
                if !trimmed.is_empty() {
                    summary = Some(trimmed.to_string());
                    break;
                }
            }
        }
        "json" | "jsonl" => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(t) = value.get("title").and_then(|v| v.as_str()) {
                    title = Some(t.to_string());
                }
                if summary.is_none() {
                    if let Some(s) = value.get("content").and_then(|v| v.as_str()) {
                        summary = Some(truncate_summary(s));
                    } else if let Some(s) = value.get("description").and_then(|v| v.as_str()) {
                        summary = Some(truncate_summary(s));
                    }
                }
            }
        }
        _ => {
            summary = first_non_empty_line(text);
        }
    }

    if title.is_none() {
        title = title_from_filename(path);
    }
    if summary.is_none() {
        summary = first_non_empty_line(text);
    }

    (title, summary)
}

fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| truncate_summary(line))
}

fn truncate_summary(text: &str) -> String {
    let trimmed = text.trim();
    let mut out = String::new();
    for ch in trimmed.chars().take(180) {
        out.push(ch);
    }
    out
}

fn title_from_filename(path: &str) -> Option<String> {
    let stem = Path::new(path).file_stem()?.to_string_lossy();
    let cleaned = stem.replace('-', " ").replace('_', " ").trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn content_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
    {
        "md" => "text/markdown; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "jsonl" => "application/x-ndjson; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "yml" | "yaml" => "text/yaml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn wants_html(req: &Request) -> bool {
    req.headers()
        .get("accept")
        .ok()
        .flatten()
        .map(|v| v.contains("text/html"))
        .unwrap_or(false)
}

fn wants_json(req: &Request) -> bool {
    req.headers()
        .get("accept")
        .ok()
        .flatten()
        .map(|v| v.contains("application/json") || v.contains("text/json"))
        .unwrap_or(false)
}

fn parse_bool(value: Option<&str>) -> bool {
    let Some(value) = value else { return false };
    matches!(
        value.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn join_url(base: &str, path: &str, query: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let mut url = if path.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{path}")
    };
    if !query.is_empty() {
        url.push('?');
        url.push_str(query);
    }
    url
}

fn build_query_string(pairs: &[(String, String)]) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        serializer.append_pair(k, v);
    }
    serializer.finish()
}

fn split_query_for_api_key(req: &Request) -> Result<(Vec<(String, String)>, Option<String>)> {
    let url = req.url()?;
    let mut pairs = Vec::new();
    let mut api_key = None;
    for (k, v) in url.query_pairs() {
        let key = k.to_string();
        if key == "api_key" || key == "moltbook_api_key" {
            if api_key.is_none() {
                api_key = Some(v.to_string());
            }
            continue;
        }
        pairs.push((key, v.to_string()));
    }
    Ok((pairs, api_key))
}

fn resolve_auth_header(
    req: &Request,
    env: &Env,
    query_key: Option<String>,
) -> Result<Option<String>> {
    resolve_auth_header_from_parts(req, env, query_key)
}

fn resolve_auth_header_from_parts(
    req: &Request,
    env: &Env,
    query_key: Option<String>,
) -> Result<Option<String>> {
    if let Ok(Some(auth)) = req.headers().get("authorization") {
        let trimmed = auth.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed));
        }
    }

    for header in ["x-moltbook-api-key", "x-oa-moltbook-api-key", "x-api-key"] {
        if let Ok(Some(key)) = req.headers().get(header) {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Ok(Some(format!("Bearer {trimmed}")));
            }
        }
    }

    if let Some(key) = query_key {
        let trimmed = key.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(Some(format!("Bearer {trimmed}")));
        }
    }

    if let Ok(var) = env.var("MOLTBOOK_API_KEY") {
        let key = var.to_string();
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Ok(Some(format!("Bearer {trimmed}")));
        }
    }

    Ok(None)
}

fn build_proxy_headers(req: &Request, auth: Option<&str>) -> Result<Headers> {
    let headers = Headers::new();
    for (name, value) in req.headers().entries() {
        let lower = name.to_lowercase();
        if is_hop_header(&lower) {
            continue;
        }
        if matches!(
            lower.as_str(),
            "x-moltbook-api-key" | "x-oa-moltbook-api-key" | "x-api-key"
        ) {
            continue;
        }
        if lower == "authorization" && auth.is_some() {
            continue;
        }
        if headers.append(&name, &value).is_err() {
            continue;
        }
    }
    if let Some(auth) = auth {
        headers.set("authorization", auth)?;
    }
    Ok(headers)
}

fn is_hop_header(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "host"
            | "content-length"
    )
}

async fn proxy_request(req: &mut Request, target_url: &str, headers: Headers) -> Result<Response> {
    let method = req.method();
    let mut init = RequestInit::new();
    init.with_method(method.clone());
    init.with_headers(headers);

    if method != Method::Get && method != Method::Head {
        let bytes = req.bytes().await?;
        if !bytes.is_empty() {
            let body = js_sys::Uint8Array::from(bytes.as_slice());
            init.with_body(Some(body.into()));
        }
    }

    let outbound = Request::new_with_init(target_url, &init)?;
    Fetch::Request(outbound).send().await
}

async fn fetch_moltbook_json(
    env: &Env,
    path: &str,
    query_pairs: Vec<(String, String)>,
    auth: Option<&str>,
) -> Result<serde_json::Value> {
    let query = build_query_string(&query_pairs);
    let url = join_url(&moltbook_api_base(env), path, &query);
    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    let headers = Headers::new();
    if let Some(auth) = auth {
        headers.set("authorization", auth)?;
    }
    init.with_headers(headers);
    let outbound = Request::new_with_init(&url, &init)?;
    let mut response = Fetch::Request(outbound).send().await?;
    let status = response.status_code();
    if !(200..=299).contains(&status) {
        let body = response.text().await.unwrap_or_default();
        return Err(worker::Error::RustError(format!(
            "moltbook api error {status}: {body}"
        )));
    }
    response.json().await
}

fn extract_posts(value: serde_json::Value) -> Result<Vec<serde_json::Value>> {
    if let Some(array) = value.as_array() {
        return Ok(array.clone());
    }

    if let Some(object) = value.as_object() {
        for key in ["posts", "data", "recentPosts", "recent_posts"] {
            if let Some(val) = object.get(key) {
                if let Some(array) = val.as_array() {
                    return Ok(array.clone());
                }
            }
        }
    }

    Err(worker::Error::RustError(
        "unexpected feed response shape".into(),
    ))
}

fn remember_seen(seen: &mut HashSet<String>, order: &mut VecDeque<String>, id: String, cap: usize) {
    if seen.insert(id.clone()) {
        order.push_back(id);
    }
    while order.len() > cap {
        if let Some(old) = order.pop_front() {
            seen.remove(&old);
        }
    }
}

fn clone_headers(headers: &Headers) -> Headers {
    let cloned = Headers::new();
    for (name, value) in headers.entries() {
        let _ = cloned.append(&name, &value);
    }
    cloned
}

fn apply_cors(response: &mut Response) -> Result<()> {
    apply_cors_headers(response.headers_mut())
}

fn apply_cors_headers(headers: &mut Headers) -> Result<()> {
    headers.set("access-control-allow-origin", "*")?;
    headers.set(
        "access-control-allow-methods",
        "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    )?;
    headers.set(
        "access-control-allow-headers",
        "authorization, content-type, x-moltbook-api-key, x-oa-moltbook-api-key, x-api-key",
    )?;
    headers.set(
        "access-control-expose-headers",
        "content-type, content-length, x-oa-proxy",
    )?;
    headers.set("access-control-max-age", "86400")?;
    Ok(())
}

fn json_error(message: &str, status: u16) -> Result<Response> {
    let mut response = Response::from_json(&ApiResponse::<serde_json::Value> {
        ok: false,
        data: None,
        error: Some(message.to_string()),
    })?
    .with_status(status);
    apply_cors(&mut response)?;
    Ok(response)
}

/// 429 rate limit response with retry_after_minutes (Moltbook parity).
fn rate_limit_429(retry_after_minutes: i64) -> Result<Response> {
    let mut response = Response::from_json(&serde_json::json!({
        "success": false,
        "error": "Rate limit exceeded",
        "retry_after_minutes": retry_after_minutes
    }))?
    .with_status(429);
    apply_cors(&mut response)?;
    Ok(response)
}

/// Returns a 401 Response for auth failures (caller returns Ok(this)).
fn json_unauthorized(message: &str) -> Response {
    let mut response = Response::from_json(&ApiResponse::<serde_json::Value> {
        ok: false,
        data: None,
        error: Some(message.to_string()),
    })
    .expect("401 json")
    .with_status(401);
    let _ = apply_cors(&mut response);
    response
}

fn moltbook_site_base(env: &Env) -> String {
    env.var("MOLTBOOK_SITE_BASE")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| MOLTBOOK_SITE_DEFAULT.to_string())
}

fn moltbook_api_base(env: &Env) -> String {
    env.var("MOLTBOOK_API_BASE")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| MOLTBOOK_API_DEFAULT.to_string())
}
