//! Compute Client for Agent Reasoning
//!
//! Agent discovers providers via NIP-89 and pays for compute using its Bitcoin wallet.
//!
//! # Primary Flow (Direct NIP-90 Events)
//!
//! 1. Discover providers via NIP-89 (kind:31990)
//! 2. Publish job request (kind:5050) tagging provider
//! 3. Receive job feedback (kind:7000) with invoice
//! 4. Pay Lightning invoice
//! 5. Receive job result (kind:6050)
//!
//! NIP-28 channels are optional and only used if the provider requires them.

use crate::agents::{
    now, parse_agent_message, parse_job_feedback, parse_job_result, publish_job_request,
    subscribe_job_responses, AgentMessage, JobStatus, KIND_JOB_FEEDBACK, KIND_JOB_REQUEST_TEXT,
    KIND_JOB_RESULT_TEXT,
};
use crate::agents::SharedRelay;
use anyhow::{anyhow, Result};
use compute::domain::UnifiedIdentity;
use nostr::{
    finalize_event, ChannelMessageEvent, Event, EventTemplate, HandlerInfo, KIND_CHANNEL_MESSAGE,
    KIND_HANDLER_INFO,
};
use nostr::nip_sa::AgentStateContent;
use openagents_spark::SparkWallet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Provider discovered via NIP-89
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub pubkey: String,
    pub name: String,
    /// NIP-28 channel ID (optional - only if provider uses channels)
    pub channel_id: Option<String>,
    pub relay_url: String,
    pub price_msats: u64,
    pub models: Vec<String>,
}

/// Compute client for agents to buy inference
pub struct ComputeClient {
    identity: UnifiedIdentity,
    pub relay: SharedRelay,
    wallet: Arc<SparkWallet>,
}

impl ComputeClient {
    /// Create a new compute client
    pub fn new(identity: UnifiedIdentity, relay: SharedRelay, wallet: Arc<SparkWallet>) -> Self {
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

        let subscription_id = format!("provider-discovery-{}", Uuid::new_v4());
        let mut rx = self
            .relay
            .subscribe_with_channel(&subscription_id, &filters)
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

            // Extract channel_id from custom tags (optional)
            let channel_id = handler
                .custom_tags
                .iter()
                .find(|(k, _)| k == "channel")
                .map(|(_, v)| v.clone());

            // Extract other custom tags
            let relay_url = handler
                .custom_tags
                .iter()
                .find(|(k, _)| k == "relay")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| self.relay.relay_url());

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
    ///
    /// Uses direct NIP-90 events. Falls back to NIP-28 channel if provider requires it.
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

        // If provider has a channel, use the channel-based flow (legacy)
        if let Some(ref channel_id) = provider.channel_id {
            return self
                .request_inference_via_channel(provider, channel_id, prompt, max_tokens, budget_sats)
                .await;
        }

        // Primary flow: Direct NIP-90 events
        self.request_inference_direct(provider, prompt, max_tokens, budget_sats)
            .await
    }

    /// Request inference using direct NIP-90 events (primary flow)
    async fn request_inference_direct(
        &self,
        provider: &ProviderInfo,
        prompt: &str,
        max_tokens: u32,
        budget_sats: u64,
    ) -> Result<String> {
        // Publish job request (kind:5050)
        let job_request_id = publish_job_request(
            self.relay.as_ref(),
            self.identity.keypair(),
            &provider.pubkey,
            prompt,
            max_tokens,
            KIND_JOB_REQUEST_TEXT,
        )
        .await
        .map_err(|e| anyhow!("Failed to publish job request: {}", e))?;

        tracing::debug!("Published job request: {}", job_request_id);

        // Subscribe to responses for this job
        let mut rx = subscribe_job_responses(self.relay.as_ref(), &job_request_id)
            .await
            .map_err(|e| anyhow!("Failed to subscribe to job responses: {}", e))?;

        // Wait for feedback and result
        let timeout = Duration::from_secs(120);
        let job_start = std::time::Instant::now();
        let mut paid = false;

        loop {
            if job_start.elapsed() > timeout {
                return Err(anyhow!("Timeout waiting for compute result"));
            }

            let event = match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
                Ok(Some(e)) => e,
                Ok(None) => return Err(anyhow!("Channel closed")),
                Err(_) => continue,
            };

            // Handle feedback events (kind:7000)
            if event.kind == KIND_JOB_FEEDBACK {
                if let Some((job_id, status, bolt11, amount)) = parse_job_feedback(&event) {
                    if job_id != job_request_id {
                        continue;
                    }

                    match status {
                        JobStatus::PaymentRequired => {
                            if paid {
                                continue;
                            }

                            let bolt11 = bolt11.ok_or_else(|| anyhow!("No invoice in feedback"))?;
                            let amount_msats = amount.unwrap_or(provider.price_msats);
                            let amount_sats = amount_msats / 1000;

                            if amount_sats > budget_sats {
                                return Err(anyhow!(
                                    "Invoice amount {} sats exceeds budget {} sats",
                                    amount_sats,
                                    budget_sats
                                ));
                            }

                            // Pay the invoice
                            tracing::debug!("Paying invoice: {} msats", amount_msats);
                            let _payment = self.wallet.send_payment_simple(&bolt11, None).await?;
                            paid = true;
                            tracing::debug!("Payment sent");
                        }
                        JobStatus::Processing => {
                            tracing::debug!("Job is processing...");
                        }
                        JobStatus::Success => {
                            tracing::debug!("Job completed successfully");
                        }
                        JobStatus::Error => {
                            return Err(anyhow!("Job failed with error"));
                        }
                        JobStatus::Cancelled => {
                            return Err(anyhow!("Job was cancelled"));
                        }
                    }
                }
            }

            // Handle result events (kind:6050)
            if event.kind == KIND_JOB_RESULT_TEXT {
                if let Some((job_id, result)) = parse_job_result(&event) {
                    if job_id == job_request_id {
                        return Ok(result);
                    }
                }
            }
        }
    }

    /// Request inference via NIP-28 channel (legacy flow for backwards compatibility)
    async fn request_inference_via_channel(
        &self,
        provider: &ProviderInfo,
        channel_id: &str,
        prompt: &str,
        max_tokens: u32,
        budget_sats: u64,
    ) -> Result<String> {
        // Subscribe to provider's channel
        let mut rx = self.subscribe_to_channel(channel_id).await?;

        // Send job request
        let request = AgentMessage::JobRequest {
            kind: KIND_JOB_REQUEST_TEXT,
            prompt: prompt.to_string(),
            max_tokens,
            target_provider: Some(provider.pubkey.clone()),
        };
        self.send_channel_message(channel_id, &request).await?;

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
                    self.send_channel_message(channel_id, &confirm).await?;
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

    /// Subscribe to a NIP-28 channel (optional coordination layer)
    async fn subscribe_to_channel(&self, channel_id: &str) -> Result<mpsc::Receiver<Event>> {
        let filters = vec![serde_json::json!({
            "kinds": [KIND_CHANNEL_MESSAGE as u64],
            "#e": [channel_id]
        })];

        let subscription_id = format!("agent-channel-{}-{}", channel_id, Uuid::new_v4());
        let rx = self
            .relay
            .subscribe_with_channel(&subscription_id, &filters)
            .await?;
        Ok(rx)
    }

    /// Send a message to a NIP-28 channel (optional coordination layer)
    async fn send_channel_message(&self, channel_id: &str, msg: &AgentMessage) -> Result<()> {
        let msg_json = serde_json::to_string(msg)?;
        let relay_url = self.relay.relay_url();

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

    /// Refresh wallet balance and update agent state
    pub async fn refresh_wallet_balance(&self, state: &mut AgentStateContent) -> Result<u64> {
        let balance = self.wallet.get_balance().await?;
        let total = balance.total_sats();
        state.update_balance(total);
        Ok(total)
    }

    /// Pay a Lightning invoice via Spark
    pub async fn pay_invoice(&self, bolt11: &str, amount_sats: Option<u64>) -> Result<String> {
        let payment = self.wallet.send_payment_simple(bolt11, amount_sats).await?;
        Ok(payment.payment.id)
    }

    /// Create an invoice for receiving funds
    pub async fn create_invoice(
        &self,
        amount_sats: u64,
        memo: Option<String>,
        expiry_seconds: Option<u64>,
    ) -> Result<String> {
        let invoice = self
            .wallet
            .create_invoice(amount_sats, memo, expiry_seconds)
            .await?;
        Ok(invoice.payment_request)
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
