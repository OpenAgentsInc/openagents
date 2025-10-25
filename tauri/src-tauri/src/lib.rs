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
        .unwrap_or_else(|| "http://127.0.0.1:7788".to_string());

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            // Start codex-bridge automatically in the background on app launch.
            tauri::async_runtime::spawn(async move {
                ensure_bridge_running().await;
            });
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, get_thread_count])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn ensure_bridge_running() {
    // If the bridge port is open we assume it's already running
    if is_port_open(8787).await { return; }
    let repo = detect_repo_root(None);
    let bin = repo.join("target").join("debug").join(if cfg!(windows) { "codex-bridge.exe" } else { "codex-bridge" });
    let mut cmd = if bin.exists() {
        let mut c = tokio::process::Command::new(bin);
        c.arg("--bind").arg("0.0.0.0:8787");
        c
    } else {
        let mut c = tokio::process::Command::new("cargo");
        c.args(["run", "-q", "-p", "codex-bridge", "--", "--bind", "0.0.0.0:8787"]);
        c
    };
    cmd.current_dir(&repo)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let _ = cmd.spawn();
    // Give it a moment then return
    let _ = tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}

async fn is_port_open(port: u16) -> bool {
    use tokio::net::TcpStream;
    TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port))
        .await
        .is_ok()
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
