//! Compute Client for Agent Reasoning
//!
//! Uses runtime DVM provider support for NIP-90 requests and wallet settlement.

use crate::agents::SharedRelay;
use anyhow::{Result, anyhow};
use nostr::nip_sa::AgentStateContent;
use nostr::HandlerType;
use openagents_runtime::{
    AgentId, ComputeKind, ComputeProvider, ComputeRequest, ComputeResponse, DvmProvider, FxSource,
    JobState, UnifiedIdentity, UnifiedIdentitySigner, WalletInvoice, WalletPayment, WalletService,
};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{Instant, sleep};

const DEFAULT_MODEL: &str = "llama3.2";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const FX_CACHE_SECONDS: u64 = 60;

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

/// Result from an inference request.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    pub text: String,
    pub cost_sats: Option<u64>,
    pub response: ComputeResponse,
}

/// Compute client for agents to buy inference
pub struct ComputeClient {
    provider: Arc<DvmProvider>,
    wallet: Arc<dyn WalletService>,
    relay_urls: Vec<String>,
    network_filter: Option<String>,
}

impl ComputeClient {
    /// Create a new compute client.
    pub fn new(
        identity: UnifiedIdentity,
        relay: SharedRelay,
        wallet: Arc<dyn WalletService>,
        network: Option<String>,
    ) -> Result<Self> {
        let agent_id = AgentId::new(identity.public_key_hex());
        let signer = Arc::new(UnifiedIdentitySigner::new(Arc::new(identity)));
        let relays = relay.relay_urls();
        let provider = Arc::new(
            DvmProvider::new(
                agent_id,
                relays.clone(),
                signer,
                Some(wallet.clone()),
                FxSource::Wallet,
                FX_CACHE_SECONDS,
            )
            .map_err(|err| anyhow!(err.to_string()))?,
        );
        Ok(Self {
            provider,
            wallet,
            relay_urls: relays,
            network_filter: network.map(|value| value.to_lowercase()),
        })
    }

    /// Discover compute providers via NIP-89 (kind 31990).
    pub fn discover_providers(&self, timeout_secs: u64) -> Result<Vec<ProviderInfo>> {
        let handlers = self
            .provider
            .list_handlers(Duration::from_secs(timeout_secs))
            .map_err(|err| anyhow!(err.to_string()))?;
        let mut providers = Vec::new();

        for handler in handlers {
            if handler.handler_type != HandlerType::ComputeProvider {
                continue;
            }

            let channel_id = handler
                .custom_tags
                .iter()
                .find(|(k, _)| k == "channel")
                .map(|(_, v)| v.clone());

            let relay_url = handler
                .custom_tags
                .iter()
                .find(|(k, _)| k == "relay")
                .map(|(_, v)| v.clone())
                .or_else(|| self.relay_urls.first().cloned())
                .unwrap_or_default();

            let network = handler
                .custom_tags
                .iter()
                .find(|(k, _)| k == "network")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "unknown".to_string())
                .to_lowercase();

            if let Some(filter) = self.network_filter.as_ref() {
                if network != *filter {
                    continue;
                }
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

    /// Pick a default model from advertised providers.
    pub fn select_model(providers: &[ProviderInfo]) -> String {
        providers
            .iter()
            .flat_map(|provider| provider.models.iter())
            .find(|model| !model.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| DEFAULT_MODEL.to_string())
    }

    /// Request inference from the DVM network.
    pub async fn request_inference(
        &self,
        prompt: &str,
        max_tokens: u32,
        budget_sats: u64,
        model: &str,
    ) -> Result<InferenceResult> {
        let max_cost_usd = self.budget_to_usd(budget_sats)?;
        let request = ComputeRequest {
            model: model.to_string(),
            kind: ComputeKind::Complete,
            input: serde_json::json!({
                "prompt": prompt,
                "max_tokens": max_tokens,
            }),
            stream: false,
            timeout_ms: Some(DEFAULT_TIMEOUT_MS),
            idempotency_key: None,
            max_cost_usd: Some(max_cost_usd),
        };

        let job_id = self
            .provider
            .submit(request)
            .map_err(|err| anyhow!(err.to_string()))?;
        let timeout = Duration::from_millis(DEFAULT_TIMEOUT_MS);
        let deadline = Instant::now() + timeout;

        loop {
            if Instant::now() >= deadline {
                let _ = self.provider.cancel(&job_id);
                return Err(anyhow!("compute request timed out"));
            }

            if let Some(state) = self.provider.get_job(&job_id) {
                match state {
                    JobState::Complete(response) => {
                        let text = extract_text(&response.output);
                        let cost_sats = self.cost_sats_from_usd(response.cost_usd);
                        return Ok(InferenceResult {
                            text,
                            cost_sats,
                            response,
                        });
                    }
                    JobState::Failed { error, .. } => {
                        return Err(anyhow!(error));
                    }
                    _ => {}
                }
            }

            sleep(Duration::from_millis(200)).await;
        }
    }

    /// Refresh wallet balance and update agent state.
    pub fn refresh_wallet_balance(&self, state: &mut AgentStateContent) -> Result<u64> {
        let balance = self.wallet.balance_sats()?;
        state.update_balance(balance);
        Ok(balance)
    }

    /// Pay a Lightning invoice.
    pub fn pay_invoice(&self, bolt11: &str, amount_sats: Option<u64>) -> Result<String> {
        let payment: WalletPayment = self.wallet.pay_invoice(bolt11, amount_sats)?;
        Ok(payment.payment_id)
    }

    /// Create an invoice for receiving funds.
    pub fn create_invoice(
        &self,
        amount_sats: u64,
        memo: Option<String>,
        expiry_seconds: Option<u64>,
    ) -> Result<String> {
        let invoice: WalletInvoice =
            self.wallet.create_invoice(amount_sats, memo, expiry_seconds)?;
        Ok(invoice.payment_request)
    }

    fn budget_to_usd(&self, budget_sats: u64) -> Result<u64> {
        let fx = self.wallet.fx_rate()?;
        if fx.sats_per_usd == 0 {
            return Err(anyhow!("invalid FX rate"));
        }
        let numerator = u128::from(budget_sats) * 1_000_000u128;
        let micro_usd = numerator / u128::from(fx.sats_per_usd);
        Ok(u64::try_from(micro_usd).unwrap_or(u64::MAX))
    }

    fn cost_sats_from_usd(&self, cost_usd: u64) -> Option<u64> {
        let fx = self.wallet.fx_rate().ok()?;
        if fx.sats_per_usd == 0 {
            return None;
        }
        let numerator = u128::from(cost_usd) * u128::from(fx.sats_per_usd);
        let sats = numerator / 1_000_000u128;
        u64::try_from(sats).ok()
    }
}

fn extract_text(output: &Value) -> String {
    if let Some(text) = output.as_str() {
        return text.to_string();
    }

    if let Some(obj) = output.as_object() {
        if let Some(text) = obj.get("text").and_then(|value| value.as_str()) {
            return text.to_string();
        }
        if let Some(text) = obj.get("content").and_then(|value| value.as_str()) {
            return text.to_string();
        }
        if let Some(text) = obj.get("message").and_then(|value| value.as_str()) {
            return text.to_string();
        }
        if let Some(text) = obj.get("output").and_then(|value| value.as_str()) {
            return text.to_string();
        }
        if let Some(choices) = obj.get("choices").and_then(|value| value.as_array()) {
            if let Some(text) = choices
                .iter()
                .find_map(|choice| choice.get("text").and_then(|value| value.as_str()))
            {
                return text.to_string();
            }
            if let Some(text) = choices.iter().find_map(|choice| {
                choice
                    .get("message")
                    .and_then(|message| message.get("content"))
                    .and_then(|value| value.as_str())
            }) {
                return text.to_string();
            }
        }
    }

    output.to_string()
}
