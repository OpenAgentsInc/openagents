use std::sync::Arc;

use chrono::{Duration, Utc};
use openagents_l402::Bolt11;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::budgets::{BudgetError, BudgetFinalizeDisposition, BudgetHooks};
use crate::rails::runtime::{
    CreditEnvelopeRequestV1, CreditOfferRequestV1, CreditScopeTypeV1, CreditSettleRequestV1,
    LiquidityPayRequestV1, LiquidityQuotePayRequestV1, RoutingCandidateQuoteV1,
    RoutingScoreRequestV1, RuntimeClientError, RuntimeInternalApiClient,
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

const HYDRA_ROUTING_SCORE_REQUEST_SCHEMA_V1: &str = "openagents.hydra.routing_score_request.v1";
const HYDRA_ROUTING_POLICY_BALANCED_V1: &str = "balanced_v1";
const HYDRA_ROUTE_PROVIDER_DIRECT: &str = "route-direct";
const HYDRA_ROUTE_PROVIDER_CEP: &str = "route-cep";
const HYDRA_ROUTING_CAPABILITY_PAY_BOLT11_V1: &str = "oa.liquidity.pay_bolt11.v1";

#[derive(Debug, Clone, Default)]
struct RoutingDecisionContext {
    decision_sha256: Option<String>,
    policy_notes: Vec<String>,
    confidence: Option<f64>,
    liquidity_score: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedRouteKind {
    Direct,
    Cep,
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
        let (resolved_route, routing_context) = self
            .resolve_route_kind(&sanitized, amount_msats, max_amount_forwarded_msats)
            .await;

        let result = match resolved_route {
            ResolvedRouteKind::Direct => {
                self.pay_direct(
                    &sanitized,
                    amount_msats,
                    created_at,
                    policy_context_sha256.as_str(),
                    reservation.reservation_id.as_str(),
                    &routing_context,
                )
                .await
            }
            ResolvedRouteKind::Cep => {
                self.pay_via_cep(
                    &sanitized,
                    amount_msats,
                    created_at,
                    policy_context_sha256.as_str(),
                    reservation.reservation_id.as_str(),
                    &routing_context,
                )
                .await
            }
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

    async fn resolve_route_kind(
        &self,
        request: &SanitizedRequest,
        amount_msats: u64,
        max_amount_forwarded_msats: u64,
    ) -> (ResolvedRouteKind, RoutingDecisionContext) {
        match request.route_policy.as_ref() {
            RoutePolicy::DirectOnly => {
                return (ResolvedRouteKind::Direct, RoutingDecisionContext::default());
            }
            RoutePolicy::ForceCep => {
                if request.cep.is_some() {
                    return (ResolvedRouteKind::Cep, RoutingDecisionContext::default());
                }
                return (ResolvedRouteKind::Direct, RoutingDecisionContext::default());
            }
            RoutePolicy::PreferAgentBalance { .. } => {}
        }

        let fallback_direct =
            should_use_direct_path(max_amount_forwarded_msats, request.route_policy.as_ref());
        let fallback = if fallback_direct || request.cep.is_none() {
            ResolvedRouteKind::Direct
        } else {
            ResolvedRouteKind::Cep
        };

        let Some(run_id) = request.run_id.as_ref() else {
            return (fallback, RoutingDecisionContext::default());
        };

        let (agent_balance_sats, min_reserve_sats, direct_allowed) =
            match request.route_policy.as_ref() {
                RoutePolicy::PreferAgentBalance {
                    agent_balance_sats,
                    min_reserve_sats,
                    direct_allowed,
                } => (*agent_balance_sats, *min_reserve_sats, *direct_allowed),
                _ => (0, 0, true),
            };
        let required_sats = msats_to_sats_ceil(max_amount_forwarded_msats);
        let has_direct_headroom =
            agent_balance_sats.saturating_sub(min_reserve_sats) >= required_sats;
        let direct_reliability_bps = if direct_allowed && has_direct_headroom {
            9_600
        } else if direct_allowed {
            7_200
        } else {
            2_000
        };

        let mut candidates = vec![RoutingCandidateQuoteV1 {
            marketplace_id: "openagents".to_string(),
            provider_id: HYDRA_ROUTE_PROVIDER_DIRECT.to_string(),
            provider_worker_id: None,
            total_price_msats: max_amount_forwarded_msats,
            latency_ms: Some(120),
            reliability_bps: direct_reliability_bps,
            constraints: serde_json::json!({
                "routeKind": "direct_liquidity",
                "directAllowed": direct_allowed,
                "hasHeadroom": has_direct_headroom
            }),
            quote_id: None,
            quote_sha256: None,
        }];

        if request.cep.is_some() {
            let cep_premium_msats = amount_msats.saturating_div(100);
            let cep_reliability_bps = if has_direct_headroom { 9_000 } else { 9_700 };
            candidates.push(RoutingCandidateQuoteV1 {
                marketplace_id: "openagents".to_string(),
                provider_id: HYDRA_ROUTE_PROVIDER_CEP.to_string(),
                provider_worker_id: None,
                total_price_msats: max_amount_forwarded_msats.saturating_add(cep_premium_msats),
                latency_ms: Some(220),
                reliability_bps: cep_reliability_bps,
                constraints: serde_json::json!({
                    "routeKind": "cep_envelope",
                    "providerId": request.cep.as_ref().map(|value| value.provider_id.clone())
                }),
                quote_id: None,
                quote_sha256: None,
            });
        }

        if candidates.len() < 2 {
            return (fallback, RoutingDecisionContext::default());
        }

        let routing_response = self
            .runtime
            .hydra_routing_score(RoutingScoreRequestV1 {
                schema: HYDRA_ROUTING_SCORE_REQUEST_SCHEMA_V1.to_string(),
                idempotency_key: format!("{}:routing", request.idempotency_key),
                run_id: run_id.clone(),
                marketplace_id: "openagents".to_string(),
                capability: HYDRA_ROUTING_CAPABILITY_PAY_BOLT11_V1.to_string(),
                policy: HYDRA_ROUTING_POLICY_BALANCED_V1.to_string(),
                objective_hash: request.trajectory_hash.clone(),
                decided_at_unix: Utc::now().timestamp().max(0) as u64,
                candidates,
            })
            .await;

        let response = match routing_response {
            Ok(response) => response,
            Err(_) => return (fallback, RoutingDecisionContext::default()),
        };

        let context = RoutingDecisionContext {
            decision_sha256: Some(response.decision_sha256.clone()),
            policy_notes: response.factors.policy_notes.clone(),
            confidence: Some(response.factors.confidence),
            liquidity_score: Some(response.factors.liquidity_score),
        };

        if response.selected.provider_id == HYDRA_ROUTE_PROVIDER_CEP {
            if request.cep.is_some() {
                return (ResolvedRouteKind::Cep, context);
            }
            return (ResolvedRouteKind::Direct, context);
        }
        if response.selected.provider_id == HYDRA_ROUTE_PROVIDER_DIRECT {
            return (ResolvedRouteKind::Direct, context);
        }

        (fallback, context)
    }

    async fn pay_direct(
        &self,
        request: &SanitizedRequest,
        amount_msats: u64,
        created_at: chrono::DateTime<Utc>,
        policy_context_sha256: &str,
        reservation_id: &str,
        routing_context: &RoutingDecisionContext,
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
                routing_decision_sha256: routing_context.decision_sha256.clone(),
                routing_policy_notes: routing_context.policy_notes.clone(),
                routing_confidence: routing_context.confidence,
                routing_liquidity_score: routing_context.liquidity_score,
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
        routing_context: &RoutingDecisionContext,
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
                intent_id: None,
                scope_type: CreditScopeTypeV1::Nip90,
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
                routing_decision_sha256: routing_context.decision_sha256.clone(),
                routing_policy_notes: routing_context.policy_notes.clone(),
                routing_confidence: routing_context.confidence,
                routing_liquidity_score: routing_context.liquidity_score,
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
        RuntimeClientError::Contract(message) => {
            NeobankError::Runtime(format!("contract={message}"))
        }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use axum::{
        Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post,
    };
    use chrono::Utc;
    use serde_json::{Value, json};
    use tokio::net::TcpListener;
    use tokio::sync::{Mutex, oneshot};

    use super::{
        CepPaymentContext, HYDRA_ROUTE_PROVIDER_CEP, QuoteAndPayBolt11Request, RoutePolicy,
        TreasuryRouter,
    };
    use crate::{
        budgets::InMemoryBudgetHooks, rails::runtime::RuntimeInternalApiClient,
        receipts::PaymentRouteKind,
    };

    #[derive(Debug, Clone, Copy)]
    enum RoutingStubMode {
        SelectCep,
        Error,
    }

    #[derive(Clone)]
    struct RoutingStubState {
        mode: RoutingStubMode,
        calls: Arc<Mutex<Vec<String>>>,
    }

    struct RuntimeStub {
        base_url: String,
        calls: Arc<Mutex<Vec<String>>>,
        shutdown: Option<oneshot::Sender<()>>,
    }

    impl RuntimeStub {
        async fn stop(mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
        }
    }

    async fn spawn_runtime_stub(mode: RoutingStubMode) -> Result<RuntimeStub> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let state = RoutingStubState {
            mode,
            calls: calls.clone(),
        };
        let app = Router::new()
            .route(
                "/internal/v1/hydra/routing/score",
                post(hydra_routing_score),
            )
            .route(
                "/internal/v1/liquidity/quote_pay",
                post(liquidity_quote_pay),
            )
            .route("/internal/v1/liquidity/pay", post(liquidity_pay))
            .route("/internal/v1/credit/offer", post(credit_offer))
            .route("/internal/v1/credit/envelope", post(credit_envelope))
            .route("/internal/v1/credit/settle", post(credit_settle))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });

        Ok(RuntimeStub {
            base_url: format!("http://{addr}"),
            calls,
            shutdown: Some(shutdown_tx),
        })
    }

    async fn record_call(calls: &Arc<Mutex<Vec<String>>>, name: &str) {
        let mut guard = calls.lock().await;
        guard.push(name.to_string());
    }

    async fn hydra_routing_score(
        State(state): State<RoutingStubState>,
        Json(body): Json<Value>,
    ) -> impl IntoResponse {
        record_call(&state.calls, "hydra_routing_score").await;
        match state.mode {
            RoutingStubMode::Error => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "dependency_unavailable",
                        "message": "hydra unavailable"
                    }
                })),
            )
                .into_response(),
            RoutingStubMode::SelectCep => {
                let run_id = body
                    .get("run_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let objective_hash = body.get("objective_hash").cloned().unwrap_or(Value::Null);
                let decided_at_unix = body
                    .get("decided_at_unix")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                Json(json!({
                    "schema": "openagents.hydra.routing_score_response.v1",
                    "decision_sha256": "d".repeat(64),
                    "policy": "balanced_v1",
                    "run_id": run_id,
                    "marketplace_id": "openagents",
                    "capability": "oa.liquidity.pay_bolt11.v1",
                    "objective_hash": objective_hash,
                    "selected": {
                        "marketplace_id": "openagents",
                        "provider_id": HYDRA_ROUTE_PROVIDER_CEP,
                        "total_price_msats": 110,
                        "latency_ms": 200,
                        "reliability_bps": 9700,
                        "constraints": {"routeKind":"cep_envelope"}
                    },
                    "candidates": [
                        {
                            "marketplace_id": "openagents",
                            "provider_id": "route-direct",
                            "total_price_msats": 100,
                            "latency_ms": 100,
                            "reliability_bps": 9600,
                            "constraints": {"routeKind":"direct_liquidity"}
                        },
                        {
                            "marketplace_id": "openagents",
                            "provider_id": HYDRA_ROUTE_PROVIDER_CEP,
                            "total_price_msats": 110,
                            "latency_ms": 200,
                            "reliability_bps": 9700,
                            "constraints": {"routeKind":"cep_envelope"}
                        }
                    ],
                    "factors": {
                        "expected_fee_msats": 2,
                        "confidence": 0.94,
                        "liquidity_score": 0.88,
                        "policy_notes": ["route:selected=cep"]
                    },
                    "receipt": {
                        "receipt_schema": "openagents.hydra.routing_decision_receipt.v1",
                        "receipt_id": "hydrart_test",
                        "canonical_json_sha256": "e".repeat(64)
                    },
                    "nostr_event": {},
                    "decided_at_unix": decided_at_unix
                }))
                .into_response()
            }
        }
    }

    async fn liquidity_quote_pay(
        State(state): State<RoutingStubState>,
        Json(body): Json<Value>,
    ) -> impl IntoResponse {
        record_call(&state.calls, "liquidity_quote_pay").await;
        let now = Utc::now().to_rfc3339();
        Json(json!({
            "schema": "openagents.liquidity.quote_pay_response.v1",
            "quote_id": "liq_quote_1",
            "idempotency_key": body.get("idempotency_key").and_then(Value::as_str).unwrap_or(""),
            "invoice_hash": "inv_hash_1",
            "host": body.get("host").and_then(Value::as_str).unwrap_or(""),
            "quoted_amount_msats": 100,
            "max_amount_msats": 120,
            "max_fee_msats": 20,
            "policy_context_sha256": "policy_sha_1",
            "valid_until": now,
            "created_at": now
        }))
    }

    async fn liquidity_pay(
        State(state): State<RoutingStubState>,
        _body: Json<Value>,
    ) -> impl IntoResponse {
        record_call(&state.calls, "liquidity_pay").await;
        Json(json!({
            "schema": "openagents.liquidity.pay_response.v1",
            "quote_id": "liq_quote_1",
            "status": "succeeded",
            "wallet_receipt_sha256": "f".repeat(64),
            "preimage_sha256": "a".repeat(64),
            "paid_at_ms": 1700000000000_i64,
            "error_code": null,
            "error_message": null,
            "receipt": {
                "schema": "openagents.liquidity.invoice_pay_receipt.v1",
                "receipt_id": "liq_receipt_1",
                "canonical_json_sha256": "f".repeat(64)
            },
            "payment_proof": {
                "type": "lightning_preimage",
                "value": "abcd"
            }
        }))
    }

    async fn credit_offer(
        State(state): State<RoutingStubState>,
        _body: Json<Value>,
    ) -> impl IntoResponse {
        record_call(&state.calls, "credit_offer").await;
        let now = Utc::now().to_rfc3339();
        Json(json!({
            "schema": "openagents.credit.offer_response.v1",
            "offer": {
                "offer_id": "offer_1",
                "agent_id": "agent_1",
                "pool_id": "pool_1",
                "scope_type": "nip90",
                "scope_id": "scope_1",
                "max_sats": 1,
                "fee_bps": 100,
                "requires_verifier": true,
                "exp": now,
                "status": "offered",
                "issued_at": now
            }
        }))
    }

    async fn credit_envelope(
        State(state): State<RoutingStubState>,
        _body: Json<Value>,
    ) -> impl IntoResponse {
        record_call(&state.calls, "credit_envelope").await;
        let now = Utc::now().to_rfc3339();
        Json(json!({
            "schema": "openagents.credit.envelope_response.v1",
            "envelope": {
                "envelope_id": "envelope_1",
                "offer_id": "offer_1",
                "agent_id": "agent_1",
                "pool_id": "pool_1",
                "provider_id": "provider_1",
                "scope_type": "nip90",
                "scope_id": "scope_1",
                "max_sats": 1,
                "fee_bps": 100,
                "exp": now,
                "status": "accepted",
                "issued_at": now
            },
            "receipt": {
                "schema": "openagents.credit.envelope_issue_receipt.v1"
            }
        }))
    }

    async fn credit_settle(
        State(state): State<RoutingStubState>,
        _body: Json<Value>,
    ) -> impl IntoResponse {
        record_call(&state.calls, "credit_settle").await;
        Json(json!({
            "schema": "openagents.credit.settle_response.v1",
            "envelope_id": "envelope_1",
            "settlement_id": "settle_1",
            "outcome": "success",
            "spent_sats": 1,
            "fee_sats": 0,
            "verification_receipt_sha256": "v".repeat(64),
            "liquidity_receipt_sha256": "l".repeat(64),
            "receipt": {
                "schema": "openagents.credit.envelope_settlement_receipt.v1",
                "canonical_json_sha256": "l".repeat(64)
            }
        }))
    }

    fn test_cep_context() -> CepPaymentContext {
        CepPaymentContext {
            agent_id: "agent_1".to_string(),
            pool_id: "pool_1".to_string(),
            provider_id: "provider_1".to_string(),
            scope_id: "scope_1".to_string(),
            max_sats_cap: Some(10),
            offer_ttl_seconds: Some(600),
            verification_passed: true,
            verification_receipt_sha256: Some("v".repeat(64)),
        }
    }

    #[tokio::test]
    async fn route_selection_uses_hydra_score_and_can_flip_to_cep() -> Result<()> {
        let stub = spawn_runtime_stub(RoutingStubMode::SelectCep).await?;
        let runtime = RuntimeInternalApiClient::new(stub.base_url.clone(), None);
        let budget_hooks = Arc::new(InMemoryBudgetHooks::default());
        let router = TreasuryRouter::new(runtime, budget_hooks);

        let response = router
            .quote_and_pay_bolt11(QuoteAndPayBolt11Request {
                invoice: "lnbc1n1test".to_string(),
                host: "l402.openagents.com".to_string(),
                max_fee_msats: 20,
                urgency: None,
                policy_context: json!({"schema":"test.policy_context.v1"}),
                run_id: Some(uuid::Uuid::new_v4().to_string()),
                trajectory_hash: Some("sha256:traj".to_string()),
                idempotency_key: "idem-route-cep".to_string(),
                route_policy: RoutePolicy::PreferAgentBalance {
                    agent_balance_sats: 100_000,
                    min_reserve_sats: 0,
                    direct_allowed: true,
                },
                cep: Some(test_cep_context()),
            })
            .await?;

        assert_eq!(response.route_kind, PaymentRouteKind::CepEnvelope);
        assert!(response.receipt.routing_decision_sha256.is_some());
        assert!(!response.receipt.routing_policy_notes.is_empty());

        let calls = stub.calls.lock().await.clone();
        assert!(calls.iter().any(|entry| entry == "hydra_routing_score"));
        assert!(calls.iter().any(|entry| entry == "credit_settle"));

        stub.stop().await;
        Ok(())
    }

    #[tokio::test]
    async fn route_selection_falls_back_when_hydra_unavailable() -> Result<()> {
        let stub = spawn_runtime_stub(RoutingStubMode::Error).await?;
        let runtime = RuntimeInternalApiClient::new(stub.base_url.clone(), None);
        let budget_hooks = Arc::new(InMemoryBudgetHooks::default());
        let router = TreasuryRouter::new(runtime, budget_hooks);

        let response = router
            .quote_and_pay_bolt11(QuoteAndPayBolt11Request {
                invoice: "lnbc1n1test".to_string(),
                host: "l402.openagents.com".to_string(),
                max_fee_msats: 20,
                urgency: None,
                policy_context: json!({"schema":"test.policy_context.v1"}),
                run_id: Some(uuid::Uuid::new_v4().to_string()),
                trajectory_hash: Some("sha256:traj".to_string()),
                idempotency_key: "idem-route-fallback".to_string(),
                route_policy: RoutePolicy::PreferAgentBalance {
                    agent_balance_sats: 100_000,
                    min_reserve_sats: 0,
                    direct_allowed: true,
                },
                cep: Some(test_cep_context()),
            })
            .await?;

        assert_eq!(response.route_kind, PaymentRouteKind::DirectLiquidity);
        assert!(response.receipt.routing_decision_sha256.is_none());

        let calls = stub.calls.lock().await.clone();
        assert!(calls.iter().any(|entry| entry == "hydra_routing_score"));
        assert!(calls.iter().any(|entry| entry == "liquidity_pay"));

        stub.stop().await;
        Ok(())
    }
}
