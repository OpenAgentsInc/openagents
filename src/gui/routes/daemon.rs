//! Daemon routes
//!
//! API routes for daemon/worker status and control.

use actix_web::{web, HttpResponse};
use tracing::info;
use ui::DaemonStatus;

use crate::gui::state::AppState;

/// Configure daemon routes (page views)
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(status_page));
}

/// Configure daemon API routes (for HTMX)
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("/status", web::get().to(get_status))
        .route("/start", web::post().to(start_daemon))
        .route("/stop", web::post().to(stop_daemon))
        .route("/restart-worker", web::post().to(restart_worker));
}

/// Simple page view for daemon
async fn status_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Daemon Status</h1><p>See the status panel in the bottom-left corner.</p>")
}

/// Get current daemon status (returns HTML for HTMX polling)
async fn get_status(state: web::Data<AppState>) -> HttpResponse {
    let info = state.daemon_info.read().await;

    let status = if info.connected {
        DaemonStatus::connected()
            .worker_status(&info.worker_status)
            .uptime(info.uptime_seconds)
            .restarts(info.total_restarts, info.consecutive_failures)
            .memory(info.memory_available_bytes, info.memory_total_bytes)
    } else {
        let mut s = DaemonStatus::disconnected();
        if let Some(ref err) = info.error {
            s = s.error(err.clone());
        }
        s
    };

    // Add PID if available
    let status = if let Some(pid) = info.worker_pid {
        status.worker_pid(pid)
    } else {
        status
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(status.build().into_string())
}

/// Start the daemon process
async fn start_daemon(state: web::Data<AppState>) -> HttpResponse {
    use std::process::Stdio;
    use tokio::process::Command;

    info!("POST /api/daemon/start called");

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let autopilot_dir = std::path::PathBuf::from(&home).join(".autopilot");
    let pid_file = autopilot_dir.join("autopilotd.pid");
    let socket_path = autopilot_dir.join("autopilotd.sock");

    // Check for stale PID file
    if pid_file.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Check if process is actually running
                let process_running = std::path::Path::new(&format!("/proc/{}", pid)).exists();
                info!("Found PID file with PID {}, process running: {}", pid, process_running);

                if !process_running {
                    // Stale PID file - clean up
                    info!("Cleaning up stale PID file and socket");
                    let _ = std::fs::remove_file(&pid_file);
                    let _ = std::fs::remove_file(&socket_path);
                } else {
                    // Process is running but socket doesn't exist - might be starting up
                    // Or it's a zombie. Give it a moment then check socket.
                    if !socket_path.exists() {
                        info!("Daemon PID {} exists but no socket - waiting briefly", pid);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        if !socket_path.exists() {
                            // Still no socket - kill the zombie and clean up
                            info!("Socket still missing, killing stale daemon PID {}", pid);
                            let _ = std::process::Command::new("kill")
                                .args(["-9", &pid.to_string()])
                                .output();
                            let _ = std::fs::remove_file(&pid_file);
                            let _ = std::fs::remove_file(&socket_path);
                        }
                    }
                }
            }
        }
    }

    // Check if already connected
    {
        let info = state.daemon_info.read().await;
        info!("Current daemon state: connected={}, worker_status={}", info.connected, info.worker_status);
        if info.connected {
            info!("Daemon already connected, returning early");
            return HttpResponse::Ok()
                .content_type("text/html")
                .body("<div class='toast'>Daemon already running</div>");
        }
    }

    // Get the current working directory for the daemon
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("openagents");
    info!("Working directory: {:?}, project: {}", cwd, project);

    // Find the daemon binary (autopilotd)
    let daemon_binary = autopilot_dir.join("bin").join("autopilotd");
    info!("Looking for daemon binary at: {:?}", daemon_binary);
    info!("Binary exists: {}", daemon_binary.exists());

    // Try multiple locations for the daemon binary
    let (cmd_desc, result) = if daemon_binary.exists() {
        // Use known-good autopilotd binary
        let args = ["--workdir", cwd.to_str().unwrap_or("."), "--project", project];
        info!("Spawning: {:?} {:?}", daemon_binary, args);
        let r = Command::new(&daemon_binary)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        (format!("{:?} {:?}", daemon_binary, args), r)
    } else {
        // Fall back to cargo run
        let args = ["run", "-p", "autopilot", "--bin", "autopilotd", "--",
                   "--workdir", cwd.to_str().unwrap_or("."), "--project", project];
        info!("Binary not found, falling back to cargo run");
        info!("Spawning: cargo {:?}", args);
        let r = Command::new("cargo")
            .args(args)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        (format!("cargo {:?}", args), r)
    };

    match result {
        Ok(mut child) => {
            info!("Daemon process spawned successfully, pid: {:?}", child.id());
            // Give daemon a moment to start
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Check if socket exists now
            info!("Checking for socket at: {:?}, exists: {}", socket_path, socket_path.exists());

            // Check if process is still running and capture any output
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process exited - capture output
                    info!("Daemon process exited immediately with status: {}", status);
                    if let Some(mut stderr) = child.stderr.take() {
                        use tokio::io::AsyncReadExt;
                        let mut stderr_buf = String::new();
                        let _ = stderr.read_to_string(&mut stderr_buf).await;
                        if !stderr_buf.is_empty() {
                            info!("Daemon stderr: {}", stderr_buf);
                        }
                    }
                    if let Some(mut stdout) = child.stdout.take() {
                        use tokio::io::AsyncReadExt;
                        let mut stdout_buf = String::new();
                        let _ = stdout.read_to_string(&mut stdout_buf).await;
                        if !stdout_buf.is_empty() {
                            info!("Daemon stdout: {}", stdout_buf);
                        }
                    }
                    HttpResponse::InternalServerError()
                        .content_type("text/html")
                        .body(format!("<div class='toast error'>Daemon exited: {}</div>", status))
                }
                Ok(None) => {
                    // Process still running
                    info!("Daemon process still running");
                    HttpResponse::Ok()
                        .content_type("text/html")
                        .body("<div class='toast'>Daemon starting...</div>")
                }
                Err(e) => {
                    info!("Failed to check daemon status: {}", e);
                    HttpResponse::Ok()
                        .content_type("text/html")
                        .body("<div class='toast'>Daemon starting...</div>")
                }
            }
        }
        Err(e) => {
            info!("Failed to spawn daemon: {} (command: {})", e, cmd_desc);
            HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(format!("<div class='toast error'>Failed to start daemon: {}</div>", e))
        }
    }
}

/// Stop the daemon
async fn stop_daemon(state: web::Data<AppState>) -> HttpResponse {
    info!("POST /api/daemon/stop called");
    let socket_path = get_socket_path();

    match send_control_command(&socket_path, "Shutdown").await {
        Ok(_) => {
            // Update state immediately
            {
                let mut info = state.daemon_info.write().await;
                info.connected = false;
                info.worker_status = "stopped".to_string();
            }
            HttpResponse::Ok()
                .content_type("text/html")
                .body("<div class='toast'>Daemon stopped</div>")
        }
        Err(e) => {
            HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(format!("<div class='toast error'>Failed to stop daemon: {}</div>", e))
        }
    }
}

/// Restart the worker
async fn restart_worker(_state: web::Data<AppState>) -> HttpResponse {
    info!("POST /api/daemon/restart-worker called");
    let socket_path = get_socket_path();

    match send_control_command(&socket_path, "RestartWorker").await {
        Ok(_) => {
            HttpResponse::Ok()
                .content_type("text/html")
                .body("<div class='toast'>Worker restart initiated</div>")
        }
        Err(e) => {
            HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(format!("<div class='toast error'>Failed to restart worker: {}</div>", e))
        }
    }
}

/// Get the daemon socket path
fn get_socket_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(&home)
        .join(".autopilot")
        .join("autopilotd.sock")
}

/// Send a control command to the daemon
async fn send_control_command(socket_path: &std::path::Path, command: &str) -> anyhow::Result<serde_json::Value> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path).await?;

    let request = serde_json::json!({ "type": command });
    let request_bytes = serde_json::to_vec(&request)?;
    stream.write_all(&request_bytes).await?;

    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;

    let response: serde_json::Value = serde_json::from_slice(&buf[..n])?;

    if response.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        Ok(response)
    } else {
        let msg = response.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        anyhow::bail!("{}", msg)
    }
}
