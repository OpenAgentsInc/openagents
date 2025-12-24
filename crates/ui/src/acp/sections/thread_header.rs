//! Thread header section component.

use maud::{Markup, html};
use crate::acp::atoms::{mode_badge, model_badge, AgentMode};

/// Connection status for the thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error,
}

impl ConnectionStatus {
    fn icon(&self) -> &'static str {
        match self {
            ConnectionStatus::Connected => "*",
            ConnectionStatus::Connecting => "o",
            ConnectionStatus::Disconnected => "o",
            ConnectionStatus::Error => "!",
        }
    }

    fn class(&self) -> &'static str {
        match self {
            ConnectionStatus::Connected => "text-green",
            ConnectionStatus::Connecting => "text-yellow animate-pulse",
            ConnectionStatus::Disconnected => "text-muted-foreground",
            ConnectionStatus::Error => "text-red",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Connecting => "Connecting...",
            ConnectionStatus::Disconnected => "Disconnected",
            ConnectionStatus::Error => "Error",
        }
    }
}

/// Thread header with session info.
pub struct ThreadHeader {
    session_id: String,
    mode: AgentMode,
    model_id: String,
    connection_status: ConnectionStatus,
}

impl ThreadHeader {
    /// Create a new thread header.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            mode: AgentMode::Code,
            model_id: String::new(),
            connection_status: ConnectionStatus::Connected,
        }
    }

    /// Set the agent mode.
    pub fn mode(mut self, mode: AgentMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the model ID.
    pub fn model(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = model_id.into();
        self
    }

    /// Set the connection status.
    pub fn connection_status(mut self, status: ConnectionStatus) -> Self {
        self.connection_status = status;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            header class="flex items-center gap-3 px-4 py-3 border-b border-border bg-card" {
                // Session icon
                span class="text-lg" { "[#]" }

                // Session ID
                div class="flex-1" {
                    h1 class="text-sm font-medium text-foreground" {
                        "Session"
                    }
                    p class="text-xs font-mono text-muted-foreground" {
                        (self.session_id)
                    }
                }

                // Mode badge
                (mode_badge(&self.mode))

                // Model badge
                @if !self.model_id.is_empty() {
                    (model_badge(&self.model_id, true))
                }

                // Connection status
                span
                    class={ "text-xs " (self.connection_status.class()) }
                    title=(self.connection_status.label())
                {
                    (self.connection_status.icon())
                }
            }
        }
    }
}
