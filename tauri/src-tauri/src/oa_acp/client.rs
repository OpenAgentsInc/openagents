use std::{collections::HashMap, path::PathBuf, sync::{Arc, atomic::{AtomicI64, Ordering}}};

use agent_client_protocol as acp;
use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin, Command}, sync::{mpsc, oneshot}, task::JoinHandle, time::{timeout, Duration}};
use tracing::{debug, info, error};

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
    stdin: ChildStdin,
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
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        info!(cmd=%command, args=?args, "spawning ACP agent");
        let mut child = cmd.spawn().map_err(|e| AcpError::Spawn(format!("{} (cmd='{}' args='{:#?}')", e, command, args)))?;
        let stdin = child.stdin.take().ok_or_else(|| AcpError::Spawn("failed to open stdin".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| AcpError::Spawn("failed to open stdout".into()))?;

        let (update_tx, update_rx) = mpsc::channel(128);
        let pending: Arc<tokio::sync::Mutex<HashMap<acp::RequestId, oneshot::Sender<RpcResult>>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let reader_task = {
            let pending = pending.clone();
            let update_tx = update_tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!(agent_stdout=%line, "agent stdout line");
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        if v.get("method").is_some() {
                            // Notification
                            let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
                            let params = v.get("params").cloned().unwrap_or(serde_json::json!({}));
                            let method_norm = method.replace('-', "").replace('_', "").replace('/', "").to_lowercase();
                            if method_norm.contains("sessionupdate") {
                                if let Ok(n) = serde_json::from_value::<acp::SessionNotification>(params) {
                                    let _ = update_tx.send(n).await;
                                }
                            }
                        } else if v.get("id").is_some() {
                            // Response
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
                terminal: true,
                meta: None,
            },
            client_info: Some(acp::Implementation { name: "openagents".to_string(), title: Some("OpenAgents".to_string()), version: env!("CARGO_PKG_VERSION").to_string() }),
            meta: None,
        };
        let _init_resp: acp::InitializeResponse = client
            .send_request(acp::AGENT_METHOD_NAMES.initialize, &init_req)
            .await
            .map_err(|e| AcpError::Protocol(format!("initialize failed: {e}")))?;

        Ok(client)
    }

    pub async fn new_session(&mut self, cwd: PathBuf) -> Result<acp::SessionId, AcpError> {
        let req = acp::NewSessionRequest { cwd, mcp_servers: Vec::new(), meta: None };
        let resp: acp::NewSessionResponse = self
            .send_request(acp::AGENT_METHOD_NAMES.session_new, &req)
            .await
            .map_err(|e| AcpError::Protocol(format!("new session failed: {e}")))?;
        Ok(resp.session_id)
    }

    pub async fn prompt(&mut self, session_id: acp::SessionId, text: String) -> Result<acp::PromptResponse, AcpError> {
        let content = acp::ContentBlock::from(text);
        let req = acp::PromptRequest { session_id, prompt: vec![content], meta: None };
        let resp: acp::PromptResponse = self
            .send_request(acp::AGENT_METHOD_NAMES.session_prompt, &req)
            .await
            .map_err(|e| AcpError::Protocol(format!("prompt failed: {e}")))?;
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
        self.stdin.write_all(&data).await.map_err(|e| AcpError::Io(e.to_string()))?;
        self.stdin.flush().await.map_err(|e| AcpError::Io(e.to_string()))?;

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
