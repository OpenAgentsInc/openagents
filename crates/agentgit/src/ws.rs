//! WebSocket support for real-time Nostr event streaming

use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::Message;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Broadcasts messages to all connected WebSocket clients
pub struct WsBroadcaster {
    tx: broadcast::Sender<String>,
}

impl WsBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Broadcast a message to all connected clients
    #[allow(dead_code)]
    pub fn broadcast(&self, msg: &str) {
        let _ = self.tx.send(msg.to_string());
    }

    /// Subscribe to broadcasts
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
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

    // Spawn task to handle this WebSocket connection
    actix_web::rt::spawn(async move {
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

    Ok(res)
}
