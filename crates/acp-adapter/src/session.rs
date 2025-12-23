//! ACP session management
//!
//! Each session represents a conversation with an AI agent, maintaining
//! context, history, and optional trajectory collection.

use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol_schema as acp;
use tokio::sync::{mpsc, RwLock};

use crate::transport::StdioTransport;

/// An active ACP session with an agent
#[derive(Clone)]
pub struct AcpAgentSession {
    /// Session ID
    pub session_id: acp::SessionId,

    /// Working directory for this session
    pub cwd: PathBuf,

    /// Transport reference (shared with connection)
    transport: Arc<StdioTransport>,

    /// Session state
    state: Arc<RwLock<SessionState>>,

    /// Channel for session updates (notifications from agent)
    updates_tx: mpsc::Sender<acp::SessionNotification>,
}

/// Internal session state
struct SessionState {
    /// Current mode ID (if modes are supported)
    current_mode: Option<acp::SessionModeId>,

    /// Available modes
    available_modes: Vec<acp::SessionMode>,

    /// Whether the session is active
    is_active: bool,
}

impl AcpAgentSession {
    /// Create a new session
    pub(crate) fn new(
        session_id: acp::SessionId,
        transport: Arc<StdioTransport>,
        cwd: PathBuf,
    ) -> Self {
        let (updates_tx, _updates_rx) = mpsc::channel(256);

        Self {
            session_id,
            cwd,
            transport,
            state: Arc::new(RwLock::new(SessionState {
                current_mode: None,
                available_modes: Vec::new(),
                is_active: true,
            })),
            updates_tx,
        }
    }

    /// Get the session ID as a string
    pub fn id(&self) -> String {
        self.session_id.to_string()
    }

    /// Check if the session is active
    pub async fn is_active(&self) -> bool {
        self.state.read().await.is_active
    }

    /// Get the current mode ID
    pub async fn current_mode(&self) -> Option<acp::SessionModeId> {
        self.state.read().await.current_mode.clone()
    }

    /// Get available modes
    pub async fn available_modes(&self) -> Vec<acp::SessionMode> {
        self.state.read().await.available_modes.clone()
    }

    /// Set the session mode
    pub async fn set_mode(&self, mode_id: acp::SessionModeId) -> crate::Result<()> {
        let request = acp::SetSessionModeRequest::new(self.session_id.clone(), mode_id.clone());

        self.transport
            .request::<_, acp::SetSessionModeResponse>("session/set_mode", &request)
            .await?;

        // Update local state
        self.state.write().await.current_mode = Some(mode_id);

        Ok(())
    }

    /// Handle a session notification from the agent
    pub(crate) async fn handle_notification(&self, notification: acp::SessionNotification) {
        // Update local state based on notification type
        match &notification.update {
            acp::SessionUpdate::CurrentModeUpdate(mode_update) => {
                self.state.write().await.current_mode =
                    Some(mode_update.current_mode_id.clone());
            }
            _ => {}
        }

        // Forward to subscribers
        if self.updates_tx.send(notification).await.is_err() {
            tracing::warn!(
                session_id = %self.session_id,
                "No subscribers for session updates"
            );
        }
    }

    /// Subscribe to session updates
    ///
    /// Returns a receiver that will receive all session notifications.
    pub fn subscribe(&self) -> mpsc::Receiver<acp::SessionNotification> {
        // Create a new channel and return the receiver
        // Note: In a full implementation, we'd use a broadcast channel
        // For now, we'll create a simple forwarding channel
        let (tx, rx) = mpsc::channel(256);
        let updates_tx = self.updates_tx.clone();

        tokio::spawn(async move {
            // This is a simplified implementation
            // A full implementation would properly fan out to multiple subscribers
            drop(updates_tx);
            drop(tx);
        });

        rx
    }

    /// Mark the session as inactive
    #[allow(dead_code)]
    pub(crate) async fn deactivate(&self) {
        self.state.write().await.is_active = false;
    }
}
