use std::{collections::HashMap, sync::Arc, path::{Path, PathBuf}};

use agent_client_protocol as acp;
use anyhow::Result;
use tokio::sync::{RwLock, Mutex};
use uuid::Uuid;
use once_cell::sync::Lazy;

use super::ACPClient;
use which::which;
use tracing::{debug, info, error};
use crate::codex_exec::{run_codex_exec_once, CodexExecOptions};
use crate::tinyvex_state::TinyvexState;
use crate::tinyvex_ws;
use crate::convex_client;

// Global tracking of streaming item_ids for Convex finalization
// Key: "thread_id|kind" (e.g. "session-123|assistant")
// Value: item_id currently being streamed
static STREAMING_ITEMS: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<SessionManagerInner>,
    tinyvex: Arc<TinyvexState>,
}

struct SessionManagerInner {
    sessions: RwLock<HashMap<acp::SessionId, Session>>,
    clients: RwLock<HashMap<acp::SessionId, Arc<Mutex<ACPClient>>>>,
}

#[derive(Clone)]
pub struct SessionMessage {
    pub id: String,
    pub role: String,
    pub content: Vec<acp::ContentBlock>,
    pub created_at: String,
}

#[derive(Clone)]
pub struct Session {
    pub id: acp::SessionId,
    pub cwd: PathBuf,
    pub messages: Vec<SessionMessage>,
    pub use_codex_exec: bool,
    pub agent_type: String,
}

impl SessionManager {
    pub fn new(tinyvex: Arc<TinyvexState>) -> Self {
        Self {
            inner: Arc::new(SessionManagerInner {
                sessions: RwLock::new(HashMap::new()),
                clients: RwLock::new(HashMap::new())
            }),
            tinyvex,
        }
    }

    pub async fn create_session(&self, agent_type: String, cwd: PathBuf) -> Result<acp::SessionId> {
        // Decide path: ACP via codex-acp/claude-code-acp (default) or legacy codex-exec adapter (env/explicit)
        let use_codex_exec = agent_type == "codex-exec" || std::env::var("OA_USE_CODEX_EXEC").ok().as_deref() == Some("1");
        let (cmd, all_args) = if use_codex_exec {
            // codex-exec path does not use ACP client spawn here
            (PathBuf::from("codex-exec"), Vec::new())
        } else {
            // Route to appropriate agent resolver
            let (cmd, pre) = match agent_type.as_str() {
                "claude-code" => resolve_claude_code_acp_exec()?,
                "codex" | _ => resolve_codex_acp_exec()?,
            };
            let tail = std::env::var("OA_ACP_AGENT_ARGS").unwrap_or_default();
            let mut args: Vec<String> = if tail.trim().is_empty() { vec![] } else { shell_words::split(&tail).unwrap_or_else(|_| tail.split_whitespace().map(|s| s.to_string()).collect()) };
            let mut merged = pre;
            merged.append(&mut args);
            // Ensure Codex ACP runs with approvals/sandbox bypass to avoid UI stalls.
            // Only add the flag for Codex (not Claude Code TS agent) since the TS agent may not recognize it.
            if agent_type != "claude-code" {
                let flag = "--dangerously-bypass-approvals-and-sandbox".to_string();
                if !merged.iter().any(|a| a == &flag) {
                    merged.push(flag);
                }
            }
            (cmd, merged)
        };
        info!(agent_type=%agent_type, bin=%cmd.display(), args=?all_args, cwd=%cwd.display(), use_codex_exec, "create_session resolved");
        let session_id = acp::SessionId(Arc::from(uuid::Uuid::new_v4().to_string()));
        if !use_codex_exec {
            // For TypeScript agents (claude-code), set NODE_PATH to tauri/node_modules
            let mut envs: Vec<(String, String)> = Vec::new();
            // Always export bypass env for Codex ACP; harmless for other agents.
            envs.push((
                "CODEX_ACP_DANGEROUSLY_BYPASS_APPROVALS_AND_SANDBOX".to_string(),
                "1".to_string(),
            ));
            // Back-compat alias used elsewhere in the repo and by older builds.
            envs.push((
                "OA_DANGEROUSLY_BYPASS_APPROVALS_AND_SANDBOX".to_string(),
                "1".to_string(),
            ));
            if agent_type == "claude-code" {
                if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
                    let base = PathBuf::from(manifest_dir)
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("."));
                    let node_modules = base.join("node_modules");
                    if node_modules.exists() {
                        envs.push(("NODE_PATH".to_string(), node_modules.display().to_string()));
                        info!("Setting NODE_PATH={}", node_modules.display());
                    }
                }
            }
            // Log final envs we add for visibility (sanitized list)
            let env_keys: Vec<String> = envs.iter().map(|(k, _)| k.clone()).collect();
            info!(?env_keys, ?all_args, agent_type=%agent_type, "ACP spawn configuration (args/env keys)");
            let mut client = ACPClient::spawn(cmd.to_string_lossy().as_ref(), &all_args, &envs, None).await?;
            let real_sid = client.new_session(cwd.clone()).await?;
            // Start forwarding updates to tinyvex and broadcasting via WebSocket
            let mut rx = client.take_update_receiver();
            let inner = self.inner.clone();
            let tinyvex = self.tinyvex.clone();
            let sid_clone = real_sid.clone();
            let agent_type_clone = agent_type.clone();
            tokio::spawn(async move {
                while let Some(notif) = rx.recv().await {
                    // Update in-memory state
                    let update_copy = notif.update.clone();
                    let mut sessions = inner.sessions.write().await;
                    if let Some(sess) = sessions.get_mut(&notif.session_id) {
                        match &notif.update {
                            acp::SessionUpdate::AgentMessageChunk(chunk) | acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                                let now = chrono::Utc::now().to_rfc3339();
                                if let Some(last) = sess.messages.last_mut() {
                                    if last.role == "assistant" {
                                        last.content.push(chunk.content.clone());
                                    } else {
                                        sess.messages.push(SessionMessage { id: Uuid::new_v4().to_string(), role: "assistant".into(), content: vec![chunk.content.clone()], created_at: now });
                                    }
                                } else {
                                    sess.messages.push(SessionMessage { id: Uuid::new_v4().to_string(), role: "assistant".into(), content: vec![chunk.content.clone()], created_at: now });
                                }
                            }
                            _ => {}
                        }
                    }
                    drop(sessions);

                    // Write to tinyvex database and broadcast to WebSocket clients
                    let thread_id = notif.session_id.0.to_string();
                    let notifications = tinyvex.tinyvex_writer.mirror_acp_update_to_tinyvex(&agent_type_clone, &thread_id, &update_copy).await;

                    // Broadcast each notification to WebSocket clients
                    for notification in notifications {
                        tinyvex_ws::broadcast_writer_notification(&tinyvex, &notification).await;
                    }

                    // Also write to Convex
                    mirror_acp_update_to_convex(&thread_id, &update_copy).await;

                    tracing::info!(
                        target: "openagents_lib::oa_acp::session_manager",
                        session_id = %notif.session_id.0,
                        kind = ?update_copy,
                        "processed ACP update via tinyvex and convex"
                    );
                }
                info!(session_id=%sid_clone.0, "update stream ended");
            });
            // override with real id
            let session = Session { id: real_sid.clone(), cwd: cwd.clone(), messages: vec![], use_codex_exec: false, agent_type: agent_type.clone() };
            self.inner.sessions.write().await.insert(real_sid.clone(), session);
            self.inner.clients.write().await.insert(real_sid.clone(), Arc::new(Mutex::new(client)));

            // Create thread in Convex (skip if not authenticated yet)
            // Thread will be auto-created by Convex when first message is sent
            let thread_id = real_sid.0.to_string();
            let working_dir = cwd.to_string_lossy().to_string();

            // Only create thread if Convex client is initialized (has auth)
            if convex_client::ConvexClientManager::is_initialized().await {
                if let Err(e) = convex_client::ConvexClientManager::create_thread(
                    None, // title - will be set from first message
                    None, // project_id
                    Some(&agent_type),
                    Some(&working_dir),
                ).await {
                    // Don't fail session creation if Convex thread creation fails
                    // (user might not be authenticated yet)
                    info!(?e, thread_id, "skipped creating thread in convex (likely not authenticated)");
                }
            } else {
                info!(thread_id, "convex not initialized, skipping thread creation");
            }

            return Ok(real_sid);
        }

        // For codex-exec path, no ACP client; we spawn per-prompt
        let session = Session { id: session_id.clone(), cwd: cwd.clone(), messages: vec![], use_codex_exec: true, agent_type: agent_type.clone() };
        self.inner.sessions.write().await.insert(session_id.clone(), session);

        // Create thread in Convex for codex-exec too (skip if not authenticated yet)
        let thread_id = session_id.0.to_string();
        let working_dir = cwd.to_string_lossy().to_string();

        if convex_client::ConvexClientManager::is_initialized().await {
            if let Err(e) = convex_client::ConvexClientManager::create_thread(
                None, // title
                None, // project_id
                Some(&agent_type),
                Some(&working_dir),
            ).await {
                info!(?e, thread_id, "skipped creating thread in convex (codex-exec, likely not authenticated)");
            }
        } else {
            info!(thread_id, "convex not initialized, skipping thread creation (codex-exec)");
        }

        Ok(session_id)
    }

    pub async fn prompt(&self, session_id: &acp::SessionId, text: String) -> Result<()> {
        // Finalize any in-progress assistant/reason streams from previous turns for this thread
        {
            let thread_id = session_id.0.to_string();
            let notifs = self.tinyvex.tinyvex_writer.finalize_streaming_for_thread(&thread_id).await;
            for notification in notifs {
                tinyvex_ws::broadcast_writer_notification(&self.tinyvex, &notification).await;
            }

            // Also finalize Convex streaming messages
            finalize_convex_streams(&thread_id).await;
        }
        // push user message
        {
            let mut sessions = self.inner.sessions.write().await;
            if let Some(sess) = sessions.get_mut(session_id) {
                let now = chrono::Utc::now().to_rfc3339();
                sess.messages.push(SessionMessage { id: Uuid::new_v4().to_string(), role: "user".into(), content: vec![acp::ContentBlock::from(text.clone())], created_at: now });
            }
        }
        // Write user message to tinyvex
        {
            let thread_id = session_id.0.to_string();
            let agent_type = {
                let sessions = self.inner.sessions.read().await;
                sessions.get(session_id).map(|s| s.agent_type.clone()).unwrap_or_else(|| "codex".to_string())
            };
            let user_chunk = acp::SessionUpdate::UserMessageChunk(acp::ContentChunk {
                content: acp::ContentBlock::Text(acp::TextContent {
                    annotations: None,
                    text: text.clone(),
                    meta: None,
                }),
                meta: None,
            });
            let notifications = self.tinyvex.tinyvex_writer.mirror_acp_update_to_tinyvex(&agent_type, &thread_id, &user_chunk).await;
            for notification in notifications {
                tinyvex_ws::broadcast_writer_notification(&self.tinyvex, &notification).await;
            }

            // Also write to Convex
            mirror_acp_update_to_convex(&thread_id, &user_chunk).await;
        }

        // send prompt either via ACP client or codex exec adapter
        let use_codex_exec = {
            let sessions = self.inner.sessions.read().await;
            sessions.get(session_id).map(|s| s.use_codex_exec).unwrap_or(false)
        };
        if use_codex_exec {
            let (cwd, sid) = {
                let sessions = self.inner.sessions.read().await;
                let s = sessions.get(session_id).ok_or_else(|| anyhow::anyhow!("session not found"))?;
                (s.cwd.clone(), s.id.clone())
            };
            let bin = resolve_codex_exec_bin()?; // resolve codex/co for exec fallback
            let extra_args = std::env::var("OA_CODEX_ARGS").ok().map(|s| s.split_whitespace().map(|x| x.to_string()).collect()).unwrap_or_else(|| vec!["exec".into(), "--json".into()]);
            let opts = CodexExecOptions { bin, cwd, extra_args };
            let inner = self.inner.clone();
            let tinyvex = self.tinyvex.clone();
            info!(session_id=%sid.0, "codex-exec spawning for prompt");
            tokio::spawn(async move {
                let _ = run_codex_exec_once(&opts, &text, |update| {
                    // Apply update to session state
                    let inner = inner.clone();
                    let sid = sid.clone();
                    let tinyvex = tinyvex.clone();
                    // Blocking write in async task
                    futures::executor::block_on(async move {
                        let update_copy = update.clone();
                        let mut sessions = inner.sessions.write().await;
                        if let Some(sess) = sessions.get_mut(&sid) {
                            debug!(?update, "codex-exec mapped update");
                            match &update {
                                acp::SessionUpdate::AgentMessageChunk(chunk) | acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                                    let now = chrono::Utc::now().to_rfc3339();
                                    if let Some(last) = sess.messages.last_mut() {
                                        if last.role == "assistant" {
                                            last.content.push(chunk.content.clone());
                                            return;
                                        }
                                    }
                                    sess.messages.push(SessionMessage { id: Uuid::new_v4().to_string(), role: "assistant".into(), content: vec![chunk.content.clone()], created_at: now });
                                }
                                acp::SessionUpdate::Plan(_plan) => {
                                    // For Phase 1, store as a thought line summarizing plan
                                    let now = chrono::Utc::now().to_rfc3339();
                                    let summary = "[Plan updated]".to_string();
                                    sess.messages.push(SessionMessage { id: Uuid::new_v4().to_string(), role: "assistant".into(), content: vec![acp::ContentBlock::from(summary)], created_at: now });
                                }
                                _ => {}
                            }
                        }

                        // Write to tinyvex database and broadcast to WebSocket clients
                        let thread_id = sid.0.to_string();
                        let notifications = tinyvex.tinyvex_writer.mirror_acp_update_to_tinyvex("codex-exec", &thread_id, &update_copy).await;

                        // Broadcast each notification to WebSocket clients
                        for notification in notifications {
                            tinyvex_ws::broadcast_writer_notification(&tinyvex, &notification).await;
                        }

                        // Also write to Convex
                        mirror_acp_update_to_convex(&thread_id, &update_copy).await;

                        tracing::info!(
                            target: "openagents_lib::oa_acp::session_manager",
                            session_id = %sid.0,
                            kind = ?update_copy,
                            "processed codex-exec update via tinyvex and convex"
                        );
                    });
                }).await;
            });
        } else {
            let mut clients = self.inner.clients.write().await;
            if let Some(client) = clients.get_mut(session_id) {
                let mut guard = client.lock().await;
                info!(session_id=%session_id.0, "ACP client prompt");
                let _ = guard.prompt(session_id.clone(), text).await;
            }
        }

        Ok(())
    }

    pub async fn get_session(&self, session_id: &acp::SessionId) -> Result<Session> {
        let sessions = self.inner.sessions.read().await;
        let s = sessions.get(session_id).ok_or_else(|| anyhow::anyhow!("session not found"))?.clone();
        Ok(s)
    }

}

// Helper: Extract text from ContentBlock
fn content_to_text(content: &acp::ContentBlock) -> String {
    match content {
        acp::ContentBlock::Text(acp::TextContent { text, .. }) => text.clone(),
        _ => String::new(),
    }
}

// Helper: Finalize all streaming messages for a thread in Convex
async fn finalize_convex_streams(thread_id: &str) {
    let mut streaming = STREAMING_ITEMS.lock().await;

    // Find all item_ids for this thread
    let keys_to_finalize: Vec<String> = streaming
        .keys()
        .filter(|k| k.starts_with(&format!("{}|", thread_id)))
        .cloned()
        .collect();

    for key in keys_to_finalize {
        if let Some(item_id) = streaming.remove(&key) {
            // Finalize this streaming message
            if let Err(e) = convex_client::ConvexClientManager::finalize_message(&item_id).await {
                error!(?e, thread_id, item_id, "failed to finalize message in convex");
            }
        }
    }
}

// Helper: Mirror ACP update to Convex (parallel to Tinyvex)
async fn mirror_acp_update_to_convex(
    thread_id: &str,
    update: &acp::SessionUpdate,
) {
    use acp::SessionUpdate as SU;

    match update {
        SU::ToolCall(tc) => {
            // Finalize any active streams before tool call (matches Tinyvex behavior)
            finalize_convex_streams(thread_id).await;

            let id = format!("{:?}", tc.id);
            let title = Some(tc.title.as_str());
            let kind = Some(format!("{:?}", tc.kind));
            let status = Some(format!("{:?}", tc.status));
            let content_json = serde_json::to_string(&tc.content).ok();
            let locations_json = serde_json::to_string(&tc.locations).ok();

            if let Err(e) = convex_client::ConvexClientManager::upsert_tool_call(
                thread_id,
                &id,
                title,
                kind.as_deref(),
                status.as_deref(),
                content_json.as_deref(),
                locations_json.as_deref(),
            ).await {
                error!(?e, thread_id, tool_call_id=%id, "failed to upsert tool call to convex");
            }
        }
        SU::ToolCallUpdate(tc) => {
            let id = format!("{:?}", tc.id);
            let title = tc.fields.title.as_deref();
            let kind = tc.fields.kind.as_ref().map(|k| format!("{:?}", k));
            let status = tc.fields.status.as_ref().map(|s| format!("{:?}", s));
            let content_json = tc.fields.content.as_ref()
                .and_then(|c| serde_json::to_string(c).ok());
            let locations_json = tc.fields.locations.as_ref()
                .and_then(|l| serde_json::to_string(l).ok());

            if let Err(e) = convex_client::ConvexClientManager::upsert_tool_call(
                thread_id,
                &id,
                title,
                kind.as_deref(),
                status.as_deref(),
                content_json.as_deref(),
                locations_json.as_deref(),
            ).await {
                error!(?e, thread_id, tool_call_id=%id, "failed to update tool call in convex");
            }
        }
        SU::Plan(p) => {
            if let Ok(entries_json) = serde_json::to_string(&p.entries) {
                if let Err(e) = convex_client::ConvexClientManager::upsert_plan(
                    thread_id,
                    &entries_json,
                ).await {
                    error!(?e, thread_id, "failed to upsert plan to convex");
                }
            }
        }
        SU::AvailableCommandsUpdate(ac) => {
            if let Ok(cmds_json) = serde_json::to_string(&ac.available_commands) {
                if let Err(e) = convex_client::ConvexClientManager::upsert_thread_state(
                    thread_id,
                    None,
                    Some(&cmds_json),
                ).await {
                    error!(?e, thread_id, "failed to upsert thread state to convex");
                }
            }
        }
        SU::CurrentModeUpdate(cm) => {
            let mode_id = format!("{:?}", cm.current_mode_id);
            if let Err(e) = convex_client::ConvexClientManager::upsert_thread_state(
                thread_id,
                Some(&mode_id),
                None,
            ).await {
                error!(?e, thread_id, mode=%mode_id, "failed to upsert current mode to convex");
            }
        }
        SU::UserMessageChunk(ch) => {
            let txt = content_to_text(&ch.content);
            if !txt.is_empty() {
                let item_id = format!("turn:{}:user", chrono::Utc::now().timestamp_millis());
                if let Err(e) = convex_client::ConvexClientManager::upsert_streaming_message(
                    thread_id,
                    &item_id,
                    "user",
                    &txt,
                    Some("message"),
                    false, // user messages are immediately finalized
                    None,
                ).await {
                    error!(?e, thread_id, item_id, "failed to upsert user message to convex");
                }
            }
        }
        SU::AgentMessageChunk(ch) => {
            let txt = content_to_text(&ch.content);
            if !txt.is_empty() {
                // Track item_id for this stream (reuse if exists, create new if not)
                let stream_key = format!("{}|assistant", thread_id);
                let mut streaming = STREAMING_ITEMS.lock().await;
                let item_id = streaming.entry(stream_key.clone()).or_insert_with(|| {
                    format!("turn:{}:assistant", chrono::Utc::now().timestamp_millis())
                }).clone();
                drop(streaming);

                if let Err(e) = convex_client::ConvexClientManager::upsert_streaming_message(
                    thread_id,
                    &item_id,
                    "assistant",
                    &txt,
                    Some("message"),
                    true, // streaming, not finalized yet
                    None,
                ).await {
                    error!(?e, thread_id, item_id, "failed to upsert assistant message to convex");
                }
            }
        }
        SU::AgentThoughtChunk(ch) => {
            let txt = content_to_text(&ch.content);
            if !txt.is_empty() {
                // Track item_id for this stream (reuse if exists, create new if not)
                let stream_key = format!("{}|reason", thread_id);
                let mut streaming = STREAMING_ITEMS.lock().await;
                let item_id = streaming.entry(stream_key.clone()).or_insert_with(|| {
                    format!("turn:{}:reason", chrono::Utc::now().timestamp_millis())
                }).clone();
                drop(streaming);

                if let Err(e) = convex_client::ConvexClientManager::upsert_streaming_message(
                    thread_id,
                    &item_id,
                    "assistant",
                    &txt,
                    Some("reason"),
                    true, // streaming
                    None,
                ).await {
                    error!(?e, thread_id, item_id, "failed to upsert thought to convex");
                }
            }
        }
    }

    // Append significant events to ACP event log (skip streaming chunks to reduce noise)
    let should_log = !matches!(
        update,
        SU::UserMessageChunk(_) | SU::AgentMessageChunk(_) | SU::AgentThoughtChunk(_)
    );

    if should_log {
        if let Ok(payload) = serde_json::to_string(update) {
            let update_kind = match update {
                SU::ToolCall(_) => "tool_call",
                SU::ToolCallUpdate(_) => "tool_call_update",
                SU::Plan(_) => "plan",
                SU::AvailableCommandsUpdate(_) => "available_commands",
                SU::CurrentModeUpdate(_) => "current_mode",
                _ => return, // Skip chunks (already filtered above, but double-check)
            };

            if let Err(e) = convex_client::ConvexClientManager::append_event(
                None, // session_id - we could track this
                None, // client_thread_doc_id
                Some(thread_id),
                Some(update_kind),
                &payload,
            ).await {
                error!(?e, thread_id, update_kind, "failed to append ACP event to convex");
            }
        }
    }
}

// Helper used by UI/diagnostics: resolve ACP agent path or return error
pub fn try_resolve_acp_agent() -> anyhow::Result<PathBuf> {
    let (bin, _args) = resolve_codex_acp_exec()?;
    Ok(bin)
}

fn resolve_claude_code_acp_exec() -> anyhow::Result<(PathBuf, Vec<String>)> {
    // 1) Explicit env override
    if let Ok(val) = std::env::var("OA_CLAUDE_CODE_ACP_CMD") {
        let val = val.trim();
        if !val.is_empty() {
            // Parse as "command arg1 arg2..." or just path
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.is_empty() {
                return Err(anyhow::anyhow!("OA_CLAUDE_CODE_ACP_CMD is empty"));
            }
            let cmd = PathBuf::from(parts[0]);
            let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
            return Ok((cmd, args));
        }
    }

    // 2) Default: tsx packages/claude-code-acp/index.ts (preferred for TypeScript)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let base = PathBuf::from(manifest_dir)
            .parent()
            .and_then(|p| p.parent()) // go to repo root
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let script = base.join("packages/claude-code-acp/index.ts");
        if script.exists() {
            // Try tsx first (better for TypeScript), fallback to node
            if let Ok(tsx) = which("tsx") {
                return Ok((tsx, vec![script.display().to_string()]));
            }
            if let Ok(node) = which("node") {
                return Ok((node, vec![script.display().to_string()]));
            }
            return Err(anyhow::anyhow!(
                "Claude Code ACP script found at {} but neither 'tsx' nor 'node' found in PATH",
                script.display()
            ));
        }
    }

    // 3) Try to find in node_modules or PATH
    if let Ok(found) = which("claude-code-acp") {
        return Ok((found, Vec::new()));
    }

    Err(anyhow::anyhow!(
        "Claude Code ACP not found. Set OA_CLAUDE_CODE_ACP_CMD to 'tsx path/to/index.ts' or ensure packages/claude-code-acp/index.ts exists and 'tsx' is in PATH"
    ))
}

fn resolve_codex_acp_exec() -> anyhow::Result<(PathBuf, Vec<String>)> {
    // 1) Explicit env override (path or which)
    if let Ok(val) = std::env::var("OA_ACP_AGENT_CMD") {
        let val = val.trim();
        if !val.is_empty() {
            // If absolute/relative path, use directly; otherwise try which
            let p = PathBuf::from(val);
            if p.is_absolute() || p.components().any(|c| matches!(c, std::path::Component::CurDir | std::path::Component::ParentDir)) {
                info!(path=%p.display(), "resolved codex-acp via OA_ACP_AGENT_CMD path");
                return Ok((p, Vec::new()));
            }
            if let Ok(found) = which(val) { info!(path=%found.display(), "resolved codex-acp via which(OA_ACP_AGENT_CMD)"); return Ok((found, Vec::new())); }
            // fallthrough to other strategies
        }
    }
    // 2) Prefer local workspace build of codex-acp if present (dev-only convenience)
    //    Try workspace-level target first, then crate-level target
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let base = PathBuf::from(manifest_dir)
            .parent()
            .and_then(|p| p.parent()) // go to repo root
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        // Check workspace-level target directory first
        let workspace_release = base.join("target/release/codex-acp");
        let workspace_debug = base.join("target/debug/codex-acp");
        if workspace_release.exists() {
            let p = workspace_release.display().to_string();
            info!(path=%p, "resolved codex-acp from workspace release build");
            return Ok((workspace_release, Vec::new()));
        }
        if workspace_debug.exists() {
            let p = workspace_debug.display().to_string();
            info!(path=%p, "resolved codex-acp from workspace debug build");
            return Ok((workspace_debug, Vec::new()));
        }
        // Fall back to crate-level target directory
        let release = base.join("crates/codex-acp/target/release/codex-acp");
        let dbg_bin = base.join("crates/codex-acp/target/debug/codex-acp");
        if release.exists() {
            let p = release.display().to_string();
            info!(path=%p, "resolved codex-acp local release build");
            return Ok((release, Vec::new()));
        }
        if dbg_bin.exists() {
            let p = dbg_bin.display().to_string();
            info!(path=%p, "resolved codex-acp local debug build");
            return Ok((dbg_bin, Vec::new()));
        }
        // Also support sibling/extern repo via env or conventional path
        if let Ok(root) = std::env::var("OA_CODEX_ACP_ROOT") {
            let root = PathBuf::from(root);
            if let Some(bin) = ensure_codex_acp_built(&root) {
                let p = bin.display().to_string();
                info!(path=%p, "resolved codex-acp via OA_CODEX_ACP_ROOT built binary");
                return Ok((bin, Vec::new()));
            }
        } else {
            let sibling = base.parent().map(|p| p.join("codex-acp"));
            if let Some(sib) = sibling {
                if let Some(bin) = ensure_codex_acp_built(&sib) {
                    let p = bin.display().to_string();
                    info!(path=%p, "resolved codex-acp via sibling repo built binary");
                    return Ok((bin, Vec::new()));
                }
            }
        }
    }
    // 3) PATH search: prefer 'codex-acp'
    if let Ok(found) = which("codex-acp") {
        let p = found.display().to_string();
        info!(path=%p, "resolved codex-acp via which(codex-acp)");
        return Ok((found, Vec::new()));
    }

    // 4) Helpful error (do not fall back to 'codex' or 'co' for ACP)
    Err(anyhow::anyhow!(
        "ACP agent not found. Build crates/codex-acp (e.g. `cargo build --manifest-path crates/codex-acp/Cargo.toml --release`) or set OA_ACP_AGENT_CMD to the built codex-acp path; otherwise set OA_USE_CODEX_EXEC=1 to use the codex exec fallback."
    ))
}

fn ensure_codex_acp_built(root: &Path) -> Option<PathBuf> {
    let rel = root.join("target/release/codex-acp");
    if rel.exists() { return Some(rel); }
    let dbg = root.join("target/debug/codex-acp");
    if dbg.exists() { return Some(dbg); }
    // Try to build if Cargo.toml exists
    let manifest = root.join("Cargo.toml");
    if manifest.exists() {
        let status = std::process::Command::new("cargo")
            .current_dir(root)
            .arg("build")
            .arg("--release")
            .status()
            .ok()?;
        if status.success() {
            let rel = root.join("target/release/codex-acp");
            if rel.exists() { return Some(rel); }
        }
    }
    None
}

fn resolve_codex_exec_bin() -> anyhow::Result<PathBuf> {
    // Prefer explicit env
    if let Ok(val) = std::env::var("OA_ACP_AGENT_CMD") {
        let p = PathBuf::from(val);
        if p.exists() { return Ok(p); }
    }
    if let Ok(val) = std::env::var("CODEX_BIN") {
        let p = PathBuf::from(val);
        if p.exists() { return Ok(p); }
    }
    if let Ok(found) = which("codex") { return Ok(found); }
    if let Ok(found) = which("co") { return Ok(found); }
    Err(anyhow::anyhow!("codex exec fallback not found. Set CODEX_BIN or OA_ACP_AGENT_CMD to codex path, or install 'codex'/'co' in PATH."))
}
