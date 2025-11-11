use std::{collections::HashMap, sync::Arc, path::PathBuf};

use agent_client_protocol as acp;
use anyhow::Result;
use tokio::sync::{mpsc, RwLock, Mutex};
use uuid::Uuid;

use super::ACPClient;
use which::which;
use tracing::{debug, info, warn, error};
use crate::codex_exec::{run_codex_exec_once, CodexExecOptions};

#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<SessionManagerInner>,
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
}

impl SessionManager {
    pub fn new() -> Self {
        Self { inner: Arc::new(SessionManagerInner { sessions: RwLock::new(HashMap::new()), clients: RwLock::new(HashMap::new()) }) }
    }

    pub async fn create_session(&self, agent_type: String, cwd: PathBuf) -> Result<acp::SessionId> {
        // Resolve agent binary path with sensible fallbacks
        let cmd = resolve_agent_bin()?;
        let args = std::env::var("OA_ACP_AGENT_ARGS").unwrap_or_default();
        let args: Vec<String> = if args.trim().is_empty() { vec![] } else { shell_words::split(&args).unwrap_or_else(|_| args.split_whitespace().map(|s| s.to_string()).collect()) };
        let use_codex_exec = agent_type == "codex-exec" || std::env::var("OA_USE_CODEX_EXEC").ok().as_deref() == Some("1");
        info!(agent_type=%agent_type, bin=%cmd.display(), args=?args, cwd=%cwd.display(), use_codex_exec, "create_session resolved");
        let session_id = acp::SessionId(Arc::from(uuid::Uuid::new_v4().to_string()));
        if !use_codex_exec {
            let mut client = ACPClient::spawn(cmd.to_string_lossy().as_ref(), &args, &[], None).await?;
            let real_sid = client.new_session(cwd.clone()).await?;
            // override with real id
            let session = Session { id: real_sid.clone(), cwd: cwd.clone(), messages: vec![], use_codex_exec: false };
            self.inner.sessions.write().await.insert(real_sid.clone(), session);
            self.inner.clients.write().await.insert(real_sid.clone(), Arc::new(Mutex::new(client)));
            return Ok(real_sid);
        }

        // For codex-exec path, no ACP client; we spawn per-prompt
        let session = Session { id: session_id.clone(), cwd, messages: vec![], use_codex_exec: true };
        self.inner.sessions.write().await.insert(session_id.clone(), session);

        Ok(session_id)
    }

    pub async fn prompt(&self, session_id: &acp::SessionId, text: String) -> Result<()> {
        // push user message
        {
            let mut sessions = self.inner.sessions.write().await;
            if let Some(sess) = sessions.get_mut(session_id) {
                let now = chrono::Utc::now().to_rfc3339();
                sess.messages.push(SessionMessage { id: Uuid::new_v4().to_string(), role: "user".into(), content: vec![acp::ContentBlock::from(text.clone())], created_at: now });
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
            let bin = resolve_agent_bin()?; // re-use resolution; should find codex/co
            let extra_args = std::env::var("OA_CODEX_ARGS").ok().map(|s| s.split_whitespace().map(|x| x.to_string()).collect()).unwrap_or_else(|| vec!["exec".into(), "--json".into()]);
            let opts = CodexExecOptions { bin, cwd, extra_args };
            let inner = self.inner.clone();
            info!(session_id=%sid.0, "codex-exec spawning for prompt");
            tokio::spawn(async move {
                let _ = run_codex_exec_once(&opts, &text, |update| {
                    // Apply update to session state
                    let inner = inner.clone();
                    let sid = sid.clone();
                    // Blocking write in async task
                    futures::executor::block_on(async move {
                        let mut sessions = inner.sessions.write().await;
                        if let Some(sess) = sessions.get_mut(&sid) {
                            debug!(?update, "codex-exec mapped update");
                            match update {
                                acp::SessionUpdate::AgentMessageChunk(chunk) | acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                                    let now = chrono::Utc::now().to_rfc3339();
                                    if let Some(last) = sess.messages.last_mut() {
                                        if last.role == "assistant" {
                                            last.content.push(chunk.content);
                                            return;
                                        }
                                    }
                                    sess.messages.push(SessionMessage { id: Uuid::new_v4().to_string(), role: "assistant".into(), content: vec![chunk.content], created_at: now });
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

fn resolve_agent_bin() -> anyhow::Result<PathBuf> {
    // 1) Explicit env override
    if let Ok(val) = std::env::var("OA_ACP_AGENT_CMD") {
        let val = val.trim();
        if !val.is_empty() {
            // If absolute/relative path, use directly; otherwise try which
            let p = PathBuf::from(val);
            if p.is_absolute() || p.components().any(|c| matches!(c, std::path::Component::CurDir | std::path::Component::ParentDir)) {
                return Ok(p);
            }
            if let Ok(found) = which(val) { return Ok(found); }
            // fallthrough to other strategies
        }
    }
    // 2) Alternate env from older code paths
    if let Ok(val) = std::env::var("CODEX_BIN") {
        let p = PathBuf::from(val);
        if p.exists() { return Ok(p); }
    }
    // 3) PATH search: prefer 'codex', then 'co'
    if let Ok(found) = which("codex") { return Ok(found); }
    if let Ok(found) = which("co") { return Ok(found); }

    // 4) Helpful error
    Err(anyhow::anyhow!(
        "Agent binary not found. Set OA_ACP_AGENT_CMD to your agent path or ensure 'codex' or 'co' is in PATH."
    ))
}
