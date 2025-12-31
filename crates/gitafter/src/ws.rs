//! Broadcast support for real-time Nostr event streaming.

use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_ws::AggregatedMessage;
use futures_util::StreamExt;
use tokio::sync::broadcast;
use tracing::warn;

/// Broadcasts messages to all subscribers.
pub struct WsBroadcaster {
    tx: broadcast::Sender<String>,
}

impl WsBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Broadcast a message to all connected listeners.
    #[allow(dead_code)]
    pub fn broadcast(&self, msg: &str) -> usize {
        match self.tx.send(msg.to_string()) {
            Ok(receiver_count) => receiver_count,
            Err(e) => {
                // No active receivers isn't an error.
                warn!("Broadcast send failed (no receivers): {}", e);
                0
            }
        }
    }

    /// Subscribe to broadcasts.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    broadcaster: std::sync::Arc<WsBroadcaster>,
) -> Result<HttpResponse, Error> {
    let (res, mut session, stream) = actix_ws::handle(&req, stream)?;

    let mut rx = broadcaster.subscribe();

    actix_web::rt::spawn(async move {
        let mut stream = stream
            .aggregate_continuations()
            .max_continuation_size(64 * 1024);

        loop {
            tokio::select! {
                msg = stream.next() => {
                    match msg {
                        Some(Ok(AggregatedMessage::Ping(data))) => {
                            if session.pong(&data).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(AggregatedMessage::Close(_))) | None => break,
                        _ => {}
                    }
                }
                result = rx.recv() => {
                    match result {
                        Ok(html) => {
                            if session.text(html).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        let _ = session.close(None).await;
    });

    Ok(res)
}
