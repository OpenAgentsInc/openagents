use anyhow::{Result, anyhow};
use async_trait::async_trait;
use nostr::{DmRelayList, Event, KIND_DM_RELAY_LIST, RELAY_LIST_METADATA_KIND, RelayListMetadata};
use nostr_client::{PoolConfig, RelayConnection, RelayPool};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;

pub type SharedRelay = Arc<dyn RelayApi>;

#[async_trait]
pub trait RelayApi: Send + Sync {
    async fn connect(&self) -> Result<()>;
    async fn disconnect(&self) -> Result<()>;
    async fn publish_event(&self, event: &Event, timeout: Duration) -> Result<()>;
    async fn subscribe_with_channel(
        &self,
        subscription_id: &str,
        filters: &[Value],
    ) -> Result<mpsc::Receiver<Event>>;
    fn relay_url(&self) -> String;
    fn relay_urls(&self) -> Vec<String> {
        vec![self.relay_url()]
    }
    async fn ingest_relay_list_event(&self, _event: &Event) -> Result<()> {
        Ok(())
    }
    async fn ingest_peer_relay_list_event(&self, _event: &Event) -> Result<()> {
        Ok(())
    }
    async fn ingest_dm_relay_list_event(&self, _event: &Event) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl RelayApi for RelayConnection {
    async fn connect(&self) -> Result<()> {
        self.connect().await?;
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.disconnect().await?;
        Ok(())
    }

    async fn publish_event(&self, event: &Event, timeout: Duration) -> Result<()> {
        self.publish_event(event, timeout).await?;
        Ok(())
    }

    async fn subscribe_with_channel(
        &self,
        subscription_id: &str,
        filters: &[Value],
    ) -> Result<mpsc::Receiver<Event>> {
        let rx = self
            .subscribe_with_channel(subscription_id, filters)
            .await?;
        Ok(rx)
    }

    fn relay_url(&self) -> String {
        self.url().to_string()
    }
}

pub struct RelayHub {
    pool: RelayPool,
    relay_urls: RwLock<Vec<String>>,
    primary_url: RwLock<String>,
    min_write_confirmations: RwLock<usize>,
    dm_relay_lists: RwLock<HashMap<String, Vec<String>>>,
    allow_relay_prune: RwLock<bool>,
}

impl RelayHub {
    pub fn new(relays: Vec<String>) -> Result<Self> {
        let mut relay_urls = Vec::new();
        let mut seen = HashSet::new();

        for relay in relays {
            let trimmed = relay.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_string()) {
                relay_urls.push(trimmed.to_string());
            }
        }

        if relay_urls.is_empty() {
            return Err(anyhow!("RelayHub requires at least one relay URL"));
        }

        let primary_url = relay_urls[0].clone();
        let default_min = PoolConfig::default().min_write_confirmations.max(1);
        let mut config = PoolConfig::default();
        config.min_write_confirmations = 1;
        let min_write_confirmations = default_min;
        let pool = RelayPool::new(config);

        Ok(Self {
            pool,
            relay_urls: RwLock::new(relay_urls),
            primary_url: RwLock::new(primary_url),
            min_write_confirmations: RwLock::new(min_write_confirmations),
            dm_relay_lists: RwLock::new(HashMap::new()),
            allow_relay_prune: RwLock::new(false),
        })
    }

    pub fn primary_url(&self) -> String {
        self.primary_url
            .read()
            .map(|url| url.clone())
            .unwrap_or_default()
    }

    pub fn relay_urls(&self) -> Vec<String> {
        self.relay_urls
            .read()
            .map(|urls| urls.clone())
            .unwrap_or_default()
    }

    pub fn min_write_confirmations(&self) -> usize {
        let target = self
            .min_write_confirmations
            .read()
            .map(|value| *value)
            .unwrap_or(1)
            .max(1);
        let relay_count = self
            .relay_urls
            .read()
            .map(|urls| urls.len().max(1))
            .unwrap_or(1);
        target.clamp(1, relay_count)
    }

    pub fn set_min_write_confirmations(&self, value: usize) -> Result<()> {
        let target = value.max(1);
        let mut min_write_confirmations = self
            .min_write_confirmations
            .write()
            .map_err(|_| anyhow!("RelayHub min confirmations lock poisoned"))?;
        *min_write_confirmations = target;
        Ok(())
    }

    pub fn allow_relay_prune(&self) -> bool {
        self.allow_relay_prune
            .read()
            .map(|value| *value)
            .unwrap_or(false)
    }

    pub fn set_allow_relay_prune(&self, value: bool) -> Result<()> {
        let mut allow_relay_prune = self
            .allow_relay_prune
            .write()
            .map_err(|_| anyhow!("RelayHub prune flag lock poisoned"))?;
        *allow_relay_prune = value;
        Ok(())
    }

    pub async fn connect_all(&self) -> Result<()> {
        let relay_urls = self
            .relay_urls
            .read()
            .map_err(|_| anyhow!("RelayHub relay list lock poisoned"))?
            .clone();
        for relay_url in relay_urls.iter() {
            self.pool.add_relay(relay_url).await?;
        }
        self.pool.connect_all().await?;
        Ok(())
    }

    pub async fn disconnect_all(&self) -> Result<()> {
        self.pool.disconnect_all().await?;
        Ok(())
    }

    pub async fn apply_relay_list_event(&self, event: &Event) -> Result<()> {
        if event.kind != RELAY_LIST_METADATA_KIND {
            return Err(anyhow!(
                "RelayHub relay list update requires kind {}",
                RELAY_LIST_METADATA_KIND
            ));
        }

        self.pool.update_relay_list(event).await?;
        let metadata = RelayListMetadata::from_event(event)
            .map_err(|e| anyhow!("Invalid relay list metadata: {}", e))?;
        self.apply_relay_list(metadata.all_relays()).await?;
        Ok(())
    }

    pub async fn apply_peer_relay_list_event(&self, event: &Event) -> Result<()> {
        if event.kind != RELAY_LIST_METADATA_KIND {
            return Err(anyhow!(
                "RelayHub relay list update requires kind {}",
                RELAY_LIST_METADATA_KIND
            ));
        }

        self.pool.update_relay_list(event).await?;
        let metadata = RelayListMetadata::from_event(event)
            .map_err(|e| anyhow!("Invalid relay list metadata: {}", e))?;
        self.ensure_relays_connected(metadata.all_relays()).await?;
        Ok(())
    }

    pub async fn apply_relay_list(&self, relays: Vec<String>) -> Result<()> {
        let mut next = Vec::new();
        let mut seen = HashSet::new();

        for relay in relays {
            let trimmed = relay.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_string()) {
                next.push(trimmed.to_string());
            }
        }

        if next.is_empty() {
            return Err(anyhow!(
                "RelayHub relay list update requires at least one relay URL"
            ));
        }

        let allow_prune = self.allow_relay_prune();

        if !allow_prune {
            let current = self
                .relay_urls
                .read()
                .map_err(|_| anyhow!("RelayHub relay list lock poisoned"))?
                .clone();
            for relay in current.iter() {
                if !seen.contains(relay) {
                    seen.insert(relay.clone());
                    next.push(relay.clone());
                }
            }
        }

        self.ensure_relays_connected(next.clone()).await?;

        {
            let mut relay_urls = self
                .relay_urls
                .write()
                .map_err(|_| anyhow!("RelayHub relay list lock poisoned"))?;
            *relay_urls = next.clone();
        }

        if let Some(primary) = next.first() {
            let mut primary_url = self
                .primary_url
                .write()
                .map_err(|_| anyhow!("RelayHub primary URL lock poisoned"))?;
            *primary_url = primary.clone();
        }

        Ok(())
    }

    pub async fn apply_dm_relay_list_event(&self, event: &Event) -> Result<()> {
        if event.kind != KIND_DM_RELAY_LIST {
            return Err(anyhow!(
                "RelayHub DM relay list update requires kind {}",
                KIND_DM_RELAY_LIST
            ));
        }

        let dm_list = DmRelayList::from_event(event)
            .map_err(|e| anyhow!("Invalid DM relay list metadata: {}", e))?;
        let relays = dedup_relays(dm_list.relays);

        {
            let mut dm_lists = self
                .dm_relay_lists
                .write()
                .map_err(|_| anyhow!("RelayHub DM relay list lock poisoned"))?;
            dm_lists.insert(event.pubkey.clone(), relays.clone());
        }

        self.ensure_relays_connected(relays).await?;
        Ok(())
    }

    async fn ensure_relays_connected(&self, relays: Vec<String>) -> Result<()> {
        let mut to_add = Vec::new();
        let existing = self
            .relay_urls
            .read()
            .map_err(|_| anyhow!("RelayHub relay list lock poisoned"))?
            .clone();
        let existing_set: HashSet<String> = existing.iter().cloned().collect();

        for relay in relays {
            if !existing_set.contains(&relay) {
                to_add.push(relay);
            }
        }

        if to_add.is_empty() {
            return Ok(());
        }

        for relay_url in &to_add {
            self.pool.add_relay(relay_url).await?;
        }
        self.pool.connect_all().await?;

        Ok(())
    }

    fn dm_relays_for_pubkey(&self, pubkey: &str) -> Vec<String> {
        self.dm_relay_lists
            .read()
            .map(|lists| lists.get(pubkey).cloned().unwrap_or_default())
            .unwrap_or_default()
    }
}

#[async_trait]
impl RelayApi for RelayHub {
    async fn connect(&self) -> Result<()> {
        self.connect_all().await
    }

    async fn disconnect(&self) -> Result<()> {
        self.disconnect_all().await
    }

    async fn publish_event(&self, event: &Event, _timeout: Duration) -> Result<()> {
        let confirmations = if event.kind == 4 {
            let mut relays = self.relay_urls();
            for tag in &event.tags {
                if tag.len() >= 2 && tag[0] == "p" {
                    relays.extend(self.dm_relays_for_pubkey(&tag[1]));
                }
            }
            relays = dedup_relays(relays);
            self.pool.publish_to_relays(event, &relays).await?
        } else {
            self.pool.publish(event).await?
        };
        let required = self.min_write_confirmations();
        if confirmations.len() < required {
            return Err(anyhow!(
                "Publish confirmations {} below required {}",
                confirmations.len(),
                required
            ));
        }
        Ok(())
    }

    async fn subscribe_with_channel(
        &self,
        subscription_id: &str,
        filters: &[Value],
    ) -> Result<mpsc::Receiver<Event>> {
        let rx = self.pool.subscribe(subscription_id, filters).await?;
        Ok(rx)
    }

    fn relay_url(&self) -> String {
        self.primary_url
            .read()
            .map(|url| url.clone())
            .unwrap_or_default()
    }

    fn relay_urls(&self) -> Vec<String> {
        self.relay_urls
            .read()
            .map(|urls| urls.clone())
            .unwrap_or_default()
    }

    async fn ingest_relay_list_event(&self, event: &Event) -> Result<()> {
        self.apply_relay_list_event(event).await
    }

    async fn ingest_peer_relay_list_event(&self, event: &Event) -> Result<()> {
        self.apply_peer_relay_list_event(event).await
    }

    async fn ingest_dm_relay_list_event(&self, event: &Event) -> Result<()> {
        self.apply_dm_relay_list_event(event).await
    }
}

fn dedup_relays(relays: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for relay in relays {
        let trimmed = relay.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            deduped.push(trimmed.to_string());
        }
    }

    deduped
}
