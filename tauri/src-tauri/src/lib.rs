use std::{path::PathBuf, sync::Arc};

use agent_client_protocol as acp;
use serde::Serialize;
use tauri::State;

// WebSocket server configuration
const DEFAULT_TINYVEX_WS_PORT: u16 = 9100;
static CHOSEN_TINYVEX_WS_PORT: once_cell::sync::OnceCell<u16> = once_cell::sync::OnceCell::new();

#[cfg(target_os = "ios")]
use tauri::webview::WebviewWindowBuilder;
#[cfg(target_os = "ios")]
use tauri_utils::config::WebviewUrl;

mod oa_acp;
use oa_acp::SessionManager;
mod codex_exec;
mod tinyvex_state;
mod tinyvex_controls;
mod tinyvex_ws;
#[cfg(not(target_os = "ios"))]
mod mdns_advertiser;
#[cfg(target_os = "ios")]
mod discovery;
use std::sync::Once;
use once_cell::sync::OnceCell;
use tracing::{error, info};

static INIT_TRACING: Once = Once::new();
pub static APP_HANDLE: OnceCell<tauri::AppHandle> = OnceCell::new();

fn init_tracing() {
    INIT_TRACING.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .try_init();
        info!("tracing initialized (set RUST_LOG for verbosity)");
    });
}

// Tauri commands (Phase 1)

#[tauri::command]
async fn create_session(
    state: State<'_, AppState>,
    agent_type: Option<String>,
    cwd: Option<String>,
) -> Result<String, String> {
    init_tracing();
    let cwd_path = match cwd {
        Some(p) => {
            let pb = PathBuf::from(p);
            if pb.is_absolute() {
                pb
            } else {
                std::env::current_dir()
                    .map_err(|e| e.to_string())?
                    .join(pb)
            }
        }
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };
    info!(?agent_type, cwd=%cwd_path.display(), "create_session called");
    state
        .sessions
        .create_session(agent_type.unwrap_or_else(|| "claude-code".to_string()), cwd_path)
        .await
        .map(|sid| sid.0.to_string())
        .map_err(|e| e.to_string())
}

// Note: command args are defined inline in the tauri::command signatures

#[tauri::command]
async fn send_prompt(state: State<'_, AppState>, session_id: String, text: String) -> Result<(), String> {
    init_tracing();
    info!(%session_id, text_len = text.len(), "send_prompt called");
    let session_id = acp::SessionId(Arc::from(session_id));
    state
        .sessions
        .prompt(&session_id, text)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
struct ACPSessionDto {
    id: String,
    messages: Vec<ACPMessageDto>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ACPMessageDto {
    id: String,
    role: String,
    content: Vec<acp::ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
}

#[tauri::command]
async fn get_session(state: State<'_, AppState>, session_id: String) -> Result<ACPSessionDto, String> {
    init_tracing();
    let session_id = acp::SessionId(Arc::from(session_id));
    let s = state
        .sessions
        .get_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;
    info!(id=%s.id.0, messages=%s.messages.len(), "get_session");

    let messages = s
        .messages
        .iter()
        .map(|m| ACPMessageDto {
            id: m.id.clone(),
            role: m.role.clone(),
            content: m.content.clone(),
            created_at: Some(m.created_at.clone()),
            metadata: None,
        })
        .collect();

    Ok(ACPSessionDto { id: s.id.0.to_string(), messages })
}

#[tauri::command]
async fn resolve_acp_agent_path() -> Result<String, String> {
    init_tracing();
    match oa_acp::try_resolve_acp_agent() {
        Ok(p) => {
            info!(path=%p.display(), "resolved codex-acp");
            Ok(p.display().to_string())
        }
        Err(e) => {
            error!(?e, "failed to resolve codex-acp");
            Err(format!(
                "ACP agent not found. Build codex-acp (e.g. cargo build --manifest-path crates/codex-acp/Cargo.toml --release) or set OA_CODEX_ACP_ROOT / OA_ACP_AGENT_CMD. Error: {e}"
            ))
        }
    }
}

#[tauri::command]
fn validate_directory(path: String) -> Result<bool, String> {
    init_tracing();
    let pb = PathBuf::from(&path);
    let is_valid = pb.exists() && pb.is_dir();
    info!(path=%path, is_valid=%is_valid, "validate_directory called");
    Ok(is_valid)
}

#[tauri::command]
async fn pick_directory(app_handle: tauri::AppHandle) -> Result<Option<String>, String> {
    init_tracing();

    #[cfg(not(target_os = "ios"))]
    {
        use tauri_plugin_dialog::DialogExt;
        use tokio::sync::oneshot;

        let (tx, rx) = oneshot::channel();

        app_handle.dialog()
            .file()
            .set_title("Select Project Directory")
            .pick_folder(move |folder_path| {
                let _ = tx.send(folder_path);
            });

        // Wait for the result
        match rx.await {
            Ok(Some(path)) => {
                let path_str = path.to_string();
                info!(path=%path_str, "Directory selected");
                Ok(Some(path_str))
            }
            Ok(None) => {
                info!("Directory selection cancelled");
                Ok(None)
            }
            Err(_) => {
                error!("Failed to receive directory selection result");
                Err("Failed to receive directory selection result".to_string())
            }
        }
    }

    #[cfg(target_os = "ios")]
    {
        // iOS doesn't support folder picking
        info!("pick_directory not available on iOS");
        Ok(None)
    }
}

// Mobile discovery commands

#[tauri::command]
fn get_platform() -> String {
    #[cfg(target_os = "ios")]
    return "ios".to_string();

    #[cfg(target_os = "android")]
    return "android".to_string();

    #[cfg(target_os = "macos")]
    return "macos".to_string();

    #[cfg(target_os = "windows")]
    return "windows".to_string();

    #[cfg(target_os = "linux")]
    return "linux".to_string();

    #[cfg(not(any(target_os = "ios", target_os = "android", target_os = "macos", target_os = "windows", target_os = "linux")))]
    return "unknown".to_string();
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn discover_servers() -> Result<Vec<discovery::ServerInfo>, String> {
    init_tracing();
    info!("discover_servers called");
    discovery::discover_servers()
        .await
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn test_server_connection(host: String, port: u16) -> Result<bool, String> {
    init_tracing();
    info!(host = %host, port = port, "test_server_connection called");
    discovery::test_connection(&host, port)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_last_server() -> Result<Option<String>, String> {
    init_tracing();
    // Use tauri's storage to retrieve last connected server
    // For now, return None - will implement persistence later
    Ok(None)
}

#[tauri::command]
async fn save_last_server(server_info: String) -> Result<(), String> {
    init_tracing();
    info!(server_info = %server_info, "save_last_server called");
    // Use tauri's storage to save last connected server
    // For now, just log - will implement persistence later
    Ok(())
}

#[tauri::command]
fn get_websocket_url() -> String {
    let port = *CHOSEN_TINYVEX_WS_PORT.get().unwrap_or(&DEFAULT_TINYVEX_WS_PORT);
    format!("ws://127.0.0.1:{}/ws", port)
}

pub struct AppState {
    sessions: SessionManager,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    // Initialize tinyvex database
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("openagents");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
    let db_path = data_dir.join("tinyvex.db");

    info!("Initializing tinyvex database at: {}", db_path.display());
    let tinyvex_db = Arc::new(tinyvex::Tinyvex::open(&db_path).expect("Failed to open tinyvex database"));
    let tinyvex_writer = Arc::new(tinyvex::Writer::new(tinyvex_db.clone()));
    let tinyvex_state = Arc::new(tinyvex_state::TinyvexState::new(tinyvex_db, tinyvex_writer));

    let tinyvex_state_for_setup = tinyvex_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            sessions: SessionManager::new(tinyvex_state.clone()),
        })
        .invoke_handler(tauri::generate_handler![
            create_session,
            send_prompt,
            get_session,
            resolve_acp_agent_path,
            validate_directory,
            pick_directory,
            get_platform,
            #[cfg(target_os = "ios")]
            discover_servers,
            #[cfg(target_os = "ios")]
            test_server_connection,
            get_last_server,
            save_last_server,
            get_websocket_url
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                use tauri::{Manager, TitleBarStyle};
                let window = app.get_webview_window("main").unwrap();
                window.set_title_bar_style(TitleBarStyle::Transparent)?;
            }
            #[cfg(target_os = "ios")]
            {
                use tauri::Manager;
                // On iOS, always load embedded assets directly (no dev server).
                let app_url = WebviewUrl::App("index.html".into());
                if app.get_webview_window("main").is_none() {
                    let _ = WebviewWindowBuilder::new(app, "main".to_string(), app_url).build();
                }
            }
            let _ = APP_HANDLE.set(app.handle().clone());

            // Start WebSocket server in Tauri's tokio runtime on desktop only.
            // On iOS, the app is a thin client and connects to a desktop server.
            #[cfg(not(target_os = "ios"))]
            {
                let tinyvex_state_clone = tinyvex_state_for_setup.clone();
                tauri::async_runtime::spawn(async move {
                    let router = tinyvex_ws::create_router(tinyvex_state_clone);
                    // Try binding DEFAULT port first; if unavailable, try a small range then fall back to 0 (OS-assigned)
                    let mut bound_listener: Option<(tokio::net::TcpListener, u16)> = None;
                    let mut try_ports: Vec<u16> = (DEFAULT_TINYVEX_WS_PORT..DEFAULT_TINYVEX_WS_PORT + 16).collect();
                    try_ports.push(0); // OS-assign as last resort
                    for p in try_ports {
                        let addr = if p == 0 { "0.0.0.0:0".to_string() } else { format!("0.0.0.0:{}", p) };
                        match tokio::net::TcpListener::bind(&addr).await {
                            Ok(listener) => {
                                let actual_port = listener.local_addr().ok().map(|a| a.port()).unwrap_or(p.max(1));
                                bound_listener = Some((listener, actual_port));
                                let _ = CHOSEN_TINYVEX_WS_PORT.set(actual_port);
                                break;
                            }
                            Err(_) => continue,
                        }
                    }
                    let (listener, port) = bound_listener.expect("Failed to bind WebSocket server on any port");
                    info!(
                        "WebSocket server listening on ws://{}:{}/ws (accessible from local network)",
                        "0.0.0.0",
                        port
                    );

                    // Advertise service via Bonjour/mDNS for mobile discovery
                    {
                        use crate::mdns_advertiser;
                        if let Err(e) = mdns_advertiser::start_advertising(port) {
                            error!(?e, "Failed to start mDNS advertising");
                        } else {
                            info!("mDNS service advertising started: _openagents._tcp.local:{}", port);
                        }
                    }

                    axum::serve(listener, router)
                        .await
                        .expect("WebSocket server failed");
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
