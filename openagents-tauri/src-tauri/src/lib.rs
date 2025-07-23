mod claude_code;

use claude_code::{ClaudeDiscovery, ClaudeManager, Message, ClaudeConversation};
use log::info;
use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;
use std::sync::Arc;

// State wrapper for Tauri
pub struct AppState {
    discovery: Arc<Mutex<ClaudeDiscovery>>,
    manager: Arc<Mutex<Option<ClaudeManager>>>,
}

#[derive(Serialize)]
struct CommandResult<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> CommandResult<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn discover_claude(state: State<'_, AppState>) -> Result<CommandResult<String>, String> {
    let mut discovery = state.discovery.lock().await;
    
    match discovery.discover_binary().await {
        Ok(path) => {
            info!("Claude binary found at: {:?}", path);
            
            // Also try to discover data directory
            let _ = discovery.discover_data_directory().await;
            
            // Initialize the manager with the binary path
            let mut manager = ClaudeManager::new();
            manager.set_binary_path(path.clone());
            
            let mut manager_lock = state.manager.lock().await;
            *manager_lock = Some(manager);
            
            Ok(CommandResult::success(path.to_string_lossy().to_string()))
        }
        Err(e) => Ok(CommandResult::error(e.to_string())),
    }
}

#[tauri::command]
async fn create_session(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<String>, String> {
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        match manager.create_session(project_path).await {
            Ok(session_id) => Ok(CommandResult::success(session_id)),
            Err(e) => Ok(CommandResult::error(e.to_string())),
        }
    } else {
        Ok(CommandResult::error("Claude Code not initialized. Please discover Claude first.".to_string()))
    }
}

#[tauri::command]
async fn send_message(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        match manager.send_message(&session_id, message).await {
            Ok(_) => Ok(CommandResult::success(())),
            Err(e) => Ok(CommandResult::error(e.to_string())),
        }
    } else {
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
async fn get_messages(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<Message>>, String> {
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        match manager.get_messages(&session_id).await {
            Ok(messages) => Ok(CommandResult::success(messages)),
            Err(e) => Ok(CommandResult::error(e.to_string())),
        }
    } else {
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
async fn stop_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        match manager.stop_session(&session_id).await {
            Ok(_) => Ok(CommandResult::success(())),
            Err(e) => Ok(CommandResult::error(e.to_string())),
        }
    } else {
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
async fn get_active_sessions(
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<(String, String)>>, String> {
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        let sessions = manager.get_active_sessions().await;
        Ok(CommandResult::success(sessions))
    } else {
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
async fn get_history(
    limit: usize,
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<ClaudeConversation>>, String> {
    let discovery = state.discovery.lock().await;
    
    match discovery.load_conversations(limit).await {
        Ok(conversations) => Ok(CommandResult::success(conversations)),
        Err(e) => Ok(CommandResult::error(e.to_string())),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    env_logger::init();
    
    let app_state = AppState {
        discovery: Arc::new(Mutex::new(ClaudeDiscovery::new())),
        manager: Arc::new(Mutex::new(None)),
    };
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            discover_claude,
            create_session,
            send_message,
            get_messages,
            stop_session,
            get_active_sessions,
            get_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
