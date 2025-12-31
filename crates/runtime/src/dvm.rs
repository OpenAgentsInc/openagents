//! Shared helpers for NIP-90 DVM providers.

use async_trait::async_trait;
use nostr::{Event, JobStatus, KIND_JOB_FEEDBACK};
use nostr_client::{PoolConfig, RelayPool};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Parsed DVM feedback status.
#[derive(Debug, Clone)]
pub(crate) enum DvmFeedbackStatus {
    Quote,
    Job(JobStatus),
    Unknown(String),
}

/// Parsed feedback event.
#[derive(Debug, Clone)]
pub(crate) struct DvmFeedback {
    pub status: DvmFeedbackStatus,
    pub status_extra: Option<String>,
    pub request_id: String,
    pub amount_msats: Option<u64>,
    pub bolt11: Option<String>,
    pub content: String,
    pub provider_pubkey: String,
    pub event_id: String,
}

pub(crate) fn parse_feedback_event(event: &Event) -> Option<DvmFeedback> {
    if event.kind != KIND_JOB_FEEDBACK {
        return None;
    }

    let mut status = None;
    let mut status_extra = None;
    let mut request_id = None;
    let mut amount_msats = None;
    let mut bolt11 = None;

    for tag in &event.tags {
        if tag.is_empty() {
            continue;
        }
        match tag[0].as_str() {
            "status" if tag.len() >= 2 => {
                status = Some(match tag[1].as_str() {
                    "quote" => DvmFeedbackStatus::Quote,
                    other => match JobStatus::from_str(other) {
                        Ok(status) => DvmFeedbackStatus::Job(status),
                        Err(_) => DvmFeedbackStatus::Unknown(other.to_string()),
                    },
                });
                if tag.len() >= 3 {
                    status_extra = Some(tag[2].clone());
                }
            }
            "e" if tag.len() >= 2 => {
                request_id = Some(tag[1].clone());
            }
            "amount" if tag.len() >= 2 => {
                amount_msats = tag[1].parse::<u64>().ok();
                if tag.len() >= 3 {
                    bolt11 = Some(tag[2].clone());
                }
            }
            _ => {}
        }
    }

    let status = status?;
    let request_id = request_id?;

    Some(DvmFeedback {
        status,
        status_extra,
        request_id,
        amount_msats,
        bolt11,
        content: event.content.clone(),
        provider_pubkey: event.pubkey.clone(),
        event_id: event.id.clone(),
    })
}

pub(crate) fn msats_to_sats(msats: u64) -> u64 {
    (msats + 999) / 1000
}

#[async_trait]
pub(crate) trait DvmTransport: Send + Sync {
    async fn connect(&self) -> Result<(), String>;
    async fn publish(&self, event: Event) -> Result<(), String>;
    async fn subscribe(
        &self,
        subscription_id: &str,
        filters: &[Value],
    ) -> Result<mpsc::Receiver<Event>, String>;
    async fn unsubscribe(&self, subscription_id: &str) -> Result<(), String>;
    async fn query(&self, filters: &[Value], timeout: Duration) -> Result<Vec<Event>, String>;
    fn relays(&self) -> Vec<String>;
}

pub(crate) struct RelayPoolTransport {
    relays: Vec<String>,
    pool: Arc<RelayPool>,
    connected: AtomicBool,
}

impl RelayPoolTransport {
    pub(crate) fn new(relays: Vec<String>) -> Self {
        Self {
            relays,
            pool: Arc::new(RelayPool::new(PoolConfig::default())),
            connected: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl DvmTransport for RelayPoolTransport {
    async fn connect(&self) -> Result<(), String> {
        if self.connected.load(Ordering::SeqCst) {
            return Ok(());
        }

        for relay in &self.relays {
            self.pool
                .add_relay(relay)
                .await
                .map_err(|err| err.to_string())?;
        }
        self.pool
            .connect_all()
            .await
            .map_err(|err| err.to_string())?;
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn publish(&self, event: Event) -> Result<(), String> {
        self.pool
            .publish(&event)
            .await
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    async fn subscribe(
        &self,
        subscription_id: &str,
        filters: &[Value],
    ) -> Result<mpsc::Receiver<Event>, String> {
        self.pool
            .subscribe(subscription_id, filters)
            .await
            .map_err(|err| err.to_string())
    }

    async fn unsubscribe(&self, subscription_id: &str) -> Result<(), String> {
        self.pool
            .unsubscribe(subscription_id)
            .await
            .map_err(|err| err.to_string())
    }

    async fn query(&self, filters: &[Value], timeout: Duration) -> Result<Vec<Event>, String> {
        let subscription_id = format!("dvm-query-{}", uuid::Uuid::new_v4());
        let mut rx = self.subscribe(&subscription_id, filters).await?;
        let start = Instant::now();
        let mut events = Vec::new();

        while start.elapsed() < timeout {
            match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(_) => {}
            }
        }

        let _ = self.unsubscribe(&subscription_id).await;
        Ok(events)
    }

    fn relays(&self) -> Vec<String> {
        self.relays.clone()
    }
}
