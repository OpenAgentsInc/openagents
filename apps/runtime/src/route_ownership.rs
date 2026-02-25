use serde::Serialize;

pub const ROUTE_INTERNAL_V1_OPENAPI_JSON: &str = "/internal/v1/openapi.json";
pub const ROUTE_INTERNAL_V1_COMMS_DELIVERY_EVENTS: &str = "/internal/v1/comms/delivery-events";
pub const ROUTE_INTERNAL_V1_RUNS: &str = "/internal/v1/runs";
pub const ROUTE_INTERNAL_V1_RUN_BY_ID: &str = "/internal/v1/runs/:run_id";
pub const ROUTE_INTERNAL_V1_RUN_EVENTS: &str = "/internal/v1/runs/:run_id/events";
pub const ROUTE_INTERNAL_V1_RUN_RECEIPT: &str = "/internal/v1/runs/:run_id/receipt";
pub const ROUTE_INTERNAL_V1_RUN_REPLAY: &str = "/internal/v1/runs/:run_id/replay";
pub const ROUTE_INTERNAL_V1_KHALA_TOPIC_MESSAGES: &str =
    "/internal/v1/khala/topics/:topic/messages";
pub const ROUTE_INTERNAL_V1_KHALA_TOPIC_WS: &str = "/internal/v1/khala/topics/:topic/ws";
pub const ROUTE_INTERNAL_V1_KHALA_FANOUT_HOOKS: &str = "/internal/v1/khala/fanout/hooks";
pub const ROUTE_INTERNAL_V1_KHALA_FANOUT_METRICS: &str = "/internal/v1/khala/fanout/metrics";
pub const ROUTE_INTERNAL_V1_PROJECTOR_CHECKPOINT: &str =
    "/internal/v1/projectors/checkpoints/:run_id";
pub const ROUTE_INTERNAL_V1_PROJECTOR_DRIFT: &str = "/internal/v1/projectors/drift";
pub const ROUTE_INTERNAL_V1_PROJECTOR_RUN_SUMMARY: &str =
    "/internal/v1/projectors/run-summary/:run_id";
pub const ROUTE_INTERNAL_V1_WORKERS: &str = "/internal/v1/workers";
pub const ROUTE_INTERNAL_V1_WORKER_BY_ID: &str = "/internal/v1/workers/:worker_id";
pub const ROUTE_INTERNAL_V1_WORKER_HEARTBEAT: &str = "/internal/v1/workers/:worker_id/heartbeat";
pub const ROUTE_INTERNAL_V1_WORKER_STATUS: &str = "/internal/v1/workers/:worker_id/status";
pub const ROUTE_INTERNAL_V1_WORKER_CHECKPOINT: &str = "/internal/v1/workers/:worker_id/checkpoint";
pub const ROUTE_INTERNAL_V1_MARKETPLACE_CATALOG_PROVIDERS: &str =
    "/internal/v1/marketplace/catalog/providers";
pub const ROUTE_INTERNAL_V1_MARKETPLACE_CATALOG_JOB_TYPES: &str =
    "/internal/v1/marketplace/catalog/job-types";
pub const ROUTE_INTERNAL_V1_MARKETPLACE_TELEMETRY_COMPUTE: &str =
    "/internal/v1/marketplace/telemetry/compute";
pub const ROUTE_INTERNAL_V1_MARKETPLACE_ROUTE_PROVIDER: &str =
    "/internal/v1/marketplace/route/provider";
pub const ROUTE_INTERNAL_V1_MARKETPLACE_QUOTE_SANDBOX_RUN: &str =
    "/internal/v1/marketplace/compute/quote/sandbox-run";
pub const ROUTE_INTERNAL_V1_MARKETPLACE_ROUTER_SELECT_COMPUTE: &str =
    "/internal/v1/marketplace/router/compute/select";
pub const ROUTE_INTERNAL_V1_HYDRA_ROUTING_SCORE: &str = "/internal/v1/hydra/routing/score";
pub const ROUTE_INTERNAL_V1_HYDRA_FX_RFQ: &str = "/internal/v1/hydra/fx/rfq";
pub const ROUTE_INTERNAL_V1_HYDRA_FX_QUOTE: &str = "/internal/v1/hydra/fx/quote";
pub const ROUTE_INTERNAL_V1_HYDRA_FX_SELECT: &str = "/internal/v1/hydra/fx/select";
pub const ROUTE_INTERNAL_V1_HYDRA_FX_SETTLE: &str = "/internal/v1/hydra/fx/settle";
pub const ROUTE_INTERNAL_V1_HYDRA_FX_RFQ_BY_ID: &str = "/internal/v1/hydra/fx/rfq/:rfq_id";
pub const ROUTE_INTERNAL_V1_HYDRA_RISK_HEALTH: &str = "/internal/v1/hydra/risk/health";
pub const ROUTE_INTERNAL_V1_HYDRA_OBSERVABILITY: &str = "/internal/v1/hydra/observability";
pub const ROUTE_INTERNAL_V1_MARKETPLACE_DISPATCH_SANDBOX_RUN: &str =
    "/internal/v1/marketplace/dispatch/sandbox-run";
pub const ROUTE_INTERNAL_V1_VERIFICATIONS_SANDBOX_RUN: &str =
    "/internal/v1/verifications/sandbox-run";
pub const ROUTE_INTERNAL_V1_VERIFICATIONS_REPO_INDEX: &str =
    "/internal/v1/verifications/repo-index";
pub const ROUTE_INTERNAL_V1_TREASURY_COMPUTE_SUMMARY: &str =
    "/internal/v1/treasury/compute/summary";
pub const ROUTE_INTERNAL_V1_TREASURY_COMPUTE_RECONCILE: &str =
    "/internal/v1/treasury/compute/reconcile";
pub const ROUTE_INTERNAL_V1_TREASURY_COMPUTE_SETTLE_SANDBOX_RUN: &str =
    "/internal/v1/treasury/compute/settle/sandbox-run";
pub const ROUTE_INTERNAL_V1_LIQUIDITY_QUOTE_PAY: &str = "/internal/v1/liquidity/quote_pay";
pub const ROUTE_INTERNAL_V1_CREDIT_INTENT: &str = "/internal/v1/credit/intent";
pub const ROUTE_INTERNAL_V1_CREDIT_OFFER: &str = "/internal/v1/credit/offer";
pub const ROUTE_INTERNAL_V1_CREDIT_ENVELOPE: &str = "/internal/v1/credit/envelope";
pub const ROUTE_INTERNAL_V1_CREDIT_SETTLE: &str = "/internal/v1/credit/settle";
pub const ROUTE_INTERNAL_V1_CREDIT_HEALTH: &str = "/internal/v1/credit/health";
pub const ROUTE_INTERNAL_V1_CREDIT_AGENT_EXPOSURE: &str =
    "/internal/v1/credit/agents/:agent_id/exposure";
pub const ROUTE_INTERNAL_V1_LIQUIDITY_STATUS: &str = "/internal/v1/liquidity/status";
pub const ROUTE_INTERNAL_V1_LIQUIDITY_PAY: &str = "/internal/v1/liquidity/pay";
pub const ROUTE_INTERNAL_V1_POOL_ADMIN_CREATE: &str = "/internal/v1/pools/:pool_id/admin/create";
pub const ROUTE_INTERNAL_V1_POOL_DEPOSIT_QUOTE: &str = "/internal/v1/pools/:pool_id/deposit_quote";
pub const ROUTE_INTERNAL_V1_POOL_DEPOSIT_CONFIRM: &str =
    "/internal/v1/pools/:pool_id/deposits/:deposit_id/confirm";
pub const ROUTE_INTERNAL_V1_POOL_WITHDRAW_REQUEST: &str =
    "/internal/v1/pools/:pool_id/withdraw_request";
pub const ROUTE_INTERNAL_V1_POOL_STATUS: &str = "/internal/v1/pools/:pool_id/status";
pub const ROUTE_INTERNAL_V1_POOL_LATEST_SNAPSHOT: &str =
    "/internal/v1/pools/:pool_id/snapshots/latest";
pub const ROUTE_INTERNAL_V1_FRAUD_INCIDENTS: &str = "/internal/v1/fraud/incidents";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteOwner {
    RuntimeService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteDelivery {
    RuntimeAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeInternalRouteOwnership {
    pub method: &'static str,
    pub path: &'static str,
    pub owner: RuntimeRouteOwner,
    pub delivery: RuntimeRouteDelivery,
    pub migration_status: &'static str,
    pub rationale: &'static str,
}

const fn runtime_authority_route(
    method: &'static str,
    path: &'static str,
    rationale: &'static str,
) -> RuntimeInternalRouteOwnership {
    RuntimeInternalRouteOwnership {
        method,
        path,
        owner: RuntimeRouteOwner::RuntimeService,
        delivery: RuntimeRouteDelivery::RuntimeAuthority,
        migration_status: "runtime_authority_canonical",
        rationale,
    }
}

pub const INTERNAL_V1_ROUTE_OWNERSHIP: &[RuntimeInternalRouteOwnership] = &[
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_OPENAPI_JSON,
        "runtime service internal contract surface",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_COMMS_DELIVERY_EVENTS,
        "runtime comms delivery ingest authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_RUNS,
        "runtime run start authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_RUN_BY_ID,
        "runtime run read authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_RUN_EVENTS,
        "runtime run event append authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_RUN_RECEIPT,
        "runtime run receipt authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_RUN_REPLAY,
        "runtime run replay authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_KHALA_TOPIC_MESSAGES,
        "runtime replay topic read authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_KHALA_TOPIC_WS,
        "runtime live topic ws authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_KHALA_FANOUT_HOOKS,
        "runtime khala fanout hooks status authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_KHALA_FANOUT_METRICS,
        "runtime khala fanout metrics authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_PROJECTOR_CHECKPOINT,
        "runtime projector checkpoint authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_PROJECTOR_DRIFT,
        "runtime projector drift authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_PROJECTOR_RUN_SUMMARY,
        "runtime projector run summary authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_WORKERS,
        "runtime worker list authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_WORKERS,
        "runtime worker register authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_WORKER_BY_ID,
        "runtime worker detail authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_WORKER_HEARTBEAT,
        "runtime worker heartbeat authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_WORKER_STATUS,
        "runtime worker status authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_WORKER_CHECKPOINT,
        "runtime worker checkpoint authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_MARKETPLACE_CATALOG_PROVIDERS,
        "runtime marketplace provider catalog authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_MARKETPLACE_CATALOG_JOB_TYPES,
        "runtime marketplace job type catalog authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_MARKETPLACE_TELEMETRY_COMPUTE,
        "runtime marketplace telemetry authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_MARKETPLACE_ROUTE_PROVIDER,
        "runtime marketplace provider route authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_MARKETPLACE_QUOTE_SANDBOX_RUN,
        "runtime marketplace quote authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_MARKETPLACE_ROUTER_SELECT_COMPUTE,
        "runtime marketplace router select authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_HYDRA_ROUTING_SCORE,
        "runtime hydra routing authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_HYDRA_FX_RFQ,
        "runtime hydra fx rfq authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_HYDRA_FX_QUOTE,
        "runtime hydra fx quote authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_HYDRA_FX_SELECT,
        "runtime hydra fx select authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_HYDRA_FX_SETTLE,
        "runtime hydra fx settle authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_HYDRA_FX_RFQ_BY_ID,
        "runtime hydra fx rfq read authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_HYDRA_RISK_HEALTH,
        "runtime hydra risk health authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_HYDRA_OBSERVABILITY,
        "runtime hydra observability authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_MARKETPLACE_DISPATCH_SANDBOX_RUN,
        "runtime marketplace dispatch authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_VERIFICATIONS_SANDBOX_RUN,
        "runtime verification authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_VERIFICATIONS_REPO_INDEX,
        "runtime repository index verification authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_TREASURY_COMPUTE_SUMMARY,
        "runtime treasury summary authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_TREASURY_COMPUTE_RECONCILE,
        "runtime treasury reconcile authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_TREASURY_COMPUTE_SETTLE_SANDBOX_RUN,
        "runtime treasury settle authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_LIQUIDITY_QUOTE_PAY,
        "runtime liquidity quote authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_CREDIT_INTENT,
        "runtime credit intent authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_CREDIT_OFFER,
        "runtime credit offer authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_CREDIT_ENVELOPE,
        "runtime credit envelope authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_CREDIT_SETTLE,
        "runtime credit settle authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_CREDIT_HEALTH,
        "runtime credit health authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_CREDIT_AGENT_EXPOSURE,
        "runtime credit exposure authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_LIQUIDITY_STATUS,
        "runtime liquidity status authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_LIQUIDITY_PAY,
        "runtime liquidity pay authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_POOL_ADMIN_CREATE,
        "runtime liquidity pool create authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_POOL_DEPOSIT_QUOTE,
        "runtime liquidity pool deposit quote authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_POOL_DEPOSIT_CONFIRM,
        "runtime liquidity pool deposit confirm authority",
    ),
    runtime_authority_route(
        "POST",
        ROUTE_INTERNAL_V1_POOL_WITHDRAW_REQUEST,
        "runtime liquidity pool withdraw request authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_POOL_STATUS,
        "runtime liquidity pool status authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_POOL_LATEST_SNAPSHOT,
        "runtime liquidity pool latest snapshot authority",
    ),
    runtime_authority_route(
        "GET",
        ROUTE_INTERNAL_V1_FRAUD_INCIDENTS,
        "runtime fraud incident read authority",
    ),
];

#[must_use]
pub fn internal_v1_route_ownership() -> &'static [RuntimeInternalRouteOwnership] {
    INTERNAL_V1_ROUTE_OWNERSHIP
}

#[must_use]
pub fn internal_v1_route_owner(method: &str, path: &str) -> Option<RuntimeRouteOwner> {
    let normalized_method = method.trim().to_ascii_uppercase();
    INTERNAL_V1_ROUTE_OWNERSHIP
        .iter()
        .find(|entry| entry.method == normalized_method && entry.path == path)
        .map(|entry| entry.owner)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn internal_route_ownership_contract_is_unambiguous() {
        let mut seen = HashSet::new();
        for entry in internal_v1_route_ownership() {
            let key = format!("{} {}", entry.method, entry.path);
            assert!(
                seen.insert(key.clone()),
                "duplicate internal route ownership entry: {key}"
            );
            assert!(
                entry.path.starts_with("/internal/v1/"),
                "non-internal path in internal route ownership contract: {}",
                entry.path
            );
        }
    }

    #[test]
    fn internal_route_ownership_contract_covers_runtime_router_contract() {
        let required = [
            ("GET", ROUTE_INTERNAL_V1_OPENAPI_JSON),
            ("POST", ROUTE_INTERNAL_V1_COMMS_DELIVERY_EVENTS),
            ("POST", ROUTE_INTERNAL_V1_RUNS),
            ("GET", ROUTE_INTERNAL_V1_RUN_BY_ID),
            ("POST", ROUTE_INTERNAL_V1_RUN_EVENTS),
            ("GET", ROUTE_INTERNAL_V1_RUN_RECEIPT),
            ("GET", ROUTE_INTERNAL_V1_RUN_REPLAY),
            ("GET", ROUTE_INTERNAL_V1_KHALA_TOPIC_MESSAGES),
            ("GET", ROUTE_INTERNAL_V1_KHALA_TOPIC_WS),
            ("GET", ROUTE_INTERNAL_V1_KHALA_FANOUT_HOOKS),
            ("GET", ROUTE_INTERNAL_V1_KHALA_FANOUT_METRICS),
            ("GET", ROUTE_INTERNAL_V1_PROJECTOR_CHECKPOINT),
            ("GET", ROUTE_INTERNAL_V1_PROJECTOR_DRIFT),
            ("GET", ROUTE_INTERNAL_V1_PROJECTOR_RUN_SUMMARY),
            ("GET", ROUTE_INTERNAL_V1_WORKERS),
            ("POST", ROUTE_INTERNAL_V1_WORKERS),
            ("GET", ROUTE_INTERNAL_V1_WORKER_BY_ID),
            ("POST", ROUTE_INTERNAL_V1_WORKER_HEARTBEAT),
            ("POST", ROUTE_INTERNAL_V1_WORKER_STATUS),
            ("GET", ROUTE_INTERNAL_V1_WORKER_CHECKPOINT),
            ("GET", ROUTE_INTERNAL_V1_MARKETPLACE_CATALOG_PROVIDERS),
            ("GET", ROUTE_INTERNAL_V1_MARKETPLACE_CATALOG_JOB_TYPES),
            ("GET", ROUTE_INTERNAL_V1_MARKETPLACE_TELEMETRY_COMPUTE),
            ("POST", ROUTE_INTERNAL_V1_MARKETPLACE_ROUTE_PROVIDER),
            ("POST", ROUTE_INTERNAL_V1_MARKETPLACE_QUOTE_SANDBOX_RUN),
            ("POST", ROUTE_INTERNAL_V1_MARKETPLACE_ROUTER_SELECT_COMPUTE),
            ("POST", ROUTE_INTERNAL_V1_HYDRA_ROUTING_SCORE),
            ("POST", ROUTE_INTERNAL_V1_HYDRA_FX_RFQ),
            ("POST", ROUTE_INTERNAL_V1_HYDRA_FX_QUOTE),
            ("POST", ROUTE_INTERNAL_V1_HYDRA_FX_SELECT),
            ("POST", ROUTE_INTERNAL_V1_HYDRA_FX_SETTLE),
            ("GET", ROUTE_INTERNAL_V1_HYDRA_FX_RFQ_BY_ID),
            ("GET", ROUTE_INTERNAL_V1_HYDRA_RISK_HEALTH),
            ("GET", ROUTE_INTERNAL_V1_HYDRA_OBSERVABILITY),
            ("POST", ROUTE_INTERNAL_V1_MARKETPLACE_DISPATCH_SANDBOX_RUN),
            ("POST", ROUTE_INTERNAL_V1_VERIFICATIONS_SANDBOX_RUN),
            ("POST", ROUTE_INTERNAL_V1_VERIFICATIONS_REPO_INDEX),
            ("GET", ROUTE_INTERNAL_V1_TREASURY_COMPUTE_SUMMARY),
            ("POST", ROUTE_INTERNAL_V1_TREASURY_COMPUTE_RECONCILE),
            (
                "POST",
                ROUTE_INTERNAL_V1_TREASURY_COMPUTE_SETTLE_SANDBOX_RUN,
            ),
            ("POST", ROUTE_INTERNAL_V1_LIQUIDITY_QUOTE_PAY),
            ("POST", ROUTE_INTERNAL_V1_CREDIT_INTENT),
            ("POST", ROUTE_INTERNAL_V1_CREDIT_OFFER),
            ("POST", ROUTE_INTERNAL_V1_CREDIT_ENVELOPE),
            ("POST", ROUTE_INTERNAL_V1_CREDIT_SETTLE),
            ("GET", ROUTE_INTERNAL_V1_CREDIT_HEALTH),
            ("GET", ROUTE_INTERNAL_V1_CREDIT_AGENT_EXPOSURE),
            ("GET", ROUTE_INTERNAL_V1_LIQUIDITY_STATUS),
            ("POST", ROUTE_INTERNAL_V1_LIQUIDITY_PAY),
            ("POST", ROUTE_INTERNAL_V1_POOL_ADMIN_CREATE),
            ("POST", ROUTE_INTERNAL_V1_POOL_DEPOSIT_QUOTE),
            ("POST", ROUTE_INTERNAL_V1_POOL_DEPOSIT_CONFIRM),
            ("POST", ROUTE_INTERNAL_V1_POOL_WITHDRAW_REQUEST),
            ("GET", ROUTE_INTERNAL_V1_POOL_STATUS),
            ("GET", ROUTE_INTERNAL_V1_POOL_LATEST_SNAPSHOT),
            ("GET", ROUTE_INTERNAL_V1_FRAUD_INCIDENTS),
        ];

        for (method, path) in required {
            assert!(
                internal_v1_route_owner(method, path).is_some(),
                "missing internal route ownership entry for {} {}",
                method,
                path
            );
        }
    }
}
