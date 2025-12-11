//! WebSocket echo server fixture for deterministic testing

use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// WebSocket echo server for deterministic testing
///
/// Echoes back any text or binary messages received.
/// Handles ping/pong automatically.
pub struct WsEchoServer {
    addr: SocketAddr,
    shutdown_tx: broadcast::Sender<()>,
    handle: JoinHandle<()>,
    /// Records all messages received (for verification)
    received: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl WsEchoServer {
    /// Start a new echo server on a random port
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let mut shutdown_rx = shutdown_tx.subscribe();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    result = listener.accept() => {
                        if let Ok((stream, _)) = result {
                            let received = Arc::clone(&received_clone);
                            tokio::spawn(Self::handle_connection(stream, received));
                        }
                    }
                }
            }
        });

        Self {
            addr,
            shutdown_tx,
            handle,
            received,
        }
    }

    /// Handle a single WebSocket connection
    async fn handle_connection(
        stream: tokio::net::TcpStream,
        received: Arc<Mutex<Vec<Vec<u8>>>>,
    ) {
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(_) => return,
        };

        let (mut write, mut read) = ws_stream.split();

        while let Some(result) = read.next().await {
            match result {
                Ok(msg) => match msg {
                    Message::Text(text) => {
                        // Record and echo
                        received.lock().await.push(text.as_bytes().to_vec());
                        let echo = format!("echo: {}", text);
                        if write.send(Message::Text(echo.into())).await.is_err() {
                            break;
                        }
                    }
                    Message::Binary(data) => {
                        // Record and echo
                        received.lock().await.push(data.to_vec());
                        if write.send(Message::Binary(data)).await.is_err() {
                            break;
                        }
                    }
                    Message::Ping(data) => {
                        if write.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Message::Pong(_) => {
                        // Ignore pong
                    }
                    Message::Close(_) => {
                        let _ = write.send(Message::Close(None)).await;
                        break;
                    }
                    Message::Frame(_) => {
                        // Raw frame, ignore
                    }
                },
                Err(_) => break,
            }
        }
    }

    /// Get the WebSocket URL (ws://...)
    pub fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    /// Get the address the server is listening on
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get all messages received by the server
    pub async fn received_messages(&self) -> Vec<Vec<u8>> {
        self.received.lock().await.clone()
    }

    /// Clear recorded messages
    pub async fn clear_received(&self) {
        self.received.lock().await.clear();
    }

    /// Shutdown the server
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.handle.await;
    }
}

/// WebSocket server that disconnects after N messages (for reconnect testing)
pub struct WsDisconnectingServer {
    addr: SocketAddr,
    shutdown_tx: broadcast::Sender<()>,
    handle: JoinHandle<()>,
}

impl WsDisconnectingServer {
    /// Start a server that disconnects after `disconnect_after` messages
    pub async fn start(disconnect_after: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let mut shutdown_rx = shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    result = listener.accept() => {
                        if let Ok((stream, _)) = result {
                            tokio::spawn(Self::handle_connection(stream, disconnect_after));
                        }
                    }
                }
            }
        });

        Self {
            addr,
            shutdown_tx,
            handle,
        }
    }

    async fn handle_connection(stream: tokio::net::TcpStream, disconnect_after: usize) {
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(_) => return,
        };

        let (mut write, mut read) = ws_stream.split();
        let mut count = 0;

        while let Some(result) = read.next().await {
            if let Ok(msg) = result {
                match msg {
                    Message::Text(text) => {
                        count += 1;
                        if count >= disconnect_after {
                            let _ = write.send(Message::Close(None)).await;
                            break;
                        }
                        let echo = format!("echo: {}", text);
                        if write.send(Message::Text(echo.into())).await.is_err() {
                            break;
                        }
                    }
                    Message::Binary(data) => {
                        // Also count binary messages (WsConnector sends as binary)
                        count += 1;
                        if count >= disconnect_after {
                            let _ = write.send(Message::Close(None)).await;
                            break;
                        }
                        // Echo as text if it looks like text
                        if let Ok(text) = String::from_utf8(data.to_vec()) {
                            let echo = format!("echo: {}", text);
                            if write.send(Message::Text(echo.into())).await.is_err() {
                                break;
                            }
                        } else {
                            if write.send(Message::Binary(data)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Message::Close(_) => {
                        let _ = write.send(Message::Close(None)).await;
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.handle.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::connect_async;

    #[tokio::test]
    async fn test_ws_echo_server_starts() {
        let server = WsEchoServer::start().await;
        assert!(server.url().starts_with("ws://"));
        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_ws_echo_server_echoes() {
        let server = WsEchoServer::start().await;

        let (ws_stream, _) = connect_async(server.url()).await.unwrap();
        let (mut write, mut read) = ws_stream.split();

        // Send a message
        write
            .send(Message::Text("hello".into()))
            .await
            .unwrap();

        // Receive echo
        if let Some(Ok(Message::Text(text))) = read.next().await {
            assert_eq!(text.as_str(), "echo: hello");
        } else {
            panic!("Expected text message");
        }

        server.shutdown().await;
    }
}
