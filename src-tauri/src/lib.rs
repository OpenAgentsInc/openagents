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
    ToolDelta { call_id: String, chunk: String },
    OutputItemDoneMessage { text: String },
    Completed { response_id: Option<String>, token_usage: Option<TokenUsageLite> },
    Raw { json: String },
    SystemNote { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsageLite {
    pub input: u64,
    pub output: u64,
    pub total: u64,
}

#[tauri::command]
async fn submit_chat(window: tauri::Window, prompt: String) -> Result<(), String> {
    // Immediate feedback in UI
    let _ = window.emit("codex:stream", &UiStreamEvent::Created);
    let _ = window.emit(
        "codex:stream",
        &UiStreamEvent::SystemNote { text: format!("Submitting prompt ({} chars) …", prompt.len()) },
    );

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
            let _ = window.emit(
                "codex:stream",
                &UiStreamEvent::SystemNote { text: "Starting Codex (proto) — first build may take a minute…".into() },
            );
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
            cmd.arg("run").arg("-q").arg("-p").arg("codex-cli").arg("--").arg("proto")
                .current_dir(&codex_dir);
        } else {
            cmd = Command::new("codex");
            cmd.arg("proto");
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

fn handle_proto_event(win: &tauri::Window, event: &serde_json::Value) {
    if let Some(msg) = event.get("msg") {
        let typ = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match typ {
            "agent_message_delta" => {
                if let Some(delta) = msg.get("delta").and_then(|d| d.as_str()) {
                    let _ = win.emit("codex:stream", &UiStreamEvent::OutputTextDelta { text: delta.to_string() });
                }
            }
            "exec_command_output_delta" => {
                let call_id = msg.get("call_id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                if let Some(chunk_b64) = msg.get("chunk").and_then(|d| d.as_str()) {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(chunk_b64) {
                        if let Ok(text) = String::from_utf8(bytes) {
                            let _ = win.emit("codex:stream", &UiStreamEvent::ToolDelta { call_id, chunk: text });
                        }
                    }
                }
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
        .invoke_handler(tauri::generate_handler![greet, get_auth_status, get_full_status, submit_chat])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
