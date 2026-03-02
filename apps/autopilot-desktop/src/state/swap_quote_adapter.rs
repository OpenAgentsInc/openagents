//! Stablesats quote adapter path with fallback semantics.

use serde::{Deserialize, Serialize};

use crate::state::swap_contract::{SwapAmount, SwapAmountUnit, SwapDirection, SwapQuoteTerms};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum SwapQuoteProvider {
    BlinkInfrastructure,
    StablesatsQuoteService,
    BlinkFallback,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum StablesatsQuoteFor {
    AmountToSellInSats(u64),
    AmountToBuyInCents(u64),
    AmountToBuyInSats(u64),
    AmountToSellInCents(u64),
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct StablesatsQuoteResponse {
    pub quote_id: String,
    pub amount_to_sell_in_sats: Option<u64>,
    pub amount_to_buy_in_cents: Option<u64>,
    pub amount_to_buy_in_sats: Option<u64>,
    pub amount_to_sell_in_cents: Option<u64>,
    pub expires_at_epoch_seconds: u64,
    pub executed: bool,
}

pub trait StablesatsQuoteClient {
    fn get_quote_to_buy_usd(
        &mut self,
        quote_for: StablesatsQuoteFor,
        immediate_execution: bool,
    ) -> Result<StablesatsQuoteResponse, String>;

    fn get_quote_to_sell_usd(
        &mut self,
        quote_for: StablesatsQuoteFor,
        immediate_execution: bool,
    ) -> Result<StablesatsQuoteResponse, String>;

    fn accept_quote(&mut self, quote_id: &str) -> Result<(), String>;
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapQuoteAdapterRequest {
    pub request_id: String,
    pub direction: SwapDirection,
    pub amount: SwapAmount,
    pub immediate_execution: bool,
    pub now_epoch_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapQuoteAdapterOutcome {
    pub provider: SwapQuoteProvider,
    pub quote: SwapQuoteTerms,
    pub fallback_reason: Option<String>,
    pub accepted_via_adapter: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapQuoteAuditReceipt {
    pub audit_id: String,
    pub goal_id: String,
    pub request_id: String,
    pub provider: SwapQuoteProvider,
    pub quote_id: String,
    pub direction: SwapDirection,
    pub amount_in: SwapAmount,
    pub amount_out: SwapAmount,
    pub fee_sats: u64,
    pub expires_at_epoch_seconds: u64,
    pub immediate_execution: bool,
    pub executed: bool,
    pub accepted_via_adapter: bool,
    pub fallback_reason: Option<String>,
    pub created_at_epoch_seconds: u64,
}

pub fn request_quote_with_fallback<C: StablesatsQuoteClient>(
    client: &mut C,
    request: &SwapQuoteAdapterRequest,
    fallback_quote: SwapQuoteTerms,
) -> SwapQuoteAdapterOutcome {
    let attempted = request_stablesats_quote(client, request);

    match attempted {
        Ok(mut outcome) => {
            if request.immediate_execution && !outcome.accepted_via_adapter {
                let accept_result = client.accept_quote(&outcome.quote.quote_id);
                match accept_result {
                    Ok(()) => {
                        outcome.accepted_via_adapter = true;
                    }
                    Err(error) => {
                        return fallback_outcome(
                            fallback_quote,
                            format!("stablesats accept quote failed: {error}"),
                        );
                    }
                }
            }
            outcome
        }
        Err(error) => fallback_outcome(fallback_quote, error),
    }
}

pub fn build_swap_quote_audit_receipt(
    goal_id: &str,
    request: &SwapQuoteAdapterRequest,
    outcome: &SwapQuoteAdapterOutcome,
) -> SwapQuoteAuditReceipt {
    SwapQuoteAuditReceipt {
        audit_id: format!("swap-quote-audit-{}", request.request_id),
        goal_id: goal_id.to_string(),
        request_id: request.request_id.clone(),
        provider: outcome.provider,
        quote_id: outcome.quote.quote_id.clone(),
        direction: outcome.quote.direction,
        amount_in: outcome.quote.amount_in,
        amount_out: outcome.quote.amount_out,
        fee_sats: outcome.quote.fee_sats,
        expires_at_epoch_seconds: outcome.quote.expires_at_epoch_seconds,
        immediate_execution: outcome.quote.immediate_execution,
        executed: outcome.accepted_via_adapter,
        accepted_via_adapter: outcome.accepted_via_adapter,
        fallback_reason: outcome.fallback_reason.clone(),
        created_at_epoch_seconds: request.now_epoch_seconds,
    }
}

fn request_stablesats_quote<C: StablesatsQuoteClient>(
    client: &mut C,
    request: &SwapQuoteAdapterRequest,
) -> Result<SwapQuoteAdapterOutcome, String> {
    if request.request_id.trim().is_empty() {
        return Err("swap request id cannot be empty".to_string());
    }
    if request.amount.amount == 0 {
        return Err("swap request amount must be greater than zero".to_string());
    }

    let quote_for = to_stablesats_quote_for(request)?;
    let response = match request.direction {
        SwapDirection::BtcToUsd => {
            client.get_quote_to_buy_usd(quote_for, request.immediate_execution)?
        }
        SwapDirection::UsdToBtc => {
            client.get_quote_to_sell_usd(quote_for, request.immediate_execution)?
        }
    };

    let quote = to_swap_quote_terms(request, &response)?;
    Ok(SwapQuoteAdapterOutcome {
        provider: SwapQuoteProvider::StablesatsQuoteService,
        quote,
        fallback_reason: None,
        accepted_via_adapter: request.immediate_execution && response.executed,
    })
}

fn to_stablesats_quote_for(
    request: &SwapQuoteAdapterRequest,
) -> Result<StablesatsQuoteFor, String> {
    match (request.direction, request.amount.unit) {
        (SwapDirection::BtcToUsd, SwapAmountUnit::Sats) => Ok(
            StablesatsQuoteFor::AmountToSellInSats(request.amount.amount),
        ),
        (SwapDirection::BtcToUsd, SwapAmountUnit::Cents) => Ok(
            StablesatsQuoteFor::AmountToBuyInCents(request.amount.amount),
        ),
        (SwapDirection::UsdToBtc, SwapAmountUnit::Sats) => {
            Ok(StablesatsQuoteFor::AmountToBuyInSats(request.amount.amount))
        }
        (SwapDirection::UsdToBtc, SwapAmountUnit::Cents) => Ok(
            StablesatsQuoteFor::AmountToSellInCents(request.amount.amount),
        ),
    }
}

fn to_swap_quote_terms(
    request: &SwapQuoteAdapterRequest,
    response: &StablesatsQuoteResponse,
) -> Result<SwapQuoteTerms, String> {
    if response.quote_id.trim().is_empty() {
        return Err("stablesats quote id is empty".to_string());
    }
    if response.expires_at_epoch_seconds <= request.now_epoch_seconds {
        return Err("stablesats quote already expired".to_string());
    }

    let (amount_in, amount_out) = match request.direction {
        SwapDirection::BtcToUsd => {
            let amount_in_sats = response
                .amount_to_sell_in_sats
                .ok_or_else(|| "missing amount_to_sell_in_sats".to_string())?;
            let amount_out_cents = response
                .amount_to_buy_in_cents
                .ok_or_else(|| "missing amount_to_buy_in_cents".to_string())?;
            (
                SwapAmount {
                    amount: amount_in_sats,
                    unit: SwapAmountUnit::Sats,
                },
                SwapAmount {
                    amount: amount_out_cents,
                    unit: SwapAmountUnit::Cents,
                },
            )
        }
        SwapDirection::UsdToBtc => {
            let amount_in_cents = response
                .amount_to_sell_in_cents
                .ok_or_else(|| "missing amount_to_sell_in_cents".to_string())?;
            let amount_out_sats = response
                .amount_to_buy_in_sats
                .ok_or_else(|| "missing amount_to_buy_in_sats".to_string())?;
            (
                SwapAmount {
                    amount: amount_in_cents,
                    unit: SwapAmountUnit::Cents,
                },
                SwapAmount {
                    amount: amount_out_sats,
                    unit: SwapAmountUnit::Sats,
                },
            )
        }
    };

    Ok(SwapQuoteTerms {
        quote_id: response.quote_id.clone(),
        direction: request.direction,
        amount_in,
        amount_out,
        expires_at_epoch_seconds: response.expires_at_epoch_seconds,
        immediate_execution: request.immediate_execution,
        fee_sats: 0,
        fee_bps: 0,
        slippage_bps: 0,
    })
}

fn fallback_outcome(fallback_quote: SwapQuoteTerms, reason: String) -> SwapQuoteAdapterOutcome {
    SwapQuoteAdapterOutcome {
        provider: SwapQuoteProvider::BlinkFallback,
        quote: fallback_quote,
        fallback_reason: Some(reason),
        accepted_via_adapter: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StablesatsQuoteClient, StablesatsQuoteFor, StablesatsQuoteResponse,
        SwapQuoteAdapterRequest, SwapQuoteProvider, build_swap_quote_audit_receipt,
        request_quote_with_fallback,
    };
    use crate::state::swap_contract::{SwapAmount, SwapAmountUnit, SwapDirection, SwapQuoteTerms};

    #[derive(Default)]
    struct FakeStablesatsClient {
        buy_response: Option<Result<StablesatsQuoteResponse, String>>,
        sell_response: Option<Result<StablesatsQuoteResponse, String>>,
        accepted_quote_ids: Vec<String>,
    }

    impl StablesatsQuoteClient for FakeStablesatsClient {
        fn get_quote_to_buy_usd(
            &mut self,
            _quote_for: StablesatsQuoteFor,
            _immediate_execution: bool,
        ) -> Result<StablesatsQuoteResponse, String> {
            self.buy_response
                .clone()
                .unwrap_or_else(|| Err("missing buy response".to_string()))
        }

        fn get_quote_to_sell_usd(
            &mut self,
            _quote_for: StablesatsQuoteFor,
            _immediate_execution: bool,
        ) -> Result<StablesatsQuoteResponse, String> {
            self.sell_response
                .clone()
                .unwrap_or_else(|| Err("missing sell response".to_string()))
        }

        fn accept_quote(&mut self, quote_id: &str) -> Result<(), String> {
            self.accepted_quote_ids.push(quote_id.to_string());
            Ok(())
        }
    }

    fn fallback_quote(direction: SwapDirection) -> SwapQuoteTerms {
        SwapQuoteTerms {
            quote_id: "blink-fallback-quote".to_string(),
            direction,
            amount_in: match direction {
                SwapDirection::BtcToUsd => SwapAmount {
                    amount: 5_000,
                    unit: SwapAmountUnit::Sats,
                },
                SwapDirection::UsdToBtc => SwapAmount {
                    amount: 500,
                    unit: SwapAmountUnit::Cents,
                },
            },
            amount_out: match direction {
                SwapDirection::BtcToUsd => SwapAmount {
                    amount: 330,
                    unit: SwapAmountUnit::Cents,
                },
                SwapDirection::UsdToBtc => SwapAmount {
                    amount: 7_500,
                    unit: SwapAmountUnit::Sats,
                },
            },
            expires_at_epoch_seconds: 2_000,
            immediate_execution: false,
            fee_sats: 0,
            fee_bps: 0,
            slippage_bps: 0,
        }
    }

    #[test]
    fn uses_stablesats_quote_when_available() {
        let request = SwapQuoteAdapterRequest {
            request_id: "req-stablesats".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount: SwapAmount {
                amount: 5_000,
                unit: SwapAmountUnit::Sats,
            },
            immediate_execution: false,
            now_epoch_seconds: 1_000,
        };
        let mut client = FakeStablesatsClient {
            buy_response: Some(Ok(StablesatsQuoteResponse {
                quote_id: "quote-stablesats-1".to_string(),
                amount_to_sell_in_sats: Some(5_000),
                amount_to_buy_in_cents: Some(330),
                amount_to_buy_in_sats: None,
                amount_to_sell_in_cents: None,
                expires_at_epoch_seconds: 1_200,
                executed: false,
            })),
            ..Default::default()
        };

        let outcome = request_quote_with_fallback(
            &mut client,
            &request,
            fallback_quote(SwapDirection::BtcToUsd),
        );
        assert_eq!(outcome.provider, SwapQuoteProvider::StablesatsQuoteService);
        assert_eq!(outcome.quote.quote_id, "quote-stablesats-1");
        assert!(outcome.fallback_reason.is_none());
        assert!(!outcome.accepted_via_adapter);
        assert!(client.accepted_quote_ids.is_empty());
    }

    #[test]
    fn immediate_execution_accepts_quote_when_not_auto_executed() {
        let request = SwapQuoteAdapterRequest {
            request_id: "req-accept".to_string(),
            direction: SwapDirection::UsdToBtc,
            amount: SwapAmount {
                amount: 500,
                unit: SwapAmountUnit::Cents,
            },
            immediate_execution: true,
            now_epoch_seconds: 1_000,
        };
        let mut client = FakeStablesatsClient {
            sell_response: Some(Ok(StablesatsQuoteResponse {
                quote_id: "quote-accept-1".to_string(),
                amount_to_sell_in_sats: None,
                amount_to_buy_in_cents: None,
                amount_to_buy_in_sats: Some(7_500),
                amount_to_sell_in_cents: Some(500),
                expires_at_epoch_seconds: 1_120,
                executed: false,
            })),
            ..Default::default()
        };

        let outcome = request_quote_with_fallback(
            &mut client,
            &request,
            fallback_quote(SwapDirection::UsdToBtc),
        );
        assert_eq!(outcome.provider, SwapQuoteProvider::StablesatsQuoteService);
        assert!(outcome.accepted_via_adapter);
        assert_eq!(
            client.accepted_quote_ids,
            vec!["quote-accept-1".to_string()]
        );
    }

    #[test]
    fn falls_back_when_stablesats_quote_path_fails() {
        let request = SwapQuoteAdapterRequest {
            request_id: "req-fallback".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount: SwapAmount {
                amount: 5_000,
                unit: SwapAmountUnit::Sats,
            },
            immediate_execution: false,
            now_epoch_seconds: 1_000,
        };
        let mut client = FakeStablesatsClient {
            buy_response: Some(Err("stablesats unavailable".to_string())),
            ..Default::default()
        };

        let outcome = request_quote_with_fallback(
            &mut client,
            &request,
            fallback_quote(SwapDirection::BtcToUsd),
        );
        assert_eq!(outcome.provider, SwapQuoteProvider::BlinkFallback);
        assert_eq!(outcome.quote.quote_id, "blink-fallback-quote");
        assert!(outcome.fallback_reason.is_some());
    }

    #[test]
    fn audit_receipt_preserves_quote_id_and_expiration_for_replay() {
        let request = SwapQuoteAdapterRequest {
            request_id: "req-audit".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount: SwapAmount {
                amount: 5_000,
                unit: SwapAmountUnit::Sats,
            },
            immediate_execution: false,
            now_epoch_seconds: 1_000,
        };
        let outcome = super::SwapQuoteAdapterOutcome {
            provider: SwapQuoteProvider::StablesatsQuoteService,
            quote: SwapQuoteTerms {
                quote_id: "quote-replay-1".to_string(),
                direction: SwapDirection::BtcToUsd,
                amount_in: SwapAmount {
                    amount: 5_000,
                    unit: SwapAmountUnit::Sats,
                },
                amount_out: SwapAmount {
                    amount: 330,
                    unit: SwapAmountUnit::Cents,
                },
                expires_at_epoch_seconds: 1_080,
                immediate_execution: false,
                fee_sats: 0,
                fee_bps: 0,
                slippage_bps: 0,
            },
            fallback_reason: None,
            accepted_via_adapter: false,
        };
        let audit = build_swap_quote_audit_receipt("goal-1", &request, &outcome);
        assert_eq!(audit.goal_id, "goal-1");
        assert_eq!(audit.quote_id, "quote-replay-1");
        assert_eq!(audit.expires_at_epoch_seconds, 1_080);
    }

    #[test]
    fn roundtrip_btc_to_usd_and_usd_to_btc_paths_accept_quotes_deterministically() {
        let mut client = FakeStablesatsClient {
            buy_response: Some(Ok(StablesatsQuoteResponse {
                quote_id: "quote-btc-usd-1".to_string(),
                amount_to_sell_in_sats: Some(5_000),
                amount_to_buy_in_cents: Some(330),
                amount_to_buy_in_sats: None,
                amount_to_sell_in_cents: None,
                expires_at_epoch_seconds: 1_120,
                executed: false,
            })),
            sell_response: Some(Ok(StablesatsQuoteResponse {
                quote_id: "quote-usd-btc-1".to_string(),
                amount_to_sell_in_sats: None,
                amount_to_buy_in_cents: None,
                amount_to_buy_in_sats: Some(7_400),
                amount_to_sell_in_cents: Some(500),
                expires_at_epoch_seconds: 1_140,
                executed: false,
            })),
            ..Default::default()
        };

        let btc_to_usd = request_quote_with_fallback(
            &mut client,
            &SwapQuoteAdapterRequest {
                request_id: "req-roundtrip-1".to_string(),
                direction: SwapDirection::BtcToUsd,
                amount: SwapAmount {
                    amount: 5_000,
                    unit: SwapAmountUnit::Sats,
                },
                immediate_execution: true,
                now_epoch_seconds: 1_000,
            },
            fallback_quote(SwapDirection::BtcToUsd),
        );
        assert_eq!(
            btc_to_usd.provider,
            SwapQuoteProvider::StablesatsQuoteService
        );
        assert_eq!(btc_to_usd.quote.direction, SwapDirection::BtcToUsd);
        assert_eq!(btc_to_usd.quote.amount_in.unit, SwapAmountUnit::Sats);
        assert_eq!(btc_to_usd.quote.amount_out.unit, SwapAmountUnit::Cents);
        assert!(btc_to_usd.accepted_via_adapter);

        let usd_to_btc = request_quote_with_fallback(
            &mut client,
            &SwapQuoteAdapterRequest {
                request_id: "req-roundtrip-2".to_string(),
                direction: SwapDirection::UsdToBtc,
                amount: SwapAmount {
                    amount: 500,
                    unit: SwapAmountUnit::Cents,
                },
                immediate_execution: true,
                now_epoch_seconds: 1_001,
            },
            fallback_quote(SwapDirection::UsdToBtc),
        );
        assert_eq!(
            usd_to_btc.provider,
            SwapQuoteProvider::StablesatsQuoteService
        );
        assert_eq!(usd_to_btc.quote.direction, SwapDirection::UsdToBtc);
        assert_eq!(usd_to_btc.quote.amount_in.unit, SwapAmountUnit::Cents);
        assert_eq!(usd_to_btc.quote.amount_out.unit, SwapAmountUnit::Sats);
        assert!(usd_to_btc.accepted_via_adapter);
        assert_eq!(
            client.accepted_quote_ids,
            vec!["quote-btc-usd-1".to_string(), "quote-usd-btc-1".to_string()]
        );
    }
}
