//! Local sidecar bootstrap and utility helpers for the Tauri backend.

use tauri::Emitter;

pub fn start_convex_sidecar(app: tauri::AppHandle) {
    // Bring Manager trait into scope for path resolution helpers on AppHandle
    use tauri::Manager as _;
    // Use dynamic port (defaults to 7788) so desktop + mobile share the same Convex.
    let port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    if std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port)).is_ok() {
        println!("[tauri/convex] detected local backend on 127.0.0.1:{}", port);
        return;
    }
    // Build candidate list: packaged resource, env override, repo-local, legacy user install
    let mut bin_candidates: Vec<std::path::PathBuf> = Vec::new();
    // Packaged resource paths (both names to handle Windows/non-Windows layouts)
    if let Ok(p) = app.path().resolve("bin/local_backend", tauri::path::BaseDirectory::Resource) { bin_candidates.push(p); }
    if let Ok(p) = app.path().resolve("bin/local_backend.exe", tauri::path::BaseDirectory::Resource) { bin_candidates.push(p); }
    // Env override
    if let Ok(p) = std::env::var("OPENAGENTS_CONVEX_BIN") { bin_candidates.push(std::path::PathBuf::from(p)); }
    // Repo-local dev path
    let repo = detect_repo_root(None);
    bin_candidates.push(repo.join("tauri").join("src-tauri").join("bin").join(if cfg!(windows) { "local_backend.exe" } else { "local_backend" }));
    // User install path used previously
    if let Ok(home) = std::env::var("HOME") { bin_candidates.push(std::path::PathBuf::from(home).join(".openagents/bin/local_backend")); }

    // Pick a candidate that exists and looks like a real binary (>1MB)
    let bin = bin_candidates.into_iter().find(|p| {
        if let Ok(meta) = std::fs::metadata(p) { meta.is_file() && meta.len() > 1_000_000 } else { false }
    }).unwrap_or_else(|| std::path::PathBuf::from("local_backend"));
    if !bin.exists() {
        println!("[tauri/convex] local backend binary not found (resource, OPENAGENTS_CONVEX_BIN, or tauri/src-tauri/bin/local_backend)");
        return;
    }
    let db_path = default_convex_db_path();
    if let Some(parent) = db_path.parent() { let _ = std::fs::create_dir_all(parent); }
    let mut cmd = std::process::Command::new(&bin);
    let interface = std::env::var("OPENAGENTS_CONVEX_INTERFACE").unwrap_or_else(|_| "0.0.0.0".to_string());
    cmd.arg(&db_path)
        .arg("--db").arg("sqlite")
        .arg("--interface").arg(interface)
        .arg("--port").arg(port.to_string())
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
    if let Ok(home) = std::env::var("HOME") { return std::path::PathBuf::from(home).join(".openagents/convex/data.sqlite3"); }
    std::path::PathBuf::from("data.sqlite3")
}

pub fn read_env_local_var(key: &str) -> Option<String> {
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

pub fn deploy_convex_functions_once(handle: tauri::AppHandle) {
    // Wait for sidecar port to be open
    let port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    for _ in 0..200 { // ~20s
        if std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port)).is_ok() { break; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port)).is_err() {
        println!("[tauri/convex] backend not reachable on 127.0.0.1:{}; skipping auto-deploy", port);
        return;
    }
    // Emit healthy status immediately once we know the port is reachable
    let _ = handle.emit("convex:local_status", serde_json::json!({ "healthy": true, "url": format!("http://127.0.0.1:{}", port) }));
    // Resolve admin key
    let admin = std::env::var("CONVEX_ADMIN_KEY")
        .ok()
        .or_else(|| read_env_local_var("CONVEX_ADMIN_KEY"))
        .or_else(|| read_env_local_var("CONVEX_SELF_HOSTED_ADMIN_KEY"))
        .unwrap_or_else(|| "carnitas|017c5405aba48afe1d1681528424e4528026e69e3b99e400ef23f2f3741a11db225497db09".to_string());
    let root = detect_repo_root(None);
    let mut cmd = std::process::Command::new("bun");
    cmd.args(["run", "convex:dev:once"]).current_dir(&root)
        .env("CONVEX_URL", format!("http://127.0.0.1:{}", port))
        .env("CONVEX_SELF_HOSTED_URL", format!("http://127.0.0.1:{}", port))
        .env("CONVEX_ADMIN_KEY", &admin)
        .env("CONVEX_SELF_HOSTED_ADMIN_KEY", &admin)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    match cmd.spawn() { Ok(child) => println!("[tauri/convex] deploying functions (dev:once) pid={}", child.id()), Err(e) => println!("[tauri/convex] failed to spawn bun convex:dev:once: {}", e) }
}

pub async fn ensure_bridge_running() {
    // If the bridge port is open we assume it's already running
    if is_port_open(8787) { return; }
    let repo = detect_repo_root(None);
    println!("[tauri/bootstrap] repo root: {}", repo.display());
    let bin = repo.join("target").join("debug").join(if cfg!(windows) { "codex-bridge.exe" } else { "codex-bridge" });
    println!("[tauri/bootstrap] bridge bin exists? {} — {}", bin.exists(), bin.display());
    // In dev, prefer `cargo run -p codex-bridge` to ensure the latest code is used.
    let mut cmd = { let mut c = std::process::Command::new("cargo"); c.args(["run", "-q", "-p", "codex-bridge", "--", "--bind", "0.0.0.0:8787", "--manage-convex", "false", "--bootstrap", "false"]); c };
    cmd.current_dir(&repo)
        .env("RUST_LOG", "info")
        .env("OPENAGENTS_MANAGE_CONVEX", "false")
        .env("OPENAGENTS_BOOTSTRAP", "false")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    match cmd.spawn() { Ok(child) => { println!("[tauri/bootstrap] spawned codex-bridge pid={} (stdout/stderr inherited)", child.id()); }, Err(e) => { println!("[tauri/bootstrap] failed to spawn bridge: {e}"); } }
    for _ in 0..10 { if is_port_open(8787) { break; } std::thread::sleep(std::time::Duration::from_millis(200)); }
}

pub fn is_port_open(port: u16) -> bool {
    std::net::TcpStream::connect((std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port)).is_ok()
}

pub fn detect_repo_root(start: Option<std::path::PathBuf>) -> std::path::PathBuf {
    fn is_repo_root(p: &std::path::Path) -> bool { p.join("expo").is_dir() && p.join("crates").is_dir() }
    let mut cur = start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let original = cur.clone();
    loop { if is_repo_root(&cur) { return cur; } if !cur.pop() { return original; } }
}

