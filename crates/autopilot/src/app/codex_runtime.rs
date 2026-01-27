use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::app::codex_app_server as app_server;

#[derive(Clone, Default)]
pub(crate) struct CodexRuntimeConfig {
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) wire_log: Option<app_server::AppServerWireLog>,
}

pub(crate) struct CodexRuntime {
    pub(crate) client: app_server::AppServerClient,
    pub(crate) channels: app_server::AppServerChannels,
    #[allow(dead_code)]
    config: CodexRuntimeConfig,
}

impl CodexRuntime {
    pub(crate) fn is_available() -> bool {
        app_server::is_codex_available()
    }

    pub(crate) async fn spawn(config: CodexRuntimeConfig) -> Result<Self> {
        let (client, channels) = app_server::AppServerClient::spawn(app_server::AppServerConfig {
            cwd: config.cwd.clone(),
            wire_log: config.wire_log.clone(),
            env: Vec::new(),
        })
        .await
        .context("Failed to spawn codex app-server")?;

        let client_info = app_server::ClientInfo {
            name: "autopilot".to_string(),
            title: Some("Autopilot".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        if let Err(err) = client.initialize(client_info).await {
            let _ = client.shutdown().await;
            return Err(err).context("Failed to initialize codex app-server");
        }

        Ok(Self {
            client,
            channels,
            config,
        })
    }

    #[allow(dead_code)]
    pub(crate) async fn restart(self) -> Result<Self> {
        let _ = self.client.shutdown().await;
        Self::spawn(self.config).await
    }

    #[allow(dead_code)]
    pub(crate) async fn shutdown(self) -> Result<()> {
        self.client.shutdown().await
    }
}
