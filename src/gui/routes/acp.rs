//! ACP (Agent Client Protocol) session management routes
//!
//! REST API for managing AI agent sessions:
//! - GET /api/acp/sessions - List active sessions
//! - POST /api/acp/sessions - Create session
//! - POST /api/acp/sessions/{id}/prompt - Send prompt
//! - POST /api/acp/sessions/{id}/cancel - Cancel session
//! - DELETE /api/acp/sessions/{id} - Close session

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::gui::state::AppState;

/// Configure ACP API routes
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("/sessions", web::get().to(list_sessions))
        .route("/sessions", web::post().to(create_session))
        .route("/sessions/{id}/prompt", web::post().to(send_prompt))
        .route("/sessions/{id}/cancel", web::post().to(cancel_session))
        .route("/sessions/{id}", web::delete().to(delete_session))
        .route("/sessions/{id}", web::get().to(get_session));
}

/// Request to create a new session
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSessionRequest {
    /// Agent type: "claude" or "codex"
    pub agent: String,
    /// Working directory for the session
    #[serde(default)]
    pub cwd: Option<String>,
    /// Initial mode (e.g., "code", "plan", "ask")
    #[serde(default)]
    pub mode: Option<String>,
}

/// Request to send a prompt
#[derive(Debug, Clone, Deserialize)]
pub struct PromptRequest {
    /// The prompt content
    pub content: String,
}

/// Session entry for UI display
#[derive(Debug, Clone, Serialize)]
pub struct SessionEntry {
    /// Entry type: "user", "assistant", "tool", "thought", "plan"
    pub entry_type: String,
    /// Content of the entry
    pub content: String,
    /// Optional tool name (for tool entries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Optional tool status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_status: Option<String>,
    /// Timestamp
    pub timestamp: String,
}

/// ACP session info returned by API
#[derive(Debug, Clone, Serialize)]
pub struct AcpSessionInfo {
    /// Session ID
    pub id: String,
    /// Agent type
    pub agent: String,
    /// Current status: "active", "completed", "cancelled", "error"
    pub status: String,
    /// Current mode ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Working directory
    pub cwd: String,
    /// Session entries for display
    pub entries: Vec<SessionEntry>,
    /// Created timestamp
    pub created_at: String,
    /// Last activity timestamp
    pub last_activity: String,
}

/// GET /api/acp/sessions - List all active sessions
async fn list_sessions(state: web::Data<AppState>) -> HttpResponse {
    let sessions = state.acp_sessions.read().await;
    let session_list: Vec<&AcpSessionInfo> = sessions.values().collect();
    HttpResponse::Ok().json(session_list)
}

/// GET /api/acp/sessions/{id} - Get a specific session
async fn get_session(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let session_id = path.into_inner();
    let sessions = state.acp_sessions.read().await;

    if let Some(session) = sessions.get(&session_id) {
        HttpResponse::Ok().json(session)
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "error": "Session not found",
            "session_id": session_id
        }))
    }
}

/// POST /api/acp/sessions - Create a new session
async fn create_session(
    state: web::Data<AppState>,
    body: web::Json<CreateSessionRequest>,
) -> HttpResponse {
    // Validate agent type
    if body.agent != "claude" && body.agent != "codex" {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid agent type",
            "message": "Agent must be 'claude' or 'codex'"
        }));
    }

    // Generate session ID
    let session_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Determine working directory
    let cwd = body
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap().to_string_lossy().to_string());

    // Create session info
    let session = AcpSessionInfo {
        id: session_id.clone(),
        agent: body.agent.clone(),
        status: "active".to_string(),
        mode: body.mode.clone(),
        cwd,
        entries: Vec::new(),
        created_at: now.clone(),
        last_activity: now,
    };

    // Store session
    {
        let mut sessions = state.acp_sessions.write().await;
        sessions.insert(session_id.clone(), session.clone());
    }

    // Broadcast session created event
    state.broadcaster.broadcast(&format!(
        r#"<div id="acp-session-list" hx-swap-oob="innerHTML">{}</div>"#,
        render_session_list_html(&state).await
    ));

    tracing::info!(
        session_id = %session_id,
        agent = %body.agent,
        "ACP session created"
    );

    HttpResponse::Created().json(session)
}

/// POST /api/acp/sessions/{id}/prompt - Send a prompt to a session
async fn send_prompt(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<PromptRequest>,
) -> HttpResponse {
    let session_id = path.into_inner();
    let now = chrono::Utc::now().to_rfc3339();

    // Check if session exists
    {
        let sessions = state.acp_sessions.read().await;
        if !sessions.contains_key(&session_id) {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "Session not found",
                "session_id": session_id
            }));
        }

        let session = sessions.get(&session_id).unwrap();
        if session.status != "active" {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Session not active",
                "status": session.status
            }));
        }
    }

    // Add user message entry
    {
        let mut sessions = state.acp_sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.entries.push(SessionEntry {
                entry_type: "user".to_string(),
                content: body.content.clone(),
                tool_name: None,
                tool_status: None,
                timestamp: now.clone(),
            });
            session.last_activity = now;
        }
    }

    // Broadcast update
    broadcast_session_update(&state, &session_id).await;

    tracing::info!(
        session_id = %session_id,
        content_len = body.content.len(),
        "Prompt sent to ACP session"
    );

    // TODO: Actually send prompt to agent via ACP connection
    // For now, return acknowledgment
    // In full implementation, this would:
    // 1. Get the AcpAgentConnection for this session
    // 2. Call connection.prompt(session_id, content)
    // 3. Stream updates back via WebSocket

    HttpResponse::Ok().json(serde_json::json!({
        "status": "prompt_received",
        "session_id": session_id,
        "message": "Prompt queued for processing"
    }))
}

/// POST /api/acp/sessions/{id}/cancel - Cancel ongoing work in a session
async fn cancel_session(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let session_id = path.into_inner();

    // Update session status
    {
        let mut sessions = state.acp_sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            if session.status == "active" {
                session.status = "cancelled".to_string();
                session.last_activity = chrono::Utc::now().to_rfc3339();
            } else {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Session not active",
                    "status": session.status
                }));
            }
        } else {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "Session not found",
                "session_id": session_id
            }));
        }
    }

    // Broadcast update
    broadcast_session_update(&state, &session_id).await;

    tracing::info!(session_id = %session_id, "ACP session cancelled");

    // TODO: Actually send cancel to agent via ACP connection
    // In full implementation: connection.cancel(session_id)

    HttpResponse::Ok().json(serde_json::json!({
        "status": "cancelled",
        "session_id": session_id
    }))
}

/// DELETE /api/acp/sessions/{id} - Close and remove a session
async fn delete_session(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let session_id = path.into_inner();

    // Remove session
    let removed = {
        let mut sessions = state.acp_sessions.write().await;
        sessions.remove(&session_id)
    };

    if removed.is_some() {
        // Broadcast session list update
        state.broadcaster.broadcast(&format!(
            r#"<div id="acp-session-list" hx-swap-oob="innerHTML">{}</div>"#,
            render_session_list_html(&state).await
        ));

        tracing::info!(session_id = %session_id, "ACP session deleted");

        HttpResponse::Ok().json(serde_json::json!({
            "status": "deleted",
            "session_id": session_id
        }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "error": "Session not found",
            "session_id": session_id
        }))
    }
}

/// Broadcast a session update via WebSocket
async fn broadcast_session_update(state: &web::Data<AppState>, session_id: &str) {
    let sessions = state.acp_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        // Broadcast session entries update for HTMX OOB swap
        let entries_html = render_session_entries_html(&session.entries);
        state.broadcaster.broadcast(&format!(
            r#"<div id="acp-session-{}-entries" hx-swap-oob="innerHTML">{}</div>"#,
            session_id, entries_html
        ));
    }
}

/// Render session list HTML for HTMX updates
async fn render_session_list_html(state: &web::Data<AppState>) -> String {
    let sessions = state.acp_sessions.read().await;

    if sessions.is_empty() {
        return r#"<p class="text-muted">No active sessions</p>"#.to_string();
    }

    let mut html = String::new();
    for session in sessions.values() {
        let status_class = match session.status.as_str() {
            "active" => "status-active",
            "completed" => "status-completed",
            "cancelled" => "status-cancelled",
            _ => "status-error",
        };

        html.push_str(&format!(
            r#"<div class="session-item" data-session-id="{}">
                <div class="session-header">
                    <span class="session-agent">{}</span>
                    <span class="session-status {}">{}</span>
                </div>
                <div class="session-id">{}</div>
            </div>"#,
            session.id,
            session.agent,
            status_class,
            session.status,
            &session.id[..8]
        ));
    }

    html
}

/// Render session entries HTML for display
fn render_session_entries_html(entries: &[SessionEntry]) -> String {
    if entries.is_empty() {
        return r#"<p class="text-muted">No messages yet</p>"#.to_string();
    }

    let mut html = String::new();
    for entry in entries {
        let entry_class = match entry.entry_type.as_str() {
            "user" => "entry-user",
            "assistant" => "entry-assistant",
            "tool" => "entry-tool",
            "thought" => "entry-thought",
            "plan" => "entry-plan",
            _ => "entry-other",
        };

        let tool_info = if let Some(name) = &entry.tool_name {
            let status = entry.tool_status.as_deref().unwrap_or("running");
            format!(r#"<span class="tool-info">{} [{}]</span>"#, name, status)
        } else {
            String::new()
        };

        html.push_str(&format!(
            r#"<div class="entry {}">
                <div class="entry-header">
                    <span class="entry-type">{}</span>
                    {}
                    <span class="entry-time">{}</span>
                </div>
                <div class="entry-content">{}</div>
            </div>"#,
            entry_class,
            entry.entry_type,
            tool_info,
            &entry.timestamp[11..19], // HH:MM:SS
            html_escape(&entry.content)
        ));
    }

    html
}

/// HTML escape helper
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
