//! Agent economics model.
//!
//! Economics define how agents get paid for their work. This is built on
//! Bitcoin/Lightning and the NIP-90 Data Vending Machine protocol.
//!
//! # The Economic Model
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                      Agent Economics Flow                            │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                      │
//! │   Customer                        Service Provider (Agent)           │
//! │      │                                    │                          │
//! │      ├── NIP-90 JobRequest ──────────────>│                          │
//! │      │   (includes bid in millisats)      │                          │
//! │      │                                    │                          │
//! │      │<─────── JobFeedback ──────────────┤                          │
//! │      │   (payment-required + bolt11)      │                          │
//! │      │                                    │                          │
//! │      ├── Lightning Payment ──────────────>│                          │
//! │      │   (pay bolt11 invoice)             │                          │
//! │      │                                    │                          │
//! │      │<─────── JobResult ────────────────┤                          │
//! │      │   (result content)                 │                          │
//! │                                                                      │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Concepts
//!
//! - **Wallet**: Self-custodial Bitcoin wallet for receiving payments
//! - **Pricing**: How the agent charges for work (per-job, per-token, etc.)
//! - **Payment Methods**: Supported payment channels (Lightning, Spark, etc.)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Economic configuration for an agent.
///
/// This defines how the agent receives payment for services.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentEconomics {
    /// Bitcoin wallet configuration for receiving payments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<WalletConfig>,

    /// Pricing model.
    #[serde(default)]
    pub pricing: PricingModel,

    /// Accepted payment methods.
    #[serde(default)]
    pub payment_methods: Vec<PaymentMethod>,

    /// Minimum payment threshold in millisats.
    ///
    /// Jobs below this threshold may be rejected or aggregated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_payment: Option<u64>,

    /// Maximum job value in millisats (for risk management).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_job_value: Option<u64>,

    /// Whether to require payment before execution.
    #[serde(default)]
    pub require_prepayment: bool,

    /// Refund policy.
    #[serde(default)]
    pub refund_policy: RefundPolicy,

    /// Revenue sharing configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revenue_sharing: Option<RevenueSharing>,
}

impl AgentEconomics {
    /// Create new economics with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create economics for a free agent.
    pub fn free() -> Self {
        Self {
            pricing: PricingModel::Free,
            ..Default::default()
        }
    }

    /// Create a builder for economics.
    pub fn builder() -> AgentEconomicsBuilder {
        AgentEconomicsBuilder::default()
    }

    /// Calculate the cost for a job.
    pub fn calculate_cost(&self, job: &JobCostEstimate) -> u64 {
        match &self.pricing {
            PricingModel::Free => 0,
            PricingModel::PerJob { millisats } => *millisats,
            PricingModel::PerToken {
                input_millisats,
                output_millisats,
            } => {
                let input_cost = job.estimated_input_tokens.unwrap_or(0) as u64 * input_millisats;
                let output_cost =
                    job.estimated_output_tokens.unwrap_or(0) as u64 * output_millisats;
                input_cost + output_cost
            }
            PricingModel::PerSecond { millisats } => {
                job.estimated_duration_secs.unwrap_or(0) as u64 * millisats
            }
            PricingModel::Tiered { tiers } => {
                // Find matching tier based on job complexity
                let complexity = job.complexity.unwrap_or(1);
                tiers
                    .iter()
                    .find(|t| complexity <= t.max_complexity)
                    .map(|t| t.millisats)
                    .unwrap_or(0)
            }
            PricingModel::Custom { .. } => {
                // Custom pricing requires negotiation
                job.bid.unwrap_or(0)
            }
        }
    }

    /// Check if a bid is acceptable for a job.
    pub fn is_bid_acceptable(&self, bid: u64, job: &JobCostEstimate) -> bool {
        let min_price = self.calculate_cost(job);

        // Check minimum threshold
        if let Some(min) = self.min_payment {
            if bid < min && min_price > 0 {
                return false;
            }
        }

        // Check maximum value
        if let Some(max) = self.max_job_value {
            if bid > max {
                return false;
            }
        }

        bid >= min_price
    }
}

/// Builder for AgentEconomics.
#[derive(Default)]
pub struct AgentEconomicsBuilder {
    economics: AgentEconomics,
}

impl AgentEconomicsBuilder {
    /// Set the wallet configuration.
    pub fn wallet(mut self, wallet: WalletConfig) -> Self {
        self.economics.wallet = Some(wallet);
        self
    }

    /// Set the pricing model.
    pub fn pricing(mut self, pricing: PricingModel) -> Self {
        self.economics.pricing = pricing;
        self
    }

    /// Add a payment method.
    pub fn payment_method(mut self, method: PaymentMethod) -> Self {
        self.economics.payment_methods.push(method);
        self
    }

    /// Set minimum payment threshold.
    pub fn min_payment(mut self, millisats: u64) -> Self {
        self.economics.min_payment = Some(millisats);
        self
    }

    /// Set maximum job value.
    pub fn max_job_value(mut self, millisats: u64) -> Self {
        self.economics.max_job_value = Some(millisats);
        self
    }

    /// Require prepayment.
    pub fn require_prepayment(mut self) -> Self {
        self.economics.require_prepayment = true;
        self
    }

    /// Set refund policy.
    pub fn refund_policy(mut self, policy: RefundPolicy) -> Self {
        self.economics.refund_policy = policy;
        self
    }

    /// Set revenue sharing.
    pub fn revenue_sharing(mut self, sharing: RevenueSharing) -> Self {
        self.economics.revenue_sharing = Some(sharing);
        self
    }

    /// Build the economics.
    pub fn build(self) -> AgentEconomics {
        self.economics
    }
}

/// Bitcoin wallet configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Wallet type.
    #[serde(default)]
    pub wallet_type: WalletType,

    /// Lightning address (for receiving payments).
    ///
    /// Example: "agent@getalby.com"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lightning_address: Option<String>,

    /// LNURL for payments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lnurl: Option<String>,

    /// Nostr Wallet Connect URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nwc_uri: Option<String>,

    /// Spark wallet address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spark_address: Option<String>,

    /// On-chain Bitcoin address (for larger payments).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitcoin_address: Option<String>,
}

impl WalletConfig {
    /// Create a wallet config with a Lightning address.
    pub fn lightning(address: impl Into<String>) -> Self {
        Self {
            wallet_type: WalletType::Lightning,
            lightning_address: Some(address.into()),
            lnurl: None,
            nwc_uri: None,
            spark_address: None,
            bitcoin_address: None,
        }
    }

    /// Create a wallet config with Spark.
    pub fn spark(address: impl Into<String>) -> Self {
        Self {
            wallet_type: WalletType::Spark,
            lightning_address: None,
            lnurl: None,
            nwc_uri: None,
            spark_address: Some(address.into()),
            bitcoin_address: None,
        }
    }

    /// Create a wallet config with Nostr Wallet Connect.
    pub fn nwc(uri: impl Into<String>) -> Self {
        Self {
            wallet_type: WalletType::NostrWalletConnect,
            lightning_address: None,
            lnurl: None,
            nwc_uri: Some(uri.into()),
            spark_address: None,
            bitcoin_address: None,
        }
    }
}

/// Wallet type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletType {
    /// Lightning Network wallet.
    #[default]
    Lightning,
    /// Spark (Lightning SDK).
    Spark,
    /// Nostr Wallet Connect (NIP-47).
    NostrWalletConnect,
    /// On-chain Bitcoin.
    OnChain,
    /// Custodial wallet service.
    Custodial,
}

/// Pricing model for agent services.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PricingModel {
    /// Free (no payment required).
    #[default]
    Free,

    /// Fixed price per job.
    PerJob {
        /// Price in millisats.
        millisats: u64,
    },

    /// Price per token.
    PerToken {
        /// Price per input token in millisats.
        input_millisats: u64,
        /// Price per output token in millisats.
        output_millisats: u64,
    },

    /// Price per second of compute.
    PerSecond {
        /// Price per second in millisats.
        millisats: u64,
    },

    /// Tiered pricing based on job complexity.
    Tiered {
        /// Price tiers.
        tiers: Vec<PriceTier>,
    },

    /// Custom pricing (negotiated via NIP-90).
    Custom {
        /// Description of custom pricing.
        description: String,
    },
}

impl PricingModel {
    /// Create a per-job pricing model.
    pub fn per_job(millisats: u64) -> Self {
        Self::PerJob { millisats }
    }

    /// Create a per-token pricing model.
    pub fn per_token(input_millisats: u64, output_millisats: u64) -> Self {
        Self::PerToken {
            input_millisats,
            output_millisats,
        }
    }

    /// Create a per-second pricing model.
    pub fn per_second(millisats: u64) -> Self {
        Self::PerSecond { millisats }
    }
}

/// Price tier for tiered pricing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTier {
    /// Maximum complexity level for this tier.
    pub max_complexity: u32,
    /// Price in millisats.
    pub millisats: u64,
    /// Tier name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Payment methods accepted by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    /// Lightning Network payment.
    Lightning,
    /// Spark payment.
    Spark,
    /// Nostr zap (NIP-57).
    Zap,
    /// On-chain Bitcoin.
    OnChain,
    /// Ecash (Cashu).
    Ecash,
    /// Credit (for trusted customers).
    Credit,
}

/// Refund policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RefundPolicy {
    /// No refunds.
    #[default]
    NoRefunds,

    /// Full refund if job fails.
    FullRefundOnFailure,

    /// Partial refund based on progress.
    PartialRefund {
        /// Minimum refund percentage.
        min_refund_percent: u8,
    },

    /// Custom refund policy.
    Custom {
        /// Policy description.
        description: String,
    },
}

/// Revenue sharing configuration.
///
/// Allows splitting revenue with other parties (e.g., agent creators,
/// compute providers, referrers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSharing {
    /// Revenue shares.
    pub shares: Vec<RevenueShare>,
}

impl RevenueSharing {
    /// Create a new revenue sharing config.
    pub fn new(shares: Vec<RevenueShare>) -> Self {
        Self { shares }
    }

    /// Calculate shares for a payment.
    pub fn calculate_shares(&self, total_millisats: u64) -> HashMap<String, u64> {
        let mut result = HashMap::new();

        for share in &self.shares {
            let amount = (total_millisats as f64 * (share.percentage as f64 / 100.0)) as u64;
            result.insert(share.recipient.clone(), amount);
        }

        result
    }
}

/// A revenue share.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueShare {
    /// Recipient identifier (npub or lightning address).
    pub recipient: String,

    /// Percentage of revenue (0-100).
    pub percentage: u8,

    /// Share type.
    #[serde(default)]
    pub share_type: ShareType,
}

/// Type of revenue share.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareType {
    /// Agent creator/owner.
    #[default]
    Creator,
    /// Compute provider.
    ComputeProvider,
    /// Platform fee.
    Platform,
    /// Referral fee.
    Referral,
    /// Other.
    Other,
}

// Types for cost calculation

/// Estimate for calculating job cost.
#[derive(Debug, Clone, Default)]
pub struct JobCostEstimate {
    /// Estimated input tokens.
    pub estimated_input_tokens: Option<u32>,
    /// Estimated output tokens.
    pub estimated_output_tokens: Option<u32>,
    /// Estimated duration in seconds.
    pub estimated_duration_secs: Option<u32>,
    /// Job complexity level (1-10).
    pub complexity: Option<u32>,
    /// Customer's bid in millisats.
    pub bid: Option<u64>,
}

/// Invoice for a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInvoice {
    /// Job ID.
    pub job_id: String,

    /// Amount in millisats.
    pub amount_millisats: u64,

    /// BOLT11 invoice string.
    pub bolt11: String,

    /// Expiry timestamp.
    pub expires_at: u64,

    /// Payment hash.
    pub payment_hash: String,

    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Payment receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentReceipt {
    /// Job ID.
    pub job_id: String,

    /// Amount paid in millisats.
    pub amount_millisats: u64,

    /// Payment preimage (proof of payment).
    pub preimage: String,

    /// Payment timestamp.
    pub paid_at: u64,

    /// Payment method used.
    pub method: PaymentMethod,
}

// Helper constants for common pricing

/// 1 satoshi in millisats
pub const SATS_TO_MILLISATS: u64 = 1000;

/// Common pricing: 1 sat per 1000 tokens
pub const PRICE_1_SAT_PER_1K_TOKENS: u64 = 1;

/// Common pricing: 10 sats per job
pub const PRICE_10_SATS_PER_JOB: u64 = 10 * SATS_TO_MILLISATS;

/// Common pricing: 100 sats per job
pub const PRICE_100_SATS_PER_JOB: u64 = 100 * SATS_TO_MILLISATS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_economics_builder() {
        let economics = AgentEconomics::builder()
            .wallet(WalletConfig::lightning("agent@getalby.com"))
            .pricing(PricingModel::per_job(10000))
            .payment_method(PaymentMethod::Lightning)
            .min_payment(1000)
            .build();

        assert!(economics.wallet.is_some());
        assert_eq!(economics.min_payment, Some(1000));
    }

    #[test]
    fn test_per_job_pricing() {
        let economics = AgentEconomics {
            pricing: PricingModel::per_job(10000),
            ..Default::default()
        };

        let job = JobCostEstimate::default();
        assert_eq!(economics.calculate_cost(&job), 10000);
    }

    #[test]
    fn test_per_token_pricing() {
        let economics = AgentEconomics {
            pricing: PricingModel::per_token(1, 2),
            ..Default::default()
        };

        let job = JobCostEstimate {
            estimated_input_tokens: Some(1000),
            estimated_output_tokens: Some(500),
            ..Default::default()
        };

        // 1000 * 1 + 500 * 2 = 2000
        assert_eq!(economics.calculate_cost(&job), 2000);
    }

    #[test]
    fn test_bid_acceptance() {
        let economics = AgentEconomics {
            pricing: PricingModel::per_job(10000),
            min_payment: Some(5000),
            max_job_value: Some(100000),
            ..Default::default()
        };

        let job = JobCostEstimate::default();

        // Bid below minimum price
        assert!(!economics.is_bid_acceptable(5000, &job));

        // Bid at minimum price
        assert!(economics.is_bid_acceptable(10000, &job));

        // Bid above minimum
        assert!(economics.is_bid_acceptable(15000, &job));

        // Bid above maximum
        assert!(!economics.is_bid_acceptable(150000, &job));
    }

    #[test]
    fn test_revenue_sharing() {
        let sharing = RevenueSharing::new(vec![
            RevenueShare {
                recipient: "creator@example.com".into(),
                percentage: 70,
                share_type: ShareType::Creator,
            },
            RevenueShare {
                recipient: "platform@example.com".into(),
                percentage: 30,
                share_type: ShareType::Platform,
            },
        ]);

        let shares = sharing.calculate_shares(10000);

        assert_eq!(shares.get("creator@example.com"), Some(&7000));
        assert_eq!(shares.get("platform@example.com"), Some(&3000));
    }

    #[test]
    fn test_free_pricing() {
        let economics = AgentEconomics::free();

        let job = JobCostEstimate {
            estimated_input_tokens: Some(1000000),
            estimated_output_tokens: Some(1000000),
            ..Default::default()
        };

        assert_eq!(economics.calculate_cost(&job), 0);
        assert!(economics.is_bid_acceptable(0, &job));
    }

    #[test]
    fn test_wallet_config() {
        let lightning = WalletConfig::lightning("agent@getalby.com");
        assert_eq!(lightning.wallet_type, WalletType::Lightning);
        assert!(lightning.lightning_address.is_some());

        let spark = WalletConfig::spark("sp1abc...");
        assert_eq!(spark.wallet_type, WalletType::Spark);
        assert!(spark.spark_address.is_some());
    }
}
