//! Nostr driver for incoming/outgoing events.

use crate::drivers::{Driver, DriverHandle, EnvelopeSink, RoutedEnvelope};
use crate::envelope::Envelope;
use crate::error::Result;
use crate::identity::{PublicKey, SigningService};
use crate::types::{AgentId, EnvelopeId, Timestamp};
use async_trait::async_trait;
use nostr::{Event, UnsignedEvent, ENCRYPTED_DM_KIND, KIND_SHORT_TEXT_NOTE, get_event_hash};
use nostr_client::{PoolConfig, RelayPool};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

/// Configuration for the Nostr driver.
#[derive(Clone, Debug)]
pub struct NostrDriverConfig {
    /// Relay URLs to connect to.
    pub relays: Vec<String>,
    /// Agent ids to subscribe for.
    pub agents: Vec<AgentId>,
    /// Subscribe to encrypted DMs.
    pub subscribe_dms: bool,
    /// Subscribe to mentions in notes.
    pub subscribe_mentions: bool,
    /// Relay pool config.
    pub pool: PoolConfig,
    /// Optional override for subscription id.
    pub subscription_id: Option<String>,
}

impl Default for NostrDriverConfig {
    fn default() -> Self {
        Self {
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
            ],
            agents: Vec::new(),
            subscribe_dms: true,
            subscribe_mentions: true,
            pool: PoolConfig::default(),
            subscription_id: None,
        }
    }
}

/// Publish request for a signed Nostr event.
#[derive(Clone, Debug)]
pub struct NostrPublishRequest {
    /// Agent id for signing.
    pub agent_id: AgentId,
    /// Nostr event kind.
    pub kind: u16,
    /// Nostr tags.
    pub tags: Vec<Vec<String>>,
    /// Nostr content.
    pub content: String,
}

/// Nostr relay driver for agent inbox events and publishing.
#[derive(Clone)]
pub struct NostrDriver {
    config: NostrDriverConfig,
    signer: Arc<dyn SigningService>,
    state: Arc<RwLock<Option<NostrDriverState>>>,
}

struct NostrDriverState {
    pool: Arc<RelayPool>,
}

impl NostrDriver {
    /// Create a new Nostr driver.
    pub fn new(config: NostrDriverConfig, signer: Arc<dyn SigningService>) -> Self {
        Self {
            config,
            signer,
            state: Arc::new(RwLock::new(None)),
        }
    }

    /// Publish a signed event for the given agent.
    pub async fn publish(&self, request: NostrPublishRequest) -> Result<String> {
        let event = self.build_event(&request)?;
        self.publish_event(event).await
    }

    /// Publish a pre-built event.
    pub async fn publish_event(&self, event: Event) -> Result<String> {
        let state = self.state.read().await;
        let Some(state) = state.as_ref() else {
            return Err("nostr driver not started".into());
        };
        state
            .pool
            .publish(&event)
            .await
            .map_err(|err| err.to_string())?;
        Ok(event.id)
    }

    fn build_event(&self, request: &NostrPublishRequest) -> Result<Event> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let pubkey = self.signer.pubkey(&request.agent_id)?;
        let pubkey_hex = pubkey.to_hex();
        let unsigned = UnsignedEvent {
            pubkey: pubkey_hex.clone(),
            created_at,
            kind: request.kind,
            tags: request.tags.clone(),
            content: request.content.clone(),
        };
        let id = get_event_hash(&unsigned).map_err(|err| err.to_string())?;
        let id_bytes = hex::decode(&id).map_err(|err| err.to_string())?;
        let sig = self.signer.sign(&request.agent_id, &id_bytes)?;

        Ok(Event {
            id,
            pubkey: pubkey_hex,
            created_at,
            kind: request.kind,
            tags: request.tags.clone(),
            content: request.content.clone(),
            sig: sig.to_hex(),
        })
    }
}

#[async_trait]
impl Driver for NostrDriver {
    fn name(&self) -> &str {
        "nostr"
    }

    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle> {
        let mut guard = self.state.write().await;
        if guard.is_some() {
            return Err("nostr driver already started".into());
        }

        if self.config.relays.is_empty() {
            return Err("nostr driver requires at least one relay".into());
        }
        if self.config.agents.is_empty() {
            return Err("nostr driver requires at least one agent".into());
        }

        let pool = Arc::new(RelayPool::new(self.config.pool.clone()));
        for relay in &self.config.relays {
            pool.add_relay(relay)
                .await
                .map_err(|err| err.to_string())?;
        }
        pool.connect_all().await.map_err(|err| err.to_string())?;

        let mut pubkey_map = HashMap::new();
        for agent_id in &self.config.agents {
            let pubkey = self.signer.pubkey(agent_id)?;
            pubkey_map.insert(pubkey.to_hex(), agent_id.clone());
        }

        let pubkeys: Vec<String> = pubkey_map.keys().cloned().collect();
        let mut filters = Vec::new();
        if self.config.subscribe_dms {
            filters.push(json!({
                "kinds": [ENCRYPTED_DM_KIND],
                "#p": pubkeys,
            }));
        }
        if self.config.subscribe_mentions {
            filters.push(json!({
                "kinds": [KIND_SHORT_TEXT_NOTE],
                "#p": pubkeys,
            }));
        }
        if filters.is_empty() {
            return Err("nostr driver has no subscription filters".into());
        }

        let subscription_id = self
            .config
            .subscription_id
            .clone()
            .unwrap_or_else(|| format!("openagents-{}", Uuid::new_v4()));
        let mut rx = pool
            .subscribe(&subscription_id, &filters)
            .await
            .map_err(|err| err.to_string())?;

        let (stop_tx, mut stop_rx) = oneshot::channel();
        let signer = self.signer.clone();
        let pool_clone = pool.clone();
        let subscription_id_clone = subscription_id.clone();
        let pubkey_map_clone = pubkey_map.clone();

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    event = rx.recv() => {
                        let Some(event) = event else {
                            break;
                        };
                        let routed = route_event(&signer, &pubkey_map_clone, &event);
                        for item in routed {
                            if sink.send(item).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
            }

            let _ = pool_clone.unsubscribe(&subscription_id_clone).await;
            let _ = pool_clone.disconnect_all().await;
            Ok(())
        });

        *guard = Some(NostrDriverState { pool });

        Ok(DriverHandle {
            id: "nostr".to_string(),
            stop_tx,
            task,
        })
    }

    async fn stop(&self, handle: DriverHandle) -> Result<()> {
        let result = handle.stop().await;
        let mut guard = self.state.write().await;
        *guard = None;
        result
    }
}

pub(crate) fn route_event(
    signer: &Arc<dyn SigningService>,
    pubkey_map: &HashMap<String, AgentId>,
    event: &Event,
) -> Vec<RoutedEnvelope> {
    let is_dm = event.kind == ENCRYPTED_DM_KIND;
    let is_mention = event.kind == KIND_SHORT_TEXT_NOTE;
    if !is_dm && !is_mention {
        return Vec::new();
    }

    let targets = event_targets(event, pubkey_map);
    targets
        .into_iter()
        .map(|agent_id| build_envelope(signer, &agent_id, event, is_dm))
        .collect()
}

fn event_targets(event: &Event, pubkey_map: &HashMap<String, AgentId>) -> Vec<AgentId> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    for tag in &event.tags {
        if tag.first().map(|value| value.as_str()) == Some("p") {
            if let Some(pubkey) = tag.get(1) {
                if let Some(agent_id) = pubkey_map.get(pubkey) {
                    if seen.insert(agent_id.clone()) {
                        targets.push(agent_id.clone());
                    }
                }
            }
        }
    }
    targets
}

fn build_envelope(
    signer: &Arc<dyn SigningService>,
    agent_id: &AgentId,
    event: &Event,
    is_dm: bool,
) -> RoutedEnvelope {
    let mut content = event.content.clone();
    let mut decrypted = None;
    let raw_content = if is_dm {
        Some(event.content.clone())
    } else {
        None
    };

    if is_dm {
        if let Ok(sender_bytes) = hex::decode(&event.pubkey) {
            let sender = PublicKey::new(sender_bytes);
            if let Ok(plaintext) = signer.decrypt(agent_id, &sender, event.content.as_bytes()) {
                content = String::from_utf8_lossy(&plaintext).to_string();
                decrypted = Some(true);
            } else {
                decrypted = Some(false);
            }
        }
    }

    let payload = json!({
        "type": if is_dm { "nostr_dm" } else { "nostr_mention" },
        "nostr": {
            "event_id": event.id,
            "kind": event.kind,
            "pubkey": event.pubkey,
            "created_at": event.created_at,
            "tags": event.tags,
            "content": content,
            "raw_content": raw_content,
            "decrypted": decrypted,
        }
    });

    let envelope = Envelope {
        id: EnvelopeId::new(event.id.clone()),
        timestamp: Timestamp::from_millis(event.created_at.saturating_mul(1000)),
        payload,
    };

    RoutedEnvelope {
        agent_id: agent_id.clone(),
        envelope,
    }
}
