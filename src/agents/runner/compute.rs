//! Compute Client for Agent Reasoning
//!
//! Agent discovers providers via NIP-89 and pays for compute using its Bitcoin wallet.
//! This is the same flow as agent_customer.rs but encapsulated for agent use.

use crate::agents::{now, parse_agent_message, AgentMessage};
use anyhow::{anyhow, Result};
use compute::domain::UnifiedIdentity;
use nostr::{
    finalize_event, ChannelMessageEvent, Event, EventTemplate, HandlerInfo, KIND_CHANNEL_MESSAGE,
    KIND_HANDLER_INFO, KIND_JOB_TEXT_GENERATION,
};
use nostr_client::RelayConnection;
use openagents_spark::SparkWallet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Provider discovered via NIP-89
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub pubkey: String,
    pub name: String,
    pub channel_id: String,
    pub relay_url: String,
    pub price_msats: u64,
    pub models: Vec<String>,
}

/// Compute client for agents to buy inference
pub struct ComputeClient {
    identity: UnifiedIdentity,
    pub relay: RelayConnection,
    wallet: Arc<SparkWallet>,
}

impl ComputeClient {
    /// Create a new compute client
    pub fn new(identity: UnifiedIdentity, relay: RelayConnection, wallet: Arc<SparkWallet>) -> Self {
        Self {
            identity,
            relay,
            wallet,
        }
    }

    /// Connect to the relay
    pub async fn connect(&self) -> anyhow::Result<()> {
        self.relay.connect().await?;
        Ok(())
    }

    /// Discover compute providers via NIP-89 (kind 31990)
    pub async fn discover_providers(&self, timeout_secs: u64) -> Result<Vec<ProviderInfo>> {
        // Subscribe to handler info events
        let filters = vec![serde_json::json!({
            "kinds": [KIND_HANDLER_INFO as u64],
            "limit": 50
        })];

        let mut rx = self
            .relay
            .subscribe_with_channel("provider-discovery", &filters)
            .await?;

        // Collect events during discovery period
        let mut events: Vec<Event> = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match tokio::time::timeout(remaining.max(Duration::from_millis(100)), rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(_) => break,
            }
        }

        // Parse providers
        let mut providers = Vec::new();

        for event in events {
            let handler = match HandlerInfo::from_event(&event) {
                Ok(h) => h,
                Err(_) => continue,
            };

            // Only want compute providers
            if handler.handler_type != nostr::HandlerType::ComputeProvider {
                continue;
            }

            // Extract channel_id from custom tags
            let channel_id = handler
                .custom_tags
                .iter()
                .find(|(k, _)| k == "channel")
                .map(|(_, v)| v.clone());

            let channel_id = match channel_id {
                Some(id) => id,
                None => continue,
            };

            // Extract other custom tags
            let relay_url = handler
                .custom_tags
                .iter()
                .find(|(k, _)| k == "relay")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| self.relay.url().to_string());

            let network = handler
                .custom_tags
                .iter()
                .find(|(k, _)| k == "network")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "unknown".to_string());

            // Skip wrong network (for now only accept regtest)
            if network != "regtest" {
                continue;
            }

            let models: Vec<String> = handler
                .custom_tags
                .iter()
                .filter(|(k, _)| k == "model")
                .map(|(_, v)| v.clone())
                .collect();

            let price_msats = handler.pricing.as_ref().map(|p| p.amount).unwrap_or(0);

            providers.push(ProviderInfo {
                pubkey: handler.pubkey.clone(),
                name: handler.metadata.name.clone(),
                channel_id,
                relay_url,
                price_msats,
                models,
            });
        }

        Ok(providers)
    }

    /// Request inference from a provider and pay for it
    pub async fn request_inference(
        &self,
        provider: &ProviderInfo,
        prompt: &str,
        max_tokens: u32,
        budget_sats: u64,
    ) -> Result<String> {
        // Check budget
        let price_sats = provider.price_msats / 1000;
        if price_sats > budget_sats {
            return Err(anyhow!(
                "Provider price {} sats exceeds budget {} sats",
                price_sats,
                budget_sats
            ));
        }

        // Subscribe to provider's channel
        let mut rx = self.subscribe_to_channel(&provider.channel_id).await?;

        // Send job request
        let request = AgentMessage::JobRequest {
            kind: KIND_JOB_TEXT_GENERATION,
            prompt: prompt.to_string(),
            max_tokens,
            target_provider: Some(provider.pubkey.clone()),
        };
        self.send_channel_message(&provider.channel_id, &request)
            .await?;

        // Wait for invoice and result
        let start_time = now().saturating_sub(60); // Accept messages from last minute
        let timeout = Duration::from_secs(120);
        let job_start = std::time::Instant::now();
        let mut our_job_id: Option<String> = None;
        let mut result_text = String::new();

        loop {
            if job_start.elapsed() > timeout {
                return Err(anyhow!("Timeout waiting for compute result"));
            }

            let event = match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
                Ok(Some(e)) => e,
                Ok(None) => return Err(anyhow!("Channel closed")),
                Err(_) => continue,
            };

            // Skip old messages
            if event.created_at < start_time {
                continue;
            }

            // Skip our own messages
            if event.pubkey == self.identity.public_key_hex() {
                continue;
            }

            let msg = match parse_agent_message(&event.content) {
                Some(m) => m,
                None => continue,
            };

            match msg {
                AgentMessage::Invoice {
                    bolt11,
                    job_id,
                    amount_msats,
                    payment_hash: _,
                } => {
                    // Skip invoices for other jobs
                    if our_job_id.is_some() && our_job_id.as_ref() != Some(&job_id) {
                        continue;
                    }

                    our_job_id = Some(job_id.clone());

                    // Check amount within budget
                    let amount_sats = amount_msats / 1000;
                    if amount_sats > budget_sats {
                        return Err(anyhow!(
                            "Invoice amount {} sats exceeds budget {} sats",
                            amount_sats,
                            budget_sats
                        ));
                    }

                    // Pay the invoice
                    let payment = self.wallet.send_payment_simple(&bolt11, None).await?;
                    let payment_id = payment.payment.id.clone();

                    // Confirm payment
                    let confirm = AgentMessage::PaymentSent { job_id, payment_id };
                    self.send_channel_message(&provider.channel_id, &confirm)
                        .await?;
                }
                AgentMessage::JobResult { job_id, result } => {
                    if our_job_id.as_ref() == Some(&job_id) {
                        result_text = result;
                        break;
                    }
                }
                AgentMessage::StreamChunk {
                    job_id,
                    chunk,
                    is_final,
                } => {
                    if our_job_id.as_ref() == Some(&job_id) {
                        result_text.push_str(&chunk);
                        if is_final {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(result_text)
    }

    /// Subscribe to a NIP-28 channel
    async fn subscribe_to_channel(&self, channel_id: &str) -> Result<mpsc::Receiver<Event>> {
        let filters = vec![serde_json::json!({
            "kinds": [KIND_CHANNEL_MESSAGE as u64],
            "#e": [channel_id]
        })];

        let rx = self
            .relay
            .subscribe_with_channel("agent-channel", &filters)
            .await?;
        Ok(rx)
    }

    /// Send a message to a NIP-28 channel
    async fn send_channel_message(&self, channel_id: &str, msg: &AgentMessage) -> Result<()> {
        let msg_json = serde_json::to_string(msg)?;
        let relay_url = self.relay.url().to_string();

        let channel_msg = ChannelMessageEvent::new(channel_id, &relay_url, &msg_json, now());

        let template = EventTemplate {
            created_at: now(),
            kind: KIND_CHANNEL_MESSAGE,
            tags: channel_msg.to_tags(),
            content: msg_json,
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())?;
        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await?;

        Ok(())
    }

    /// Get the cheapest available provider
    pub fn select_cheapest_provider<'a>(
        providers: &'a [ProviderInfo],
        budget_sats: u64,
    ) -> Option<&'a ProviderInfo> {
        providers
            .iter()
            .filter(|p| p.price_msats / 1000 <= budget_sats)
            .min_by_key(|p| p.price_msats)
    }
}
