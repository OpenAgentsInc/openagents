//! Relay Durable Object implementation.
//!
//! This is the Cloudflare-specific wrapper around `nostr-relay` that provides:
//! - WebSocket handling via Cloudflare's WebSocket API
//! - Event storage (in-memory for now, SQLite later)
//! - HTTP endpoints for relay info (NIP-11)
//! - NIP-90 DVM job processing

use crate::dvm::DvmProcessor;
use crate::signing::ServiceIdentity;
use nostr::{is_job_request_kind, JobRequest, JobStatus, KIND_JOB_FEEDBACK};
use nostr_relay::{ClientMessage, Filter, RelayMessage, SubscriptionManager};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use worker::*;

/// Relay Durable Object.
///
/// Manages WebSocket connections, event storage, subscriptions, and NIP-90 DVM processing.
#[durable_object]
pub struct RelayDurableObject {
    state: State,
    env: Env,
    /// Global subscription manager (simplified - all connections share subscriptions)
    subscriptions: RefCell<SubscriptionManager>,
    /// In-memory event storage (temporary - will migrate to SQLite)
    events: RefCell<Vec<nostr::Event>>,
    /// Active WebSocket connections for broadcasting
    websockets: RefCell<Vec<WebSocket>>,
    /// Service identity for signing DVM responses (lazily loaded)
    #[allow(dead_code)]
    service_identity: RefCell<Option<ServiceIdentity>>,
}

impl DurableObject for RelayDurableObject {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            subscriptions: RefCell::new(SubscriptionManager::new()),
            events: RefCell::new(Vec::new()),
            websockets: RefCell::new(Vec::new()),
            service_identity: RefCell::new(None),
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

    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        // Process the message
        if let WebSocketIncomingMessage::String(text) = message {
            // Try to parse the message to check for NIP-90 job requests
            if let Ok(ClientMessage::Event(ref event)) = ClientMessage::from_json(&text) {
                if is_job_request_kind(event.kind) {
                    // Handle NIP-90 job request asynchronously
                    match self.handle_job_request(&ws, event).await {
                        Ok(()) => return Ok(()),
                        Err(e) => {
                            let notice = RelayMessage::notice(&format!("job error: {}", e));
                            ws.send_with_str(&notice.to_json())?;
                            return Ok(());
                        }
                    }
                }
            }

            // Process non-NIP-90 messages synchronously
            match self.process_message(&text) {
                Ok(responses) => {
                    // Send responses back to the client
                    for response in responses {
                        let json = response.to_json();
                        ws.send_with_str(&json)?;
                    }
                }
                Err(e) => {
                    // Send error notice
                    let notice = RelayMessage::notice(&format!("error: {}", e));
                    ws.send_with_str(&notice.to_json())?;
                }
            }
        }

        Ok(())
    }

    async fn websocket_close(
        &self,
        _ws: WebSocket,
        _code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        // Note: With global subscription manager, we don't remove subscriptions on close
        // In production, we'd track subscription ownership and clean up properly
        Ok(())
    }
}

impl RelayDurableObject {
    /// Handle WebSocket upgrade.
    async fn handle_websocket_upgrade(&self) -> Result<Response> {
        // Create WebSocket pair
        let pair = WebSocketPair::new()?;
        let server = pair.server;
        let client = pair.client;

        // Accept the WebSocket with hibernation support
        self.state.accept_web_socket(&server);

        // Track the WebSocket for broadcasting
        self.websockets.borrow_mut().push(server);

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

    /// Store an event in memory and broadcast to matching subscribers.
    fn store_event(&self, event: nostr::Event) -> bool {
        let mut events = self.events.borrow_mut();

        // Check for duplicate
        if events.iter().any(|e| e.id == event.id) {
            return false;
        }

        // Store the event
        events.push(event.clone());

        // Broadcast to matching subscribers
        drop(events); // Release borrow before broadcasting
        self.broadcast_event(&event);

        true
    }

    /// Broadcast an event to all matching subscribers.
    fn broadcast_event(&self, event: &nostr::Event) {
        let manager = self.subscriptions.borrow();
        let websockets = self.websockets.borrow();

        // Get all subscriptions that match this event
        let matching = manager.matching(event);

        for sub in matching {
            // Send to all connected websockets
            for ws in websockets.iter() {
                let msg = RelayMessage::event(&sub.id, event.clone());
                let _ = ws.send_with_str(&msg.to_json());
            }
        }
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
    pub fn process_message(&self, message: &str) -> Result<Vec<RelayMessage>> {
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

                // Store event (also broadcasts to subscribers)
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
                self.subscriptions
                    .borrow_mut()
                    .add(&subscription_id, filters);
            }

            ClientMessage::Close { subscription_id } => {
                self.subscriptions.borrow_mut().remove(&subscription_id);
                responses.push(RelayMessage::closed(&subscription_id, ""));
            }

            ClientMessage::Auth(_) => {
                // NIP-42 not implemented yet
                responses.push(RelayMessage::notice("AUTH not implemented"));
            }
        }

        Ok(responses)
    }

    // =========================================================================
    // NIP-90 DVM Job Handling
    // =========================================================================

    /// Handle a NIP-90 job request.
    ///
    /// This processes the job using Cloudflare Workers AI and publishes
    /// feedback and result events.
    async fn handle_job_request(&self, ws: &WebSocket, event: &nostr::Event) -> Result<()> {
        // Verify event ID first
        if let Err(e) = nostr_relay::verify_event_id(event) {
            let ok = RelayMessage::ok_failure(&event.id, &e.to_string());
            ws.send_with_str(&ok.to_json())?;
            return Ok(());
        }

        // Parse the job request from the event
        let job_request = match JobRequest::from_event(event) {
            Ok(req) => req,
            Err(e) => {
                let ok = RelayMessage::ok_failure(&event.id, &format!("invalid job: {}", e));
                ws.send_with_str(&ok.to_json())?;
                return Ok(());
            }
        };

        // Store the job request event
        self.store_event(event.clone());
        let ok = RelayMessage::ok_success(&event.id);
        ws.send_with_str(&ok.to_json())?;

        // Get or initialize service identity
        let identity = self.get_or_init_service_identity()?;

        // Send "processing" feedback
        let feedback_event =
            self.create_feedback_event(&identity, event, JobStatus::Processing, None)?;
        self.store_event(feedback_event.clone());
        self.broadcast_event(&feedback_event);

        // Process the job using Cloudflare AI
        let processor = DvmProcessor::new(&self.env);
        let result = processor.process(&job_request).await;

        match result {
            Ok(content) => {
                // Create and broadcast result event
                let result_event = self.create_result_event(&identity, event, &content)?;
                self.store_event(result_event.clone());
                self.broadcast_event(&result_event);
            }
            Err(e) => {
                // Send error feedback
                let error_feedback =
                    self.create_feedback_event(&identity, event, JobStatus::Error, Some(&e))?;
                self.store_event(error_feedback.clone());
                self.broadcast_event(&error_feedback);
            }
        }

        Ok(())
    }

    /// Get or initialize the service identity from environment.
    fn get_or_init_service_identity(&self) -> Result<ServiceIdentity> {
        // Note: We recreate from env each time since Clone isn't available

        // Try to load from environment
        let privkey = self
            .env
            .secret("SERVICE_PRIVKEY")
            .map(|s| s.to_string())
            .unwrap_or_else(|_| {
                // Use a default test key if not configured
                // In production, this should always be set
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
            });

        let identity = ServiceIdentity::from_hex(&privkey)
            .map_err(|e| Error::RustError(format!("invalid service key: {}", e)))?;

        Ok(identity)
    }

    /// Create a job feedback event (kind 7000).
    fn create_feedback_event(
        &self,
        identity: &ServiceIdentity,
        request: &nostr::Event,
        status: JobStatus,
        message: Option<&str>,
    ) -> Result<nostr::Event> {
        let created_at = (Date::now().as_millis() / 1000) as u64;

        let mut tags = vec![
            vec!["e".to_string(), request.id.clone()],
            vec!["p".to_string(), request.pubkey.clone()],
            vec!["status".to_string(), status.as_str().to_string()],
        ];

        if let Some(msg) = message {
            tags[2].push(msg.to_string());
        }

        let content = message.unwrap_or("").to_string();

        // Compute event ID and sign
        let (id, sig) = identity
            .finalize_event(created_at, KIND_JOB_FEEDBACK, &tags, &content)
            .map_err(|e| Error::RustError(e))?;

        Ok(nostr::Event {
            id,
            pubkey: identity.pubkey().to_string(),
            created_at,
            kind: KIND_JOB_FEEDBACK,
            tags,
            content,
            sig,
        })
    }

    /// Create a job result event (kind 6xxx).
    fn create_result_event(
        &self,
        identity: &ServiceIdentity,
        request: &nostr::Event,
        content: &str,
    ) -> Result<nostr::Event> {
        let created_at = (Date::now().as_millis() / 1000) as u64;
        let result_kind = request.kind + 1000; // e.g., 5050 -> 6050

        let tags = vec![
            vec!["e".to_string(), request.id.clone()],
            vec!["p".to_string(), request.pubkey.clone()],
            vec![
                "request".to_string(),
                serde_json::to_string(request).unwrap_or_default(),
            ],
        ];

        // Compute event ID and sign
        let (id, sig) = identity
            .finalize_event(created_at, result_kind, &tags, content)
            .map_err(|e| Error::RustError(e))?;

        Ok(nostr::Event {
            id,
            pubkey: identity.pubkey().to_string(),
            created_at,
            kind: result_kind,
            tags,
            content: content.to_string(),
            sig,
        })
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
