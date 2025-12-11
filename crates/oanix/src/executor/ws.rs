//! WebSocket connector that manages connections for WsFs.

use crate::executor::{ExecutorConfig, ExecutorError};
use crate::services::WsFs;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::{interval, sleep, timeout, Duration};
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream, WebSocketStream,
};

/// Handle to a live WebSocket connection.
struct ConnectionHandle {
    /// Connection ID
    conn_id: String,
    /// Task that reads from socket and writes to WsFs inbox
    inbox_task: JoinHandle<()>,
    /// Task that reads from WsFs outbox and writes to socket
    outbox_task: JoinHandle<()>,
    /// Task that sends ping frames for keepalive
    ping_task: JoinHandle<()>,
    /// Channel to send messages to the socket
    message_tx: tokio::sync::mpsc::Sender<Message>,
}

/// WebSocket connector that manages actual connections for WsFs.
///
/// The connector monitors WsFs for connection requests and manages
/// the lifecycle of actual WebSocket connections. Messages are routed
/// between WsFs buffers and actual sockets.
pub struct WsConnector {
    /// The WsFs to manage connections for
    ws_fs: Arc<WsFs>,
    /// Configuration
    config: ExecutorConfig,
    /// Shutdown signal receiver
    shutdown_rx: broadcast::Receiver<()>,
    /// Active connections
    connections: HashMap<String, ConnectionHandle>,
}

impl WsConnector {
    /// Create a new WebSocket connector.
    pub fn new(
        ws_fs: Arc<WsFs>,
        config: ExecutorConfig,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            ws_fs,
            config,
            shutdown_rx,
            connections: HashMap::new(),
        }
    }

    /// Run the connector loop.
    ///
    /// This will monitor WsFs for connection requests and manage
    /// the lifecycle of connections until shutdown.
    pub async fn run(mut self) {
        tracing::info!("WsConnector started");

        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    tracing::info!("WsConnector shutting down");
                    break;
                }
                _ = sleep(self.config.poll_interval) => {
                    self.process_connections().await;
                }
            }
        }

        // Clean up all connections
        self.shutdown_all().await;

        tracing::info!("WsConnector stopped");
    }

    /// Process connection state changes.
    async fn process_connections(&mut self) {
        // Handle new connections (Connecting state)
        self.handle_connecting().await;

        // Handle closing connections
        self.handle_closing().await;

        // Clean up closed/errored connections
        self.cleanup_finished();
    }

    /// Handle connections in Connecting state.
    async fn handle_connecting(&mut self) {
        let connecting = self.ws_fs.connecting_connections();

        for (conn_id, url) in connecting {
            if self.connections.contains_key(&conn_id) {
                continue; // Already being handled
            }

            // Check connection limit
            if self.connections.len() >= self.config.ws_max_concurrent {
                tracing::warn!(
                    "WebSocket connection limit reached ({}/{})",
                    self.connections.len(),
                    self.config.ws_max_concurrent
                );
                continue;
            }

            tracing::debug!("Opening WebSocket connection {} to {}", conn_id, url);

            match self.open_connection(&conn_id, &url).await {
                Ok(handle) => {
                    self.connections.insert(conn_id.clone(), handle);
                    let _ = self.ws_fs.set_connected(&conn_id);
                    tracing::info!("WebSocket connection {} opened", conn_id);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to open WebSocket connection {}: {}",
                        conn_id,
                        e
                    );
                    let _ = self.ws_fs.set_error(&conn_id, e.to_string());
                }
            }
        }
    }

    /// Handle connections in Closing state.
    async fn handle_closing(&mut self) {
        let closing = self.ws_fs.closing_connections();

        for conn_id in closing {
            if let Some(handle) = self.connections.remove(&conn_id) {
                tracing::debug!("Closing WebSocket connection {}", conn_id);

                // Send close message
                let _ = handle.message_tx.send(Message::Close(None)).await;

                // Abort tasks
                handle.inbox_task.abort();
                handle.outbox_task.abort();
                handle.ping_task.abort();

                let _ = self.ws_fs.set_closed(&conn_id);
                tracing::info!("WebSocket connection {} closed", conn_id);
            }
        }
    }

    /// Clean up connections that are no longer active in WsFs.
    fn cleanup_finished(&mut self) {
        let active_ids: Vec<String> = self.ws_fs.list_connections();
        let to_remove: Vec<String> = self
            .connections
            .keys()
            .filter(|id| !active_ids.contains(id))
            .cloned()
            .collect();

        for conn_id in to_remove {
            if let Some(handle) = self.connections.remove(&conn_id) {
                handle.inbox_task.abort();
                handle.outbox_task.abort();
                handle.ping_task.abort();
                tracing::debug!("Cleaned up WebSocket connection {}", conn_id);
            }
        }
    }

    /// Open a WebSocket connection.
    async fn open_connection(
        &self,
        conn_id: &str,
        url: &str,
    ) -> Result<ConnectionHandle, ExecutorError> {
        // Connect with timeout
        let (ws_stream, _response) = timeout(self.config.ws_connect_timeout, connect_async(url))
            .await
            .map_err(|_| {
                ExecutorError::Timeout(format!(
                    "Connection timed out after {:?}",
                    self.config.ws_connect_timeout
                ))
            })?
            .map_err(ExecutorError::from)?;

        let (write, read) = ws_stream.split();

        // Create message channel for outbox task
        let (message_tx, message_rx) = tokio::sync::mpsc::channel::<Message>(100);

        // Spawn inbox task (socket -> WsFs)
        let ws_fs_clone = Arc::clone(&self.ws_fs);
        let conn_id_clone = conn_id.to_string();
        let inbox_task = tokio::spawn(async move {
            Self::inbox_loop(ws_fs_clone, conn_id_clone, read).await;
        });

        // Spawn outbox task (WsFs -> socket)
        let ws_fs_clone = Arc::clone(&self.ws_fs);
        let conn_id_clone = conn_id.to_string();
        let poll_interval = self.config.poll_interval;
        let outbox_task = tokio::spawn(async move {
            Self::outbox_loop(ws_fs_clone, conn_id_clone, write, message_rx, poll_interval).await;
        });

        // Spawn ping task for keepalive
        let message_tx_clone = message_tx.clone();
        let ping_interval = self.config.ws_ping_interval;
        let ping_task = tokio::spawn(async move {
            Self::ping_loop(message_tx_clone, ping_interval).await;
        });

        Ok(ConnectionHandle {
            conn_id: conn_id.to_string(),
            inbox_task,
            outbox_task,
            ping_task,
            message_tx,
        })
    }

    /// Loop that reads from socket and writes to WsFs inbox.
    async fn inbox_loop(
        ws_fs: Arc<WsFs>,
        conn_id: String,
        mut read: futures_util::stream::SplitStream<
            WebSocketStream<MaybeTlsStream<TcpStream>>,
        >,
    ) {
        while let Some(result) = read.next().await {
            match result {
                Ok(msg) => match msg {
                    Message::Binary(data) => {
                        let _ = ws_fs.receive_message(&conn_id, data.to_vec());
                    }
                    Message::Text(text) => {
                        let _ = ws_fs.receive_message(&conn_id, text.as_bytes().to_vec());
                    }
                    Message::Ping(_) | Message::Pong(_) => {
                        // Handled by tungstenite automatically
                    }
                    Message::Close(_) => {
                        tracing::debug!("WebSocket {} received close frame", conn_id);
                        let _ = ws_fs.set_closed(&conn_id);
                        break;
                    }
                    Message::Frame(_) => {
                        // Raw frame, ignore
                    }
                },
                Err(e) => {
                    tracing::error!("WebSocket {} read error: {}", conn_id, e);
                    let _ = ws_fs.set_error(&conn_id, e.to_string());
                    break;
                }
            }
        }
    }

    /// Loop that reads from WsFs outbox and writes to socket.
    async fn outbox_loop(
        ws_fs: Arc<WsFs>,
        conn_id: String,
        mut write: futures_util::stream::SplitSink<
            WebSocketStream<MaybeTlsStream<TcpStream>>,
            Message,
        >,
        mut message_rx: tokio::sync::mpsc::Receiver<Message>,
        poll_interval: Duration,
    ) {
        let mut poll_timer = interval(poll_interval);

        loop {
            tokio::select! {
                // Handle messages from internal channel (ping, close)
                Some(msg) = message_rx.recv() => {
                    if matches!(msg, Message::Close(_)) {
                        let _ = write.send(msg).await;
                        break;
                    }
                    if let Err(e) = write.send(msg).await {
                        tracing::error!("WebSocket {} write error: {}", conn_id, e);
                        let _ = ws_fs.set_error(&conn_id, e.to_string());
                        break;
                    }
                }
                // Poll WsFs outbox
                _ = poll_timer.tick() => {
                    match ws_fs.drain_outbox(&conn_id) {
                        Ok(messages) => {
                            for data in messages {
                                let msg = Message::Binary(data.into());
                                if let Err(e) = write.send(msg).await {
                                    tracing::error!("WebSocket {} write error: {}", conn_id, e);
                                    let _ = ws_fs.set_error(&conn_id, e.to_string());
                                    return;
                                }
                            }
                        }
                        Err(_) => {
                            // Connection might have been removed
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Loop that sends ping frames for keepalive.
    async fn ping_loop(message_tx: tokio::sync::mpsc::Sender<Message>, ping_interval: Duration) {
        let mut timer = interval(ping_interval);

        loop {
            timer.tick().await;
            if message_tx.send(Message::Ping(vec![].into())).await.is_err() {
                break;
            }
        }
    }

    /// Shutdown all active connections.
    async fn shutdown_all(&mut self) {
        for (conn_id, handle) in self.connections.drain() {
            tracing::debug!("Shutting down WebSocket connection {}", conn_id);
            let _ = handle.message_tx.send(Message::Close(None)).await;
            handle.inbox_task.abort();
            handle.outbox_task.abort();
            handle.ping_task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ws_connector_creation() {
        let ws_fs = Arc::new(WsFs::new());
        let config = ExecutorConfig::default();
        let (tx, rx) = broadcast::channel(1);

        let connector = WsConnector::new(ws_fs, config, rx);
        assert!(connector.connections.is_empty());

        drop(tx);
    }
}
