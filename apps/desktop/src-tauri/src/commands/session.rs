//! Session-related Tauri commands

use log::info;
use tauri::State;
use crate::claude_code::{ClaudeManager, Message};
use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub async fn discover_claude(state: State<'_, AppState>) -> Result<CommandResult<String>, String> {
    let mut discovery = state.discovery.lock().await;
    
    match discovery.discover_binary().await {
        Ok(path) => {
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
pub async fn create_session(
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
pub async fn send_message(
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
pub async fn trigger_claude_response(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    info!("trigger_claude_response called - session_id: {}, message: {}", session_id, message);
    info!("🚨 [RUST] This command triggers Claude WITHOUT creating a user message!");
    
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        info!("Manager found, triggering Claude response...");
        // This will call a new method that doesn't create a user message
        match manager.trigger_response(&session_id, message).await {
            Ok(_) => {
                info!("Claude response triggered successfully");
                Ok(CommandResult::success(()))
            },
            Err(e) => {
                info!("Error triggering Claude response: {}", e);
                Ok(CommandResult::error(e.to_string()))
            },
        }
    } else {
        info!("Manager not initialized");
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn get_messages(
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
pub async fn stop_session(
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
pub async fn get_active_sessions(
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
pub async fn handle_claude_event(
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