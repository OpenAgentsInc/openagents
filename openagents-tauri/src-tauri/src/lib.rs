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
async fn discover_claude(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResult<String>, String> {
    let mut discovery = state.discovery.lock().await;
    
    match discovery.discover_binary().await {
        Ok(path) => {
            // Also try to discover data directory
            let _ = discovery.discover_data_directory().await;
            
            // Initialize the manager with the binary path and app handle
            let mut manager = ClaudeManager::new();
            manager.set_binary_path(path.clone());
            manager.set_app_handle(app_handle);
            
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
    app_handle: tauri::AppHandle,
) -> Result<CommandResult<String>, String> {
    info!("create_session called - project_path: {}", project_path);
    let mut manager_lock = state.manager.lock().await;
    
    if let Some(ref mut manager) = *manager_lock {
        // Set the app handle if not already set
        manager.set_app_handle(app_handle);
        
        info!("Manager found, creating session...");
        match manager.create_session(project_path).await {
            Ok(session_id) => {
                info!("Session created successfully with ID: {}", session_id);
                Ok(CommandResult::success(session_id))
            },
            Err(e) => {
                info!("Error creating session: {}", e);
                Ok(CommandResult::error(e.to_string()))
            },
        }
    } else {
        info!("Manager not initialized");
        Ok(CommandResult::error("Claude Code not initialized. Please discover Claude first.".to_string()))
    }
}

#[tauri::command]
async fn send_message(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    info!("send_message called - session_id: {}, message: {}", session_id, message);
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        info!("Manager found, sending message...");
        match manager.send_message(&session_id, message).await {
            Ok(_) => {
                info!("Message sent successfully");
                Ok(CommandResult::success(()))
            },
            Err(e) => {
                info!("Error sending message: {}", e);
                Ok(CommandResult::error(e.to_string()))
            },
        }
    } else {
        info!("Manager not initialized");
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

#[tauri::command]
fn get_project_directory() -> Result<CommandResult<String>, String> {
    // Try to find git repository root first
    if let Ok(output) = std::process::Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !git_root.is_empty() {
                return Ok(CommandResult::success(git_root));
            }
        }
    }
    
    // Fall back to current directory if not in a git repo
    match std::env::current_dir() {
        Ok(path) => Ok(CommandResult::success(path.to_string_lossy().to_string())),
        Err(e) => Ok(CommandResult::error(e.to_string())),
    }
}

#[tauri::command]
async fn handle_claude_event(
    event_type: String,
    _payload: serde_json::Value,
    _state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    // This command is for future use when we need the frontend to send specific events
    // For now, it's a placeholder that can be extended
    match event_type.as_str() {
        "claude:send_message" => {
            // The existing send_message command already handles this
            Ok(CommandResult::success(()))
        }
        _ => Ok(CommandResult::error(format!("Unknown event type: {}", event_type)))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging with info level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("OpenAgents starting up...");
    
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
            get_project_directory,
            handle_claude_event,
        ])
        .setup(|app| {
            // During development, try to prevent window from stealing focus on hot reload
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                
                // Store initial focus state
                let window_focused = app.get_webview_window("main")
                    .map(|w| w.is_focused().unwrap_or(false))
                    .unwrap_or(false);
                
                // If window wasn't focused initially, try to minimize focus disruption
                if !window_focused {
                    if let Some(window) = app.get_webview_window("main") {
                        // Set window to not be always on top
                        let _ = window.set_always_on_top(false);
                        
                        // Set skip taskbar to false to ensure normal window behavior
                        let _ = window.set_skip_taskbar(false);
                    }
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
