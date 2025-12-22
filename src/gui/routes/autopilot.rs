//! Autopilot routes

use std::sync::Arc;

use actix_web::{web, HttpResponse};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use ui::FullAutoSwitch;

use crate::gui::state::{AppState, AutopilotProcess};
use crate::gui::ws::WsBroadcaster;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(dashboard))
        .route("/sessions", web::get().to(sessions_page))
        .route("/metrics", web::get().to(metrics_page));
}

/// Configure API routes (called from main routes)
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("/toggle", web::post().to(toggle_full_auto))
        .route("/status", web::get().to(get_status));
}

/// Toggle full auto mode and return new switch HTML
async fn toggle_full_auto(state: web::Data<AppState>) -> HttpResponse {
    let mut full_auto = state.full_auto.write().await;
    *full_auto = !*full_auto;
    let new_state = *full_auto;
    drop(full_auto);

    if new_state {
        // Spawn autopilot process
        if let Err(e) = spawn_autopilot_process(&state).await {
            tracing::error!("Failed to spawn autopilot: {}", e);
            // Revert state
            *state.full_auto.write().await = false;

            // Broadcast error
            state.broadcaster.broadcast(&format!(
                r#"<div class="log-error">Failed to start autopilot: {}</div>"#,
                e
            ));

            let switch = FullAutoSwitch::new(false).build();
            return HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(switch.into_string());
        }
    } else {
        // Stop autopilot process
        stop_autopilot_process(&state).await;
    }

    // Return the new switch HTML for HTMX to swap
    // Also include script to show/hide log pane
    let switch = FullAutoSwitch::new(new_state).build();
    let script = if new_state {
        r#"<script>document.getElementById('autopilot-log').classList.remove('hidden')</script>"#
    } else {
        r#"<script>document.getElementById('autopilot-log').classList.add('hidden')</script>"#
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(format!("{}{}", switch.into_string(), script))
}

/// Spawn the autopilot process with output streaming
async fn spawn_autopilot_process(state: &web::Data<AppState>) -> anyhow::Result<()> {
    use std::process::Stdio;

    // Check if already running
    {
        let guard = state.autopilot_process.read().await;
        if guard.is_some() {
            anyhow::bail!("Autopilot already running");
        }
    }

    // Build command - same as .cargo/config.toml fullauto alias
    let mut cmd = Command::new("cargo");
    cmd.args([
        "run", "-p", "autopilot", "--bin", "autopilot", "--",
        "run", "--with-issues", "--full-auto", "--max-turns", "99999",
        "Begin autonomous work. Call issue_ready to get the first issue, or if none exist, review the active directives and create issues to advance them."
    ]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let broadcaster = state.broadcaster.clone();

    // Broadcast startup message
    broadcaster.broadcast(r#"<div class="log-line" style="color: #22c55e;">Autopilot starting...</div>"#);

    // Spawn output reader task
    let output_task = tokio::spawn(async move {
        read_output_loop(stdout, stderr, broadcaster, &mut shutdown_rx).await;
    });

    // Store in state
    let autopilot = AutopilotProcess {
        child,
        output_task,
        shutdown_tx,
    };
    *state.autopilot_process.write().await = Some(autopilot);

    Ok(())
}

/// Read stdout/stderr and broadcast to WebSocket
async fn read_output_loop(
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    broadcaster: Arc<WsBroadcaster>,
    shutdown_rx: &mut tokio::sync::mpsc::Receiver<()>,
) {
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                broadcaster.broadcast(r#"<div class="log-line" style="color: #888;">Autopilot stopped.</div>"#);
                break;
            }

            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        // Escape HTML and wrap in log line
                        let escaped = html_escape(&text);
                        let html = format!(r#"<div class="log-line">{}</div>"#, escaped);
                        broadcaster.broadcast(&html);
                    }
                    Ok(None) => {
                        // EOF - process exited
                        broadcaster.broadcast(r#"<div class="log-error">Autopilot process exited.</div>"#);
                        break;
                    }
                    Err(e) => {
                        broadcaster.broadcast(&format!(
                            r#"<div class="log-error">Read error: {}</div>"#,
                            e
                        ));
                        break;
                    }
                }
            }

            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        let escaped = html_escape(&text);
                        let html = format!(r#"<div class="log-error">{}</div>"#, escaped);
                        broadcaster.broadcast(&html);
                    }
                    Ok(None) => {
                        // EOF on stderr is normal
                    }
                    Err(_) => {
                        // Ignore stderr read errors
                    }
                }
            }
        }
    }
}

/// Stop the autopilot process gracefully
async fn stop_autopilot_process(state: &web::Data<AppState>) {
    let mut guard = state.autopilot_process.write().await;

    if let Some(mut autopilot) = guard.take() {
        // Signal shutdown to output reader
        let _ = autopilot.shutdown_tx.send(()).await;

        // Kill the process (sends SIGKILL on Unix, TerminateProcess on Windows)
        let _ = autopilot.child.kill().await;
        let _ = autopilot.child.wait().await;

        // Wait for output task to finish
        let _ = autopilot.output_task.await;
    }
}

/// Get autopilot process status
async fn get_status(state: web::Data<AppState>) -> HttpResponse {
    let guard = state.autopilot_process.read().await;
    let full_auto = *state.full_auto.read().await;

    let status = serde_json::json!({
        "full_auto": full_auto,
        "running": guard.is_some(),
    });

    HttpResponse::Ok().json(status)
}

/// Escape HTML special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

async fn dashboard() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Autopilot</h1><p>Coming soon...</p>")
}

async fn sessions_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Sessions</h1><p>Coming soon...</p>")
}

async fn metrics_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Metrics</h1><p>Coming soon...</p>")
}
