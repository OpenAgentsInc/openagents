// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_bridge_token() -> Option<String> {
    use std::path::PathBuf;
    // Prefer OPENAGENTS_HOME if set (points to ~/.openagents). Otherwise derive from HOME/USERPROFILE.
    let base = if let Ok(home) = std::env::var("OPENAGENTS_HOME") {
        PathBuf::from(home)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".openagents")
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        PathBuf::from(profile).join(".openagents")
    } else {
        return None;
    };
    let p = base.join("bridge.json");
    let data = std::fs::read_to_string(&p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&data).ok()?;
    v.get("token").and_then(|x| x.as_str()).map(|s| s.to_string())
}

use std::sync::Arc;
use tokio::sync::Mutex;

struct BridgeState {
    child: Mutex<Option<tokio::process::Child>>,
    bind: Mutex<Option<String>>,                 // e.g., 127.0.0.1:8787
    logs: Arc<Mutex<Vec<String>>>,               // ring buffer (last N)
}

impl Default for BridgeState {
    fn default() -> Self {
        Self { child: Mutex::new(None), bind: Mutex::new(None), logs: Arc::new(Mutex::new(Vec::new())) }
    }
}

fn ensure_bridge_token() -> Option<String> {
    get_bridge_token().or_else(|| {
        // Generate one via oa-bridge rules? For now return None; oa-bridge will generate if missing
        None
    })
}

async fn first_free_port() -> u16 {
    for p in 8787u16..=8798u16 {
        if tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, p)).await.is_ok() {
            return p;
        }
    }
    8787
}

#[tauri::command]
async fn bridge_start(state: tauri::State<'_, Arc<BridgeState>>, bind: Option<String>, token: Option<String>) -> Result<String, String> {
    {
        let child = state.child.lock().await;
        if child.is_some() { return Ok(state.bind.lock().await.clone().unwrap_or_default()); }
    }
    let port = if let Some(b) = &bind {
        b.split(':').last().and_then(|s| s.parse::<u16>().ok()).unwrap_or(first_free_port().await)
    } else {
        first_free_port().await
    };
    let host = format!("127.0.0.1:{}", port);
    let mut cmd = tokio::process::Command::new("oa-bridge");
    cmd.arg("--bind").arg(host.clone());
    // Pass token via env if provided
    let tok = token.or_else(ensure_bridge_token);
    if let Some(t) = &tok { cmd.env("OPENAGENTS_BRIDGE_TOKEN", t); }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("failed to spawn oa-bridge: {e}"))?;
    // Pipe logs
    if let Some(mut out) = child.stdout.take() {
        let logs = state.inner().logs.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut reader = tokio::io::BufReader::new(&mut out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut lg = logs.lock().await;
                lg.push(line);
                if lg.len() > 400 { let drain = lg.len() - 400; lg.drain(0..drain); }
            }
        });
    }
    if let Some(mut err) = child.stderr.take() {
        let logs = state.inner().logs.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut reader = tokio::io::BufReader::new(&mut err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut lg = logs.lock().await;
                lg.push(line);
                if lg.len() > 400 { let drain = lg.len() - 400; lg.drain(0..drain); }
            }
        });
    }
    {
        *state.bind.lock().await = Some(host.clone());
        *state.child.lock().await = Some(child);
    }
    Ok(host)
}

#[tauri::command]
async fn bridge_stop(state: tauri::State<'_, Arc<BridgeState>>) -> Result<bool, String> {
    let mut guard = state.child.lock().await;
    if let Some(mut child) = guard.take() {
        let _ = child.kill().await; // ignore errors
        *state.bind.lock().await = None;
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
async fn bridge_status(state: tauri::State<'_, Arc<BridgeState>>) -> Result<serde_json::Value, String> {
    let running = state.child.lock().await.is_some();
    let bind = state.bind.lock().await.clone();
    let logs = state.logs.lock().await.clone();
    Ok(serde_json::json!({
        "running": running,
        "bind": bind,
        "logs": logs.into_iter().rev().take(100).collect::<Vec<_>>()
    }))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            std::env::set_var("WINIT_UNIX_BACKEND", "x11");
            std::env::set_var("GDK_BACKEND", "x11");
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    let state = Arc::new(BridgeState::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![greet, get_bridge_token, bridge_start, bridge_stop, bridge_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
