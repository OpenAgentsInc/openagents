//! Codex backend implementation
//!
//! Uses the Codex app-server for availability checks and model discovery.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use anyhow::anyhow;
use tokio::sync::mpsc;

use crate::app::codex_app_server as app_server;
use crate::app::codex_runtime::{CodexRuntime, CodexRuntimeConfig};

use super::backend::{
    AgentAvailability, AgentBackend, AgentConfig, AgentKind, AgentSession, ModelInfo,
};
use crate::app::events::ResponseEvent;

const CODEX_PATHS: &[&str] = &[
    ".npm-global/bin/codex",
    ".local/bin/codex",
    "node_modules/.bin/codex",
    "/usr/local/bin/codex",
    "/opt/homebrew/bin/codex",
];

/// Codex backend
pub struct CodexBackend {
    /// Cached availability status
    availability: std::sync::RwLock<Option<AgentAvailability>>,
}

impl CodexBackend {
    pub fn new() -> Self {
        Self {
            availability: std::sync::RwLock::new(None),
        }
    }
}

impl Default for CodexBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentBackend for CodexBackend {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn check_availability(&self) -> AgentAvailability {
        if let Some(cached) = self.availability.read().unwrap().as_ref() {
            return cached.clone();
        }

        let result = check_codex_availability();
        *self.availability.write().unwrap() = Some(result.clone());
        result
    }

    fn available_models(&self) -> Pin<Box<dyn Future<Output = Vec<ModelInfo>> + Send + '_>> {
        Box::pin(async move {
            let cwd = std::env::current_dir().ok();
            let runtime = match CodexRuntime::spawn(CodexRuntimeConfig {
                cwd,
                wire_log: None,
            })
            .await
            {
                Ok(runtime) => runtime,
                Err(_) => return Vec::new(),
            };
            let CodexRuntime { client, .. } = runtime;

            let mut models = Vec::new();
            let mut cursor: Option<String> = None;
            loop {
                let response = match client
                    .model_list(app_server::ModelListParams {
                        cursor: cursor.clone(),
                        limit: Some(50),
                    })
                    .await
                {
                    Ok(response) => response,
                    Err(_) => break,
                };
                for model in response.data {
                    let name = if model.display_name.is_empty() {
                        model.model.clone()
                    } else {
                        model.display_name.clone()
                    };
                    let description = if model.description.is_empty() {
                        None
                    } else {
                        Some(model.description.clone())
                    };
                    models.push(ModelInfo {
                        id: model.id,
                        name,
                        description,
                        is_default: model.is_default,
                    });
                }
                if response.next_cursor.is_none() {
                    break;
                }
                cursor = response.next_cursor;
            }

            let _ = client.shutdown().await;
            models
        })
    }

    fn default_model_id(&self) -> Option<&str> {
        None
    }

    fn connect(
        &self,
        _config: AgentConfig,
        _response_tx: mpsc::UnboundedSender<ResponseEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Box<dyn AgentSession>>> + Send + '_>> {
        Box::pin(async move {
            Err(anyhow!(
                "Codex sessions are driven through the app-server UI path."
            ))
        })
    }
}

fn check_codex_availability() -> AgentAvailability {
    if let Ok(path) = which::which("codex-app-server") {
        return AgentAvailability {
            available: true,
            executable_path: Some(path),
            version: None,
            error: None,
        };
    }

    if let Ok(path) = which::which("codex") {
        return AgentAvailability {
            available: true,
            executable_path: Some(path),
            version: None,
            error: None,
        };
    }

    if let Some(home) = dirs::home_dir() {
        for path in CODEX_PATHS {
            let full_path = if path.starts_with('/') {
                PathBuf::from(path)
            } else {
                home.join(path)
            };
            if full_path.exists() {
                return AgentAvailability {
                    available: true,
                    executable_path: Some(full_path),
                    version: None,
                    error: None,
                };
            }
        }
    }

    AgentAvailability {
        available: false,
        executable_path: None,
        version: None,
        error: Some("Codex CLI not found. Install with: npm install -g @openai/codex".to_string()),
    }
}
