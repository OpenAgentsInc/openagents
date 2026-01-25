//! Tauri commands for unified agent interface

use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::broadcast;

use crate::agent::unified::AgentId;
use crate::contracts::ipc::{
    ConnectUnifiedAgentResponse,
    DisconnectUnifiedAgentResponse,
    GetUnifiedAgentStatusResponse,
    GetUnifiedConversationItemsResponse,
    SendUnifiedMessageResponse,
    StartUnifiedSessionResponse,
};
use crate::state::AppState;

/// Connect an agent to a workspace using the unified interface
#[tauri::command]
pub(crate) async fn connect_unified_agent(
    agent_id_str: String,
    workspace_path: String,
    workspace_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ConnectUnifiedAgentResponse, String> {
    let agent_id = AgentId::from_str(&agent_id_str)
        .ok_or_else(|| format!("Invalid agent ID: {}", agent_id_str))?;
    
    let workspace_path = Path::new(&workspace_path);
    // For now, use default codex_home resolution
    // TODO: Properly resolve workspace-specific codex_home
    let codex_home = crate::codex_home::resolve_default_codex_home();
    
    let agent_manager = state.agent_manager.lock().await;
    let session_id = agent_manager
        .connect_agent(
            agent_id,
            workspace_path,
            workspace_id.clone(),
            app.clone(),
            codex_home,
        )
        .await?;
    
    // Set up event forwarding from unified stream to Tauri events (once per app).
    if !state
        .unified_forwarder_started
        .swap(true, Ordering::SeqCst)
    {
        let unified_rx = agent_manager.get_unified_events_receiver();
        let app_clone = app.clone();

        tokio::spawn(async move {
            let mut rx = unified_rx;
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        // Emit unified event to frontend
                        let _ = app_clone.emit("unified-event", &event);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        eprintln!("Unified event channel closed");
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!("Unified event channel lagged by {} messages", n);
                        // Continue receiving
                    }
                }
            }
        });
    }
    
    Ok(ConnectUnifiedAgentResponse {
        success: true,
        session_id: session_id.clone(),
        agent_id: agent_id_str,
        workspace_id,
    })
}

/// Disconnect an agent session
#[tauri::command]
pub(crate) async fn disconnect_unified_agent(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<DisconnectUnifiedAgentResponse, String> {
    let agent_manager = state.agent_manager.lock().await;
    let agent_id = agent_manager
        .get_agent_for_session(&session_id)
        .await
        .ok_or("Session not found")?;
    
    let agent = agent_manager
        .get_agent(agent_id)
        .await
        .ok_or("Agent not found")?;
    
    agent.disconnect(&session_id).await?;
    
    Ok(DisconnectUnifiedAgentResponse {
        success: true,
        session_id,
    })
}

/// Start a new session/thread
#[tauri::command]
pub(crate) async fn start_unified_session(
    session_id: String,
    workspace_path: String,
    state: State<'_, AppState>,
) -> Result<StartUnifiedSessionResponse, String> {
    let agent_manager = state.agent_manager.lock().await;
    let agent_id = agent_manager
        .get_agent_for_session(&session_id)
        .await
        .ok_or("Session not found")?;
    
    let agent = agent_manager
        .get_agent(agent_id)
        .await
        .ok_or("Agent not found")?;
    
    let cwd = Path::new(&workspace_path);
    agent.start_session(&session_id, cwd).await?;
    
    Ok(StartUnifiedSessionResponse {
        success: true,
        session_id,
    })
}

/// Send a message to an agent session
#[tauri::command]
pub(crate) async fn send_unified_message(
    session_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<SendUnifiedMessageResponse, String> {
    eprintln!("send_unified_message called: session_id={}, text_len={}", session_id, text.len());
    
    let agent_manager = state.agent_manager.lock().await;
    
    // Try to find agent by session_id (could be workspace_id or actual ACP session ID)
    let agent_id = agent_manager
        .get_agent_for_session(&session_id)
        .await;
    
    eprintln!("Agent lookup result: {:?}", agent_id);
    
    // If not found by session_id, try to find by agent_id directly (for Codex, there's only one)
    let agent_id = agent_id.unwrap_or(crate::agent::unified::AgentId::Codex);
    
    eprintln!("Using agent_id: {:?}", agent_id);
    
    let agent = agent_manager
        .get_agent(agent_id)
        .await
        .ok_or("Agent not found")?;
    
    eprintln!("Agent found, calling send_message...");
    
    // The agent's send_message will use the actual ACP session ID from the connection
    agent.send_message(&session_id, text).await?;
    
    eprintln!("Message sent successfully");
    
    Ok(SendUnifiedMessageResponse {
        success: true,
        session_id,
    })
}

/// Get conversation items for a session
#[tauri::command]
pub(crate) async fn get_unified_conversation_items(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<GetUnifiedConversationItemsResponse, String> {
    let agent_manager = state.agent_manager.lock().await;
    let agent_id = agent_manager
        .get_agent_for_session(&session_id)
        .await
        .ok_or("Session not found")?;
    
    let agent = agent_manager
        .get_agent(agent_id)
        .await
        .ok_or("Agent not found")?;
    
    let items = agent.get_conversation_items(&session_id).await?;
    
    Ok(GetUnifiedConversationItemsResponse {
        success: true,
        session_id,
        items,
    })
}

/// Get agent status for a session
#[tauri::command]
pub(crate) async fn get_unified_agent_status(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<GetUnifiedAgentStatusResponse, String> {
    let agent_manager = state.agent_manager.lock().await;
    let agent_id = agent_manager
        .get_agent_for_session(&session_id)
        .await;
    
    Ok(GetUnifiedAgentStatusResponse {
        session_id,
        agent_id: agent_id.map(|id| id.as_str().to_string()),
        connected: agent_id.is_some(),
    })
}
