use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::backend::events::{AppServerEvent, EventSink, TerminalOutput};
use crate::file_logger::FileLogger;

#[derive(Clone)]
pub(crate) struct TauriEventSink {
    app: AppHandle,
    file_logger: Arc<FileLogger>,
}

impl TauriEventSink {
    pub(crate) async fn new(app: AppHandle) -> anyhow::Result<Self> {
        let file_logger = match FileLogger::new().await {
            Ok(logger) => {
                Arc::new(logger)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create FileLogger");
                // Create a dummy logger that does nothing
                return Err(e);
            }
        };
        Ok(Self { app, file_logger })
    }

    /// Flush all buffered events (call on disconnect)
    #[expect(dead_code)]
    pub(crate) async fn flush_all(&self) {
        if let Err(e) = self.file_logger.flush_all_app_server_events().await {
            tracing::warn!(error = %e, "Failed to flush all app-server events");
        }
        if let Err(e) = self.file_logger.flush_all_acp_events().await {
            tracing::warn!(error = %e, "Failed to flush all ACP events");
        }
    }
}

impl EventSink for TauriEventSink {
    fn emit_app_server_event(&self, event: AppServerEvent) {
        // Emit to frontend
        let _ = self.app.emit("app-server-event", &event);

        // Buffer event and flush when message completes (spawn async task to avoid blocking)
        let logger = self.file_logger.clone();
        let event_value = serde_json::to_value(&event).unwrap_or_default();
        tokio::spawn(async move {
            if let Err(e) = logger.check_and_flush_app_server(&event_value).await {
                tracing::warn!(error = %e, "Failed to buffer/flush app-server event");
            }
        });
    }

    fn emit_terminal_output(&self, event: TerminalOutput) {
        let _ = self.app.emit("terminal-output", event);
    }
}
