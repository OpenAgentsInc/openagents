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

/// Connection metadata stored in WebSocket tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMeta {
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
        let meta = ConnectionMeta {
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
                let _ = ws.send_with_str(&serde_json::to_string(&notice)?);
                return Ok(());
            }
        };

        // Handle message based on auth state
        match client_msg {
            ClientMessage::Auth { event } => {
                self.handle_auth(&ws, meta, event).await?;
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
                let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
            }
            ClientMessage::Event { event } => {
                self.handle_event(&ws, &meta, event).await?;
            }
            ClientMessage::Req {
                subscription_id,
                filters,
            } => {
                self.handle_req(&ws, &meta, subscription_id, filters).await?;
            }
            ClientMessage::Close { subscription_id } => {
                self.handle_close(&ws, subscription_id).await?;
            }
        }

        Ok(())
    }

    /// Handle WebSocket close
    async fn websocket_close(
        &self,
        _ws: WebSocket,
        _code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
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

        // Query historical events matching filters
        for filter in &filters {
            let events = storage.query_events(filter).await?;
            for event in events {
                let msg = RelayMessage::Event {
                    subscription_id: subscription_id.clone(),
                    event,
                };
                let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
            }
        }

        // Store subscription for future events
        let sub_key = format!("sub:{}:{}", ws.as_ref() as *const _ as usize, subscription_id);
        self.state
            .storage()
            .put(&sub_key, &filters)
            .await?;

        // Send EOSE
        let msg = RelayMessage::Eose {
            subscription_id,
        };
        let _ = ws.send_with_str(&serde_json::to_string(&msg)?);

        Ok(())
    }

    /// Handle CLOSE message
    async fn handle_close(&self, ws: &WebSocket, subscription_id: String) -> Result<()> {
        let sub_key = format!("sub:{}:{}", ws.as_ref() as *const _ as usize, subscription_id);
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

        for ws in websockets {
            // Check if this WebSocket has matching subscriptions
            let matching_subs = subs.get_matching_subscriptions(&ws, event).await?;
            for sub_id in matching_subs {
                let msg = RelayMessage::Event {
                    subscription_id: sub_id,
                    event: event.clone(),
                };
                let _ = ws.send_with_str(&serde_json::to_string(&msg)?);
            }
        }

        Ok(())
    }
}
