// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn get_thread_count(convex_url: Option<String>) -> Result<usize, String> {
    use convex::{FunctionResult, Value};
    use std::collections::BTreeMap;

    let url = convex_url
        .or_else(|| std::env::var("CONVEX_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:3210".to_string());

    let mut client = convex::ConvexClient::new(&url)
        .await
        .map_err(|e| format!("convex connect error: {e}"))?;

    let result = client
        .query("threads:list", BTreeMap::new())
        .await
        .map_err(|e| format!("convex query error: {e}"))?;

    match result {
        FunctionResult::Value(Value::Array(items)) => Ok(items.len()),
        FunctionResult::Value(_) => Ok(0),
        FunctionResult::ErrorMessage(msg) => Err(msg),
        FunctionResult::ConvexError(err) => Err(err.to_string()),
    }
}

#[derive(serde::Serialize)]
struct SimpleStatus { healthy: bool, url: String }

#[tauri::command]
fn get_local_convex_status() -> SimpleStatus {
    let url = "http://127.0.0.1:3210".to_string();
    let healthy = is_port_open(3210);
    SimpleStatus { healthy, url }
}

#[derive(serde::Serialize)]
struct ThreadSummary {
    id: String,
    thread_id: Option<String>,
    title: String,
    updated_at: f64,
}

#[derive(serde::Serialize)]
struct MessageRow {
    id: Option<String>,
    #[serde(rename = "threadId")]
    thread_id: Option<String>,
    role: Option<String>,
    kind: Option<String>,
    text: Option<String>,
    data: Option<serde_json::Value>,
    ts: f64,
}

#[tauri::command]
async fn list_recent_threads(limit: Option<u32>, convex_url: Option<String>) -> Result<Vec<ThreadSummary>, String> {
    use convex::{FunctionResult, Value};
    use std::collections::BTreeMap;

    let url = convex_url
        .or_else(|| std::env::var("CONVEX_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:3210".to_string());

    let mut client = convex::ConvexClient::new(&url)
        .await
        .map_err(|e| format!("convex connect error: {e}"))?;

    let res = client
        .query("threads:list", BTreeMap::new())
        .await
        .map_err(|e| format!("convex query error: {e}"))?;

    let mut rows: Vec<ThreadSummary> = Vec::new();
    match res {
        FunctionResult::Value(Value::Array(items)) => {
            for item in items {
                let json: serde_json::Value = item.into();
                let id = json.get("_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                if id.is_empty() { continue; }
                let title = json.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let updated_at = json.get("updatedAt").and_then(|x| x.as_f64()).unwrap_or(0.0);
                let thread_id = json.get("threadId").and_then(|x| x.as_str()).map(|s| s.to_string());
                rows.push(ThreadSummary { id, thread_id, title, updated_at });
            }
            // Sort by updated_at desc and clamp to limit (default 10)
            rows.sort_by(|a, b| b.updated_at.total_cmp(&a.updated_at));
            let take = limit.unwrap_or(10) as usize;
            if rows.len() > take { rows.truncate(take); }
            Ok(rows)
        }
        FunctionResult::Value(_) => Ok(Vec::new()),
        FunctionResult::ErrorMessage(msg) => Err(msg),
        FunctionResult::ConvexError(err) => Err(err.to_string()),
    }
}

#[tauri::command]
#[allow(non_snake_case)]
async fn list_messages_for_thread(threadId: String, limit: Option<u32>, convex_url: Option<String>) -> Result<Vec<MessageRow>, String> {
    use convex::{FunctionResult, Value};
    use std::collections::BTreeMap;

    let url = convex_url
        .or_else(|| std::env::var("CONVEX_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:3210".to_string());

    let mut client = convex::ConvexClient::new(&url)
        .await
        .map_err(|e| format!("convex connect error: {e}"))?;

    let mut args: BTreeMap<String, Value> = BTreeMap::new();
    args.insert("threadId".into(), Value::from(threadId));
    if let Some(l) = limit { args.insert("limit".into(), Value::from(l as i64)); }

    let res = client
        .query("messages:forThread", args)
        .await
        .map_err(|e| format!("convex query error: {e}"))?;

    match res {
        FunctionResult::Value(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let json: serde_json::Value = item.into();
                // Filter: hide initial user instructions/preface messages
                let text_s = json.get("text").and_then(|x| x.as_str()).unwrap_or("");
                let kind_s = json.get("kind").and_then(|x| x.as_str()).unwrap_or("");
                let role_s = json.get("role").and_then(|x| x.as_str()).unwrap_or("");
                let hide = text_s.trim_start().starts_with("<user_instructions>")
                    || kind_s == "preface" || kind_s == "instructions"
                    || (role_s == "system" && text_s.contains("Repository Guidelines"));
                if hide { continue; }
                let id = json.get("_id").and_then(|x| x.as_str()).map(|s| s.to_string());
                let thread_id = json.get("threadId").and_then(|x| x.as_str()).map(|s| s.to_string());
                let role = json.get("role").and_then(|x| x.as_str()).map(|s| s.to_string());
                let kind = json.get("kind").and_then(|x| x.as_str()).map(|s| s.to_string());
                let text = json.get("text").and_then(|x| x.as_str()).map(|s| s.to_string());
                let data = json.get("data").cloned();
                let ts = json.get("ts").and_then(|x| x.as_f64()).unwrap_or(0.0);
                out.push(MessageRow { id, thread_id, role, kind, text, data, ts });
            }
            Ok(out)
        }
        FunctionResult::Value(_) => Ok(Vec::new()),
        FunctionResult::ErrorMessage(msg) => Err(msg),
        FunctionResult::ConvexError(err) => Err(err.to_string()),
    }
}

#[tauri::command]
#[allow(non_snake_case)]
async fn subscribe_thread_messages(window: tauri::WebviewWindow, threadId: String, limit: Option<u32>, convex_url: Option<String>) -> Result<(), String> {
    use convex::{FunctionResult, Value};
    use futures::StreamExt;
    use std::collections::BTreeMap;

    let url = convex_url
        .or_else(|| std::env::var("CONVEX_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:3210".to_string());

    let mut client = convex::ConvexClient::new(&url)
        .await
        .map_err(|e| format!("convex connect error: {e}"))?;

    let mut args: BTreeMap<String, Value> = BTreeMap::new();
    args.insert("threadId".into(), Value::from(threadId.clone()));
    if let Some(l) = limit { args.insert("limit".into(), Value::from(l as i64)); }

    let mut sub = client
        .subscribe("messages:forThread", args)
        .await
        .map_err(|e| format!("convex subscribe error: {e}"))?;

    tauri::async_runtime::spawn(async move {
        while let Some(result) = sub.next().await {
            if let FunctionResult::Value(Value::Array(items)) = result {
                let mut rows: Vec<MessageRow> = Vec::with_capacity(items.len());
                for item in items {
                    let json: serde_json::Value = item.into();
                    let text_s = json.get("text").and_then(|x| x.as_str()).unwrap_or("");
                    let kind_s = json.get("kind").and_then(|x| x.as_str()).unwrap_or("");
                    let role_s = json.get("role").and_then(|x| x.as_str()).unwrap_or("");
                    let hide = text_s.trim_start().starts_with("<user_instructions>")
                        || kind_s == "preface" || kind_s == "instructions"
                        || (role_s == "system" && text_s.contains("Repository Guidelines"));
                    if hide { continue; }
                    let id = json.get("_id").and_then(|x| x.as_str()).map(|s| s.to_string());
                    let thread_id = json.get("threadId").and_then(|x| x.as_str()).map(|s| s.to_string());
                    let role = json.get("role").and_then(|x| x.as_str()).map(|s| s.to_string());
                    let kind = json.get("kind").and_then(|x| x.as_str()).map(|s| s.to_string());
                    let text = json.get("text").and_then(|x| x.as_str()).map(|s| s.to_string());
                    let data = json.get("data").cloned();
                    let ts = json.get("ts").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    rows.push(MessageRow { id, thread_id, role, kind, text, data, ts });
                }
                let _ = window.emit("convex:messages", &rows);
            }
        }
    });

    Ok(())
}

use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Try to start a local Convex backend (binary sidecar) on 127.0.0.1:3210.
            // This is offline-first and avoids the CLI `convex dev` path.
            if std::env::var("OPENAGENTS_SKIP_EMBEDDED_CONVEX").ok().as_deref() != Some("1") {
                tauri::async_runtime::spawn(async move {
                    start_convex_sidecar();
                });
                // Dev quality-of-life: once the backend is up, push functions in the background
                // so you don't need a separate terminal. Logs are inherited into this console.
                tauri::async_runtime::spawn(async move {
                    deploy_convex_functions_once();
                });
                // Emit a local convex status event to the UI using the sidecar URL (3210)
                // so the dot reflects the embedded backend instead of the bridge's default (7788).
                {
                    use tauri::Manager;
                    let handle = app.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        // probe up to ~10s
                        for _ in 0..100u32 {
                            if is_port_open(3210) {
                                let _ = handle.emit("convex.local_status", serde_json::json!({
                                    "healthy": true,
                                    "url": "http://127.0.0.1:3210"
                                }));
                                return;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                        let _ = handle.emit("convex.local_status", serde_json::json!({
                            "healthy": false,
                            "url": "http://127.0.0.1:3210"
                        }));
                    });
                }
            }
            // Start codex-bridge automatically in the background on app launch.
            tauri::async_runtime::spawn(async move {
                ensure_bridge_running().await;
            });
            // Emit a bridge.ready event as soon as the WS port is open so the UI connects once.
            {
                use tauri::Manager;
                let handle = app.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    for _ in 0..50u32 { // ~5s probe window
                        if is_port_open(8787) {
                            let _ = handle.emit("bridge.ready", ());
                            println!("[tauri/bootstrap] bridge.ready emitted");
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                });
            }
            // Maximize the main window on startup for an almost-fullscreen layout.
            #[allow(unused_must_use)]
            {
                use tauri::Manager as _;
                if let Some(win) = app.get_webview_window("main") {
                    win.maximize();
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, get_thread_count, list_recent_threads, list_messages_for_thread, subscribe_thread_messages, get_local_convex_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn start_convex_sidecar() {
    // Probe 127.0.0.1:3210; if not open, try to spawn a local backend binary.
    if std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 3210)).is_ok() {
        println!("[tauri/convex] detected local backend on 127.0.0.1:3210");
        return;
    }
    let bin_candidates: Vec<std::path::PathBuf> = {
        let mut v = Vec::new();
        if let Ok(p) = std::env::var("OPENAGENTS_CONVEX_BIN") { v.push(std::path::PathBuf::from(p)); }
        // Repo-local dev path
        let repo = detect_repo_root(None);
        v.push(repo.join("tauri").join("src-tauri").join("bin").join(if cfg!(windows) { "local_backend.exe" } else { "local_backend" }));
        // User install path used previously
        if let Ok(home) = std::env::var("HOME") { v.push(std::path::PathBuf::from(home).join(".openagents/bin/local_backend")); }
        v
    };
    // Pick a candidate that exists and looks like a real binary (>1MB)
    let bin = bin_candidates.into_iter().find(|p| {
        if let Ok(meta) = std::fs::metadata(p) {
            meta.is_file() && meta.len() > 1_000_000
        } else { false }
    }).unwrap_or_else(|| std::path::PathBuf::from("local_backend"));
    if !bin.exists() {
        println!("[tauri/convex] local backend binary not found (set OPENAGENTS_CONVEX_BIN or place tauri/src-tauri/bin/local_backend)");
        return;
    }
    let db_path = default_convex_db_path();
    if let Some(parent) = db_path.parent() { let _ = std::fs::create_dir_all(parent); }
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg(&db_path)
        .arg("--db").arg("sqlite")
        .arg("--interface").arg("127.0.0.1")
        .arg("--port").arg("3210")
        .arg("--disable-beacon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    match cmd.spawn() {
        Ok(child) => println!("[tauri/convex] spawned local backend pid={} bin={} db={}", child.id(), bin.display(), db_path.display()),
        Err(e) => println!("[tauri/convex] failed to spawn local backend: {} bin={} db={}", e, bin.display(), db_path.display()),
    }
}

fn default_convex_db_path() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".openagents/convex/data.sqlite3");
    }
    std::path::PathBuf::from("data.sqlite3")
}

fn read_env_local_var(key: &str) -> Option<String> {
    let root = detect_repo_root(None);
    let path = root.join(".env.local");
    if !path.exists() { return None; }
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() { continue; }
            if let Some(idx) = line.find('=') {
                let (k, v) = (&line[..idx], &line[idx+1..]);
                if k.trim() == key { return Some(v.trim().to_string()); }
            }
        }
    }
    None
}

fn deploy_convex_functions_once() {
    // Wait for 3210 to be open
    for _ in 0..200 { // ~20s
        if std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 3210)).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 3210)).is_err() {
        println!("[tauri/convex] backend not reachable on 127.0.0.1:3210; skipping auto-deploy");
        return;
    }
    // Resolve admin key
    let admin = std::env::var("CONVEX_ADMIN_KEY")
        .ok()
        .or_else(|| read_env_local_var("CONVEX_ADMIN_KEY"))
        .or_else(|| read_env_local_var("CONVEX_SELF_HOSTED_ADMIN_KEY"))
        .unwrap_or_else(|| "carnitas|017c5405aba48afe1d1681528424e4528026e69e3b99e400ef23f2f3741a11db225497db09".to_string());
    let root = detect_repo_root(None);
    let mut cmd = std::process::Command::new("bun");
    cmd.args(["run", "convex:dev:once"]) // calls scripts/convex-cli.sh dev:once
        .current_dir(&root)
        .env("CONVEX_URL", "http://127.0.0.1:3210")
        .env("CONVEX_SELF_HOSTED_URL", "http://127.0.0.1:3210")
        .env("CONVEX_ADMIN_KEY", &admin)
        .env("CONVEX_SELF_HOSTED_ADMIN_KEY", &admin)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    match cmd.spawn() {
        Ok(child) => println!("[tauri/convex] deploying functions (dev:once) pid={}", child.id()),
        Err(e) => println!("[tauri/convex] failed to spawn bun convex:dev:once: {}", e),
    }
}

async fn ensure_bridge_running() {
    // If the bridge port is open we assume it's already running
    if is_port_open(8787) { return; }
    let repo = detect_repo_root(None);
    println!("[tauri/bootstrap] repo root: {}", repo.display());
    let bin = repo.join("target").join("debug").join(if cfg!(windows) { "codex-bridge.exe" } else { "codex-bridge" });
    println!("[tauri/bootstrap] bridge bin exists? {} — {}", bin.exists(), bin.display());

    // In dev, prefer `cargo run -p codex-bridge` to ensure the latest code is used.
    // This avoids stale binaries that don't reflect recent changes.
    let mut cmd = {
        let mut c = std::process::Command::new("cargo");
        c.args([
            "run", "-q", "-p", "codex-bridge", "--",
            "--bind", "0.0.0.0:8787",
            // Bridge should not manage Convex; Tauri sidecar handles it on 3210.
            "--manage-convex", "false",
            "--bootstrap", "false",
        ]);
        c
    };
    cmd.current_dir(&repo)
        .env("RUST_LOG", "info")
        // Ensure the bridge does not manage Convex or attempt bootstrap when spawned by the app.
        .env("OPENAGENTS_MANAGE_CONVEX", "false")
        .env("OPENAGENTS_BOOTSTRAP", "false")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    match cmd.spawn() {
        Ok(child) => {
            println!("[tauri/bootstrap] spawned codex-bridge pid={} (stdout/stderr inherited)", child.id());
        }
        Err(e) => {
            println!("[tauri/bootstrap] failed to spawn bridge: {e}");
        }
    }
    // Probe port quickly (<= 2s) so UI can flip fast if ready
    for _ in 0..10 { if is_port_open(8787) { break; } std::thread::sleep(std::time::Duration::from_millis(200)); }
}

fn is_port_open(port: u16) -> bool {
    std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port)).is_ok()
}

fn detect_repo_root(start: Option<std::path::PathBuf>) -> std::path::PathBuf {
    fn is_repo_root(p: &std::path::Path) -> bool {
        p.join("expo").is_dir() && p.join("crates").is_dir()
    }
    let mut cur = start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let original = cur.clone();
    loop {
        if is_repo_root(&cur) { return cur; }
        if !cur.pop() { return original; }
    }
}
