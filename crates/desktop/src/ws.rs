//! Simplified WebSocket broadcaster for the desktop app

use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_ws::AggregatedMessage;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Minimal broadcaster for global WebSocket updates
pub struct WsBroadcaster {
    sender: broadcast::Sender<String>,
}

impl WsBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcast HTML fragment to all connected clients
    pub fn broadcast(&self, html: &str) {
        let _ = self.sender.send(html.to_string());
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    broadcaster: Arc<WsBroadcaster>,
) -> Result<HttpResponse, Error> {
    let (res, mut session, stream) = actix_ws::handle(&req, stream)?;

    let mut rx = broadcaster.subscribe();

    actix_web::rt::spawn(async move {
        let mut stream = stream
            .aggregate_continuations()
            .max_continuation_size(64 * 1024);

        loop {
            tokio::select! {
                // Client messages (ping/pong handling)
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
                // Server broadcasts
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
