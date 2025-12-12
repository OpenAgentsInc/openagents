//! Relay Durable Object implementation.
//!
//! This is the Cloudflare-specific wrapper around `nostr-relay` that provides:
//! - WebSocket handling via Cloudflare's WebSocket API
//! - Event storage (in-memory for now, SQLite later)
//! - HTTP endpoints for relay info (NIP-11)

use nostr_relay::{ClientMessage, Filter, RelayMessage, SubscriptionManager};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use worker::*;

/// Relay Durable Object.
///
/// Manages WebSocket connections, event storage, and subscriptions.
#[durable_object]
pub struct RelayDurableObject {
    #[allow(dead_code)]
    state: State,
    #[allow(dead_code)]
    env: Env,
    /// Subscriptions per WebSocket connection (by connection ID)
    subscriptions: RefCell<HashMap<String, SubscriptionManager>>,
    /// In-memory event storage (temporary - will migrate to SQLite)
    events: RefCell<Vec<nostr::Event>>,
}

impl DurableObject for RelayDurableObject {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            subscriptions: RefCell::new(HashMap::new()),
            events: RefCell::new(Vec::new()),
        }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        // Check for WebSocket upgrade
        let upgrade = req.headers().get("Upgrade")?;
        if upgrade.as_deref() == Some("websocket") {
            return self.handle_websocket_upgrade().await;
        }

        // HTTP endpoints
        let path = req.path();
        match path.as_str() {
            "/" => self.handle_relay_info(),
            "/health" => Response::ok("OK"),
            _ => Response::error("Not Found", 404),
        }
    }
}

impl RelayDurableObject {
    /// Handle WebSocket upgrade.
    async fn handle_websocket_upgrade(&self) -> Result<Response> {
        // Create WebSocket pair
        let pair = WebSocketPair::new()?;
        let server = pair.server;
        let client = pair.client;

        // Accept the connection
        server.accept()?;

        // Generate connection ID
        let conn_id = format!("conn_{}", Date::now().as_millis());

        // Initialize subscription manager for this connection
        self.subscriptions
            .borrow_mut()
            .insert(conn_id.clone(), SubscriptionManager::new());

        // Return the WebSocket response
        Response::from_websocket(client)
    }

    /// Handle NIP-11 relay information document.
    fn handle_relay_info(&self) -> Result<Response> {
        let info = RelayInfo {
            name: "OpenAgents Relay".to_string(),
            description: "Nostr relay for OpenAgents swarm compute network".to_string(),
            pubkey: None,
            contact: None,
            supported_nips: vec![1, 9, 11, 40, 90],
            software: "openagents-cloudflare".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        headers.set("Access-Control-Allow-Origin", "*")?;

        Ok(Response::from_json(&info)?.with_headers(headers))
    }

    /// Store an event in memory.
    fn store_event(&self, event: nostr::Event) -> bool {
        let mut events = self.events.borrow_mut();
        // Check for duplicate
        if events.iter().any(|e| e.id == event.id) {
            return false;
        }
        events.push(event);
        true
    }

    /// Query events from memory.
    fn query_events(&self, filter: &Filter) -> Vec<nostr::Event> {
        let events = self.events.borrow();
        events
            .iter()
            .filter(|event| filter.matches(event))
            .take(filter.limit.unwrap_or(100) as usize)
            .cloned()
            .collect()
    }

    /// Process a client message.
    pub fn process_message(
        &self,
        conn_id: &str,
        message: &str,
    ) -> Result<Vec<RelayMessage>> {
        let client_msg = ClientMessage::from_json(message)
            .map_err(|e| Error::RustError(format!("parse error: {}", e)))?;

        let mut responses = Vec::new();

        match client_msg {
            ClientMessage::Event(event) => {
                // Verify event (at least the ID)
                if let Err(e) = nostr_relay::verify_event_id(&event) {
                    responses.push(RelayMessage::ok_failure(&event.id, &e.to_string()));
                    return Ok(responses);
                }

                // Store event
                let stored = self.store_event(event.clone());
                if stored {
                    responses.push(RelayMessage::ok_success(&event.id));
                } else {
                    responses.push(RelayMessage::ok_duplicate(&event.id));
                }
            }

            ClientMessage::Req {
                subscription_id,
                filters,
            } => {
                // Query stored events
                for filter in &filters {
                    for event in self.query_events(filter) {
                        responses.push(RelayMessage::event(&subscription_id, event));
                    }
                }

                // Send EOSE
                responses.push(RelayMessage::eose(&subscription_id));

                // Store subscription
                if let Some(manager) = self.subscriptions.borrow_mut().get_mut(conn_id) {
                    manager.add(&subscription_id, filters);
                }
            }

            ClientMessage::Close { subscription_id } => {
                if let Some(manager) = self.subscriptions.borrow_mut().get_mut(conn_id) {
                    manager.remove(&subscription_id);
                }
            }

            ClientMessage::Auth(_) => {
                // NIP-42 not implemented yet
                responses.push(RelayMessage::notice("AUTH not implemented"));
            }
        }

        Ok(responses)
    }
}

/// NIP-11 Relay Information Document.
#[derive(Debug, Serialize, Deserialize)]
struct RelayInfo {
    name: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contact: Option<String>,
    supported_nips: Vec<u32>,
    software: String,
    version: String,
}
