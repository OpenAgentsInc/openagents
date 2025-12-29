use anyhow::{anyhow, Result};
use async_trait::async_trait;
use nostr::Event;
use nostr_client::{PoolConfig, RelayConnection, RelayPool};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
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
        let rx = self.subscribe_with_channel(subscription_id, filters).await?;
        Ok(rx)
    }

    fn relay_url(&self) -> String {
        self.url().to_string()
    }
}

pub struct RelayHub {
    pool: RelayPool,
    relay_urls: Vec<String>,
    primary_url: String,
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
        let mut config = PoolConfig::default();
        config.min_write_confirmations = config.min_write_confirmations.min(relay_urls.len());
        let pool = RelayPool::new(config);

        Ok(Self {
            pool,
            relay_urls,
            primary_url,
        })
    }

    pub fn primary_url(&self) -> &str {
        &self.primary_url
    }

    pub fn relay_urls(&self) -> &[String] {
        &self.relay_urls
    }

    pub async fn connect_all(&self) -> Result<()> {
        for relay_url in &self.relay_urls {
            self.pool.add_relay(relay_url).await?;
        }
        self.pool.connect_all().await?;
        Ok(())
    }

    pub async fn disconnect_all(&self) -> Result<()> {
        self.pool.disconnect_all().await?;
        Ok(())
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
        self.pool.publish(event).await?;
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
        self.primary_url.clone()
    }

    fn relay_urls(&self) -> Vec<String> {
        self.relay_urls.clone()
    }
}
