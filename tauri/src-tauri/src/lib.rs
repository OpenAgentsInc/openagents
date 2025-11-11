use std::{collections::HashMap, path::PathBuf, sync::Arc};

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::{Mutex, RwLock};

mod oa_acp;
use oa_acp::{ACPClient, AcpError, SessionManager};
mod codex_exec;
use std::sync::Once;
use tracing::{debug, error, info, warn};

static INIT_TRACING: Once = Once::new();

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

#[derive(Debug, Deserialize)]
struct SendPromptArgs {
    session_id: String,
    text: String,
}

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

pub struct AppState {
    sessions: SessionManager,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            sessions: SessionManager::new(),
        })
        .invoke_handler(tauri::generate_handler![create_session, send_prompt, get_session])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                use tauri::{Manager, TitleBarStyle};
                let window = app.get_webview_window("main").unwrap();
                window.set_title_bar_style(TitleBarStyle::Transparent)?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
