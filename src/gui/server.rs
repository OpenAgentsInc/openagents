//! Unified Actix server

use actix_web::{web, App, HttpServer};
use std::time::Duration;

use super::routes;
use super::state::{AppState, DaemonInfo, fetch_claude_info_fast, fetch_claude_model};

/// Start the unified server
pub async fn start_server() -> anyhow::Result<u16> {
    // Initialize authentication token
    let auth_token = auth::AuthToken::init().await?;
    let auth_token = web::Data::new(auth_token);

    // Print token for CLI usage
    println!("\n┌──────────────────────────────────────────────────────────────┐");
    println!("│ OpenAgents API Authentication Token                         │");
    println!("├──────────────────────────────────────────────────────────────┤");
    println!("│ Token: {}  │", auth_token.token());
    println!("│ File:  {:60} │", auth_token.token_file_path().display().to_string());
    println!("├──────────────────────────────────────────────────────────────┤");
    println!("│ Use this token to authenticate API requests:                │");
    println!("│ Authorization: Bearer {}           │", auth_token.token());
    println!("└──────────────────────────────────────────────────────────────┘\n");

    // Create shared state
    let state = web::Data::new(AppState::new());

    // Spawn background task to fetch Claude info (fast version - instant)
    let state_clone = state.clone();
    tokio::spawn(async move {
        // Fast check first (file reads + version command - instant)
        let info = fetch_claude_info_fast().await;
        {
            let mut guard = state_clone.claude_info.write().await;
            *guard = info;
        }

        // Then fetch current model in background (slow - makes API call)
        if let Some(model) = fetch_claude_model().await {
            let mut guard = state_clone.claude_info.write().await;
            guard.model = Some(model);
        }
    });

    // Spawn background task to poll daemon status
    let state_clone = state.clone();
    tokio::spawn(async move {
        poll_daemon_status(state_clone).await;
    });

    // Start server on random available port
    let auth_token_clone = auth_token.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .app_data(auth_token_clone.clone())
            .configure(routes::configure)
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    // Spawn server with tokio (not actix_web::rt::spawn which needs LocalSet)
    tokio::spawn(server.run());

    Ok(port)
}

/// Poll the daemon socket for status updates
async fn poll_daemon_status(state: web::Data<AppState>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let socket_path = std::path::PathBuf::from(&home)
        .join(".autopilot")
        .join("autopilotd.sock");

    loop {
        // Try to connect and get status
        let result = async {
            let mut stream = UnixStream::connect(&socket_path).await?;

            let request = serde_json::json!({ "type": "Status" });
            let request_bytes = serde_json::to_vec(&request)?;
            stream.write_all(&request_bytes).await?;

            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).await?;

            let response: serde_json::Value = serde_json::from_slice(&buf[..n])?;
            Ok::<_, anyhow::Error>(response)
        }
        .await;

        match result {
            Ok(response) => {
                // Parse daemon metrics from response
                if let Some(data) = response.get("data") {
                    let mut info = state.daemon_info.write().await;
                    info.connected = true;
                    info.error = None;

                    if let Some(status) = data.get("worker_status").and_then(|v| v.as_str()) {
                        info.worker_status = status.to_string();
                    }
                    if let Some(pid) = data.get("worker_pid").and_then(|v| v.as_u64()) {
                        info.worker_pid = Some(pid as u32);
                    } else {
                        info.worker_pid = None;
                    }
                    if let Some(uptime) = data.get("uptime_seconds").and_then(|v| v.as_u64()) {
                        info.uptime_seconds = uptime;
                    }
                    if let Some(restarts) = data.get("total_restarts").and_then(|v| v.as_u64()) {
                        info.total_restarts = restarts;
                    }
                    if let Some(failures) = data.get("consecutive_failures").and_then(|v| v.as_u64()) {
                        info.consecutive_failures = failures as u32;
                    }
                    if let Some(mem_avail) = data.get("memory_available_bytes").and_then(|v| v.as_u64()) {
                        info.memory_available_bytes = mem_avail;
                    }
                    if let Some(mem_total) = data.get("memory_total_bytes").and_then(|v| v.as_u64()) {
                        info.memory_total_bytes = mem_total;
                    }
                    info.last_updated = Some(chrono::Utc::now());
                }
            }
            Err(_) => {
                // Connection failed - daemon not running or socket unavailable
                let mut info = state.daemon_info.write().await;
                *info = DaemonInfo {
                    connected: false,
                    worker_status: "disconnected".to_string(),
                    error: Some("Daemon not running".to_string()),
                    ..Default::default()
                };
            }
        }

        // Poll every 3 seconds
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
