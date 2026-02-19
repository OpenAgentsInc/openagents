//! AI Server Management for Autopilot
//!
//! Manages the lifecycle of the local bun server that provides LM backend
//! for the Adjutant agent's DSPy pipeline.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};

pub mod config;

pub use config::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub uptime: i64,
    pub version: String,
    pub models: Vec<String>,
}

#[derive(Debug)]
pub struct AiServerManager {
    process: Option<Child>,
    config: AiServerConfig,
    client: reqwest::Client,
}

impl AiServerManager {
    pub fn new(config: AiServerConfig) -> Self {
        Self {
            process: None,
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Start the AI Gateway server
    pub async fn start(&mut self) -> Result<()> {
        // Check if server is already running
        if self.is_running().await {
            info!(port = self.config.port, "AI Gateway server already running");
            return Ok(());
        }

        // Ensure port is available
        self.ensure_port_available().await?;

        // Find ai-server directory
        let ai_server_path = Self::find_ai_server_path()?;

        info!(
            host = %self.config.host,
            port = self.config.port,
            "Starting AI Gateway server"
        );

        // Install dependencies if needed
        self.install_dependencies(&ai_server_path).await?;

        // Spawn bun server process
        let cmd = Command::new("bun")
            .args(["run", "server.ts"])
            .current_dir(&ai_server_path)
            .env("AI_SERVER_PORT", self.config.port.to_string())
            .env("AI_SERVER_HOST", &self.config.host)
            .env("AI_GATEWAY_API_KEY", &self.config.api_key)
            .env("NODE_ENV", "production")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start bun server: {}", e))?;

        // Wait for server to be ready
        self.wait_for_ready().await?;

        self.process = Some(cmd);

        info!("AI Gateway server started successfully");
        Ok(())
    }

    /// Stop the AI Gateway server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping AI Gateway server...");

            // Try graceful shutdown first
            if let Err(e) = process.kill() {
                warn!(error = %e, "Failed to kill server process");
            }

            // Wait for process to exit
            if let Err(e) = process.wait() {
                warn!(error = %e, "Failed to wait for server process");
            }

            info!("AI Gateway server stopped");
        }
        Ok(())
    }

    /// Check if the server is running and healthy
    pub async fn is_running(&self) -> bool {
        self.health_check().await.is_ok()
    }

    /// Get server health status
    pub async fn health_check(&self) -> Result<HealthResponse> {
        let url = format!("http://{}:{}/health", self.config.host, self.config.port);

        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Health check failed: {}", e))?;

        if response.status().is_success() {
            let health: HealthResponse = response
                .json()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to parse health response: {}", e))?;
            Ok(health)
        } else {
            Err(anyhow::anyhow!(
                "Server returned status: {}",
                response.status()
            ))
        }
    }

    /// Get server usage analytics
    pub async fn get_analytics(&self) -> Result<serde_json::Value> {
        let url = format!("http://{}:{}/analytics", self.config.host, self.config.port);

        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Analytics request failed: {}", e))?;

        if response.status().is_success() {
            let analytics: serde_json::Value = response
                .json()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to parse analytics response: {}", e))?;
            Ok(analytics)
        } else {
            Err(anyhow::anyhow!(
                "Analytics request failed with status: {}",
                response.status()
            ))
        }
    }

    /// Restart the server
    pub async fn restart(&mut self) -> Result<()> {
        info!("Restarting AI Gateway server...");
        self.stop().await?;
        sleep(Duration::from_secs(2)).await;
        self.start().await?;
        Ok(())
    }

    /// Find the ai-server directory relative to the binary
    fn find_ai_server_path() -> Result<PathBuf> {
        // Try different possible locations
        let possible_paths = vec![
            PathBuf::from("./ai-server"),
            PathBuf::from("../ai-server"),
            PathBuf::from("../../ai-server"),
        ];

        for path in possible_paths {
            if path.exists() && path.join("package.json").exists() {
                return Ok(path.canonicalize()?);
            }
        }

        Err(anyhow::anyhow!(
            "Could not find ai-server directory. Ensure it exists with package.json"
        ))
    }

    /// Install Node.js dependencies if needed
    async fn install_dependencies(&self, ai_server_path: &PathBuf) -> Result<()> {
        let node_modules = ai_server_path.join("node_modules");

        if !node_modules.exists() {
            info!("Installing AI server dependencies...");

            let output = Command::new("bun")
                .args(["install"])
                .current_dir(ai_server_path)
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to install dependencies: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!(
                    "Dependency installation failed: {}",
                    stderr
                ));
            }

            info!("Dependencies installed successfully");
        }

        Ok(())
    }

    /// Check if the configured port is available
    async fn ensure_port_available(&self) -> Result<()> {
        match std::net::TcpListener::bind(format!("{}:{}", self.config.host, self.config.port)) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(
                "Port {} is not available: {}. Please change AI_SERVER_PORT in your environment",
                self.config.port,
                e
            )),
        }
    }

    /// Wait for the server to become ready
    async fn wait_for_ready(&self) -> Result<()> {
        let start_time = Instant::now();
        let timeout = Duration::from_secs(30);
        let check_interval = Duration::from_millis(500);

        info!("Waiting for AI Gateway server to be ready...");

        while start_time.elapsed() < timeout {
            if self.health_check().await.is_ok() {
                info!("AI Gateway server is ready");
                return Ok(());
            }

            sleep(check_interval).await;
        }

        Err(anyhow::anyhow!(
            "AI Gateway server failed to start within {} seconds",
            timeout.as_secs()
        ))
    }
}

impl Drop for AiServerManager {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }
    }
}

/// Global server manager instance
static GLOBAL_AI_SERVER: LazyLock<Mutex<Option<AiServerManager>>> =
    LazyLock::new(|| Mutex::new(None));

/// Initialize the global AI server manager
pub fn init_ai_server(config: AiServerConfig) -> Result<()> {
    let mut server = GLOBAL_AI_SERVER
        .lock()
        .map_err(|e| anyhow::anyhow!("AI server lock poisoned: {}", e))?;
    *server = Some(AiServerManager::new(config));
    Ok(())
}

/// Start the global AI server
pub async fn start_ai_server() -> Result<()> {
    let server = {
        let mut server_guard = GLOBAL_AI_SERVER
            .lock()
            .map_err(|e| anyhow::anyhow!("AI server lock poisoned: {}", e))?;
        server_guard.take()
    };

    if let Some(mut server) = server {
        let result = server.start().await;
        // Put the server back
        {
            let mut server_guard = GLOBAL_AI_SERVER
                .lock()
                .map_err(|e| anyhow::anyhow!("AI server lock poisoned: {}", e))?;
            *server_guard = Some(server);
        }
        result
    } else {
        Err(anyhow::anyhow!(
            "AI server not initialized. Call init_ai_server() first"
        ))
    }
}

/// Stop the global AI server
pub async fn stop_ai_server() -> Result<()> {
    let server = {
        let mut server_guard = GLOBAL_AI_SERVER
            .lock()
            .map_err(|e| anyhow::anyhow!("AI server lock poisoned: {}", e))?;
        server_guard.take()
    };

    if let Some(mut server) = server {
        let result = server.stop().await;
        // Put the server back
        {
            let mut server_guard = GLOBAL_AI_SERVER
                .lock()
                .map_err(|e| anyhow::anyhow!("AI server lock poisoned: {}", e))?;
            *server_guard = Some(server);
        }
        result
    } else {
        Ok(()) // Already stopped or never started
    }
}

/// Check if the global AI server is running
pub async fn is_ai_server_running() -> bool {
    let server = {
        let server_guard = match GLOBAL_AI_SERVER.lock() {
            Ok(server_guard) => server_guard,
            Err(err) => {
                warn!(error = %err, "AI server lock poisoned");
                return false;
            }
        };
        (*server_guard).clone()
    };

    if let Some(server) = server {
        server.is_running().await
    } else {
        false
    }
}

impl Clone for AiServerManager {
    fn clone(&self) -> Self {
        Self {
            process: None, // Don't clone the process handle
            config: self.config.clone(),
            client: self.client.clone(),
        }
    }
}

/// Get the AI server health status
pub async fn get_ai_server_health() -> Result<HealthResponse> {
    let server = {
        let server_guard = GLOBAL_AI_SERVER
            .lock()
            .map_err(|e| anyhow::anyhow!("AI server lock poisoned: {}", e))?;
        (*server_guard).clone()
    };

    if let Some(server) = server {
        server.health_check().await
    } else {
        Err(anyhow::anyhow!("AI server not initialized"))
    }
}
