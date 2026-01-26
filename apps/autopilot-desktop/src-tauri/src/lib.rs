mod acp;
mod agent;
mod backend;
mod codex;
mod codex_home;
pub mod contracts;
mod event_sink;
mod file_logger;
mod full_auto;
mod signature_registry;
mod state;
mod types;

use tauri::Manager;

fn load_app_env() {
    let mut candidates = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".env"));
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join(".env"));
        }
    }

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(repo_root) = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
    {
        candidates.push(
            repo_root
                .join("apps")
                .join("autopilot-desktop")
                .join(".env"),
        );
    }

    for env_path in candidates {
        if env_path.exists() {
            if let Err(err) = dotenvy::from_path(&env_path) {
                eprintln!("Warning: failed to load {}: {}", env_path.display(), err);
            }
            break;
        }
    }
}

#[cfg(target_os = "linux")]
fn configure_linux_display_backend() {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let display = std::env::var("DISPLAY").ok();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();

    if session_type.as_deref() == Some("wayland")
        && display
            .as_deref()
            .map(|value| !value.is_empty())
            .unwrap_or(false)
        && wayland_display
            .as_deref()
            .map(|value| !value.is_empty())
            .unwrap_or(false)
        && std::env::var_os("AUTOPILOT_FORCE_WAYLAND").is_none()
    {
        std::env::set_var("GDK_BACKEND", "x11");
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub fn build_app() -> tauri::Builder<tauri::Wry> {
    #[cfg(target_os = "linux")]
    configure_linux_display_backend();

    load_app_env();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = state::AppState::load(&app.handle());
            app.manage(state);

            // Initialize AI server configuration
            match ai_server::AiServerConfig::from_env() {
                Ok(config) => {
                    if let Err(e) = ai_server::init_ai_server(config) {
                        eprintln!("Warning: Failed to initialize AI server: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: AI server configuration error: {}", e);
                    eprintln!("AI server will not be available. Set AI_GATEWAY_API_KEY to enable.");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            codex::codex_doctor,
            codex::test_codex_connection,
            codex::connect_workspace,
            codex::disconnect_workspace,
            codex::get_workspace_connection_status,
            codex::start_thread,
            codex::list_threads,
            codex::resume_thread,
            codex::send_user_message,
            codex::set_full_auto,
            codex::get_current_directory,
            codex::account_rate_limits,
            codex::list_models,
            // Unified agent commands
            agent::commands::connect_unified_agent,
            agent::commands::disconnect_unified_agent,
            agent::commands::start_unified_session,
            agent::commands::send_unified_message,
            agent::commands::get_unified_conversation_items,
            agent::commands::get_unified_agent_status,
            signature_registry::list_dsrs_signatures,
            signature_registry::get_dsrs_signature,
            // AI server commands
            start_ai_server,
            stop_ai_server,
            restart_ai_server,
            get_ai_server_status,
            get_ai_server_analytics,
        ])
}

// AI Server Commands

#[tauri::command]
async fn start_ai_server() -> Result<String, String> {
    ai_server::start_ai_server()
        .await
        .map_err(|e| e.to_string())?;
    Ok("AI server started successfully".to_string())
}

#[tauri::command]
async fn stop_ai_server() -> Result<String, String> {
    ai_server::stop_ai_server()
        .await
        .map_err(|e| e.to_string())?;
    Ok("AI server stopped successfully".to_string())
}

#[tauri::command]
async fn restart_ai_server() -> Result<String, String> {
    ai_server::stop_ai_server()
        .await
        .map_err(|e| e.to_string())?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    ai_server::start_ai_server()
        .await
        .map_err(|e| e.to_string())?;
    Ok("AI server restarted successfully".to_string())
}

#[tauri::command]
async fn get_ai_server_status() -> Result<serde_json::Value, String> {
    let is_running = ai_server::is_ai_server_running().await;

    if is_running {
        match ai_server::get_ai_server_health().await {
            Ok(health) => Ok(serde_json::json!({
                "running": true,
                "healthy": true,
                "health": health
            })),
            Err(e) => Ok(serde_json::json!({
                "running": false,
                "healthy": false,
                "error": e.to_string()
            })),
        }
    } else {
        Ok(serde_json::json!({
            "running": false,
            "healthy": false,
            "error": "Server not running"
        }))
    }
}

#[tauri::command]
async fn get_ai_server_analytics() -> Result<serde_json::Value, String> {
    // Note: This would need to be implemented to get analytics from the global server
    // For now, return a placeholder
    Ok(serde_json::json!({
        "totalRequests": 0,
        "totalTokens": 0,
        "uptime": 0,
        "modelBreakdown": {}
    }))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    build_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
