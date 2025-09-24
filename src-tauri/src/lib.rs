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
// use tokio::io::AsyncBufReadExt as _; // redundant; already imported via TokioBufReader above
use std::collections::HashMap;
use tokio::sync::oneshot;
use std::ffi::OsStr;
use std::io::Write as _;
mod tasks;
use tasks::*;

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
    SessionConfigured { session_id: String, rollout_path: Option<String> },
    TaskUpdate { task_id: String, status: String, message: Option<String> },
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
    Instructions { ikind: String, text: String },
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
    // (no-op) leftover variable removed
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
    // Fallback: try assistant text
    for v in head.iter() {
        let (t, p) = if let Some(t) = v.get("type").and_then(|s| s.as_str()) {
            (t, v.get("payload"))
        } else if let Some(item) = v.get("item") { (item.get("type").and_then(|s| s.as_str()).unwrap_or(""), item.get("payload")) } else { ("", None) };
        if t == "event_msg" {
            if let Some(p) = p { if p.get("type").and_then(|s| s.as_str()) == Some("agent_message") {
                if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) { return Some(msg.to_string()); }
            } }
        } else if t == "response_item" {
            if let Some(p) = p { if p.get("type").and_then(|s| s.as_str()) == Some("message") {
                if p.get("role").and_then(|s| s.as_str()) == Some("assistant") {
                    if let Some(arr) = p.get("content").and_then(|c| c.as_array()) {
                        for c in arr { if let Some(txt) = c.get("text").and_then(|s| s.as_str()) { return Some(txt.to_string()); } }
                    }
                }
            } }
        }
    }
    None
}

fn text_has_instruction_wrappers(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("<user_instructions>") || lower.contains("</user_instructions>") ||
    lower.contains("<environment_context>") || lower.contains("</environment_context>")
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
    // Drop markdown heading markers
    if t.starts_with('#') {
        t = t.trim_start_matches('#').trim().to_string();
    }
    if t.len() > 80 { t.truncate(80); }
    t
}

fn is_bad_title_candidate(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.starts_with("#") || lower.starts_with("repository guidelines") || lower.starts_with("<cwd>") || lower.is_empty()
}

fn strip_code_fences(s: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in s.lines() {
        let l = line.trim();
        if l.starts_with("```") { in_fence = !in_fence; continue; }
        if !in_fence { out.push_str(line); out.push('\n'); }
    }
    if out.ends_with('\n') { out.pop(); }
    out
}

fn first_sentence(s: &str) -> String {
    let mut end = s.find(['.', '!', '?']).unwrap_or_else(|| s.find('\n').unwrap_or(s.len()));
    if end < s.len() && matches!(s.as_bytes()[end], b'.' | b'!' | b'?') { end += 1; }
    s[..end].trim().to_string()
}

fn limit_words(s: &str, max_words: usize) -> String {
    let mut out = String::new();
    for (i, w) in s.split_whitespace().enumerate() {
        if i >= max_words { break; }
        if !out.is_empty() { out.push(' '); }
        out.push_str(w);
    }
    out
}

fn looks_like_filepath(tok: &str) -> bool {
    let has_slash = tok.contains('/');
    let has_dot = tok.split('/').last().map(|t| t.contains('.')).unwrap_or(false);
    has_slash || has_dot
}

fn extract_filename_hint(s: &str) -> Option<String> {
    for tok in s.split_whitespace() {
        let t = tok.trim_matches(|c: char| c == ',' || c == ':' || c == ';' || c == ')' || c == '(' || c == '`' || c == '"');
        if looks_like_filepath(t) {
            let name = std::path::Path::new(t).file_name().and_then(OsStr::to_str).unwrap_or(t);
            if !name.is_empty() { return Some(name.to_string()); }
        }
    }
    None
}

fn simple_title_case(s: &str) -> String {
    let mut out = String::new();
    for (i, w) in s.split_whitespace().enumerate() {
        let mut chars = w.chars();
        let cw = match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        };
        if i > 0 { out.push(' '); }
        out.push_str(&cw);
    }
    out
}

fn smart_title_from_texts(user: &str, _assistant: Option<&str>) -> String {
    let mut base = strip_code_fences(user);
    base = sanitize_title(base);
    if base.len() > 240 { base = first_sentence(&base); }
    let lower = base.to_lowercase();
    let imperative_starts = [
        "fix ", "add ", "create ", "update ", "remove ", "refactor ", "implement ", "investigate ",
        "optimize ", "improve ", "build ", "write ", "document ", "explain ",
    ];
    let question_starts = ["how ", "how to ", "what ", "why ", "can ", "does ", "is ", "are "];

    let mut title = if imperative_starts.iter().any(|p| lower.starts_with(p)) {
        simple_title_case(&limit_words(&base, 10))
    } else if question_starts.iter().any(|p| lower.starts_with(p)) {
        let sent = first_sentence(&base);
        limit_words(&sent, 12)
    } else {
        limit_words(&first_sentence(&base), 10)
    };

    if let Some(name) = extract_filename_hint(user) {
        if !title.to_lowercase().contains(&name.to_lowercase()) {
            if title.len() + name.len() + 3 <= 80 {
                title.push_str(" (");
                title.push_str(&name);
                title.push(')');
            }
        }
    }

    if title.ends_with('.') { title.pop(); }
    if title.len() > 80 { title.truncate(80); }
    title
}

fn derive_title_from_head(head: &[serde_json::Value]) -> Option<String> {
    let mut user_text: Option<String> = None;
    let mut assistant_text: Option<String> = None;
    for v in head.iter() {
        let (t, p) = if let Some(t) = v.get("type").and_then(|s| s.as_str()) {
            (t, v.get("payload"))
        } else if let Some(item) = v.get("item") {
            (item.get("type").and_then(|s| s.as_str()).unwrap_or(""), item.get("payload"))
        } else { ("", None) };

        if user_text.is_none() {
            if t == "event_msg" {
                if let Some(p) = p {
                    if p.get("type").and_then(|s| s.as_str()) == Some("user_message") {
                        let is_instruction = p.get("payload").and_then(|x| x.get("kind")).and_then(|k| k.as_str()).map(|k| k=="user_instructions" || k=="environment_context").unwrap_or(false);
                        if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) {
                            if !is_instruction { user_text = Some(msg.to_string()); }
                        }
                    }
                }
            } else if t == "response_item" {
                if let Some(p) = p {
                    if p.get("type").and_then(|s| s.as_str()) == Some("message") {
                        let role = p.get("role").and_then(|s| s.as_str()).unwrap_or("");
                        if role == "user" {
                            let text = content_vec_to_text(p.get("content").unwrap_or(&serde_json::Value::Null));
                            if !text_has_instruction_wrappers(&text) && !text.trim().is_empty() {
                                user_text = Some(text);
                            }
                        }
                    }
                }
            }
        }

        if assistant_text.is_none() {
            if t == "response_item" {
                if let Some(p) = p {
                    if p.get("type").and_then(|s| s.as_str()) == Some("message") {
                        let role = p.get("role").and_then(|s| s.as_str()).unwrap_or("");
                        if role == "assistant" {
                            let text = content_vec_to_text(p.get("content").unwrap_or(&serde_json::Value::Null));
                            if !text.trim().is_empty() { assistant_text = Some(text); }
                        }
                    }
                }
            } else if t == "event_msg" {
                if let Some(p) = p {
                    if p.get("type").and_then(|s| s.as_str()) == Some("agent_message") {
                        if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) {
                            if !msg.trim().is_empty() { assistant_text = Some(msg.to_string()); }
                        }
                    }
                }
            }
        }
        if user_text.is_some() && assistant_text.is_some() { break; }
    }

    if let Some(user) = user_text {
        Some(smart_title_from_texts(&user, assistant_text.as_deref()))
    } else {
        None
    }
}

fn sidecar_title_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_string_lossy().to_string();
    s.push_str(".title");
    std::path::PathBuf::from(s)
}

fn read_sidecar_title(path: &std::path::Path) -> Option<String> {
    let p = sidecar_title_path(path);
    match std::fs::read_to_string(&p) { Ok(s) => Some(s.trim().to_string()).filter(|t| !t.is_empty()), Err(_) => None }
}

fn write_sidecar_title(path: &std::path::Path, title: &str) -> anyhow::Result<()> {
    let p = sidecar_title_path(path);
    let mut f = std::fs::File::create(p)?;
    f.write_all(title.as_bytes())?;
    Ok(())
}

#[tauri::command]
async fn list_recent_chats(window: tauri::Window, limit: Option<usize>) -> Result<Vec<UiChatSummary>, String> {
    let limit = limit.unwrap_or(20);
    let files = collect_rollout_files(2000).await.map_err(|e| e.to_string())?;
    let mut out: Vec<UiChatSummary> = Vec::new();
    let mut candidates: Vec<(usize, std::path::PathBuf)> = Vec::new();
    let mut raw_titles: Vec<String> = Vec::new();
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
                                if !text.trim().is_empty() {
                                    has_message = true;
                                    if role == "user" && title.is_none() && !text_has_instruction_wrappers(&text) {
                                        let cand = smart_title_from_texts(&text, None);
                                        if !is_bad_title_candidate(&cand) { title = Some(cand); }
                                    }
                                }
                            }
                        }
                    } else if t == "event_msg" {
                        if let Some(p) = p {
                            match p.get("type").and_then(|s| s.as_str()).unwrap_or("") {
                                "user_message" => {
                                    if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) {
                                        let is_instruction = p.get("payload").and_then(|x| x.get("kind")).and_then(|k| k.as_str()).map(|k| k=="user_instructions" || k=="environment_context").unwrap_or(false);
                                        if !msg.trim().is_empty() {
                                            has_message = true;
                                            if title.is_none() && !is_instruction {
                                                let cand = smart_title_from_texts(msg, None);
                                                if !is_bad_title_candidate(&cand) { title = Some(cand); }
                                            }
                                        }
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
                    if title.is_none() { title = derive_title_from_head(&[v]); }
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
        let mut title = read_sidecar_title(&f).or(title).unwrap_or_else(|| "(no title)".into());
        if title.is_empty() { title = f.file_name().and_then(|s| s.to_str()).unwrap_or("(untitled)").to_string(); }
        title = sanitize_title(title);
        let idx = out.len();
        candidates.push((idx, f.clone()));
        raw_titles.push(title.clone());
        out.push(UiChatSummary { id, path: f.display().to_string(), started_at, title, cwd });
        if out.len() >= limit { break; }
    }
    // Summarize via existing proto (off-record). If summarization fails, keep current titles.
    if !raw_titles.is_empty() {
        if let Ok(summaries) = summarize_titles_via_proto(&window, raw_titles.clone()).await {
            for (i, (idx, path)) in candidates.iter().enumerate() {
                if let Some(sum) = summaries.get(i) {
                    if let Some(item) = out.get_mut(*idx) { item.title = sum.clone(); }
                    // Persist sidecar for future loads
                    let _ = write_sidecar_title(path, summaries[i].as_str());
                }
            }
        }
    }
    Ok(out)
}

fn content_vec_to_text(arr: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(a) = arr.as_array() {
        for it in a {
            if let Some(t) = it.get("text").and_then(|v| v.as_str()) {
                if !out.is_empty() { out.push('\n'); }
                out.push_str(t);
            }
        }
    }
    out
}

#[tauri::command]
async fn generate_titles_for_all(force: Option<bool>) -> Result<usize, String> {
    let force = force.unwrap_or(false);
    let files = collect_rollout_files(5000).await.map_err(|e| e.to_string())?;
    let mut updated = 0usize;
    for f in files {
        if !force {
            if let Some(existing) = read_sidecar_title(&f) {
                if !existing.trim().is_empty() { continue; }
            }
        }
        let file = match tokio_fs::File::open(&f).await { Ok(x) => x, Err(_) => continue };
        let mut reader = tokio::io::BufReader::new(file).lines();
        let mut head: Vec<serde_json::Value> = Vec::new();
        let mut count = 0usize;
        while count < 150 {
            match reader.next_line().await { Ok(Some(line)) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) { head.push(v); }
                count += 1;
            } Ok(None) | Err(_) => break }
        }
        if let Some(mut title) = derive_title_from_head(&head).or_else(|| parse_summary_title_from_head(&head)) {
            title = sanitize_title(title);
            if title.is_empty() { continue; }
            if let Err(e) = write_sidecar_title(&f, &title) { eprintln!("write title sidecar failed: {}", e); continue; }
            updated += 1;
        }
    }
    Ok(updated)
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
                        if trimmed.is_empty() { /* skip empty */ }
                        else if role == "user" {
                            if text_has_instruction_wrappers(&text) { out.push(UiDisplayItem::Instructions { ikind: "user_instructions".to_string(), text }); }
                            else { out.push(UiDisplayItem::User { text }); }
                        }
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
                        if let Some(msg) = p.get("payload").and_then(|x| x.get("message")).and_then(|s| s.as_str()) {
                            let kind = p.get("payload").and_then(|x| x.get("kind")).and_then(|k| k.as_str()).unwrap_or("").to_string();
                            if !msg.trim().is_empty() {
                                if kind == "user_instructions" || kind == "environment_context" { out.push(UiDisplayItem::Instructions { ikind: kind, text: msg.to_string() }); }
                                else { out.push(UiDisplayItem::User { text: msg.to_string() }); }
                            }
                        }
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

#[tauri::command]
async fn new_chat_session(window: tauri::Window) -> Result<(), String> {
    let state = window.state::<Arc<Mutex<McpState>>>();
    let shared_arc = { state.inner().clone() };
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        if let Some(mut child) = guard.child.take() {
            let _ = child.kill();
        }
        // Restart proto which emits session_configured and creates new rollout file
        guard.start(&window, shared_arc.clone()).map_err(|e| format!("start proto: {e}"))?;
    }
    Ok(())
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
    config_reasoning_effort: String,
    summarize_wait: HashMap<String, oneshot::Sender<String>>,
    summarize_buf: HashMap<String, String>,
}

impl Default for McpState {
    fn default() -> Self {
        Self { child: None, stdout: None, next_id: AtomicU64::new(1), conversation_id: None, tx: None, config_reasoning_effort: "high".to_string(), summarize_wait: HashMap::new(), summarize_buf: HashMap::new() }
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
                .arg("-c").arg(format!("model_reasoning_effort={}", self.config_reasoning_effort))
                .current_dir(&codex_dir);
        } else {
            cmd = Command::new("codex");
            cmd.arg("proto")
                .arg("-c").arg("approval_policy=never")
                .arg("-c").arg("sandbox_mode=danger-full-access")
                .arg("-c").arg("model=gpt-5")
                .arg("-c").arg(format!("model_reasoning_effort={}", self.config_reasoning_effort));
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
            // no-op
            let shared_state = shared.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.trim().is_empty() { continue; }
                    let v: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
                    let id_str = v.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                    if !id_str.is_empty() {
                        // Check if this belongs to an off-record summarize request
                        if let Ok(mut guard) = shared_state.lock() {
                            if guard.summarize_wait.contains_key(&id_str) {
                                if let Some(msg) = v.get("msg") {
                                    let typ = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                    match typ {
                                        "agent_message_delta" => {
                                            if let Some(delta) = msg.get("delta").and_then(|d| d.as_str()) {
                                                let entry = guard.summarize_buf.entry(id_str.clone()).or_default();
                                                entry.push_str(delta);
                                            }
                                        }
                                        "agent_message" => {
                                            let mut text = msg.get("message").and_then(|d| d.as_str()).unwrap_or("").to_string();
                                            if text.is_empty() { if let Some(buf) = guard.summarize_buf.remove(&id_str) { text = buf; } }
                                            if let Some(tx) = guard.summarize_wait.remove(&id_str) { let _ = tx.send(text); }
                                        }
                                        _ => {}
                                    }
                                }
                                continue; // do not forward to UI
                            }
                        }
                    }
                    // Update conversation_id on session_configured
                    if let Some(msg) = v.get("msg") {
                        if msg.get("type").and_then(|t| t.as_str()) == Some("session_configured") {
                            if let Some(sid) = msg.get("session_id").and_then(|s| s.as_str()) {
                                if let Ok(mut guard) = shared_state.lock() {
                                    guard.conversation_id = Some(sid.to_string());
                                }
                            }
                        }
                    }
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
        if let Some(err) = self.child.as_mut().and_then(|c| c.stderr.take()) {
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

    // removed; not used

    fn send_user_message(&self, prompt: &str) -> anyhow::Result<()> {
        let id = format!("{}", self.next_request_id());
        let submission = serde_json::json!({
            "id": id,
            "op": { "type": "user_input", "items": [ { "type": "text", "text": prompt } ] }
        });
        self.send_json(&submission)
    }

    fn send_offrecord(&mut self, prompt: &str) -> anyhow::Result<oneshot::Receiver<String>> {
        let id = format!("{}", self.next_request_id());
        let (tx, rx) = oneshot::channel();
        self.summarize_wait.insert(id.clone(), tx);
        let submission = serde_json::json!({
            "id": id,
            "op": { "type": "user_input", "items": [ { "type": "text", "text": prompt } ] }
        });
        self.send_json(&submission)?;
        Ok(rx)
    }
}

fn send_json_line(tx: &UnboundedSender<Vec<u8>>, value: &serde_json::Value) -> anyhow::Result<()> {
    let mut buf = serde_json::to_vec(value)?;
    buf.push(b'\n');
    tx.send(buf).map_err(|e| anyhow::anyhow!("send stdin: {e}"))
}

async fn summarize_titles_via_proto(window: &tauri::Window, titles: Vec<String>) -> anyhow::Result<Vec<String>> {
    if titles.is_empty() { return Ok(vec![]); }
    let state = window.state::<Arc<Mutex<McpState>>>();
    // Ensure proto running
    {
        let mut guard = state.lock().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if guard.child.is_none() {
            let shared = state.inner().clone();
            guard.start(window, shared)?;
        }
    }
    // Build prompt
    let mut lines = String::new();
    for t in titles.iter() { lines.push_str("- "); lines.push_str(t); lines.push('\n'); }
    let prompt = format!("Summarize each item below into 3–5 concise words. Return exactly one line per item, no numbering or bullets, no quotes.\n{}", lines);
    // Send off-record
    let rx = {
        let mut guard = state.lock().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        guard.send_offrecord(&prompt)?
    };
    let text = match tokio::time::timeout(std::time::Duration::from_secs(25), rx).await {
        Ok(Ok(s)) => s,
        _ => return Ok(titles),
    };
    let mut out: Vec<String> = text
        .lines()
        .map(|l| l.trim().trim_start_matches(|c: char| c.is_ascii_digit() || c=='-' || c=='.').trim())
        .filter(|l| !l.is_empty())
        .map(|s| {
            let mut s = s.to_string();
            if s.ends_with('.') { s.pop(); }
            s
        })
        .collect();
    if out.len() != titles.len() { out = titles; }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlanArgs { pub id: String, pub goal: String }

fn fallback_plan_from_goal(goal: &str) -> Vec<Subtask> {
    let mut out = Vec::new();
    let sentences: Vec<&str> = goal
        .split(|c| c == '.' || c == '\n' || c == ';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    for (i, s) in sentences.into_iter().take(10).enumerate() {
        out.push(Subtask { id: format!("s{:02}", i + 1), title: s.to_string(), status: SubtaskStatus::Pending, inputs: serde_json::json!({}), session_id: None, rollout_path: None, last_error: None });
    }
    if out.is_empty() {
        out.push(Subtask { id: "s01".into(), title: goal.trim().to_string(), status: SubtaskStatus::Pending, inputs: serde_json::json!({}), session_id: None, rollout_path: None, last_error: None });
    }
    out
}

#[tauri::command]
async fn task_plan_cmd(window: tauri::Window, args: TaskPlanArgs) -> Result<Task, String> {
    // Emit start
    let _ = window.emit("codex:stream", &UiStreamEvent::TaskUpdate { task_id: args.id.clone(), status: "planning".into(), message: Some("Planning subtasks".into()) });
    let mut task = task_get(&args.id).map_err(|e| e.to_string())?;
    // Try off-record planning
    let prompt = format!("Break the following goal into 8–15 atomic steps. Return JSON array of objects: {{id,title,inputs}}.\nGoal:\n{}", args.goal);
    let rx = {
        let state = window.state::<Arc<Mutex<McpState>>>();
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        if guard.child.is_none() {
            let shared = state.inner().clone();
            guard.start(&window, shared).map_err(|e| e.to_string())?;
        }
        guard.send_offrecord(&prompt).map_err(|e| e.to_string())?
    };
    let mut planned: Vec<Subtask> = Vec::new();
    match tokio::time::timeout(std::time::Duration::from_secs(25), rx).await {
        Ok(Ok(text)) => {
            // Try to parse JSON
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(arr) = v.as_array() {
                    for (i, item) in arr.iter().enumerate() {
                        let title = item.get("title").and_then(|s| s.as_str()).unwrap_or("").to_string();
                        let id = item.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()).unwrap_or_else(|| format!("s{:02}", i+1));
                        let inputs = item.get("inputs").cloned().unwrap_or(serde_json::json!({}));
                        if !title.trim().is_empty() {
                            planned.push(Subtask { id, title, status: SubtaskStatus::Pending, inputs, session_id: None, rollout_path: None, last_error: None });
                        }
                    }
                }
            }
            if planned.is_empty() { planned = fallback_plan_from_goal(&args.goal); }
        }
        _ => { planned = fallback_plan_from_goal(&args.goal); }
    }
    task.queue = planned;
    task.status = TaskStatus::Planned;
    let task = task_update(task).map_err(|e| e.to_string())?;
    let _ = window.emit("codex:stream", &UiStreamEvent::TaskUpdate { task_id: task.id.clone(), status: "planned".into(), message: Some(format!("{} subtasks", task.queue.len())) });
    Ok(task)
}

fn normalize_effort(e: &str) -> Option<&'static str> {
    match e.to_lowercase().as_str() {
        // Codex expects: minimal | low | medium | high
        // Treat "none"/"off" as "minimal"
        "none" | "off" => Some("minimal"),
        "low" => Some("low"),
        "medium" | "med" => Some("medium"),
        "high" => Some("high"),
        _ => None,
    }
}

#[tauri::command]
async fn set_reasoning_effort(window: tauri::Window, effort: String) -> Result<(), String> {
    let Some(eff) = normalize_effort(&effort) else { return Err(format!("invalid effort: {effort}")); };
    let state = window.state::<Arc<Mutex<McpState>>>();
    // Clone the Arc for passing into start
    let shared_arc = { state.inner().clone() };
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.config_reasoning_effort = eff.to_string();
        if let Some(mut child) = guard.child.take() { let _ = child.kill(); }
        // Restart with new effort
        guard.start(&window, shared_arc.clone()).map_err(|e| format!("restart proto: {e}"))?;
    }
    Ok(())
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
            "session_configured" => {
                let session_id = msg.get("session_id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                let rollout_path = msg.get("rollout_path").and_then(|p| p.as_str()).map(|s| s.to_string());
                let _ = win.emit("codex:stream", &UiStreamEvent::SessionConfigured { session_id, rollout_path });
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
        .invoke_handler(tauri::generate_handler![greet, get_auth_status, get_full_status, submit_chat, list_recent_chats, load_chat, set_reasoning_effort, generate_titles_for_all, new_chat_session, tasks_list_cmd, task_create_cmd, task_get_cmd, task_update_cmd, task_delete_cmd, task_plan_cmd, task_run_cmd, task_pause_cmd, task_cancel_cmd])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn tasks_list_cmd() -> Result<Vec<TaskMeta>, String> { tasks_list().map_err(|e| e.to_string()) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskArgs { pub name: String, pub approvals: String, pub sandbox: String, pub max_turns: Option<u32>, pub max_tokens: Option<u32>, pub max_minutes: Option<u32> }

#[tauri::command]
async fn task_create_cmd(args: CreateTaskArgs) -> Result<Task, String> {
    task_create(&args.name, AutonomyBudget { approvals: args.approvals, sandbox: args.sandbox, max_turns: args.max_turns, max_tokens: args.max_tokens, max_minutes: args.max_minutes }).map_err(|e| e.to_string())
}

#[tauri::command]
async fn task_get_cmd(id: String) -> Result<Task, String> { task_get(&id).map_err(|e| e.to_string()) }

#[tauri::command]
async fn task_update_cmd(task: Task) -> Result<Task, String> { task_update(task).map_err(|e| e.to_string()) }

#[tauri::command]
async fn task_delete_cmd(id: String) -> Result<(), String> { task_delete(&id).map_err(|e| e.to_string()) }

#[tauri::command]
async fn task_run_cmd(window: tauri::Window, id: String) -> Result<Task, String> {
    let mut task = task_get(&id).map_err(|e| e.to_string())?;
    if let Some(i) = next_pending_index(&task) {
        task = start_subtask(task, i);
        let task = task_update(task).map_err(|e| e.to_string())?;
        let _ = window.emit("codex:stream", &UiStreamEvent::TaskUpdate { task_id: id.clone(), status: "running".into(), message: Some(format!("Starting {}", task.queue[i].title)) });
        let mut task = task;
        task = complete_subtask(task, i);
        let task = task_update(task).map_err(|e| e.to_string())?;
        let _ = window.emit("codex:stream", &UiStreamEvent::TaskUpdate { task_id: id.clone(), status: "advanced".into(), message: Some(format!("Completed {}", task.queue[i].title)) });
        Ok(task)
    } else {
        let _ = window.emit("codex:stream", &UiStreamEvent::TaskUpdate { task_id: id.clone(), status: "completed".into(), message: None });
        task_get(&id).map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn task_pause_cmd(window: tauri::Window, id: String) -> Result<Task, String> {
    let mut task = task_get(&id).map_err(|e| e.to_string())?;
    task.status = TaskStatus::Paused;
    let task = task_update(task).map_err(|e| e.to_string())?;
    let _ = window.emit("codex:stream", &UiStreamEvent::TaskUpdate { task_id: id, status: "paused".into(), message: None });
    Ok(task)
}

#[tauri::command]
async fn task_cancel_cmd(window: tauri::Window, id: String) -> Result<Task, String> {
    let mut task = task_get(&id).map_err(|e| e.to_string())?;
    task.status = TaskStatus::Canceled;
    let task = task_update(task).map_err(|e| e.to_string())?;
    let _ = window.emit("codex:stream", &UiStreamEvent::TaskUpdate { task_id: id, status: "canceled".into(), message: None });
    Ok(task)
}
