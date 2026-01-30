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
use worker::*;

const MOLTBOOK_SITE_DEFAULT: &str = "https://www.moltbook.com";
const MOLTBOOK_API_DEFAULT: &str = "https://www.moltbook.com/api/v1";
const INDEX_LIMIT_DEFAULT: usize = 100;
const INDEX_LIMIT_MAX: usize = 500;
const WATCH_SEEN_CAP: usize = 2000;

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

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get_async("/", handle_root)
        .get_async("/health", handle_health)
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
            "moltbook_proxy": "/moltbook/site/",
            "moltbook_api": "/moltbook/api/",
            "moltbook_index": "/moltbook/index"
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
            "watch": "/moltbook/watch"
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
