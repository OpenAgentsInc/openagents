use std::{collections::HashMap, path::PathBuf, sync::{Arc, atomic::{AtomicI64, Ordering}}};

use agent_client_protocol as acp;
use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin, Command}, sync::{mpsc, oneshot}, task::JoinHandle, time::{timeout, Duration}};
use tracing::{info, error};

#[derive(thiserror::Error, Debug)]
pub enum AcpError {
    #[error("spawn error: {0}")]
    Spawn(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("rpc error: {0}")]
    Rpc(String),
    #[error("protocol error: {0}")]
    Protocol(String),
}

pub struct ACPClient {
    child: Child,
    stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
    _reader_task: JoinHandle<()>,
    next_id: AtomicI64,
    pending: Arc<tokio::sync::Mutex<HashMap<acp::RequestId, oneshot::Sender<RpcResult>>>>,
    _update_tx: mpsc::Sender<acp::SessionNotification>,
    update_rx: mpsc::Receiver<acp::SessionNotification>,
}

#[derive(Debug)]
enum RpcResult {
    Ok(serde_json::Value),
    Err(serde_json::Value),
}

impl ACPClient {
    pub async fn spawn(command: &str, args: &[String], envs: &[(String, String)], cwd: Option<PathBuf>) -> Result<Self, AcpError> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        for (k, v) in envs {
            cmd.env(k, v);
        }
        // On macOS apps, PATH is often very limited; append common Homebrew paths
        if let Ok(current_path) = std::env::var("PATH") {
            let mut pieces: Vec<&str> = current_path.split(':').collect();
            if !pieces.iter().any(|p| *p == "/opt/homebrew/bin") {
                pieces.push("/opt/homebrew/bin");
            }
            if !pieces.iter().any(|p| *p == "/usr/local/bin") {
                pieces.push("/usr/local/bin");
            }
            let new_path = pieces.join(":");
            cmd.env("PATH", new_path);
        }
        let cwd_log = cwd.as_ref().map(|p| p.display().to_string());
        if let Some(ref cwd_dir) = cwd {
            cmd.current_dir(cwd_dir);
        }
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        // Log important env toggles and PATH for debugging spawn issues
        let bypass1 = std::env::var("CODEX_ACP_DANGEROUSLY_BYPASS_APPROVALS_AND_SANDBOX").unwrap_or_else(|_| "<unset>".into());
        let bypass2 = std::env::var("OA_DANGEROUSLY_BYPASS_APPROVALS_AND_SANDBOX").unwrap_or_else(|_| "<unset>".into());
        let path_env = std::env::var("PATH").unwrap_or_default();
        info!(cmd=%command, args=?args, cwd=?cwd_log, bypass_env=%bypass1, bypass_env_alt=%bypass2, path=%path_env, "spawning ACP agent");
        let mut child = cmd.spawn().map_err(|e| AcpError::Spawn(format!("{} (cmd='{}' args='{:#?}')", e, command, args)))?;
        let stdin = child.stdin.take().ok_or_else(|| AcpError::Spawn("failed to open stdin".into()))?;
        let stdin = Arc::new(tokio::sync::Mutex::new(stdin));
        let stdout = child.stdout.take().ok_or_else(|| AcpError::Spawn("failed to open stdout".into()))?;

        let (update_tx, update_rx) = mpsc::channel(128);
        let pending: Arc<tokio::sync::Mutex<HashMap<acp::RequestId, oneshot::Sender<RpcResult>>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let reader_task = {
            let pending = pending.clone();
            let update_tx = update_tx.clone();
            let stdin = stdin.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Elevate to info so it shows up in app logs
                    info!(agent_stdout=%line, "agent stdout line");
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        let maybe_method = v.get("method").and_then(|m| m.as_str());
                        let maybe_id = v.get("id").cloned();
                        match (maybe_method, maybe_id) {
                            (Some(method), Some(id_val)) => {
                                // Agent request to Client (expects response)
                                let params = v.get("params").cloned().unwrap_or(serde_json::json!({}));
                                let method_norm = method.replace('-', "").replace('_', "").replace('/', "").to_lowercase();
                                // Helper to write a JSON-RPC response
                                async fn respond(stdin: &Arc<tokio::sync::Mutex<ChildStdin>>, id_val: serde_json::Value, result: Option<serde_json::Value>, error: Option<serde_json::Value>) {
                                    let mut obj = serde_json::json!({"jsonrpc": "2.0", "id": id_val});
                                    if let Some(res) = result { obj["result"] = res; }
                                    if let Some(err) = error { obj["error"] = err; }
                                    let mut buf = match serde_json::to_vec(&obj) { Ok(b) => b, Err(_) => return };
                                    buf.push(b'\n');
                                    let mut w = stdin.lock().await; let _ = w.write_all(&buf).await; let _ = w.flush().await;
                                }

                                if method_norm.contains("sessionrequestpermission") {
                                    if let Ok(req) = serde_json::from_value::<acp::RequestPermissionRequest>(params) {
                                        // Auto-approve: prefer approved-for-session, else approved, else first Allow*, else first
                                        let mut selection: Option<acp::PermissionOptionId> = None;
                                        for o in &req.options { if o.id.0.as_ref() == "approved-for-session" { selection = Some(o.id.clone()); break; } }
                                        if selection.is_none() { for o in &req.options { if o.id.0.as_ref() == "approved" { selection = Some(o.id.clone()); break; } } }
                                        if selection.is_none() { for o in &req.options { if matches!(o.kind, acp::PermissionOptionKind::AllowOnce | acp::PermissionOptionKind::AllowAlways) { selection = Some(o.id.clone()); break; } } }
                                        if selection.is_none() { selection = req.options.first().map(|o| o.id.clone()); }
                                        let selected = selection.unwrap_or_else(|| acp::PermissionOptionId(Arc::from("approved")));
                                        let resp = acp::RequestPermissionResponse { outcome: acp::RequestPermissionOutcome::Selected { option_id: selected }, meta: None };
                                        let result = serde_json::to_value(resp).ok();
                                        respond(&stdin, id_val, result, None).await;
                                    } else {
                                        let err = serde_json::json!({"code": -32602, "message": "invalid params"});
                                        respond(&stdin, id_val, None, Some(err)).await;
                                    }
                                } else if method_norm.contains("fswritetextfile") {
                                    if let Ok(req) = serde_json::from_value::<acp::WriteTextFileRequest>(params) {
                                        let res = std::fs::write(&req.path, req.content);
                                        if res.is_ok() {
                                            let result = serde_json::to_value(acp::WriteTextFileResponse::default()).ok();
                                            respond(&stdin, id_val, result, None).await;
                                        } else {
                                            let err = serde_json::json!({"code": -32000, "message": format!("write failed: {}", res.err().unwrap())});
                                            respond(&stdin, id_val, None, Some(err)).await;
                                        }
                                    } else {
                                        let err = serde_json::json!({"code": -32602, "message": "invalid params"});
                                        respond(&stdin, id_val, None, Some(err)).await;
                                    }
                                } else if method_norm.contains("fsreadtextfile") {
                                    if let Ok(req) = serde_json::from_value::<acp::ReadTextFileRequest>(params) {
                                        let content = std::fs::read_to_string(&req.path).unwrap_or_default();
                                        let final_content = if let Some(limit) = req.limit {
                                            let start = req.line.unwrap_or(1) as usize;
                                            let lines: Vec<&str> = content.lines().collect();
                                            let start_idx = start.saturating_sub(1).min(lines.len());
                                            let end_idx = (start_idx + (limit as usize)).min(lines.len());
                                            lines[start_idx..end_idx].join("\n")
                                        } else { content };
                                        let resp = acp::ReadTextFileResponse { content: final_content, meta: None };
                                        let result = serde_json::to_value(resp).ok();
                                        respond(&stdin, id_val, result, None).await;
                                    } else {
                                        let err = serde_json::json!({"code": -32602, "message": "invalid params"});
                                        respond(&stdin, id_val, None, Some(err)).await;
                                    }
                                } else {
                                    let err = serde_json::json!({"code": -32601, "message": format!("method not handled: {}", method)});
                                    respond(&stdin, id_val, None, Some(err)).await;
                                }
                            }
                            (Some(method), None) => {
                                // Notification (no response expected)
                                let params = v.get("params").cloned().unwrap_or(serde_json::json!({}));
                                let method_norm = method.replace('-', "").replace('_', "").replace('/', "").to_lowercase();
                                if method_norm.contains("sessionupdate") {
                                    if let Ok(n) = serde_json::from_value::<acp::SessionNotification>(params) {
                                        let _ = update_tx.send(n).await;
                                    }
                                }
                            }
                            (None, Some(_)) => {
                                // Response to a request we sent
                                let id: acp::RequestId = match v.get("id") {
                                    Some(serde_json::Value::Null) => acp::RequestId::Null,
                                    Some(serde_json::Value::Number(n)) => acp::RequestId::Number(n.as_i64().unwrap_or_default()),
                                    Some(serde_json::Value::String(s)) => acp::RequestId::Str(s.clone()),
                                    _ => acp::RequestId::Null,
                                };
                                let payload = if let Some(res) = v.get("result") { RpcResult::Ok(res.clone()) } else { RpcResult::Err(v.get("error").cloned().unwrap_or(serde_json::json!({}))) };
                                let tx_opt = { pending.lock().await.remove(&id) };
                                if let Some(tx) = tx_opt { let _ = tx.send(payload); }
                            }
                            _ => {}
                        }
                    }
                }
            })
        };

        let mut client = Self {
            child,
            stdin,
            _reader_task: reader_task,
            next_id: AtomicI64::new(1),
            pending,
            _update_tx: update_tx,
            update_rx,
        };

        // initialize handshake
        let init_req = acp::InitializeRequest {
            protocol_version: acp::VERSION,
            client_capabilities: acp::ClientCapabilities {
                fs: acp::FileSystemCapability { read_text_file: true, write_text_file: true, meta: None },
                terminal: false,
                meta: None,
            },
            client_info: Some(acp::Implementation { name: "openagents".to_string(), title: Some("OpenAgents".to_string()), version: env!("CARGO_PKG_VERSION").to_string() }),
            meta: None,
        };
        info!("sending ACP initialize request");
        let _init_resp: acp::InitializeResponse = client
            .send_request(acp::AGENT_METHOD_NAMES.initialize, &init_req)
            .await
            .map_err(|e| AcpError::Protocol(format!("initialize failed: {e}")))?;
        info!("ACP initialize completed");

        Ok(client)
    }

    pub async fn new_session(&mut self, cwd: PathBuf) -> Result<acp::SessionId, AcpError> {
        let req = acp::NewSessionRequest { cwd, mcp_servers: Vec::new(), meta: None };
        info!(?req, "sending ACP new_session");
        let resp: acp::NewSessionResponse = self
            .send_request(acp::AGENT_METHOD_NAMES.session_new, &req)
            .await
            .map_err(|e| AcpError::Protocol(format!("new session failed: {e}")))?;
        info!(session_id=%resp.session_id.0, "ACP new_session completed");
        Ok(resp.session_id)
    }

    pub async fn prompt(&mut self, session_id: acp::SessionId, text: String) -> Result<acp::PromptResponse, AcpError> {
        let content = acp::ContentBlock::from(text);
        let req = acp::PromptRequest { session_id, prompt: vec![content], meta: None };
        info!(?req, "sending ACP prompt");
        let resp: acp::PromptResponse = self
            .send_request(acp::AGENT_METHOD_NAMES.session_prompt, &req)
            .await
            .map_err(|e| AcpError::Protocol(format!("prompt failed: {e}")))?;
        info!(?resp, "ACP prompt completed");
        Ok(resp)
    }

    pub fn take_update_receiver(&mut self) -> mpsc::Receiver<acp::SessionNotification> {
        let (_dummy_tx, dummy_rx) = mpsc::channel(1);
        std::mem::replace(&mut self.update_rx, dummy_rx)
    }

    async fn send_request<T: Serialize + ?Sized, R: DeserializeOwned>(&mut self, method: &str, params: &T) -> Result<R> {
        let id_num = self.next_id.fetch_add(1, Ordering::SeqCst);
        let id = acp::RequestId::Number(id_num);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id_num,
            "method": method,
            "params": params,
        });

        let (tx, rx) = oneshot::channel();
        {
            self.pending.lock().await.insert(id.clone(), tx);
        }

        let mut data = serde_json::to_vec(&request).map_err(|e| anyhow!(e))?;
        data.push(b'\n');
        {
            let mut w = self.stdin.lock().await;
            w.write_all(&data).await.map_err(|e| AcpError::Io(e.to_string()))?;
            w.flush().await.map_err(|e| AcpError::Io(e.to_string()))?;
        }

        let res = timeout(Duration::from_secs(60), rx).await.map_err(|_| AcpError::Rpc("timeout".into()))
            .and_then(|r| r.map_err(|e| AcpError::Rpc(e.to_string())))?;

        match res {
            RpcResult::Ok(val) => serde_json::from_value(val).map_err(|e| anyhow!(e)),
            RpcResult::Err(err) => Err(anyhow!("rpc error: {}", err)),
        }
    }
}

impl Drop for ACPClient {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
