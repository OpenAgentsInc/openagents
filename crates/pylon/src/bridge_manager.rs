//! FM Bridge subprocess manager
//!
//! Handles starting, health-checking, and stopping the Foundation Bridge Swift binary
//! for Apple Foundation Models support.

use crate::config::PylonConfig;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const DEFAULT_PORT: u16 = 11435;
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum BridgeError {
    BinaryNotFound,
    SpawnFailed(std::io::Error),
    HealthCheckTimeout,
    HealthCheckFailed(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::BinaryNotFound => write!(f, "FM Bridge binary not found"),
            BridgeError::SpawnFailed(e) => write!(f, "Failed to spawn FM Bridge: {}", e),
            BridgeError::HealthCheckTimeout => write!(f, "FM Bridge health check timed out"),
            BridgeError::HealthCheckFailed(e) => write!(f, "FM Bridge health check failed: {}", e),
        }
    }
}

impl std::error::Error for BridgeError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BridgeStatus {
    NotStarted,
    Starting,
    Running,
    Failed,
}

pub struct BridgeManager {
    child: Option<Child>,
    port: u16,
    pub status: BridgeStatus,
    pub error_message: Option<String>,
}

impl BridgeManager {
    pub fn new() -> Self {
        Self {
            child: None,
            port: DEFAULT_PORT,
            status: BridgeStatus::NotStarted,
            error_message: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Get the base URL for the FM Bridge
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Find the FM Bridge binary
    pub fn find_binary() -> Option<PathBuf> {
        // 1. User-installed location (~/.openagents/pylon/bin/foundation-bridge)
        if let Ok(pylon_dir) = PylonConfig::pylon_dir() {
            let path = pylon_dir.join("bin/foundation-bridge");
            if path.exists() {
                return Some(path);
            }
        }

        // 2. Next to current executable (for bundled app)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let path = dir.join("foundation-bridge");
                if path.exists() {
                    return Some(path);
                }
            }
        }

        // 3. Development paths (relative to cwd)
        let dev_paths = [
            "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge",
            "swift/foundation-bridge/.build/arm64-apple-macosx/debug/foundation-bridge",
            "../swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge",
            "../../../swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge",
        ];

        for path_str in &dev_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Check if the FM Bridge binary is available
    pub fn is_available() -> bool {
        Self::find_binary().is_some()
    }

    /// Start the FM Bridge process
    pub fn start(&mut self) -> Result<(), BridgeError> {
        self.status = BridgeStatus::Starting;

        let binary_path = Self::find_binary().ok_or(BridgeError::BinaryNotFound)?;

        tracing::info!("Starting FM Bridge from: {:?}", binary_path);

        let child = Command::new(&binary_path)
            .arg(self.port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(BridgeError::SpawnFailed)?;

        self.child = Some(child);
        Ok(())
    }

    /// Wait for the bridge to become healthy (blocking)
    pub fn wait_ready(&mut self) -> Result<(), BridgeError> {
        self.wait_ready_timeout(DEFAULT_STARTUP_TIMEOUT)
    }

    /// Wait for the bridge to become healthy with custom timeout (blocking)
    pub fn wait_ready_timeout(&mut self, timeout: Duration) -> Result<(), BridgeError> {
        let url = format!("{}/health", self.url());
        let deadline = Instant::now() + timeout;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(1))
            .build()
            .map_err(|e| BridgeError::HealthCheckFailed(e.to_string()))?;

        while Instant::now() < deadline {
            // Check if child process is still running
            if let Some(ref mut child) = self.child {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        self.status = BridgeStatus::Failed;
                        self.error_message = Some(format!("Bridge exited with status: {}", status));
                        return Err(BridgeError::HealthCheckFailed(
                            "Bridge process exited unexpectedly".to_string(),
                        ));
                    }
                    Ok(None) => {
                        // Still running, continue health check
                    }
                    Err(e) => {
                        self.status = BridgeStatus::Failed;
                        self.error_message = Some(e.to_string());
                        return Err(BridgeError::HealthCheckFailed(e.to_string()));
                    }
                }
            }

            // Try health check
            match client.get(&url).send() {
                Ok(resp) if resp.status().is_success() => {
                    self.status = BridgeStatus::Running;
                    tracing::info!("FM Bridge ready at {}", self.url());
                    return Ok(());
                }
                _ => {
                    std::thread::sleep(HEALTH_CHECK_INTERVAL);
                }
            }
        }

        self.status = BridgeStatus::Failed;
        self.error_message = Some("Health check timed out".to_string());
        Err(BridgeError::HealthCheckTimeout)
    }

    /// Check if the bridge is currently running
    pub fn is_running(&self) -> bool {
        self.status == BridgeStatus::Running
    }

    /// Stop the bridge process
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.status = BridgeStatus::NotStarted;
    }
}

impl Default for BridgeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BridgeManager {
    fn drop(&mut self) {
        self.stop();
    }
}
