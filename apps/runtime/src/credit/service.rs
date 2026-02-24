use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use nostr::nip32::{Label, LabelEvent, LabelTarget};
use nostr::{Event, EventTemplate, finalize_event};
use openagents_l402::Bolt11;

use crate::artifacts::sign_receipt_sha256;
use crate::credit::store::{CreditReceiptInsertInput, CreditStore, CreditStoreError};
use crate::credit::types::{
    CREDIT_AGENT_EXPOSURE_RESPONSE_SCHEMA_V1, CREDIT_ENVELOPE_REQUEST_SCHEMA_V1,
    CREDIT_ENVELOPE_RESPONSE_SCHEMA_V1, CREDIT_HEALTH_RESPONSE_SCHEMA_V1,
    CREDIT_INTENT_REQUEST_SCHEMA_V1, CREDIT_INTENT_RESPONSE_SCHEMA_V1,
    CREDIT_OFFER_REQUEST_SCHEMA_V1, CREDIT_OFFER_RESPONSE_SCHEMA_V1,
    CREDIT_SETTLE_REQUEST_SCHEMA_V1, CREDIT_SETTLE_RESPONSE_SCHEMA_V1,
    CREDIT_UNDERWRITING_AUDIT_SCHEMA_V1, CreditAgentExposureResponseV1, CreditCircuitBreakersV1,
    CreditEnvelopeRequestV1, CreditEnvelopeResponseV1, CreditEnvelopeRow, CreditEnvelopeStatusV1,
    CreditHealthResponseV1, CreditIntentRequestV1, CreditIntentResponseV1, CreditIntentRow,
    CreditLiquidityPayEventRow, CreditOfferRequestV1, CreditOfferResponseV1, CreditOfferRow,
    CreditOfferStatusV1, CreditPolicySnapshotV1, CreditScopeTypeV1, CreditSettleRequestV1,
    CreditSettleResponseV1, CreditSettlementOutcomeV1, CreditSettlementRow,
    CreditUnderwritingAuditRow,
    DEFAULT_NOTICE_SCHEMA_V1, DefaultNoticeV1, ENVELOPE_ISSUE_RECEIPT_SCHEMA_V1,
    ENVELOPE_SETTLEMENT_RECEIPT_SCHEMA_V1, EnvelopeIssueReceiptV1, EnvelopeSettlementReceiptV1,
};
use crate::liquidity::types::{PayRequestV1, QuotePayRequestV1};
use crate::liquidity::{LiquidityError, LiquidityService};

#[derive(Debug, thiserror::Error)]
pub enum CreditError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("dependency unavailable: {0}")]
    DependencyUnavailable(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl CreditError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::NotFound => "not_found",
            Self::Conflict(_) => "conflict",
            Self::DependencyUnavailable(_) => "dependency_unavailable",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidRequest(message)
            | Self::Conflict(message)
            | Self::DependencyUnavailable(message)
            | Self::Internal(message) => message.clone(),
            Self::NotFound => "not found".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreditPolicyConfig {
    pub max_sats_per_envelope: u64,
    pub max_outstanding_envelopes_per_agent: u64,
    pub max_offer_ttl_seconds: u64,

    pub underwriting_history_days: i64,
    pub underwriting_base_sats: u64,
    pub underwriting_k: f64,
    pub underwriting_default_penalty_multiplier: f64,

    pub min_fee_bps: u32,
    pub max_fee_bps: u32,
    pub fee_risk_scaler: f64,

    pub health_window_seconds: i64,
    pub health_settlement_sample_limit: u32,
    pub health_ln_pay_sample_limit: u32,
    pub circuit_breaker_min_sample: u64,
    pub loss_rate_halt_threshold: f64,
    pub ln_failure_rate_halt_threshold: f64,
    pub ln_failure_large_settlement_cap_sats: u64,
}

impl Default for CreditPolicyConfig {
    fn default() -> Self {
        Self {
            max_sats_per_envelope: 100_000,
            max_outstanding_envelopes_per_agent: 3,
            max_offer_ttl_seconds: 60 * 60,

            underwriting_history_days: 30,
            underwriting_base_sats: 2_000,
            underwriting_k: 150.0,
            underwriting_default_penalty_multiplier: 2.0,

            min_fee_bps: 50,
            max_fee_bps: 2_000,
            fee_risk_scaler: 400.0,

            health_window_seconds: 6 * 60 * 60,
            health_settlement_sample_limit: 200,
            health_ln_pay_sample_limit: 200,
            circuit_breaker_min_sample: 5,
            loss_rate_halt_threshold: 0.50,
            ln_failure_rate_halt_threshold: 0.50,
            ln_failure_large_settlement_cap_sats: 5_000,
        }
    }
}

#[derive(Clone)]
pub struct CreditService {
    store: Arc<dyn CreditStore>,
    liquidity: Arc<LiquidityService>,
    receipt_signing_key: Option<[u8; 32]>,
    policy: CreditPolicyConfig,
}

impl CreditService {
    pub fn new(
        store: Arc<dyn CreditStore>,
        liquidity: Arc<LiquidityService>,
        receipt_signing_key: Option<[u8; 32]>,
    ) -> Self {
        Self::new_with_policy(
            store,
            liquidity,
            receipt_signing_key,
            CreditPolicyConfig::default(),
        )
    }

    pub fn new_with_policy(
        store: Arc<dyn CreditStore>,
        liquidity: Arc<LiquidityService>,
        receipt_signing_key: Option<[u8; 32]>,
        policy: CreditPolicyConfig,
    ) -> Self {
        Self {
            store,
            liquidity,
            receipt_signing_key,
            policy,
        }
    }

    pub async fn health(&self) -> Result<CreditHealthResponseV1, CreditError> {
        let now = Utc::now();
        self.compute_health(now).await
    }

    pub async fn agent_exposure(
        &self,
        agent_id: &str,
    ) -> Result<CreditAgentExposureResponseV1, CreditError> {
        let now = Utc::now();
        let agent_id = sanitize_pubkey(agent_id)?;
        let decision = self
            .compute_underwriting_decision(agent_id.as_str(), now)
            .await?;

        Ok(CreditAgentExposureResponseV1 {
            schema: CREDIT_AGENT_EXPOSURE_RESPONSE_SCHEMA_V1.to_string(),
            agent_id,
            open_envelope_count: decision.stats.open_envelope_count,
            open_exposure_sats: decision.stats.open_exposure_sats,
            settled_count_30d: decision.stats.settled_count_30d,
            success_volume_sats_30d: decision.stats.success_volume_sats_30d,
            pass_rate_30d: decision.stats.pass_rate_30d,
            loss_count_30d: decision.stats.loss_count_30d,
            underwriting_limit_sats: decision.limit_sats,
            underwriting_fee_bps: decision.fee_bps,
            requires_verifier: decision.requires_verifier,
            computed_at: now,
        })
    }

    pub async fn intent(
        &self,
        body: CreditIntentRequestV1,
    ) -> Result<CreditIntentResponseV1, CreditError> {
        if body.schema.trim() != CREDIT_INTENT_REQUEST_SCHEMA_V1 {
            return Err(CreditError::InvalidRequest(format!(
                "schema must be {CREDIT_INTENT_REQUEST_SCHEMA_V1}"
            )));
        }
        if body.scope_type != CreditScopeTypeV1::Nip90 {
            return Err(CreditError::InvalidRequest(
                "scope_type must be nip90".to_string(),
            ));
        }

        let idempotency_key = body.idempotency_key.trim().to_string();
        if idempotency_key.is_empty() {
            return Err(CreditError::InvalidRequest(
                "idempotency_key is required".to_string(),
            ));
        }
        let agent_id = sanitize_pubkey(&body.agent_id)?;
        let scope_id = body.scope_id.trim().to_string();
        if scope_id.is_empty() {
            return Err(CreditError::InvalidRequest(
                "scope_id is required".to_string(),
            ));
        }
        if body.max_sats == 0 {
            return Err(CreditError::InvalidRequest(
                "max_sats must be > 0".to_string(),
            ));
        }
        if body.max_sats > self.policy.max_sats_per_envelope {
            return Err(CreditError::InvalidRequest(format!(
                "max_sats exceeds max_sats_per_envelope ({})",
                self.policy.max_sats_per_envelope
            )));
        }

        let now = Utc::now();
        if body.exp <= now {
            return Err(CreditError::InvalidRequest(
                "exp must be in the future".to_string(),
            ));
        }
        let max_exp = now + Duration::seconds(self.policy.max_offer_ttl_seconds as i64);
        if body.exp > max_exp {
            return Err(CreditError::InvalidRequest(format!(
                "exp exceeds max_offer_ttl_seconds ({})",
                self.policy.max_offer_ttl_seconds
            )));
        }

        let policy_context_sha256 =
            canonical_sha256(&body.policy_context).map_err(CreditError::Internal)?;
        #[derive(Serialize)]
        struct IntentFingerprint<'a> {
            schema: &'a str,
            idempotency_key: &'a str,
            agent_id: &'a str,
            scope_type: &'a str,
            scope_id: &'a str,
            max_sats: u64,
            exp: &'a DateTime<Utc>,
            policy_context_sha256: &'a str,
        }

        let request_fingerprint_sha256 = canonical_sha256(&IntentFingerprint {
            schema: CREDIT_INTENT_REQUEST_SCHEMA_V1,
            idempotency_key: idempotency_key.as_str(),
            agent_id: agent_id.as_str(),
            scope_type: body.scope_type.as_str(),
            scope_id: scope_id.as_str(),
            max_sats: body.max_sats,
            exp: &body.exp,
            policy_context_sha256: policy_context_sha256.as_str(),
        })
        .map_err(CreditError::Internal)?;

        let intent_id = format!("cepi_{}", &sha256_hex(idempotency_key.as_bytes())[..24]);
        let intent = CreditIntentRow {
            intent_id,
            idempotency_key: idempotency_key.clone(),
            agent_id: agent_id.clone(),
            scope_type: body.scope_type.as_str().to_string(),
            scope_id: scope_id.clone(),
            max_sats: i64::try_from(body.max_sats)
                .map_err(|_| CreditError::InvalidRequest("max_sats too large".to_string()))?,
            exp: body.exp,
            created_at: now,
        };
        let raw_json = json!({
            "schema": CREDIT_INTENT_REQUEST_SCHEMA_V1,
            "idempotency_key": idempotency_key,
            "agent_id": agent_id,
            "scope_type": body.scope_type.as_str(),
            "scope_id": scope_id,
            "max_sats": body.max_sats,
            "exp": body.exp,
            "policy_context": body.policy_context,
            "policy_context_sha256": policy_context_sha256,
            "request_fingerprint_sha256": request_fingerprint_sha256,
        });

        let stored = self
            .store
            .create_or_get_intent(intent, request_fingerprint_sha256, raw_json)
            .await
            .map_err(map_store_error)?;

        Ok(CreditIntentResponseV1 {
            schema: CREDIT_INTENT_RESPONSE_SCHEMA_V1.to_string(),
            intent: stored,
        })
    }

    pub async fn offer(
        &self,
        body: CreditOfferRequestV1,
    ) -> Result<CreditOfferResponseV1, CreditError> {
        if body.schema.trim() != CREDIT_OFFER_REQUEST_SCHEMA_V1 {
            return Err(CreditError::InvalidRequest(format!(
                "schema must be {CREDIT_OFFER_REQUEST_SCHEMA_V1}"
            )));
        }
        if body.scope_type != CreditScopeTypeV1::Nip90 {
            return Err(CreditError::InvalidRequest(
                "scope_type must be nip90".to_string(),
            ));
        }

        let agent_id = sanitize_pubkey(&body.agent_id)?;
        let pool_id = sanitize_pubkey(&body.pool_id)?;
        let scope_id = body.scope_id.trim().to_string();
        if scope_id.is_empty() {
            return Err(CreditError::InvalidRequest(
                "scope_id is required".to_string(),
            ));
        }
        if body.max_sats == 0 {
            return Err(CreditError::InvalidRequest(
                "max_sats must be > 0".to_string(),
            ));
        }
        if body.max_sats > self.policy.max_sats_per_envelope {
            return Err(CreditError::InvalidRequest(format!(
                "max_sats exceeds max_sats_per_envelope ({})",
                self.policy.max_sats_per_envelope
            )));
        }

        let issued_at = Utc::now();
        if body.exp <= issued_at {
            return Err(CreditError::InvalidRequest(
                "exp must be in the future".to_string(),
            ));
        }

        let max_exp = issued_at + Duration::seconds(self.policy.max_offer_ttl_seconds as i64);
        if body.exp > max_exp {
            return Err(CreditError::InvalidRequest(format!(
                "exp exceeds max_offer_ttl_seconds ({})",
                self.policy.max_offer_ttl_seconds
            )));
        }

        let intent_id = body
            .intent_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(intent_id) = intent_id.as_deref() {
            let intent = self
                .store
                .get_intent(intent_id)
                .await
                .map_err(map_store_error)?
                .ok_or(CreditError::NotFound)?;
            if intent.agent_id != agent_id {
                return Err(CreditError::Conflict(
                    "intent mismatch: agent_id differs".to_string(),
                ));
            }
            if intent.scope_type != body.scope_type.as_str() || intent.scope_id != scope_id {
                return Err(CreditError::Conflict(
                    "intent mismatch: scope differs".to_string(),
                ));
            }
            let intent_max_sats = u64::try_from(intent.max_sats).unwrap_or(u64::MAX);
            if body.max_sats > intent_max_sats {
                return Err(CreditError::Conflict(
                    "intent mismatch: max_sats exceeds intent".to_string(),
                ));
            }
            if body.exp > intent.exp {
                return Err(CreditError::Conflict(
                    "intent mismatch: exp exceeds intent".to_string(),
                ));
            }
        }

        let decision = self
            .compute_underwriting_decision(agent_id.as_str(), issued_at)
            .await?;
        let underwriting_limit_sats = body.max_sats.min(decision.limit_sats).max(1);
        let underwriting_fee_bps = decision.fee_bps;
        let requires_verifier = decision.requires_verifier;

        #[derive(Serialize)]
        struct OfferFingerprint<'a> {
            schema: &'a str,
            intent_id: Option<&'a str>,
            agent_id: &'a str,
            pool_id: &'a str,
            scope_type: &'a str,
            scope_id: &'a str,
            max_sats: u64,
            fee_bps: u32,
            requires_verifier: bool,
            exp: &'a DateTime<Utc>,
        }

        let request_fingerprint_sha256 = canonical_sha256(&OfferFingerprint {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1,
            intent_id: intent_id.as_deref(),
            agent_id: agent_id.as_str(),
            pool_id: pool_id.as_str(),
            scope_type: body.scope_type.as_str(),
            scope_id: scope_id.as_str(),
            max_sats: body.max_sats,
            fee_bps: body.fee_bps,
            requires_verifier: body.requires_verifier,
            exp: &body.exp,
        })
        .map_err(CreditError::Internal)?;

        let offer_id = format!("cepo_{}", &request_fingerprint_sha256[..24]);

        let offer = CreditOfferRow {
            offer_id,
            agent_id,
            pool_id,
            scope_type: body.scope_type.as_str().to_string(),
            scope_id,
            max_sats: i64::try_from(underwriting_limit_sats)
                .map_err(|_| CreditError::InvalidRequest("max_sats too large".to_string()))?,
            fee_bps: i32::try_from(underwriting_fee_bps)
                .map_err(|_| CreditError::InvalidRequest("fee_bps too large".to_string()))?,
            requires_verifier,
            exp: body.exp,
            status: CreditOfferStatusV1::Offered.as_str().to_string(),
            issued_at,
        };

        let stored = self
            .store
            .create_or_get_offer(offer, request_fingerprint_sha256)
            .await
            .map_err(map_store_error)?;

        let audit_json = json!({
            "schema": CREDIT_UNDERWRITING_AUDIT_SCHEMA_V1,
            "offerId": stored.offer_id,
            "intentId": intent_id,
            "issuedAt": stored.issued_at,
            "inputs": decision.audit_inputs,
            "decision": {
                "limitSats": decision.limit_sats,
                "feeBps": decision.fee_bps,
                "requiresVerifier": decision.requires_verifier,
                "riskScore": decision.risk_score
            }
        });
        let audit_sha256 = canonical_sha256(&audit_json).map_err(CreditError::Internal)?;
        match self
            .store
            .put_underwriting_audit(CreditUnderwritingAuditRow {
                offer_id: stored.offer_id.clone(),
                canonical_json_sha256: audit_sha256,
                audit_json,
                created_at: stored.issued_at,
            })
            .await
        {
            Ok(()) => {}
            Err(CreditStoreError::Conflict(_)) => {}
            Err(error) => return Err(map_store_error(error)),
        }

        Ok(CreditOfferResponseV1 {
            schema: CREDIT_OFFER_RESPONSE_SCHEMA_V1.to_string(),
            offer: stored,
        })
    }

    pub async fn envelope(
        &self,
        body: CreditEnvelopeRequestV1,
    ) -> Result<CreditEnvelopeResponseV1, CreditError> {
        if body.schema.trim() != CREDIT_ENVELOPE_REQUEST_SCHEMA_V1 {
            return Err(CreditError::InvalidRequest(format!(
                "schema must be {CREDIT_ENVELOPE_REQUEST_SCHEMA_V1}"
            )));
        }

        let offer_id = body.offer_id.trim().to_string();
        if offer_id.is_empty() {
            return Err(CreditError::InvalidRequest(
                "offer_id is required".to_string(),
            ));
        }
        let provider_id = sanitize_pubkey(&body.provider_id)?;

        let offer = self
            .store
            .get_offer(offer_id.as_str())
            .await
            .map_err(map_store_error)?
            .ok_or(CreditError::NotFound)?;

        if offer.status != CreditOfferStatusV1::Offered.as_str() {
            return Err(CreditError::Conflict(
                "offer is not in offered status".to_string(),
            ));
        }
        let now = Utc::now();
        if offer.exp <= now {
            return Err(CreditError::InvalidRequest("offer expired".to_string()));
        }
        if !offer.requires_verifier {
            return Err(CreditError::InvalidRequest(
                "offer.requires_verifier must be true".to_string(),
            ));
        }
        if u64::try_from(offer.max_sats).unwrap_or(u64::MAX) > self.policy.max_sats_per_envelope {
            return Err(CreditError::InvalidRequest(
                "offer.max_sats exceeds max_sats_per_envelope".to_string(),
            ));
        }

        let health = self.compute_health(now).await?;
        if health.breakers.halt_new_envelopes {
            return Err(CreditError::DependencyUnavailable(
                "credit circuit breaker: halt_new_envelopes".to_string(),
            ));
        }

        let (open_count, _open_exposure_sats) = self
            .store
            .get_agent_open_envelope_stats(offer.agent_id.as_str(), now)
            .await
            .map_err(map_store_error)?;
        if open_count >= self.policy.max_outstanding_envelopes_per_agent {
            return Err(CreditError::Conflict(
                "max outstanding envelopes exceeded".to_string(),
            ));
        }

        #[derive(Serialize)]
        struct EnvelopeFingerprint<'a> {
            schema: &'a str,
            offer_id: &'a str,
            provider_id: &'a str,
        }

        let request_fingerprint_sha256 = canonical_sha256(&EnvelopeFingerprint {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1,
            offer_id: offer_id.as_str(),
            provider_id: provider_id.as_str(),
        })
        .map_err(CreditError::Internal)?;

        let envelope_id = format!("cepe_{}", &request_fingerprint_sha256[..24]);

        let issued_at = Utc::now();
        let envelope = CreditEnvelopeRow {
            envelope_id: envelope_id.clone(),
            offer_id: offer.offer_id.clone(),
            agent_id: offer.agent_id.clone(),
            pool_id: offer.pool_id.clone(),
            provider_id: provider_id.clone(),
            scope_type: offer.scope_type.clone(),
            scope_id: offer.scope_id.clone(),
            max_sats: offer.max_sats,
            fee_bps: offer.fee_bps,
            exp: offer.exp,
            status: CreditEnvelopeStatusV1::Accepted.as_str().to_string(),
            issued_at,
        };

        let stored = self
            .store
            .create_or_get_envelope(envelope, request_fingerprint_sha256)
            .await
            .map_err(map_store_error)?;

        // Mark offer accepted (best-effort; offer immutability isn't critical for settlement).
        let _ = self
            .store
            .update_offer_status(
                offer_id.as_str(),
                CreditOfferStatusV1::Accepted.as_str(),
                issued_at,
            )
            .await;

        let receipt = build_envelope_issue_receipt(&stored, self.receipt_signing_key.as_ref())?;
        store_receipt(
            self.store.as_ref(),
            "envelope",
            stored.envelope_id.as_str(),
            &receipt,
        )
        .await?;

        Ok(CreditEnvelopeResponseV1 {
            schema: CREDIT_ENVELOPE_RESPONSE_SCHEMA_V1.to_string(),
            envelope: stored,
            receipt,
        })
    }

    pub async fn settle(
        &self,
        body: CreditSettleRequestV1,
    ) -> Result<CreditSettleResponseV1, CreditError> {
        if body.schema.trim() != CREDIT_SETTLE_REQUEST_SCHEMA_V1 {
            return Err(CreditError::InvalidRequest(format!(
                "schema must be {CREDIT_SETTLE_REQUEST_SCHEMA_V1}"
            )));
        }

        let envelope_id = body.envelope_id.trim().to_string();
        if envelope_id.is_empty() {
            return Err(CreditError::InvalidRequest(
                "envelope_id is required".to_string(),
            ));
        }
        let verification_receipt_sha256 = body.verification_receipt_sha256.trim().to_string();
        if verification_receipt_sha256.is_empty() {
            return Err(CreditError::InvalidRequest(
                "verification_receipt_sha256 is required".to_string(),
            ));
        }

        let Some(envelope) = self
            .store
            .get_envelope(envelope_id.as_str())
            .await
            .map_err(map_store_error)?
        else {
            return Err(CreditError::NotFound);
        };

        if let Some(existing) = self
            .store
            .get_settlement_by_envelope(envelope_id.as_str())
            .await
            .map_err(map_store_error)?
        {
            let schema = if existing.outcome == CreditSettlementOutcomeV1::Success.as_str() {
                ENVELOPE_SETTLEMENT_RECEIPT_SCHEMA_V1
            } else {
                DEFAULT_NOTICE_SCHEMA_V1
            };
            let receipt_row = self
                .store
                .get_receipt_by_unique("settlement", existing.settlement_id.as_str(), schema)
                .await
                .map_err(map_store_error)?
                .ok_or_else(|| CreditError::Internal("missing stored receipt".to_string()))?;
            return Ok(CreditSettleResponseV1 {
                schema: CREDIT_SETTLE_RESPONSE_SCHEMA_V1.to_string(),
                envelope_id,
                outcome: existing.outcome,
                spent_sats: u64::try_from(existing.spent_sats).unwrap_or(0),
                fee_sats: u64::try_from(existing.fee_sats).unwrap_or(0),
                verification_receipt_sha256: existing.verification_receipt_sha256,
                liquidity_receipt_sha256: existing.liquidity_receipt_sha256,
                receipt: receipt_row.receipt_json,
            });
        }

        let now = Utc::now();
        if envelope.status != CreditEnvelopeStatusV1::Accepted.as_str() {
            return Err(CreditError::Conflict(
                "envelope is not in accepted status".to_string(),
            ));
        }

        let request_fingerprint_sha256 = canonical_sha256(&json!({
            "schema": CREDIT_SETTLE_REQUEST_SCHEMA_V1,
            "envelope_id": envelope.envelope_id,
            "verification_passed": body.verification_passed,
            "verification_receipt_sha256": verification_receipt_sha256,
            "provider_invoice_hash": sha256_hex(body.provider_invoice.as_bytes()),
            "provider_host": body.provider_host.trim().to_ascii_lowercase(),
            "max_fee_msats": body.max_fee_msats,
        }))
        .map_err(CreditError::Internal)?;

        let settlement_id = format!("ceps_{}", &request_fingerprint_sha256[..24]);

        if now > envelope.exp {
            let receipt = build_default_notice(
                &envelope,
                settlement_id.as_str(),
                "expired",
                0,
                Some(verification_receipt_sha256.as_str()),
                self.receipt_signing_key.as_ref(),
            )?;
            let (row, _created) = self
                .store
                .create_or_get_settlement(
                    CreditSettlementRow {
                        settlement_id: settlement_id.clone(),
                        envelope_id: envelope.envelope_id.clone(),
                        outcome: CreditSettlementOutcomeV1::Expired.as_str().to_string(),
                        spent_sats: 0,
                        fee_sats: 0,
                        verification_receipt_sha256: verification_receipt_sha256.clone(),
                        liquidity_receipt_sha256: None,
                        created_at: now,
                    },
                    request_fingerprint_sha256.clone(),
                )
                .await
                .map_err(map_store_error)?;
            self.store
                .update_envelope_status(
                    envelope.envelope_id.as_str(),
                    CreditEnvelopeStatusV1::Defaulted.as_str(),
                    now,
                )
                .await
                .map_err(map_store_error)?;
            store_receipt(
                self.store.as_ref(),
                "settlement",
                row.settlement_id.as_str(),
                &receipt,
            )
            .await?;
            return Ok(CreditSettleResponseV1 {
                schema: CREDIT_SETTLE_RESPONSE_SCHEMA_V1.to_string(),
                envelope_id,
                outcome: row.outcome,
                spent_sats: 0,
                fee_sats: 0,
                verification_receipt_sha256,
                liquidity_receipt_sha256: None,
                receipt: serde_json::to_value(receipt)
                    .map_err(|error| CreditError::Internal(error.to_string()))?,
            });
        }

        if !body.verification_passed {
            let receipt = build_default_notice(
                &envelope,
                settlement_id.as_str(),
                "verification_failed",
                0,
                Some(verification_receipt_sha256.as_str()),
                self.receipt_signing_key.as_ref(),
            )?;
            let (row, _created) = self
                .store
                .create_or_get_settlement(
                    CreditSettlementRow {
                        settlement_id: settlement_id.clone(),
                        envelope_id: envelope.envelope_id.clone(),
                        outcome: CreditSettlementOutcomeV1::Failed.as_str().to_string(),
                        spent_sats: 0,
                        fee_sats: 0,
                        verification_receipt_sha256: verification_receipt_sha256.clone(),
                        liquidity_receipt_sha256: None,
                        created_at: now,
                    },
                    request_fingerprint_sha256.clone(),
                )
                .await
                .map_err(map_store_error)?;
            self.store
                .update_envelope_status(
                    envelope.envelope_id.as_str(),
                    CreditEnvelopeStatusV1::Defaulted.as_str(),
                    now,
                )
                .await
                .map_err(map_store_error)?;
            store_receipt(
                self.store.as_ref(),
                "settlement",
                row.settlement_id.as_str(),
                &receipt,
            )
            .await?;
            return Ok(CreditSettleResponseV1 {
                schema: CREDIT_SETTLE_RESPONSE_SCHEMA_V1.to_string(),
                envelope_id,
                outcome: row.outcome,
                spent_sats: 0,
                fee_sats: 0,
                verification_receipt_sha256,
                liquidity_receipt_sha256: None,
                receipt: serde_json::to_value(receipt)
                    .map_err(|error| CreditError::Internal(error.to_string()))?,
            });
        }

        let invoice = body.provider_invoice.trim().to_string();
        if invoice.is_empty() {
            return Err(CreditError::InvalidRequest(
                "provider_invoice is required".to_string(),
            ));
        }
        let host = body.provider_host.trim().to_ascii_lowercase();
        if host.is_empty() {
            return Err(CreditError::InvalidRequest(
                "provider_host is required".to_string(),
            ));
        }
        let amount_msats = Bolt11::amount_msats(invoice.as_str()).ok_or_else(|| {
            CreditError::InvalidRequest("provider_invoice must be a bolt11 with amount".to_string())
        })?;

        let max_amount_msats = u64::try_from(envelope.max_sats)
            .map_err(|_| CreditError::Internal("envelope.max_sats invalid".to_string()))?
            .saturating_mul(1000);
        if amount_msats > max_amount_msats {
            return Err(CreditError::InvalidRequest(
                "invoice exceeds envelope max_sats".to_string(),
            ));
        }

        let spent_sats_preview = msats_to_sats_ceil(amount_msats);
        let health = self.compute_health(now).await?;
        if health.breakers.halt_large_settlements
            && spent_sats_preview > self.policy.ln_failure_large_settlement_cap_sats
        {
            return Err(CreditError::DependencyUnavailable(
                "credit circuit breaker: halt_large_settlements".to_string(),
            ));
        }

        // Pay provider invoice via liquidity service (issuer pays provider).
        let policy_context = json!({
            "schema": "openagents.credit.policy_context.v1",
            "envelope_id": envelope.envelope_id,
            "agent_id": envelope.agent_id,
            "pool_id": envelope.pool_id,
            "provider_id": envelope.provider_id,
            "scope_type": envelope.scope_type,
            "scope_id": envelope.scope_id,
            "caller_context": body.policy_context
        });

        let quote = self
            .liquidity
            .quote_pay(QuotePayRequestV1 {
                schema: crate::liquidity::types::QUOTE_PAY_REQUEST_SCHEMA_V1.to_string(),
                idempotency_key: format!("cep:quote:{}", &request_fingerprint_sha256[..24]),
                invoice: invoice.clone(),
                host: host.clone(),
                max_amount_msats,
                max_fee_msats: body.max_fee_msats,
                urgency: Some("normal".to_string()),
                policy_context,
            })
            .await
            .map_err(map_liquidity_error)?;

        let paid = self
            .liquidity
            .pay(PayRequestV1 {
                schema: crate::liquidity::types::PAY_REQUEST_SCHEMA_V1.to_string(),
                quote_id: quote.quote_id.clone(),
                run_id: None,
                trajectory_hash: None,
            })
            .await
            .map_err(map_liquidity_error)?;

        self.store
            .put_liquidity_pay_event(CreditLiquidityPayEventRow {
                quote_id: quote.quote_id.clone(),
                envelope_id: envelope.envelope_id.clone(),
                status: paid.status.clone(),
                error_code: paid.error_code.clone(),
                amount_msats: i64::try_from(amount_msats).unwrap_or(i64::MAX),
                host: host.clone(),
                created_at: now,
            })
            .await
            .map_err(map_store_error)?;

        if paid.status != "succeeded" {
            return Err(CreditError::DependencyUnavailable(format!(
                "liquidity pay failed: {} {:?}",
                paid.status, paid.error_code
            )));
        }

        let spent_sats = spent_sats_preview;
        let fee_bps = u64::try_from(envelope.fee_bps)
            .map_err(|_| CreditError::Internal("envelope.fee_bps invalid".to_string()))?;
        let fee_sats = compute_fee_sats(spent_sats, fee_bps);

        let label_event = maybe_build_label_event(
            self.receipt_signing_key.as_ref(),
            true,
            envelope.agent_id.as_str(),
            envelope.provider_id.as_str(),
            envelope.envelope_id.as_str(),
            envelope.scope_id.as_str(),
        );
        let (label_event_id, label_event_sha256, label_event_json) = match &label_event {
            Some(event) => {
                let sha = canonical_sha256(event).ok();
                (
                    Some(event.id.clone()),
                    sha,
                    serde_json::to_value(event).ok(),
                )
            }
            None => (None, None, None),
        };

        let settlement_receipt = build_settlement_receipt(
            &envelope,
            CreditSettlementOutcomeV1::Success.as_str(),
            spent_sats,
            fee_sats,
            verification_receipt_sha256.as_str(),
            paid.receipt.canonical_json_sha256.as_str(),
            label_event_id.as_deref(),
            label_event_sha256.as_deref(),
            label_event_json.clone(),
            now,
            self.receipt_signing_key.as_ref(),
        )?;

        let (row, _created) = self
            .store
            .create_or_get_settlement(
                CreditSettlementRow {
                    settlement_id: settlement_id.clone(),
                    envelope_id: envelope.envelope_id.clone(),
                    outcome: CreditSettlementOutcomeV1::Success.as_str().to_string(),
                    spent_sats: i64::try_from(spent_sats).unwrap_or(i64::MAX),
                    fee_sats: i64::try_from(fee_sats).unwrap_or(i64::MAX),
                    verification_receipt_sha256: verification_receipt_sha256.clone(),
                    liquidity_receipt_sha256: Some(paid.receipt.canonical_json_sha256.clone()),
                    created_at: now,
                },
                request_fingerprint_sha256,
            )
            .await
            .map_err(map_store_error)?;
        self.store
            .update_envelope_status(
                envelope.envelope_id.as_str(),
                CreditEnvelopeStatusV1::Settled.as_str(),
                now,
            )
            .await
            .map_err(map_store_error)?;
        store_receipt(
            self.store.as_ref(),
            "settlement",
            row.settlement_id.as_str(),
            &settlement_receipt,
        )
        .await?;

        Ok(CreditSettleResponseV1 {
            schema: CREDIT_SETTLE_RESPONSE_SCHEMA_V1.to_string(),
            envelope_id,
            outcome: row.outcome,
            spent_sats,
            fee_sats,
            verification_receipt_sha256,
            liquidity_receipt_sha256: row.liquidity_receipt_sha256.clone(),
            receipt: serde_json::to_value(settlement_receipt)
                .map_err(|error| CreditError::Internal(error.to_string()))?,
        })
    }

    async fn compute_underwriting_decision(
        &self,
        agent_id: &str,
        now: DateTime<Utc>,
    ) -> Result<UnderwritingDecision, CreditError> {
        let since = now - Duration::days(self.policy.underwriting_history_days.max(1));
        let settlements = self
            .store
            .list_recent_settlements_for_agent(
                agent_id,
                since,
                self.policy.health_settlement_sample_limit.max(200),
            )
            .await
            .map_err(map_store_error)?;

        let mut success_volume_sats_30d: u64 = 0;
        let mut success_count_30d: u64 = 0;
        let mut loss_count_30d: u64 = 0;
        let mut weighted_loss_score: f64 = 0.0;

        for row in &settlements {
            if row.outcome == CreditSettlementOutcomeV1::Success.as_str() {
                success_count_30d = success_count_30d.saturating_add(1);
                let spent = u64::try_from(row.spent_sats).unwrap_or(0);
                success_volume_sats_30d = success_volume_sats_30d.saturating_add(spent);
            } else {
                loss_count_30d = loss_count_30d.saturating_add(1);
                weighted_loss_score += loss_weight(now, row.created_at);
            }
        }

        let settled_count_30d: u64 = settlements.len() as u64;
        let pass_rate_30d = if settled_count_30d == 0 {
            1.0
        } else {
            (success_count_30d as f64) / (settled_count_30d as f64)
        };

        let (open_envelope_count, open_exposure_sats_i64) = self
            .store
            .get_agent_open_envelope_stats(agent_id, now)
            .await
            .map_err(map_store_error)?;
        let open_exposure_sats = u64::try_from(open_exposure_sats_i64).unwrap_or(0);

        let raw_limit = (self.policy.underwriting_base_sats as f64)
            + (self.policy.underwriting_k * (success_volume_sats_30d as f64).sqrt());
        let loss_penalty = 1.0
            / (1.0 + (weighted_loss_score * self.policy.underwriting_default_penalty_multiplier));
        let exposure_penalty = if raw_limit <= 1.0 {
            1.0
        } else {
            1.0 / (1.0 + ((open_exposure_sats as f64) / raw_limit.max(1.0)))
        };

        let limit_sats = (raw_limit * loss_penalty * exposure_penalty)
            .round()
            .clamp(1.0, self.policy.max_sats_per_envelope as f64) as u64;

        let risk_score = (1.0 - pass_rate_30d).max(0.0) * 2.0
            + (weighted_loss_score * 0.5)
            + ((open_exposure_sats as f64) / 50_000.0).min(50.0).sqrt();
        let fee_bps = (risk_score * self.policy.fee_risk_scaler).round().clamp(
            self.policy.min_fee_bps as f64,
            self.policy.max_fee_bps as f64,
        ) as u32;

        let stats = UnderwritingStats {
            settled_count_30d,
            success_volume_sats_30d,
            pass_rate_30d,
            loss_count_30d,
            open_envelope_count,
            open_exposure_sats,
            weighted_loss_score,
        };

        let audit_inputs = json!({
            "schema": "openagents.credit.underwriting_inputs.v1",
            "agentId": agent_id,
            "since": since,
            "settledCount30d": stats.settled_count_30d,
            "successVolumeSats30d": stats.success_volume_sats_30d,
            "passRate30d": stats.pass_rate_30d,
            "lossCount30d": stats.loss_count_30d,
            "weightedLossScore": stats.weighted_loss_score,
            "openEnvelopeCount": stats.open_envelope_count,
            "openExposureSats": stats.open_exposure_sats,
            "policy": {
                "baseSats": self.policy.underwriting_base_sats,
                "k": self.policy.underwriting_k,
                "defaultPenaltyMultiplier": self.policy.underwriting_default_penalty_multiplier,
                "maxSatsPerEnvelope": self.policy.max_sats_per_envelope
            }
        });

        Ok(UnderwritingDecision {
            limit_sats,
            fee_bps,
            requires_verifier: true,
            risk_score,
            stats,
            audit_inputs,
        })
    }

    async fn compute_health(
        &self,
        now: DateTime<Utc>,
    ) -> Result<CreditHealthResponseV1, CreditError> {
        let since = now - Duration::seconds(self.policy.health_window_seconds.max(60));
        let (open_envelope_count, open_reserved_commitments_sats_i64) = self
            .store
            .get_global_open_envelope_stats(now)
            .await
            .map_err(map_store_error)?;
        let open_reserved_commitments_sats =
            u64::try_from(open_reserved_commitments_sats_i64).unwrap_or(0);

        let settlements = self
            .store
            .list_recent_settlements(since, self.policy.health_settlement_sample_limit.max(50))
            .await
            .map_err(map_store_error)?;
        let settlement_sample = settlements.len() as u64;
        let loss_count = settlements
            .iter()
            .filter(|row| row.outcome != CreditSettlementOutcomeV1::Success.as_str())
            .count() as u64;
        let loss_rate = if settlement_sample == 0 {
            0.0
        } else {
            (loss_count as f64) / (settlement_sample as f64)
        };

        let ln_pay_events = self
            .store
            .list_recent_liquidity_pay_events(since, self.policy.health_ln_pay_sample_limit.max(50))
            .await
            .map_err(map_store_error)?;
        let ln_pay_sample = ln_pay_events.len() as u64;
        let ln_fail_count = ln_pay_events
            .iter()
            .filter(|row| row.status != "succeeded")
            .count() as u64;
        let ln_failure_rate = if ln_pay_sample == 0 {
            0.0
        } else {
            (ln_fail_count as f64) / (ln_pay_sample as f64)
        };

        let breakers = CreditCircuitBreakersV1 {
            halt_new_envelopes: settlement_sample >= self.policy.circuit_breaker_min_sample
                && loss_rate > self.policy.loss_rate_halt_threshold,
            halt_large_settlements: ln_pay_sample >= self.policy.circuit_breaker_min_sample
                && ln_failure_rate > self.policy.ln_failure_rate_halt_threshold,
        };

        Ok(CreditHealthResponseV1 {
            schema: CREDIT_HEALTH_RESPONSE_SCHEMA_V1.to_string(),
            generated_at: now,
            open_envelope_count,
            open_reserved_commitments_sats,
            settlement_sample,
            loss_count,
            loss_rate,
            ln_pay_sample,
            ln_fail_count,
            ln_failure_rate,
            breakers,
            policy: CreditPolicySnapshotV1 {
                max_sats_per_envelope: self.policy.max_sats_per_envelope,
                max_outstanding_envelopes_per_agent: self.policy.max_outstanding_envelopes_per_agent,
                max_offer_ttl_seconds: self.policy.max_offer_ttl_seconds,
                underwriting_history_days: self.policy.underwriting_history_days,
                underwriting_base_sats: self.policy.underwriting_base_sats,
                underwriting_k: self.policy.underwriting_k,
                underwriting_default_penalty_multiplier: self
                    .policy
                    .underwriting_default_penalty_multiplier,
                min_fee_bps: self.policy.min_fee_bps,
                max_fee_bps: self.policy.max_fee_bps,
                fee_risk_scaler: self.policy.fee_risk_scaler,
                health_window_seconds: self.policy.health_window_seconds,
                health_settlement_sample_limit: self.policy.health_settlement_sample_limit,
                health_ln_pay_sample_limit: self.policy.health_ln_pay_sample_limit,
                circuit_breaker_min_sample: self.policy.circuit_breaker_min_sample,
                loss_rate_halt_threshold: self.policy.loss_rate_halt_threshold,
                ln_failure_rate_halt_threshold: self.policy.ln_failure_rate_halt_threshold,
                ln_failure_large_settlement_cap_sats: self.policy.ln_failure_large_settlement_cap_sats,
            },
        })
    }
}

#[derive(Debug, Clone)]
struct UnderwritingStats {
    settled_count_30d: u64,
    success_volume_sats_30d: u64,
    pass_rate_30d: f64,
    loss_count_30d: u64,
    open_envelope_count: u64,
    open_exposure_sats: u64,
    weighted_loss_score: f64,
}

#[derive(Debug, Clone)]
struct UnderwritingDecision {
    limit_sats: u64,
    fee_bps: u32,
    requires_verifier: bool,
    risk_score: f64,
    stats: UnderwritingStats,
    audit_inputs: Value,
}

fn loss_weight(now: DateTime<Utc>, created_at: DateTime<Utc>) -> f64 {
    let age_seconds = (now - created_at).num_seconds().max(0);
    if age_seconds <= 3600 {
        1.0
    } else if age_seconds <= 86_400 {
        0.75
    } else if age_seconds <= 7 * 86_400 {
        0.50
    } else {
        0.25
    }
}

fn sanitize_pubkey(raw: &str) -> Result<String, CreditError> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.len() != 64 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(CreditError::InvalidRequest(
            "agent_id/pool_id/provider_id must be 64-char hex pubkeys".to_string(),
        ));
    }
    Ok(trimmed)
}

fn msats_to_sats_ceil(amount_msats: u64) -> u64 {
    amount_msats.saturating_add(999).saturating_div(1000).max(1)
}

fn compute_fee_sats(spent_sats: u64, fee_bps: u64) -> u64 {
    let numer = (spent_sats as u128).saturating_mul(fee_bps as u128);
    let fee = (numer + 9_999) / 10_000;
    u64::try_from(fee).unwrap_or(u64::MAX)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn canonical_sha256(value: &impl Serialize) -> Result<String, String> {
    let canonical_json =
        protocol::hash::canonical_json(value).map_err(|error| error.to_string())?;
    let digest = Sha256::digest(canonical_json.as_bytes());
    Ok(hex::encode(digest))
}

fn map_store_error(error: CreditStoreError) -> CreditError {
    match error {
        CreditStoreError::Conflict(message) => CreditError::Conflict(message),
        CreditStoreError::NotFound(_) => CreditError::NotFound,
        CreditStoreError::Db(message) => CreditError::Internal(message),
    }
}

fn map_liquidity_error(error: LiquidityError) -> CreditError {
    match error {
        LiquidityError::InvalidRequest(message) => CreditError::InvalidRequest(message),
        LiquidityError::NotFound => {
            CreditError::DependencyUnavailable("quote not found".to_string())
        }
        LiquidityError::Conflict(message) => CreditError::Conflict(message),
        LiquidityError::DependencyUnavailable(message) => {
            CreditError::DependencyUnavailable(message)
        }
        LiquidityError::Internal(message) => CreditError::Internal(message),
    }
}

fn build_envelope_issue_receipt(
    envelope: &CreditEnvelopeRow,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<EnvelopeIssueReceiptV1, CreditError> {
    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        offer_id: &'a str,
        envelope_id: &'a str,
        agent_id: &'a str,
        pool_id: &'a str,
        provider_id: &'a str,
        scope_type: &'a str,
        scope_id: &'a str,
        max_sats: i64,
        fee_bps: i32,
        exp: &'a DateTime<Utc>,
        issued_at: &'a DateTime<Utc>,
    }

    let canonical_json_sha256 = canonical_sha256(&ReceiptHashInput {
        schema: ENVELOPE_ISSUE_RECEIPT_SCHEMA_V1,
        offer_id: envelope.offer_id.as_str(),
        envelope_id: envelope.envelope_id.as_str(),
        agent_id: envelope.agent_id.as_str(),
        pool_id: envelope.pool_id.as_str(),
        provider_id: envelope.provider_id.as_str(),
        scope_type: envelope.scope_type.as_str(),
        scope_id: envelope.scope_id.as_str(),
        max_sats: envelope.max_sats,
        fee_bps: envelope.fee_bps,
        exp: &envelope.exp,
        issued_at: &envelope.issued_at,
    })
    .map_err(CreditError::Internal)?;
    let receipt_id = format!("ceir_{}", &canonical_json_sha256[..24]);
    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| CreditError::Internal(error.to_string()))?,
        ),
        None => None,
    };

    Ok(EnvelopeIssueReceiptV1 {
        schema: ENVELOPE_ISSUE_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id,
        offer_id: envelope.offer_id.clone(),
        envelope_id: envelope.envelope_id.clone(),
        agent_id: envelope.agent_id.clone(),
        pool_id: envelope.pool_id.clone(),
        provider_id: envelope.provider_id.clone(),
        scope_type: envelope.scope_type.clone(),
        scope_id: envelope.scope_id.clone(),
        max_sats: u64::try_from(envelope.max_sats)
            .map_err(|_| CreditError::Internal("max_sats invalid".to_string()))?,
        fee_bps: u32::try_from(envelope.fee_bps)
            .map_err(|_| CreditError::Internal("fee_bps invalid".to_string()))?,
        exp: envelope.exp,
        issued_at: envelope.issued_at,
        canonical_json_sha256,
        signature,
    })
}

fn build_settlement_receipt(
    envelope: &CreditEnvelopeRow,
    outcome: &str,
    spent_sats: u64,
    fee_sats: u64,
    verification_receipt_sha256: &str,
    liquidity_receipt_sha256: &str,
    label_event_id: Option<&str>,
    label_event_sha256: Option<&str>,
    label_event_json: Option<Value>,
    created_at: DateTime<Utc>,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<EnvelopeSettlementReceiptV1, CreditError> {
    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        envelope_id: &'a str,
        agent_id: &'a str,
        pool_id: &'a str,
        provider_id: &'a str,
        scope_type: &'a str,
        scope_id: &'a str,
        outcome: &'a str,
        spent_sats: u64,
        fee_sats: u64,
        verification_receipt_sha256: &'a str,
        liquidity_receipt_sha256: &'a str,
        label_event_id: Option<&'a str>,
        label_event_sha256: Option<&'a str>,
        created_at: &'a DateTime<Utc>,
    }

    let canonical_json_sha256 = canonical_sha256(&ReceiptHashInput {
        schema: ENVELOPE_SETTLEMENT_RECEIPT_SCHEMA_V1,
        envelope_id: envelope.envelope_id.as_str(),
        agent_id: envelope.agent_id.as_str(),
        pool_id: envelope.pool_id.as_str(),
        provider_id: envelope.provider_id.as_str(),
        scope_type: envelope.scope_type.as_str(),
        scope_id: envelope.scope_id.as_str(),
        outcome,
        spent_sats,
        fee_sats,
        verification_receipt_sha256,
        liquidity_receipt_sha256,
        label_event_id,
        label_event_sha256,
        created_at: &created_at,
    })
    .map_err(CreditError::Internal)?;
    let receipt_id = format!("cesr_{}", &canonical_json_sha256[..24]);
    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| CreditError::Internal(error.to_string()))?,
        ),
        None => None,
    };

    Ok(EnvelopeSettlementReceiptV1 {
        schema: ENVELOPE_SETTLEMENT_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id,
        envelope_id: envelope.envelope_id.clone(),
        agent_id: envelope.agent_id.clone(),
        pool_id: envelope.pool_id.clone(),
        provider_id: envelope.provider_id.clone(),
        scope_type: envelope.scope_type.clone(),
        scope_id: envelope.scope_id.clone(),
        outcome: outcome.to_string(),
        spent_sats,
        fee_sats,
        verification_receipt_sha256: verification_receipt_sha256.to_string(),
        liquidity_receipt_sha256: liquidity_receipt_sha256.to_string(),
        label_event_id: label_event_id.map(str::to_string),
        label_event_sha256: label_event_sha256.map(str::to_string),
        label_event: label_event_json,
        created_at,
        canonical_json_sha256,
        signature,
    })
}

fn build_default_notice(
    envelope: &CreditEnvelopeRow,
    settlement_id: &str,
    reason: &str,
    loss_sats: u64,
    verification_receipt_sha256: Option<&str>,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<DefaultNoticeV1, CreditError> {
    let label_event = maybe_build_label_event(
        receipt_signing_key,
        false,
        envelope.agent_id.as_str(),
        envelope.provider_id.as_str(),
        envelope.envelope_id.as_str(),
        envelope.scope_id.as_str(),
    );
    let (label_event_id, label_event_sha256, label_event_json) = match &label_event {
        Some(event) => {
            let sha = canonical_sha256(event).ok();
            (
                Some(event.id.clone()),
                sha,
                serde_json::to_value(event).ok(),
            )
        }
        None => (None, None, None),
    };

    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        settlement_id: &'a str,
        envelope_id: &'a str,
        agent_id: &'a str,
        pool_id: &'a str,
        provider_id: &'a str,
        scope_type: &'a str,
        scope_id: &'a str,
        reason: &'a str,
        loss_sats: u64,
        verification_receipt_sha256: Option<&'a str>,
        label_event_id: Option<&'a str>,
        label_event_sha256: Option<&'a str>,
        created_at: &'a DateTime<Utc>,
    }

    let created_at = Utc::now();
    let canonical_json_sha256 = canonical_sha256(&ReceiptHashInput {
        schema: DEFAULT_NOTICE_SCHEMA_V1,
        settlement_id,
        envelope_id: envelope.envelope_id.as_str(),
        agent_id: envelope.agent_id.as_str(),
        pool_id: envelope.pool_id.as_str(),
        provider_id: envelope.provider_id.as_str(),
        scope_type: envelope.scope_type.as_str(),
        scope_id: envelope.scope_id.as_str(),
        reason,
        loss_sats,
        verification_receipt_sha256,
        label_event_id: label_event_id.as_deref(),
        label_event_sha256: label_event_sha256.as_deref(),
        created_at: &created_at,
    })
    .map_err(CreditError::Internal)?;
    let receipt_id = format!("cedn_{}", &canonical_json_sha256[..24]);
    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| CreditError::Internal(error.to_string()))?,
        ),
        None => None,
    };

    Ok(DefaultNoticeV1 {
        schema: DEFAULT_NOTICE_SCHEMA_V1.to_string(),
        receipt_id,
        envelope_id: envelope.envelope_id.clone(),
        agent_id: envelope.agent_id.clone(),
        pool_id: envelope.pool_id.clone(),
        provider_id: envelope.provider_id.clone(),
        scope_type: envelope.scope_type.clone(),
        scope_id: envelope.scope_id.clone(),
        reason: reason.to_string(),
        loss_sats,
        verification_receipt_sha256: verification_receipt_sha256.map(str::to_string),
        label_event_id,
        label_event_sha256,
        label_event: label_event_json,
        created_at,
        canonical_json_sha256,
        signature,
    })
}

fn maybe_build_label_event(
    signing_key: Option<&[u8; 32]>,
    success: bool,
    agent_pubkey: &str,
    provider_pubkey: &str,
    envelope_id: &str,
    scope_id: &str,
) -> Option<Event> {
    let secret_key = signing_key?;

    let value = if success { "success" } else { "default" };
    let labels = vec![Label::new(value, "openagents.credit")];
    let targets = vec![
        LabelTarget::pubkey(agent_pubkey.to_string(), None::<String>),
        LabelTarget::pubkey(provider_pubkey.to_string(), None::<String>),
        LabelTarget::topic(format!("openagents.credit:scope:{scope_id}")),
    ];
    let label_event = LabelEvent::new(labels, targets)
        .with_content(format!("envelope={envelope_id} scope={scope_id}"));

    if label_event.validate().is_err() {
        return None;
    }

    let template = EventTemplate {
        created_at: Utc::now().timestamp().max(0) as u64,
        kind: nostr::nip32::KIND_LABEL as u16,
        tags: label_event.to_tags(),
        content: label_event.content,
    };

    finalize_event(&template, secret_key).ok()
}

async fn store_receipt<T: Serialize>(
    store: &dyn CreditStore,
    entity_kind: &str,
    entity_id: &str,
    receipt: &T,
) -> Result<(), CreditError> {
    let receipt_value =
        serde_json::to_value(receipt).map_err(|error| CreditError::Internal(error.to_string()))?;
    let canonical_json_sha256 = receipt_value
        .get("canonical_json_sha256")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let receipt_id = receipt_value
        .get("receipt_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let schema = receipt_value
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if canonical_json_sha256.is_empty() || receipt_id.is_empty() || schema.is_empty() {
        return Err(CreditError::Internal(
            "receipt missing receipt_id/schema/canonical_json_sha256".to_string(),
        ));
    }

    let signature_json = receipt_value
        .get("signature")
        .cloned()
        .filter(|value| !value.is_null());

    store
        .put_receipt(CreditReceiptInsertInput {
            receipt_id,
            entity_kind: entity_kind.to_string(),
            entity_id: entity_id.to_string(),
            schema,
            canonical_json_sha256,
            signature_json,
            receipt_json: receipt_value,
            created_at: Utc::now(),
        })
        .await
        .map_err(map_store_error)?;
    Ok(())
}
