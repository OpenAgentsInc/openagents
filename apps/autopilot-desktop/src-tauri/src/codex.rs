use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, State};
use tokio::time::timeout;

pub(crate) use crate::backend::app_server::WorkspaceSession;
use crate::backend::app_server::{
    build_codex_command_with_bin, build_codex_path_env, check_codex_installation,
    spawn_workspace_session as spawn_workspace_session_inner,
};
use crate::acp::AcpConnection;
use crate::codex_home::resolve_workspace_codex_home;
use crate::event_sink::TauriEventSink;
use crate::contracts::ipc::{
    AccountRateLimitsResponse,
    CodexDoctorResponse,
    CurrentDirectory,
    ListModelsResponse,
    SendUserMessageResponse,
    StartThreadResponse,
    TestCodexConnectionResponse,
    WorkspaceConnectionResponse,
    WorkspaceConnectionStatusResponse,
};
use crate::state::AppState;
use crate::types::WorkspaceEntry;

pub(crate) async fn spawn_workspace_session(
    entry: WorkspaceEntry,
    default_codex_bin: Option<String>,
    app_handle: AppHandle,
    codex_home: Option<PathBuf>,
) -> Result<Arc<WorkspaceSession>, String> {
    let client_version = app_handle.package_info().version.to_string();
    let event_sink = TauriEventSink::new(app_handle)
        .await
        .map_err(|e| format!("Failed to create event sink: {}", e))?;
    spawn_workspace_session_inner(
        entry,
        default_codex_bin,
        client_version,
        event_sink,
        codex_home,
    )
    .await
}

#[tauri::command]
pub(crate) async fn codex_doctor(
    codex_bin: Option<String>,
    state: State<'_, AppState>,
) -> Result<CodexDoctorResponse, String> {
    let default_bin = {
        let settings = state.app_settings.lock().await;
        settings.codex_bin.clone()
    };
    let resolved = codex_bin
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or(default_bin);
    let path_env = build_codex_path_env(resolved.as_deref());
    let version = check_codex_installation(resolved.clone()).await?;
    let mut command = build_codex_command_with_bin(resolved.clone());
    command.arg("app-server");
    command.arg("--help");
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    let app_server_ok = match timeout(Duration::from_secs(5), command.output()).await {
        Ok(result) => result
            .map(|output| output.status.success())
            .unwrap_or(false),
        Err(_) => false,
    };
    let details = if app_server_ok {
        None
    } else {
        Some("Failed to run `codex app-server --help`.".to_string())
    };
    Ok(CodexDoctorResponse {
        ok: version.is_some() && app_server_ok,
        codex_bin: resolved,
        version,
        app_server_ok,
        details,
        path: path_env,
    })
}

#[tauri::command]
pub(crate) async fn test_codex_connection(
    workspace_path: String,
    codex_bin: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<TestCodexConnectionResponse, String> {
    use uuid::Uuid;

    let entry = WorkspaceEntry {
        id: Uuid::new_v4().to_string(),
        name: "Test Workspace".to_string(),
        path: workspace_path,
        codex_bin: codex_bin.clone(),
    };

    let default_bin = {
        let settings = state.app_settings.lock().await;
        settings.codex_bin.clone()
    };

    let session = spawn_workspace_session(entry.clone(), default_bin, app, None).await?;

    // Test by calling model/list
    let result = session.send_request("model/list", json!({})).await;

    // Clean up
    {
        let mut child = session.child.lock().await;
        let _ = child.kill().await;
    }

    match result {
        Ok(models) => Ok(TestCodexConnectionResponse {
            success: true,
            message: "Successfully connected to Codex".to_string(),
            models: Some(models),
        }),
        Err(e) => Ok(TestCodexConnectionResponse {
            success: false,
            message: format!("Connected but failed to list models: {}", e),
            models: None,
        }),
    }
}

#[tauri::command]
pub(crate) async fn connect_workspace(
    workspace_id: String,
    workspace_path: String,
    codex_bin: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<WorkspaceConnectionResponse, String> {

    // Check if already connected
    {
        let sessions = state.sessions.lock().await;
        if sessions.contains_key(&workspace_id) {
            return Ok(WorkspaceConnectionResponse {
                success: true,
                message: "Workspace already connected".to_string(),
                workspace_id,
            });
        }
    }

    let entry = WorkspaceEntry {
        id: workspace_id.clone(),
        name: PathBuf::from(&workspace_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Workspace")
            .to_string(),
        path: workspace_path,
        codex_bin: codex_bin.clone(),
    };

    let default_bin = {
        let settings = state.app_settings.lock().await;
        settings.codex_bin.clone()
    };

    let codex_home = resolve_workspace_codex_home(&entry, None);
    let codex_home_for_acp = codex_home.clone();
    
    // Start ACP connection FIRST so we can mirror requests to it
    let workspace_path = std::path::Path::new(&entry.path);
    let app_for_acp = app.clone();
    let workspace_id_for_acp = workspace_id.clone();
    let (command, args, env) = crate::agent::resolver::resolve_codex_config(codex_home_for_acp).await
        .map_err(|e| format!("Failed to resolve Codex for ACP: {}", e))?;

    let acp_conn_result = AcpConnection::new(
        workspace_id.clone(),
        workspace_path,
        command,
        args,
        env,
        app.clone()
    ).await;
    
    let session = spawn_workspace_session(entry.clone(), default_bin, app.clone(), codex_home).await?;

    state
        .sessions
        .lock()
        .await
        .insert(workspace_id.clone(), session);

    // Store ACP connection if it succeeded
    match acp_conn_result {
        Ok(acp_conn) => {
            let acp_conn_arc = Arc::new(acp_conn);
            
            // Send initialized notification to ACP (after codex app-server is initialized)
            if let Err(e) = acp_conn_arc.send_notification("initialized", None).await {
                eprintln!("Failed to send initialized to ACP: {}", e);
            }
            
            state
                .acp_connections
                .lock()
                .await
                .insert(workspace_id.clone(), acp_conn_arc);
            eprintln!("ACP connection started for workspace: {}", workspace_id);
        }
        Err(e) => {
            eprintln!("Failed to start ACP connection (non-fatal): {}", e);
            // Emit an error event so the user knows ACP isn't working
            let error_event = crate::acp::AcpEvent {
                workspace_id: workspace_id_for_acp.clone(),
                message: serde_json::json!({
                    "type": "acp/error",
                    "error": e.to_string(),
                    "message": "codex-acp not found or failed to start. Install from https://github.com/zed-industries/codex-acp",
                }),
            };
            let _ = app_for_acp.emit("acp-event", error_event);
            // Don't fail the whole connection if ACP fails
        }
    }

    Ok(WorkspaceConnectionResponse {
        success: true,
        message: "Workspace connected successfully".to_string(),
        workspace_id,
    })
}

#[tauri::command]
pub(crate) async fn disconnect_workspace(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<WorkspaceConnectionResponse, String> {
    let session = {
        let mut sessions = state.sessions.lock().await;
        sessions.remove(&workspace_id)
    };

    if let Some(session) = session {
        let mut child = session.child.lock().await;
        let _ = child.kill().await;
    }
    
    // Also disconnect ACP connection
    let acp_conn = {
        let mut acp_connections = state.acp_connections.lock().await;
        acp_connections.remove(&workspace_id)
    };
    
    if let Some(acp_conn) = acp_conn {
        if let Err(e) = acp_conn.kill().await {
            eprintln!("Failed to kill ACP connection: {}", e);
        }
    }

    // Note: Events will be flushed when completion is detected (turn/completed event)
    // Buffered events will be written when the completion event is received

    Ok(WorkspaceConnectionResponse {
        success: true,
        message: "Workspace disconnected successfully".to_string(),
        workspace_id,
    })
}

#[tauri::command]
pub(crate) async fn get_workspace_connection_status(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<WorkspaceConnectionStatusResponse, String> {
    let connected = {
        let sessions = state.sessions.lock().await;
        sessions.contains_key(&workspace_id)
    };

    Ok(WorkspaceConnectionStatusResponse {
        workspace_id,
        connected,
    })
}

#[tauri::command]
pub(crate) async fn start_thread(
    workspace_id: String,
    state: State<'_, AppState>,
    _app: AppHandle,
) -> Result<StartThreadResponse, String> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&workspace_id)
        .ok_or("workspace not connected")?;
    let params = json!({
        "cwd": session.entry.path,
        "approvalPolicy": "on-request"
    });
    
    // Also send to ACP (with proper ACP format)
    // Note: We send session/new but don't wait for the response here
    // The session ID will be extracted from the response in the stdout capture
    let acp_conn = {
        let acp_connections = state.acp_connections.lock().await;
        acp_connections.get(&workspace_id).cloned()
    };
    if let Some(acp_conn) = acp_conn {
        // ACP requires mcpServers field (can be empty)
        let acp_params = json!({
            "cwd": session.entry.path,
            "mcpServers": [],
        });
        if let Err(e) = acp_conn.send_request("session/new", acp_params).await {
            eprintln!("Failed to send session/new to ACP: {}", e);
        }
        // Give ACP a moment to respond with session ID
        // (In a real implementation, we'd wait for the response, but for Phase 1 this is fine)
    }
    
    session
        .send_request("thread/start", params)
        .await
        .map(StartThreadResponse)
}

#[tauri::command]
pub(crate) async fn send_user_message(
    workspace_id: String,
    thread_id: String,
    text: String,
    model: Option<String>,
    access_mode: Option<String>,
    state: State<'_, AppState>,
    _app: AppHandle,
) -> Result<SendUserMessageResponse, String> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&workspace_id)
        .ok_or("workspace not connected")?;
    let access_mode = access_mode.unwrap_or_else(|| "current".to_string());
    let sandbox_policy = match access_mode.as_str() {
        "full-access" => json!({
            "type": "dangerFullAccess"
        }),
        "read-only" => json!({
            "type": "readOnly"
        }),
        _ => json!({
            "type": "workspaceWrite",
            "writableRoots": [session.entry.path],
            "networkAccess": true
        }),
    };

    let approval_policy = if access_mode == "full-access" {
        "never"
    } else {
        "on-request"
    };

    let trimmed_text = text.trim();
    if trimmed_text.is_empty() {
        return Err("empty user message".to_string());
    }

    let input = vec![json!({ "type": "text", "text": trimmed_text })];

    let params = json!({
        "threadId": thread_id,
        "input": input,
        "cwd": session.entry.path,
        "approvalPolicy": approval_policy,
        "sandboxPolicy": sandbox_policy,
        "model": model,
    });
    
    // Also send to ACP (convert to ACP format)
    let acp_conn = {
        let acp_connections = state.acp_connections.lock().await;
        acp_connections.get(&workspace_id).cloned()
    };
    if let Some(acp_conn) = acp_conn {
        // Get the ACP session ID (different from thread_id)
        // Try a few times with a small delay in case the session/new response hasn't arrived yet
        let mut acp_session_id = None;
        for _ in 0..5 {
            acp_session_id = acp_conn.get_session_id().await;
            if acp_session_id.is_some() {
                break;
            }
            // Wait a bit for the session/new response to arrive
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        if let Some(acp_session_id) = acp_session_id {
            // Convert to ACP format: session/prompt
            // ACP uses "prompt" not "input", and sessionId not threadId
            let acp_params = json!({
                "sessionId": acp_session_id,
                "prompt": input,  // ACP uses "prompt" field
            });
            if let Err(e) = acp_conn.send_request("session/prompt", acp_params).await {
                eprintln!("Failed to send session/prompt to ACP: {}", e);
            }
        } else {
            eprintln!("ACP session ID not available after waiting, skipping prompt to ACP");
        }
    }
    
    session
        .send_request("turn/start", params)
        .await
        .map(SendUserMessageResponse)
}

#[tauri::command]
pub(crate) async fn get_current_directory() -> Result<CurrentDirectory, String> {
    std::env::current_dir()
        .map(|path| CurrentDirectory(path.to_string_lossy().to_string()))
        .map_err(|e| format!("Failed to get current directory: {}", e))
}

#[tauri::command]
pub(crate) async fn account_rate_limits(
    workspace_id: String,
    state: State<'_, AppState>,
    _app: AppHandle,
) -> Result<AccountRateLimitsResponse, String> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&workspace_id)
        .ok_or("workspace not connected")?;
    
    // Don't send to ACP - this method doesn't exist in ACP protocol
    // ACP doesn't have account/rateLimits/read
    
    session
        .send_request("account/rateLimits/read", json!(null))
        .await
        .map(AccountRateLimitsResponse)
}

#[tauri::command]
pub(crate) async fn list_models(
    workspace_id: String,
    state: State<'_, AppState>,
    _app: AppHandle,
) -> Result<ListModelsResponse, String> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&workspace_id)
        .ok_or("workspace not connected")?;
    
    // Don't send to ACP - model/list doesn't exist in ACP protocol
    // Models are returned in session/new response or via other ACP methods
    
    session
        .send_request("model/list", json!({}))
        .await
        .map(ListModelsResponse)
}
