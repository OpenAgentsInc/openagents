use axum::{
    Router,
    routing::{get, post},
};

use super::*;

pub(super) fn build_internal_router() -> Router<AppState> {
    let router = Router::<AppState>::new();
    let router = add_health_and_spec_routes(router);
    let router = add_run_routes(router);
    let router = add_khala_and_projector_routes(router);
    let router = add_worker_routes(router);
    let router = add_marketplace_routes(router);
    let router = add_hydra_liquidity_credit_routes(router);
    let router = add_aegis_routes(router);
    let router = add_verification_and_treasury_routes(router);
    add_pool_and_fraud_routes(router)
}

fn add_health_and_spec_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .route(
            route_ownership::ROUTE_INTERNAL_V1_OPENAPI_JSON,
            get(internal_openapi_spec),
        )
}

fn add_run_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            route_ownership::ROUTE_INTERNAL_V1_COMMS_DELIVERY_EVENTS,
            post(record_comms_delivery_event),
        )
        .route(route_ownership::ROUTE_INTERNAL_V1_RUNS, post(start_run))
        .route(route_ownership::ROUTE_INTERNAL_V1_RUN_BY_ID, get(get_run))
        .route(
            route_ownership::ROUTE_INTERNAL_V1_RUN_EVENTS,
            post(append_run_event),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_RUN_RECEIPT,
            get(get_run_receipt),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_RUN_REPLAY,
            get(get_run_replay),
        )
}

fn add_khala_and_projector_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            route_ownership::ROUTE_INTERNAL_V1_KHALA_TOPIC_MESSAGES,
            get(get_khala_topic_messages),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_KHALA_TOPIC_WS,
            get(get_khala_topic_ws),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_KHALA_FANOUT_HOOKS,
            get(get_khala_fanout_hooks),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_KHALA_FANOUT_METRICS,
            get(get_khala_fanout_metrics),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_SPACETIME_SYNC_METRICS,
            get(get_spacetime_sync_observability),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_PROJECTOR_CHECKPOINT,
            get(get_run_checkpoint),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_PROJECTOR_DRIFT,
            get(get_projector_drift),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_PROJECTOR_RUN_SUMMARY,
            get(get_projector_run_summary),
        )
}

fn add_worker_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            route_ownership::ROUTE_INTERNAL_V1_WORKERS,
            get(list_workers).post(register_worker),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_WORKER_BY_ID,
            get(get_worker),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_WORKER_HEARTBEAT,
            post(heartbeat_worker),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_WORKER_STATUS,
            post(transition_worker),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_WORKER_CHECKPOINT,
            get(get_worker_checkpoint),
        )
}

fn add_marketplace_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            route_ownership::ROUTE_INTERNAL_V1_MARKETPLACE_CATALOG_PROVIDERS,
            get(get_provider_catalog),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_MARKETPLACE_CATALOG_JOB_TYPES,
            get(get_job_types),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_MARKETPLACE_TELEMETRY_COMPUTE,
            get(get_compute_telemetry),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_MARKETPLACE_ROUTE_PROVIDER,
            post(route_provider),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_MARKETPLACE_QUOTE_SANDBOX_RUN,
            post(quote_sandbox_run),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_MARKETPLACE_ROUTER_SELECT_COMPUTE,
            post(router_select_compute),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_MARKETPLACE_DISPATCH_SANDBOX_RUN,
            post(dispatch_sandbox_run),
        )
}

fn add_hydra_liquidity_credit_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            route_ownership::ROUTE_INTERNAL_V1_HYDRA_ROUTING_SCORE,
            post(hydra_routing_score),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_HYDRA_FX_RFQ,
            post(hydra_fx_rfq_create),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_HYDRA_FX_QUOTE,
            post(hydra_fx_quote_upsert),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_HYDRA_FX_SELECT,
            post(hydra_fx_select),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_HYDRA_FX_SETTLE,
            post(hydra_fx_settle),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_HYDRA_FX_RFQ_BY_ID,
            get(hydra_fx_rfq_get),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_HYDRA_RISK_HEALTH,
            get(hydra_risk_health),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_HYDRA_OBSERVABILITY,
            get(hydra_observability),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_LIQUIDITY_QUOTE_PAY,
            post(liquidity_quote_pay),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_CREDIT_INTENT,
            post(credit_intent),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_CREDIT_OFFER,
            post(credit_offer),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_CREDIT_ENVELOPE,
            post(credit_envelope),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_CREDIT_SETTLE,
            post(credit_settle),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_CREDIT_HEALTH,
            get(credit_health),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_CREDIT_AGENT_EXPOSURE,
            get(credit_agent_exposure),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_LIQUIDITY_STATUS,
            get(liquidity_status),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_LIQUIDITY_PAY,
            post(liquidity_pay),
        )
}

fn add_aegis_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            route_ownership::ROUTE_INTERNAL_V1_AEGIS_CLASSIFY,
            post(aegis_classify),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_AEGIS_VERIFY,
            post(aegis_verify),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_AEGIS_RISK_BUDGET,
            get(aegis_risk_budget),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_AEGIS_WARRANTY_ISSUE,
            post(aegis_warranty_issue),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_AEGIS_CLAIMS_OPEN,
            post(aegis_claim_open),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_AEGIS_CLAIMS_RESOLVE,
            post(aegis_claim_resolve),
        )
}

fn add_verification_and_treasury_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            route_ownership::ROUTE_INTERNAL_V1_VERIFICATIONS_SANDBOX_RUN,
            post(verify_sandbox_run),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_VERIFICATIONS_REPO_INDEX,
            post(verify_repo_index),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_TREASURY_COMPUTE_SUMMARY,
            get(get_compute_treasury_summary),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_TREASURY_COMPUTE_RECONCILE,
            post(reconcile_compute_treasury),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_TREASURY_COMPUTE_SETTLE_SANDBOX_RUN,
            post(settle_sandbox_run),
        )
}

fn add_pool_and_fraud_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            route_ownership::ROUTE_INTERNAL_V1_POOL_ADMIN_CREATE,
            post(liquidity_pool_create_pool),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_POOL_DEPOSIT_QUOTE,
            post(liquidity_pool_deposit_quote),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_POOL_DEPOSIT_CONFIRM,
            post(liquidity_pool_confirm_deposit),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_POOL_WITHDRAW_REQUEST,
            post(liquidity_pool_withdraw_request),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_POOL_STATUS,
            get(liquidity_pool_status),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_POOL_LATEST_SNAPSHOT,
            get(liquidity_pool_latest_snapshot),
        )
        .route(
            route_ownership::ROUTE_INTERNAL_V1_FRAUD_INCIDENTS,
            get(get_fraud_incidents),
        )
}
