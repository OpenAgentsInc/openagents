// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::Serialize;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use base64::Engine as _;
use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs::File;
use std::io::{BufRead, BufReader as StdBufReader};
use tokio::fs as tokio_fs;
use tokio::io::AsyncBufReadExt as _;
use std::collections::HashMap;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiAuthStatus {
    /// "ChatGPT" | "ApiKey" or None if not authenticated
    pub method: Option<String>,
    /// Email parsed from ChatGPT id_token (if present)
    pub email: Option<String>,
    /// Plan type parsed from ChatGPT id_token (e.g., Free, Plus, Pro, ...)
    pub plan: Option<String>,
}

#[derive(Deserialize)]
struct AuthJson {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    #[serde(default)]
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    id_token: String,
}

#[derive(Deserialize)]
struct IdClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<AuthClaims>,
}

#[derive(Deserialize)]
struct AuthClaims {
    #[serde(default)]
    chatgpt_plan_type: Option<serde_json::Value>,
}

fn default_codex_home() -> Option<PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME") {
        if !val.is_empty() {
            return PathBuf::from(val).canonicalize().ok();
        }
    }
    dirs::home_dir().map(|mut h| {
        h.push(".codex");
        h
    })
}

fn parse_plan_type(val: Option<serde_json::Value>) -> Option<String> {
    match val {
        Some(serde_json::Value::String(s)) => Some(s),
        Some(serde_json::Value::Object(map)) => map.get("Known").and_then(|v| v.as_str().map(|s| s.to_string())),
        _ => None,
    }
}

fn parse_id_token_info(id_token: &str) -> (Option<String>, Option<String>) {
    let mut parts = id_token.split('.');
    let (_h, p, _s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => return (None, None),
    };

    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(p)
        .ok();
    let Some(bytes) = decoded else { return (None, None) };
    let claims: IdClaims = match serde_json::from_slice(&bytes) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let email = claims.email;
    let plan = parse_plan_type(claims.auth.and_then(|a| a.chatgpt_plan_type));
    (email, plan)
}

fn read_latest_session_file() -> Option<PathBuf> {
    let home = default_codex_home()?;
    let sessions = home.join("sessions");
    let years = std::fs::read_dir(&sessions).ok()?;
    let mut latest_path: Option<PathBuf> = None;
    let mut latest_mtime = std::time::SystemTime::UNIX_EPOCH;
    for year in years.flatten() {
        let year_path = year.path();
        let months = std::fs::read_dir(&year_path).ok()?;
        for month in months.flatten() {
            let month_path = month.path();
            let days = std::fs::read_dir(&month_path).ok()?;
            for day in days.flatten() {
                let day_path = day.path();
                let files = std::fs::read_dir(&day_path).ok()?;
                for f in files.flatten() {
                    let p = f.path();
                    if p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                        if let Ok(meta) = f.metadata() {
                            if let Ok(mtime) = meta.modified() {
                                if mtime > latest_mtime {
                                    latest_mtime = mtime;
                                    latest_path = Some(p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    latest_path
}

fn parse_between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let s = text.find(start)? + start.len();
    let e = text[s..].find(end)? + s;
    Some(&text[s..e])
}

fn read_session_info() -> Option<(WorkspaceStatus, ClientStatus, TokenUsageStatus)> {
    let path = read_latest_session_file()?;
    let file = File::open(&path).ok()?;
    let reader = StdBufReader::new(file);
    let mut session_id: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut cli_version: Option<String> = None;
    let mut approval_mode: Option<String> = None;
    let mut sandbox_mode: Option<String> = None;

    for (idx, line_res) in reader.lines().enumerate() {
        let line = line_res.ok()?;
        if idx == 0 {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if v.get("type").and_then(|x| x.as_str()) == Some("session_meta") {
                    let payload = v.get("payload").cloned().unwrap_or(serde_json::Value::Null);
                    session_id = payload.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
                    cwd = payload.get("cwd").and_then(|x| x.as_str()).map(|s| s.to_string());
                    cli_version = payload.get("cli_version").and_then(|x| x.as_str()).map(|s| s.to_string());
                }
            }
        } else if approval_mode.is_none() || sandbox_mode.is_none() {
            // Best-effort parse from the environment_context embedded in text.
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if v.get("type").and_then(|x| x.as_str()) == Some("response_item") {
                    if let Some(text) = v
                        .get("payload")
                        .and_then(|p| p.get("content"))
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.get(0))
                        .and_then(|o| o.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        if approval_mode.is_none() {
                            if let Some(val) = parse_between(text, "<approval_policy>", "</approval_policy>") {
                                approval_mode = Some(val.to_string());
                            }
                        }
                        if sandbox_mode.is_none() {
                            if let Some(val) = parse_between(text, "<sandbox_mode>", "</sandbox_mode>") {
                                sandbox_mode = Some(val.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let mut agents_files = Vec::new();
    if let Some(ref p) = cwd {
        let candidate = std::path::Path::new(p).join("AGENTS.md");
        if candidate.exists() { agents_files.push("AGENTS.md".to_string()); }
    }

    let workspace = WorkspaceStatus { path: cwd, approval_mode, sandbox: sandbox_mode, agents_files };
    let client = ClientStatus { cli_version };
    let token = TokenUsageStatus { session_id, input: Some(0), output: Some(0), total: Some(0) };
    Some((workspace, client, token))
}

#[tauri::command]
async fn get_auth_status() -> UiAuthStatus {
    let codex_home = match default_codex_home() {
        Some(p) => p,
        None => return UiAuthStatus::default(),
    };
    let auth_file = codex_home.join("auth.json");
    let contents = match fs::read_to_string(&auth_file) {
        Ok(c) => c,
        Err(_) => return UiAuthStatus::default(),
    };

    let parsed: AuthJson = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return UiAuthStatus::default(),
    };

    if let Some(api_key) = parsed.openai_api_key {
        if !api_key.is_empty() {
            return UiAuthStatus {
                method: Some("ApiKey".to_string()),
                email: None,
                plan: None,
            };
        }
    }

    if let Some(tokens) = parsed.tokens {
        let (email, plan) = parse_id_token_info(&tokens.id_token);
        return UiAuthStatus {
            method: Some("ChatGPT".to_string()),
            email,
            plan,
        };
    }

    UiAuthStatus::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceStatus {
    pub path: Option<String>,
    pub approval_mode: Option<String>,
    pub sandbox: Option<String>,
    pub agents_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStatus {
    pub signed_in_with: Option<String>,
    pub login: Option<String>,
    pub plan: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelStatus {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summaries: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientStatus {
    pub cli_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsageStatus {
    pub session_id: Option<String>,
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageLimitsStatus {
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FullStatus {
    pub workspace: WorkspaceStatus,
    pub account: AccountStatus,
    pub model: ModelStatus,
    pub client: ClientStatus,
    pub token_usage: TokenUsageStatus,
    pub usage_limits: UsageLimitsStatus,
}

fn account_status_from_auth(auth: &UiAuthStatus) -> AccountStatus {
    let signed_in_with = auth.method.clone();
    let login = auth.email.clone();
    let plan = auth.plan.clone();
    AccountStatus { signed_in_with, login, plan }
}

fn model_status_defaults() -> ModelStatus {
    ModelStatus {
        name: Some("gpt-5".to_string()),
        provider: Some("OpenAI".to_string()),
        reasoning_effort: Some("Medium".to_string()),
        reasoning_summaries: Some("Auto".to_string()),
    }
}

// client_status removed; version comes from session meta where available.

#[tauri::command]
async fn get_full_status() -> FullStatus {
    let auth = get_auth_status().await;
    let (workspace, client, token_usage) = read_session_info().unwrap_or_else(|| {
        let cwd = std::env::current_dir().ok().map(|p| p.display().to_string());
        (
            WorkspaceStatus { path: cwd, approval_mode: None, sandbox: None, agents_files: Vec::new() },
            ClientStatus { cli_version: Some("0.0.0".into()) },
            TokenUsageStatus { session_id: None, input: Some(0), output: Some(0), total: Some(0) },
        )
    });

    FullStatus {
        workspace,
        account: account_status_from_auth(&auth),
        model: model_status_defaults(),
        client,
        token_usage,
        usage_limits: UsageLimitsStatus { note: Some("Rate limit data not available yet.".to_string()) },
    }
}

// ---- Streaming UI plumbing (stub events for now) ----

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "kind")]
pub enum UiStreamEvent {
    #[default]
    Created,
    OutputTextDelta { text: String },
    ToolDelta { call_id: String, chunk: String, is_stderr: bool },
    OutputItemDoneMessage { text: String },
    Completed { response_id: Option<String>, token_usage: Option<TokenUsageLite> },
    Raw { json: String },
    SystemNote { text: String },
    ReasoningDelta { text: String },
    ReasoningSummary { text: String },
    ReasoningBreak {},
    ToolBegin { call_id: String, title: String },
    ToolEnd { call_id: String, exit_code: Option<i64> },
}

// ---- Chat listing and loading ----

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiChatSummary {
    pub id: String,
    pub path: String,
    pub started_at: String,
    pub title: String,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "kind")]
pub enum UiDisplayItem {
    #[default]
    Empty,
    User { text: String },
    Assistant { text: String },
    Reasoning { text: String },
    Tool { title: String, text: String },
}

fn file_is_rollout(path: &std::path::Path) -> bool {
    match (path.file_name().and_then(|s| s.to_str()), path.extension().and_then(|s| s.to_str())) {
        (Some(name), Some("jsonl")) => name.starts_with("rollout-"),
        _ => false,
    }
}

async fn collect_rollout_files(limit_scan: usize) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let home = default_codex_home().ok_or_else(|| anyhow::anyhow!("no CODEX_HOME"))?;

    // Search both active and archived sessions
    for sub in ["sessions", "archived_sessions"] {
        let mut root = home.clone();
        root.push(sub);
        if !root.exists() { continue; }

        // Traverse YYYY/MM/DD shallowly
        let mut years = match tokio_fs::read_dir(&root).await { Ok(d) => d, Err(_) => continue };
        let mut year_dirs: Vec<std::path::PathBuf> = Vec::new();
        while let Some(ent) = years.next_entry().await? { if ent.file_type().await?.is_dir() { year_dirs.push(ent.path()) } }
        // Sort desc by name
        year_dirs.sort_by(|a,b| b.file_name().cmp(&a.file_name()));
        'outer:
        for y in year_dirs {
            let mut months = tokio_fs::read_dir(&y).await?;
            let mut month_dirs: Vec<std::path::PathBuf> = Vec::new();
            while let Some(ent) = months.next_entry().await? { if ent.file_type().await?.is_dir() { month_dirs.push(ent.path()) } }
            month_dirs.sort_by(|a,b| b.file_name().cmp(&a.file_name()));
            for m in month_dirs {
                let mut days = tokio_fs::read_dir(&m).await?;
                let mut day_dirs: Vec<std::path::PathBuf> = Vec::new();
                while let Some(ent) = days.next_entry().await? { if ent.file_type().await?.is_dir() { day_dirs.push(ent.path()) } }
                day_dirs.sort_by(|a,b| b.file_name().cmp(&a.file_name()));
                for d in day_dirs {
                    let mut rd = tokio_fs::read_dir(&d).await?;
                    let mut day_files: Vec<std::path::PathBuf> = Vec::new();
                    while let Some(ent) = rd.next_entry().await? {
                        if ent.file_type().await?.is_file() && file_is_rollout(&ent.path()) { day_files.push(ent.path()); }
                    }
                    // Sort by mtime desc
                    day_files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
                    day_files.reverse();
                    for f in day_files { files.push(f); if files.len() >= limit_scan { break 'outer; } }
                }
            }
        }
        if files.len() >= limit_scan { break; }
    }
    let mut year_dirs: Vec<std::path::PathBuf> = Vec::new();
    // Fallback: if nothing found in structured dirs, scan entire CODEX_HOME recursively for any .jsonl
    if files.is_empty() {
        let mut any_jsonl: Vec<std::path::PathBuf> = Vec::new();
        let mut stack: Vec<std::path::PathBuf> = vec![home.clone()];
        while let Some(dir) = stack.pop() {
            let rd = match std::fs::read_dir(&dir) { Ok(d) => d, Err(_) => continue };
            for ent in rd.flatten() {
                let path = ent.path();
                if path.is_dir() {
                    // Skip obvious heavy or hidden dirs under home to keep it snappy
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if name.starts_with('.') { continue; }
                        if name == "node_modules" || name == "target" || name == "dist" { continue; }
                    }
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    any_jsonl.push(path);
                    if any_jsonl.len() >= limit_scan { break; }
                }
            }
            if any_jsonl.len() >= limit_scan { break; }
        }
        // Sort by mtime desc
        any_jsonl.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        any_jsonl.reverse();
        files = any_jsonl;
    }
    Ok(files)
}

fn parse_summary_title_from_head(head: &[serde_json::Value]) -> Option<String> {
    for v in head.iter() {
        // Support both flattened and nested { item: { type, payload } } forms
        let (t, p) = if let Some(t) = v.get("type").and_then(|s| s.as_str()) {
            (t, v.get("payload"))
        } else if let Some(item) = v.get("item") {
            (item.get("type").and_then(|s| s.as_str()).unwrap_or(""), item.get("payload"))
        } else { ("", None) };
        if t == "event_msg" {
            if let Some(p) = p {
                if p.get("type").and_then(|s| s.as_str()) == Some("user_message") {
                    if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) {
                        return Some(msg.to_string());
                    }
                }
            }
        } else if t == "response_item" {
            if let Some(p) = p {
                if p.get("type").and_then(|s| s.as_str()) == Some("message") {
                    let role = p.get("role").and_then(|s| s.as_str()).unwrap_or("");
                    if role == "user" {
                        if let Some(arr) = p.get("content").and_then(|c| c.as_array()) {
                            for c in arr {
                                if let Some(txt) = c.get("text").and_then(|s| s.as_str()) { return Some(txt.to_string()); }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn sanitize_title(mut s: String) -> String {
    // Strip common XML-ish wrappers and trim
    for (open, close) in [
        ("<user_instructions>", "</user_instructions>"),
        ("<environment_context>", "</environment_context>"),
    ] {
        if s.contains(open) && s.contains(close) {
            if let Some(start) = s.find(open) { if let Some(end) = s.find(close) { s = s[start+open.len()..end].to_string(); } }
        }
    }
    let line = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let mut t = line.to_string();
    if t.len() > 80 { t.truncate(80); }
    t
}

#[tauri::command]
async fn list_recent_chats(limit: Option<usize>) -> Result<Vec<UiChatSummary>, String> {
    let limit = limit.unwrap_or(20);
    let files = collect_rollout_files(2000).await.map_err(|e| e.to_string())?;
    let mut out: Vec<UiChatSummary> = Vec::new();
    for f in files.into_iter().take(200) {
        // Read the first ~120 lines for meta + title (accommodate older formats)
        let file = match tokio_fs::File::open(&f).await { Ok(x) => x, Err(_) => continue };
        let mut reader = tokio::io::BufReader::new(file).lines();
        let mut head: Vec<serde_json::Value> = Vec::new();
        let mut meta_id: Option<String> = None;
        let mut started_at: Option<String> = None;
        let mut cwd: Option<String> = None;
        let mut title: Option<String> = None;
        let mut read_count = 0usize;
        let mut has_message = false;
        while read_count < 120 {
            match reader.next_line().await { Ok(Some(line)) => {
                read_count += 1;
                if line.trim().is_empty() { continue; }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    head.push(v.clone());
                    // Support both flattened and nested forms for meta
                    let (t, p) = if let Some(t) = v.get("type").and_then(|s| s.as_str()) {
                        (t, v.get("payload"))
                    } else if let Some(item) = v.get("item") {
                        (item.get("type").and_then(|s| s.as_str()).unwrap_or(""), item.get("payload"))
                    } else { ("", None) };
                    if t == "session_meta" {
                        if let Some(p) = p {
                            if let Some(meta) = p.get("meta") {
                                meta_id = meta.get("id").and_then(|s| s.as_str()).map(|s| s.to_string());
                                started_at = meta.get("timestamp").and_then(|s| s.as_str()).map(|s| s.to_string());
                                cwd = meta.get("cwd").and_then(|s| s.as_str()).map(|s| s.to_string());
                            }
                        }
                    }
                    // Derive title and detect presence of messages
                    if t == "response_item" {
                        if let Some(p) = p {
                            if p.get("type").and_then(|s| s.as_str()) == Some("message") {
                                let role = p.get("role").and_then(|s| s.as_str()).unwrap_or("");
                                let text = content_vec_to_text(p.get("content").unwrap_or(&serde_json::Value::Null));
                                if !text.trim().is_empty() { has_message = true; if role == "user" && title.is_none() { title = Some(text.clone()); } }
                            }
                        }
                    } else if t == "event_msg" {
                        if let Some(p) = p {
                            match p.get("type").and_then(|s| s.as_str()).unwrap_or("") {
                                "user_message" => {
                                    if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) {
                                        if !msg.trim().is_empty() { has_message = true; if title.is_none() { title = Some(msg.to_string()); } }
                                    }
                                }
                                "agent_message" => {
                                    if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) {
                                        if !msg.trim().is_empty() { has_message = true; }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    if title.is_none() { title = parse_summary_title_from_head(&[v]); }
                }
            }
            Ok(None) | Err(_) => break,
        }
        }
        // end while read_count
        // Skip files with no visible messages at all
        if !has_message { continue; }
        let id = meta_id.unwrap_or_else(|| f.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string());
        let started_at = started_at.unwrap_or_else(|| "".into());
        let mut title = title.unwrap_or_else(|| "(no title)".into());
        if title.is_empty() { title = f.file_name().and_then(|s| s.to_str()).unwrap_or("(untitled)").to_string(); }
        title = sanitize_title(title);
        out.push(UiChatSummary { id, path: f.display().to_string(), started_at, title, cwd });
        if out.len() >= limit { break; }
    }
    Ok(out)
}

fn content_vec_to_text(arr: &serde_json::Value) -> String {
    let mut s = String::new();
    if let Some(a) = arr.as_array() {
        for it in a {
            if let Some(t) = it.get("text").and_then(|s| s.as_str()) {
                if !s.is_empty() { if !s.is_empty() { s.push_str(t); } else { s.push_str(t); } }
            }
        }
    }
    s
}

#[tauri::command]
async fn load_chat(path: String) -> Result<Vec<UiDisplayItem>, String> {
    let text = tokio_fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
    let mut out: Vec<UiDisplayItem> = Vec::new();
    let mut func_names: HashMap<String, String> = HashMap::new();
    let mut custom_names: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        if line.trim().is_empty() { continue; }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        // Support flattened and nested shapes
        let (t, p) = if let Some(t) = v.get("type").and_then(|s| s.as_str()) {
            (t, v.get("payload"))
        } else if let Some(item) = v.get("item") {
            (item.get("type").and_then(|s| s.as_str()).unwrap_or(""), item.get("payload"))
        } else { ("", None) };

        if t == "response_item" {
            if let Some(p) = p {
                match p.get("type").and_then(|s| s.as_str()).unwrap_or("") {
                    "message" => {
                        let role = p.get("role").and_then(|s| s.as_str()).unwrap_or("");
                        let text = content_vec_to_text(p.get("content").unwrap_or(&serde_json::Value::Null));
                        let trimmed = text.trim();
                        if trimmed.is_empty() { /* skip empty */ } else if role == "user" { out.push(UiDisplayItem::User { text }); }
                        else if role == "assistant" { out.push(UiDisplayItem::Assistant { text }); }
                    }
                    "reasoning" => {
                        // Show summaries concatenated
                        if let Some(arr) = p.get("summary").and_then(|x| x.as_array()) {
                            let mut s = String::new();
                            for it in arr { if let Some(t) = it.get("text").and_then(|s| s.as_str()) { if !s.is_empty() { s.push('\n'); } s.push_str(t); } }
                            if !s.is_empty() { out.push(UiDisplayItem::Reasoning { text: s }); }
                        }
                    }
                    "function_call" => {
                        let call_id = p.get("call_id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                        let name = p.get("name").and_then(|s| s.as_str()).unwrap_or("").to_string();
                        if !call_id.is_empty() && !name.is_empty() { func_names.insert(call_id, name); }
                    }
                    "function_call_output" => {
                        let call_id = p.get("call_id").and_then(|s| s.as_str()).unwrap_or("");
                        let content = p.get("output").and_then(|o| o.get("content")).and_then(|s| s.as_str()).unwrap_or("");
                        let title = if let Some(name) = func_names.get(call_id) { format!("Function: {}", name) } else if call_id.is_empty() { "Function output".to_string() } else { format!("Function {call_id}") };
                        out.push(UiDisplayItem::Tool { title, text: content.to_string() });
                    }
                    "custom_tool_call" => {
                        let call_id = p.get("call_id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                        let name = p.get("name").and_then(|s| s.as_str()).unwrap_or("").to_string();
                        if !call_id.is_empty() && !name.is_empty() { custom_names.insert(call_id, name); }
                    }
                    "custom_tool_call_output" => {
                        let call_id = p.get("call_id").and_then(|s| s.as_str()).unwrap_or("");
                        let content = p.get("output").and_then(|s| s.as_str()).unwrap_or("");
                        let title = if let Some(name) = custom_names.get(call_id) { format!("Tool: {}", name) } else if call_id.is_empty() { "Tool output".to_string() } else { format!("Tool {call_id}") };
                        out.push(UiDisplayItem::Tool { title, text: content.to_string() });
                    }
                    "local_shell_call" => {
                        let mut title = "Shell".to_string();
                        if let Some(action) = p.get("action") { if let Some(cmd) = action.get("command").and_then(|a| a.as_array()) {
                            let joined = cmd.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
                            if !joined.is_empty() { title = format!("Shell: {}", joined); }
                        } }
                        out.push(UiDisplayItem::Tool { title, text: String::new() });
                    }
                    _ => {}
                }
            }
        } else if t == "event_msg" {
            if let Some(p) = p {
                match p.get("type").and_then(|s| s.as_str()).unwrap_or("") {
                    "user_message" => {
                        if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) { if !msg.trim().is_empty() { out.push(UiDisplayItem::User { text: msg.to_string() }); } }
                    }
                    "agent_message" => {
                        if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) { if !msg.trim().is_empty() { out.push(UiDisplayItem::Assistant { text: msg.to_string() }); } }
                    }
                    "agent_reasoning" => {
                        if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) { if !msg.trim().is_empty() { out.push(UiDisplayItem::Reasoning { text: msg.to_string() }); } }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsageLite {
    pub input: u64,
    pub output: u64,
    pub total: u64,
}

#[tauri::command]
async fn submit_chat(window: tauri::Window, prompt: String) -> Result<(), String> {
    // Ensure Protocol streamer is running; then send the prompt.
    let state = window.state::<Arc<Mutex<McpState>>>();
    // Clone the Arc for passing into the reader task
    let shared_arc = {
        let s = window.state::<Arc<Mutex<McpState>>>();
        s.inner().clone()
    };

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        if guard.child.is_none() {
            // (quietly start proto; stderr is forwarded to raw log if present)
            guard.start(&window, shared_arc.clone()).map_err(|e| format!("start proto: {e}"))?;
        }
        // Protocol path does not require an explicit newConversation
    }

    // Wait briefly for conversationId to come back from MCP server
    for _ in 0..100u32 {
        if let Ok(guard) = state.lock() {
            if guard.conversation_id.is_some() { break; }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.send_user_message(&prompt).map_err(|e| format!("sendUserMessage: {e}"))
    }
}

// Minimal MCP client state
struct McpState {
    child: Option<Child>,
    stdout: Option<ChildStdout>,
    next_id: AtomicU64,
    conversation_id: Option<String>,
    tx: Option<UnboundedSender<Vec<u8>>>,
}

impl Default for McpState {
    fn default() -> Self {
        Self { child: None, stdout: None, next_id: AtomicU64::new(1), conversation_id: None, tx: None }
    }
}

impl McpState {
    fn start(&mut self, window: &tauri::Window, shared: Arc<Mutex<McpState>>) -> anyhow::Result<()> {
        // Prefer running from codex-rs workspace; run Protocol stream (codex proto).
        let cwd = std::env::current_dir()?;
        let codex_dir = cwd.join("codex-rs");
        let mut cmd;
        if codex_dir.join("Cargo.toml").exists() {
            cmd = Command::new("cargo");
            cmd.arg("run").arg("-q").arg("-p").arg("codex-cli").arg("--")
                .arg("proto")
                // Bust out of sandbox: approval never + danger-full-access, and set model/effort
                .arg("-c").arg("approval_policy=never")
                .arg("-c").arg("sandbox_mode=danger-full-access")
                .arg("-c").arg("model=gpt-5")
                .arg("-c").arg("model_reasoning_effort=high")
                .current_dir(&codex_dir);
        } else {
            cmd = Command::new("codex");
            cmd.arg("proto")
                .arg("-c").arg("approval_policy=never")
                .arg("-c").arg("sandbox_mode=danger-full-access")
                .arg("-c").arg("model=gpt-5")
                .arg("-c").arg("model_reasoning_effort=high");
        }
        cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn()?;
        let mut stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
        let stderr = child.stderr.take();
        self.stdout = Some(stdout);
        self.child = Some(child);

        // Spawn writer task owning stdin and receiving lines to write.
        let (tx, mut rx): (UnboundedSender<Vec<u8>>, UnboundedReceiver<Vec<u8>>) = unbounded_channel();
        self.tx = Some(tx);
        tauri::async_runtime::spawn(async move {
            use tokio::io::AsyncWriteExt;
            while let Some(buf) = rx.recv().await {
                let _ = stdin.write_all(&buf).await;
                let _ = stdin.flush().await;
            }
        });

        // Spawn reader loop to translate Protocol stream → UI events
        if let Some(out) = self.stdout.take() {
            let reader = TokioBufReader::new(out);
            let win = window.clone();
            let tx_clone = self.tx.clone();
            let shared_state = shared.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.trim().is_empty() { continue; }
                    let v: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
                    // Forward raw message for visibility
                    let _ = win.emit("codex:stream", &UiStreamEvent::Raw { json: line.clone() });

                    // Protocol stream line -> map to UI events
                    handle_proto_event(&win, &v);
                }
            });
        }
        // Also forward stderr (build logs) to UI raw pane
        if let Some(err) = stderr {
            let win2 = window.clone();
            tauri::async_runtime::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = TokioBufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = win2.emit("codex:stream", &UiStreamEvent::Raw { json: format!("[stderr] {}", line) });
                }
            });
        }
        // Also forward stderr lines (build logs) so first-time builds are visible.
        if let Some(mut err) = self.child.as_mut().and_then(|c| c.stderr.take()) {
            let win2 = window.clone();
            tauri::async_runtime::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = TokioBufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = win2.emit("codex:stream", &UiStreamEvent::Raw { json: format!("[stderr] {}", line) });
                }
            });
        }
        Ok(())
    }

    fn send_json(&self, value: &serde_json::Value) -> anyhow::Result<()> {
        let tx = self.tx.as_ref().ok_or_else(|| anyhow::anyhow!("stdin tx missing"))?;
        send_json_line(tx, value)
    }

    fn next_request_id(&self) -> u64 { self.next_id.fetch_add(1, Ordering::SeqCst) }

    fn new_conversation(&mut self) -> anyhow::Result<()> { Ok(()) }

    fn send_user_message(&self, prompt: &str) -> anyhow::Result<()> {
        let id = format!("{}", self.next_request_id());
        let submission = serde_json::json!({
            "id": id,
            "op": { "type": "user_input", "items": [ { "type": "text", "text": prompt } ] }
        });
        self.send_json(&submission)
    }
}

fn send_json_line(tx: &UnboundedSender<Vec<u8>>, value: &serde_json::Value) -> anyhow::Result<()> {
    let mut buf = serde_json::to_vec(value)?;
    buf.push(b'\n');
    tx.send(buf).map_err(|e| anyhow::anyhow!("send stdin: {e}"))
}

fn guess_filename_from_command(cmd: &str) -> Option<String> {
    // Strip quotes to simplify tokenization
    let cleaned = cmd.replace('\"', " ").replace('\'', " ");
    let mut candidate: Option<String> = None;
    for tok in cleaned.split_whitespace() {
        let t = tok.trim_matches(|c: char| c == ';' || c == ')' || c == '(');
        if t.starts_with('-') || t == "cd" || t == "&&" || t == "sed" || t == "-n" || t == "cat" || t == "head" || t == "tail" {
            continue;
        }
        let looks_like_path = t.contains('/') || t.contains('.') && !t.ends_with('.') && !t.starts_with(".");
        if looks_like_path {
            candidate = Some(t.to_string());
        }
    }
    candidate.map(|s| {
        // Keep only the filename for readability
        std::path::Path::new(&s)
            .file_name()
            .and_then(|os| os.to_str())
            .unwrap_or(&s)
            .to_string()
    })
}

fn handle_proto_event(win: &tauri::Window, event: &serde_json::Value) {
    if let Some(msg) = event.get("msg") {
        let typ = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match typ {
            "agent_message_delta" => {
                if let Some(delta) = msg.get("delta").and_then(|d| d.as_str()) {
                    let _ = win.emit("codex:stream", &UiStreamEvent::OutputTextDelta { text: delta.to_string() });
                }
            }
            "agent_reasoning_delta" => {
                if let Some(delta) = msg.get("delta").and_then(|d| d.as_str()) {
                    let _ = win.emit("codex:stream", &UiStreamEvent::ReasoningDelta { text: delta.to_string() });
                }
            }
            "agent_reasoning_raw_content_delta" => {
                if let Some(delta) = msg.get("delta").and_then(|d| d.as_str()) {
                    let _ = win.emit("codex:stream", &UiStreamEvent::ReasoningDelta { text: delta.to_string() });
                }
            }
            "agent_reasoning" => {
                if let Some(text) = msg.get("text").and_then(|d| d.as_str()) {
                    let _ = win.emit("codex:stream", &UiStreamEvent::ReasoningSummary { text: text.to_string() });
                }
            }
            "agent_reasoning_section_break" => {
                let _ = win.emit("codex:stream", &UiStreamEvent::ReasoningBreak {});
            }
            "exec_command_output_delta" => {
                let call_id = msg.get("call_id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                let is_stderr = matches!(msg.get("stream").and_then(|s| s.as_str()), Some("stderr"));
                if let Some(chunk_b64) = msg.get("chunk").and_then(|d| d.as_str()) {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(chunk_b64) {
                        if let Ok(text) = String::from_utf8(bytes) {
                            let _ = win.emit("codex:stream", &UiStreamEvent::ToolDelta { call_id, chunk: text, is_stderr });
                        }
                    }
                }
            }
            "exec_command_begin" => {
                let call_id = msg.get("call_id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                // Derive a friendly title with details (e.g., Read architecture.md, List docs, Search docs)
                let mut parts: Vec<String> = Vec::new();
                if let Some(arr) = msg.get("parsed_cmd").and_then(|v| v.as_array()) {
                    for el in arr.iter() {
                        if let Some(obj) = el.as_object() {
                            if let Some((k, vsub)) = obj.iter().next() {
                                if k == "Unknown" { continue; }
                                let title_part = match k.as_str() {
                                    "Read" => {
                                        let mut name = vsub.get("name").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                        if name.is_empty() {
                                            let cmd_str = msg
                                                .get("command")
                                                .and_then(|c| c.as_array())
                                                .map(|a| a.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>().join(" "))
                                                .unwrap_or_default();
                                            if let Some(n) = guess_filename_from_command(&cmd_str) { name = n; }
                                        }
                                        if name.is_empty() { "Read".to_string() } else { format!("Read {}", name) }
                                    }
                                    "ListFiles" => {
                                        let path = vsub.get("path").and_then(|s| s.as_str()).unwrap_or("");
                                        if !path.is_empty() { format!("List {}", path) } else { "ListFiles".to_string() }
                                    }
                                    "Search" => {
                                        let path = vsub.get("path").and_then(|s| s.as_str()).unwrap_or("");
                                        if !path.is_empty() { format!("Search {}", path) } else { "Search".to_string() }
                                    }
                                    other => other.to_string(),
                                };
                                parts.push(title_part);
                            }
                        }
                    }
                }
                // Remove duplicates while preserving order
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                parts.retain(|p| seen.insert(p.clone()));

                if parts.is_empty() {
                    // Heuristic from the command string
                    let cmd_str = msg
                        .get("command")
                        .and_then(|c| c.as_array())
                        .map(|a| a.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>().join(" "))
                        .unwrap_or_default();
                    let mut guess = String::new();
                    if cmd_str.contains("rg ") || cmd_str.contains("find ") {
                        guess = "Search".into();
                    } else if cmd_str.contains("ls ") {
                        guess = "ListFiles".into();
                    } else if cmd_str.contains("sed -n") || cmd_str.contains("cat ") || cmd_str.contains("head ") {
                        guess = "Read".into();
                    }
                    if guess.is_empty() { guess = "exec".into(); }
                    parts.push(guess);
                }

                let title = parts.join(" → ");
                let _ = win.emit("codex:stream", &UiStreamEvent::ToolBegin { call_id, title });
            }
            "exec_command_end" => {
                let call_id = msg.get("call_id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                let exit_code = msg.get("exit_code").and_then(|c| c.as_i64());
                let _ = win.emit("codex:stream", &UiStreamEvent::ToolEnd { call_id, exit_code });
            }
            "token_count" => {
                let input = msg.get("usage").and_then(|u| u.get("input_tokens")).and_then(|x| x.as_u64()).unwrap_or(0);
                let output = msg.get("usage").and_then(|u| u.get("output_tokens")).and_then(|x| x.as_u64()).unwrap_or(0);
                let total = input + output;
                let _ = win.emit("codex:stream", &UiStreamEvent::Completed { response_id: None, token_usage: Some(TokenUsageLite { input, output, total }) });
            }
            _ => {}
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Mutex::new(McpState::default())))
        .invoke_handler(tauri::generate_handler![greet, get_auth_status, get_full_status, submit_chat, list_recent_chats, load_chat])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
