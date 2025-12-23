//! WebSocket support for real-time Nostr event streaming

use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::Message;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tracing::warn;

/// Broadcasts messages to all connected WebSocket clients
pub struct WsBroadcaster {
    tx: broadcast::Sender<String>,
    /// Active WebSocket task handles for graceful shutdown
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl WsBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self {
            tx,
            tasks: Mutex::new(Vec::new()),
        }
    }

    /// Broadcast a message to all connected clients
    /// Returns the number of receivers that received the message
    #[allow(dead_code)]
    pub fn broadcast(&self, msg: &str) -> usize {
        match self.tx.send(msg.to_string()) {
            Ok(receiver_count) => receiver_count,
            Err(e) => {
                // This happens when there are no active receivers
                // Not an error condition - just means no clients connected
                warn!("Broadcast send failed (no receivers): {}", e);
                0
            }
        }
    }

    /// Subscribe to broadcasts
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Store a WebSocket task handle
    async fn add_task(&self, handle: JoinHandle<()>) {
        let mut tasks = self.tasks.lock().await;
        tasks.push(handle);
    }

    /// Clean up completed tasks
    async fn cleanup_tasks(&self) {
        let mut tasks = self.tasks.lock().await;
        tasks.retain(|task| !task.is_finished());
    }

    /// Shutdown all WebSocket tasks gracefully
    #[allow(dead_code)]
    pub async fn shutdown(&self) {
        let mut tasks = self.tasks.lock().await;
        for task in tasks.drain(..) {
            task.abort();
            let _ = task.await;
        }
    }
}

/// WebSocket handler
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    broadcaster: Arc<WsBroadcaster>,
) -> Result<HttpResponse, actix_web::Error> {
    let (res, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let mut rx = broadcaster.subscribe();

    // Clean up completed tasks periodically
    broadcaster.cleanup_tasks().await;

    // Spawn task to handle this WebSocket connection and store the handle
    let handle = actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                // Receive messages from client
                Some(Ok(msg)) = msg_stream.next() => {
                    match msg {
                        Message::Ping(bytes) => {
                            let _ = session.pong(&bytes).await;
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                // Receive broadcasts
                Ok(broadcast_msg) = rx.recv() => {
                    let _ = session.text(broadcast_msg).await;
                }
                else => break,
            }
        }
    });

    broadcaster.add_task(handle).await;

    Ok(res)
}
