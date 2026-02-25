use std::collections::HashMap;

use chrono::Utc;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    config::HydraFxPolicyConfig,
    fx::types::{FX_RFQ_RESPONSE_SCHEMA_V1, FxRfqRecordV1, FxRfqRequestV1, FxRfqResponseV1},
};

#[derive(Debug, Error)]
pub enum FxServiceError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    PolicyDenied(String),
    #[error("{0}")]
    Internal(String),
}

#[derive(Clone)]
struct FxRfqIdempotencyRecord {
    request_sha256: String,
    rfq_id: String,
}

#[derive(Default)]
struct FxState {
    rfqs_by_id: HashMap<String, FxRfqRecordV1>,
    rfq_idempotency: HashMap<String, FxRfqIdempotencyRecord>,
}

pub struct FxService {
    policy: HydraFxPolicyConfig,
    state: Mutex<FxState>,
}

impl FxService {
    #[must_use]
    pub fn new(policy: HydraFxPolicyConfig) -> Self {
        Self {
            policy,
            state: Mutex::new(FxState::default()),
        }
    }

    pub async fn create_or_get_rfq(
        &self,
        request: FxRfqRequestV1,
    ) -> Result<FxRfqResponseV1, FxServiceError> {
        let normalized_sell_asset = normalize_asset(request.sell.asset.as_str());
        let normalized_buy_asset = normalize_asset(request.buy_asset.as_str());
        self.validate_request(
            &request,
            normalized_sell_asset.as_str(),
            normalized_buy_asset.as_str(),
        )?;

        let idempotency_key = request.idempotency_key.trim();
        let request_sha256 = compute_request_sha256(
            &request,
            normalized_sell_asset.as_str(),
            normalized_buy_asset.as_str(),
        )?;

        let mut state = self.state.lock().await;
        if let Some(existing) = state.rfq_idempotency.get(idempotency_key) {
            if existing.request_sha256 != request_sha256 {
                return Err(FxServiceError::Conflict(
                    "idempotency key replay with different request payload".to_string(),
                ));
            }
            let rfq = state
                .rfqs_by_id
                .get(existing.rfq_id.as_str())
                .ok_or_else(|| {
                    FxServiceError::Internal(
                        "idempotency mapping points to missing RFQ".to_string(),
                    )
                })?;
            return Ok(FxRfqResponseV1 {
                schema: FX_RFQ_RESPONSE_SCHEMA_V1.to_string(),
                rfq: rfq.clone(),
            });
        }

        let now_unix = u64::try_from(Utc::now().timestamp()).unwrap_or(0);
        let expires_at_unix = now_unix.saturating_add(u64::from(request.quote_ttl_seconds));
        let rfq_id = format!("fxrfq_{}", Uuid::now_v7().simple());
        let rfq = FxRfqRecordV1 {
            rfq_id: rfq_id.clone(),
            requester_id: request.requester_id.trim().to_string(),
            budget_scope_id: request.budget_scope_id.trim().to_string(),
            sell: crate::fx::types::FxMoneyV1 {
                asset: normalized_sell_asset,
                amount: request.sell.amount,
                unit: request.sell.unit.trim().to_string(),
            },
            buy_asset: normalized_buy_asset,
            min_buy_amount: request.min_buy_amount,
            max_spread_bps: request.max_spread_bps,
            max_fee_bps: request.max_fee_bps,
            max_latency_ms: request.max_latency_ms,
            quote_ttl_seconds: request.quote_ttl_seconds,
            status: "open".to_string(),
            created_at_unix: now_unix,
            expires_at_unix,
            policy_context: request.policy_context,
        };
        state.rfq_idempotency.insert(
            idempotency_key.to_string(),
            FxRfqIdempotencyRecord {
                request_sha256,
                rfq_id: rfq_id.clone(),
            },
        );
        state.rfqs_by_id.insert(rfq_id, rfq.clone());

        Ok(FxRfqResponseV1 {
            schema: FX_RFQ_RESPONSE_SCHEMA_V1.to_string(),
            rfq,
        })
    }

    pub async fn get_rfq(&self, rfq_id: &str) -> Result<FxRfqResponseV1, FxServiceError> {
        let rfq_id = rfq_id.trim();
        if rfq_id.is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "rfq_id must not be empty".to_string(),
            ));
        }
        let state = self.state.lock().await;
        let rfq = state
            .rfqs_by_id
            .get(rfq_id)
            .ok_or_else(|| FxServiceError::NotFound(format!("hydra fx rfq not found: {rfq_id}")))?;
        Ok(FxRfqResponseV1 {
            schema: FX_RFQ_RESPONSE_SCHEMA_V1.to_string(),
            rfq: rfq.clone(),
        })
    }

    fn validate_request(
        &self,
        request: &FxRfqRequestV1,
        normalized_sell_asset: &str,
        normalized_buy_asset: &str,
    ) -> Result<(), FxServiceError> {
        if request.schema.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "schema must not be empty".to_string(),
            ));
        }
        if request.idempotency_key.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "idempotency_key must not be empty".to_string(),
            ));
        }
        if request.requester_id.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "requester_id must not be empty".to_string(),
            ));
        }
        if request.budget_scope_id.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "budget_scope_id must not be empty".to_string(),
            ));
        }
        if normalized_sell_asset.is_empty() || normalized_buy_asset.is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "sell.asset and buy_asset must not be empty".to_string(),
            ));
        }
        if request.sell.amount == 0 || request.min_buy_amount == 0 {
            return Err(FxServiceError::InvalidRequest(
                "sell.amount and min_buy_amount must be greater than zero".to_string(),
            ));
        }
        if request.sell.unit.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "sell.unit must not be empty".to_string(),
            ));
        }
        if !request.policy_context.is_object() {
            return Err(FxServiceError::InvalidRequest(
                "policy_context must be a JSON object".to_string(),
            ));
        }
        let pair = normalize_pair(normalized_sell_asset, normalized_buy_asset);
        if !self.policy.allowed_pairs.contains(pair.as_str()) {
            return Err(FxServiceError::PolicyDenied(format!(
                "asset pair not allowed: {pair}"
            )));
        }
        if request.max_spread_bps > self.policy.max_spread_bps {
            return Err(FxServiceError::PolicyDenied(format!(
                "max_spread_bps exceeds policy max {}",
                self.policy.max_spread_bps
            )));
        }
        if request.max_fee_bps > self.policy.max_fee_bps {
            return Err(FxServiceError::PolicyDenied(format!(
                "max_fee_bps exceeds policy max {}",
                self.policy.max_fee_bps
            )));
        }
        if request.quote_ttl_seconds < self.policy.min_quote_ttl_seconds
            || request.quote_ttl_seconds > self.policy.max_quote_ttl_seconds
        {
            return Err(FxServiceError::InvalidRequest(format!(
                "quote_ttl_seconds must be within [{}..={}]",
                self.policy.min_quote_ttl_seconds, self.policy.max_quote_ttl_seconds
            )));
        }

        Ok(())
    }
}

fn normalize_asset(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

fn normalize_pair(sell_asset: &str, buy_asset: &str) -> String {
    format!("{sell_asset}->{buy_asset}")
}

#[derive(Serialize)]
struct RfqFingerprintInput<'a> {
    requester_id: &'a str,
    budget_scope_id: &'a str,
    sell_asset: &'a str,
    sell_amount: u64,
    sell_unit: &'a str,
    buy_asset: &'a str,
    min_buy_amount: u64,
    max_spread_bps: u32,
    max_fee_bps: u32,
    max_latency_ms: u32,
    quote_ttl_seconds: u32,
    policy_context: &'a serde_json::Value,
}

fn compute_request_sha256(
    request: &FxRfqRequestV1,
    normalized_sell_asset: &str,
    normalized_buy_asset: &str,
) -> Result<String, FxServiceError> {
    protocol::hash::canonical_hash(&RfqFingerprintInput {
        requester_id: request.requester_id.trim(),
        budget_scope_id: request.budget_scope_id.trim(),
        sell_asset: normalized_sell_asset,
        sell_amount: request.sell.amount,
        sell_unit: request.sell.unit.trim(),
        buy_asset: normalized_buy_asset,
        min_buy_amount: request.min_buy_amount,
        max_spread_bps: request.max_spread_bps,
        max_fee_bps: request.max_fee_bps,
        max_latency_ms: request.max_latency_ms,
        quote_ttl_seconds: request.quote_ttl_seconds,
        policy_context: &request.policy_context,
    })
    .map_err(|error| FxServiceError::Internal(format!("rfq hash failed: {error}")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample_policy() -> HydraFxPolicyConfig {
        HydraFxPolicyConfig {
            allowed_pairs: ["USD->BTC_LN".to_string()].into_iter().collect(),
            max_spread_bps: 300,
            max_fee_bps: 100,
            min_quote_ttl_seconds: 5,
            max_quote_ttl_seconds: 60,
        }
    }

    fn sample_request(idempotency_key: &str) -> FxRfqRequestV1 {
        FxRfqRequestV1 {
            schema: crate::fx::types::FX_RFQ_REQUEST_SCHEMA_V1.to_string(),
            idempotency_key: idempotency_key.to_string(),
            requester_id: "autopilot:user-1".to_string(),
            budget_scope_id: "budget:scope-1".to_string(),
            sell: crate::fx::types::FxMoneyV1 {
                asset: "usd".to_string(),
                amount: 100_000,
                unit: "cents".to_string(),
            },
            buy_asset: "btc_ln".to_string(),
            min_buy_amount: 2_000_000,
            max_spread_bps: 100,
            max_fee_bps: 50,
            max_latency_ms: 5_000,
            quote_ttl_seconds: 15,
            policy_context: json!({"policy":"balanced_v1"}),
        }
    }

    #[tokio::test]
    async fn rfq_create_or_get_replays_same_idempotency_payload() {
        let service = FxService::new(sample_policy());
        let first = service
            .create_or_get_rfq(sample_request("idem-1"))
            .await
            .expect("first create");
        let second = service
            .create_or_get_rfq(sample_request("idem-1"))
            .await
            .expect("second replay");
        assert_eq!(first.rfq.rfq_id, second.rfq.rfq_id);
    }

    #[tokio::test]
    async fn rfq_create_or_get_conflicts_on_idempotency_drift() {
        let service = FxService::new(sample_policy());
        service
            .create_or_get_rfq(sample_request("idem-1"))
            .await
            .expect("first create");
        let mut drifted = sample_request("idem-1");
        drifted.max_fee_bps = 90;
        let error = service
            .create_or_get_rfq(drifted)
            .await
            .expect_err("drift should conflict");
        assert!(matches!(error, FxServiceError::Conflict(_)));
    }

    #[tokio::test]
    async fn rfq_create_or_get_rejects_pair_not_allowed() {
        let service = FxService::new(sample_policy());
        let mut request = sample_request("idem-2");
        request.buy_asset = "EUR".to_string();
        let error = service
            .create_or_get_rfq(request)
            .await
            .expect_err("invalid pair should fail");
        assert!(matches!(error, FxServiceError::PolicyDenied(_)));
    }
}
