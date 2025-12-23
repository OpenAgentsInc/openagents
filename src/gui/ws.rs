//! WebSocket broadcaster for real-time updates

use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::AggregatedMessage;
use futures_util::StreamExt;
use tokio::sync::broadcast;

/// Broadcaster for global WebSocket updates
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
///
/// SECURITY: Requires authentication via query parameter or Authorization header.
/// Token can be provided in two ways:
/// - Query parameter: ws://localhost:port/ws?token=<token>
/// - Authorization header: Authorization: Bearer <token>
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<super::state::AppState>,
    auth_token: web::Data<auth::AuthToken>,
) -> Result<HttpResponse, Error> {
    // Check for token in query parameters
    let query_token = req.query_string()
        .split('&')
        .find_map(|param| {
            let mut parts = param.splitn(2, '=');
            if parts.next() == Some("token") {
                parts.next()
            } else {
                None
            }
        });

    // Check for token in Authorization header
    let header_token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    // Validate token from either source
    let provided_token = query_token.or(header_token);
    match provided_token {
        Some(token) if auth_token.validate(token) => {
            // Authentication successful, proceed with WebSocket upgrade
        }
        _ => {
            // Authentication failed
            return Err(actix_web::error::ErrorUnauthorized(
                "Invalid or missing authentication token"
            ));
        }
    }

    let (res, mut session, stream) = actix_ws::handle(&req, stream)?;

    let mut rx = state.broadcaster.subscribe();

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
