//! Control socket server for daemon management

use crate::daemon::supervisor::{DaemonMetrics, SharedMetrics, WorkerSupervisor};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

/// Request types for control socket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlRequest {
    /// Get daemon status
    Status,
    /// Restart the worker
    RestartWorker,
    /// Stop the worker
    StopWorker,
    /// Start the worker
    StartWorker,
    /// Get metrics
    GetMetrics,
    /// Shutdown the daemon
    Shutdown,
}

/// Response from control socket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ControlResponse {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn ok_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}

/// Control socket server
pub struct ControlServer {
    socket_path: std::path::PathBuf,
}

impl ControlServer {
    /// Create a new control server
    pub fn new(socket_path: impl AsRef<Path>) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
        }
    }

    /// Start the control server
    ///
    /// The `shared_metrics` is read directly for Status requests without needing
    /// to lock the supervisor. Other commands that modify state still lock the supervisor.
    pub async fn run(
        &self,
        supervisor: Arc<Mutex<WorkerSupervisor>>,
        shared_metrics: SharedMetrics,
        shutdown_tx: tokio::sync::mpsc::Sender<()>,
    ) -> Result<()> {
        // Remove stale socket
        let _ = std::fs::remove_file(&self.socket_path);

        // Ensure parent directory exists
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        eprintln!("Control socket listening at {:?}", self.socket_path);

        // The loop will be terminated when the task is aborted
        // tokio::select! will propagate the cancellation
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let supervisor = supervisor.clone();
                            let shared_metrics = shared_metrics.clone();
                            let shutdown_tx = shutdown_tx.clone();

                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, supervisor, shared_metrics, shutdown_tx).await {
                                    eprintln!("Control connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Error accepting connection: {}", e);
                        }
                    }
                }
            }
        }
    }
}

/// Handle a single control connection
async fn handle_connection(
    mut stream: UnixStream,
    supervisor: Arc<Mutex<WorkerSupervisor>>,
    shared_metrics: SharedMetrics,
    shutdown_tx: tokio::sync::mpsc::Sender<()>,
) -> Result<()> {
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;

    if n == 0 {
        return Ok(());
    }

    let request: ControlRequest = serde_json::from_slice(&buf[..n])?;
    let response = process_request(request, supervisor, shared_metrics, shutdown_tx).await;

    let response_bytes = serde_json::to_vec(&response)?;
    stream.write_all(&response_bytes).await?;

    Ok(())
}

/// Process a control request
async fn process_request(
    request: ControlRequest,
    supervisor: Arc<Mutex<WorkerSupervisor>>,
    shared_metrics: SharedMetrics,
    shutdown_tx: tokio::sync::mpsc::Sender<()>,
) -> ControlResponse {
    match request {
        ControlRequest::Status => {
            // Read from shared metrics without blocking the supervisor
            let metrics = shared_metrics
                .read()
                .map(|guard| guard.clone())
                .unwrap_or_default();
            ControlResponse::ok_with_data(
                format!("Worker is {}", metrics.worker_status),
                serde_json::to_value(&metrics).unwrap_or_default(),
            )
        }
        ControlRequest::GetMetrics => {
            // Read from shared metrics without blocking the supervisor
            let metrics = shared_metrics
                .read()
                .map(|guard| guard.clone())
                .unwrap_or_default();
            ControlResponse::ok_with_data(
                "Metrics retrieved",
                serde_json::to_value(&metrics).unwrap_or_default(),
            )
        }
        ControlRequest::RestartWorker => {
            let mut guard = supervisor.lock().await;
            match guard.restart_worker() {
                Ok(_) => ControlResponse::ok("Worker restart initiated"),
                Err(e) => ControlResponse::error(format!("Failed to restart worker: {}", e)),
            }
        }
        ControlRequest::StopWorker => {
            let mut guard = supervisor.lock().await;
            guard.stop_worker();
            ControlResponse::ok("Worker stopped")
        }
        ControlRequest::StartWorker => {
            let mut guard = supervisor.lock().await;
            match guard.spawn_worker() {
                Ok(_) => ControlResponse::ok("Worker started"),
                Err(e) => ControlResponse::error(format!("Failed to start worker: {}", e)),
            }
        }
        ControlRequest::Shutdown => {
            let _ = shutdown_tx.send(()).await;
            ControlResponse::ok("Daemon shutting down")
        }
    }
}

/// Client for connecting to the control socket
pub struct ControlClient {
    socket_path: std::path::PathBuf,
}

impl ControlClient {
    /// Create a new control client
    pub fn new(socket_path: impl AsRef<Path>) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
        }
    }

    /// Send a request and get a response
    pub async fn send(&self, request: ControlRequest) -> Result<ControlResponse> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;

        let request_bytes = serde_json::to_vec(&request)?;
        stream.write_all(&request_bytes).await?;

        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await?;

        let response: ControlResponse = serde_json::from_slice(&buf[..n])?;
        Ok(response)
    }

    /// Get status
    pub async fn status(&self) -> Result<DaemonMetrics> {
        let response = self.send(ControlRequest::Status).await?;
        if let Some(data) = response.data {
            let metrics: DaemonMetrics = serde_json::from_value(data)?;
            Ok(metrics)
        } else {
            anyhow::bail!("No metrics in response")
        }
    }

    /// Restart worker
    pub async fn restart_worker(&self) -> Result<()> {
        let response = self.send(ControlRequest::RestartWorker).await?;
        if response.success {
            Ok(())
        } else {
            anyhow::bail!("{}", response.message)
        }
    }

    /// Stop worker
    pub async fn stop_worker(&self) -> Result<()> {
        let response = self.send(ControlRequest::StopWorker).await?;
        if response.success {
            Ok(())
        } else {
            anyhow::bail!("{}", response.message)
        }
    }

    /// Shutdown daemon
    pub async fn shutdown(&self) -> Result<()> {
        let response = self.send(ControlRequest::Shutdown).await?;
        if response.success {
            Ok(())
        } else {
            anyhow::bail!("{}", response.message)
        }
    }
}
