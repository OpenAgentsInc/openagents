//! BTC <-> stablesat USD swap contract and policy model.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum SwapDirection {
    BtcToUsd,
    UsdToBtc,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum SwapAmountUnit {
    Sats,
    Cents,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapAmount {
    pub amount: u64,
    pub unit: SwapAmountUnit,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapQuoteTerms {
    pub quote_id: String,
    pub direction: SwapDirection,
    pub amount_in: SwapAmount,
    pub amount_out: SwapAmount,
    pub expires_at_epoch_seconds: u64,
    pub immediate_execution: bool,
    pub fee_sats: u64,
    pub fee_bps: u32,
    pub slippage_bps: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum SwapFailureKind {
    QuoteUnavailable,
    QuoteExpired,
    SlippageExceeded,
    FeeLimitExceeded,
    PolicyViolation,
    WalletUnavailable,
    SettlementFailed,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapFailure {
    pub kind: SwapFailureKind,
    pub retryable: bool,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapPolicy {
    pub max_per_swap_sats: u64,
    pub max_per_swap_cents: u64,
    pub max_daily_converted_sats: u64,
    pub max_daily_converted_cents: u64,
    pub max_fee_sats: u64,
    pub max_slippage_bps: u32,
    pub require_quote_confirmation: bool,
}

impl Default for SwapPolicy {
    fn default() -> Self {
        Self {
            max_per_swap_sats: 500_000,
            max_per_swap_cents: 5_000_00,
            max_daily_converted_sats: 2_000_000,
            max_daily_converted_cents: 20_000_00,
            max_fee_sats: 5_000,
            max_slippage_bps: 150,
            require_quote_confirmation: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapExecutionRequest {
    pub request_id: String,
    pub direction: SwapDirection,
    pub amount: SwapAmount,
    pub quote_ttl_seconds: u64,
    pub immediate_execution: bool,
    pub max_fee_sats_override: Option<u64>,
    pub max_slippage_bps_override: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapExecutionReceipt {
    pub request_id: String,
    pub direction: SwapDirection,
    pub quote_id: Option<String>,
    pub amount_in: Option<SwapAmount>,
    pub amount_out: Option<SwapAmount>,
    pub started_at_epoch_seconds: u64,
    pub finished_at_epoch_seconds: u64,
    pub succeeded: bool,
    pub failure: Option<SwapFailure>,
}

impl SwapPolicy {
    pub fn validate_request(
        &self,
        request: &SwapExecutionRequest,
        daily_converted_sats: u64,
        daily_converted_cents: u64,
    ) -> Result<(), String> {
        if request.request_id.trim().is_empty() {
            return Err("Swap request id cannot be empty".to_string());
        }
        if request.amount.amount == 0 {
            return Err("Swap amount must be greater than zero".to_string());
        }
        if request.quote_ttl_seconds == 0 {
            return Err("Swap quote ttl must be greater than zero".to_string());
        }

        match request.amount.unit {
            SwapAmountUnit::Sats => {
                if request.amount.amount > self.max_per_swap_sats {
                    return Err(format!(
                        "Swap sats amount {} exceeds per-swap limit {}",
                        request.amount.amount, self.max_per_swap_sats
                    ));
                }
                if daily_converted_sats.saturating_add(request.amount.amount)
                    > self.max_daily_converted_sats
                {
                    return Err(format!(
                        "Swap sats would exceed daily limit {}",
                        self.max_daily_converted_sats
                    ));
                }
            }
            SwapAmountUnit::Cents => {
                if request.amount.amount > self.max_per_swap_cents {
                    return Err(format!(
                        "Swap cents amount {} exceeds per-swap limit {}",
                        request.amount.amount, self.max_per_swap_cents
                    ));
                }
                if daily_converted_cents.saturating_add(request.amount.amount)
                    > self.max_daily_converted_cents
                {
                    return Err(format!(
                        "Swap cents would exceed daily limit {}",
                        self.max_daily_converted_cents
                    ));
                }
            }
        }

        if let Some(max_fee_override) = request.max_fee_sats_override
            && max_fee_override > self.max_fee_sats
        {
            return Err(format!(
                "Swap max fee override {} exceeds policy {}",
                max_fee_override, self.max_fee_sats
            ));
        }
        if let Some(max_slippage_override) = request.max_slippage_bps_override
            && max_slippage_override > self.max_slippage_bps
        {
            return Err(format!(
                "Swap max slippage override {} exceeds policy {}",
                max_slippage_override, self.max_slippage_bps
            ));
        }

        Ok(())
    }

    pub fn validate_quote(
        &self,
        quote: &SwapQuoteTerms,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        if quote.quote_id.trim().is_empty() {
            return Err("Swap quote id cannot be empty".to_string());
        }
        if now_epoch_seconds >= quote.expires_at_epoch_seconds {
            return Err("Swap quote expired".to_string());
        }
        if quote.fee_sats > self.max_fee_sats {
            return Err(format!(
                "Swap quote fee {} exceeds policy {}",
                quote.fee_sats, self.max_fee_sats
            ));
        }
        if quote.slippage_bps > self.max_slippage_bps {
            return Err(format!(
                "Swap quote slippage {} exceeds policy {}",
                quote.slippage_bps, self.max_slippage_bps
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SwapAmount, SwapAmountUnit, SwapDirection, SwapExecutionRequest, SwapPolicy, SwapQuoteTerms,
    };

    #[test]
    fn swap_policy_rejects_request_over_per_swap_limit() {
        let policy = SwapPolicy::default();
        let request = SwapExecutionRequest {
            request_id: "swap-1".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount: SwapAmount {
                amount: policy.max_per_swap_sats.saturating_add(1),
                unit: SwapAmountUnit::Sats,
            },
            quote_ttl_seconds: 30,
            immediate_execution: true,
            max_fee_sats_override: None,
            max_slippage_bps_override: None,
        };
        let error = policy
            .validate_request(&request, 0, 0)
            .expect_err("request above per-swap sats limit should fail");
        assert!(error.contains("exceeds per-swap limit"));
    }

    #[test]
    fn swap_policy_rejects_expired_quote() {
        let policy = SwapPolicy::default();
        let quote = SwapQuoteTerms {
            quote_id: "quote-1".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount_in: SwapAmount {
                amount: 1000,
                unit: SwapAmountUnit::Sats,
            },
            amount_out: SwapAmount {
                amount: 800,
                unit: SwapAmountUnit::Cents,
            },
            expires_at_epoch_seconds: 100,
            immediate_execution: true,
            fee_sats: 50,
            fee_bps: 10,
            slippage_bps: 20,
        };
        let error = policy
            .validate_quote(&quote, 100)
            .expect_err("expired quote should fail");
        assert!(error.contains("expired"));
    }

    #[test]
    fn swap_policy_accepts_valid_request_and_quote() {
        let policy = SwapPolicy::default();
        let request = SwapExecutionRequest {
            request_id: "swap-ok".to_string(),
            direction: SwapDirection::UsdToBtc,
            amount: SwapAmount {
                amount: 100_00,
                unit: SwapAmountUnit::Cents,
            },
            quote_ttl_seconds: 60,
            immediate_execution: false,
            max_fee_sats_override: Some(100),
            max_slippage_bps_override: Some(20),
        };
        policy
            .validate_request(&request, 0, 0)
            .expect("valid request should pass");

        let quote = SwapQuoteTerms {
            quote_id: "quote-ok".to_string(),
            direction: SwapDirection::UsdToBtc,
            amount_in: request.amount,
            amount_out: SwapAmount {
                amount: 5_000,
                unit: SwapAmountUnit::Sats,
            },
            expires_at_epoch_seconds: 999,
            immediate_execution: false,
            fee_sats: 100,
            fee_bps: 20,
            slippage_bps: 20,
        };
        policy
            .validate_quote(&quote, 900)
            .expect("valid quote should pass");
    }
}
