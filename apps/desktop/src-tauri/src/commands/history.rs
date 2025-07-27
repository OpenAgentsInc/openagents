//! History-related Tauri commands

use log::info;
use tauri::State;
use crate::claude_code::{ClaudeConversation, UnifiedSession};
use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub async fn get_history(
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
pub async fn get_unified_history(
    limit: usize,
    user_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<UnifiedSession>>, String> {
    info!("get_unified_history called with params:");
    info!("  limit: {}", limit);
    info!("  user_id: {:?}", user_id);
    
    let mut discovery = state.discovery.lock().await;
    
    match discovery.load_unified_sessions(limit, user_id).await {
        Ok(sessions) => Ok(CommandResult::success(sessions)),
        Err(e) => Ok(CommandResult::error(e.to_string())),
    }
}