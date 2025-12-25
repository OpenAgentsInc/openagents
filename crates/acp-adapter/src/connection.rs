//! ACP connection management
//!
//! This module provides the main connection type for communicating with
//! AI agents via the Agent Client Protocol.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol_schema as acp;
use tokio::process::Child;
use tokio::sync::RwLock;

use crate::AgentCommand;
use crate::error::{AcpError, Result};
use crate::session::AcpAgentSession;
use crate::telemetry::{ActionEvent, ApmTelemetry};
use crate::transport::StdioTransport;

/// ACP connection to an agent subprocess
///
/// Manages the lifecycle of an agent process and provides methods for
/// creating sessions, sending prompts, and handling responses.
pub struct AcpAgentConnection {
    /// Agent name for display/logging
    pub agent_name: String,

    /// Stdio transport for JSON-RPC communication
    transport: Arc<StdioTransport>,

    /// Active sessions indexed by session ID
    sessions: Arc<RwLock<HashMap<String, AcpAgentSession>>>,

    /// Agent capabilities from initialization
    agent_capabilities: acp::AgentCapabilities,

    /// Working directory for the agent
    #[allow(dead_code)]
    root_dir: PathBuf,

    /// Child process handle
    child: Child,

    /// Protocol version negotiated with agent
    protocol_version: acp::ProtocolVersion,
}

impl AcpAgentConnection {
    /// Create a new ACP connection via stdio transport
    ///
    /// Spawns the agent as a subprocess and performs the ACP initialization
    /// handshake to negotiate capabilities.
    pub async fn stdio(
        agent_name: impl Into<String>,
        command: AgentCommand,
        root_dir: &Path,
    ) -> Result<Self> {
        let agent_name = agent_name.into();

        // Spawn the agent subprocess
        let mut cmd = tokio::process::Command::new(&command.path);
        cmd.args(&command.args)
            .envs(command.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .current_dir(root_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        tracing::info!(
            agent = %agent_name,
            path = %command.path.display(),
            "Spawning agent subprocess"
        );

        let mut child = cmd.spawn().map_err(|e| {
            AcpError::SpawnError(std::io::Error::new(
                e.kind(),
                format!("Failed to spawn {}: {}", agent_name, e),
            ))
        })?;

        // Take stdio handles
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AcpError::SpawnError(std::io::Error::other("Failed to take stdin")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AcpError::SpawnError(std::io::Error::other("Failed to take stdout")))?;

        // Create transport
        let transport = Arc::new(StdioTransport::new(stdin, stdout));

        // Perform initialization handshake
        let init_request = acp::InitializeRequest::new(acp::ProtocolVersion::V1)
            .client_capabilities(
                acp::ClientCapabilities::new()
                    .fs(acp::FileSystemCapability::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true),
            )
            .client_info(acp::Implementation::new(
                "openagents",
                env!("CARGO_PKG_VERSION"),
            ));

        tracing::debug!("Sending initialize request");

        let init_response: acp::InitializeResponse = transport
            .request("initialize", &init_request)
            .await
            .map_err(|e| AcpError::InitializationError(e.to_string()))?;

        tracing::info!(
            agent = %agent_name,
            protocol_version = ?init_response.protocol_version,
            "Agent initialized"
        );

        Ok(Self {
            agent_name,
            transport,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            agent_capabilities: init_response.agent_capabilities,
            root_dir: root_dir.to_path_buf(),
            child,
            protocol_version: init_response.protocol_version,
        })
    }

    /// Get the agent's capabilities
    pub fn capabilities(&self) -> &acp::AgentCapabilities {
        &self.agent_capabilities
    }

    /// Get the negotiated protocol version
    pub fn protocol_version(&self) -> &acp::ProtocolVersion {
        &self.protocol_version
    }

    /// Create a new session with the agent
    pub async fn new_session(&self, cwd: PathBuf) -> Result<AcpAgentSession> {
        let request = acp::NewSessionRequest::new(cwd.clone());

        tracing::debug!(cwd = %cwd.display(), "Creating new session");

        let response: acp::NewSessionResponse = self
            .transport
            .request("session/new", &request)
            .await
            .map_err(|e| AcpError::ProtocolError(e.to_string()))?;

        let session =
            AcpAgentSession::new(response.session_id.clone(), self.transport.clone(), cwd);

        // Store session
        let session_id = response.session_id.to_string();
        self.sessions
            .write()
            .await
            .insert(session_id.clone(), session.clone());

        tracing::info!(session_id = %session_id, "Session created");

        Ok(session)
    }

    /// Create a new session with APM telemetry enabled
    ///
    /// Returns both the session and a receiver for ActionEvents.
    /// The caller should spawn a task to consume events from the receiver.
    pub async fn new_session_with_telemetry(
        &self,
        cwd: PathBuf,
    ) -> Result<(
        AcpAgentSession,
        tokio::sync::mpsc::UnboundedReceiver<ActionEvent>,
    )> {
        let request = acp::NewSessionRequest::new(cwd.clone());

        tracing::debug!(cwd = %cwd.display(), "Creating new session with telemetry");

        let response: acp::NewSessionResponse = self
            .transport
            .request("session/new", &request)
            .await
            .map_err(|e| AcpError::ProtocolError(e.to_string()))?;

        let session_id = response.session_id.to_string();

        // Create telemetry tracker
        let (telemetry, rx) = ApmTelemetry::new(&session_id);
        let telemetry = Arc::new(telemetry);

        let session = AcpAgentSession::with_telemetry(
            response.session_id.clone(),
            self.transport.clone(),
            cwd,
            telemetry,
        );

        // Store session
        self.sessions
            .write()
            .await
            .insert(session_id.clone(), session.clone());

        tracing::info!(session_id = %session_id, "Session with telemetry created");

        Ok((session, rx))
    }

    /// Send a prompt to a session
    pub async fn prompt(
        &self,
        session_id: &acp::SessionId,
        content: impl Into<String>,
    ) -> Result<acp::PromptResponse> {
        let content = content.into();

        tracing::debug!(
            session_id = %session_id,
            content_len = content.len(),
            "Sending prompt"
        );

        let request = acp::PromptRequest::new(
            session_id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(content))],
        );

        let response: acp::PromptResponse = self
            .transport
            .request("session/prompt", &request)
            .await
            .map_err(|e| AcpError::ProtocolError(e.to_string()))?;

        Ok(response)
    }

    /// Cancel ongoing work in a session
    pub async fn cancel(&self, session_id: &acp::SessionId) {
        let notification = acp::CancelNotification::new(session_id.clone());

        if let Err(e) = self.transport.notify("session/cancel", &notification).await {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "Failed to send cancel notification"
            );
        }
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<AcpAgentSession> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str) -> Result<()> {
        self.sessions.write().await.remove(session_id);
        tracing::info!(session_id = %session_id, "Session closed");
        Ok(())
    }

    /// Check if the agent process is still running
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for AcpAgentConnection {
    fn drop(&mut self) {
        // Kill the child process when the connection is dropped
        if let Err(e) = self.child.start_kill() {
            tracing::warn!(error = %e, "Failed to kill agent process");
        }
    }
}
