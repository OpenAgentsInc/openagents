use crate::client::{OpencodeClient, OpencodeClientConfig};
use crate::error::{Error, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::time::{Duration, sleep};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub hostname: String,
    pub port: u16,
    pub directory: Option<PathBuf>,
    pub timeout_ms: u64,
    pub executable: Option<PathBuf>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            hostname: "127.0.0.1".to_string(),
            port: 4096,
            directory: None,
            timeout_ms: 30000,
            executable: None,
        }
    }
}

impl ServerOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = hostname.into();
        self
    }

    pub fn directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.directory = Some(dir.into());
        self
    }

    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    pub fn executable(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable = Some(path.into());
        self
    }
}

pub struct OpencodeServer {
    process: Child,
    url: String,
    port: u16,
}

impl OpencodeServer {
    pub async fn spawn(options: ServerOptions) -> Result<Self> {
        let executable = options
            .executable
            .unwrap_or_else(|| PathBuf::from("opencode"));

        let url = format!("http://{}:{}", options.hostname, options.port);

        info!("Starting OpenCode server at {}", url);

        let mut cmd = Command::new(&executable);
        cmd.arg("serve")
            .arg("--port")
            .arg(options.port.to_string())
            .arg("--hostname")
            .arg(&options.hostname)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(dir) = &options.directory {
            cmd.current_dir(dir);
        }

        let process = cmd.spawn().map_err(|e| Error::SpawnFailed {
            message: format!("Failed to spawn opencode: {}", e),
        })?;

        let server = Self {
            process,
            url: url.clone(),
            port: options.port,
        };

        server.wait_for_health(options.timeout_ms).await?;

        info!("OpenCode server ready at {}", url);

        Ok(server)
    }

    async fn wait_for_health(&self, timeout_ms: u64) -> Result<()> {
        let client = reqwest::Client::new();
        let health_url = format!("{}/global/health", self.url);
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        let mut attempts = 0u32;

        loop {
            attempts += 1;

            if start.elapsed() > timeout {
                return Err(Error::HealthCheckFailed { attempts });
            }

            match client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    debug!("Health check passed after {} attempts", attempts);
                    return Ok(());
                }
                Ok(resp) => {
                    debug!(
                        "Health check attempt {} returned status {}",
                        attempts,
                        resp.status()
                    );
                }
                Err(e) => {
                    debug!("Health check attempt {} failed: {}", attempts, e);
                }
            }

            sleep(Duration::from_millis(100)).await;
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn close(mut self) -> Result<()> {
        info!("Shutting down OpenCode server");

        if let Err(e) = self.process.kill().await {
            warn!("Failed to kill server process: {}", e);
        }

        Ok(())
    }

    pub fn is_running(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }
}

pub async fn create_opencode(options: ServerOptions) -> Result<(OpencodeClient, OpencodeServer)> {
    let server = OpencodeServer::spawn(options.clone()).await?;

    let client_config = OpencodeClientConfig::new()
        .base_url(server.url())
        .directory(options.directory.unwrap_or_else(|| PathBuf::from(".")));

    let client = OpencodeClient::new(client_config)?;

    Ok((client, server))
}
