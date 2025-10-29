use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use agent_client_protocol as acp;
use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
use agent_client_protocol::Agent as _;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::convex_write::mirror_acp_update_to_convex;
use crate::state::AppState;

/// Runtime for a single Codex ACP adapter process and its ACP client connection.
// Global sender to the Codex ACP thread.
static RUNTIME_SENDER: OnceCell<tokio::sync::mpsc::UnboundedSender<CommandMsg>> = OnceCell::new();

/// Minimal client delegate that handles session_update notifications and fs read/write.
struct ClientDelegate {
    state: Arc<AppState>,
    // Reverse map: sessionId → threadDocId, derived from runtime.sessions
    sessions: Arc<Mutex<HashMap<String, String>>>,
}

impl ClientDelegate {
    fn new(state: Arc<AppState>) -> Self {
        Self { state, sessions: Arc::new(Mutex::new(HashMap::new())) }
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for ClientDelegate {
    async fn session_notification(&self, n: acp::SessionNotification) -> acp::Result<()> {
        let st = self.state.clone();
        let sessions = self.sessions.clone();
        let tx = self.state.tx.clone();
        // Mirror to Convex
        let sid = n.session_id.to_string();
        let thread_id = {
            let map = sessions.lock().await;
            map.get(&sid).cloned().or_else(|| st.current_convex_thread.blocking_lock().clone())
        };
        if let Some(thread_doc_id) = thread_id {
            mirror_acp_update_to_convex(&st, &thread_doc_id, &n.update).await;
        }
        if std::env::var("BRIDGE_ACP_EMIT").ok().as_deref() == Some("1") {
            if let Ok(line) = serde_json::to_string(&serde_json::json!({ "type": "bridge.acp", "notification": n })) {
                let _ = tx.send(line);
            }
        }
        Ok(())
    }

    async fn request_permission(
        &self,
        params: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        // Auto-select an allow option when present; otherwise select the first option.
        let picked = params
            .options
            .iter()
            .find(|o| matches!(o.kind, acp::PermissionOptionKind::AllowAlways | acp::PermissionOptionKind::AllowOnce))
            .or_else(|| params.options.first())
            .map(|o| o.id.clone());
        let outcome = match picked {
            Some(option_id) => acp::RequestPermissionOutcome::Selected { option_id },
            None => acp::RequestPermissionOutcome::Cancelled,
        };
        Ok(acp::RequestPermissionResponse { outcome, meta: None })
    }

    async fn read_text_file(&self, params: acp::ReadTextFileRequest) -> acp::Result<acp::ReadTextFileResponse> {
        // Scope reads to cwd; for now, trust agent to provide relative paths
        let path = params.path;
        let data = std::fs::read_to_string(&path).map_err(|e| acp::Error::internal_error().with_data(e.to_string()))?;
        Ok(acp::ReadTextFileResponse { content: data, meta: None })
    }

    async fn write_text_file(&self, params: acp::WriteTextFileRequest) -> acp::Result<acp::WriteTextFileResponse> {
        let path = params.path;
        std::fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new("."))).ok();
        std::fs::write(&path, params.content).map_err(|e| acp::Error::internal_error().with_data(e.to_string()))?;
        Ok(acp::WriteTextFileResponse { meta: None })
    }
}

// Helper to pick a permission option to auto-select.
fn pick_permission_option(options: &[acp::PermissionOption]) -> acp::RequestPermissionOutcome {
    if let Some(opt) = options.iter().find(|o| matches!(o.kind, acp::PermissionOptionKind::AllowAlways | acp::PermissionOptionKind::AllowOnce)) {
        return acp::RequestPermissionOutcome::Selected { option_id: opt.id.clone() };
    }
    if let Some(first) = options.first() {
        return acp::RequestPermissionOutcome::Selected { option_id: first.id.clone() };
    }
    acp::RequestPermissionOutcome::Cancelled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_allows_when_present() {
        let opts = vec![
            acp::PermissionOption { id: acp::PermissionOptionId("1".into()), name: "Reject".into(), kind: acp::PermissionOptionKind::RejectOnce, meta: None },
            acp::PermissionOption { id: acp::PermissionOptionId("2".into()), name: "Allow".into(), kind: acp::PermissionOptionKind::AllowOnce, meta: None },
        ];
        let out = pick_permission_option(&opts);
        match out { acp::RequestPermissionOutcome::Selected{ option_id } => assert_eq!(option_id.0.as_ref(), "2"), _ => panic!("expected Selected") }
    }

    #[test]
    fn pick_first_when_no_allow() {
        let opts = vec![
            acp::PermissionOption { id: acp::PermissionOptionId("a".into()), name: "Reject".into(), kind: acp::PermissionOptionKind::RejectOnce, meta: None },
        ];
        let out = pick_permission_option(&opts);
        match out { acp::RequestPermissionOutcome::Selected{ option_id } => assert_eq!(option_id.0.as_ref(), "a"), _ => panic!("expected Selected") }
    }

    #[test]
    fn pick_cancelled_when_empty() {
        let opts: Vec<acp::PermissionOption> = vec![];
        let out = pick_permission_option(&opts);
        matches!(out, acp::RequestPermissionOutcome::Cancelled);
    }
}

/// Spawn `codex-acp` and establish an ACP client connection. Returns a runtime handle.
async fn spawn_codex_acp(state: Arc<AppState>, sessions_rev: Arc<Mutex<HashMap<String, String>>>) -> Result<(Rc<acp::ClientSideConnection>, tokio::task::JoinHandle<()>)> {
    let exec = state
        .opts
        .codex_acp_bin
        .clone()
        .unwrap_or_else(|| PathBuf::from("codex-acp"));

    let mut cmd = Command::new(&exec);
    // Default to repo root working dir if known; otherwise inherit
    if let Some(tid) = state.current_convex_thread.lock().await.clone() { let _ = tid; }
    // Stdio pipes
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().context("failed to spawn codex-acp")?;
    let stdout = child.stdout.take().context("codex-acp missing stdout")?;
    let stdin = child.stdin.take().context("codex-acp missing stdin")?;
    // stderr to console
    let mut stderr = child.stderr.take().context("codex-acp missing stderr")?;
    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut rd = BufReader::new(&mut stderr);
        let mut line = String::new();
        while rd.read_line(&mut line).await.ok().filter(|&n| n > 0).is_some() {
            info!("agent stderr: {}", line.trim());
            line.clear();
        }
    });

    // Build client connection using the crate helper
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
    let client_delegate = ClientDelegate { state: state.clone(), sessions: sessions_rev.clone() };
    let (connection, io_task) = acp::ClientSideConnection::new(client_delegate, stdin.compat_write(), stdout.compat(), |fut| {
        tokio::task::spawn_local(fut);
    });
    let io_join = tokio::task::spawn_local(async move { let _ = io_task.await; });

    // Initialize once with our capabilities
    use agent_client_protocol::Agent as _;
    let resp = connection
        .initialize(acp::InitializeRequest {
            protocol_version: acp::VERSION,
            client_capabilities: acp::ClientCapabilities {
                fs: acp::FileSystemCapability { read_text_file: true, write_text_file: true, meta: None },
                terminal: false,
                meta: None,
            },
            client_info: Some(acp::Implementation { name: "openagents".into(), title: None, version: env!("CARGO_PKG_VERSION").into() }),
            meta: None,
        })
        .await?;
    info!(?resp.protocol_version, "codex-acp connected");

    Ok((Rc::new(connection), io_join))
}

/// Ensure a Codex ACP runtime exists and send a prompt for a given thread.
enum CommandMsg { Prompt { cwd: Option<PathBuf>, thread_doc_id: String, text: String } }

fn ensure_runtime(state: Arc<AppState>) -> Result<tokio::sync::mpsc::UnboundedSender<CommandMsg>> {
    if let Some(tx) = RUNTIME_SENDER.get() { return Ok(tx.clone()); }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CommandMsg>();
    RUNTIME_SENDER.set(tx.clone()).ok();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_io().build().unwrap();
        rt.block_on(async move {
            let local = tokio::task::LocalSet::new();
            local.run_until(async move {
                // Spawn adapter and connection
                let sessions = Arc::new(Mutex::new(HashMap::<String, acp::SessionId>::new()));
                let sessions_rev = Arc::new(Mutex::new(HashMap::<String, String>::new()));
                let (connection, _io) = match spawn_codex_acp(state.clone(), sessions_rev.clone()).await { Ok(v) => v, Err(e) => { error!(?e, "codex-acp spawn failed"); return; } };
                while let Some(msg) = rx.recv().await {
                    match msg {
                        CommandMsg::Prompt { cwd, thread_doc_id, text } => {
                            // ensure session
                            let sid = {
                                let mut map = sessions.lock().await;
                                if let Some(s) = map.get(&thread_doc_id) { s.clone() } else {
                                    let req = acp::NewSessionRequest { mcp_servers: vec![], cwd: cwd.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))), meta: None };
                                    match connection.new_session(req).await { Ok(resp) => { let sid = resp.session_id.clone(); map.insert(thread_doc_id.clone(), sid.clone()); sessions_rev.lock().await.insert(sid.to_string(), thread_doc_id.clone()); sid }, Err(e) => { error!(?e, "new_session failed"); continue; } }
                                }
                            };
                            // prompt
                            let block = acp::ContentBlock::Text(acp::TextContent { text: text.clone(), annotations: None, meta: None });
                            let req = acp::PromptRequest { session_id: sid, prompt: vec![block], meta: None };
                            if let Err(e) = connection.prompt(req).await { error!(?e, "prompt failed"); }
                        }
                    }
                }
            }).await;
        });
    });
    Ok(tx)
}

pub async fn codex_acp_prompt(state: Arc<AppState>, cwd: Option<PathBuf>, thread_doc_id: &str, text: &str) -> Result<()> {
    let tx = ensure_runtime(state)?;
    tx.send(CommandMsg::Prompt { cwd, thread_doc_id: thread_doc_id.to_string(), text: text.to_string() }).map_err(|_| anyhow::anyhow!("codex-acp runtime not available"))
}
