use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, State};
use tokio::time::timeout;

pub(crate) use crate::backend::app_server::WorkspaceSession;
use crate::backend::app_server::{
    build_codex_command_with_bin, build_codex_path_env, check_codex_installation,
    spawn_workspace_session as spawn_workspace_session_inner,
};
use crate::codex_home::resolve_workspace_codex_home;
use crate::event_sink::TauriEventSink;
use crate::contracts::ipc::{
    AccountRateLimitsResponse,
    CodexDoctorResponse,
    CurrentDirectory,
    ListModelsResponse,
    ListThreadsResponse,
    ResumeThreadResponse,
    SendUserMessageResponse,
    StartThreadResponse,
    TestCodexConnectionResponse,
    WorkspaceConnectionResponse,
    WorkspaceConnectionStatusResponse,
    SetFullAutoResponse,
};
use crate::full_auto::{FullAutoMap, FullAutoState, DEFAULT_CONTINUE_PROMPT};
use crate::state::AppState;
use crate::types::WorkspaceEntry;

pub(crate) async fn spawn_workspace_session(
    entry: WorkspaceEntry,
    default_codex_bin: Option<String>,
    app_handle: AppHandle,
    codex_home: Option<PathBuf>,
    full_auto: FullAutoMap,
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
        full_auto,
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

    let session = spawn_workspace_session(
        entry.clone(),
        default_bin,
        app,
        None,
        state.full_auto.clone(),
    )
    .await?;

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

    let path = PathBuf::from(&workspace_path);
    if !path.exists() || !path.is_dir() {
        return Ok(WorkspaceConnectionResponse {
            success: false,
            message: format!("Working directory not found: {}", workspace_path),
            workspace_id,
        });
    }

    let entry = WorkspaceEntry {
        id: workspace_id.clone(),
        name: path
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

    let session = spawn_workspace_session(
        entry.clone(),
        default_bin,
        app.clone(),
        codex_home,
        state.full_auto.clone(),
    )
    .await?;

    state
        .sessions
        .lock()
        .await
        .insert(workspace_id.clone(), session);

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

    {
        let mut full_auto = state.full_auto.lock().await;
        full_auto.remove(&workspace_id);
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
    let full_auto_enabled = {
        let full_auto = state.full_auto.lock().await;
        full_auto
            .get(&workspace_id)
            .map(|config| config.enabled)
            .unwrap_or(false)
    };
    let params = json!({
        "cwd": session.entry.path,
        "approvalPolicy": if full_auto_enabled { "never" } else { "on-request" }
    });
    
    session
        .send_request("thread/start", params)
        .await
        .map(StartThreadResponse)
}

#[tauri::command]
pub(crate) async fn list_threads(
    workspace_id: String,
    cursor: Option<String>,
    limit: Option<u32>,
    sort_key: Option<String>,
    archived: Option<bool>,
    state: State<'_, AppState>,
    _app: AppHandle,
) -> Result<ListThreadsResponse, String> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&workspace_id)
        .ok_or("workspace not connected")?;
    let params = json!({
        "cursor": cursor,
        "limit": limit,
        "sortKey": sort_key,
        "archived": archived,
    });

    session
        .send_request("thread/list", params)
        .await
        .map(ListThreadsResponse)
}

#[tauri::command]
pub(crate) async fn resume_thread(
    workspace_id: String,
    thread_id: String,
    state: State<'_, AppState>,
    _app: AppHandle,
) -> Result<ResumeThreadResponse, String> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&workspace_id)
        .ok_or("workspace not connected")?;
    let params = json!({
        "threadId": thread_id,
    });

    session
        .send_request("thread/resume", params)
        .await
        .map(ResumeThreadResponse)
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
    let full_auto_enabled = {
        let full_auto = state.full_auto.lock().await;
        full_auto
            .get(&workspace_id)
            .map(|config| config.enabled)
            .unwrap_or(false)
    };
    let access_mode = if full_auto_enabled {
        "full-access".to_string()
    } else {
        access_mode.unwrap_or_else(|| "current".to_string())
    };
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
    
    session
        .send_request("turn/start", params)
        .await
        .map(SendUserMessageResponse)
}

#[tauri::command]
pub(crate) async fn set_full_auto(
    workspace_id: String,
    enabled: bool,
    thread_id: Option<String>,
    continue_prompt: Option<String>,
    state: State<'_, AppState>,
) -> Result<SetFullAutoResponse, String> {
    let mut map = state.full_auto.lock().await;

    if enabled {
        let entry = map
            .entry(workspace_id.clone())
            .or_insert_with(|| FullAutoState::new(thread_id.clone(), continue_prompt.clone()));
        entry.enabled = true;
        if let Some(thread_id) = thread_id {
            entry.thread_id = Some(thread_id);
        }
        entry.set_continue_prompt(continue_prompt);
        return Ok(SetFullAutoResponse {
            workspace_id,
            enabled: true,
            thread_id: entry.thread_id.clone(),
            continue_prompt: entry.continue_prompt.clone(),
        });
    }

    map.remove(&workspace_id);

    Ok(SetFullAutoResponse {
        workspace_id,
        enabled: false,
        thread_id: None,
        continue_prompt: DEFAULT_CONTINUE_PROMPT.to_string(),
    })
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
