use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol_schema as acp;
use futures::StreamExt;
use opencode_sdk::{
    create_opencode, Event, EventStream, OpencodeClient, OpencodeServer, ServerOptions,
};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::error::{AcpError, Result};
use crate::session::AcpAgentSession;

#[derive(Debug, Clone)]
pub struct OpencodeAgentConfig {
    pub executable_path: Option<PathBuf>,
    pub port: u16,
    pub hostname: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub timeout_ms: u64,
}

impl Default for OpencodeAgentConfig {
    fn default() -> Self {
        Self {
            executable_path: None,
            port: 4096,
            hostname: "127.0.0.1".to_string(),
            model: None,
            provider: None,
            timeout_ms: 30000,
        }
    }
}

impl OpencodeAgentConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn executable_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable_path = Some(path.into());
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = hostname.into();
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }
}

pub struct OpencodeAgentConnection {
    client: Arc<OpencodeClient>,
    server: Arc<Mutex<Option<OpencodeServer>>>,
    sessions: Arc<RwLock<std::collections::HashMap<String, AcpAgentSession>>>,
    notification_tx: mpsc::Sender<acp::SessionNotification>,
    _config: OpencodeAgentConfig,
}

impl OpencodeAgentConnection {
    pub async fn connect(
        config: OpencodeAgentConfig,
        root_dir: &Path,
    ) -> Result<(Self, mpsc::Receiver<acp::SessionNotification>)> {
        let options = ServerOptions::default()
            .port(config.port)
            .hostname(&config.hostname)
            .timeout_ms(config.timeout_ms)
            .directory(root_dir);

        let options = if let Some(ref path) = config.executable_path {
            options.executable(path)
        } else {
            options
        };

        info!(
            port = config.port,
            hostname = %config.hostname,
            "Connecting to OpenCode server"
        );

        let (client, server) = create_opencode(options)
            .await
            .map_err(|e| AcpError::ConnectionFailed(format!("OpenCode server failed: {}", e)))?;

        let (notification_tx, notification_rx) = mpsc::channel(1000);

        let connection = Self {
            client: Arc::new(client),
            server: Arc::new(Mutex::new(Some(server))),
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            notification_tx,
            _config: config,
        };

        connection.start_event_listener().await?;

        Ok((connection, notification_rx))
    }

    async fn start_event_listener(&self) -> Result<()> {
        let mut events = self
            .client
            .events()
            .await
            .map_err(|e| AcpError::ConnectionFailed(format!("Event stream failed: {}", e)))?;

        let tx = self.notification_tx.clone();
        let sessions = self.sessions.clone();

        tokio::spawn(async move {
            while let Some(event_result) = events.next_event().await {
                match event_result {
                    Ok(event) => {
                        if let Some(notification) =
                            convert_event_to_notification(&event, &sessions).await
                        {
                            if tx.send(notification).await.is_err() {
                                debug!("Notification receiver dropped, stopping event listener");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Event stream error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn new_session(&self, cwd: PathBuf) -> Result<AcpAgentSession> {
        let opencode_session = self
            .client
            .session_create(Default::default())
            .await
            .map_err(|e| AcpError::SessionCreationFailed(e.to_string()))?;

        let session_id = acp::SessionId::new(&opencode_session.id);
        let session = AcpAgentSession::new(session_id.clone(), cwd);

        self.sessions
            .write()
            .await
            .insert(opencode_session.id.clone(), session.clone());

        Ok(session)
    }

    pub async fn prompt(
        &self,
        session_id: &acp::SessionId,
        content: impl Into<String>,
    ) -> Result<()> {
        let id = session_id.as_str();
        self.client
            .session_prompt(id, content)
            .await
            .map_err(|e| AcpError::RequestFailed(e.to_string()))?;
        Ok(())
    }

    pub async fn cancel(&self, session_id: &acp::SessionId) -> Result<()> {
        let id = session_id.as_str();
        self.client
            .session_abort(id)
            .await
            .map_err(|e| AcpError::RequestFailed(e.to_string()))?;
        Ok(())
    }

    pub async fn get_session(&self, session_id: &str) -> Option<AcpAgentSession> {
        self.sessions.read().await.get(session_id).cloned()
    }

    pub async fn list_sessions(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }

    pub async fn close_session(&self, session_id: &str) -> Result<()> {
        self.client
            .session_delete(session_id)
            .await
            .map_err(|e| AcpError::RequestFailed(e.to_string()))?;
        self.sessions.write().await.remove(session_id);
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        if let Some(server) = self.server.lock().await.take() {
            server
                .close()
                .await
                .map_err(|e| AcpError::ConnectionFailed(e.to_string()))?;
        }
        Ok(())
    }

    pub fn client(&self) -> &OpencodeClient {
        &self.client
    }
}

async fn convert_event_to_notification(
    event: &Event,
    sessions: &Arc<RwLock<std::collections::HashMap<String, AcpAgentSession>>>,
) -> Option<acp::SessionNotification> {
    match event {
        Event::MessagePartUpdated { part, delta } => {
            if let Some(text) = delta {
                let part_obj = part.as_object()?;
                let session_id = part_obj.get("sessionID")?.as_str()?;

                let chunk = acp::AgentMessageChunk {
                    content: text.clone(),
                    is_final: false,
                };

                Some(acp::SessionNotification {
                    session_id: acp::SessionId::new(session_id),
                    update: acp::SessionUpdate::AgentMessageChunk { chunk },
                })
            } else {
                None
            }
        }

        Event::SessionIdle { session_id } => Some(acp::SessionNotification {
            session_id: acp::SessionId::new(session_id),
            update: acp::SessionUpdate::Idle,
        }),

        Event::SessionStatus { session_id, status } => {
            let status_update = match status {
                opencode_sdk::events::SessionStatus::Busy => acp::SessionUpdate::Working,
                opencode_sdk::events::SessionStatus::Idle => acp::SessionUpdate::Idle,
                opencode_sdk::events::SessionStatus::Compacting => acp::SessionUpdate::Working,
            };

            Some(acp::SessionNotification {
                session_id: acp::SessionId::new(session_id),
                update: status_update,
            })
        }

        Event::SessionError { session_id, error } => {
            let sid = session_id.as_ref()?;
            Some(acp::SessionNotification {
                session_id: acp::SessionId::new(sid),
                update: acp::SessionUpdate::Error {
                    error: acp::SessionError {
                        message: error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                        code: None,
                        recoverable: false,
                    },
                },
            })
        }

        Event::PermissionUpdated { permission } => {
            let session_id = &permission.session_id;
            Some(acp::SessionNotification {
                session_id: acp::SessionId::new(session_id),
                update: acp::SessionUpdate::PermissionRequest {
                    request: acp::PermissionRequest {
                        id: acp::PermissionRequestId::new(&permission.id),
                        permission_type: permission.tool.clone(),
                        message: permission
                            .metadata
                            .as_ref()
                            .and_then(|m| m.get("message"))
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        metadata: permission.metadata.clone(),
                    },
                },
            })
        }

        Event::TodoUpdated { session_id, todos } => {
            let items: Vec<acp::PlanItem> = todos
                .iter()
                .map(|t| acp::PlanItem {
                    id: acp::PlanItemId::new(&t.id),
                    content: t.content.clone(),
                    status: match t.status {
                        opencode_sdk::TodoStatus::Pending => acp::PlanItemStatus::Pending,
                        opencode_sdk::TodoStatus::InProgress => acp::PlanItemStatus::InProgress,
                        opencode_sdk::TodoStatus::Completed => acp::PlanItemStatus::Completed,
                        opencode_sdk::TodoStatus::Cancelled => acp::PlanItemStatus::Cancelled,
                    },
                })
                .collect();

            Some(acp::SessionNotification {
                session_id: acp::SessionId::new(session_id),
                update: acp::SessionUpdate::PlanUpdate {
                    plan: acp::Plan { items },
                },
            })
        }

        _ => None,
    }
}

pub async fn connect_opencode(
    config: OpencodeAgentConfig,
    root_dir: &Path,
) -> Result<(OpencodeAgentConnection, mpsc::Receiver<acp::SessionNotification>)> {
    OpencodeAgentConnection::connect(config, root_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = OpencodeAgentConfig::new()
            .port(5000)
            .hostname("localhost")
            .model("codex-sonnet-4");

        assert_eq!(config.port, 5000);
        assert_eq!(config.hostname, "localhost");
        assert_eq!(config.model, Some("codex-sonnet-4".to_string()));
    }
}
