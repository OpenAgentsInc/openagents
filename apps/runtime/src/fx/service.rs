use std::collections::HashMap;

use chrono::Utc;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    config::HydraFxPolicyConfig,
    fx::types::{
        FX_QUOTE_UPSERT_RESPONSE_SCHEMA_V1, FX_RFQ_RESPONSE_SCHEMA_V1,
        FX_SELECT_RESPONSE_SCHEMA_V1, FX_SETTLE_RESPONSE_SCHEMA_V1,
        FX_SETTLEMENT_RECEIPT_SCHEMA_V1, FxDecisionReceiptLinkageV1, FxMoneyV1, FxQuoteStatusV1,
        FxQuoteUpsertRequestV1, FxQuoteUpsertResponseV1, FxQuoteV1, FxRfqRecordV1, FxRfqRequestV1,
        FxRfqResponseV1, FxSelectRequestV1, FxSelectResponseV1, FxSelectionFactorsV1,
        FxSettleRequestV1, FxSettleResponseV1, FxSettlementReceiptV1, FxSettlementStatusV1,
    },
    treasury::{SettlementStatus, Treasury, TreasuryError},
};

const FX_SELECTION_RECEIPT_SCHEMA_V1: &str = "openagents.hydra.fx_selection_receipt.v1";
const FX_SELECT_POLICY_REPUTATION_FIRST_V0: &str = "reputation_first_v0";

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

#[derive(Clone)]
struct FxQuoteIdempotencyRecord {
    request_sha256: String,
    quote_id: String,
}

#[derive(Clone)]
struct FxSelectIdempotencyRecord {
    request_sha256: String,
    response: FxSelectResponseV1,
}

#[derive(Clone)]
struct FxSettleIdempotencyRecord {
    request_sha256: String,
    response: FxSettleResponseV1,
}

#[derive(Clone, Serialize)]
struct ScoredQuote {
    quote: FxQuoteV1,
    all_in_cost_bps: u32,
    expiry_safety_seconds: u64,
    reliability_gap_bps: u32,
    weighted_score: u64,
}

#[derive(Default)]
struct FxState {
    rfqs_by_id: HashMap<String, FxRfqRecordV1>,
    rfq_idempotency: HashMap<String, FxRfqIdempotencyRecord>,
    quotes_by_id: HashMap<String, FxQuoteV1>,
    quotes_by_rfq: HashMap<String, Vec<String>>,
    quote_idempotency: HashMap<String, FxQuoteIdempotencyRecord>,
    select_idempotency: HashMap<String, FxSelectIdempotencyRecord>,
    selections_by_rfq: HashMap<String, FxSelectResponseV1>,
    settle_idempotency: HashMap<String, FxSettleIdempotencyRecord>,
    settlements_by_id: HashMap<String, FxSettleResponseV1>,
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
        self.validate_rfq_request(
            &request,
            normalized_sell_asset.as_str(),
            normalized_buy_asset.as_str(),
        )?;

        let idempotency_key = request.idempotency_key.trim();
        let request_sha256 = compute_rfq_request_sha256(
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

        let now_unix = now_unix();
        let expires_at_unix = now_unix.saturating_add(u64::from(request.quote_ttl_seconds));
        let rfq_id = format!("fxrfq_{}", Uuid::now_v7().simple());
        let rfq = FxRfqRecordV1 {
            rfq_id: rfq_id.clone(),
            requester_id: request.requester_id.trim().to_string(),
            budget_scope_id: request.budget_scope_id.trim().to_string(),
            sell: FxMoneyV1 {
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

    pub async fn upsert_quote(
        &self,
        request: FxQuoteUpsertRequestV1,
    ) -> Result<FxQuoteUpsertResponseV1, FxServiceError> {
        let request_sha256 = compute_quote_upsert_request_sha256(&request)?;
        let idempotency_key = request.idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "idempotency_key must not be empty".to_string(),
            ));
        }

        let mut state = self.state.lock().await;
        if let Some(existing) = state.quote_idempotency.get(idempotency_key) {
            if existing.request_sha256 != request_sha256 {
                return Err(FxServiceError::Conflict(
                    "idempotency key replay with different quote payload".to_string(),
                ));
            }
            let quote = state
                .quotes_by_id
                .get(existing.quote_id.as_str())
                .ok_or_else(|| {
                    FxServiceError::Internal(
                        "idempotency mapping points to missing FX quote".to_string(),
                    )
                })?;
            return Ok(FxQuoteUpsertResponseV1 {
                schema: FX_QUOTE_UPSERT_RESPONSE_SCHEMA_V1.to_string(),
                quote: quote.clone(),
            });
        }

        let rfq = state
            .rfqs_by_id
            .get(request.quote.rfq_id.trim())
            .ok_or_else(|| FxServiceError::NotFound("hydra fx rfq not found".to_string()))?
            .clone();
        let normalized_quote = self.normalize_quote_for_upsert(request.quote, &rfq)?;

        if let Some(existing) = state
            .quotes_by_id
            .get(normalized_quote.quote_id.as_str())
            .cloned()
        {
            if existing.quote_sha256 != normalized_quote.quote_sha256 {
                return Err(FxServiceError::Conflict(
                    "quote_id already exists with different immutable quote payload".to_string(),
                ));
            }
            state.quote_idempotency.insert(
                idempotency_key.to_string(),
                FxQuoteIdempotencyRecord {
                    request_sha256,
                    quote_id: existing.quote_id.clone(),
                },
            );
            return Ok(FxQuoteUpsertResponseV1 {
                schema: FX_QUOTE_UPSERT_RESPONSE_SCHEMA_V1.to_string(),
                quote: existing,
            });
        }

        let quote_id = normalized_quote.quote_id.clone();
        let rfq_quotes = state.quotes_by_rfq.entry(rfq.rfq_id.clone()).or_default();
        rfq_quotes.push(quote_id.clone());
        state
            .quotes_by_id
            .insert(quote_id.clone(), normalized_quote.clone());
        state.quote_idempotency.insert(
            idempotency_key.to_string(),
            FxQuoteIdempotencyRecord {
                request_sha256,
                quote_id,
            },
        );
        Ok(FxQuoteUpsertResponseV1 {
            schema: FX_QUOTE_UPSERT_RESPONSE_SCHEMA_V1.to_string(),
            quote: normalized_quote,
        })
    }

    pub async fn select_quote(
        &self,
        request: FxSelectRequestV1,
    ) -> Result<FxSelectResponseV1, FxServiceError> {
        let policy = normalize_select_policy(request.policy.as_str())?;
        let idempotency_key = request.idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "idempotency_key must not be empty".to_string(),
            ));
        }
        if request.rfq_id.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "rfq_id must not be empty".to_string(),
            ));
        }

        let request_sha256 = compute_select_request_sha256(request.rfq_id.trim(), policy.as_str())?;
        let mut state = self.state.lock().await;
        if let Some(existing) = state.select_idempotency.get(idempotency_key) {
            if existing.request_sha256 != request_sha256 {
                return Err(FxServiceError::Conflict(
                    "idempotency key replay with different select payload".to_string(),
                ));
            }
            return Ok(existing.response.clone());
        }

        let rfq = state
            .rfqs_by_id
            .get(request.rfq_id.trim())
            .ok_or_else(|| FxServiceError::NotFound("hydra fx rfq not found".to_string()))?
            .clone();
        let now = now_unix();
        let mut scored_quotes = self.collect_scored_quotes(&state, &rfq, now)?;
        if scored_quotes.is_empty() {
            return Err(FxServiceError::Conflict(
                "no eligible FX quotes available for selection".to_string(),
            ));
        }

        scored_quotes.sort_by(|left, right| {
            left.weighted_score
                .cmp(&right.weighted_score)
                .then_with(|| left.all_in_cost_bps.cmp(&right.all_in_cost_bps))
                .then_with(|| left.quote.latency_ms.cmp(&right.quote.latency_ms))
                .then_with(|| left.reliability_gap_bps.cmp(&right.reliability_gap_bps))
                .then_with(|| right.expiry_safety_seconds.cmp(&left.expiry_safety_seconds))
                .then_with(|| left.quote.provider_id.cmp(&right.quote.provider_id))
                .then_with(|| left.quote.quote_id.cmp(&right.quote.quote_id))
        });

        let selected = scored_quotes
            .first()
            .cloned()
            .ok_or_else(|| FxServiceError::Internal("missing selected FX quote".to_string()))?;
        let selected_quote = with_selection_metadata(&selected.quote, &selected, policy.as_str())?;
        let candidates: Vec<FxQuoteV1> = scored_quotes
            .iter()
            .map(|candidate| with_selection_metadata(&candidate.quote, candidate, policy.as_str()))
            .collect::<Result<Vec<_>, _>>()?;

        let confidence = compute_selection_confidence(
            selected.quote.reliability_bps,
            selected.all_in_cost_bps,
            selected.expiry_safety_seconds,
            rfq.quote_ttl_seconds,
        );
        let factors = FxSelectionFactorsV1 {
            expected_spread_bps: selected.quote.spread_bps,
            expected_fee_bps: selected.quote.fee_bps,
            confidence,
            policy_notes: vec![
                format!("policy={policy}"),
                format!("candidate_count={}", candidates.len()),
                format!("weights.cost_bps={}", self.policy.selection_weight_cost_bps),
                format!(
                    "weights.latency_bps={}",
                    self.policy.selection_weight_latency_bps
                ),
                format!(
                    "weights.reliability_bps={}",
                    self.policy.selection_weight_reliability_bps
                ),
                format!(
                    "min_quote_validity_seconds={}",
                    self.policy.min_quote_validity_seconds
                ),
                format!("selected.quote_id={}", selected.quote.quote_id),
                format!("selected.weighted_score={}", selected.weighted_score),
            ],
        };

        #[derive(Serialize)]
        struct SelectionDecisionHashInput<'a> {
            schema: &'a str,
            rfq_id: &'a str,
            policy: &'a str,
            selected_quote_id: &'a str,
            candidates: &'a [ScoredQuote],
            factors: &'a FxSelectionFactorsV1,
            decided_at_unix: u64,
        }

        let decided_at_unix = now;
        let decision_sha256 = protocol::hash::canonical_hash(&SelectionDecisionHashInput {
            schema: FX_SELECT_RESPONSE_SCHEMA_V1,
            rfq_id: rfq.rfq_id.as_str(),
            policy: policy.as_str(),
            selected_quote_id: selected.quote.quote_id.as_str(),
            candidates: &scored_quotes,
            factors: &factors,
            decided_at_unix,
        })
        .map_err(|error| FxServiceError::Internal(format!("selection hash failed: {error}")))?;

        #[derive(Serialize)]
        struct SelectionReceiptHashInput<'a> {
            schema: &'a str,
            decision_sha256: &'a str,
            rfq_id: &'a str,
            policy: &'a str,
            selected: &'a FxQuoteV1,
            factors: &'a FxSelectionFactorsV1,
            decided_at_unix: u64,
        }
        let receipt_sha256 = protocol::hash::canonical_hash(&SelectionReceiptHashInput {
            schema: FX_SELECTION_RECEIPT_SCHEMA_V1,
            decision_sha256: decision_sha256.as_str(),
            rfq_id: rfq.rfq_id.as_str(),
            policy: policy.as_str(),
            selected: &selected_quote,
            factors: &factors,
            decided_at_unix,
        })
        .map_err(|error| {
            FxServiceError::Internal(format!("selection receipt hash failed: {error}"))
        })?;
        let receipt = FxDecisionReceiptLinkageV1 {
            receipt_schema: FX_SELECTION_RECEIPT_SCHEMA_V1.to_string(),
            receipt_id: format!("hydrafxsel_{}", &receipt_sha256[..16]),
            canonical_json_sha256: receipt_sha256,
        };

        if let Some(chosen) = state.quotes_by_id.get_mut(selected.quote.quote_id.as_str()) {
            chosen.status = FxQuoteStatusV1::Selected;
        }

        let response = FxSelectResponseV1 {
            schema: FX_SELECT_RESPONSE_SCHEMA_V1.to_string(),
            rfq_id: rfq.rfq_id.clone(),
            policy,
            decision_sha256,
            selected: selected_quote,
            candidates,
            factors,
            receipt: Some(receipt),
            decided_at_unix,
        };
        state.select_idempotency.insert(
            idempotency_key.to_string(),
            FxSelectIdempotencyRecord {
                request_sha256,
                response: response.clone(),
            },
        );
        state
            .selections_by_rfq
            .insert(rfq.rfq_id.clone(), response.clone());
        Ok(response)
    }

    pub async fn settle_quote(
        &self,
        request: FxSettleRequestV1,
        treasury: &Treasury,
    ) -> Result<FxSettleResponseV1, FxServiceError> {
        if request.idempotency_key.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "idempotency_key must not be empty".to_string(),
            ));
        }
        if request.rfq_id.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "rfq_id must not be empty".to_string(),
            ));
        }
        if request.quote_id.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "quote_id must not be empty".to_string(),
            ));
        }
        if request.reservation_id.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "reservation_id must not be empty".to_string(),
            ));
        }
        if !request.policy_context.is_object() && !request.policy_context.is_null() {
            return Err(FxServiceError::InvalidRequest(
                "policy_context must be a JSON object".to_string(),
            ));
        }

        let normalized_policy_context = normalize_constraints(request.policy_context.clone());
        let request_sha256 = compute_settle_request_sha256(
            request.rfq_id.trim(),
            request.quote_id.trim(),
            request.reservation_id.trim(),
            &normalized_policy_context,
        )?;

        let idempotency_key = request.idempotency_key.trim().to_string();
        let (rfq, quote) = {
            let state = self.state.lock().await;
            if let Some(existing) = state.settle_idempotency.get(idempotency_key.as_str()) {
                if existing.request_sha256 != request_sha256 {
                    return Err(FxServiceError::Conflict(
                        "idempotency key replay with different settle payload".to_string(),
                    ));
                }
                return Ok(existing.response.clone());
            }

            let rfq = state
                .rfqs_by_id
                .get(request.rfq_id.trim())
                .ok_or_else(|| FxServiceError::NotFound("hydra fx rfq not found".to_string()))?
                .clone();
            let quote = state
                .quotes_by_id
                .get(request.quote_id.trim())
                .ok_or_else(|| FxServiceError::NotFound("hydra fx quote not found".to_string()))?
                .clone();
            if quote.rfq_id != rfq.rfq_id {
                return Err(FxServiceError::InvalidRequest(
                    "quote.rfq_id does not match settle.rfq_id".to_string(),
                ));
            }
            if !matches!(
                quote.status,
                FxQuoteStatusV1::Selected | FxQuoteStatusV1::Active
            ) {
                return Err(FxServiceError::Conflict(
                    "quote is not in a settle-eligible status".to_string(),
                ));
            }
            (rfq, quote)
        };

        let job_hash =
            compute_fx_settlement_job_hash(rfq.rfq_id.as_str(), quote.quote_id.as_str())?;
        let expected_reservation_id = reservation_id_from_job_hash(job_hash.as_str());
        if request.reservation_id.trim() != expected_reservation_id {
            return Err(FxServiceError::Conflict(format!(
                "reservation_id conflict: expected {expected_reservation_id}"
            )));
        }

        let (reservation, _created) = treasury
            .reserve_compute_job(
                rfq.requester_id.as_str(),
                job_hash.as_str(),
                quote.provider_id.as_str(),
                "hydra-fx",
                quote.buy.amount,
            )
            .await
            .map_err(map_treasury_error)?;
        if reservation.reservation_id != expected_reservation_id {
            return Err(FxServiceError::Conflict(
                "treasury reservation id mismatch".to_string(),
            ));
        }

        let now = now_unix();
        let quote_expired = quote.valid_until_unix <= now;
        let release_allowed_by_policy = normalized_policy_context
            .get("release_allowed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let verification_passed = !quote_expired;
        let release_allowed = !quote_expired && release_allowed_by_policy;
        let exit_code = if quote_expired { 124 } else { 0 };
        let (settled, _created) = treasury
            .settle_compute_job(
                job_hash.as_str(),
                verification_passed,
                exit_code,
                release_allowed,
            )
            .await
            .map_err(map_treasury_error)?;
        let status = match settled.status {
            SettlementStatus::Released => FxSettlementStatusV1::Released,
            SettlementStatus::Withheld => FxSettlementStatusV1::Withheld,
            SettlementStatus::Reserved => FxSettlementStatusV1::Failed,
        };

        #[derive(Serialize)]
        struct SettlementIdHashInput<'a> {
            rfq_id: &'a str,
            quote_id: &'a str,
            reservation_id: &'a str,
            policy_context: &'a serde_json::Value,
            settlement_status: &'a str,
        }
        let settlement_status = match status {
            FxSettlementStatusV1::Released => "released",
            FxSettlementStatusV1::Withheld => "withheld",
            FxSettlementStatusV1::Failed => "failed",
        };
        let settlement_id_sha = protocol::hash::canonical_hash(&SettlementIdHashInput {
            rfq_id: rfq.rfq_id.as_str(),
            quote_id: quote.quote_id.as_str(),
            reservation_id: reservation.reservation_id.as_str(),
            policy_context: &normalized_policy_context,
            settlement_status,
        })
        .map_err(|error| FxServiceError::Internal(format!("settlement id hash failed: {error}")))?;
        let settlement_id = format!("fxstl_{}", &settlement_id_sha[..16]);

        #[derive(Serialize)]
        struct ReceiptHashInput<'a> {
            schema: &'a str,
            settlement_id: &'a str,
            rfq_id: &'a str,
            quote_id: &'a str,
            provider_id: &'a str,
            sell: &'a FxMoneyV1,
            buy: &'a FxMoneyV1,
            spread_bps: u32,
            fee_bps: u32,
            settled_at_unix: u64,
            reservation_id: &'a str,
            treasury_job_hash: &'a str,
            policy_context: &'a serde_json::Value,
            settlement_status: &'a str,
            withheld_reason: &'a Option<String>,
        }

        let receipt_sha = protocol::hash::canonical_hash(&ReceiptHashInput {
            schema: FX_SETTLEMENT_RECEIPT_SCHEMA_V1,
            settlement_id: settlement_id.as_str(),
            rfq_id: rfq.rfq_id.as_str(),
            quote_id: quote.quote_id.as_str(),
            provider_id: quote.provider_id.as_str(),
            sell: &quote.sell,
            buy: &quote.buy,
            spread_bps: quote.spread_bps,
            fee_bps: quote.fee_bps,
            settled_at_unix: now,
            reservation_id: reservation.reservation_id.as_str(),
            treasury_job_hash: job_hash.as_str(),
            policy_context: &normalized_policy_context,
            settlement_status,
            withheld_reason: &settled.withheld_reason,
        })
        .map_err(|error| {
            FxServiceError::Internal(format!("settlement receipt hash failed: {error}"))
        })?;
        let receipt_id = format!("hydrafxsr_{}", &receipt_sha[..16]);
        let receipt = FxSettlementReceiptV1 {
            schema: FX_SETTLEMENT_RECEIPT_SCHEMA_V1.to_string(),
            receipt_id,
            settlement_id: settlement_id.clone(),
            rfq_id: rfq.rfq_id.clone(),
            quote_id: quote.quote_id.clone(),
            provider_id: quote.provider_id.clone(),
            sell: quote.sell.clone(),
            buy: quote.buy.clone(),
            spread_bps: quote.spread_bps,
            fee_bps: quote.fee_bps,
            settled_at_unix: now,
            wallet_receipt: None,
            canonical_json_sha256: receipt_sha,
        };
        let response = FxSettleResponseV1 {
            schema: FX_SETTLE_RESPONSE_SCHEMA_V1.to_string(),
            settlement_id,
            status,
            receipt,
        };

        let mut state = self.state.lock().await;
        if let Some(existing) = state.settle_idempotency.get(idempotency_key.as_str()) {
            if existing.request_sha256 != request_sha256 {
                return Err(FxServiceError::Conflict(
                    "idempotency key replay with different settle payload".to_string(),
                ));
            }
            return Ok(existing.response.clone());
        }
        state.settle_idempotency.insert(
            idempotency_key,
            FxSettleIdempotencyRecord {
                request_sha256,
                response: response.clone(),
            },
        );
        state
            .settlements_by_id
            .insert(response.settlement_id.clone(), response.clone());
        Ok(response)
    }

    fn validate_rfq_request(
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

    fn normalize_quote_for_upsert(
        &self,
        mut quote: FxQuoteV1,
        rfq: &FxRfqRecordV1,
    ) -> Result<FxQuoteV1, FxServiceError> {
        if quote.quote_id.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "quote.quote_id must not be empty".to_string(),
            ));
        }
        if quote.provider_id.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "quote.provider_id must not be empty".to_string(),
            ));
        }
        if quote.rfq_id.trim() != rfq.rfq_id {
            return Err(FxServiceError::InvalidRequest(
                "quote.rfq_id does not match RFQ".to_string(),
            ));
        }
        if quote.sell.amount == 0 || quote.buy.amount == 0 {
            return Err(FxServiceError::InvalidRequest(
                "quote.sell.amount and quote.buy.amount must be greater than zero".to_string(),
            ));
        }
        if quote.sell.unit.trim().is_empty() || quote.buy.unit.trim().is_empty() {
            return Err(FxServiceError::InvalidRequest(
                "quote units must not be empty".to_string(),
            ));
        }

        quote.sell.asset = normalize_asset(quote.sell.asset.as_str());
        quote.buy.asset = normalize_asset(quote.buy.asset.as_str());
        if quote.sell.asset != rfq.sell.asset {
            return Err(FxServiceError::InvalidRequest(
                "quote.sell.asset must match RFQ sell.asset".to_string(),
            ));
        }
        if quote.buy.asset != rfq.buy_asset {
            return Err(FxServiceError::InvalidRequest(
                "quote.buy.asset must match RFQ buy_asset".to_string(),
            ));
        }
        if quote.sell.unit.trim() != rfq.sell.unit.trim() {
            return Err(FxServiceError::InvalidRequest(
                "quote.sell.unit must match RFQ sell.unit".to_string(),
            ));
        }
        if quote.sell.amount != rfq.sell.amount {
            return Err(FxServiceError::InvalidRequest(
                "quote.sell.amount must match RFQ sell.amount".to_string(),
            ));
        }
        if quote.buy.amount < rfq.min_buy_amount {
            return Err(FxServiceError::PolicyDenied(
                "quote.buy.amount below RFQ minimum".to_string(),
            ));
        }
        if quote.spread_bps > rfq.max_spread_bps || quote.spread_bps > self.policy.max_spread_bps {
            return Err(FxServiceError::PolicyDenied(
                "quote.spread_bps exceeds policy or RFQ bounds".to_string(),
            ));
        }
        if quote.fee_bps > rfq.max_fee_bps || quote.fee_bps > self.policy.max_fee_bps {
            return Err(FxServiceError::PolicyDenied(
                "quote.fee_bps exceeds policy or RFQ bounds".to_string(),
            ));
        }
        if quote.latency_ms > rfq.max_latency_ms {
            return Err(FxServiceError::PolicyDenied(
                "quote.latency_ms exceeds RFQ bounds".to_string(),
            ));
        }
        if !quote.constraints.is_object() && !quote.constraints.is_null() {
            return Err(FxServiceError::InvalidRequest(
                "quote.constraints must be a JSON object".to_string(),
            ));
        }
        quote.constraints = normalize_constraints(quote.constraints);
        if !matches!(quote.status, FxQuoteStatusV1::Active) {
            return Err(FxServiceError::InvalidRequest(
                "quote.status must be active for upsert".to_string(),
            ));
        }
        let min_valid_until =
            now_unix().saturating_add(u64::from(self.policy.min_quote_validity_seconds));
        if quote.valid_until_unix <= min_valid_until {
            return Err(FxServiceError::PolicyDenied(
                "quote.valid_until_unix does not satisfy minimum validity window".to_string(),
            ));
        }
        if quote.valid_until_unix > rfq.expires_at_unix {
            quote.valid_until_unix = rfq.expires_at_unix;
        }
        let computed_sha = compute_quote_sha256(&quote)?;
        if !quote.quote_sha256.trim().is_empty() && quote.quote_sha256.trim() != computed_sha {
            return Err(FxServiceError::Conflict(
                "quote.quote_sha256 does not match canonical quote payload".to_string(),
            ));
        }
        quote.quote_sha256 = computed_sha;
        Ok(quote)
    }

    fn collect_scored_quotes(
        &self,
        state: &FxState,
        rfq: &FxRfqRecordV1,
        now_unix: u64,
    ) -> Result<Vec<ScoredQuote>, FxServiceError> {
        let quote_ids = state
            .quotes_by_rfq
            .get(rfq.rfq_id.as_str())
            .cloned()
            .unwrap_or_default();
        let mut scored = Vec::new();
        for quote_id in quote_ids {
            let Some(quote) = state.quotes_by_id.get(quote_id.as_str()) else {
                continue;
            };
            if !matches!(
                quote.status,
                FxQuoteStatusV1::Active | FxQuoteStatusV1::Selected
            ) {
                continue;
            }
            if quote.valid_until_unix <= now_unix {
                continue;
            }
            if quote.valid_until_unix.saturating_sub(now_unix)
                < u64::from(self.policy.min_quote_validity_seconds)
            {
                continue;
            }
            if quote.spread_bps > rfq.max_spread_bps || quote.fee_bps > rfq.max_fee_bps {
                continue;
            }
            if quote.latency_ms > rfq.max_latency_ms {
                continue;
            }
            if quote.buy.amount < rfq.min_buy_amount {
                continue;
            }

            let all_in_cost_bps = quote.spread_bps.saturating_add(quote.fee_bps);
            let reliability_gap_bps = 10_000u32.saturating_sub(quote.reliability_bps.min(10_000));
            let expiry_safety_seconds = quote.valid_until_unix.saturating_sub(now_unix);
            let weighted_score = self.compute_weighted_score(
                all_in_cost_bps,
                quote.latency_ms,
                reliability_gap_bps,
                expiry_safety_seconds,
                rfq.quote_ttl_seconds,
            );
            scored.push(ScoredQuote {
                quote: quote.clone(),
                all_in_cost_bps,
                expiry_safety_seconds,
                reliability_gap_bps,
                weighted_score,
            });
        }
        Ok(scored)
    }

    fn compute_weighted_score(
        &self,
        all_in_cost_bps: u32,
        latency_ms: u32,
        reliability_gap_bps: u32,
        expiry_safety_seconds: u64,
        rfq_ttl_seconds: u32,
    ) -> u64 {
        let cost_component = u64::from(all_in_cost_bps)
            .saturating_mul(u64::from(self.policy.selection_weight_cost_bps));
        let latency_component = u64::from(latency_ms)
            .saturating_mul(u64::from(self.policy.selection_weight_latency_bps));
        let reliability_component = u64::from(reliability_gap_bps)
            .saturating_mul(u64::from(self.policy.selection_weight_reliability_bps));
        let expiry_penalty = u64::from(rfq_ttl_seconds)
            .saturating_sub(expiry_safety_seconds.min(u64::from(rfq_ttl_seconds)));
        let expiry_component = expiry_penalty.saturating_mul(10);
        cost_component
            .saturating_add(latency_component)
            .saturating_add(reliability_component)
            .saturating_add(expiry_component)
    }
}

fn now_unix() -> u64 {
    u64::try_from(Utc::now().timestamp()).unwrap_or(0)
}

fn normalize_asset(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

fn normalize_pair(sell_asset: &str, buy_asset: &str) -> String {
    format!("{sell_asset}->{buy_asset}")
}

fn normalize_constraints(value: serde_json::Value) -> serde_json::Value {
    if value.is_null() {
        serde_json::json!({})
    } else {
        value
    }
}

fn normalize_select_policy(value: &str) -> Result<String, FxServiceError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == FX_SELECT_POLICY_REPUTATION_FIRST_V0 {
        return Ok(FX_SELECT_POLICY_REPUTATION_FIRST_V0.to_string());
    }
    Err(FxServiceError::InvalidRequest(format!(
        "unsupported FX select policy: {trimmed}"
    )))
}

fn with_selection_metadata(
    quote: &FxQuoteV1,
    scored: &ScoredQuote,
    policy: &str,
) -> Result<FxQuoteV1, FxServiceError> {
    let mut enriched = quote.clone();
    let mut constraints = match normalize_constraints(enriched.constraints.clone()) {
        serde_json::Value::Object(map) => map,
        _ => {
            return Err(FxServiceError::InvalidRequest(
                "quote.constraints must be a JSON object".to_string(),
            ));
        }
    };
    constraints.insert(
        "selection".to_string(),
        serde_json::json!({
            "policy": policy,
            "all_in_cost_bps": scored.all_in_cost_bps,
            "reliability_gap_bps": scored.reliability_gap_bps,
            "expiry_safety_seconds": scored.expiry_safety_seconds,
            "weighted_score": scored.weighted_score
        }),
    );
    enriched.constraints = serde_json::Value::Object(constraints);
    Ok(enriched)
}

fn compute_selection_confidence(
    reliability_bps: u32,
    all_in_cost_bps: u32,
    expiry_safety_seconds: u64,
    rfq_ttl_seconds: u32,
) -> f64 {
    let reliability_score = (f64::from(reliability_bps.min(10_000)) / 10_000.0).clamp(0.0, 1.0);
    let cost_score = (1.0 - (f64::from(all_in_cost_bps.min(10_000)) / 10_000.0)).clamp(0.0, 1.0);
    let expiry_score = if rfq_ttl_seconds == 0 {
        0.0
    } else {
        (expiry_safety_seconds as f64 / f64::from(rfq_ttl_seconds)).clamp(0.0, 1.0)
    };
    ((reliability_score * 0.55) + (cost_score * 0.30) + (expiry_score * 0.15)).clamp(0.0, 1.0)
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

fn compute_rfq_request_sha256(
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

#[derive(Serialize)]
struct QuoteFingerprintInput<'a> {
    quote_id: &'a str,
    rfq_id: &'a str,
    provider_id: &'a str,
    sell: &'a FxMoneyV1,
    buy: &'a FxMoneyV1,
    spread_bps: u32,
    fee_bps: u32,
    latency_ms: u32,
    reliability_bps: u32,
    valid_until_unix: u64,
    status: &'a FxQuoteStatusV1,
    constraints: &'a serde_json::Value,
}

fn compute_quote_sha256(quote: &FxQuoteV1) -> Result<String, FxServiceError> {
    protocol::hash::canonical_hash(&QuoteFingerprintInput {
        quote_id: quote.quote_id.as_str(),
        rfq_id: quote.rfq_id.as_str(),
        provider_id: quote.provider_id.as_str(),
        sell: &quote.sell,
        buy: &quote.buy,
        spread_bps: quote.spread_bps,
        fee_bps: quote.fee_bps,
        latency_ms: quote.latency_ms,
        reliability_bps: quote.reliability_bps,
        valid_until_unix: quote.valid_until_unix,
        status: &quote.status,
        constraints: &quote.constraints,
    })
    .map_err(|error| FxServiceError::Internal(format!("quote hash failed: {error}")))
}

#[derive(Serialize)]
struct QuoteUpsertRequestFingerprintInput<'a> {
    schema: &'a str,
    quote: &'a FxQuoteV1,
}

fn compute_quote_upsert_request_sha256(
    request: &FxQuoteUpsertRequestV1,
) -> Result<String, FxServiceError> {
    protocol::hash::canonical_hash(&QuoteUpsertRequestFingerprintInput {
        schema: request.schema.as_str(),
        quote: &request.quote,
    })
    .map_err(|error| FxServiceError::Internal(format!("quote upsert hash failed: {error}")))
}

#[derive(Serialize)]
struct SelectRequestFingerprintInput<'a> {
    rfq_id: &'a str,
    policy: &'a str,
}

fn compute_select_request_sha256(rfq_id: &str, policy: &str) -> Result<String, FxServiceError> {
    protocol::hash::canonical_hash(&SelectRequestFingerprintInput { rfq_id, policy })
        .map_err(|error| FxServiceError::Internal(format!("select hash failed: {error}")))
}

#[derive(Serialize)]
struct SettleRequestFingerprintInput<'a> {
    rfq_id: &'a str,
    quote_id: &'a str,
    reservation_id: &'a str,
    policy_context: &'a serde_json::Value,
}

fn compute_settle_request_sha256(
    rfq_id: &str,
    quote_id: &str,
    reservation_id: &str,
    policy_context: &serde_json::Value,
) -> Result<String, FxServiceError> {
    protocol::hash::canonical_hash(&SettleRequestFingerprintInput {
        rfq_id,
        quote_id,
        reservation_id,
        policy_context,
    })
    .map_err(|error| FxServiceError::Internal(format!("settle hash failed: {error}")))
}

#[derive(Serialize)]
struct FxSettlementJobHashInput<'a> {
    rfq_id: &'a str,
    quote_id: &'a str,
}

fn compute_fx_settlement_job_hash(rfq_id: &str, quote_id: &str) -> Result<String, FxServiceError> {
    let hash = protocol::hash::canonical_hash(&FxSettlementJobHashInput { rfq_id, quote_id })
        .map_err(|error| {
            FxServiceError::Internal(format!("settlement job hash failed: {error}"))
        })?;
    Ok(format!("fxjob_{}", &hash[..24]))
}

fn reservation_id_from_job_hash(job_hash: &str) -> String {
    let normalized = job_hash.trim();
    if normalized.len() >= 16 {
        format!("rsv_{}", &normalized[..16])
    } else if normalized.is_empty() {
        "rsv_invalid".to_string()
    } else {
        format!("rsv_{normalized}")
    }
}

fn map_treasury_error(error: TreasuryError) -> FxServiceError {
    match error {
        TreasuryError::OwnerMismatch => {
            FxServiceError::Conflict("treasury owner mismatch for reservation".to_string())
        }
        TreasuryError::AmountMismatch => {
            FxServiceError::Conflict("treasury amount mismatch for reservation".to_string())
        }
        TreasuryError::NotReserved => {
            FxServiceError::Conflict("treasury reservation missing".to_string())
        }
        TreasuryError::AlreadySettled => {
            FxServiceError::Conflict("treasury job already settled".to_string())
        }
        TreasuryError::SettlementConflict => {
            FxServiceError::Conflict("treasury settlement conflict".to_string())
        }
        TreasuryError::InsufficientBudget => {
            FxServiceError::PolicyDenied("insufficient treasury budget".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::fx::types::{
        FX_QUOTE_UPSERT_REQUEST_SCHEMA_V1, FX_RFQ_REQUEST_SCHEMA_V1, FX_SELECT_REQUEST_SCHEMA_V1,
    };

    fn sample_policy() -> HydraFxPolicyConfig {
        HydraFxPolicyConfig {
            allowed_pairs: ["USD->BTC_LN".to_string()].into_iter().collect(),
            max_spread_bps: 300,
            max_fee_bps: 100,
            min_quote_ttl_seconds: 5,
            max_quote_ttl_seconds: 60,
            min_quote_validity_seconds: 2,
            selection_weight_cost_bps: 4_000,
            selection_weight_latency_bps: 2_000,
            selection_weight_reliability_bps: 4_000,
        }
    }

    fn sample_rfq_request(idempotency_key: &str) -> FxRfqRequestV1 {
        FxRfqRequestV1 {
            schema: FX_RFQ_REQUEST_SCHEMA_V1.to_string(),
            idempotency_key: idempotency_key.to_string(),
            requester_id: "autopilot:user-1".to_string(),
            budget_scope_id: "budget:scope-1".to_string(),
            sell: FxMoneyV1 {
                asset: "usd".to_string(),
                amount: 100_000,
                unit: "cents".to_string(),
            },
            buy_asset: "btc_ln".to_string(),
            min_buy_amount: 2_000_000,
            max_spread_bps: 120,
            max_fee_bps: 60,
            max_latency_ms: 5_000,
            quote_ttl_seconds: 30,
            policy_context: json!({"policy":"balanced_v1"}),
        }
    }

    fn sample_quote(
        rfq_id: &str,
        quote_id: &str,
        provider_id: &str,
        spread_bps: u32,
        fee_bps: u32,
        latency_ms: u32,
        reliability_bps: u32,
        valid_until_unix: u64,
    ) -> FxQuoteV1 {
        FxQuoteV1 {
            quote_id: quote_id.to_string(),
            rfq_id: rfq_id.to_string(),
            provider_id: provider_id.to_string(),
            sell: FxMoneyV1 {
                asset: "USD".to_string(),
                amount: 100_000,
                unit: "cents".to_string(),
            },
            buy: FxMoneyV1 {
                asset: "BTC_LN".to_string(),
                amount: 2_500_000,
                unit: "msats".to_string(),
            },
            spread_bps,
            fee_bps,
            latency_ms,
            reliability_bps,
            valid_until_unix,
            status: FxQuoteStatusV1::Active,
            constraints: json!({}),
            quote_sha256: String::new(),
        }
    }

    #[tokio::test]
    async fn rfq_create_or_get_replays_same_idempotency_payload() {
        let service = FxService::new(sample_policy());
        let first = service
            .create_or_get_rfq(sample_rfq_request("idem-1"))
            .await
            .expect("first create");
        let second = service
            .create_or_get_rfq(sample_rfq_request("idem-1"))
            .await
            .expect("second replay");
        assert_eq!(first.rfq.rfq_id, second.rfq.rfq_id);
    }

    #[tokio::test]
    async fn rfq_create_or_get_conflicts_on_idempotency_drift() {
        let service = FxService::new(sample_policy());
        service
            .create_or_get_rfq(sample_rfq_request("idem-1"))
            .await
            .expect("first create");
        let mut drifted = sample_rfq_request("idem-1");
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
        let mut request = sample_rfq_request("idem-2");
        request.buy_asset = "EUR".to_string();
        let error = service
            .create_or_get_rfq(request)
            .await
            .expect_err("invalid pair should fail");
        assert!(matches!(error, FxServiceError::PolicyDenied(_)));
    }

    #[tokio::test]
    async fn selection_is_deterministic_for_fixed_quote_set() {
        let service = FxService::new(sample_policy());
        let rfq = service
            .create_or_get_rfq(sample_rfq_request("rfq-idem"))
            .await
            .expect("create rfq");
        let valid_until = now_unix().saturating_add(30);
        for (idempotency, quote) in [
            (
                "q-idem-1",
                sample_quote(
                    rfq.rfq.rfq_id.as_str(),
                    "quote-a",
                    "provider-a",
                    80,
                    30,
                    1200,
                    9_000,
                    valid_until,
                ),
            ),
            (
                "q-idem-2",
                sample_quote(
                    rfq.rfq.rfq_id.as_str(),
                    "quote-b",
                    "provider-b",
                    70,
                    20,
                    900,
                    8_700,
                    valid_until,
                ),
            ),
        ] {
            service
                .upsert_quote(FxQuoteUpsertRequestV1 {
                    schema: FX_QUOTE_UPSERT_REQUEST_SCHEMA_V1.to_string(),
                    idempotency_key: idempotency.to_string(),
                    quote,
                })
                .await
                .expect("upsert quote");
        }

        let first = service
            .select_quote(FxSelectRequestV1 {
                schema: FX_SELECT_REQUEST_SCHEMA_V1.to_string(),
                idempotency_key: "select-idem-1".to_string(),
                rfq_id: rfq.rfq.rfq_id.clone(),
                policy: FX_SELECT_POLICY_REPUTATION_FIRST_V0.to_string(),
            })
            .await
            .expect("select quote");
        let replay = service
            .select_quote(FxSelectRequestV1 {
                schema: FX_SELECT_REQUEST_SCHEMA_V1.to_string(),
                idempotency_key: "select-idem-1".to_string(),
                rfq_id: rfq.rfq.rfq_id.clone(),
                policy: FX_SELECT_POLICY_REPUTATION_FIRST_V0.to_string(),
            })
            .await
            .expect("select replay");
        assert_eq!(first.decision_sha256, replay.decision_sha256);
        assert_eq!(first.selected.quote_id, replay.selected.quote_id);
    }

    #[tokio::test]
    async fn selection_tie_break_is_stable() {
        let service = FxService::new(sample_policy());
        let rfq = service
            .create_or_get_rfq(sample_rfq_request("rfq-tie-idem"))
            .await
            .expect("create rfq");
        let valid_until = now_unix().saturating_add(30);
        for (idempotency, quote) in [
            (
                "tie-1",
                sample_quote(
                    rfq.rfq.rfq_id.as_str(),
                    "quote-z",
                    "provider-b",
                    60,
                    20,
                    1000,
                    9_000,
                    valid_until,
                ),
            ),
            (
                "tie-2",
                sample_quote(
                    rfq.rfq.rfq_id.as_str(),
                    "quote-a",
                    "provider-a",
                    60,
                    20,
                    1000,
                    9_000,
                    valid_until,
                ),
            ),
        ] {
            service
                .upsert_quote(FxQuoteUpsertRequestV1 {
                    schema: FX_QUOTE_UPSERT_REQUEST_SCHEMA_V1.to_string(),
                    idempotency_key: idempotency.to_string(),
                    quote,
                })
                .await
                .expect("upsert tie quote");
        }
        let selected = service
            .select_quote(FxSelectRequestV1 {
                schema: FX_SELECT_REQUEST_SCHEMA_V1.to_string(),
                idempotency_key: "tie-select-idem".to_string(),
                rfq_id: rfq.rfq.rfq_id.clone(),
                policy: FX_SELECT_POLICY_REPUTATION_FIRST_V0.to_string(),
            })
            .await
            .expect("select quote");
        assert_eq!(selected.selected.provider_id, "provider-a");
        assert_eq!(selected.selected.quote_id, "quote-a");
    }

    #[tokio::test]
    async fn selection_returns_no_quote_conflict_when_none_eligible() {
        let service = FxService::new(sample_policy());
        let rfq = service
            .create_or_get_rfq(sample_rfq_request("rfq-empty-idem"))
            .await
            .expect("create rfq");
        let error = service
            .select_quote(FxSelectRequestV1 {
                schema: FX_SELECT_REQUEST_SCHEMA_V1.to_string(),
                idempotency_key: "empty-select-idem".to_string(),
                rfq_id: rfq.rfq.rfq_id,
                policy: FX_SELECT_POLICY_REPUTATION_FIRST_V0.to_string(),
            })
            .await
            .expect_err("selection should fail without quotes");
        assert!(matches!(error, FxServiceError::Conflict(_)));
    }
}
