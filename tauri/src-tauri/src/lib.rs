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
use std::io::ErrorKind;

struct BridgeState {
    child: Mutex<Option<tokio::process::Child>>,
    bind: Mutex<Option<String>>,                 // e.g., 127.0.0.1:8787
    logs: Arc<Mutex<Vec<String>>>,               // ring buffer (last N)
    starting: Mutex<bool>,                       // guard to avoid double-spawn races
}

impl Default for BridgeState {
    fn default() -> Self {
        Self { child: Mutex::new(None), bind: Mutex::new(None), logs: Arc::new(Mutex::new(Vec::new())), starting: Mutex::new(false) }
    }
}

fn ensure_bridge_token() -> Option<String> {
    // oa-bridge will generate and persist a token if missing
    get_bridge_token()
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
    // Prevent concurrent spawns (React StrictMode, HMR)
    {
        let mut starting = state.starting.lock().await;
        if *starting {
            // Another start in progress: return current bind or empty; caller can poll status
            return Ok(state.bind.lock().await.clone().unwrap_or_default());
        }
        if state.child.lock().await.is_some() {
            return Ok(state.bind.lock().await.clone().unwrap_or_default());
        }
        *starting = true;
    }

    let base_port = if let Some(b) = &bind {
        b.split(':').last().and_then(|s| s.parse::<u16>().ok()).unwrap_or(first_free_port().await)
    } else { first_free_port().await };
    let mut host = String::new();
    let tok = token.or_else(ensure_bridge_token);
    // Attempt to spawn bridge binary, falling back to `cargo run -p oa-bridge` for dev
    let spawn_bridge = |
        program: &str, args: &[&str], tok: &Option<String>
    | -> Result<tokio::process::Child, std::io::Error> {
        let mut c = tokio::process::Command::new(program);
        for a in args { c.arg(a); }
        if let Some(t) = tok { c.env("OPENAGENTS_BRIDGE_TOKEN", t); }
        c.stdout(std::process::Stdio::piped());
        c.stderr(std::process::Stdio::piped());
        c.spawn()
    };

    // Try up to 5 successive ports to avoid AddrInUse races
    let mut child = loop {
        let mut found: Option<tokio::process::Child> = None;
        for offset in 0..5u16 {
            let port = base_port.saturating_add(offset);
            host = format!("127.0.0.1:{}", port);
            match spawn_bridge("oa-bridge", &["--bind", &host], &tok) {
                Ok(mut ch) => {
                    // If child exits immediately, try next port
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                    if let Ok(Some(_st)) = ch.try_wait() { continue; }
                    found = Some(ch); break;
                }
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // Fallback to cargo (dev)
                    match spawn_bridge("cargo", &["run", "-p", "oa-bridge", "--", "--bind", &host], &tok) {
                        Ok(mut ch) => {
                            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                            if let Ok(Some(_st)) = ch.try_wait() { continue; }
                            found = Some(ch); break;
                        }
                        Err(_e2) => continue,
                    }
                }
                Err(_e) => continue,
            }
        }
        if let Some(ch) = found { break ch; }
        else { return Err("failed to start oa-bridge on available ports".into()); }
    };
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
        *state.starting.lock().await = false;
    }
    // Monitor child liveness and clear state if it exits
    let state_for_monitor = state.inner().clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let mut guard = state_for_monitor.child.lock().await;
            if let Some(ref mut ch) = *guard {
                match ch.try_wait() {
                    Ok(Some(_status)) => {
                        // Exited — clear and stop monitoring
                        *guard = None;
                        *state_for_monitor.bind.lock().await = None;
                        break;
                    }
                    Ok(None) => { /* still running */ }
                    Err(_) => { /* transient error; leave as is */ }
                }
            } else {
                break;
            }
        }
    });
    Ok(host)
}

#[tauri::command]
async fn bridge_stop(state: tauri::State<'_, Arc<BridgeState>>) -> Result<bool, String> {
    let mut guard = state.child.lock().await;
    if let Some(mut child) = guard.take() {
        // Kill and wait for exit (best effort)
        let _ = child.kill().await;
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(2), child.wait()).await;
        *state.bind.lock().await = None;
        state.logs.lock().await.clear();
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
async fn bridge_status(state: tauri::State<'_, Arc<BridgeState>>) -> Result<serde_json::Value, String> {
    let running = {
        let mut guard = state.child.lock().await;
        if let Some(ch) = guard.as_mut() {
            match ch.try_wait() { Ok(None) => true, Ok(Some(_)) | Err(_) => { *guard = None; false } }
        } else { false }
    };
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
