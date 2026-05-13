//! Relay pool for multi-relay fanout.

use crate::error::{ClientError, Result};
use crate::relay::{PublishConfirmation, RelayConfig, RelayConnection};
use crate::subscription::Subscription;
use nostr::Event;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

/// Relay pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of relays in the pool.
    pub max_relays: usize,
    /// Relay configuration template.
    pub relay_config: RelayConfig,
    /// Maximum time to wait for one relay fanout operation.
    pub fanout_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_relays: 16,
            relay_config: RelayConfig::default(),
            fanout_timeout: Duration::from_secs(5),
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
        let results = futures_util::future::join_all(relays.into_iter().map(|relay| async move {
            let relay_url = relay.url().to_string();
            match relay.connect().await {
                Ok(()) => (relay_url, true, None),
                Err(error) => (relay_url, false, Some(error.to_string())),
            }
        }))
        .await;

        let successful = results.iter().filter(|(_, ok, _)| *ok).count();
        for (relay_url, ok, error) in results {
            if !ok {
                debug!(
                    "relay connect failed on {}: {}",
                    relay_url,
                    error.as_deref().unwrap_or("unknown error")
                );
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

        let event = Arc::new(event.clone());
        let fanout_timeout = self.config.fanout_timeout;
        Ok(
            futures_util::future::join_all(relays.into_iter().map(|relay| {
                let event = Arc::clone(&event);
                async move {
                    let relay_url = relay.url().to_string();
                    match tokio::time::timeout(fanout_timeout, relay.publish(event.as_ref())).await
                    {
                        Ok(Ok(confirmation)) => confirmation,
                        Ok(Err(error)) => PublishConfirmation {
                            relay_url,
                            event_id: event.id.clone(),
                            accepted: false,
                            message: error.to_string(),
                        },
                        Err(_) => PublishConfirmation {
                            relay_url,
                            event_id: event.id.clone(),
                            accepted: false,
                            message: format!("publish timeout after {:?}", fanout_timeout),
                        },
                    }
                }
            }))
            .await,
        )
    }

    /// Send subscription to all connected relays.
    pub async fn subscribe(&self, subscription: Subscription) -> Result<()> {
        let relays: Vec<Arc<RelayConnection>> =
            self.relays.read().await.values().cloned().collect();
        if relays.is_empty() {
            return Err(ClientError::NotConnected);
        }

        let fanout_timeout = self.config.fanout_timeout;
        let results = futures_util::future::join_all(relays.into_iter().map(|relay| {
            let subscription = subscription.clone();
            async move {
                let relay_url = relay.url().to_string();
                let subscription_id = subscription.id.clone();
                match tokio::time::timeout(fanout_timeout, relay.subscribe(subscription)).await {
                    Ok(Ok(())) => (relay_url, true, None),
                    Ok(Err(error)) => (relay_url, false, Some(error.to_string())),
                    Err(_) => {
                        relay
                            .remove_subscription_local(subscription_id.as_str())
                            .await;
                        (
                            relay_url,
                            false,
                            Some(format!("subscribe timeout after {:?}", fanout_timeout)),
                        )
                    }
                }
            }
        }))
        .await;

        let successful = results.iter().filter(|(_, ok, _)| *ok).count();
        let failures = results
            .into_iter()
            .filter_map(|(relay_url, ok, error)| {
                if ok {
                    None
                } else {
                    Some(format!(
                        "{}: {}",
                        relay_url,
                        error.unwrap_or_else(|| "unknown error".to_string())
                    ))
                }
            })
            .collect::<Vec<_>>();
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
    use crate::relay::RelayConfig;
    use crate::subscription::Subscription;
    use futures_util::{SinkExt, StreamExt};
    use nostr::{Event, EventTemplate, finalize_event};
    use serde_json::json;
    use std::time::{Duration, Instant};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    fn test_event() -> Event {
        finalize_event(
            &EventTemplate {
                created_at: 1,
                kind: 1,
                tags: Vec::new(),
                content: "relay fanout test".to_string(),
            },
            &[7u8; 32],
        )
        .expect("sign test event")
    }

    async fn live_relay_task(listener: TcpListener) -> tokio::task::JoinHandle<Vec<String>> {
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept client");
            let mut socket = accept_async(stream).await.expect("upgrade websocket");
            let mut payloads = Vec::new();
            while let Some(message) = socket.next().await {
                let Ok(Message::Text(payload)) = message else {
                    continue;
                };
                let payload = payload.to_string();
                payloads.push(payload.clone());
                if payload.contains("\"REQ\"") {
                    socket
                        .send(Message::Text("[\"EOSE\",\"test-subscription\"]".into()))
                        .await
                        .expect("send eose");
                    break;
                }
                if payload.contains("\"EVENT\"") {
                    break;
                }
            }
            payloads
        })
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn connect_all_does_not_wait_serially_for_slow_relays() {
        let slow_one = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind slow relay one");
        let slow_one_url = format!("ws://{}", slow_one.local_addr().expect("slow one addr"));
        let slow_two = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind slow relay two");
        let slow_two_url = format!("ws://{}", slow_two.local_addr().expect("slow two addr"));
        let live = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind live relay");
        let live_url = format!("ws://{}", live.local_addr().expect("live addr"));
        let live_task = live_relay_task(live).await;

        let pool = RelayPool::new(PoolConfig {
            max_relays: 3,
            relay_config: RelayConfig {
                connect_timeout: Duration::from_millis(250),
                ..RelayConfig::default()
            },
            fanout_timeout: Duration::from_millis(250),
        });
        pool.add_relay(slow_one_url.as_str())
            .await
            .expect("add slow relay one");
        pool.add_relay(slow_two_url.as_str())
            .await
            .expect("add slow relay two");
        pool.add_relay(live_url.as_str())
            .await
            .expect("add live relay");

        let started = Instant::now();
        pool.connect_all()
            .await
            .expect("connect at least one relay");
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_millis(450),
            "connect_all should be bounded by one timeout window, elapsed={elapsed:?}"
        );
        pool.disconnect_all().await.expect("disconnect pool");
        live_task.abort();
        drop(slow_one);
        drop(slow_two);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn publish_fanout_reports_partial_relay_failures() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind relay");
        let relay_addr = listener.local_addr().expect("relay addr");
        let live_relay_url = format!("ws://{relay_addr}");

        let missing_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind missing relay");
        let missing_relay_url = format!("ws://{}", missing_listener.local_addr().expect("addr"));
        drop(missing_listener);

        let relay_task = live_relay_task(listener).await;
        let pool = RelayPool::new(PoolConfig {
            max_relays: 2,
            fanout_timeout: Duration::from_millis(250),
            ..PoolConfig::default()
        });
        pool.add_relay(live_relay_url.as_str())
            .await
            .expect("add live relay");
        pool.add_relay(missing_relay_url.as_str())
            .await
            .expect("add missing relay");
        pool.connect_all()
            .await
            .expect("connect at least one relay");

        let confirmations = pool.publish(&test_event()).await.expect("publish fanout");
        assert!(
            confirmations
                .iter()
                .any(|confirmation| confirmation.accepted),
            "one relay should accept the event"
        );
        assert!(
            confirmations
                .iter()
                .any(|confirmation| !confirmation.accepted),
            "failed relay should be reported as a partial failure"
        );
        pool.disconnect_all().await.expect("disconnect pool");
        let payloads = relay_task.await.expect("relay task");
        assert!(
            payloads.iter().any(|payload| payload.contains("\"EVENT\"")),
            "live relay should receive the event payload"
        );
    }

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

        let relay_task = live_relay_task(listener).await;

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

        let payloads = relay_task.await.expect("relay task");
        assert!(
            payloads.iter().any(|payload| payload.contains("\"REQ\"")),
            "live relay should receive the subscription request"
        );
        pool.disconnect_all().await.expect("disconnect pool");
    }
}
