use std::{collections::HashMap, sync::Arc, path::{Path, PathBuf}};

use agent_client_protocol as acp;
use anyhow::Result;
use tokio::sync::{RwLock, Mutex};
use uuid::Uuid;

use super::ACPClient;
use which::which;
use tracing::{debug, info};
use crate::codex_exec::{run_codex_exec_once, CodexExecOptions};
use crate::tinyvex_state::TinyvexState;
use crate::tinyvex_ws;

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
            (cmd, merged)
        };
        info!(agent_type=%agent_type, bin=%cmd.display(), args=?all_args, cwd=%cwd.display(), use_codex_exec, "create_session resolved");
        let session_id = acp::SessionId(Arc::from(uuid::Uuid::new_v4().to_string()));
        if !use_codex_exec {
            // For TypeScript agents (claude-code), set NODE_PATH to tauri/node_modules
            let mut envs: Vec<(String, String)> = Vec::new();
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

                    tracing::info!(
                        target: "openagents_lib::oa_acp::session_manager",
                        session_id = %notif.session_id.0,
                        kind = ?update_copy,
                        "processed ACP update via tinyvex"
                    );
                }
                info!(session_id=%sid_clone.0, "update stream ended");
            });
            // override with real id
            let session = Session { id: real_sid.clone(), cwd: cwd.clone(), messages: vec![], use_codex_exec: false, agent_type: agent_type.clone() };
            self.inner.sessions.write().await.insert(real_sid.clone(), session);
            self.inner.clients.write().await.insert(real_sid.clone(), Arc::new(Mutex::new(client)));
            return Ok(real_sid);
        }

        // For codex-exec path, no ACP client; we spawn per-prompt
        let session = Session { id: session_id.clone(), cwd, messages: vec![], use_codex_exec: true, agent_type };
        self.inner.sessions.write().await.insert(session_id.clone(), session);

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
                                acp::SessionUpdate::Plan(ref _plan) => {
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

                        tracing::info!(
                            target: "openagents_lib::oa_acp::session_manager",
                            session_id = %sid.0,
                            kind = ?update_copy,
                            "processed codex-exec update via tinyvex"
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

    pub fn tinyvex_state(&self) -> &Arc<TinyvexState> {
        &self.tinyvex
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
                return Ok((p, Vec::new()));
            }
            if let Ok(found) = which(val) { return Ok((found, Vec::new())); }
            // fallthrough to other strategies
        }
    }
    // 2) Prefer local workspace build of codex-acp if present (dev-only convenience)
    //    Try release then debug under crates/codex-acp
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let base = PathBuf::from(manifest_dir)
            .parent()
            .and_then(|p| p.parent()) // go to repo root
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let release = base.join("crates/codex-acp/target/release/codex-acp");
        let debug = base.join("crates/codex-acp/target/debug/codex-acp");
        if release.exists() { return Ok((release, Vec::new())); }
        if debug.exists() { return Ok((debug, Vec::new())); }
        // Also support sibling/extern repo via env or conventional path
        if let Ok(root) = std::env::var("OA_CODEX_ACP_ROOT") {
            let root = PathBuf::from(root);
            if let Some(bin) = ensure_codex_acp_built(&root) { return Ok((bin, Vec::new())); }
        } else {
            let sibling = base.parent().map(|p| p.join("codex-acp"));
            if let Some(sib) = sibling {
                if let Some(bin) = ensure_codex_acp_built(&sib) { return Ok((bin, Vec::new())); }
            }
        }
    }
    // 3) PATH search: prefer 'codex-acp'
    if let Ok(found) = which("codex-acp") { return Ok((found, Vec::new())); }

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
