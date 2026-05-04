//! Relay pool for multi-relay fanout.

use crate::error::{ClientError, Result};
use crate::relay::{PublishConfirmation, RelayConfig, RelayConnection};
use crate::subscription::Subscription;
use nostr::Event;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// Relay pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of relays in the pool.
    pub max_relays: usize,
    /// Relay configuration template.
    pub relay_config: RelayConfig,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_relays: 16,
            relay_config: RelayConfig::default(),
        }
    }
}

/// Minimal multi-relay pool.
pub struct RelayPool {
    relays: Arc<RwLock<HashMap<String, Arc<RelayConnection>>>>,
    config: PoolConfig,
}

impl RelayPool {
    /// Create new relay pool.
    pub fn new(config: PoolConfig) -> Self {
        Self {
            relays: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Add relay URL to pool (not connected until `connect_all` or `connect_relay`).
    pub async fn add_relay(&self, url: &str) -> Result<()> {
        let mut relays = self.relays.write().await;
        if relays.contains_key(url) {
            return Ok(());
        }
        if relays.len() >= self.config.max_relays {
            return Err(ClientError::Internal(format!(
                "maximum relay limit ({}) reached",
                self.config.max_relays
            )));
        }
        let relay = RelayConnection::with_config(url, self.config.relay_config.clone())?;
        relays.insert(url.to_string(), Arc::new(relay));
        Ok(())
    }

    /// Connect a specific relay.
    pub async fn connect_relay(&self, url: &str) -> Result<()> {
        let relay = self
            .relays
            .read()
            .await
            .get(url)
            .cloned()
            .ok_or_else(|| ClientError::InvalidRequest(format!("unknown relay: {}", url)))?;
        relay.connect().await
    }

    /// Connect all relays.
    pub async fn connect_all(&self) -> Result<()> {
        let relays: Vec<Arc<RelayConnection>> =
            self.relays.read().await.values().cloned().collect();
        let mut successful = 0usize;
        for relay in relays {
            match relay.connect().await {
                Ok(()) => successful += 1,
                Err(error) => debug!("relay connect failed: {}", error),
            }
        }
        if successful == 0 {
            return Err(ClientError::Connection(
                "failed to connect to any relay".to_string(),
            ));
        }
        Ok(())
    }

    /// Disconnect all relays.
    pub async fn disconnect_all(&self) -> Result<()> {
        let relays: Vec<Arc<RelayConnection>> =
            self.relays.read().await.values().cloned().collect();
        for relay in relays {
            relay.disconnect().await?;
        }
        Ok(())
    }

    /// Publish event to all connected relays.
    pub async fn publish(&self, event: &Event) -> Result<Vec<PublishConfirmation>> {
        let relays: Vec<Arc<RelayConnection>> =
            self.relays.read().await.values().cloned().collect();
        if relays.is_empty() {
            return Err(ClientError::NotConnected);
        }

        let mut confirmations = Vec::new();
        for relay in relays {
            match relay.publish(event).await {
                Ok(confirmation) => confirmations.push(confirmation),
                Err(error) => confirmations.push(PublishConfirmation {
                    relay_url: relay.url().to_string(),
                    event_id: event.id.clone(),
                    accepted: false,
                    message: error.to_string(),
                }),
            }
        }
        Ok(confirmations)
    }

    /// Send subscription to all connected relays.
    pub async fn subscribe(&self, subscription: Subscription) -> Result<()> {
        let relays: Vec<Arc<RelayConnection>> =
            self.relays.read().await.values().cloned().collect();
        if relays.is_empty() {
            return Err(ClientError::NotConnected);
        }
        let mut successful = 0usize;
        let mut failures = Vec::new();
        for relay in relays {
            match relay.subscribe(subscription.clone()).await {
                Ok(()) => successful += 1,
                Err(error) => failures.push(format!("{}: {}", relay.url(), error)),
            }
        }
        if successful == 0 {
            if failures.is_empty() {
                return Err(ClientError::NotConnected);
            }
            return Err(ClientError::Connection(format!(
                "failed to subscribe on any relay: {}",
                failures.join("; ")
            )));
        }
        Ok(())
    }

    /// Subscribe using raw filters.
    pub async fn subscribe_filters(
        &self,
        subscription_id: impl Into<String>,
        filters: Vec<Value>,
    ) -> Result<()> {
        self.subscribe(Subscription::new(subscription_id.into(), filters))
            .await
    }

    /// Close subscription on all relays.
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        let relays: Vec<Arc<RelayConnection>> =
            self.relays.read().await.values().cloned().collect();
        for relay in relays {
            relay.unsubscribe(subscription_id).await?;
        }
        Ok(())
    }

    /// Get relay by URL.
    pub async fn relay(&self, url: &str) -> Option<Arc<RelayConnection>> {
        self.relays.read().await.get(url).cloned()
    }

    /// Snapshot all relays currently tracked by the pool.
    pub async fn relays(&self) -> Vec<Arc<RelayConnection>> {
        self.relays.read().await.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{PoolConfig, RelayPool};
    use crate::subscription::Subscription;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    #[tokio::test(flavor = "current_thread")]
    async fn subscribe_succeeds_when_at_least_one_relay_is_connected() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind relay");
        let relay_addr = listener.local_addr().expect("relay addr");
        let live_relay_url = format!("ws://{relay_addr}");

        let missing_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind missing relay");
        let missing_relay_url = format!("ws://{}", missing_listener.local_addr().expect("addr"));
        drop(missing_listener);

        let relay_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept client");
            let mut socket = accept_async(stream).await.expect("upgrade websocket");
            while let Some(message) = socket.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                if payload.contains("\"REQ\"") {
                    socket
                        .send(Message::Text("[\"EOSE\",\"test-subscription\"]".into()))
                        .await
                        .expect("send eose");
                    return payload.to_string();
                }
            }
            panic!("relay did not receive subscription payload");
        });

        let pool = RelayPool::new(PoolConfig::default());
        pool.add_relay(live_relay_url.as_str())
            .await
            .expect("add live relay");
        pool.add_relay(missing_relay_url.as_str())
            .await
            .expect("add missing relay");
        pool.connect_all()
            .await
            .expect("connect at least one relay");
        pool.subscribe(Subscription::new(
            "test-subscription".to_string(),
            vec![json!({"kinds": [1]})],
        ))
        .await
        .expect("subscribe with one live relay");

        let payload = relay_task.await.expect("relay task");
        assert!(
            payload.contains("\"REQ\""),
            "live relay should receive the subscription request"
        );
        pool.disconnect_all().await.expect("disconnect pool");
    }
}
