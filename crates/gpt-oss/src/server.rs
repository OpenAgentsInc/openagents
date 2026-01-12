//! LlamaServerManager - Automatic llama-server lifecycle management
//!
//! Handles discovery, startup, health checking, and shutdown of llama-server.

use crate::GptOssClient;
use crate::error::ServerError;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Status of the llama-server process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    NotStarted,
    Starting,
    Running,
    Failed,
    Stopped,
}

/// Manages the llama-server process lifecycle
pub struct LlamaServerManager {
    child: Option<Child>,
    port: u16,
    model_path: Option<PathBuf>,
    binary_path: Option<PathBuf>,
    pub status: ServerStatus,
    pub error_message: Option<String>,
}

impl Default for LlamaServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LlamaServerManager {
    /// Create a new server manager with default settings
    pub fn new() -> Self {
        Self {
            child: None,
            port: 8000,
            model_path: None,
            binary_path: None,
            status: ServerStatus::NotStarted,
            error_message: None,
        }
    }

    /// Set the port for the server (default: 8000)
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set an explicit model path
    pub fn with_model(mut self, path: PathBuf) -> Self {
        self.model_path = Some(path);
        self
    }

    /// Set an explicit binary path (skips discovery)
    pub fn with_binary(mut self, path: PathBuf) -> Self {
        self.binary_path = Some(path);
        self
    }

    /// Get the server URL
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Check if llama-server binary is available
    pub fn is_available() -> bool {
        Self::find_binary().is_some()
    }

    /// Find llama-server binary in PATH or common locations
    pub fn find_binary() -> Option<PathBuf> {
        // First try `which llama-server`
        if let Ok(output) = Command::new("which").arg("llama-server").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    let p = PathBuf::from(&path);
                    if p.exists() {
                        debug!("Found llama-server via PATH: {}", path);
                        return Some(p);
                    }
                }
            }
        }

        // Fallback: check common installation paths
        let mut common_paths = vec![
            PathBuf::from("/usr/bin/llama-server"),
            PathBuf::from("/usr/local/bin/llama-server"),
            PathBuf::from("/opt/homebrew/bin/llama-server"), // macOS Homebrew
        ];

        // Check ~/code/llama.cpp build directory
        if let Some(home) = dirs::home_dir() {
            common_paths.push(home.join("code/llama.cpp/build/bin/llama-server"));
            common_paths.push(home.join("code/llama.cpp/llama-server"));
        }

        let common_paths = common_paths;

        for p in common_paths {
            if p.exists() {
                debug!("Found llama-server at: {:?}", p);
                return Some(p);
            }
        }

        None
    }

    /// Discover a GGUF model file from common locations
    ///
    /// Search order:
    /// 1. LLAMA_MODEL_PATH environment variable
    /// 2. ~/models/gpt-oss/*.gguf
    /// 3. ~/.local/share/llama/models/*.gguf
    pub fn discover_model() -> Option<PathBuf> {
        // 1. Check environment variable
        if let Ok(path) = std::env::var("LLAMA_MODEL_PATH") {
            let p = PathBuf::from(&path);
            if p.exists() {
                debug!("Using model from LLAMA_MODEL_PATH: {}", path);
                return Some(p);
            }
            warn!("LLAMA_MODEL_PATH set but file not found: {}", path);
        }

        // Get home directory
        let home = dirs::home_dir()?;

        // 2. Check ~/models/gpt-oss/
        let gpt_oss_models = home.join("models/gpt-oss");
        if let Some(model) = Self::find_gguf_in_dir(&gpt_oss_models) {
            return Some(model);
        }

        // 3. Check ~/.local/share/llama/models/
        let xdg_models = home.join(".local/share/llama/models");
        if let Some(model) = Self::find_gguf_in_dir(&xdg_models) {
            return Some(model);
        }

        // 4. Check ~/.openagents/models/
        let openagents_models = home.join(".openagents/models");
        if let Some(model) = Self::find_gguf_in_dir(&openagents_models) {
            return Some(model);
        }

        None
    }

    /// Find the first GGUF file in a directory, preferring smaller models (20b over 120b)
    fn find_gguf_in_dir(dir: &PathBuf) -> Option<PathBuf> {
        if !dir.exists() {
            return None;
        }

        let entries: Vec<_> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "gguf")
                    .unwrap_or(false)
            })
            .collect();

        if entries.is_empty() {
            return None;
        }

        let paths: Vec<PathBuf> = entries.into_iter().map(|e| e.path()).collect();

        // Prefer 20b model over larger ones
        for p in &paths {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.contains("20b") {
                    debug!("Discovered model (preferred 20b): {:?}", p);
                    return Some(p.clone());
                }
            }
        }

        // Otherwise return first one found
        let model = paths.into_iter().next()?;
        debug!("Discovered model: {:?}", model);
        Some(model)
    }

    /// Start the llama-server process
    pub fn start(&mut self) -> Result<(), ServerError> {
        self.status = ServerStatus::Starting;
        self.error_message = None;

        // Find binary
        let binary = self
            .binary_path
            .clone()
            .or_else(Self::find_binary)
            .ok_or(ServerError::BinaryNotFound)?;

        // Find model
        let model = self
            .model_path
            .clone()
            .or_else(Self::discover_model)
            .ok_or(ServerError::NoModelsDiscovered)?;

        if !model.exists() {
            return Err(ServerError::ModelNotFound(model));
        }

        info!("Starting llama-server:");
        info!("  Binary: {:?}", binary);
        info!("  Model: {:?}", model);
        info!("  Port: {}", self.port);

        // Spawn the server process
        let child = Command::new(&binary)
            .arg("--model")
            .arg(&model)
            .arg("--port")
            .arg(self.port.to_string())
            .arg("--chat-template")
            .arg("chatml")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ServerError::SpawnFailed)?;

        self.child = Some(child);
        Ok(())
    }

    /// Wait for the server to become healthy with a timeout
    pub async fn wait_ready_timeout(&mut self, timeout: Duration) -> Result<(), ServerError> {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(500);

        info!(
            "Waiting for server to become ready (timeout: {:?})...",
            timeout
        );

        let client = GptOssClient::with_base_url(&self.url())
            .map_err(|e| ServerError::HealthCheckFailed(e.to_string()))?;

        loop {
            // Check if process has exited
            if let Some(ref mut child) = self.child {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        self.status = ServerStatus::Failed;
                        let msg = format!("Server process exited with status: {}", status);
                        self.error_message = Some(msg.clone());
                        return Err(ServerError::ServerExited(msg));
                    }
                    Ok(None) => {} // Still running
                    Err(e) => {
                        self.status = ServerStatus::Failed;
                        let msg = format!("Failed to check process status: {}", e);
                        self.error_message = Some(msg.clone());
                        return Err(ServerError::ServerExited(msg));
                    }
                }
            }

            // Try health check
            match client.health().await {
                Ok(true) => {
                    self.status = ServerStatus::Running;
                    info!("Server is ready!");
                    return Ok(());
                }
                Ok(false) => {
                    debug!("Health check returned false, retrying...");
                }
                Err(e) => {
                    debug!("Health check failed: {}, retrying...", e);
                }
            }

            // Check timeout
            if start.elapsed() >= timeout {
                self.status = ServerStatus::Failed;
                let msg = "Health check timed out".to_string();
                self.error_message = Some(msg);
                return Err(ServerError::HealthCheckTimeout(timeout));
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Stop the server process
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            info!("Stopping llama-server (PID: {:?})...", child.id());
            let _ = child.kill();
            let _ = child.wait();
            self.status = ServerStatus::Stopped;
        }
    }

    /// Check if the server is currently running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,     // Still running
                Ok(Some(_)) => false, // Exited
                Err(_) => false,      // Error checking
            }
        } else {
            false
        }
    }
}

impl Drop for LlamaServerManager {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let manager = LlamaServerManager::new();
        assert_eq!(manager.port, 8000);
        assert_eq!(manager.status, ServerStatus::NotStarted);
        assert!(manager.model_path.is_none());
    }

    #[test]
    fn test_builder_pattern() {
        let manager = LlamaServerManager::new()
            .with_port(9000)
            .with_model(PathBuf::from("/tmp/test.gguf"));

        assert_eq!(manager.port, 9000);
        assert_eq!(manager.model_path, Some(PathBuf::from("/tmp/test.gguf")));
    }

    #[test]
    fn test_url_generation() {
        let manager = LlamaServerManager::new().with_port(8080);
        assert_eq!(manager.url(), "http://localhost:8080");
    }
}
