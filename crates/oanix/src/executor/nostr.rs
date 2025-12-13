//! Nostr relay connector that bridges NostrFs to actual relays.

use crate::executor::ExecutorConfig;
use crate::services::{NostrFs, WsFs, WsState};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::sleep;

/// Nostr relay connector that manages relay connections for NostrFs.
///
/// The connector connects to relays listed in NostrFs, sends outbox events
/// via NIP-01 EVENT messages, and routes received events to the inbox.
pub struct NostrRelayConnector {
    /// The NostrFs to connect to relays for
    nostr_fs: Arc<NostrFs>,
    /// Optional WsFs for managing WebSocket connections
    /// If None, the connector manages its own connections
    ws_fs: Option<Arc<WsFs>>,
    /// Configuration
    config: ExecutorConfig,
    /// Shutdown signal receiver
    shutdown_rx: broadcast::Receiver<()>,
    /// Map of relay URL to connection ID (in WsFs)
    relay_connections: HashMap<String, String>,
    /// Subscriptions: sub_id -> (relay_conn_id, filters_json)
    subscriptions: HashMap<String, (String, String)>,
}

impl NostrRelayConnector {
    /// Create a new Nostr relay connector.
    pub fn new(
        nostr_fs: Arc<NostrFs>,
        ws_fs: Option<Arc<WsFs>>,
        config: ExecutorConfig,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            nostr_fs,
            ws_fs,
            config,
            shutdown_rx,
            relay_connections: HashMap::new(),
            subscriptions: HashMap::new(),
        }
    }

    /// Run the connector loop.
    pub async fn run(mut self) {
        tracing::info!("NostrRelayConnector started");

        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    tracing::info!("NostrRelayConnector shutting down");
                    break;
                }
                _ = sleep(self.config.poll_interval) => {
                    self.process_relays().await;
                }
            }
        }

        tracing::info!("NostrRelayConnector stopped");
    }

    /// Process relay connections and messages.
    async fn process_relays(&mut self) {
        // Ensure we're connected to all configured relays
        self.ensure_relay_connections().await;

        // Send pending outbox events
        self.flush_outbox().await;

        // Process incoming messages from relays
        self.process_inbox().await;

        // Handle subscriptions
        self.process_subscriptions().await;
    }

    /// Ensure connections to all configured relays.
    async fn ensure_relay_connections(&mut self) {
        let relays = self.nostr_fs.relays();

        // Get WsFs reference
        let ws_fs = match &self.ws_fs {
            Some(fs) => fs,
            None => {
                tracing::warn!(
                    "NostrRelayConnector has no WsFs attached, cannot connect to relays"
                );
                return;
            }
        };

        for relay_url in relays {
            // Skip if already connected
            if let Some(conn_id) = self.relay_connections.get(&relay_url) {
                // Check if still connected
                if let Some(info) = ws_fs.get_connection(conn_id) {
                    if info.state == WsState::Open {
                        continue;
                    }
                }
                // Connection is gone or not open, remove it
                self.relay_connections.remove(&relay_url);
            }

            // Check connection limit
            if ws_fs.connection_count() >= self.config.ws_max_concurrent {
                tracing::warn!(
                    "WebSocket connection limit reached, cannot connect to relay {}",
                    relay_url
                );
                continue;
            }

            // Open new connection
            match ws_fs.open_connection(&relay_url) {
                Ok(conn_id) => {
                    tracing::info!("Opened relay connection {} to {}", conn_id, relay_url);
                    self.relay_connections.insert(relay_url, conn_id);
                }
                Err(e) => {
                    tracing::error!("Failed to open relay connection to {}: {}", relay_url, e);
                }
            }
        }
    }

    /// Send pending outbox events to connected relays.
    async fn flush_outbox(&mut self) {
        let events = self.nostr_fs.outbox_events();

        if events.is_empty() {
            return;
        }

        let ws_fs = match &self.ws_fs {
            Some(fs) => fs,
            None => return,
        };

        for event in events {
            let event_id = event.id.clone();

            // Build NIP-01 EVENT message
            let event_json = match serde_json::to_string(&event) {
                Ok(json) => json,
                Err(e) => {
                    tracing::error!("Failed to serialize event {}: {}", event_id, e);
                    continue;
                }
            };

            let message = format!(r#"["EVENT",{}]"#, event_json);
            let message_bytes = message.into_bytes();

            // Send to all connected relays
            let mut sent_to_any = false;
            for (relay_url, conn_id) in &self.relay_connections {
                if let Some(info) = ws_fs.get_connection(conn_id) {
                    if info.state == WsState::Open {
                        if ws_fs.send_message(conn_id, message_bytes.clone()).is_ok() {
                            tracing::debug!("Sent event {} to relay {}", event_id, relay_url);
                            self.nostr_fs.mark_sent(&event_id, relay_url);
                            sent_to_any = true;
                        }
                    }
                }
            }

            // Remove from outbox if sent to at least one relay
            if sent_to_any {
                self.nostr_fs.remove_from_outbox(&event_id);
            }
        }
    }

    /// Process incoming messages from relays.
    async fn process_inbox(&mut self) {
        let ws_fs = match &self.ws_fs {
            Some(fs) => Arc::clone(fs),
            None => return,
        };

        // Collect relay connections to iterate
        let relay_conns: Vec<(String, String)> = self
            .relay_connections
            .iter()
            .map(|(url, id)| (url.clone(), id.clone()))
            .collect();

        for (relay_url, conn_id) in relay_conns {
            // Read all available messages from this connection
            let mut messages = Vec::new();
            while let Ok(Some(data)) = ws_fs.read_message(&conn_id) {
                messages.push(data);
            }

            // Process collected messages
            for data in messages {
                self.handle_relay_message(&relay_url, &data).await;
            }
        }
    }

    /// Handle a message received from a relay.
    async fn handle_relay_message(&mut self, relay_url: &str, data: &[u8]) {
        let message = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!("Received non-UTF8 message from relay {}", relay_url);
                return;
            }
        };

        // Parse NIP-01 message
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(message);
        let value = match parsed {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to parse relay message from {}: {}", relay_url, e);
                return;
            }
        };

        let array = match value.as_array() {
            Some(a) => a,
            None => {
                tracing::warn!("Relay message is not an array from {}", relay_url);
                return;
            }
        };

        if array.is_empty() {
            return;
        }

        let msg_type = array[0].as_str().unwrap_or_default();

        match msg_type {
            "EVENT" => {
                // ["EVENT", subscription_id, event]
                if array.len() >= 3 {
                    if let Ok(event) = serde_json::from_value(array[2].clone()) {
                        self.nostr_fs.add_to_inbox(event);
                        tracing::debug!("Received event from relay {}", relay_url);
                    }
                }
            }
            "EOSE" => {
                // ["EOSE", subscription_id]
                if array.len() >= 2 {
                    let sub_id = array[1].as_str().unwrap_or_default();
                    tracing::debug!("EOSE for subscription {} from {}", sub_id, relay_url);
                }
            }
            "OK" => {
                // ["OK", event_id, accepted, message]
                if array.len() >= 3 {
                    let event_id = array[1].as_str().unwrap_or_default();
                    let accepted = array[2].as_bool().unwrap_or(false);
                    if accepted {
                        tracing::debug!("Event {} accepted by relay {}", event_id, relay_url);
                    } else {
                        let msg = array.get(3).and_then(|v| v.as_str()).unwrap_or("unknown");
                        tracing::warn!(
                            "Event {} rejected by relay {}: {}",
                            event_id,
                            relay_url,
                            msg
                        );
                    }
                }
            }
            "NOTICE" => {
                // ["NOTICE", message]
                if array.len() >= 2 {
                    let notice = array[1].as_str().unwrap_or_default();
                    tracing::info!("Notice from relay {}: {}", relay_url, notice);
                }
            }
            "CLOSED" => {
                // ["CLOSED", subscription_id, message]
                if array.len() >= 2 {
                    let sub_id = array[1].as_str().unwrap_or_default();
                    tracing::debug!("Subscription {} closed by relay {}", sub_id, relay_url);
                    self.subscriptions.remove(sub_id);
                }
            }
            _ => {
                tracing::debug!("Unknown message type {} from relay {}", msg_type, relay_url);
            }
        }
    }

    /// Process subscription requests from NostrFs.
    async fn process_subscriptions(&mut self) {
        let subscriptions = self.nostr_fs.subscriptions();

        let ws_fs = match &self.ws_fs {
            Some(fs) => fs,
            None => return,
        };

        for (sub_id, filters) in subscriptions {
            // Skip if already subscribed
            if self.subscriptions.contains_key(&sub_id) {
                continue;
            }

            // Build NIP-01 REQ message
            let filters_json = match serde_json::to_string(&filters) {
                Ok(json) => json,
                Err(e) => {
                    tracing::error!(
                        "Failed to serialize filters for subscription {}: {}",
                        sub_id,
                        e
                    );
                    continue;
                }
            };

            // Remove the outer brackets from the array for NIP-01 format
            let filters_inner = filters_json.trim_start_matches('[').trim_end_matches(']');
            let message = format!(r#"["REQ","{}",{}]"#, sub_id, filters_inner);
            let message_bytes = message.into_bytes();

            // Send to all connected relays
            for (relay_url, conn_id) in &self.relay_connections {
                if let Some(info) = ws_fs.get_connection(conn_id) {
                    if info.state == WsState::Open {
                        if ws_fs.send_message(conn_id, message_bytes.clone()).is_ok() {
                            tracing::debug!("Sent subscription {} to relay {}", sub_id, relay_url);
                            self.subscriptions
                                .insert(sub_id.clone(), (conn_id.clone(), filters_json.clone()));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nostr_connector_creation() {
        let nostr_fs = Arc::new(NostrFs::generate().unwrap());
        let ws_fs = Arc::new(WsFs::new());
        let config = ExecutorConfig::default();
        let (tx, rx) = broadcast::channel(1);

        let connector = NostrRelayConnector::new(nostr_fs, Some(ws_fs), config, rx);
        assert!(connector.relay_connections.is_empty());
        assert!(connector.subscriptions.is_empty());

        drop(tx);
    }
}
