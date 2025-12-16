//! Background chat handler for routing messages to AI backends.
//!
//! Runs in a separate thread with a tokio runtime to handle async
//! operations while the main thread runs the winit event loop.

use mechacoder::{Backend, ClientMessage, Router, RouterConfig, ServerMessage};
use std::thread::JoinHandle;
use tokio::sync::mpsc;

/// Spawns a background thread with a tokio runtime to handle chat messages.
///
/// Returns a JoinHandle for the spawned thread (can be ignored if you don't
/// need to wait for it to complete).
pub fn spawn_chat_handler(
    client_rx: mpsc::UnboundedReceiver<ClientMessage>,
    server_tx: mpsc::UnboundedSender<ServerMessage>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        runtime.block_on(async move {
            let mut handler = ChatHandler::new(client_rx, server_tx);
            handler.run().await;
        });
    })
}

/// Background handler that routes messages to AI backends.
struct ChatHandler {
    client_rx: mpsc::UnboundedReceiver<ClientMessage>,
    server_tx: mpsc::UnboundedSender<ServerMessage>,
    router: Router,
}

impl ChatHandler {
    fn new(
        client_rx: mpsc::UnboundedReceiver<ClientMessage>,
        server_tx: mpsc::UnboundedSender<ServerMessage>,
    ) -> Self {
        let mut router = Router::new(RouterConfig::default());
        router.detect_sync();

        let status = router.status();
        log::info!(
            "[ChatHandler] Detected backends: {:?}, active: {:?}",
            status.detected,
            status.active
        );

        Self {
            client_rx,
            server_tx,
            router,
        }
    }

    async fn run(&mut self) {
        log::info!("[ChatHandler] Starting message loop");

        while let Some(msg) = self.client_rx.recv().await {
            self.handle_message(msg).await;
        }

        log::info!("[ChatHandler] Message loop ended");
    }

    async fn handle_message(&mut self, msg: ClientMessage) {
        match msg {
            ClientMessage::SendMessage { content, cwd } => {
                log::info!("[ChatHandler] Received message: {}", content);

                match self.router.route() {
                    Some(Backend::ClaudeCode) => {
                        log::info!("[ChatHandler] Routing to Claude Code");
                        self.run_claude_code(&content, &cwd).await;
                    }
                    Some(backend) => {
                        log::info!("[ChatHandler] Routing to {:?} (stub)", backend);
                        self.run_local_agent(&content).await;
                    }
                    None => {
                        log::warn!("[ChatHandler] No backend available");
                        self.run_local_agent(&content).await;
                    }
                }
            }
            ClientMessage::Cancel => {
                log::info!("[ChatHandler] Cancel requested (not implemented)");
            }
        }
    }

    async fn run_claude_code(&self, message: &str, cwd: &str) {
        // Create a channel for the session to send messages
        let (session_tx, mut session_rx) = mpsc::unbounded_channel();

        // Spawn the Claude session
        let message = message.to_string();
        let cwd = cwd.to_string();
        tokio::spawn(async move {
            mechacoder::session::run_claude_session(message, cwd, session_tx).await;
        });

        // Forward messages from session to UI
        while let Some(msg) = session_rx.recv().await {
            log::info!(
                "[ChatHandler] Forwarding to UI: {:?}",
                std::mem::discriminant(&msg)
            );
            if self.server_tx.send(msg.clone()).is_err() {
                log::warn!("[ChatHandler] UI channel closed");
                break;
            }

            // Check if we're done
            if matches!(msg, ServerMessage::Done { .. }) {
                break;
            }
        }
    }

    async fn run_local_agent(&self, message: &str) {
        log::info!("[LocalAgent] Received message: {}", message);

        // Simulate a streaming response
        let response = format!(
            "Local agent stub received your message: \"{}\"\n\n\
             Claude Code is not available. Install it with:\n\
             npm install -g @anthropic/claude-code",
            message
        );

        // Stream the response character by character (with some chunking)
        for chunk in response.chars().collect::<Vec<_>>().chunks(10) {
            let text: String = chunk.iter().collect();
            if self
                .server_tx
                .send(ServerMessage::TextDelta { text })
                .is_err()
            {
                log::warn!("[LocalAgent] UI channel closed");
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }

        let _ = self.server_tx.send(ServerMessage::Done { error: None });
    }
}
