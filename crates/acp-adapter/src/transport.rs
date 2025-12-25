//! Stdio transport for JSON-RPC communication
//!
//! Handles the low-level JSON-RPC message passing over stdin/stdout
//! following the ACP protocol specification.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::error::{AcpError, Result};

/// JSON-RPC request structure
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: T,
}

/// JSON-RPC notification structure
#[derive(Debug, Serialize)]
struct JsonRpcNotification<T> {
    jsonrpc: &'static str,
    method: String,
    params: T,
}

/// JSON-RPC response structure
#[derive(Debug, serde::Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Debug, serde::Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

/// Pending request awaiting a response
type PendingRequest = oneshot::Sender<Result<Value>>;

/// Stdio transport for ACP communication
pub struct StdioTransport {
    /// Stdin writer (protected by mutex for thread safety)
    stdin: Mutex<ChildStdin>,

    /// Request ID counter
    request_counter: AtomicU64,

    /// Pending requests awaiting responses
    pending_requests: Arc<Mutex<HashMap<u64, PendingRequest>>>,

    /// Channel for incoming notifications
    #[allow(dead_code)]
    notification_tx: mpsc::Sender<Value>,

    /// Handle to the reader task
    #[allow(dead_code)]
    reader_task: tokio::task::JoinHandle<()>,
}

impl StdioTransport {
    /// Create a new stdio transport
    pub fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        let pending_requests: Arc<Mutex<HashMap<u64, PendingRequest>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (notification_tx, _notification_rx) = mpsc::channel(256);

        // Spawn reader task
        let pending = pending_requests.clone();
        let notif_tx = notification_tx.clone();
        let reader_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        tracing::debug!("Agent stdout closed");
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<JsonRpcResponse>(trimmed) {
                            Ok(response) => {
                                if let Some(id) = response.id {
                                    // This is a response to a request
                                    let mut pending = pending.lock().await;
                                    if let Some(sender) = pending.remove(&id) {
                                        let result = if let Some(error) = response.error {
                                            Err(AcpError::ProtocolError(format!(
                                                "[{}] {}",
                                                error.code, error.message
                                            )))
                                        } else if let Some(result) = response.result {
                                            Ok(result)
                                        } else {
                                            Ok(Value::Null)
                                        };

                                        if sender.send(result).is_err() {
                                            tracing::warn!(id = id, "Response receiver dropped");
                                        }
                                    } else {
                                        tracing::warn!(
                                            id = id,
                                            "Received response for unknown request"
                                        );
                                    }
                                } else {
                                    // This is a notification
                                    if let Some(result) = response.result {
                                        if notif_tx.send(result).await.is_err() {
                                            tracing::warn!("Notification receiver dropped");
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    line = %trimmed,
                                    "Failed to parse JSON-RPC message"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Error reading from agent stdout");
                        break;
                    }
                }
            }

            // Clean up pending requests
            let mut pending = pending.lock().await;
            for (id, sender) in pending.drain() {
                tracing::debug!(
                    id = id,
                    "Cancelling pending request due to connection close"
                );
                let _ = sender.send(Err(AcpError::ConnectionClosed));
            }
        });

        Self {
            stdin: Mutex::new(stdin),
            request_counter: AtomicU64::new(1),
            pending_requests,
            notification_tx,
            reader_task,
        }
    }

    /// Send a request and wait for response
    pub async fn request<T, R>(&self, method: &str, params: &T) -> Result<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let id = self.request_counter.fetch_add(1, Ordering::SeqCst);

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        // Create response channel
        let (tx, rx) = oneshot::channel();

        // Register pending request
        self.pending_requests.lock().await.insert(id, tx);

        // Send request
        let json = serde_json::to_string(&request)?;
        tracing::trace!(id = id, method = %method, "Sending request");

        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        // Wait for response
        let result = rx.await.map_err(|_| AcpError::ConnectionClosed)??;

        // Deserialize response
        serde_json::from_value(result).map_err(|e| AcpError::SerializationError(e))
    }

    /// Send a notification (no response expected)
    pub async fn notify<T>(&self, method: &str, params: &T) -> Result<()>
    where
        T: Serialize,
    {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
        };

        let json = serde_json::to_string(&notification)?;
        tracing::trace!(method = %method, "Sending notification");

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        Ok(())
    }
}
