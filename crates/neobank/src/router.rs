use std::sync::Arc;

use chrono::{Duration, Utc};
use openagents_l402::Bolt11;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::budgets::{BudgetError, BudgetFinalizeDisposition, BudgetHooks};
use crate::rails::runtime::{
    CreditEnvelopeRequestV1, CreditOfferRequestV1, CreditSettleRequestV1, LiquidityPayRequestV1,
    LiquidityQuotePayRequestV1, RuntimeClientError, RuntimeInternalApiClient,
};
use crate::receipts::{
    PaymentAttemptReceiptInput, PaymentAttemptReceiptV1, PaymentRouteKind, ReceiptSigner,
    build_payment_attempt_receipt, canonical_sha256,
};

#[derive(Debug, thiserror::Error)]
pub enum NeobankError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("budget error: {0}")]
    Budget(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("receipt error: {0}")]
    Receipt(String),
}

#[derive(Debug, Clone)]
pub struct CepPaymentContext {
    pub agent_id: String,
    pub pool_id: String,
    pub provider_id: String,
    pub scope_id: String,
    pub max_sats_cap: Option<u64>,
    pub offer_ttl_seconds: Option<i64>,
    pub verification_passed: bool,
    pub verification_receipt_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RoutePolicy {
    DirectOnly,
    ForceCep,
    PreferAgentBalance {
        agent_balance_sats: u64,
        min_reserve_sats: u64,
        direct_allowed: bool,
    },
}

impl Default for RoutePolicy {
    fn default() -> Self {
        Self::DirectOnly
    }
}

#[derive(Debug, Clone)]
pub struct QuoteAndPayBolt11Request {
    pub invoice: String,
    pub host: String,
    pub max_fee_msats: u64,
    pub urgency: Option<String>,
    pub policy_context: Value,
    pub run_id: Option<String>,
    pub trajectory_hash: Option<String>,
    pub idempotency_key: String,
    pub route_policy: RoutePolicy,
    pub cep: Option<CepPaymentContext>,
}

#[derive(Debug, Clone)]
pub struct QuoteAndPayBolt11Response {
    pub route_kind: PaymentRouteKind,
    pub status: String,
    pub budget_reservation_id: String,
    pub liquidity_quote_id: Option<String>,
    pub liquidity_receipt_sha256: Option<String>,
    pub credit_offer_id: Option<String>,
    pub credit_envelope_id: Option<String>,
    pub credit_settlement_receipt_sha256: Option<String>,
    pub receipt: PaymentAttemptReceiptV1,
}

pub struct TreasuryRouter {
    runtime: RuntimeInternalApiClient,
    budget_hooks: Arc<dyn BudgetHooks>,
    receipt_signer: Option<Arc<dyn ReceiptSigner>>,
}

impl TreasuryRouter {
    #[must_use]
    pub fn new(runtime: RuntimeInternalApiClient, budget_hooks: Arc<dyn BudgetHooks>) -> Self {
        Self {
            runtime,
            budget_hooks,
            receipt_signer: None,
        }
    }

    #[must_use]
    pub fn with_receipt_signer(mut self, signer: Arc<dyn ReceiptSigner>) -> Self {
        self.receipt_signer = Some(signer);
        self
    }

    pub async fn quote_and_pay_bolt11(
        &self,
        request: QuoteAndPayBolt11Request,
    ) -> Result<QuoteAndPayBolt11Response, NeobankError> {
        let sanitized = sanitize_request(request)?;
        let policy_context_sha256 = canonical_sha256(&sanitized.policy_context)
            .map_err(|e| NeobankError::Receipt(e.to_string()))?;

        let amount_msats = Bolt11::amount_msats(sanitized.invoice.as_str()).ok_or_else(|| {
            NeobankError::InvalidRequest(
                "invoice must include amount_msats (amountless not supported)".to_string(),
            )
        })?;
        let max_amount_forwarded_msats = amount_msats.saturating_add(sanitized.max_fee_msats);
        let reservation = self
            .budget_hooks
            .reserve(
                sanitized.idempotency_key.as_str(),
                max_amount_forwarded_msats,
                policy_context_sha256.as_str(),
            )
            .map_err(map_budget_error)?;

        let created_at = Utc::now();
        let direct_allowed =
            should_use_direct_path(max_amount_forwarded_msats, sanitized.route_policy.as_ref());

        let result = if direct_allowed {
            self.pay_direct(
                &sanitized,
                amount_msats,
                created_at,
                policy_context_sha256.as_str(),
                reservation.reservation_id.as_str(),
            )
            .await
        } else {
            self.pay_via_cep(
                &sanitized,
                amount_msats,
                created_at,
                policy_context_sha256.as_str(),
                reservation.reservation_id.as_str(),
            )
            .await
        };

        match result {
            Ok(response) => {
                self.budget_hooks
                    .finalize(
                        reservation.reservation_id.as_str(),
                        BudgetFinalizeDisposition::Commit,
                    )
                    .map_err(map_budget_error)?;
                Ok(response)
            }
            Err(error) => {
                let _ = self.budget_hooks.finalize(
                    reservation.reservation_id.as_str(),
                    BudgetFinalizeDisposition::Release,
                );
                Err(error)
            }
        }
    }

    async fn pay_direct(
        &self,
        request: &SanitizedRequest,
        amount_msats: u64,
        created_at: chrono::DateTime<Utc>,
        policy_context_sha256: &str,
        reservation_id: &str,
    ) -> Result<QuoteAndPayBolt11Response, NeobankError> {
        let quote = self
            .runtime
            .quote_pay(LiquidityQuotePayRequestV1 {
                schema: "openagents.liquidity.quote_pay_request.v1".to_string(),
                idempotency_key: format!("neobank:liq:{}", request.idempotency_key),
                invoice: request.invoice.clone(),
                host: request.host.clone(),
                max_amount_msats: amount_msats.saturating_add(request.max_fee_msats),
                max_fee_msats: request.max_fee_msats,
                urgency: request.urgency.clone(),
                policy_context: request.policy_context.clone(),
            })
            .await
            .map_err(map_runtime_error)?;

        let paid = self
            .runtime
            .pay(LiquidityPayRequestV1 {
                schema: "openagents.liquidity.pay_request.v1".to_string(),
                quote_id: quote.quote_id.clone(),
                run_id: request.run_id.clone(),
                trajectory_hash: request.trajectory_hash.clone(),
            })
            .await
            .map_err(map_runtime_error)?;

        let status = paid.status.clone();
        if status != "succeeded" {
            return Err(NeobankError::Runtime(format!(
                "liquidity pay failed: status={} code={:?} message={:?}",
                status, paid.error_code, paid.error_message
            )));
        }

        let receipt = build_payment_attempt_receipt(
            PaymentAttemptReceiptInput {
                idempotency_key: request.idempotency_key.clone(),
                route_kind: PaymentRouteKind::DirectLiquidity,
                status: status.clone(),
                invoice_hash: quote.invoice_hash.clone(),
                host: quote.host.clone(),
                quoted_amount_msats: quote.quoted_amount_msats,
                max_fee_msats: quote.max_fee_msats,
                policy_context_sha256: policy_context_sha256.to_string(),
                run_id: request.run_id.clone(),
                trajectory_hash: request.trajectory_hash.clone(),
                liquidity_quote_id: Some(quote.quote_id.clone()),
                liquidity_receipt_sha256: Some(paid.receipt.canonical_json_sha256.clone()),
                credit_offer_id: None,
                credit_envelope_id: None,
                credit_settlement_receipt_sha256: None,
                error_code: None,
                error_message: None,
                created_at,
            },
            self.receipt_signer.as_deref(),
        )
        .map_err(|e| NeobankError::Receipt(e.to_string()))?;

        Ok(QuoteAndPayBolt11Response {
            route_kind: PaymentRouteKind::DirectLiquidity,
            status,
            budget_reservation_id: reservation_id.to_string(),
            liquidity_quote_id: Some(quote.quote_id),
            liquidity_receipt_sha256: Some(paid.receipt.canonical_json_sha256),
            credit_offer_id: None,
            credit_envelope_id: None,
            credit_settlement_receipt_sha256: None,
            receipt,
        })
    }

    async fn pay_via_cep(
        &self,
        request: &SanitizedRequest,
        amount_msats: u64,
        created_at: chrono::DateTime<Utc>,
        policy_context_sha256: &str,
        reservation_id: &str,
    ) -> Result<QuoteAndPayBolt11Response, NeobankError> {
        let Some(cep) = request.cep.as_ref() else {
            return Err(NeobankError::InvalidRequest(
                "route policy selected CEP but cep context is missing".to_string(),
            ));
        };

        let max_sats_needed =
            msats_to_sats_ceil(amount_msats.saturating_add(request.max_fee_msats));
        if let Some(cap) = cep.max_sats_cap {
            if cap < max_sats_needed {
                return Err(NeobankError::InvalidRequest(format!(
                    "cep.max_sats_cap {cap} below required {max_sats_needed}"
                )));
            }
        }

        let offer_ttl_seconds = cep.offer_ttl_seconds.unwrap_or(900).clamp(60, 3600);
        let exp = Utc::now() + Duration::seconds(offer_ttl_seconds);

        let offer = self
            .runtime
            .credit_offer(CreditOfferRequestV1 {
                schema: "openagents.credit.offer_request.v1".to_string(),
                agent_id: cep.agent_id.clone(),
                pool_id: cep.pool_id.clone(),
                scope_type: "nip90".to_string(),
                scope_id: cep.scope_id.clone(),
                max_sats: max_sats_needed,
                fee_bps: 100,
                requires_verifier: true,
                exp,
            })
            .await
            .map_err(map_runtime_error)?;
        let offered_max_sats = u64::try_from(offer.offer.max_sats).unwrap_or(0);
        if offered_max_sats < max_sats_needed {
            return Err(NeobankError::Runtime(format!(
                "credit offer max_sats {offered_max_sats} below required {max_sats_needed}"
            )));
        }

        let envelope = self
            .runtime
            .credit_envelope(CreditEnvelopeRequestV1 {
                schema: "openagents.credit.envelope_request.v1".to_string(),
                offer_id: offer.offer.offer_id.clone(),
                provider_id: cep.provider_id.clone(),
            })
            .await
            .map_err(map_runtime_error)?;

        let verification_receipt_sha256 = cep
            .verification_receipt_sha256
            .clone()
            .unwrap_or_else(|| deterministic_verification_sha256(request.idempotency_key.as_str()));

        let settled = self
            .runtime
            .credit_settle(CreditSettleRequestV1 {
                schema: "openagents.credit.settle_request.v1".to_string(),
                envelope_id: envelope.envelope.envelope_id.clone(),
                verification_passed: cep.verification_passed,
                verification_receipt_sha256,
                provider_invoice: request.invoice.clone(),
                provider_host: request.host.clone(),
                max_fee_msats: request.max_fee_msats,
                policy_context: request.policy_context.clone(),
            })
            .await
            .map_err(map_runtime_error)?;

        if settled.outcome != "success" {
            return Err(NeobankError::Runtime(format!(
                "cep settle failed: outcome={} envelope_id={}",
                settled.outcome, settled.envelope_id
            )));
        }

        let liquidity_receipt_sha256 = settled
            .liquidity_receipt_sha256
            .clone()
            .or_else(|| extract_canonical_sha256(&settled.receipt))
            .ok_or_else(|| {
                NeobankError::Runtime("missing liquidity or settlement receipt sha256".to_string())
            })?;

        let receipt = build_payment_attempt_receipt(
            PaymentAttemptReceiptInput {
                idempotency_key: request.idempotency_key.clone(),
                route_kind: PaymentRouteKind::CepEnvelope,
                status: settled.outcome.clone(),
                invoice_hash: sha256_hex(request.invoice.as_bytes()),
                host: request.host.clone(),
                quoted_amount_msats: amount_msats,
                max_fee_msats: request.max_fee_msats,
                policy_context_sha256: policy_context_sha256.to_string(),
                run_id: request.run_id.clone(),
                trajectory_hash: request.trajectory_hash.clone(),
                liquidity_quote_id: None,
                liquidity_receipt_sha256: settled.liquidity_receipt_sha256.clone(),
                credit_offer_id: Some(offer.offer.offer_id.clone()),
                credit_envelope_id: Some(envelope.envelope.envelope_id.clone()),
                credit_settlement_receipt_sha256: Some(liquidity_receipt_sha256.clone()),
                error_code: None,
                error_message: None,
                created_at,
            },
            self.receipt_signer.as_deref(),
        )
        .map_err(|e| NeobankError::Receipt(e.to_string()))?;

        Ok(QuoteAndPayBolt11Response {
            route_kind: PaymentRouteKind::CepEnvelope,
            status: settled.outcome,
            budget_reservation_id: reservation_id.to_string(),
            liquidity_quote_id: None,
            liquidity_receipt_sha256: settled.liquidity_receipt_sha256,
            credit_offer_id: Some(offer.offer.offer_id),
            credit_envelope_id: Some(envelope.envelope.envelope_id),
            credit_settlement_receipt_sha256: Some(liquidity_receipt_sha256),
            receipt,
        })
    }
}

#[derive(Debug, Clone)]
struct SanitizedRequest {
    invoice: String,
    host: String,
    max_fee_msats: u64,
    urgency: Option<String>,
    policy_context: Value,
    run_id: Option<String>,
    trajectory_hash: Option<String>,
    idempotency_key: String,
    route_policy: Arc<RoutePolicy>,
    cep: Option<CepPaymentContext>,
}

fn sanitize_request(request: QuoteAndPayBolt11Request) -> Result<SanitizedRequest, NeobankError> {
    let invoice = request.invoice.trim().to_string();
    if invoice.is_empty() {
        return Err(NeobankError::InvalidRequest(
            "invoice is required".to_string(),
        ));
    }

    let host = request.host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return Err(NeobankError::InvalidRequest("host is required".to_string()));
    }

    let idempotency_key = request.idempotency_key.trim().to_string();
    if idempotency_key.is_empty() {
        return Err(NeobankError::InvalidRequest(
            "idempotency_key is required".to_string(),
        ));
    }

    if matches!(request.route_policy, RoutePolicy::ForceCep) && request.cep.is_none() {
        return Err(NeobankError::InvalidRequest(
            "route_policy=ForceCep requires cep context".to_string(),
        ));
    }

    Ok(SanitizedRequest {
        invoice,
        host,
        max_fee_msats: request.max_fee_msats,
        urgency: request
            .urgency
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        policy_context: request.policy_context,
        run_id: request
            .run_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        trajectory_hash: request
            .trajectory_hash
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        idempotency_key,
        route_policy: Arc::new(request.route_policy),
        cep: request.cep,
    })
}

fn should_use_direct_path(max_amount_msats: u64, route_policy: &RoutePolicy) -> bool {
    match route_policy {
        RoutePolicy::DirectOnly => true,
        RoutePolicy::ForceCep => false,
        RoutePolicy::PreferAgentBalance {
            agent_balance_sats,
            min_reserve_sats,
            direct_allowed,
        } => {
            if !direct_allowed {
                return false;
            }
            let required_sats = msats_to_sats_ceil(max_amount_msats);
            agent_balance_sats.saturating_sub(*min_reserve_sats) >= required_sats
        }
    }
}

fn map_runtime_error(error: RuntimeClientError) -> NeobankError {
    match error {
        RuntimeClientError::Transport(message) => NeobankError::Runtime(message),
        RuntimeClientError::Parse(message) => NeobankError::Runtime(message),
        RuntimeClientError::Api {
            status,
            code,
            message,
        } => NeobankError::Runtime(format!("status={status} code={code} message={message}")),
    }
}

fn map_budget_error(error: BudgetError) -> NeobankError {
    NeobankError::Budget(error.to_string())
}

fn deterministic_verification_sha256(idempotency_key: &str) -> String {
    let payload = format!("openagents.neobank.verification:{idempotency_key}");
    sha256_hex(payload.as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn msats_to_sats_ceil(msats: u64) -> u64 {
    if msats == 0 {
        return 0;
    }
    msats.saturating_add(999) / 1000
}

fn extract_canonical_sha256(receipt: &Value) -> Option<String> {
    receipt
        .get("canonical_json_sha256")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            receipt
                .get("canonicalJsonSha256")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}
