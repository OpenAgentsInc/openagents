//! Agent economics model - Bitcoin/Lightning payments.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentEconomics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<WalletConfig>,
    #[serde(default)]
    pub pricing: PricingModel,
    #[serde(default)]
    pub payment_methods: Vec<PaymentMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_payment: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_job_value: Option<u64>,
    #[serde(default)]
    pub require_prepayment: bool,
    #[serde(default)]
    pub refund_policy: RefundPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revenue_sharing: Option<RevenueSharing>,
}

impl AgentEconomics {
    pub fn new() -> Self { Self::default() }
    pub fn free() -> Self {
        Self { pricing: PricingModel::Free, ..Default::default() }
    }
    pub fn builder() -> AgentEconomicsBuilder { AgentEconomicsBuilder::default() }

    pub fn calculate_cost(&self, job: &JobCostEstimate) -> u64 {
        match &self.pricing {
            PricingModel::Free => 0,
            PricingModel::PerJob { millisats } => *millisats,
            PricingModel::PerToken { input_millisats, output_millisats } => {
                let input_cost = job.estimated_input_tokens.unwrap_or(0) as u64 * input_millisats;
                let output_cost = job.estimated_output_tokens.unwrap_or(0) as u64 * output_millisats;
                input_cost + output_cost
            }
            PricingModel::PerSecond { millisats } => {
                job.estimated_duration_secs.unwrap_or(0) as u64 * millisats
            }
            PricingModel::Tiered { tiers } => {
                let complexity = job.complexity.unwrap_or(1);
                tiers.iter().find(|t| complexity <= t.max_complexity).map(|t| t.millisats).unwrap_or(0)
            }
            PricingModel::Custom { .. } => job.bid.unwrap_or(0),
        }
    }

    pub fn is_bid_acceptable(&self, bid: u64, job: &JobCostEstimate) -> bool {
        let min_price = self.calculate_cost(job);
        if let Some(min) = self.min_payment {
            if bid < min && min_price > 0 { return false; }
        }
        if let Some(max) = self.max_job_value {
            if bid > max { return false; }
        }
        bid >= min_price
    }
}

#[derive(Default)]
pub struct AgentEconomicsBuilder {
    economics: AgentEconomics,
}

impl AgentEconomicsBuilder {
    pub fn wallet(mut self, wallet: WalletConfig) -> Self {
        self.economics.wallet = Some(wallet);
        self
    }
    pub fn pricing(mut self, pricing: PricingModel) -> Self {
        self.economics.pricing = pricing;
        self
    }
    pub fn build(self) -> AgentEconomics {
        self.economics
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletConfig {
    #[serde(default)]
    pub wallet_type: WalletType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lightning_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lnurl: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nwc_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spark_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitcoin_address: Option<String>,
}

impl WalletConfig {
    pub fn lightning(address: impl Into<String>) -> Self {
        Self { wallet_type: WalletType::Lightning, lightning_address: Some(address.into()), ..Default::default() }
    }
    pub fn spark(address: impl Into<String>) -> Self {
        Self { wallet_type: WalletType::Spark, spark_address: Some(address.into()), ..Default::default() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletType {
    #[default]
    Lightning,
    Spark,
    NostrWalletConnect,
    OnChain,
    Custodial,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PricingModel {
    #[default]
    Free,
    PerJob { millisats: u64 },
    PerToken { input_millisats: u64, output_millisats: u64 },
    PerSecond { millisats: u64 },
    Tiered { tiers: Vec<PriceTier> },
    Custom { description: String },
}

impl PricingModel {
    pub fn per_job(millisats: u64) -> Self {
        Self::PerJob { millisats }
    }
    pub fn per_token(input_millisats: u64, output_millisats: u64) -> Self {
        Self::PerToken { input_millisats, output_millisats }
    }
    pub fn per_second(millisats: u64) -> Self {
        Self::PerSecond { millisats }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTier {
    pub max_complexity: u32,
    pub millisats: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Lightning,
    Spark,
    Zap,
    OnChain,
    Ecash,
    Credit,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RefundPolicy {
    #[default]
    NoRefunds,
    FullRefundOnFailure,
    PartialRefund { min_refund_percent: u8 },
    Custom { description: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSharing {
    pub shares: Vec<RevenueShare>,
}

impl RevenueSharing {
    pub fn new(shares: Vec<RevenueShare>) -> Self {
        Self { shares }
    }
    pub fn calculate_shares(&self, total_millisats: u64) -> HashMap<String, u64> {
        let mut result = HashMap::new();
        for share in &self.shares {
            let amount = (total_millisats as f64 * (share.percentage as f64 / 100.0)) as u64;
            result.insert(share.recipient.clone(), amount);
        }
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueShare {
    pub recipient: String,
    pub percentage: u8,
    #[serde(default)]
    pub share_type: ShareType,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareType {
    #[default]
    Creator,
    ComputeProvider,
    Platform,
    Referral,
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct JobCostEstimate {
    pub estimated_input_tokens: Option<u32>,
    pub estimated_output_tokens: Option<u32>,
    pub estimated_duration_secs: Option<u32>,
    pub complexity: Option<u32>,
    pub bid: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInvoice {
    pub job_id: String,
    pub amount_millisats: u64,
    pub bolt11: String,
    pub expires_at: u64,
    pub payment_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentReceipt {
    pub job_id: String,
    pub amount_millisats: u64,
    pub preimage: String,
    pub paid_at: u64,
    pub method: PaymentMethod,
}

pub const SATS_TO_MILLISATS: u64 = 1000;
pub const PRICE_1_SAT_PER_1K_TOKENS: u64 = 1;
pub const PRICE_10_SATS_PER_JOB: u64 = 10 * SATS_TO_MILLISATS;
pub const PRICE_100_SATS_PER_JOB: u64 = 100 * SATS_TO_MILLISATS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_per_job_pricing() {
        let economics = AgentEconomics { pricing: PricingModel::per_job(10000), ..Default::default() };
        let job = JobCostEstimate::default();
        assert_eq!(economics.calculate_cost(&job), 10000);
    }
}
