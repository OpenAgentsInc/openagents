//! NexusRelay Durable Object
//!
//! Handles WebSocket connections from Nostr clients with NIP-42 authentication.
//! All operations require authentication (private relay for compute marketplace).

use serde::{Deserialize, Serialize};
use worker::*;

use crate::nip01::{ClientMessage, RelayMessage};
use crate::nip42;
use crate::storage::Storage;
use crate::subscription::SubscriptionManager;

/// Configure WebSocket auto-response for ping/pong keep-alive
fn setup_websocket_auto_response(state: &State) {
    // Set up auto-response for ping/pong to prevent idle timeouts
    // This runs at the edge without waking the Durable Object
    if let Ok(pair) = WebSocketRequestResponsePair::new("ping", "pong") {
        state.set_websocket_auto_response(&pair);
    }
}

/// Connection metadata stored in WebSocket tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMeta {
    /// Unique connection ID (stable across hibernation)
    pub conn_id: String,
    /// Authenticated pubkey (None if not yet authenticated)
    pub pubkey: Option<String>,
    /// Auth challenge sent to client
    pub challenge: String,
    /// Whether client has authenticated
    pub authenticated: bool,
}

/// NexusRelay Durable Object
///
/// Single instance handles all connections for the relay.
/// Uses WebSocket hibernation API for efficient persistent connections.
#[durable_object]
pub struct NexusRelay {
    state: State,
    env: Env,
}

impl DurableObject for NexusRelay {
    fn new(state: State, env: Env) -> Self {
        // Set up WebSocket auto-response for keep-alive (runs at edge, no DO wake)
        setup_websocket_auto_response(&state);
        Self { state, env }
    }

    /// Handle incoming HTTP requests (for WebSocket upgrade)
    async fn fetch(&self, req: Request) -> Result<Response> {
        // Check for WebSocket upgrade
        let upgrade_header = req.headers().get("Upgrade")?.unwrap_or_default();
        if upgrade_header.to_lowercase() != "websocket" {
            return Response::error("Expected WebSocket upgrade", 426);
        }

        // Create WebSocket pair
        let pair = WebSocketPair::new()?;
        let server = pair.server;

        // Generate auth challenge
        let challenge = nip42::generate_challenge();

        // Store connection metadata as attachment (for hibernation)
        // Use challenge as conn_id since it's already unique per connection
        let meta = ConnectionMeta {
            conn_id: challenge.clone(),
            pubkey: None,
            challenge: challenge.clone(),
            authenticated: false,
        };

        // Accept websocket with hibernation API (don't call server.accept() separately)
        self.state.accept_web_socket(&server);

        // Serialize metadata as attachment for hibernation
        server.serialize_attachment(&meta)?;

        // Send AUTH challenge immediately (NIP-42)
        let auth_msg = RelayMessage::Auth { challenge };
        let _ = server.send_with_str(&serde_json::to_string(&auth_msg)?);

        // Return proper 101 response with client WebSocket
        Ok(ResponseBuilder::new()
            .with_status(101)
            .with_websocket(pair.client)
            .empty())
    }

    /// Handle incoming WebSocket messages (hibernation API)
    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        // CRITICAL: Wrap everything in error handling to prevent connection crashes
        // Any unhandled error in this function will close the WebSocket
        if let Err(e) = self.handle_websocket_message_inner(&ws, message).await {
            console_log!("websocket_message error (non-fatal): {:?}", e);
            // Send notice to client but don't propagate error
            if let Ok(json) = serde_json::to_string(&RelayMessage::Notice {
                message: "internal error".to_string(),
            }) {
                let _ = ws.send_with_str(&json);
            }
        }
        Ok(()) // Always return Ok to keep connection alive
    }

    /// Handle WebSocket close
    async fn websocket_close(
        &self,
        ws: WebSocket,
        code: usize,
        reason: String,
        was_clean: bool,
    ) -> Result<()> {
        // Log the close for debugging
        let meta: ConnectionMeta = ws.deserialize_attachment()?.unwrap_or_else(|| ConnectionMeta {
            conn_id: String::new(),
            pubkey: None,
            challenge: String::new(),
            authenticated: false,
        });
        console_log!(
            "websocket_close: conn_id={}, code={}, reason={}, was_clean={}",
            &meta.conn_id[..16.min(meta.conn_id.len())],
            code,
            reason,
            was_clean
        );
        // Clean up subscriptions for this connection
        // Subscriptions are tracked per-WebSocket in tags, so they're automatically cleaned up
        Ok(())
    }

    /// Handle WebSocket errors
    async fn websocket_error(&self, _ws: WebSocket, error: Error) -> Result<()> {
        console_log!("WebSocket error: {:?}", error);
        Ok(())
    }
}

impl NexusRelay {
    /// Inner message handler that can return errors
    async fn handle_websocket_message_inner(
        &self,
        ws: &WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        let msg_text = match message {
            WebSocketIncomingMessage::String(s) => s,
            WebSocketIncomingMessage::Binary(b) => {
                String::from_utf8(b).unwrap_or_else(|_| "".to_string())
            }
        };

        // Get connection metadata from attachment (hibernation API)
        let meta: ConnectionMeta = ws
            .deserialize_attachment()?
            .unwrap_or_else(|| ConnectionMeta {
                conn_id: String::new(),
                pubkey: None,
                challenge: String::new(),
                authenticated: false,
            });

        // Parse client message
        let client_msg: ClientMessage = match serde_json::from_str(&msg_text) {
            Ok(msg) => msg,
            Err(e) => {
                let notice = RelayMessage::Notice {
                    message: format!("Invalid message: {}", e),
                };
                if let Ok(json) = serde_json::to_string(&notice) {
                    let _ = ws.send_with_str(&json);
                }
                return Ok(());
            }
        };

        // Handle message based on auth state
        match client_msg {
            ClientMessage::Auth { event } => {
                self.handle_auth(ws, meta, event).await?;
            }
            _ if !meta.authenticated => {
                // Reject all non-AUTH messages if not authenticated
                let msg = match &client_msg {
                    ClientMessage::Req { subscription_id, .. } => RelayMessage::Closed {
                        subscription_id: subscription_id.clone(),
                        message: "auth-required: authentication required".to_string(),
                    },
                    ClientMessage::Event { event } => RelayMessage::Ok {
                        event_id: event.id.clone(),
                        accepted: false,
                        message: "auth-required: authentication required".to_string(),
                    },
                    ClientMessage::Close { .. } => RelayMessage::Notice {
                        message: "auth-required: authentication required".to_string(),
                    },
                    ClientMessage::Auth { .. } => unreachable!(),
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = ws.send_with_str(&json);
                }
            }
            ClientMessage::Event { event } => {
                self.handle_event(ws, &meta, event).await?;
            }
            ClientMessage::Req {
                subscription_id,
                filters,
            } => {
                self.handle_req(ws, &meta, subscription_id, filters).await?;
            }
            ClientMessage::Close { subscription_id } => {
                self.handle_close(ws, &meta, subscription_id).await?;
            }
        }

        Ok(())
    }

    /// Handle AUTH message
    async fn handle_auth(
        &self,
        ws: &WebSocket,
        mut meta: ConnectionMeta,
        event: nostr::Event,
    ) -> Result<()> {
        let relay_url = if let Ok(url) = self.env.var("RELAY_URL") {
            url.to_string()
        } else if let Ok(name) = self.env.var("RELAY_NAME") {
            let raw = name.to_string();
            if raw.starts_with("ws://") || raw.starts_with("wss://") {
                raw
            } else {
                format!("wss://{}", raw)
            }
        } else {
            "wss://nexus.openagents.com".to_string()
        };

        match nip42::validate_auth_event(&event, &meta.challenge, &relay_url) {
            Ok(pubkey) => {
                // Update connection metadata
                meta.pubkey = Some(pubkey.clone());
                meta.authenticated = true;

                // Update attachment with new metadata (hibernation API)
                ws.serialize_attachment(&meta)?;

                let msg = RelayMessage::Ok {
                    event_id: event.id.clone(),
                    accepted: true,
                    message: format!("auth accepted for {}", pubkey),
                };
                let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
            }
            Err(e) => {
                let msg = RelayMessage::Ok {
                    event_id: event.id,
                    accepted: false,
                    message: format!("auth-required: {}", e),
                };
                let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
            }
        }

        Ok(())
    }

    /// Handle EVENT message
    async fn handle_event(
        &self,
        ws: &WebSocket,
        meta: &ConnectionMeta,
        event: nostr::Event,
    ) -> Result<()> {
        if meta.pubkey.is_none() {
            return Err(Error::RustError("Not authenticated".to_string()));
        }

        // Basic validation (note: we can't verify signatures in minimal mode)
        // Just check format basics
        if event.id.len() != 64 || event.pubkey.len() != 64 || event.sig.len() != 128 {
            let msg = RelayMessage::Ok {
                event_id: event.id,
                accepted: false,
                message: "invalid: malformed event".to_string(),
            };
            let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
            return Ok(());
        }

        // Store event in D1 and DO cache
        let storage = Storage::new(&self.state, &self.env);
        match storage.store_event(&event).await {
            Ok(_) => {
                // Broadcast to matching subscriptions
                self.broadcast_event(&event).await?;

                let msg = RelayMessage::Ok {
                    event_id: event.id,
                    accepted: true,
                    message: String::new(),
                };
                let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
            }
            Err(e) => {
                let msg = RelayMessage::Ok {
                    event_id: event.id,
                    accepted: false,
                    message: format!("error: {}", e),
                };
                let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
            }
        }

        Ok(())
    }

    /// Handle REQ message
    async fn handle_req(
        &self,
        ws: &WebSocket,
        meta: &ConnectionMeta,
        subscription_id: String,
        filters: Vec<crate::subscription::Filter>,
    ) -> Result<()> {
        let storage = Storage::new(&self.state, &self.env);

        console_log!(
            "handle_req: sub_id={}, conn_id={}, {} filters",
            subscription_id,
            &meta.conn_id[..16.min(meta.conn_id.len())],
            filters.len()
        );
        for (i, f) in filters.iter().enumerate() {
            console_log!(
                "  Filter[{}]: kinds={:?}, e_tags={:?}",
                i,
                f.kinds,
                f.e_tags
            );
        }

        // CRITICAL: Store subscription FIRST before querying historical events
        // This ensures we don't miss real-time events while querying history
        let sub_key = format!("sub:{}:{}", meta.conn_id, subscription_id);
        console_log!("  Storing subscription with key: {}", sub_key);
        self.state
            .storage()
            .put(&sub_key, &filters)
            .await?;

        // Query historical events matching filters (don't fail the whole request on D1 error)
        for filter in &filters {
            match storage.query_events(filter).await {
                Ok(events) => {
                    for event in events {
                        let msg = RelayMessage::Event {
                            subscription_id: subscription_id.clone(),
                            event,
                        };
                        let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
                    }
                }
                Err(e) => {
                    console_log!("  Warning: query_events failed: {:?}", e);
                    // Continue - subscription is still stored for future events
                }
            }
        }

        // Send EOSE
        let msg = RelayMessage::Eose {
            subscription_id,
        };
        let _ = ws.send_with_str(&serde_json::to_string(&msg)?);

        Ok(())
    }

    /// Handle CLOSE message
    async fn handle_close(&self, ws: &WebSocket, meta: &ConnectionMeta, subscription_id: String) -> Result<()> {
        // Use stable conn_id (not pointer address) to find subscription
        let sub_key = format!("sub:{}:{}", meta.conn_id, subscription_id);
        self.state.storage().delete(&sub_key).await?;

        let msg = RelayMessage::Closed {
            subscription_id,
            message: "subscription closed".to_string(),
        };
        let _ = ws.send_with_str(&serde_json::to_string(&msg)?);

        Ok(())
    }

    /// Broadcast event to all matching subscriptions
    async fn broadcast_event(&self, event: &nostr::Event) -> Result<()> {
        let subs = SubscriptionManager::new(&self.state);
        let websockets = self.state.get_websockets();

        console_log!(
            "broadcast_event: kind={}, id={}..., {} websockets connected",
            event.kind,
            &event.id[..16.min(event.id.len())],
            websockets.len()
        );

        for ws in websockets {
            // Check if this WebSocket has matching subscriptions
            // CRITICAL: Don't let errors in one WebSocket crash all connections
            let matching_subs = match subs.get_matching_subscriptions(&ws, event).await {
                Ok(subs) => subs,
                Err(e) => {
                    console_log!("  Error getting subscriptions for WebSocket: {:?}", e);
                    continue; // Skip this WebSocket, try others
                }
            };
            console_log!(
                "  WebSocket has {} matching subscriptions for event {}",
                matching_subs.len(),
                &event.id[..16.min(event.id.len())]
            );
            for sub_id in matching_subs {
                console_log!("    Sending to subscription: {}", sub_id);
                let msg = RelayMessage::Event {
                    subscription_id: sub_id,
                    event: event.clone(),
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    match ws.send_with_str(&json) {
                        Ok(_) => console_log!("    -> Send succeeded for {}", &event.id[..16.min(event.id.len())]),
                        Err(e) => console_log!("    -> Send FAILED for {}: {:?}", &event.id[..16.min(event.id.len())], e),
                    }
                }
            }
        }

        Ok(())
    }
}
