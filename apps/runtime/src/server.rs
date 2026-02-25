use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use futures::StreamExt;
use openagents_l402::Bolt11;
use openagents_proto::hydra_routing::{
    ROUTING_SCORE_RESPONSE_SCHEMA_V1, RoutingCandidateQuoteV1 as HydraRoutingCandidateQuoteV1,
    RoutingDecisionFactorsV1 as HydraRoutingDecisionFactorsV1,
    RoutingDecisionReceiptLinkageV1 as HydraRoutingDecisionReceiptLinkageV1,
    RoutingScoreRequestV1 as HydraRoutingScoreRequestV1,
    RoutingScoreResponseV1 as HydraRoutingScoreResponseV1,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    artifacts::{ArtifactError, RuntimeReceipt, build_receipt, build_replay_jsonl},
    authority::AuthorityError,
    bridge::{
        BridgeNostrPublisher, CommerceMessageKindV1, CommerceMessageV1, PricingBandV1,
        PricingStageV1, ProviderAdV1, build_commerce_message_event, build_provider_ad_event,
    },
    config::Config,
    credit::service::{CreditError, CreditService},
    credit::store as credit_store,
    credit::types::{
        CreditAgentExposureResponseV1, CreditEnvelopeRequestV1, CreditEnvelopeResponseV1,
        CreditHealthResponseV1, CreditIntentRequestV1, CreditIntentResponseV1,
        CreditOfferRequestV1, CreditOfferResponseV1, CreditScopeTypeV1, CreditSettleRequestV1,
        CreditSettleResponseV1,
    },
    db::RuntimeDb,
    fanout::{ExternalFanoutHook, FanoutError, FanoutHub, FanoutMessage, FanoutTopicWindow},
    fraud::FraudIncidentLog,
    fx::{
        service::{FxService, FxServiceError},
        types::{
            FX_QUOTE_UPSERT_REQUEST_SCHEMA_V1, FX_RFQ_REQUEST_SCHEMA_V1,
            FX_SELECT_REQUEST_SCHEMA_V1, FxQuoteUpsertRequestV1, FxQuoteUpsertResponseV1,
            FxRfqRequestV1, FxRfqResponseV1, FxSelectRequestV1, FxSelectResponseV1,
        },
    },
    lightning_node,
    liquidity::store,
    liquidity::types::{
        LiquidityStatusResponseV1, PayRequestV1, PayResponseV1, QuotePayRequestV1,
        QuotePayResponseV1,
    },
    liquidity::{LiquidityError, LiquidityService},
    liquidity_pool::{
        LiquidityPoolError, LiquidityPoolService,
        service::{
            HttpWalletExecutorClient, UnavailableWalletExecutorClient, WalletExecutorClient,
            WithdrawThrottlePolicy,
        },
        store as liquidity_pool_store,
        types::{
            DepositQuoteRequestV1, DepositQuoteResponseV1, PoolCreateRequestV1,
            PoolCreateResponseV1, PoolPartitionKindV1, PoolSnapshotResponseV1,
            PoolStatusResponseV1, WithdrawRequestV1, WithdrawResponseV1,
        },
    },
    marketplace::{
        ComputeAllInQuoteV1, PricingBand, PricingStage, ProviderCatalogEntry, ProviderSelection,
        build_provider_catalog, compute_all_in_quote_v1, is_provider_worker,
        select_provider_for_capability, select_provider_for_capability_excluding,
    },
    orchestration::{OrchestrationError, RuntimeOrchestrator},
    sync_auth::{AuthorizedKhalaTopic, SyncAuthError, SyncAuthorizer, SyncPrincipal},
    treasury::Treasury,
    types::{
        AppendRunEventRequest, ProjectionCheckpoint, ProjectionDriftReport, RegisterWorkerRequest,
        RunProjectionSummary, RuntimeRun, StartRunRequest, WorkerHeartbeatRequest, WorkerOwner,
        WorkerStatus, WorkerStatusTransitionRequest,
    },
    workers::{InMemoryWorkerRegistry, WorkerError, WorkerSnapshot},
};

mod helpers;
use helpers::*;

#[derive(Clone)]
pub struct AppState {
    config: Config,
    db: Option<Arc<RuntimeDb>>,
    liquidity: Arc<LiquidityService>,
    credit: Arc<CreditService>,
    fx: Arc<FxService>,
    liquidity_pool: Arc<LiquidityPoolService>,
    orchestrator: Arc<RuntimeOrchestrator>,
    workers: Arc<InMemoryWorkerRegistry>,
    fanout: Arc<FanoutHub>,
    sync_auth: Arc<SyncAuthorizer>,
    khala_delivery: Arc<KhalaDeliveryControl>,
    compute_abuse: Arc<ComputeAbuseControls>,
    compute_telemetry: Arc<ComputeTelemetry>,
    hydra_observability: Arc<HydraObservabilityTelemetry>,
    treasury: Arc<Treasury>,
    fraud: Arc<FraudIncidentLog>,
    comms_delivery_events: Arc<Mutex<HashMap<String, CommsDeliveryAccepted>>>,
    fleet_seq: Arc<AtomicU64>,
    fraud_seq: Arc<AtomicU64>,
    started_at: chrono::DateTime<Utc>,
}

impl AppState {
    #[must_use]
    pub fn new(
        config: Config,
        orchestrator: Arc<RuntimeOrchestrator>,
        workers: Arc<InMemoryWorkerRegistry>,
        fanout: Arc<FanoutHub>,
        sync_auth: Arc<SyncAuthorizer>,
        db: Option<Arc<RuntimeDb>>,
    ) -> Self {
        let liquidity_store = match db.clone() {
            Some(db) => store::postgres(db),
            None => store::memory(),
        };
        let credit_store = match db.clone() {
            Some(db) => credit_store::postgres(db),
            None => credit_store::memory(),
        };
        let liquidity = Arc::new(LiquidityService::new(
            liquidity_store,
            config.liquidity_wallet_executor_base_url.clone(),
            config.liquidity_wallet_executor_auth_token.clone(),
            config.liquidity_wallet_executor_timeout_ms,
            config.liquidity_quote_ttl_seconds,
            config.bridge_nostr_secret_key,
        ));
        let credit = Arc::new(CreditService::new_with_policy(
            credit_store,
            liquidity.clone(),
            config.bridge_nostr_secret_key,
            config.credit_policy.clone(),
        ));
        let fx = Arc::new(FxService::new(config.hydra_fx_policy.clone()));

        let pool_store = match db.clone() {
            Some(db) => liquidity_pool_store::postgres(db),
            None => liquidity_pool_store::memory(),
        };
        let wallet_executor_client: Arc<dyn WalletExecutorClient> =
            match HttpWalletExecutorClient::new(
                config.liquidity_wallet_executor_base_url.clone(),
                config.liquidity_wallet_executor_auth_token.clone(),
                config.liquidity_wallet_executor_timeout_ms,
            ) {
                Ok(client) => Arc::new(client),
                Err(error) => Arc::new(UnavailableWalletExecutorClient::new(error.to_string())),
            };

        let state = Self {
            liquidity,
            credit,
            fx,
            liquidity_pool: Arc::new(
                LiquidityPoolService::new_with_lightning_node(
                    pool_store,
                    wallet_executor_client,
                    lightning_node::from_env(),
                    config.bridge_nostr_secret_key,
                )
                .with_withdraw_delay_hours(config.liquidity_pool_withdraw_delay_hours)
                .with_withdraw_throttle_policy(WithdrawThrottlePolicy {
                    lp_mode_enabled: config.liquidity_pool_withdraw_throttle.lp_mode_enabled,
                    stress_liability_ratio_bps: config
                        .liquidity_pool_withdraw_throttle
                        .stress_liability_ratio_bps,
                    halt_liability_ratio_bps: config
                        .liquidity_pool_withdraw_throttle
                        .halt_liability_ratio_bps,
                    stress_connected_ratio_bps: config
                        .liquidity_pool_withdraw_throttle
                        .stress_connected_ratio_bps,
                    halt_connected_ratio_bps: config
                        .liquidity_pool_withdraw_throttle
                        .halt_connected_ratio_bps,
                    stress_outbound_coverage_bps: config
                        .liquidity_pool_withdraw_throttle
                        .stress_outbound_coverage_bps,
                    halt_outbound_coverage_bps: config
                        .liquidity_pool_withdraw_throttle
                        .halt_outbound_coverage_bps,
                    stress_extra_delay_hours: config
                        .liquidity_pool_withdraw_throttle
                        .stress_extra_delay_hours,
                    halt_extra_delay_hours: config
                        .liquidity_pool_withdraw_throttle
                        .halt_extra_delay_hours,
                    stress_execution_cap_per_tick: config
                        .liquidity_pool_withdraw_throttle
                        .stress_execution_cap_per_tick,
                }),
            ),
            db,
            config,
            orchestrator,
            workers,
            fanout,
            sync_auth,
            khala_delivery: Arc::new(KhalaDeliveryControl::default()),
            compute_abuse: Arc::new(ComputeAbuseControls::default()),
            compute_telemetry: Arc::new(ComputeTelemetry::default()),
            hydra_observability: Arc::new(HydraObservabilityTelemetry::default()),
            treasury: Arc::new(Treasury::default()),
            fraud: Arc::new(FraudIncidentLog::default()),
            comms_delivery_events: Arc::new(Mutex::new(HashMap::new())),
            fleet_seq: Arc::new(AtomicU64::new(0)),
            fraud_seq: Arc::new(AtomicU64::new(0)),
            started_at: Utc::now(),
        };

        maybe_spawn_provider_multihoming_autopilot(&state);
        maybe_spawn_liquidity_pool_snapshot_worker(&state);
        maybe_spawn_treasury_reconciliation_worker(&state);

        state
    }
}

#[derive(Debug, Clone)]
struct KhalaConsumerState {
    last_poll_at: Option<chrono::DateTime<Utc>>,
    last_cursor: u64,
    slow_consumer_strikes: u32,
}

impl Default for KhalaConsumerState {
    fn default() -> Self {
        Self {
            last_poll_at: None,
            last_cursor: 0,
            slow_consumer_strikes: 0,
        }
    }
}

#[derive(Default)]
struct KhalaDeliveryControl {
    consumers: Mutex<HashMap<String, KhalaConsumerState>>,
    recent_disconnect_causes: Mutex<VecDeque<String>>,
    total_polls: AtomicU64,
    throttled_polls: AtomicU64,
    limited_polls: AtomicU64,
    fairness_limited_polls: AtomicU64,
    slow_consumer_evictions: AtomicU64,
    served_messages: AtomicU64,
}

#[derive(Default)]
struct ComputeAbuseControls {
    dispatch_window: Mutex<HashMap<String, VecDeque<chrono::DateTime<Utc>>>>,
}

const COMPUTE_DISPATCH_WINDOW_SECONDS: i64 = 60;
const COMPUTE_DISPATCH_MAX_PER_WINDOW: usize = 30;
const COMPUTE_TELEMETRY_LATENCY_SAMPLES: usize = 256;

impl ComputeAbuseControls {
    async fn enforce_dispatch_rate(&self, owner_key: &str) -> Result<(), ApiError> {
        let now = Utc::now();
        let window_start = now - chrono::Duration::seconds(COMPUTE_DISPATCH_WINDOW_SECONDS);

        let mut windows = self.dispatch_window.lock().await;
        let entries = windows.entry(owner_key.to_string()).or_default();
        while matches!(entries.front(), Some(front) if *front < window_start) {
            entries.pop_front();
        }
        if entries.len() >= COMPUTE_DISPATCH_MAX_PER_WINDOW {
            return Err(ApiError::RateLimited {
                retry_after_ms: 1_000,
                reason_code: "compute_dispatch_rate_limited".to_string(),
            });
        }
        entries.push_back(now);
        Ok(())
    }
}

#[derive(Default)]
struct ComputeTelemetry {
    owners: Mutex<HashMap<String, OwnerComputeTelemetry>>,
}

#[derive(Clone, Debug, Default)]
struct OwnerComputeTelemetry {
    dispatch_total: u64,
    dispatch_not_found: u64,
    dispatch_errors: u64,
    dispatch_fallbacks: u64,
    latencies_ms: VecDeque<u64>,
    updated_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
struct OwnerComputeTelemetrySnapshot {
    dispatch_total: u64,
    dispatch_not_found: u64,
    dispatch_errors: u64,
    dispatch_fallbacks: u64,
    latency_ms_avg: Option<u64>,
    latency_ms_p50: Option<u64>,
    samples: usize,
    updated_at: Option<chrono::DateTime<Utc>>,
}

impl ComputeTelemetry {
    async fn record_dispatch_not_found(&self, owner_key: &str) {
        self.record(owner_key, |entry| {
            entry.dispatch_total = entry.dispatch_total.saturating_add(1);
            entry.dispatch_not_found = entry.dispatch_not_found.saturating_add(1);
        })
        .await;
    }

    async fn record_dispatch_error(&self, owner_key: &str) {
        self.record(owner_key, |entry| {
            entry.dispatch_total = entry.dispatch_total.saturating_add(1);
            entry.dispatch_errors = entry.dispatch_errors.saturating_add(1);
        })
        .await;
    }

    async fn record_dispatch_success(&self, owner_key: &str, latency_ms: u64, fallback: bool) {
        self.record(owner_key, |entry| {
            entry.dispatch_total = entry.dispatch_total.saturating_add(1);
            if fallback {
                entry.dispatch_fallbacks = entry.dispatch_fallbacks.saturating_add(1);
            }
            if entry.latencies_ms.len() >= COMPUTE_TELEMETRY_LATENCY_SAMPLES {
                entry.latencies_ms.pop_front();
            }
            entry.latencies_ms.push_back(latency_ms);
        })
        .await;
    }

    async fn snapshot(&self, owner_key: &str) -> OwnerComputeTelemetrySnapshot {
        let owners = self.owners.lock().await;
        let Some(entry) = owners.get(owner_key) else {
            return OwnerComputeTelemetrySnapshot {
                dispatch_total: 0,
                dispatch_not_found: 0,
                dispatch_errors: 0,
                dispatch_fallbacks: 0,
                latency_ms_avg: None,
                latency_ms_p50: None,
                samples: 0,
                updated_at: None,
            };
        };

        let samples = entry.latencies_ms.len();
        let latency_ms_avg = if samples == 0 {
            None
        } else {
            Some(entry.latencies_ms.iter().sum::<u64>() / samples as u64)
        };
        let latency_ms_p50 = if samples == 0 {
            None
        } else {
            let mut sorted = entry.latencies_ms.iter().copied().collect::<Vec<_>>();
            sorted.sort_unstable();
            Some(sorted[(samples - 1) / 2])
        };

        OwnerComputeTelemetrySnapshot {
            dispatch_total: entry.dispatch_total,
            dispatch_not_found: entry.dispatch_not_found,
            dispatch_errors: entry.dispatch_errors,
            dispatch_fallbacks: entry.dispatch_fallbacks,
            latency_ms_avg,
            latency_ms_p50,
            samples,
            updated_at: entry.updated_at,
        }
    }

    async fn record<F: FnOnce(&mut OwnerComputeTelemetry)>(&self, owner_key: &str, f: F) {
        let now = Utc::now();
        let mut owners = self.owners.lock().await;
        let entry = owners.entry(owner_key.to_string()).or_default();
        f(entry);
        entry.updated_at = Some(now);
    }
}

#[derive(Default)]
struct HydraObservabilityTelemetry {
    state: Mutex<HydraObservabilityState>,
}

#[derive(Debug, Clone)]
struct HydraObservabilityState {
    routing_decision_total: u64,
    routing_selected_route_direct: u64,
    routing_selected_route_cep: u64,
    routing_selected_route_other: u64,
    confidence_lt_040: u64,
    confidence_040_070: u64,
    confidence_070_090: u64,
    confidence_gte_090: u64,
    breaker_halt_new_envelopes: bool,
    breaker_halt_large_settlements: bool,
    breaker_transition_total: u64,
    breaker_recovery_total: u64,
    breaker_halt_new_envelopes_transition_total: u64,
    breaker_halt_new_envelopes_recovery_total: u64,
    breaker_halt_large_settlements_transition_total: u64,
    breaker_halt_large_settlements_recovery_total: u64,
    last_breaker_transition_at: Option<chrono::DateTime<Utc>>,
    withdraw_throttle_mode: Option<String>,
    withdraw_throttle_reasons: Vec<String>,
    withdraw_throttle_extra_delay_hours: Option<i64>,
    withdraw_throttle_execution_cap_per_tick: Option<u32>,
    withdraw_throttle_affected_requests_total: u64,
    withdraw_throttle_rejected_requests_total: u64,
    withdraw_throttle_stressed_requests_total: u64,
}

impl Default for HydraObservabilityState {
    fn default() -> Self {
        Self {
            routing_decision_total: 0,
            routing_selected_route_direct: 0,
            routing_selected_route_cep: 0,
            routing_selected_route_other: 0,
            confidence_lt_040: 0,
            confidence_040_070: 0,
            confidence_070_090: 0,
            confidence_gte_090: 0,
            breaker_halt_new_envelopes: false,
            breaker_halt_large_settlements: false,
            breaker_transition_total: 0,
            breaker_recovery_total: 0,
            breaker_halt_new_envelopes_transition_total: 0,
            breaker_halt_new_envelopes_recovery_total: 0,
            breaker_halt_large_settlements_transition_total: 0,
            breaker_halt_large_settlements_recovery_total: 0,
            last_breaker_transition_at: None,
            withdraw_throttle_mode: None,
            withdraw_throttle_reasons: Vec::new(),
            withdraw_throttle_extra_delay_hours: None,
            withdraw_throttle_execution_cap_per_tick: None,
            withdraw_throttle_affected_requests_total: 0,
            withdraw_throttle_rejected_requests_total: 0,
            withdraw_throttle_stressed_requests_total: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct HydraRoutingObservabilityV1 {
    decision_total: u64,
    selected_route_direct: u64,
    selected_route_cep: u64,
    selected_route_other: u64,
    confidence_lt_040: u64,
    confidence_040_070: u64,
    confidence_070_090: u64,
    confidence_gte_090: u64,
}

#[derive(Debug, Clone, Serialize)]
struct HydraBreakersObservabilityV1 {
    halt_new_envelopes: bool,
    halt_large_settlements: bool,
    transition_total: u64,
    recovery_total: u64,
    halt_new_envelopes_transition_total: u64,
    halt_new_envelopes_recovery_total: u64,
    halt_large_settlements_transition_total: u64,
    halt_large_settlements_recovery_total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_transition_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
struct HydraWithdrawalThrottleObservabilityV1 {
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_delay_hours: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_cap_per_tick: Option<u32>,
    affected_requests_total: u64,
    rejected_requests_total: u64,
    stressed_requests_total: u64,
}

#[derive(Debug, Clone, Serialize)]
struct HydraObservabilityResponseV1 {
    schema: String,
    generated_at: chrono::DateTime<Utc>,
    routing: HydraRoutingObservabilityV1,
    breakers: HydraBreakersObservabilityV1,
    withdrawal_throttle: HydraWithdrawalThrottleObservabilityV1,
}

impl HydraObservabilityTelemetry {
    async fn record_risk_health(&self, health: &HydraRiskHealthResponseV1) {
        let now = Utc::now();
        let mut state = self.state.lock().await;

        let next_halt_new_envelopes = health.credit_breakers.halt_new_envelopes;
        let next_halt_large_settlements = health.credit_breakers.halt_large_settlements;

        if next_halt_new_envelopes != state.breaker_halt_new_envelopes {
            state.breaker_transition_total = state.breaker_transition_total.saturating_add(1);
            state.breaker_halt_new_envelopes_transition_total = state
                .breaker_halt_new_envelopes_transition_total
                .saturating_add(1);
            if state.breaker_halt_new_envelopes && !next_halt_new_envelopes {
                state.breaker_recovery_total = state.breaker_recovery_total.saturating_add(1);
                state.breaker_halt_new_envelopes_recovery_total = state
                    .breaker_halt_new_envelopes_recovery_total
                    .saturating_add(1);
            }
            state.last_breaker_transition_at = Some(now);
        }
        if next_halt_large_settlements != state.breaker_halt_large_settlements {
            state.breaker_transition_total = state.breaker_transition_total.saturating_add(1);
            state.breaker_halt_large_settlements_transition_total = state
                .breaker_halt_large_settlements_transition_total
                .saturating_add(1);
            if state.breaker_halt_large_settlements && !next_halt_large_settlements {
                state.breaker_recovery_total = state.breaker_recovery_total.saturating_add(1);
                state.breaker_halt_large_settlements_recovery_total = state
                    .breaker_halt_large_settlements_recovery_total
                    .saturating_add(1);
            }
            state.last_breaker_transition_at = Some(now);
        }

        state.breaker_halt_new_envelopes = next_halt_new_envelopes;
        state.breaker_halt_large_settlements = next_halt_large_settlements;
        state.withdraw_throttle_mode = health.liquidity.withdraw_throttle_mode.clone();
        state.withdraw_throttle_reasons = health.liquidity.withdraw_throttle_reasons.clone();
        state.withdraw_throttle_extra_delay_hours =
            health.liquidity.withdraw_throttle_extra_delay_hours;
        state.withdraw_throttle_execution_cap_per_tick =
            health.liquidity.withdraw_throttle_execution_cap_per_tick;
    }

    async fn record_routing_decision(
        &self,
        selected_provider_id: &str,
        confidence: f64,
        throttle_mode: Option<&str>,
    ) {
        let mut state = self.state.lock().await;
        state.routing_decision_total = state.routing_decision_total.saturating_add(1);

        match selected_provider_id {
            HYDRA_ROUTE_PROVIDER_DIRECT => {
                state.routing_selected_route_direct =
                    state.routing_selected_route_direct.saturating_add(1);
            }
            HYDRA_ROUTE_PROVIDER_CEP => {
                state.routing_selected_route_cep =
                    state.routing_selected_route_cep.saturating_add(1);
            }
            _ => {
                state.routing_selected_route_other =
                    state.routing_selected_route_other.saturating_add(1);
            }
        }

        let confidence = confidence.clamp(0.0, 1.0);
        if confidence < 0.40 {
            state.confidence_lt_040 = state.confidence_lt_040.saturating_add(1);
        } else if confidence < 0.70 {
            state.confidence_040_070 = state.confidence_040_070.saturating_add(1);
        } else if confidence < 0.90 {
            state.confidence_070_090 = state.confidence_070_090.saturating_add(1);
        } else {
            state.confidence_gte_090 = state.confidence_gte_090.saturating_add(1);
        }

        if let Some(mode) = throttle_mode {
            match mode {
                "normal" | "" => {}
                "stressed" => {
                    state.withdraw_throttle_affected_requests_total = state
                        .withdraw_throttle_affected_requests_total
                        .saturating_add(1);
                    state.withdraw_throttle_stressed_requests_total = state
                        .withdraw_throttle_stressed_requests_total
                        .saturating_add(1);
                }
                _ => {
                    state.withdraw_throttle_affected_requests_total = state
                        .withdraw_throttle_affected_requests_total
                        .saturating_add(1);
                }
            }
        }
    }

    async fn record_withdraw_request_throttle(
        &self,
        mode: Option<crate::liquidity_pool::types::WithdrawThrottleModeV1>,
        rejected: bool,
    ) {
        let mut state = self.state.lock().await;
        if rejected {
            state.withdraw_throttle_affected_requests_total = state
                .withdraw_throttle_affected_requests_total
                .saturating_add(1);
            state.withdraw_throttle_rejected_requests_total = state
                .withdraw_throttle_rejected_requests_total
                .saturating_add(1);
            return;
        }

        if let Some(mode) = mode {
            match mode {
                crate::liquidity_pool::types::WithdrawThrottleModeV1::Normal => {}
                crate::liquidity_pool::types::WithdrawThrottleModeV1::Stressed => {
                    state.withdraw_throttle_affected_requests_total = state
                        .withdraw_throttle_affected_requests_total
                        .saturating_add(1);
                    state.withdraw_throttle_stressed_requests_total = state
                        .withdraw_throttle_stressed_requests_total
                        .saturating_add(1);
                }
                crate::liquidity_pool::types::WithdrawThrottleModeV1::Halted => {
                    state.withdraw_throttle_affected_requests_total = state
                        .withdraw_throttle_affected_requests_total
                        .saturating_add(1);
                    state.withdraw_throttle_rejected_requests_total = state
                        .withdraw_throttle_rejected_requests_total
                        .saturating_add(1);
                }
            }
        }
    }

    async fn snapshot(&self) -> HydraObservabilityResponseV1 {
        let state = self.state.lock().await.clone();
        HydraObservabilityResponseV1 {
            schema: "openagents.hydra.observability_response.v1".to_string(),
            generated_at: Utc::now(),
            routing: HydraRoutingObservabilityV1 {
                decision_total: state.routing_decision_total,
                selected_route_direct: state.routing_selected_route_direct,
                selected_route_cep: state.routing_selected_route_cep,
                selected_route_other: state.routing_selected_route_other,
                confidence_lt_040: state.confidence_lt_040,
                confidence_040_070: state.confidence_040_070,
                confidence_070_090: state.confidence_070_090,
                confidence_gte_090: state.confidence_gte_090,
            },
            breakers: HydraBreakersObservabilityV1 {
                halt_new_envelopes: state.breaker_halt_new_envelopes,
                halt_large_settlements: state.breaker_halt_large_settlements,
                transition_total: state.breaker_transition_total,
                recovery_total: state.breaker_recovery_total,
                halt_new_envelopes_transition_total: state
                    .breaker_halt_new_envelopes_transition_total,
                halt_new_envelopes_recovery_total: state.breaker_halt_new_envelopes_recovery_total,
                halt_large_settlements_transition_total: state
                    .breaker_halt_large_settlements_transition_total,
                halt_large_settlements_recovery_total: state
                    .breaker_halt_large_settlements_recovery_total,
                last_transition_at: state.last_breaker_transition_at,
            },
            withdrawal_throttle: HydraWithdrawalThrottleObservabilityV1 {
                mode: state.withdraw_throttle_mode,
                reasons: state.withdraw_throttle_reasons,
                extra_delay_hours: state.withdraw_throttle_extra_delay_hours,
                execution_cap_per_tick: state.withdraw_throttle_execution_cap_per_tick,
                affected_requests_total: state.withdraw_throttle_affected_requests_total,
                rejected_requests_total: state.withdraw_throttle_rejected_requests_total,
                stressed_requests_total: state.withdraw_throttle_stressed_requests_total,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct KhalaDeliveryMetricsSnapshot {
    total_polls: u64,
    throttled_polls: u64,
    limited_polls: u64,
    fairness_limited_polls: u64,
    slow_consumer_evictions: u64,
    served_messages: u64,
    active_consumers: usize,
    recent_disconnect_causes: Vec<String>,
}

impl KhalaDeliveryControl {
    fn record_total_poll(&self, served_messages: usize) {
        self.total_polls.fetch_add(1, Ordering::Relaxed);
        self.served_messages
            .fetch_add(served_messages as u64, Ordering::Relaxed);
    }

    fn record_throttled_poll(&self) {
        self.throttled_polls.fetch_add(1, Ordering::Relaxed);
    }

    fn record_limit_capped(&self) {
        self.limited_polls.fetch_add(1, Ordering::Relaxed);
    }

    fn record_fairness_limited(&self) {
        self.fairness_limited_polls.fetch_add(1, Ordering::Relaxed);
    }

    fn record_slow_consumer_eviction(&self) {
        self.slow_consumer_evictions.fetch_add(1, Ordering::Relaxed);
    }

    async fn record_disconnect_cause(&self, cause: &str) {
        let mut causes = self.recent_disconnect_causes.lock().await;
        causes.push_back(cause.to_string());
        while causes.len() > 32 {
            let _ = causes.pop_front();
        }
    }

    async fn snapshot(&self) -> KhalaDeliveryMetricsSnapshot {
        let active_consumers = self.consumers.lock().await.len();
        let recent_disconnect_causes = self
            .recent_disconnect_causes
            .lock()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        KhalaDeliveryMetricsSnapshot {
            total_polls: self.total_polls.load(Ordering::Relaxed),
            throttled_polls: self.throttled_polls.load(Ordering::Relaxed),
            limited_polls: self.limited_polls.load(Ordering::Relaxed),
            fairness_limited_polls: self.fairness_limited_polls.load(Ordering::Relaxed),
            slow_consumer_evictions: self.slow_consumer_evictions.load(Ordering::Relaxed),
            served_messages: self.served_messages.load(Ordering::Relaxed),
            active_consumers,
            recent_disconnect_causes,
        }
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: String,
    build_sha: String,
    uptime_seconds: i64,
    authority_write_mode: String,
    authority_writer_active: bool,
    fanout_driver: String,
    db_configured: bool,
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: &'static str,
    authority_ready: bool,
    projector_ready: bool,
    workers_ready: bool,
    authority_writer_active: bool,
    fanout_driver: String,
}

#[derive(Debug, Deserialize)]
struct StartRunBody {
    worker_id: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AppendRunEventBody {
    event_type: String,
    #[serde(default)]
    payload: serde_json::Value,
    idempotency_key: Option<String>,
    expected_previous_seq: Option<u64>,
}

#[derive(Debug, Serialize)]
struct RunResponse {
    run: RuntimeRun,
}

#[derive(Debug, Serialize)]
struct WorkerResponse {
    worker: WorkerSnapshot,
}

#[derive(Debug, Serialize)]
struct CheckpointResponse {
    checkpoint: ProjectionCheckpoint,
}

#[derive(Debug, Serialize)]
struct DriftResponse {
    drift: ProjectionDriftReport,
}

#[derive(Debug, Serialize)]
struct RunSummaryResponse {
    summary: RunProjectionSummary,
}

#[derive(Debug, Deserialize)]
struct FanoutPollQuery {
    after_seq: Option<u64>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct FanoutPollResponse {
    topic: String,
    driver: String,
    messages: Vec<FanoutMessage>,
    oldest_available_cursor: Option<u64>,
    head_cursor: Option<u64>,
    queue_depth: Option<usize>,
    dropped_messages: Option<u64>,
    next_cursor: u64,
    replay_complete: bool,
    limit_applied: usize,
    limit_capped: bool,
    fairness_applied: bool,
    active_topic_count: usize,
    outbound_queue_limit: usize,
    consumer_lag: Option<u64>,
    slow_consumer_strikes: u32,
    slow_consumer_max_strikes: u32,
    recommended_reconnect_backoff_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum KhalaWsFrame {
    Hello {
        topic: String,
        after_seq: u64,
        limit: usize,
        recommended_reconnect_backoff_ms: u64,
    },
    Message {
        message: FanoutMessage,
    },
    StaleCursor {
        topic: String,
        requested_cursor: u64,
        oldest_available_cursor: u64,
        head_cursor: u64,
        reason_codes: Vec<String>,
        replay_lag: u64,
        replay_budget_events: u64,
        qos_tier: String,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Debug, Serialize)]
struct FanoutHooksResponse {
    driver: String,
    hooks: Vec<ExternalFanoutHook>,
    delivery_metrics: KhalaDeliveryMetricsSnapshot,
    topic_windows: Vec<FanoutTopicWindow>,
}

#[derive(Debug, Serialize)]
struct FanoutMetricsResponse {
    driver: String,
    delivery_metrics: KhalaDeliveryMetricsSnapshot,
    topic_windows: Vec<FanoutTopicWindow>,
}

#[derive(Debug, Deserialize)]
struct FanoutMetricsQuery {
    topic_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OwnerQuery {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
}

#[derive(Debug, Serialize)]
struct WorkersListResponse {
    workers: Vec<WorkerSnapshot>,
}

#[derive(Debug, Deserialize)]
struct ProviderCatalogQuery {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    capability: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ComputeTelemetryQuery {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    #[serde(default)]
    capability: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FraudIncidentsQuery {
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct FraudIncidentsResponse {
    schema: String,
    incidents: Vec<crate::fraud::FraudIncident>,
}

#[derive(Debug, Serialize)]
struct ProviderCatalogResponse {
    providers: Vec<ProviderCatalogEntry>,
}

#[derive(Debug, Serialize)]
struct JobTypesResponse {
    job_types: Vec<protocol::jobs::JobTypeInfo>,
}

#[derive(Debug, Deserialize)]
struct RouteProviderBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    capability: String,
}

#[derive(Debug, Deserialize)]
struct QuoteSandboxRunBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    request: protocol::SandboxRunRequest,
}

#[derive(Debug, Serialize)]
struct QuoteSandboxRunResponse {
    schema: String,
    job_hash: String,
    capability: String,
    selection: ProviderSelection,
    quote: ComputeAllInQuoteV1,
}

#[derive(Debug, Deserialize)]
struct RouterCandidateQuoteV1 {
    marketplace_id: String,
    provider_id: String,
    #[serde(default)]
    provider_worker_id: Option<String>,
    total_price_msats: u64,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    latency_ms: Option<u64>,
    #[serde(default)]
    reliability_bps: Option<u32>,
    #[serde(default)]
    constraints: serde_json::Value,
    #[serde(default)]
    quote_id: Option<String>,
    #[serde(default)]
    quote_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct NormalizedCandidateQuoteV1 {
    marketplace_id: String,
    provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_worker_id: Option<String>,
    total_price_msats: u64,
    latency_ms: Option<u64>,
    reliability_bps: u32,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    constraints: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    quote_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quote_sha256: Option<String>,
}

const ROUTER_POLICY_BALANCED_LATENCY_PENALTY_MSATS_PER_MS: u64 = 1;
const ROUTER_POLICY_BALANCED_RELIABILITY_PENALTY_MSATS_PER_100_BPS: u64 = 20;
const ROUTER_POLICY_BALANCED_DEFAULT_LATENCY_MS: u64 = 250;
const HYDRA_ROUTING_DECISION_RECEIPT_SCHEMA_V1: &str =
    "openagents.hydra.routing_decision_receipt.v1";
const HYDRA_RISK_HEALTH_RESPONSE_SCHEMA_V1: &str = "openagents.hydra.risk_health_response.v1";
const HYDRA_ROUTE_PROVIDER_DIRECT: &str = "route-direct";
const HYDRA_ROUTE_PROVIDER_CEP: &str = "route-cep";

#[derive(Debug, Deserialize)]
struct RouterSelectComputeBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    run_id: Uuid,
    capability: String,
    #[serde(default)]
    objective_hash: Option<String>,
    #[serde(default)]
    marketplace_id: Option<String>,
    candidates: Vec<RouterCandidateQuoteV1>,
    #[serde(default)]
    policy: Option<String>,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default)]
    decided_at_unix: Option<u64>,
}

#[derive(Debug, Serialize)]
struct RouterSelectComputeResponse {
    schema: String,
    decision_sha256: String,
    policy: String,
    run_id: String,
    capability: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    objective_hash: Option<String>,
    selected: NormalizedCandidateQuoteV1,
    candidates: Vec<NormalizedCandidateQuoteV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nostr_event: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct HydraRiskCreditBreakersV1 {
    halt_new_envelopes: bool,
    halt_large_settlements: bool,
}

#[derive(Debug, Clone, Serialize)]
struct HydraRiskLiquidityStateV1 {
    wallet_executor_configured: bool,
    wallet_executor_reachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    withdraw_throttle_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    withdraw_throttle_reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    withdraw_throttle_extra_delay_hours: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    withdraw_throttle_execution_cap_per_tick: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
struct HydraRiskRoutingStateV1 {
    degraded: bool,
    direct_disabled: bool,
    #[serde(default)]
    reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct HydraRiskHealthResponseV1 {
    schema: String,
    generated_at: chrono::DateTime<Utc>,
    credit_breakers: HydraRiskCreditBreakersV1,
    liquidity: HydraRiskLiquidityStateV1,
    routing: HydraRiskRoutingStateV1,
}

#[derive(Debug, Deserialize)]
struct DispatchSandboxRunBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    request: protocol::SandboxRunRequest,
}

#[derive(Debug, Serialize)]
struct DispatchSandboxRunResponse {
    job_hash: String,
    selection: ProviderSelection,
    response: protocol::SandboxRunResponse,
    latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_from_provider_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SandboxVerificationBody {
    request: protocol::SandboxRunRequest,
    response: protocol::SandboxRunResponse,
}

#[derive(Debug, Serialize)]
struct SandboxVerificationResponse {
    passed: bool,
    exit_code: i32,
    violations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RepoIndexVerificationBody {
    request: protocol::RepoIndexRequest,
    response: protocol::RepoIndexResponse,
}

#[derive(Debug, Serialize)]
struct RepoIndexVerificationResponse {
    passed: bool,
    tree_sha256: String,
    violations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SettleSandboxRunBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    run_id: Uuid,
    provider_id: String,
    provider_worker_id: String,
    amount_msats: u64,
    #[serde(default)]
    quote: Option<ComputeAllInQuoteV1>,
    #[serde(default)]
    route_policy: Option<SettleSandboxRunRoutePolicyV1>,
    #[serde(default)]
    cep: Option<SettleSandboxRunCepRouteV1>,
    request: protocol::SandboxRunRequest,
    response: protocol::SandboxRunResponse,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum SettleSandboxRunRoutePolicyV1 {
    DirectOnly,
    ForceCep,
    PreferAgentBalance {
        agent_balance_sats: u64,
        min_reserve_sats: u64,
        #[serde(default = "default_true")]
        direct_allowed: bool,
    },
}

impl Default for SettleSandboxRunRoutePolicyV1 {
    fn default() -> Self {
        Self::DirectOnly
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SettleSandboxRunCepRouteV1 {
    agent_id: String,
    pool_id: String,
    #[serde(default)]
    scope_id: Option<String>,
    provider_invoice: String,
    provider_host: String,
    #[serde(default)]
    max_fee_msats: Option<u64>,
    #[serde(default)]
    offer_ttl_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
struct SettleSandboxRunResponse {
    job_hash: String,
    reservation_id: String,
    amount_msats: u64,
    verification_passed: bool,
    exit_code: i32,
    #[serde(default)]
    violations: Vec<String>,
    settlement_status: String,
    settlement_route: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    credit_offer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credit_envelope_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credit_settlement_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credit_liquidity_receipt_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_receipt_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CommsDeliveryEventRequest {
    event_id: String,
    provider: String,
    delivery_state: String,
    #[serde(default)]
    message_id: Option<String>,
    #[serde(default)]
    integration_id: Option<String>,
    #[serde(default)]
    recipient: Option<String>,
    #[serde(default)]
    occurred_at: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
struct CommsDeliveryAccepted {
    event_id: String,
    status: String,
    #[serde(rename = "idempotentReplay")]
    idempotent_replay: bool,
}

#[derive(Debug, Deserialize)]
struct DriftQuery {
    topic: String,
}

#[derive(Debug, Deserialize)]
struct RegisterWorkerBody {
    worker_id: Option<String>,
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    workspace_ref: Option<String>,
    codex_home_ref: Option<String>,
    adapter: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct WorkerHeartbeatBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    #[serde(default)]
    metadata_patch: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct WorkerTransitionBody {
    owner_user_id: Option<u64>,
    owner_guest_scope: Option<String>,
    status: WorkerStatus,
    reason: Option<String>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(readiness))
        .route("/internal/v1/openapi.json", get(internal_openapi_spec))
        .route(
            "/internal/v1/comms/delivery-events",
            post(record_comms_delivery_event),
        )
        .route("/internal/v1/runs", post(start_run))
        .route("/internal/v1/runs/:run_id", get(get_run))
        .route("/internal/v1/runs/:run_id/events", post(append_run_event))
        .route("/internal/v1/runs/:run_id/receipt", get(get_run_receipt))
        .route("/internal/v1/runs/:run_id/replay", get(get_run_replay))
        .route(
            "/internal/v1/khala/topics/:topic/messages",
            get(get_khala_topic_messages),
        )
        .route(
            "/internal/v1/khala/topics/:topic/ws",
            get(get_khala_topic_ws),
        )
        .route(
            "/internal/v1/khala/fanout/hooks",
            get(get_khala_fanout_hooks),
        )
        .route(
            "/internal/v1/khala/fanout/metrics",
            get(get_khala_fanout_metrics),
        )
        .route(
            "/internal/v1/projectors/checkpoints/:run_id",
            get(get_run_checkpoint),
        )
        .route("/internal/v1/projectors/drift", get(get_projector_drift))
        .route(
            "/internal/v1/projectors/run-summary/:run_id",
            get(get_projector_run_summary),
        )
        .route(
            "/internal/v1/workers",
            get(list_workers).post(register_worker),
        )
        .route("/internal/v1/workers/:worker_id", get(get_worker))
        .route(
            "/internal/v1/workers/:worker_id/heartbeat",
            post(heartbeat_worker),
        )
        .route(
            "/internal/v1/workers/:worker_id/status",
            post(transition_worker),
        )
        .route(
            "/internal/v1/workers/:worker_id/checkpoint",
            get(get_worker_checkpoint),
        )
        .route(
            "/internal/v1/marketplace/catalog/providers",
            get(get_provider_catalog),
        )
        .route(
            "/internal/v1/marketplace/catalog/job-types",
            get(get_job_types),
        )
        .route(
            "/internal/v1/marketplace/telemetry/compute",
            get(get_compute_telemetry),
        )
        .route(
            "/internal/v1/marketplace/route/provider",
            post(route_provider),
        )
        .route(
            "/internal/v1/marketplace/compute/quote/sandbox-run",
            post(quote_sandbox_run),
        )
        .route(
            "/internal/v1/marketplace/router/compute/select",
            post(router_select_compute),
        )
        .route(
            "/internal/v1/hydra/routing/score",
            post(hydra_routing_score),
        )
        .route("/internal/v1/hydra/fx/rfq", post(hydra_fx_rfq_create))
        .route("/internal/v1/hydra/fx/quote", post(hydra_fx_quote_upsert))
        .route("/internal/v1/hydra/fx/select", post(hydra_fx_select))
        .route("/internal/v1/hydra/fx/rfq/:rfq_id", get(hydra_fx_rfq_get))
        .route("/internal/v1/hydra/risk/health", get(hydra_risk_health))
        .route("/internal/v1/hydra/observability", get(hydra_observability))
        .route(
            "/internal/v1/marketplace/dispatch/sandbox-run",
            post(dispatch_sandbox_run),
        )
        .route(
            "/internal/v1/verifications/sandbox-run",
            post(verify_sandbox_run),
        )
        .route(
            "/internal/v1/verifications/repo-index",
            post(verify_repo_index),
        )
        .route(
            "/internal/v1/treasury/compute/summary",
            get(get_compute_treasury_summary),
        )
        .route(
            "/internal/v1/treasury/compute/reconcile",
            post(reconcile_compute_treasury),
        )
        .route(
            "/internal/v1/treasury/compute/settle/sandbox-run",
            post(settle_sandbox_run),
        )
        .route(
            "/internal/v1/liquidity/quote_pay",
            post(liquidity_quote_pay),
        )
        .route("/internal/v1/credit/intent", post(credit_intent))
        .route("/internal/v1/credit/offer", post(credit_offer))
        .route("/internal/v1/credit/envelope", post(credit_envelope))
        .route("/internal/v1/credit/settle", post(credit_settle))
        .route("/internal/v1/credit/health", get(credit_health))
        .route(
            "/internal/v1/credit/agents/:agent_id/exposure",
            get(credit_agent_exposure),
        )
        .route("/internal/v1/liquidity/status", get(liquidity_status))
        .route("/internal/v1/liquidity/pay", post(liquidity_pay))
        .route(
            "/internal/v1/pools/:pool_id/admin/create",
            post(liquidity_pool_create_pool),
        )
        .route(
            "/internal/v1/pools/:pool_id/deposit_quote",
            post(liquidity_pool_deposit_quote),
        )
        .route(
            "/internal/v1/pools/:pool_id/deposits/:deposit_id/confirm",
            post(liquidity_pool_confirm_deposit),
        )
        .route(
            "/internal/v1/pools/:pool_id/withdraw_request",
            post(liquidity_pool_withdraw_request),
        )
        .route(
            "/internal/v1/pools/:pool_id/status",
            get(liquidity_pool_status),
        )
        .route(
            "/internal/v1/pools/:pool_id/snapshots/latest",
            get(liquidity_pool_latest_snapshot),
        )
        .route("/internal/v1/fraud/incidents", get(get_fraud_incidents))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime_seconds = (Utc::now() - state.started_at).num_seconds();
    Json(HealthResponse {
        status: "ok",
        service: state.config.service_name,
        build_sha: state.config.build_sha,
        uptime_seconds,
        authority_write_mode: state.config.authority_write_mode.as_str().to_string(),
        authority_writer_active: state.config.authority_write_mode.writes_enabled(),
        fanout_driver: state.fanout.driver_name().to_string(),
        db_configured: state.db.is_some(),
    })
}

async fn internal_openapi_spec() -> Result<Json<serde_json::Value>, ApiError> {
    let yaml = include_str!("../docs/openapi-internal-v1.yaml");
    let value = serde_yaml::from_str::<serde_yaml::Value>(yaml)
        .map_err(|error| ApiError::Internal(error.to_string()))?;
    let json_value =
        serde_json::to_value(value).map_err(|error| ApiError::Internal(error.to_string()))?;
    Ok(Json(json_value))
}

async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let runtime_readiness = state.orchestrator.readiness();
    let workers_ready = state.workers.is_ready();
    let ready = runtime_readiness.is_ready() && workers_ready;
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(ReadinessResponse {
            status: if ready { "ready" } else { "not_ready" },
            authority_ready: runtime_readiness.authority_ready,
            projector_ready: runtime_readiness.projector_ready,
            workers_ready,
            authority_writer_active: state.config.authority_write_mode.writes_enabled(),
            fanout_driver: state.fanout.driver_name().to_string(),
        }),
    )
}

async fn record_comms_delivery_event(
    State(state): State<AppState>,
    Json(body): Json<CommsDeliveryEventRequest>,
) -> Result<(StatusCode, Json<CommsDeliveryAccepted>), ApiError> {
    let event_id = body.event_id.trim().to_string();
    if event_id.is_empty() {
        return Err(ApiError::InvalidRequest("event_id is required".to_string()));
    }
    let provider = body.provider.trim().to_ascii_lowercase();
    if provider.is_empty() {
        return Err(ApiError::InvalidRequest("provider is required".to_string()));
    }
    let delivery_state = body.delivery_state.trim().to_ascii_lowercase();
    if delivery_state.is_empty() {
        return Err(ApiError::InvalidRequest(
            "delivery_state is required".to_string(),
        ));
    }
    if body.payload.is_null() {
        return Err(ApiError::InvalidRequest("payload is required".to_string()));
    }

    let key = format!("{provider}::{event_id}");
    let mut guard = state.comms_delivery_events.lock().await;
    if let Some(existing) = guard.get(&key).cloned() {
        return Ok((
            StatusCode::OK,
            Json(CommsDeliveryAccepted {
                idempotent_replay: true,
                ..existing
            }),
        ));
    }

    let accepted = CommsDeliveryAccepted {
        event_id,
        status: "accepted".to_string(),
        idempotent_replay: false,
    };
    guard.insert(key, accepted.clone());
    Ok((StatusCode::ACCEPTED, Json(accepted)))
}

async fn start_run(
    State(state): State<AppState>,
    Json(body): Json<StartRunBody>,
) -> Result<(StatusCode, Json<RunResponse>), ApiError> {
    ensure_runtime_write_authority(&state)?;
    let run = state
        .orchestrator
        .start_run(StartRunRequest {
            worker_id: body.worker_id,
            metadata: body.metadata,
        })
        .await
        .map_err(ApiError::from_orchestration)?;
    publish_latest_run_event(&state, &run).await?;
    Ok((StatusCode::CREATED, Json(RunResponse { run })))
}

async fn append_run_event(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    Json(body): Json<AppendRunEventBody>,
) -> Result<Json<RunResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    if body.event_type.trim() == "payment" {
        return Err(ApiError::InvalidRequest(
            "payment events must be emitted via treasury settlement endpoints".to_string(),
        ));
    }
    let run = state
        .orchestrator
        .append_run_event(
            run_id,
            AppendRunEventRequest {
                event_type: body.event_type,
                payload: body.payload,
                idempotency_key: body.idempotency_key,
                expected_previous_seq: body.expected_previous_seq,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;
    publish_latest_run_event(&state, &run).await?;
    Ok(Json(RunResponse { run }))
}

async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunResponse>, ApiError> {
    let run = state
        .orchestrator
        .get_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(RunResponse { run }))
}

async fn get_run_receipt(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RuntimeReceipt>, ApiError> {
    let run = state
        .orchestrator
        .get_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    let mut receipt = build_receipt(&run).map_err(ApiError::from_artifacts)?;

    let Some(secret_key) = state.config.bridge_nostr_secret_key else {
        if state.config.verifier_strict {
            return Err(ApiError::Internal(
                "receipt signer missing (RUNTIME_VERIFIER_STRICT=true)".to_string(),
            ));
        }
        return Ok(Json(receipt));
    };

    let signature =
        crate::artifacts::sign_receipt_sha256(&secret_key, receipt.canonical_json_sha256.as_str())
            .map_err(ApiError::from_artifacts)?;
    if !state.config.verifier_allowed_signer_pubkeys.is_empty()
        && !state
            .config
            .verifier_allowed_signer_pubkeys
            .contains(signature.signer_pubkey.as_str())
    {
        return Err(ApiError::Internal(
            "receipt signer pubkey is not in active key graph".to_string(),
        ));
    }
    if !crate::artifacts::verify_receipt_signature(&signature).map_err(ApiError::from_artifacts)? {
        return Err(ApiError::Internal(
            "receipt signature verification failed".to_string(),
        ));
    }

    for payment in &mut receipt.payments {
        let payment_signature = crate::artifacts::sign_receipt_sha256(
            &secret_key,
            payment.canonical_json_sha256.as_str(),
        )
        .map_err(ApiError::from_artifacts)?;
        if !state.config.verifier_allowed_signer_pubkeys.is_empty()
            && !state
                .config
                .verifier_allowed_signer_pubkeys
                .contains(payment_signature.signer_pubkey.as_str())
        {
            return Err(ApiError::Internal(
                "payment receipt signer pubkey is not in active key graph".to_string(),
            ));
        }
        if !crate::artifacts::verify_receipt_signature(&payment_signature)
            .map_err(ApiError::from_artifacts)?
        {
            return Err(ApiError::Internal(
                "payment receipt signature verification failed".to_string(),
            ));
        }
        payment.signature = Some(payment_signature);
    }

    receipt.signature = Some(signature);
    Ok(Json(receipt))
}

async fn verify_contract_critical_run_receipt(
    state: &AppState,
    run_id: Uuid,
) -> Result<(), ApiError> {
    let run = state
        .orchestrator
        .get_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    let receipt = build_receipt(&run).map_err(ApiError::from_artifacts)?;

    let Some(secret_key) = state.config.bridge_nostr_secret_key else {
        if state.config.verifier_strict {
            return Err(ApiError::Internal(
                "receipt signer missing (RUNTIME_VERIFIER_STRICT=true)".to_string(),
            ));
        }
        return Ok(());
    };

    let signature =
        crate::artifacts::sign_receipt_sha256(&secret_key, receipt.canonical_json_sha256.as_str())
            .map_err(ApiError::from_artifacts)?;
    if !state.config.verifier_allowed_signer_pubkeys.is_empty()
        && !state
            .config
            .verifier_allowed_signer_pubkeys
            .contains(signature.signer_pubkey.as_str())
    {
        return Err(ApiError::Internal(
            "receipt signer pubkey is not in active key graph".to_string(),
        ));
    }
    if !crate::artifacts::verify_receipt_signature(&signature).map_err(ApiError::from_artifacts)? {
        return Err(ApiError::Internal(
            "receipt signature verification failed".to_string(),
        ));
    }

    for payment in &receipt.payments {
        let payment_signature = crate::artifacts::sign_receipt_sha256(
            &secret_key,
            payment.canonical_json_sha256.as_str(),
        )
        .map_err(ApiError::from_artifacts)?;
        if !state.config.verifier_allowed_signer_pubkeys.is_empty()
            && !state
                .config
                .verifier_allowed_signer_pubkeys
                .contains(payment_signature.signer_pubkey.as_str())
        {
            return Err(ApiError::Internal(
                "payment receipt signer pubkey is not in active key graph".to_string(),
            ));
        }
        if !crate::artifacts::verify_receipt_signature(&payment_signature)
            .map_err(ApiError::from_artifacts)?
        {
            return Err(ApiError::Internal(
                "payment receipt signature verification failed".to_string(),
            ));
        }
    }

    Ok(())
}

async fn get_run_replay(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let run = state
        .orchestrator
        .get_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    let replay = build_replay_jsonl(&run).map_err(ApiError::from_artifacts)?;
    Ok(([(header::CONTENT_TYPE, "application/x-ndjson")], replay))
}

async fn get_khala_topic_messages(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Query(query): Query<FanoutPollQuery>,
) -> Result<Json<FanoutPollResponse>, ApiError> {
    if topic.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "topic is required for khala fanout polling".to_string(),
        ));
    }
    enforce_khala_origin_policy(&state, &headers)?;
    let principal = authorize_khala_topic_access(&state, &headers, &topic).await?;
    let after_seq = query.after_seq.unwrap_or(0);
    let requested_limit = query
        .limit
        .unwrap_or(state.config.khala_poll_default_limit)
        .max(1);
    let principal_key = khala_principal_key(&principal);
    let active_topic_count = {
        let prefix = format!("{principal_key}|");
        let consumers = state.khala_delivery.consumers.lock().await;
        consumers
            .keys()
            .filter(|key| key.starts_with(prefix.as_str()))
            .count()
    };
    let mut limit = requested_limit
        .min(state.config.khala_poll_max_limit)
        .min(state.config.khala_outbound_queue_limit);
    let mut fairness_applied = false;
    if active_topic_count >= 2 && limit > state.config.khala_fair_topic_slice_limit {
        limit = state.config.khala_fair_topic_slice_limit;
        fairness_applied = true;
        state.khala_delivery.record_fairness_limited();
    }
    let limit_capped = requested_limit > limit;
    if limit_capped {
        state.khala_delivery.record_limit_capped();
    }
    let window = state
        .fanout
        .topic_window(&topic)
        .await
        .map_err(ApiError::from_fanout)?;
    let (oldest_available_cursor, head_cursor, queue_depth, dropped_messages) =
        fanout_window_details(window.as_ref());
    let consumer_lag = head_cursor.map(|head| head.saturating_sub(after_seq));
    let consumer_key = khala_consumer_key(&principal, topic.as_str());
    let now = Utc::now();
    let jitter_ms = deterministic_jitter_ms(
        consumer_key.as_str(),
        after_seq,
        state.config.khala_reconnect_jitter_ms,
    );
    let reconnect_backoff_ms = state
        .config
        .khala_reconnect_base_backoff_ms
        .saturating_add(jitter_ms);

    let slow_consumer_strikes = {
        let mut consumers = state.khala_delivery.consumers.lock().await;
        if !consumers.contains_key(&consumer_key)
            && consumers.len() >= state.config.khala_consumer_registry_capacity
            && let Some(oldest_key) = consumers
                .iter()
                .min_by_key(|(_, value)| value.last_poll_at)
                .map(|(key, _)| key.clone())
        {
            let _ = consumers.remove(&oldest_key);
        }
        let consumer_state = consumers.entry(consumer_key.clone()).or_default();
        if let Some(last_poll_at) = consumer_state.last_poll_at {
            let elapsed_ms = now
                .signed_duration_since(last_poll_at)
                .num_milliseconds()
                .max(0) as u64;
            if elapsed_ms < state.config.khala_poll_min_interval_ms {
                consumer_state.last_poll_at = Some(now);
                drop(consumers);
                state.khala_delivery.record_throttled_poll();
                state
                    .khala_delivery
                    .record_disconnect_cause("rate_limited")
                    .await;
                let retry_after_ms = state
                    .config
                    .khala_poll_min_interval_ms
                    .saturating_sub(elapsed_ms)
                    .saturating_add(jitter_ms);
                return Err(ApiError::RateLimited {
                    retry_after_ms,
                    reason_code: "poll_interval_guard".to_string(),
                });
            }
        }

        let lag = consumer_lag.unwrap_or(0);
        if lag > state.config.khala_slow_consumer_lag_threshold {
            consumer_state.slow_consumer_strikes =
                consumer_state.slow_consumer_strikes.saturating_add(1);
        } else {
            consumer_state.slow_consumer_strikes = 0;
        }
        if consumer_state.slow_consumer_strikes >= state.config.khala_slow_consumer_max_strikes {
            let strikes = consumer_state.slow_consumer_strikes;
            let _ = consumers.remove(&consumer_key);
            drop(consumers);
            state.khala_delivery.record_slow_consumer_eviction();
            state
                .khala_delivery
                .record_disconnect_cause("slow_consumer_evicted")
                .await;
            return Err(ApiError::SlowConsumerEvicted {
                topic: topic.clone(),
                lag,
                lag_threshold: state.config.khala_slow_consumer_lag_threshold,
                strikes,
                max_strikes: state.config.khala_slow_consumer_max_strikes,
                suggested_after_seq: oldest_available_cursor,
            });
        }

        consumer_state.last_poll_at = Some(now);
        consumer_state.slow_consumer_strikes
    };

    let messages = state
        .fanout
        .poll(&topic, after_seq, limit)
        .await
        .map_err(ApiError::from_fanout)?;
    state.khala_delivery.record_total_poll(messages.len());
    let next_cursor = messages
        .last()
        .map_or(after_seq, |message| message.sequence);
    {
        let mut consumers = state.khala_delivery.consumers.lock().await;
        if let Some(consumer_state) = consumers.get_mut(&consumer_key) {
            consumer_state.last_cursor = next_cursor;
            consumer_state.last_poll_at = Some(now);
        }
    }
    let replay_complete = head_cursor.map_or(true, |head| next_cursor >= head);
    Ok(Json(FanoutPollResponse {
        topic,
        driver: state.fanout.driver_name().to_string(),
        messages,
        oldest_available_cursor,
        head_cursor,
        queue_depth,
        dropped_messages,
        next_cursor,
        replay_complete,
        limit_applied: limit,
        limit_capped,
        fairness_applied,
        active_topic_count,
        outbound_queue_limit: state.config.khala_outbound_queue_limit,
        consumer_lag,
        slow_consumer_strikes,
        slow_consumer_max_strikes: state.config.khala_slow_consumer_max_strikes,
        recommended_reconnect_backoff_ms: reconnect_backoff_ms,
    }))
}

async fn get_khala_topic_ws(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(topic): Path<String>,
    Query(query): Query<FanoutPollQuery>,
) -> Result<impl IntoResponse, ApiError> {
    if topic.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "topic is required for khala websocket".to_string(),
        ));
    }
    enforce_khala_origin_policy(&state, &headers)?;
    let principal = authorize_khala_topic_access(&state, &headers, &topic).await?;
    let after_seq = query.after_seq.unwrap_or(0);
    let requested_limit = query
        .limit
        .unwrap_or(state.config.khala_poll_default_limit)
        .max(1);

    let state_for_socket = state.clone();
    let topic_for_socket = topic.clone();
    Ok(ws.on_upgrade(move |socket| {
        khala_ws_stream(
            state_for_socket,
            socket,
            principal,
            topic_for_socket,
            after_seq,
            requested_limit,
        )
    }))
}

async fn khala_ws_stream(
    state: AppState,
    mut socket: WebSocket,
    principal: SyncPrincipal,
    topic: String,
    mut after_seq: u64,
    requested_limit: usize,
) {
    let principal_key = khala_principal_key(&principal);
    let consumer_key = khala_consumer_key(&principal, topic.as_str());
    {
        let mut consumers = state.khala_delivery.consumers.lock().await;
        if !consumers.contains_key(&consumer_key)
            && consumers.len() >= state.config.khala_consumer_registry_capacity
            && let Some(oldest_key) = consumers
                .iter()
                .min_by_key(|(_, value)| value.last_poll_at)
                .map(|(key, _)| key.clone())
        {
            let _ = consumers.remove(&oldest_key);
        }
        consumers.entry(consumer_key.clone()).or_default();
    }

    let jitter_ms = deterministic_jitter_ms(
        consumer_key.as_str(),
        after_seq,
        state.config.khala_reconnect_jitter_ms,
    );
    let reconnect_backoff_ms = state
        .config
        .khala_reconnect_base_backoff_ms
        .saturating_add(jitter_ms);

    let hello = KhalaWsFrame::Hello {
        topic: topic.clone(),
        after_seq,
        limit: requested_limit,
        recommended_reconnect_backoff_ms: reconnect_backoff_ms,
    };
    if let Ok(payload) = serde_json::to_string(&hello) {
        let _ = socket.send(Message::Text(payload)).await;
    }

    let mut slow_consumer_strikes = 0u32;
    let mut last_head_cursor = None::<u64>;

    loop {
        let active_topic_count = {
            let prefix = format!("{principal_key}|");
            let consumers = state.khala_delivery.consumers.lock().await;
            consumers
                .keys()
                .filter(|key| key.starts_with(prefix.as_str()))
                .count()
        };

        let mut limit = requested_limit
            .min(state.config.khala_poll_max_limit)
            .min(state.config.khala_outbound_queue_limit);
        if active_topic_count >= 2 && limit > state.config.khala_fair_topic_slice_limit {
            limit = state.config.khala_fair_topic_slice_limit;
            state.khala_delivery.record_fairness_limited();
        }

        let window = state.fanout.topic_window(&topic).await.ok().flatten();
        let (_oldest_available_cursor, head_cursor, _queue_depth, _dropped_messages) =
            fanout_window_details(window.as_ref());
        if head_cursor != last_head_cursor {
            last_head_cursor = head_cursor;
        }
        let consumer_lag = head_cursor.map(|head| head.saturating_sub(after_seq));

        if consumer_lag.unwrap_or(0) > state.config.khala_slow_consumer_lag_threshold {
            slow_consumer_strikes = slow_consumer_strikes.saturating_add(1);
        } else {
            slow_consumer_strikes = 0;
        }
        if slow_consumer_strikes >= state.config.khala_slow_consumer_max_strikes {
            state.khala_delivery.record_slow_consumer_eviction();
            state
                .khala_delivery
                .record_disconnect_cause("slow_consumer_evicted")
                .await;
            let frame = KhalaWsFrame::Error {
                code: "slow_consumer_evicted".to_string(),
                message: format!(
                    "topic={} lag={} threshold={} strikes={} max_strikes={}",
                    topic,
                    consumer_lag.unwrap_or(0),
                    state.config.khala_slow_consumer_lag_threshold,
                    slow_consumer_strikes,
                    state.config.khala_slow_consumer_max_strikes
                ),
            };
            if let Ok(payload) = serde_json::to_string(&frame) {
                let _ = socket.send(Message::Text(payload)).await;
            }
            break;
        }

        match state.fanout.poll(&topic, after_seq, limit).await {
            Ok(messages) => {
                state.khala_delivery.record_total_poll(messages.len());
                let next_cursor = messages
                    .last()
                    .map_or(after_seq, |message| message.sequence);
                {
                    let mut consumers = state.khala_delivery.consumers.lock().await;
                    if let Some(consumer_state) = consumers.get_mut(&consumer_key) {
                        consumer_state.last_cursor = next_cursor;
                        consumer_state.last_poll_at = Some(Utc::now());
                    }
                }
                after_seq = next_cursor;

                for message in messages {
                    let frame = KhalaWsFrame::Message { message };
                    let Ok(payload) = serde_json::to_string(&frame) else {
                        continue;
                    };
                    if socket.send(Message::Text(payload)).await.is_err() {
                        state
                            .khala_delivery
                            .record_disconnect_cause("send_failed")
                            .await;
                        break;
                    }
                }
            }
            Err(FanoutError::StaleCursor {
                topic: stale_topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
            }) => {
                state
                    .khala_delivery
                    .record_disconnect_cause("stale_cursor")
                    .await;
                let frame = KhalaWsFrame::StaleCursor {
                    topic: stale_topic,
                    requested_cursor,
                    oldest_available_cursor,
                    head_cursor,
                    reason_codes,
                    replay_lag,
                    replay_budget_events,
                    qos_tier,
                };
                if let Ok(payload) = serde_json::to_string(&frame) {
                    let _ = socket.send(Message::Text(payload)).await;
                }
                break;
            }
            Err(error) => {
                state
                    .khala_delivery
                    .record_disconnect_cause("fanout_error")
                    .await;
                let frame = KhalaWsFrame::Error {
                    code: "fanout_error".to_string(),
                    message: error.to_string(),
                };
                if let Ok(payload) = serde_json::to_string(&frame) {
                    let _ = socket.send(Message::Text(payload)).await;
                }
                break;
            }
        }

        tokio::select! {
            biased;
            next = socket.next() => {
                match next {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
    }

    let mut consumers = state.khala_delivery.consumers.lock().await;
    consumers.remove(&consumer_key);
}

async fn get_khala_fanout_hooks(
    State(state): State<AppState>,
) -> Result<Json<FanoutHooksResponse>, ApiError> {
    let delivery_metrics = state.khala_delivery.snapshot().await;
    let topic_windows = state
        .fanout
        .topic_windows(20)
        .await
        .map_err(ApiError::from_fanout)?;
    Ok(Json(FanoutHooksResponse {
        driver: state.fanout.driver_name().to_string(),
        hooks: state.fanout.external_hooks(),
        delivery_metrics,
        topic_windows,
    }))
}

async fn get_khala_fanout_metrics(
    State(state): State<AppState>,
    Query(query): Query<FanoutMetricsQuery>,
) -> Result<Json<FanoutMetricsResponse>, ApiError> {
    let delivery_metrics = state.khala_delivery.snapshot().await;
    let topic_windows = state
        .fanout
        .topic_windows(query.topic_limit.unwrap_or(20))
        .await
        .map_err(ApiError::from_fanout)?;
    Ok(Json(FanoutMetricsResponse {
        driver: state.fanout.driver_name().to_string(),
        delivery_metrics,
        topic_windows,
    }))
}

fn enforce_khala_origin_policy(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if !state.config.khala_enforce_origin {
        return Ok(());
    }
    let Some(origin_header) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let origin = origin_header
        .to_str()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_ascii_lowercase();
    if origin.is_empty() {
        return Ok(());
    }
    if state.config.khala_allowed_origins.contains(&origin) {
        return Ok(());
    }

    tracing::warn!(
        origin = %origin,
        allowed = ?state.config.khala_allowed_origins,
        "khala origin denied by policy"
    );
    Err(ApiError::KhalaOriginDenied(
        "origin_not_allowed".to_string(),
    ))
}

async fn authorize_khala_topic_access(
    state: &AppState,
    headers: &HeaderMap,
    topic: &str,
) -> Result<SyncPrincipal, ApiError> {
    let authorization_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let token = SyncAuthorizer::extract_bearer_token(authorization_header)
        .map_err(ApiError::from_sync_auth)?;
    let principal = state
        .sync_auth
        .authenticate(token)
        .map_err(ApiError::from_sync_auth)?;
    let authorized_topic = state
        .sync_auth
        .authorize_topic(&principal, topic)
        .map_err(ApiError::from_sync_auth)?;

    match authorized_topic {
        AuthorizedKhalaTopic::WorkerLifecycle { worker_id } => {
            let owner = WorkerOwner {
                user_id: principal.user_id,
                guest_scope: if principal.user_id.is_some() {
                    None
                } else {
                    principal.org_id.clone()
                },
            };
            match state.workers.get_worker(&worker_id, &owner).await {
                Ok(_) => {}
                Err(WorkerError::NotFound(_)) | Err(WorkerError::Forbidden(_)) => {
                    tracing::warn!(
                        topic,
                        worker_id,
                        user_id = principal.user_id,
                        org_id = ?principal.org_id,
                        device_id = ?principal.device_id,
                        "khala auth denied: worker owner mismatch"
                    );
                    return Err(ApiError::KhalaForbiddenTopic("owner_mismatch".to_string()));
                }
                Err(error) => {
                    tracing::warn!(
                        topic,
                        worker_id,
                        user_id = principal.user_id,
                        org_id = ?principal.org_id,
                        device_id = ?principal.device_id,
                        reason = %error,
                        "khala auth denied while validating worker ownership"
                    );
                    return Err(ApiError::KhalaForbiddenTopic(error.to_string()));
                }
            }
        }
        AuthorizedKhalaTopic::FleetWorkers { .. }
        | AuthorizedKhalaTopic::RunEvents { .. }
        | AuthorizedKhalaTopic::CodexWorkerEvents => {}
    }

    Ok(principal)
}

async fn get_run_checkpoint(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    let checkpoint = state
        .orchestrator
        .checkpoint_for_run(run_id)
        .await
        .map_err(ApiError::from_orchestration)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(CheckpointResponse { checkpoint }))
}

async fn get_projector_drift(
    State(state): State<AppState>,
    Query(query): Query<DriftQuery>,
) -> Result<Json<DriftResponse>, ApiError> {
    if query.topic.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "topic is required for drift lookup".to_string(),
        ));
    }

    let drift = state
        .orchestrator
        .projectors()
        .drift_for_topic(&query.topic)
        .await
        .map_err(|error| ApiError::Internal(error.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(DriftResponse { drift }))
}

async fn get_projector_run_summary(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunSummaryResponse>, ApiError> {
    let summary = state
        .orchestrator
        .projectors()
        .run_summary(run_id)
        .await
        .map_err(|error| ApiError::Internal(error.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(RunSummaryResponse { summary }))
}

async fn register_worker(
    State(state): State<AppState>,
    Json(body): Json<RegisterWorkerBody>,
) -> Result<(StatusCode, Json<WorkerResponse>), ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let mut metadata = body.metadata;
    if metadata_has_role(&metadata, "provider") {
        qualify_provider_metadata(&metadata).await?;
        annotate_provider_metadata(&mut metadata);
    }
    let snapshot = state
        .workers
        .register_worker(RegisterWorkerRequest {
            worker_id: body.worker_id,
            owner,
            workspace_ref: body.workspace_ref,
            codex_home_ref: body.codex_home_ref,
            adapter: body.adapter,
            metadata,
        })
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(&state, &snapshot).await?;
    maybe_spawn_nostr_provider_ad_mirror(&state, &snapshot);
    Ok((
        StatusCode::CREATED,
        Json(WorkerResponse { worker: snapshot }),
    ))
}

async fn get_worker(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Query(query): Query<OwnerQuery>,
) -> Result<Json<WorkerResponse>, ApiError> {
    let owner = owner_from_parts(query.owner_user_id, query.owner_guest_scope)?;
    let snapshot = state
        .workers
        .get_worker(&worker_id, &owner)
        .await
        .map_err(ApiError::from_worker)?;
    Ok(Json(WorkerResponse { worker: snapshot }))
}

async fn list_workers(
    State(state): State<AppState>,
    Query(query): Query<OwnerQuery>,
) -> Result<Json<WorkersListResponse>, ApiError> {
    let owner = owner_from_parts(query.owner_user_id, query.owner_guest_scope)?;
    let workers = state
        .workers
        .list_workers(&owner)
        .await
        .map_err(ApiError::from_worker)?;
    Ok(Json(WorkersListResponse { workers }))
}

async fn get_provider_catalog(
    State(state): State<AppState>,
    Query(query): Query<ProviderCatalogQuery>,
) -> Result<Json<ProviderCatalogResponse>, ApiError> {
    let guest_scope = query.owner_guest_scope.clone().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let owner_filter = match (query.owner_user_id, guest_scope) {
        (None, None) => None,
        (user_id, guest_scope) => Some(owner_from_parts(user_id, guest_scope)?),
    };

    let workers = match owner_filter.as_ref() {
        Some(owner) => state
            .workers
            .list_workers(owner)
            .await
            .map_err(ApiError::from_worker)?,
        None => state.workers.list_all_workers().await,
    };

    let mut providers = build_provider_catalog(&workers);
    if let Some(capability) = query
        .capability
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        providers.retain(|provider| provider.capabilities.iter().any(|cap| cap == capability));
    }

    Ok(Json(ProviderCatalogResponse { providers }))
}

async fn route_provider(
    State(state): State<AppState>,
    Json(body): Json<RouteProviderBody>,
) -> Result<Json<ProviderSelection>, ApiError> {
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let workers = state.workers.list_all_workers().await;
    let selection = select_provider_for_capability(&workers, Some(&owner), &body.capability)
        .ok_or(ApiError::NotFound)?;
    Ok(Json(selection))
}

async fn quote_sandbox_run(
    State(state): State<AppState>,
    Json(body): Json<QuoteSandboxRunBody>,
) -> Result<Json<QuoteSandboxRunResponse>, ApiError> {
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    state
        .compute_abuse
        .enforce_dispatch_rate(owner_key.as_str())
        .await?;

    validate_sandbox_request_phase0(&body.request)?;
    let job_hash = protocol::hash::canonical_hash(&body.request)
        .map_err(|error| ApiError::InvalidRequest(format!("invalid sandbox request: {error}")))?;

    let workers = state.workers.list_all_workers().await;
    let selection =
        select_provider_for_capability(&workers, Some(&owner), PHASE0_REQUIRED_PROVIDER_CAPABILITY)
            .ok_or(ApiError::NotFound)?;

    let issued_at_unix = Utc::now().timestamp().max(0) as u64;
    let quote = compute_all_in_quote_v1(
        &selection.provider,
        PHASE0_REQUIRED_PROVIDER_CAPABILITY,
        job_hash.as_str(),
        issued_at_unix,
    )
    .ok_or_else(|| ApiError::Internal("unable to compute all-in quote".to_string()))?;

    Ok(Json(QuoteSandboxRunResponse {
        schema: "openagents.marketplace.compute_quote.v1".to_string(),
        job_hash,
        capability: PHASE0_REQUIRED_PROVIDER_CAPABILITY.to_string(),
        selection,
        quote,
    }))
}

async fn router_select_compute(
    State(state): State<AppState>,
    Json(body): Json<RouterSelectComputeBody>,
) -> Result<Json<RouterSelectComputeResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);

    let capability = body.capability.trim();
    if capability.is_empty() {
        return Err(ApiError::InvalidRequest(
            "capability must not be empty".to_string(),
        ));
    }
    if body.candidates.is_empty() {
        return Err(ApiError::InvalidRequest(
            "candidates must not be empty".to_string(),
        ));
    }

    let policy = body
        .policy
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("lowest_total_cost_v1")
        .to_string();
    if !matches!(
        policy.as_str(),
        "lowest_total_cost_v1" | "balanced_v1" | "reliability_first_v1"
    ) {
        return Err(ApiError::InvalidRequest(format!(
            "unsupported policy: {policy}"
        )));
    }

    let objective_hash = body
        .objective_hash
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let decided_at_unix = body
        .decided_at_unix
        .filter(|value| *value > 0)
        .unwrap_or_else(|| Utc::now().timestamp().max(0) as u64);

    let marketplace_id = body
        .marketplace_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("openagents")
        .to_string();

    let mut candidates = Vec::with_capacity(body.candidates.len());
    for candidate in &body.candidates {
        if candidate.marketplace_id.trim().is_empty() {
            return Err(ApiError::InvalidRequest(
                "candidate.marketplace_id must not be empty".to_string(),
            ));
        }
        if candidate.provider_id.trim().is_empty() {
            return Err(ApiError::InvalidRequest(
                "candidate.provider_id must not be empty".to_string(),
            ));
        }
        if candidate.total_price_msats == 0 {
            return Err(ApiError::InvalidRequest(
                "candidate.total_price_msats must be greater than zero".to_string(),
            ));
        }
        let currency = candidate.currency.as_deref().unwrap_or("msats").trim();
        if currency != "msats" {
            return Err(ApiError::InvalidRequest(format!(
                "unsupported currency: {currency}"
            )));
        }

        candidates.push(NormalizedCandidateQuoteV1 {
            marketplace_id: candidate.marketplace_id.trim().to_string(),
            provider_id: candidate.provider_id.trim().to_string(),
            provider_worker_id: candidate
                .provider_worker_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            total_price_msats: candidate.total_price_msats,
            latency_ms: candidate.latency_ms.filter(|value| *value > 0),
            reliability_bps: candidate.reliability_bps.unwrap_or(0).min(10_000),
            constraints: candidate.constraints.clone(),
            quote_id: candidate
                .quote_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            quote_sha256: candidate
                .quote_sha256
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        });
    }

    candidates.sort_by(|left, right| match policy.as_str() {
        "reliability_first_v1" => {
            let left_gap = 10_000u32.saturating_sub(left.reliability_bps) as u64;
            let right_gap = 10_000u32.saturating_sub(right.reliability_bps) as u64;
            left_gap
                .cmp(&right_gap)
                .then_with(|| left.total_price_msats.cmp(&right.total_price_msats))
                .then_with(|| {
                    left.latency_ms
                        .unwrap_or(u64::MAX)
                        .cmp(&right.latency_ms.unwrap_or(u64::MAX))
                })
                .then_with(|| left.marketplace_id.cmp(&right.marketplace_id))
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        }
        "balanced_v1" => {
            let left_latency = left
                .latency_ms
                .unwrap_or(ROUTER_POLICY_BALANCED_DEFAULT_LATENCY_MS);
            let right_latency = right
                .latency_ms
                .unwrap_or(ROUTER_POLICY_BALANCED_DEFAULT_LATENCY_MS);
            let left_gap = 10_000u32.saturating_sub(left.reliability_bps) as u64;
            let right_gap = 10_000u32.saturating_sub(right.reliability_bps) as u64;

            let left_latency_penalty =
                left_latency.saturating_mul(ROUTER_POLICY_BALANCED_LATENCY_PENALTY_MSATS_PER_MS);
            let right_latency_penalty =
                right_latency.saturating_mul(ROUTER_POLICY_BALANCED_LATENCY_PENALTY_MSATS_PER_MS);
            let left_rel_penalty = left_gap
                .saturating_div(100)
                .saturating_mul(ROUTER_POLICY_BALANCED_RELIABILITY_PENALTY_MSATS_PER_100_BPS);
            let right_rel_penalty = right_gap
                .saturating_div(100)
                .saturating_mul(ROUTER_POLICY_BALANCED_RELIABILITY_PENALTY_MSATS_PER_100_BPS);
            let left_score = left
                .total_price_msats
                .saturating_add(left_latency_penalty)
                .saturating_add(left_rel_penalty);
            let right_score = right
                .total_price_msats
                .saturating_add(right_latency_penalty)
                .saturating_add(right_rel_penalty);

            left_score
                .cmp(&right_score)
                .then_with(|| left.total_price_msats.cmp(&right.total_price_msats))
                .then_with(|| left_latency.cmp(&right_latency))
                .then_with(|| left_gap.cmp(&right_gap))
                .then_with(|| left.marketplace_id.cmp(&right.marketplace_id))
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        }
        _ => {
            let left_gap = 10_000u32.saturating_sub(left.reliability_bps) as u64;
            let right_gap = 10_000u32.saturating_sub(right.reliability_bps) as u64;
            left.total_price_msats
                .cmp(&right.total_price_msats)
                .then_with(|| {
                    left.latency_ms
                        .unwrap_or(u64::MAX)
                        .cmp(&right.latency_ms.unwrap_or(u64::MAX))
                })
                .then_with(|| left_gap.cmp(&right_gap))
                .then_with(|| left.marketplace_id.cmp(&right.marketplace_id))
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        }
    });

    let selected = candidates
        .first()
        .cloned()
        .ok_or_else(|| ApiError::InvalidRequest("candidates must not be empty".to_string()))?;

    let run_id = body.run_id.to_string();

    #[derive(Serialize)]
    struct DecisionHashInput<'a> {
        schema: &'a str,
        policy: &'a str,
        run_id: &'a str,
        capability: &'a str,
        objective_hash: &'a Option<String>,
        selected: &'a NormalizedCandidateQuoteV1,
        candidates: &'a [NormalizedCandidateQuoteV1],
    }

    let decision_hash_input = DecisionHashInput {
        schema: "openagents.marketplace.router_decision.v1",
        policy: policy.as_str(),
        run_id: run_id.as_str(),
        capability,
        objective_hash: &objective_hash,
        selected: &selected,
        candidates: &candidates,
    };
    let decision_sha256 = protocol::hash::canonical_hash(&decision_hash_input)
        .map_err(|error| ApiError::Internal(format!("decision hash failed: {error}")))?;

    let nostr_event = match state.config.bridge_nostr_secret_key {
        Some(secret_key) => {
            let message_id = format!("decision_{}", &decision_sha256[..16]);
            let order_id = format!("order_{}", &decision_sha256[..16]);
            let payload = CommerceMessageV1 {
                message_id,
                kind: CommerceMessageKindV1::Accept,
                marketplace_id: marketplace_id.clone(),
                actor_id: owner_key.clone(),
                created_at_unix: decided_at_unix,
                rfq_id: None,
                offer_id: None,
                quote_id: selected.quote_id.clone(),
                order_id: Some(order_id),
                receipt_id: None,
                objective_hash: objective_hash.clone(),
                run_id: Some(run_id.clone()),
                body: serde_json::json!({
                    "schema": "openagents.marketplace.router_decision_payload.v1",
                    "decision_sha256": decision_sha256.clone(),
                    "policy": policy.clone(),
                    "marketplace_id": marketplace_id.clone(),
                    "capability": capability,
                    "objective_hash": objective_hash.clone(),
                    "selected": selected.clone(),
                    "candidates": candidates.clone(),
                }),
            };

            let event = build_commerce_message_event(&secret_key, Some(decided_at_unix), &payload)
                .map_err(|error| ApiError::Internal(error.to_string()))?;
            Some(
                serde_json::to_value(&event)
                    .map_err(|error| ApiError::Internal(error.to_string()))?,
            )
        }
        None => None,
    };

    let idempotency_key = body
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("router_decision:{decision_sha256}"));

    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "receipt".to_string(),
                payload: serde_json::json!({
                    "receipt_type": "RouterDecision",
                    "payload": {
                        "schema": "openagents.marketplace.router_decision.v1",
                        "decision_sha256": decision_sha256.clone(),
                        "decided_at_unix": decided_at_unix,
                        "policy": policy.clone(),
                        "marketplace_id": marketplace_id.clone(),
                        "capability": capability,
                        "objective_hash": objective_hash.clone(),
                        "selected": selected.clone(),
                        "candidates": candidates.clone(),
                        "nostr_event": nostr_event.clone(),
                    }
                }),
                idempotency_key: Some(idempotency_key),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    verify_contract_critical_run_receipt(&state, body.run_id).await?;

    Ok(Json(RouterSelectComputeResponse {
        schema: "openagents.marketplace.router_decision.v1".to_string(),
        decision_sha256,
        policy,
        run_id,
        capability: capability.to_string(),
        objective_hash,
        selected,
        candidates,
        nostr_event,
    }))
}

async fn hydra_fx_rfq_create(
    State(state): State<AppState>,
    Json(body): Json<FxRfqRequestV1>,
) -> Result<Json<FxRfqResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    if body.schema.trim() != FX_RFQ_REQUEST_SCHEMA_V1 {
        return Err(ApiError::InvalidRequest(format!(
            "schema must be {}",
            FX_RFQ_REQUEST_SCHEMA_V1
        )));
    }
    let response = state
        .fx
        .create_or_get_rfq(body)
        .await
        .map_err(api_error_from_fx)?;
    Ok(Json(response))
}

async fn hydra_fx_quote_upsert(
    State(state): State<AppState>,
    Json(body): Json<FxQuoteUpsertRequestV1>,
) -> Result<Json<FxQuoteUpsertResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    if body.schema.trim() != FX_QUOTE_UPSERT_REQUEST_SCHEMA_V1 {
        return Err(ApiError::InvalidRequest(format!(
            "schema must be {}",
            FX_QUOTE_UPSERT_REQUEST_SCHEMA_V1
        )));
    }
    let response = state
        .fx
        .upsert_quote(body)
        .await
        .map_err(api_error_from_fx)?;
    Ok(Json(response))
}

async fn hydra_fx_select(
    State(state): State<AppState>,
    Json(body): Json<FxSelectRequestV1>,
) -> Result<Json<FxSelectResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    if body.schema.trim() != FX_SELECT_REQUEST_SCHEMA_V1 {
        return Err(ApiError::InvalidRequest(format!(
            "schema must be {}",
            FX_SELECT_REQUEST_SCHEMA_V1
        )));
    }
    let response = state
        .fx
        .select_quote(body)
        .await
        .map_err(api_error_from_fx)?;
    Ok(Json(response))
}

async fn hydra_fx_rfq_get(
    State(state): State<AppState>,
    Path(rfq_id): Path<String>,
) -> Result<Json<FxRfqResponseV1>, ApiError> {
    let response = state
        .fx
        .get_rfq(rfq_id.as_str())
        .await
        .map_err(api_error_from_fx)?;
    Ok(Json(response))
}

async fn compute_hydra_risk_health(
    state: &AppState,
) -> Result<HydraRiskHealthResponseV1, ApiError> {
    let credit = state.credit.health().await.map_err(api_error_from_credit)?;
    let liquidity = state.liquidity.status().await;
    let mut withdraw_throttle_mode: Option<String> = None;
    let mut withdraw_throttle_reasons: Vec<String> = Vec::new();
    let mut withdraw_throttle_extra_delay_hours: Option<i64> = None;
    let mut withdraw_throttle_execution_cap_per_tick: Option<u32> = None;
    let mut withdraw_throttle_halted = false;
    let mut withdraw_throttle_stressed = false;

    let mut reasons = Vec::new();
    let mut direct_disabled =
        !liquidity.wallet_executor_configured || !liquidity.wallet_executor_reachable;
    if direct_disabled {
        reasons.push("direct_liquidity_unavailable".to_string());
    }
    if state
        .config
        .liquidity_pool_withdraw_throttle
        .lp_mode_enabled
    {
        if let Some(primary_pool_id) = state
            .config
            .liquidity_pool_snapshot_pool_ids
            .first()
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            match state
                .liquidity_pool
                .withdraw_throttle_status(primary_pool_id)
                .await
            {
                Ok(throttle) => {
                    withdraw_throttle_mode = Some(throttle.mode.as_str().to_string());
                    withdraw_throttle_reasons = throttle.reasons.clone();
                    withdraw_throttle_extra_delay_hours = Some(throttle.extra_delay_hours);
                    withdraw_throttle_execution_cap_per_tick = throttle.execution_cap_per_tick;
                    withdraw_throttle_halted = throttle.mode
                        == crate::liquidity_pool::types::WithdrawThrottleModeV1::Halted;
                    withdraw_throttle_stressed = throttle.mode
                        == crate::liquidity_pool::types::WithdrawThrottleModeV1::Stressed;
                    if withdraw_throttle_halted {
                        direct_disabled = true;
                        reasons.push("llp_withdrawals_halted".to_string());
                    } else if withdraw_throttle_stressed {
                        reasons.push("llp_withdrawals_stressed".to_string());
                    }
                }
                Err(LiquidityPoolError::NotFound) => {
                    reasons.push("llp_pool_not_found".to_string());
                }
                Err(error) => {
                    tracing::warn!(
                        pool_id = primary_pool_id,
                        reason = %error,
                        "hydra risk health failed to read withdraw throttle status"
                    );
                    reasons.push("llp_throttle_unavailable".to_string());
                }
            }
        }
    }
    if credit.breakers.halt_new_envelopes {
        reasons.push("cep_halt_new_envelopes".to_string());
    }
    if credit.breakers.halt_large_settlements {
        reasons.push("cep_halt_large_settlements".to_string());
    }
    if credit.loss_rate > 0.35 {
        reasons.push("credit_loss_rate_elevated".to_string());
    }
    if credit.ln_failure_rate > 0.35 {
        reasons.push("liquidity_ln_failure_rate_elevated".to_string());
    }

    let degraded = direct_disabled
        || credit.breakers.halt_new_envelopes
        || credit.breakers.halt_large_settlements
        || credit.loss_rate > 0.35
        || credit.ln_failure_rate > 0.35
        || withdraw_throttle_stressed
        || withdraw_throttle_halted;

    let response = HydraRiskHealthResponseV1 {
        schema: HYDRA_RISK_HEALTH_RESPONSE_SCHEMA_V1.to_string(),
        generated_at: Utc::now(),
        credit_breakers: HydraRiskCreditBreakersV1 {
            halt_new_envelopes: credit.breakers.halt_new_envelopes,
            halt_large_settlements: credit.breakers.halt_large_settlements,
        },
        liquidity: HydraRiskLiquidityStateV1 {
            wallet_executor_configured: liquidity.wallet_executor_configured,
            wallet_executor_reachable: liquidity.wallet_executor_reachable,
            error_code: liquidity.error_code,
            withdraw_throttle_mode,
            withdraw_throttle_reasons,
            withdraw_throttle_extra_delay_hours,
            withdraw_throttle_execution_cap_per_tick,
        },
        routing: HydraRiskRoutingStateV1 {
            degraded,
            direct_disabled,
            reasons,
        },
    };
    state
        .hydra_observability
        .record_risk_health(&response)
        .await;
    Ok(response)
}

async fn hydra_risk_health(
    State(state): State<AppState>,
) -> Result<Json<HydraRiskHealthResponseV1>, ApiError> {
    Ok(Json(compute_hydra_risk_health(&state).await?))
}

async fn hydra_observability(
    State(state): State<AppState>,
) -> Result<Json<HydraObservabilityResponseV1>, ApiError> {
    Ok(Json(state.hydra_observability.snapshot().await))
}

async fn hydra_routing_score(
    State(state): State<AppState>,
    Json(body): Json<HydraRoutingScoreRequestV1>,
) -> Result<Json<HydraRoutingScoreResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;

    let idempotency_key = body.idempotency_key.trim();
    if idempotency_key.is_empty() {
        return Err(ApiError::InvalidRequest(
            "idempotency_key must not be empty".to_string(),
        ));
    }

    let run_id = Uuid::parse_str(body.run_id.trim())
        .map_err(|_| ApiError::InvalidRequest("run_id must be a valid UUID".to_string()))?;
    let run_id_string = run_id.to_string();

    let capability = body.capability.trim();
    if capability.is_empty() {
        return Err(ApiError::InvalidRequest(
            "capability must not be empty".to_string(),
        ));
    }
    if body.candidates.is_empty() {
        return Err(ApiError::InvalidRequest(
            "candidates must not be empty".to_string(),
        ));
    }

    let policy = body.policy.trim();
    let policy = if policy.is_empty() {
        "lowest_total_cost_v1".to_string()
    } else {
        policy.to_string()
    };
    if !matches!(
        policy.as_str(),
        "lowest_total_cost_v1" | "balanced_v1" | "reliability_first_v1"
    ) {
        return Err(ApiError::InvalidRequest(format!(
            "unsupported policy: {policy}"
        )));
    }

    let objective_hash = body
        .objective_hash
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let decided_at_unix = if body.decided_at_unix > 0 {
        body.decided_at_unix
    } else {
        Utc::now().timestamp().max(0) as u64
    };
    let risk_health = compute_hydra_risk_health(&state).await?;
    let mut risk_notes = risk_health.routing.reasons.clone();

    let marketplace_id = {
        let trimmed = body.marketplace_id.trim();
        if trimmed.is_empty() {
            "openagents".to_string()
        } else {
            trimmed.to_string()
        }
    };

    let mut candidates: Vec<NormalizedCandidateQuoteV1> = Vec::with_capacity(body.candidates.len());
    for candidate in &body.candidates {
        if candidate.marketplace_id.trim().is_empty() {
            return Err(ApiError::InvalidRequest(
                "candidate.marketplace_id must not be empty".to_string(),
            ));
        }
        if candidate.provider_id.trim().is_empty() {
            return Err(ApiError::InvalidRequest(
                "candidate.provider_id must not be empty".to_string(),
            ));
        }
        if candidate.total_price_msats == 0 {
            return Err(ApiError::InvalidRequest(
                "candidate.total_price_msats must be greater than zero".to_string(),
            ));
        }
        let provider_id = candidate.provider_id.trim();
        if provider_id == HYDRA_ROUTE_PROVIDER_DIRECT && risk_health.routing.direct_disabled {
            risk_notes.push("direct_candidate_filtered_by_risk".to_string());
            continue;
        }
        if provider_id == HYDRA_ROUTE_PROVIDER_CEP && risk_health.credit_breakers.halt_new_envelopes
        {
            risk_notes.push("cep_candidate_filtered_by_breaker".to_string());
            continue;
        }
        let constraints = match candidate.constraints.clone() {
            serde_json::Value::Null => serde_json::json!({}),
            serde_json::Value::Object(_) => candidate.constraints.clone(),
            _ => {
                return Err(ApiError::InvalidRequest(
                    "candidate.constraints must be an object".to_string(),
                ));
            }
        };

        candidates.push(NormalizedCandidateQuoteV1 {
            marketplace_id: candidate.marketplace_id.trim().to_string(),
            provider_id: provider_id.to_string(),
            provider_worker_id: candidate
                .provider_worker_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            total_price_msats: candidate.total_price_msats,
            latency_ms: candidate.latency_ms.filter(|value| *value > 0),
            reliability_bps: candidate.reliability_bps.min(10_000),
            constraints,
            quote_id: candidate
                .quote_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            quote_sha256: candidate
                .quote_sha256
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        });
    }
    if candidates.is_empty() {
        return Err(ApiError::Conflict(
            "hydra risk breakers denied all routing candidates".to_string(),
        ));
    }

    candidates.sort_by(|left, right| match policy.as_str() {
        "reliability_first_v1" => {
            let left_gap = 10_000u32.saturating_sub(left.reliability_bps) as u64;
            let right_gap = 10_000u32.saturating_sub(right.reliability_bps) as u64;
            left_gap
                .cmp(&right_gap)
                .then_with(|| left.total_price_msats.cmp(&right.total_price_msats))
                .then_with(|| {
                    left.latency_ms
                        .unwrap_or(u64::MAX)
                        .cmp(&right.latency_ms.unwrap_or(u64::MAX))
                })
                .then_with(|| left.marketplace_id.cmp(&right.marketplace_id))
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        }
        "balanced_v1" => {
            let left_latency = left
                .latency_ms
                .unwrap_or(ROUTER_POLICY_BALANCED_DEFAULT_LATENCY_MS);
            let right_latency = right
                .latency_ms
                .unwrap_or(ROUTER_POLICY_BALANCED_DEFAULT_LATENCY_MS);
            let left_gap = 10_000u32.saturating_sub(left.reliability_bps) as u64;
            let right_gap = 10_000u32.saturating_sub(right.reliability_bps) as u64;
            let left_latency_penalty =
                left_latency.saturating_mul(ROUTER_POLICY_BALANCED_LATENCY_PENALTY_MSATS_PER_MS);
            let right_latency_penalty =
                right_latency.saturating_mul(ROUTER_POLICY_BALANCED_LATENCY_PENALTY_MSATS_PER_MS);
            let left_rel_penalty = left_gap
                .saturating_div(100)
                .saturating_mul(ROUTER_POLICY_BALANCED_RELIABILITY_PENALTY_MSATS_PER_100_BPS);
            let right_rel_penalty = right_gap
                .saturating_div(100)
                .saturating_mul(ROUTER_POLICY_BALANCED_RELIABILITY_PENALTY_MSATS_PER_100_BPS);
            let left_score = left
                .total_price_msats
                .saturating_add(left_latency_penalty)
                .saturating_add(left_rel_penalty);
            let right_score = right
                .total_price_msats
                .saturating_add(right_latency_penalty)
                .saturating_add(right_rel_penalty);

            left_score
                .cmp(&right_score)
                .then_with(|| left.total_price_msats.cmp(&right.total_price_msats))
                .then_with(|| left_latency.cmp(&right_latency))
                .then_with(|| left_gap.cmp(&right_gap))
                .then_with(|| left.marketplace_id.cmp(&right.marketplace_id))
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        }
        _ => {
            let left_gap = 10_000u32.saturating_sub(left.reliability_bps) as u64;
            let right_gap = 10_000u32.saturating_sub(right.reliability_bps) as u64;
            left.total_price_msats
                .cmp(&right.total_price_msats)
                .then_with(|| {
                    left.latency_ms
                        .unwrap_or(u64::MAX)
                        .cmp(&right.latency_ms.unwrap_or(u64::MAX))
                })
                .then_with(|| left_gap.cmp(&right_gap))
                .then_with(|| left.marketplace_id.cmp(&right.marketplace_id))
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        }
    });

    let selected = candidates
        .first()
        .cloned()
        .ok_or_else(|| ApiError::InvalidRequest("candidates must not be empty".to_string()))?;

    let min_price = candidates
        .iter()
        .map(|candidate| candidate.total_price_msats)
        .min()
        .unwrap_or(selected.total_price_msats);
    let max_price = candidates
        .iter()
        .map(|candidate| candidate.total_price_msats)
        .max()
        .unwrap_or(selected.total_price_msats);
    let price_span = max_price.saturating_sub(min_price);
    let price_position = if price_span == 0 {
        1.0
    } else {
        1.0 - ((selected.total_price_msats.saturating_sub(min_price)) as f64 / price_span as f64)
    }
    .clamp(0.0, 1.0);

    let reliability_score = (selected.reliability_bps as f64 / 10_000.0).clamp(0.0, 1.0);
    let latency_score = {
        let latency = selected
            .latency_ms
            .unwrap_or(ROUTER_POLICY_BALANCED_DEFAULT_LATENCY_MS) as f64;
        let normalized =
            1.0 - (latency / (ROUTER_POLICY_BALANCED_DEFAULT_LATENCY_MS.saturating_mul(4) as f64));
        normalized.clamp(0.0, 1.0)
    };
    let breadth_score = ((candidates.len() as f64) / 8.0).clamp(0.15, 1.0);
    let mut confidence =
        ((reliability_score * 0.60) + (price_position * 0.20) + (latency_score * 0.20))
            .clamp(0.0, 1.0);
    let mut liquidity_score = ((reliability_score * 0.60) + (breadth_score * 0.40)).clamp(0.0, 1.0);
    if risk_health.routing.degraded {
        confidence = (confidence * 0.75).clamp(0.0, 1.0);
        liquidity_score = (liquidity_score * 0.80).clamp(0.0, 1.0);
        risk_notes.push("routing_confidence_degraded".to_string());
    }
    risk_notes.sort();
    risk_notes.dedup();

    let factors = HydraRoutingDecisionFactorsV1 {
        expected_fee_msats: selected.total_price_msats.saturating_div(50).max(1),
        confidence,
        liquidity_score,
        policy_notes: {
            let mut notes = vec![
                format!("policy:{policy}"),
                format!("candidate_count:{}", candidates.len()),
                format!("selected_reliability_bps:{}", selected.reliability_bps),
            ];
            notes.extend(risk_notes);
            notes
        },
    };
    state
        .hydra_observability
        .record_routing_decision(
            selected.provider_id.as_str(),
            factors.confidence,
            risk_health.liquidity.withdraw_throttle_mode.as_deref(),
        )
        .await;

    #[derive(Serialize)]
    struct DecisionHashInput<'a> {
        schema: &'a str,
        policy: &'a str,
        run_id: &'a str,
        marketplace_id: &'a str,
        capability: &'a str,
        objective_hash: &'a Option<String>,
        selected: &'a NormalizedCandidateQuoteV1,
        candidates: &'a [NormalizedCandidateQuoteV1],
        factors: &'a HydraRoutingDecisionFactorsV1,
        decided_at_unix: u64,
    }

    let decision_hash_input = DecisionHashInput {
        schema: ROUTING_SCORE_RESPONSE_SCHEMA_V1,
        policy: policy.as_str(),
        run_id: run_id_string.as_str(),
        marketplace_id: marketplace_id.as_str(),
        capability,
        objective_hash: &objective_hash,
        selected: &selected,
        candidates: &candidates,
        factors: &factors,
        decided_at_unix,
    };
    let decision_sha256 = protocol::hash::canonical_hash(&decision_hash_input)
        .map_err(|error| ApiError::Internal(format!("decision hash failed: {error}")))?;

    let selected_wire = HydraRoutingCandidateQuoteV1 {
        marketplace_id: selected.marketplace_id.clone(),
        provider_id: selected.provider_id.clone(),
        provider_worker_id: selected.provider_worker_id.clone(),
        total_price_msats: selected.total_price_msats,
        latency_ms: selected.latency_ms,
        reliability_bps: selected.reliability_bps,
        constraints: selected.constraints.clone(),
        quote_id: selected.quote_id.clone(),
        quote_sha256: selected.quote_sha256.clone(),
    };
    let candidates_wire: Vec<HydraRoutingCandidateQuoteV1> = candidates
        .iter()
        .map(|candidate| HydraRoutingCandidateQuoteV1 {
            marketplace_id: candidate.marketplace_id.clone(),
            provider_id: candidate.provider_id.clone(),
            provider_worker_id: candidate.provider_worker_id.clone(),
            total_price_msats: candidate.total_price_msats,
            latency_ms: candidate.latency_ms,
            reliability_bps: candidate.reliability_bps,
            constraints: candidate.constraints.clone(),
            quote_id: candidate.quote_id.clone(),
            quote_sha256: candidate.quote_sha256.clone(),
        })
        .collect();

    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        decision_sha256: &'a str,
        run_id: &'a str,
        marketplace_id: &'a str,
        capability: &'a str,
        policy: &'a str,
        objective_hash: &'a Option<String>,
        decided_at_unix: u64,
        selected: &'a HydraRoutingCandidateQuoteV1,
        factors: &'a HydraRoutingDecisionFactorsV1,
    }

    let receipt_sha256 = protocol::hash::canonical_hash(&ReceiptHashInput {
        schema: HYDRA_ROUTING_DECISION_RECEIPT_SCHEMA_V1,
        decision_sha256: decision_sha256.as_str(),
        run_id: run_id_string.as_str(),
        marketplace_id: marketplace_id.as_str(),
        capability,
        policy: policy.as_str(),
        objective_hash: &objective_hash,
        decided_at_unix,
        selected: &selected_wire,
        factors: &factors,
    })
    .map_err(|error| ApiError::Internal(format!("receipt hash failed: {error}")))?;
    let receipt_id = format!("hydrart_{}", &receipt_sha256[..16]);
    let receipt = serde_json::json!({
        "schema": HYDRA_ROUTING_DECISION_RECEIPT_SCHEMA_V1,
        "receipt_id": receipt_id.clone(),
        "decision_sha256": decision_sha256.clone(),
        "run_id": run_id_string.clone(),
        "marketplace_id": marketplace_id.clone(),
        "capability": capability.to_string(),
        "policy": policy.clone(),
        "objective_hash": objective_hash.clone(),
        "decided_at_unix": decided_at_unix,
        "selected": selected_wire.clone(),
        "factors": factors.clone(),
        "canonical_json_sha256": receipt_sha256.clone(),
    });

    let receipt_linkage = HydraRoutingDecisionReceiptLinkageV1 {
        receipt_schema: HYDRA_ROUTING_DECISION_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id,
        canonical_json_sha256: receipt_sha256,
    };

    let nostr_event = match state.config.bridge_nostr_secret_key {
        Some(secret_key) => {
            let message_id = format!("hydra_decision_{}", &decision_sha256[..16]);
            let order_id = format!("hydra_order_{}", &decision_sha256[..16]);
            let payload = CommerceMessageV1 {
                message_id,
                kind: CommerceMessageKindV1::Accept,
                marketplace_id: marketplace_id.clone(),
                actor_id: "hydra.runtime".to_string(),
                created_at_unix: decided_at_unix,
                rfq_id: None,
                offer_id: None,
                quote_id: selected.quote_id.clone(),
                order_id: Some(order_id),
                receipt_id: Some(receipt_linkage.receipt_id.clone()),
                objective_hash: objective_hash.clone(),
                run_id: Some(run_id_string.clone()),
                body: serde_json::json!({
                    "schema": "openagents.hydra.routing_decision_payload.v1",
                    "decision_sha256": decision_sha256.clone(),
                    "policy": policy.clone(),
                    "marketplace_id": marketplace_id.clone(),
                    "capability": capability.to_string(),
                    "objective_hash": objective_hash.clone(),
                    "selected": selected_wire.clone(),
                    "factors": factors.clone(),
                    "receipt": receipt_linkage.clone(),
                }),
            };
            let event = build_commerce_message_event(&secret_key, Some(decided_at_unix), &payload)
                .map_err(|error| ApiError::Internal(error.to_string()))?;
            Some(
                serde_json::to_value(&event)
                    .map_err(|error| ApiError::Internal(error.to_string()))?,
            )
        }
        None => None,
    };

    state
        .orchestrator
        .append_run_event(
            run_id,
            AppendRunEventRequest {
                event_type: "receipt".to_string(),
                payload: serde_json::json!({
                    "receipt_type": "HydraRoutingDecision",
                    "payload": receipt
                }),
                idempotency_key: Some(idempotency_key.to_string()),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    verify_contract_critical_run_receipt(&state, run_id).await?;

    Ok(Json(HydraRoutingScoreResponseV1 {
        schema: ROUTING_SCORE_RESPONSE_SCHEMA_V1.to_string(),
        decision_sha256,
        policy,
        run_id: run_id_string,
        marketplace_id,
        capability: capability.to_string(),
        objective_hash,
        selected: selected_wire,
        candidates: candidates_wire,
        factors,
        receipt: Some(receipt_linkage),
        nostr_event: nostr_event.unwrap_or_else(|| serde_json::json!({})),
        decided_at_unix,
    }))
}

async fn dispatch_sandbox_run(
    State(state): State<AppState>,
    Json(body): Json<DispatchSandboxRunBody>,
) -> Result<Json<DispatchSandboxRunResponse>, ApiError> {
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    state
        .compute_abuse
        .enforce_dispatch_rate(owner_key.as_str())
        .await?;

    validate_sandbox_request_phase0(&body.request)?;
    let job_hash = protocol::hash::canonical_hash(&body.request)
        .map_err(|error| ApiError::InvalidRequest(format!("invalid sandbox request: {error}")))?;

    let workers = state.workers.list_all_workers().await;
    let selection = match select_provider_for_capability(
        &workers,
        Some(&owner),
        PHASE0_REQUIRED_PROVIDER_CAPABILITY,
    ) {
        Some(selection) => selection,
        None => {
            state
                .compute_telemetry
                .record_dispatch_not_found(owner_key.as_str())
                .await;
            return Err(ApiError::NotFound);
        }
    };

    match dispatch_sandbox_request_to_provider(&selection, &workers, &job_hash, &body.request).await
    {
        Ok((response, latency_ms)) => {
            let provider_failed = matches!(
                response.status,
                protocol::jobs::sandbox::SandboxStatus::Timeout
                    | protocol::jobs::sandbox::SandboxStatus::Cancelled
                    | protocol::jobs::sandbox::SandboxStatus::Error
            );
            if provider_failed {
                let reason = format!("dispatch_status:{:?}", response.status);
                if let Err(error) = apply_provider_failure_strike(
                    &state,
                    selection.provider.worker_id.as_str(),
                    job_hash.as_str(),
                    reason.as_str(),
                )
                .await
                {
                    tracing::warn!(
                        worker_id = %selection.provider.worker_id,
                        err = ?error,
                        "provider failure strike update failed"
                    );
                }
            }
            if provider_failed && selection.tier == crate::marketplace::ProviderSelectionTier::Owned
            {
                if let Some(alt) = select_provider_for_capability_excluding(
                    &workers,
                    Some(&owner),
                    PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                    Some(selection.provider.worker_id.as_str()),
                ) {
                    if alt.provider.worker_id != selection.provider.worker_id {
                        let fallback_from_provider_id =
                            Some(selection.provider.provider_id.clone());
                        let (response, latency_ms) = dispatch_sandbox_request_to_provider(
                            &alt,
                            &workers,
                            &job_hash,
                            &body.request,
                        )
                        .await?;
                        state
                            .compute_telemetry
                            .record_dispatch_success(owner_key.as_str(), latency_ms, true)
                            .await;
                        return Ok(Json(DispatchSandboxRunResponse {
                            job_hash,
                            selection: alt,
                            response,
                            latency_ms,
                            fallback_from_provider_id,
                        }));
                    }
                }
                if let Some(reserve) = select_provider_for_capability_excluding(
                    &workers,
                    None,
                    PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                    Some(selection.provider.worker_id.as_str()),
                ) {
                    if reserve.provider.worker_id != selection.provider.worker_id {
                        let fallback_from_provider_id =
                            Some(selection.provider.provider_id.clone());
                        let (response, latency_ms) = dispatch_sandbox_request_to_provider(
                            &reserve,
                            &workers,
                            &job_hash,
                            &body.request,
                        )
                        .await?;
                        state
                            .compute_telemetry
                            .record_dispatch_success(owner_key.as_str(), latency_ms, true)
                            .await;
                        return Ok(Json(DispatchSandboxRunResponse {
                            job_hash,
                            selection: reserve,
                            response,
                            latency_ms,
                            fallback_from_provider_id,
                        }));
                    }
                }
            }

            state
                .compute_telemetry
                .record_dispatch_success(owner_key.as_str(), latency_ms, false)
                .await;
            Ok(Json(DispatchSandboxRunResponse {
                job_hash,
                selection,
                response,
                latency_ms,
                fallback_from_provider_id: None,
            }))
        }
        Err(error) => {
            if let Err(err) = apply_provider_failure_strike(
                &state,
                selection.provider.worker_id.as_str(),
                job_hash.as_str(),
                "dispatch_error",
            )
            .await
            {
                tracing::warn!(
                    worker_id = %selection.provider.worker_id,
                    err = ?err,
                    "provider dispatch failure strike update failed"
                );
            }
            if selection.tier == crate::marketplace::ProviderSelectionTier::Owned {
                if let Some(alt) = select_provider_for_capability_excluding(
                    &workers,
                    Some(&owner),
                    PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                    Some(selection.provider.worker_id.as_str()),
                ) {
                    if alt.provider.worker_id != selection.provider.worker_id {
                        let fallback_from_provider_id =
                            Some(selection.provider.provider_id.clone());
                        let (response, latency_ms) = dispatch_sandbox_request_to_provider(
                            &alt,
                            &workers,
                            &job_hash,
                            &body.request,
                        )
                        .await?;
                        state
                            .compute_telemetry
                            .record_dispatch_success(owner_key.as_str(), latency_ms, true)
                            .await;
                        return Ok(Json(DispatchSandboxRunResponse {
                            job_hash,
                            selection: alt,
                            response,
                            latency_ms,
                            fallback_from_provider_id,
                        }));
                    }
                }
                if let Some(reserve) = select_provider_for_capability_excluding(
                    &workers,
                    None,
                    PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                    Some(selection.provider.worker_id.as_str()),
                ) {
                    if reserve.provider.worker_id != selection.provider.worker_id {
                        let fallback_from_provider_id =
                            Some(selection.provider.provider_id.clone());
                        let (response, latency_ms) = dispatch_sandbox_request_to_provider(
                            &reserve,
                            &workers,
                            &job_hash,
                            &body.request,
                        )
                        .await?;
                        state
                            .compute_telemetry
                            .record_dispatch_success(owner_key.as_str(), latency_ms, true)
                            .await;
                        return Ok(Json(DispatchSandboxRunResponse {
                            job_hash,
                            selection: reserve,
                            response,
                            latency_ms,
                            fallback_from_provider_id,
                        }));
                    }
                }
            }
            state
                .compute_telemetry
                .record_dispatch_error(owner_key.as_str())
                .await;
            Err(error)
        }
    }
}

async fn get_job_types() -> Json<JobTypesResponse> {
    let job_types = protocol::jobs::registered_job_types();
    Json(JobTypesResponse { job_types })
}

#[derive(Debug, Serialize)]
struct ComputeTelemetryResponse {
    schema: String,
    owner_key: String,
    capability: String,
    provider_total: usize,
    provider_eligible_owned: usize,
    provider_eligible_reserve: usize,
    provider_eligible_total: usize,
    dispatch: OwnerComputeTelemetrySnapshot,
}

async fn get_compute_telemetry(
    State(state): State<AppState>,
    Query(query): Query<ComputeTelemetryQuery>,
) -> Result<Json<ComputeTelemetryResponse>, ApiError> {
    let owner = owner_from_parts(query.owner_user_id, query.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    let capability = query
        .capability
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(PHASE0_REQUIRED_PROVIDER_CAPABILITY)
        .to_string();

    let workers = state.workers.list_all_workers().await;
    let providers = build_provider_catalog(&workers);
    let provider_total = providers.len();

    let mut eligible_owned = 0usize;
    let mut eligible_reserve = 0usize;
    for provider in &providers {
        if !provider_is_eligible_for_capability(provider, capability.as_str()) {
            continue;
        }
        if owners_match(&provider.owner, &owner) {
            eligible_owned += 1;
            continue;
        }
        if provider.reserve_pool {
            eligible_reserve += 1;
        }
    }
    let provider_eligible_total = eligible_owned.saturating_add(eligible_reserve);

    let dispatch = state.compute_telemetry.snapshot(owner_key.as_str()).await;

    Ok(Json(ComputeTelemetryResponse {
        schema: "openagents.marketplace.compute_telemetry.v1".to_string(),
        owner_key,
        capability,
        provider_total,
        provider_eligible_owned: eligible_owned,
        provider_eligible_reserve: eligible_reserve,
        provider_eligible_total,
        dispatch,
    }))
}

async fn get_compute_treasury_summary(
    State(state): State<AppState>,
    Query(query): Query<OwnerQuery>,
) -> Result<Json<crate::treasury::ComputeTreasurySummary>, ApiError> {
    let owner = owner_from_parts(query.owner_user_id, query.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    let summary = state
        .treasury
        .summarize_compute_owner(owner_key.as_str(), 50)
        .await;
    Ok(Json(summary))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReconcileComputeTreasuryBody {
    #[serde(default)]
    max_age_seconds: Option<i64>,
    #[serde(default)]
    max_jobs: Option<usize>,
}

async fn reconcile_compute_treasury(
    State(state): State<AppState>,
    Json(body): Json<ReconcileComputeTreasuryBody>,
) -> Result<Json<crate::treasury::ComputeTreasuryReconcileSummary>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let max_age_seconds = body.max_age_seconds.unwrap_or_else(|| {
        i64::try_from(state.config.treasury_reservation_ttl_seconds).unwrap_or(i64::MAX)
    });
    let max_jobs = body
        .max_jobs
        .unwrap_or(state.config.treasury_reconciliation_max_jobs);
    let summary = state
        .treasury
        .reconcile_reserved_compute_jobs(max_age_seconds, max_jobs)
        .await;
    Ok(Json(summary))
}

async fn liquidity_quote_pay(
    State(state): State<AppState>,
    Json(body): Json<QuotePayRequestV1>,
) -> Result<Json<QuotePayResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = state
        .liquidity
        .quote_pay(body)
        .await
        .map_err(api_error_from_liquidity)?;
    Ok(Json(response))
}

async fn liquidity_status(
    State(state): State<AppState>,
) -> Result<Json<LiquidityStatusResponseV1>, ApiError> {
    Ok(Json(state.liquidity.status().await))
}

async fn credit_offer(
    State(state): State<AppState>,
    Json(body): Json<CreditOfferRequestV1>,
) -> Result<Json<CreditOfferResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = state
        .credit
        .offer(body)
        .await
        .map_err(api_error_from_credit)?;
    Ok(Json(response))
}

async fn credit_intent(
    State(state): State<AppState>,
    Json(body): Json<CreditIntentRequestV1>,
) -> Result<Json<CreditIntentResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = state
        .credit
        .intent(body)
        .await
        .map_err(api_error_from_credit)?;
    Ok(Json(response))
}

async fn credit_envelope(
    State(state): State<AppState>,
    Json(body): Json<CreditEnvelopeRequestV1>,
) -> Result<Json<CreditEnvelopeResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = state
        .credit
        .envelope(body)
        .await
        .map_err(api_error_from_credit)?;
    Ok(Json(response))
}

async fn credit_settle(
    State(state): State<AppState>,
    Json(body): Json<CreditSettleRequestV1>,
) -> Result<Json<CreditSettleResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = state
        .credit
        .settle(body)
        .await
        .map_err(api_error_from_credit)?;
    Ok(Json(response))
}

async fn credit_health(
    State(state): State<AppState>,
) -> Result<Json<CreditHealthResponseV1>, ApiError> {
    let response = state.credit.health().await.map_err(api_error_from_credit)?;
    Ok(Json(response))
}

async fn credit_agent_exposure(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<CreditAgentExposureResponseV1>, ApiError> {
    let response = state
        .credit
        .agent_exposure(agent_id.as_str())
        .await
        .map_err(api_error_from_credit)?;
    Ok(Json(response))
}

async fn liquidity_pay(
    State(state): State<AppState>,
    Json(body): Json<PayRequestV1>,
) -> Result<Json<PayResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = state
        .liquidity
        .pay(body)
        .await
        .map_err(api_error_from_liquidity)?;
    Ok(Json(response))
}

async fn liquidity_pool_create_pool(
    State(state): State<AppState>,
    Path(pool_id): Path<String>,
    Json(body): Json<PoolCreateRequestV1>,
) -> Result<Json<PoolCreateResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = state
        .liquidity_pool
        .create_pool(pool_id.as_str(), body)
        .await
        .map_err(api_error_from_liquidity_pool)?;
    Ok(Json(response))
}

async fn liquidity_pool_deposit_quote(
    State(state): State<AppState>,
    Path(pool_id): Path<String>,
    Json(body): Json<DepositQuoteRequestV1>,
) -> Result<Json<DepositQuoteResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = state
        .liquidity_pool
        .deposit_quote(pool_id.as_str(), body)
        .await
        .map_err(api_error_from_liquidity_pool)?;
    Ok(Json(response))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ConfirmDepositResponseV1 {
    schema: String,
    deposit: crate::liquidity_pool::types::DepositRow,
    shares_minted: bool,
}

async fn liquidity_pool_confirm_deposit(
    State(state): State<AppState>,
    Path((pool_id, deposit_id)): Path<(String, String)>,
) -> Result<Json<ConfirmDepositResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let (deposit, shares_minted) = state
        .liquidity_pool
        .confirm_deposit(pool_id.as_str(), deposit_id.as_str())
        .await
        .map_err(api_error_from_liquidity_pool)?;
    Ok(Json(ConfirmDepositResponseV1 {
        schema: "openagents.liquidity.pool.confirm_deposit_response.v1".to_string(),
        deposit,
        shares_minted,
    }))
}

async fn liquidity_pool_withdraw_request(
    State(state): State<AppState>,
    Path(pool_id): Path<String>,
    Json(body): Json<WithdrawRequestV1>,
) -> Result<Json<WithdrawResponseV1>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let response = match state
        .liquidity_pool
        .withdraw_request(pool_id.as_str(), body)
        .await
    {
        Ok(response) => {
            state
                .hydra_observability
                .record_withdraw_request_throttle(
                    response.withdraw_throttle.as_ref().map(|value| value.mode),
                    false,
                )
                .await;
            response
        }
        Err(error) => {
            let rejected_by_throttle = matches!(
                &error,
                LiquidityPoolError::Conflict(message)
                if message.contains("withdrawals halted by throttle")
            );
            if rejected_by_throttle {
                state
                    .hydra_observability
                    .record_withdraw_request_throttle(
                        Some(crate::liquidity_pool::types::WithdrawThrottleModeV1::Halted),
                        true,
                    )
                    .await;
            }
            return Err(api_error_from_liquidity_pool(error));
        }
    };
    Ok(Json(response))
}

async fn liquidity_pool_status(
    State(state): State<AppState>,
    Path(pool_id): Path<String>,
) -> Result<Json<PoolStatusResponseV1>, ApiError> {
    let response = state
        .liquidity_pool
        .status(pool_id.as_str())
        .await
        .map_err(api_error_from_liquidity_pool)?;
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct PoolLatestSnapshotQuery {
    #[serde(default)]
    generate: Option<bool>,
    #[serde(default)]
    partition_kind: Option<PoolPartitionKindV1>,
}

async fn liquidity_pool_latest_snapshot(
    State(state): State<AppState>,
    Path(pool_id): Path<String>,
    Query(query): Query<PoolLatestSnapshotQuery>,
) -> Result<Json<PoolSnapshotResponseV1>, ApiError> {
    let partition_kind = query.partition_kind.unwrap_or(PoolPartitionKindV1::Llp);
    if query.generate.unwrap_or(false) {
        ensure_runtime_write_authority(&state)?;
        let response = state
            .liquidity_pool
            .generate_snapshot(pool_id.as_str(), partition_kind)
            .await
            .map_err(api_error_from_liquidity_pool)?;
        return Ok(Json(response));
    }

    let response = state
        .liquidity_pool
        .latest_snapshot(pool_id.as_str(), partition_kind)
        .await
        .map_err(api_error_from_liquidity_pool)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(response))
}

async fn get_fraud_incidents(
    State(state): State<AppState>,
    Query(query): Query<FraudIncidentsQuery>,
) -> Result<Json<FraudIncidentsResponse>, ApiError> {
    let limit = query.limit.unwrap_or(50).min(200);
    let incidents = state.fraud.list(limit).await;
    Ok(Json(FraudIncidentsResponse {
        schema: "openagents.fraud.incident_list.v1".to_string(),
        incidents,
    }))
}

async fn verify_sandbox_run(
    State(_state): State<AppState>,
    Json(body): Json<SandboxVerificationBody>,
) -> Result<Json<SandboxVerificationResponse>, ApiError> {
    let outcome = crate::verification::verify_sandbox_run(&body.request, &body.response);
    Ok(Json(SandboxVerificationResponse {
        passed: outcome.passed,
        exit_code: outcome.exit_code,
        violations: outcome.violations,
    }))
}

async fn verify_repo_index(
    State(_state): State<AppState>,
    Json(body): Json<RepoIndexVerificationBody>,
) -> Result<Json<RepoIndexVerificationResponse>, ApiError> {
    let outcome = crate::verification::verify_repo_index(&body.request, &body.response);

    Ok(Json(RepoIndexVerificationResponse {
        passed: outcome.passed,
        tree_sha256: outcome.tree_sha256,
        violations: outcome.violations,
    }))
}

async fn settle_sandbox_run(
    State(state): State<AppState>,
    Json(body): Json<SettleSandboxRunBody>,
) -> Result<Json<SettleSandboxRunResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let owner_key = owner_rate_key(&owner);
    if body.provider_id.trim().is_empty() || body.provider_worker_id.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "provider_id and provider_worker_id are required".to_string(),
        ));
    }
    if body.amount_msats == 0 {
        return Err(ApiError::InvalidRequest(
            "amount_msats must be greater than zero".to_string(),
        ));
    }

    validate_sandbox_request_phase0(&body.request)?;
    let job_hash = protocol::hash::canonical_hash(&body.request)
        .map_err(|error| ApiError::InvalidRequest(format!("invalid sandbox request: {error}")))?;

    let mut price_integrity_passed = true;
    let mut price_integrity_violation = false;
    let mut price_integrity_variance_msats: i64 = 0;
    let mut price_integrity_variance_bps: u64 = 0;
    let mut quote_sha256: Option<String> = None;
    let mut quote_id: Option<String> = None;
    let mut quoted_provider_price_msats: Option<u64> = None;
    let mut quoted_total_price_msats: Option<u64> = None;

    if let Some(quote) = body.quote.as_ref() {
        if quote.provider_id.as_str() != body.provider_id.trim() {
            return Err(ApiError::InvalidRequest(
                "quote.provider_id does not match provider_id".to_string(),
            ));
        }
        if quote.provider_worker_id.as_str() != body.provider_worker_id.trim() {
            return Err(ApiError::InvalidRequest(
                "quote.provider_worker_id does not match provider_worker_id".to_string(),
            ));
        }
        if quote.capability.as_str() != PHASE0_REQUIRED_PROVIDER_CAPABILITY {
            return Err(ApiError::InvalidRequest(format!(
                "quote.capability must be {}",
                PHASE0_REQUIRED_PROVIDER_CAPABILITY
            )));
        }
        if quote.objective_hash.as_str() != job_hash.as_str() {
            return Err(ApiError::InvalidRequest(
                "quote.objective_hash does not match job_hash".to_string(),
            ));
        }

        let now_unix = Utc::now().timestamp().max(0) as u64;
        if quote.valid_until_unix > 0 && quote.valid_until_unix < now_unix {
            return Err(ApiError::InvalidRequest("quote has expired".to_string()));
        }

        let workers = state.workers.list_all_workers().await;
        let providers = build_provider_catalog(&workers);
        let provider = providers
            .iter()
            .find(|provider| provider.worker_id == body.provider_worker_id.trim())
            .ok_or_else(|| {
                ApiError::InvalidRequest(format!(
                    "unknown provider_worker_id {}",
                    body.provider_worker_id.trim()
                ))
            })?;

        let expected = compute_all_in_quote_v1(
            provider,
            PHASE0_REQUIRED_PROVIDER_CAPABILITY,
            job_hash.as_str(),
            quote.issued_at_unix,
        )
        .ok_or_else(|| ApiError::Internal("unable to compute expected quote".to_string()))?;
        if expected.quote_sha256 != quote.quote_sha256 || expected.quote_id != quote.quote_id {
            return Err(ApiError::InvalidRequest(
                "quote does not match current provider terms".to_string(),
            ));
        }

        quote_sha256 = Some(quote.quote_sha256.clone());
        quote_id = Some(quote.quote_id.clone());
        quoted_provider_price_msats = Some(quote.provider_price_msats);
        quoted_total_price_msats = Some(quote.total_price_msats);

        let quoted = quote.provider_price_msats as i64;
        let delivered = body.amount_msats as i64;
        price_integrity_variance_msats = delivered.saturating_sub(quoted);
        let abs_variance_msats = if price_integrity_variance_msats < 0 {
            price_integrity_variance_msats
                .checked_abs()
                .unwrap_or(i64::MAX) as u64
        } else {
            price_integrity_variance_msats as u64
        };
        price_integrity_variance_bps = if quote.provider_price_msats == 0 {
            0
        } else {
            abs_variance_msats
                .saturating_mul(10_000)
                .saturating_div(quote.provider_price_msats)
        };
        price_integrity_passed = abs_variance_msats == PRICE_INTEGRITY_TOLERANCE_MSATS;
        price_integrity_violation = price_integrity_variance_msats > 0
            && abs_variance_msats > PRICE_INTEGRITY_TOLERANCE_MSATS;

        state
            .orchestrator
            .append_run_event(
                body.run_id,
                AppendRunEventRequest {
                    event_type: "receipt".to_string(),
                    payload: serde_json::json!({
                        "receipt_type": "QuoteCommitted",
                        "payload": {
                            "job_hash": job_hash.clone(),
                            "quote_id": quote.quote_id.clone(),
                            "quote_sha256": quote.quote_sha256.clone(),
                            "issued_at_unix": quote.issued_at_unix,
                            "valid_until_unix": quote.valid_until_unix,
                            "provider_id": quote.provider_id.clone(),
                            "provider_worker_id": quote.provider_worker_id.clone(),
                            "quoted_provider_price_msats": quote.provider_price_msats,
                            "quoted_total_price_msats": quote.total_price_msats,
                        }
                    }),
                    idempotency_key: Some(format!(
                        "quote-commit:{job_hash}:{}",
                        quote.quote_sha256
                    )),
                    expected_previous_seq: None,
                },
            )
            .await
            .map_err(ApiError::from_orchestration)?;

        state
            .orchestrator
            .append_run_event(
                body.run_id,
                AppendRunEventRequest {
                    event_type: "receipt".to_string(),
                    payload: serde_json::json!({
                        "receipt_type": if price_integrity_passed { "PriceIntegrityPassed" } else { "PriceIntegrityFailed" },
                        "payload": {
                            "job_hash": job_hash.clone(),
                            "quote_id": quote_id.clone(),
                            "quote_sha256": quote_sha256.clone(),
                            "quoted_provider_price_msats": quote.provider_price_msats,
                            "quoted_total_price_msats": quote.total_price_msats,
                            "delivered_provider_amount_msats": body.amount_msats,
                            "delivered_total_price_msats_estimate": body.amount_msats.saturating_add(quote.operator_fee_msats).saturating_add(quote.policy_adder_msats),
                            "variance_msats": price_integrity_variance_msats,
                            "variance_bps": price_integrity_variance_bps,
                            "tolerance_msats": PRICE_INTEGRITY_TOLERANCE_MSATS,
                        }
                    }),
                    idempotency_key: Some(format!("price-integrity:{job_hash}:{}", quote.quote_sha256)),
                    expected_previous_seq: None,
                },
            )
            .await
            .map_err(ApiError::from_orchestration)?;
    }

    if let Some(quoted_provider_price_msats) = quoted_provider_price_msats {
        apply_provider_price_integrity_signal(
            &state,
            body.provider_worker_id.trim(),
            job_hash.as_str(),
            quote_id.as_deref(),
            quote_sha256.as_deref(),
            quoted_provider_price_msats,
            body.amount_msats,
            price_integrity_variance_msats,
            price_integrity_variance_bps,
            price_integrity_violation,
        )
        .await?;
    }

    let settle_amount_msats = quoted_provider_price_msats.unwrap_or(body.amount_msats);
    let route_policy = body.route_policy.clone().unwrap_or_default();
    let use_direct_settlement =
        should_use_direct_objective_route(settle_amount_msats, &route_policy);
    let cep_route = body.cep.clone();
    if !use_direct_settlement && cep_route.is_none() {
        return Err(ApiError::InvalidRequest(
            "route_policy selected CEP but cep config is missing".to_string(),
        ));
    }

    let (reservation, _created) = state
        .treasury
        .reserve_compute_job(
            owner_key.as_str(),
            job_hash.as_str(),
            body.provider_id.trim(),
            body.provider_worker_id.trim(),
            settle_amount_msats,
        )
        .await
        .map_err(ApiError::from_treasury)?;

    // Emit reservation receipt into the run for replay evidence (idempotent).
    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "receipt".to_string(),
                payload: serde_json::json!({
                    "receipt_type": "BudgetReserved",
                    "payload": {
                        "scope": "compute_job",
                        "amount_msats": reservation.amount_msats,
                        "reservation_id": reservation.reservation_id,
                        "job_hash": job_hash.clone(),
                        "provider_id": reservation.provider_id.clone(),
                    }
                }),
                idempotency_key: Some(format!("budget-reserved:{job_hash}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    let outcome = crate::verification::verify_sandbox_run(&body.request, &body.response);
    let violations = outcome.violations.clone();
    if violations.is_empty() {
        if price_integrity_passed {
            apply_provider_success_signal(
                &state,
                body.provider_worker_id.trim(),
                job_hash.as_str(),
            )
            .await?;
        }
    } else {
        apply_provider_violation_strike(
            &state,
            body.provider_worker_id.trim(),
            job_hash.as_str(),
            &violations,
        )
        .await?;
    }

    let verification_receipt_payload = serde_json::json!({
        "receipt_type": if outcome.passed { "VerificationPassed" } else { "VerificationFailed" },
        "payload": { "job_hash": job_hash.clone(), "exit_code": outcome.exit_code, "violations": violations.clone() },
    });
    let verification_receipt_sha256 = canonical_sha256_hex(&verification_receipt_payload)?;

    // Record verification receipt evidence (idempotent).
    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "receipt".to_string(),
                payload: verification_receipt_payload,
                idempotency_key: Some(format!("verify:{job_hash}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    let verification_command = body
        .request
        .commands
        .last()
        .map(|command| command.cmd.clone())
        .unwrap_or_else(|| "sandbox_run".to_string());
    let verification_duration_ms = body.response.runs.last().map(|run| run.duration_ms);

    // Emit verification event into the receipt bundle (idempotent).
    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "verification".to_string(),
                payload: serde_json::json!({
                    "command": verification_command,
                    "exit_code": outcome.exit_code,
                    "cwd": body.request.repo.mount_path,
                    "duration_ms": verification_duration_ms,
                }),
                idempotency_key: Some(format!("verification:{job_hash}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    let settlement_release_eligible = outcome.passed && price_integrity_passed;
    let mut cep_settlement = None;
    if !use_direct_settlement {
        let cep = cep_route
            .as_ref()
            .ok_or_else(|| ApiError::InvalidRequest("cep config is required".to_string()))?;
        let settled = execute_cep_objective_settlement(
            &state,
            &reservation,
            body.provider_id.trim(),
            job_hash.as_str(),
            settle_amount_msats,
            settlement_release_eligible,
            verification_receipt_sha256.as_str(),
            cep,
        )
        .await?;
        if settlement_release_eligible && settled.outcome != "success" {
            return Err(ApiError::DependencyUnavailable(format!(
                "cep settlement outcome={} envelope_id={}",
                settled.outcome, settled.envelope_id
            )));
        }
        cep_settlement = Some(settled);
    }

    let (settled, _changed) = state
        .treasury
        .settle_compute_job(
            job_hash.as_str(),
            outcome.passed,
            outcome.exit_code,
            price_integrity_passed,
        )
        .await
        .map_err(ApiError::from_treasury)?;

    let settlement_status = match settled.status {
        crate::treasury::SettlementStatus::Released => "released",
        crate::treasury::SettlementStatus::Withheld => "withheld",
        crate::treasury::SettlementStatus::Reserved => "reserved",
    };
    let settlement_route = if use_direct_settlement {
        "direct_liquidity"
    } else {
        "cep_envelope"
    };

    let payment_released = settled.status == crate::treasury::SettlementStatus::Released;
    let payment_amount = if payment_released {
        cep_settlement
            .as_ref()
            .map(|value| value.settled_amount_msats)
            .unwrap_or(settled.amount_msats)
    } else {
        0
    };

    let settled_reservation_id = settled.reservation_id.clone();
    let settled_provider_id = settled.provider_id.clone();

    let withheld_reason = if payment_released {
        None
    } else if !outcome.passed {
        Some("verification_failed")
    } else if !price_integrity_passed {
        Some("price_integrity_failed")
    } else {
        Some("withheld")
    };

    let credit_offer_id = cep_settlement.as_ref().map(|value| value.offer_id.clone());
    let credit_envelope_id = cep_settlement
        .as_ref()
        .map(|value| value.envelope_id.clone());
    let credit_settlement_id = cep_settlement
        .as_ref()
        .map(|value| value.settlement_id.clone());
    let credit_liquidity_receipt_sha256 = cep_settlement
        .as_ref()
        .and_then(|value| value.liquidity_receipt_sha256.clone());

    if let Some(cep) = cep_settlement.as_ref() {
        state
            .orchestrator
            .append_run_event(
                body.run_id,
                AppendRunEventRequest {
                    event_type: "receipt".to_string(),
                    payload: serde_json::json!({
                        "receipt_type": "CepSettlementLinked",
                        "payload": {
                            "job_hash": job_hash.clone(),
                            "offer_id": cep.offer_id.clone(),
                            "envelope_id": cep.envelope_id.clone(),
                            "settlement_id": cep.settlement_id.clone(),
                            "settlement_outcome": cep.outcome.clone(),
                            "liquidity_receipt_sha256": cep.liquidity_receipt_sha256.clone(),
                            "verification_receipt_sha256": verification_receipt_sha256.clone(),
                        }
                    }),
                    idempotency_key: Some(format!(
                        "cep-settlement:{job_hash}:{}",
                        cep.settlement_id
                    )),
                    expected_previous_seq: None,
                },
            )
            .await
            .map_err(ApiError::from_orchestration)?;
    }

    let payment_event = serde_json::json!({
        "rail": "lightning",
        "asset_id": "BTC_LN",
        "amount_msats": payment_amount,
        "payment_proof": if payment_released {
            if let Some(cep) = cep_settlement.as_ref() {
                serde_json::json!({
                    "type": "cep_envelope",
                    "reservation_id": settled_reservation_id.clone(),
                    "provider_id": settled_provider_id.clone(),
                    "offer_id": cep.offer_id.clone(),
                    "envelope_id": cep.envelope_id.clone(),
                    "settlement_id": cep.settlement_id.clone(),
                    "liquidity_receipt_sha256": cep.liquidity_receipt_sha256.clone(),
                })
            } else {
                serde_json::json!({
                    "type": "internal_ledger",
                    "reservation_id": settled_reservation_id.clone(),
                    "provider_id": settled_provider_id.clone(),
                })
            }
        } else {
            serde_json::json!({
                "type": "withheld",
                "reservation_id": settled_reservation_id.clone(),
                "reason": withheld_reason,
                "exit_code": outcome.exit_code,
            })
        },
        "job_hash": job_hash.clone(),
        "status": settlement_status,
    });

    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "payment".to_string(),
                payload: payment_event,
                idempotency_key: Some(format!("payment:{job_hash}:{settlement_status}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    state
        .orchestrator
        .append_run_event(
            body.run_id,
            AppendRunEventRequest {
                event_type: "receipt".to_string(),
                payload: serde_json::json!({
                    "receipt_type": if payment_released { "PaymentReleased" } else { "PaymentWithheld" },
                    "payload": {
                        "job_hash": job_hash.clone(),
                        "amount_msats": payment_amount,
                        "reason": withheld_reason,
                        "quoted_provider_price_msats": quoted_provider_price_msats,
                        "quoted_total_price_msats": quoted_total_price_msats,
                        "delivered_provider_amount_msats": body.amount_msats,
                        "price_integrity_variance_msats": price_integrity_variance_msats,
                        "price_integrity_variance_bps": price_integrity_variance_bps,
                        "reservation_id": settled_reservation_id,
                        "provider_id": settled_provider_id,
                        "settlement_route": settlement_route,
                        "verification_receipt_sha256": verification_receipt_sha256.clone(),
                        "credit_offer_id": credit_offer_id.clone(),
                        "credit_envelope_id": credit_envelope_id.clone(),
                        "credit_settlement_id": credit_settlement_id.clone(),
                        "credit_liquidity_receipt_sha256": credit_liquidity_receipt_sha256.clone(),
                    }
                }),
                idempotency_key: Some(format!("receipt-payment:{job_hash}:{settlement_status}")),
                expected_previous_seq: None,
            },
        )
        .await
        .map_err(ApiError::from_orchestration)?;

    let run_receipt_path = format!("/internal/v1/runs/{}/receipt", body.run_id);
    let run_replay_path = format!("/internal/v1/runs/{}/replay", body.run_id);
    let run_id_string = body.run_id.to_string();

    if !violations.is_empty() {
        if let Some(incident) = crate::fraud::FraudIncident::new(
            "compute_verification_violation",
            "medium",
            Some(body.provider_id.trim().to_string()),
            Some(body.provider_worker_id.trim().to_string()),
            Some(job_hash.clone()),
            Some(run_id_string.clone()),
            quote_sha256.clone(),
            vec!["payment_withheld".to_string(), "strike_applied".to_string()],
            serde_json::json!({
                "job_hash": job_hash.clone(),
                "run_id": run_id_string.clone(),
                "settlement_status": settlement_status,
                "withheld_reason": withheld_reason,
                "exit_code": outcome.exit_code,
                "violations": violations.clone(),
                "paths": {
                    "run_receipt": run_receipt_path.as_str(),
                    "run_replay": run_replay_path.as_str(),
                }
            }),
        ) {
            let _created = record_fraud_incident(&state, incident.clone()).await;
            let run = state
                .orchestrator
                .append_run_event(
                    body.run_id,
                    AppendRunEventRequest {
                        event_type: "fraud".to_string(),
                        payload: serde_json::json!({
                            "schema": "openagents.fraud.incident_pointer.v1",
                            "incident_id": incident.incident_id,
                            "incident_type": incident.incident_type,
                            "severity": incident.severity,
                            "job_hash": job_hash.clone(),
                        }),
                        idempotency_key: Some(format!(
                            "fraud:{job_hash}:compute_verification_violation"
                        )),
                        expected_previous_seq: None,
                    },
                )
                .await
                .map_err(ApiError::from_orchestration)?;
            publish_latest_run_event(&state, &run).await?;
        }
    }

    if price_integrity_violation {
        if let Some(incident) = crate::fraud::FraudIncident::new(
            "compute_price_integrity_violation",
            "high",
            Some(body.provider_id.trim().to_string()),
            Some(body.provider_worker_id.trim().to_string()),
            Some(job_hash.clone()),
            Some(run_id_string.clone()),
            quote_sha256.clone(),
            vec![
                "payment_withheld".to_string(),
                "price_integrity_penalty".to_string(),
            ],
            serde_json::json!({
                "job_hash": job_hash.clone(),
                "run_id": run_id_string.clone(),
                "settlement_status": settlement_status,
                "withheld_reason": withheld_reason,
                "quote_id": quote_id,
                "quote_sha256": quote_sha256,
                "quoted_provider_price_msats": quoted_provider_price_msats,
                "quoted_total_price_msats": quoted_total_price_msats,
                "delivered_provider_amount_msats": body.amount_msats,
                "variance_msats": price_integrity_variance_msats,
                "variance_bps": price_integrity_variance_bps,
                "tolerance_msats": PRICE_INTEGRITY_TOLERANCE_MSATS,
                "paths": {
                    "run_receipt": run_receipt_path.as_str(),
                    "run_replay": run_replay_path.as_str(),
                }
            }),
        ) {
            let _created = record_fraud_incident(&state, incident.clone()).await;
            let run = state
                .orchestrator
                .append_run_event(
                    body.run_id,
                    AppendRunEventRequest {
                        event_type: "fraud".to_string(),
                        payload: serde_json::json!({
                            "schema": "openagents.fraud.incident_pointer.v1",
                            "incident_id": incident.incident_id,
                            "incident_type": incident.incident_type,
                            "severity": incident.severity,
                            "job_hash": job_hash.clone(),
                        }),
                        idempotency_key: Some(format!(
                            "fraud:{job_hash}:compute_price_integrity_violation"
                        )),
                        expected_previous_seq: None,
                    },
                )
                .await
                .map_err(ApiError::from_orchestration)?;
            publish_latest_run_event(&state, &run).await?;
        }
    }

    verify_contract_critical_run_receipt(&state, body.run_id).await?;

    Ok(Json(SettleSandboxRunResponse {
        job_hash,
        reservation_id: settled.reservation_id,
        amount_msats: settled.amount_msats,
        verification_passed: outcome.passed,
        exit_code: outcome.exit_code,
        violations,
        settlement_status: settlement_status.to_string(),
        settlement_route: settlement_route.to_string(),
        credit_offer_id,
        credit_envelope_id,
        credit_settlement_id,
        credit_liquidity_receipt_sha256,
        verification_receipt_sha256: Some(verification_receipt_sha256),
    }))
}

#[derive(Debug, Clone)]
struct CepObjectiveSettlementResult {
    offer_id: String,
    envelope_id: String,
    settlement_id: String,
    outcome: String,
    settled_amount_msats: u64,
    liquidity_receipt_sha256: Option<String>,
}

fn default_true() -> bool {
    true
}

fn should_use_direct_objective_route(
    max_amount_msats: u64,
    policy: &SettleSandboxRunRoutePolicyV1,
) -> bool {
    match policy {
        SettleSandboxRunRoutePolicyV1::DirectOnly => true,
        SettleSandboxRunRoutePolicyV1::ForceCep => false,
        SettleSandboxRunRoutePolicyV1::PreferAgentBalance {
            agent_balance_sats,
            min_reserve_sats,
            direct_allowed,
        } => {
            if !direct_allowed {
                return false;
            }
            agent_balance_sats.saturating_sub(*min_reserve_sats)
                >= msats_to_sats_ceil(max_amount_msats)
        }
    }
}

fn msats_to_sats_ceil(amount_msats: u64) -> u64 {
    if amount_msats == 0 {
        return 0;
    }
    amount_msats.saturating_add(999).saturating_div(1000)
}

fn canonical_sha256_hex(value: &serde_json::Value) -> Result<String, ApiError> {
    let canonical_json = protocol::hash::canonical_json(value)
        .map_err(|error| ApiError::Internal(error.to_string()))?;
    let digest = sha2::Sha256::digest(canonical_json.as_bytes());
    Ok(hex::encode(digest))
}

fn derive_credit_envelope_id(offer_id: &str, provider_id: &str) -> Result<String, ApiError> {
    let fingerprint = serde_json::json!({
        "schema": "openagents.credit.envelope_request.v1",
        "offer_id": offer_id,
        "provider_id": provider_id,
    });
    let request_fingerprint_sha256 = canonical_sha256_hex(&fingerprint)?;
    Ok(format!("cepe_{}", &request_fingerprint_sha256[..24]))
}

async fn execute_cep_objective_settlement(
    state: &AppState,
    reservation: &crate::treasury::ComputeJobSettlement,
    provider_id: &str,
    job_hash: &str,
    settle_amount_msats: u64,
    verification_passed: bool,
    verification_receipt_sha256: &str,
    cep: &SettleSandboxRunCepRouteV1,
) -> Result<CepObjectiveSettlementResult, ApiError> {
    let agent_id = cep.agent_id.trim().to_ascii_lowercase();
    if agent_id.is_empty() {
        return Err(ApiError::InvalidRequest(
            "cep.agent_id is required".to_string(),
        ));
    }
    let pool_id = cep.pool_id.trim().to_ascii_lowercase();
    if pool_id.is_empty() {
        return Err(ApiError::InvalidRequest(
            "cep.pool_id is required".to_string(),
        ));
    }
    let invoice = cep.provider_invoice.trim().to_string();
    if invoice.is_empty() {
        return Err(ApiError::InvalidRequest(
            "cep.provider_invoice is required".to_string(),
        ));
    }
    let host = cep.provider_host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return Err(ApiError::InvalidRequest(
            "cep.provider_host is required".to_string(),
        ));
    }

    let invoice_amount_msats = Bolt11::amount_msats(invoice.as_str()).ok_or_else(|| {
        ApiError::InvalidRequest("cep.provider_invoice must include amount_msats".to_string())
    })?;
    let max_fee_msats = cep.max_fee_msats.unwrap_or(1_000);
    let max_sats = msats_to_sats_ceil(
        invoice_amount_msats
            .saturating_add(max_fee_msats)
            .max(settle_amount_msats.saturating_add(max_fee_msats)),
    );

    let scope_id = cep
        .scope_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(job_hash)
        .to_string();

    let max_ttl_seconds = i64::try_from(state.config.credit_policy.max_offer_ttl_seconds)
        .unwrap_or(3600)
        .max(60);
    let offer_ttl_seconds = cep
        .offer_ttl_seconds
        .unwrap_or(max_ttl_seconds)
        .clamp(60, max_ttl_seconds);
    let exp = reservation.reserved_at + chrono::Duration::seconds(offer_ttl_seconds);
    if exp <= Utc::now() {
        return Err(ApiError::Conflict(
            "cep offer window expired for this reservation".to_string(),
        ));
    }

    let offered = state
        .credit
        .offer(CreditOfferRequestV1 {
            schema: "openagents.credit.offer_request.v1".to_string(),
            agent_id,
            pool_id,
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id,
            max_sats,
            fee_bps: 100,
            requires_verifier: true,
            exp,
        })
        .await
        .map_err(api_error_from_credit)?;

    let envelope_id = match state
        .credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: "openagents.credit.envelope_request.v1".to_string(),
            offer_id: offered.offer.offer_id.clone(),
            provider_id: provider_id.to_string(),
        })
        .await
    {
        Ok(enveloped) => enveloped.envelope.envelope_id,
        Err(CreditError::Conflict(message)) if message == "offer is not in offered status" => {
            derive_credit_envelope_id(offered.offer.offer_id.as_str(), provider_id)?
        }
        Err(error) => return Err(api_error_from_credit(error)),
    };

    let settled = state
        .credit
        .settle(CreditSettleRequestV1 {
            schema: "openagents.credit.settle_request.v1".to_string(),
            envelope_id: envelope_id.clone(),
            verification_passed,
            verification_receipt_sha256: verification_receipt_sha256.to_string(),
            provider_invoice: invoice,
            provider_host: host,
            max_fee_msats,
            policy_context: serde_json::json!({
                "schema": "openagents.credit.policy_context.v1",
                "objective": PHASE0_REQUIRED_PROVIDER_CAPABILITY,
                "job_hash": job_hash,
                "reservation_id": reservation.reservation_id,
                "route": "cep_envelope"
            }),
        })
        .await
        .map_err(api_error_from_credit)?;

    Ok(CepObjectiveSettlementResult {
        offer_id: offered.offer.offer_id,
        envelope_id,
        settlement_id: settled.settlement_id,
        outcome: settled.outcome.clone(),
        settled_amount_msats: settled.spent_sats.saturating_mul(1000),
        liquidity_receipt_sha256: settled.liquidity_receipt_sha256,
    })
}

async fn heartbeat_worker(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(body): Json<WorkerHeartbeatBody>,
) -> Result<Json<WorkerResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let listing_patch = metadata_patch_touches_provider_listing(&body.metadata_patch);
    if body.metadata_patch.get("pricing_stage").is_some()
        || body.metadata_patch.get("pricing_bands").is_some()
    {
        let existing = state
            .workers
            .get_worker(&worker_id, &owner)
            .await
            .map_err(ApiError::from_worker)?;
        let mut merged = existing.worker.metadata.clone();
        merge_metadata_patch_shallow(&mut merged, &body.metadata_patch)?;
        if metadata_has_role(&merged, "provider") {
            qualify_provider_pricing(&merged)?;
        }
    }

    let snapshot = state
        .workers
        .heartbeat(
            &worker_id,
            WorkerHeartbeatRequest {
                owner,
                metadata_patch: body.metadata_patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(&state, &snapshot).await?;
    if listing_patch {
        maybe_spawn_nostr_provider_ad_mirror(&state, &snapshot);
    }
    Ok(Json(WorkerResponse { worker: snapshot }))
}

async fn transition_worker(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(body): Json<WorkerTransitionBody>,
) -> Result<Json<WorkerResponse>, ApiError> {
    ensure_runtime_write_authority(&state)?;
    let owner = owner_from_parts(body.owner_user_id, body.owner_guest_scope)?;
    let snapshot = state
        .workers
        .transition_status(
            &worker_id,
            WorkerStatusTransitionRequest {
                owner,
                status: body.status,
                reason: body.reason,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(&state, &snapshot).await?;
    maybe_spawn_nostr_provider_ad_mirror(&state, &snapshot);
    Ok(Json(WorkerResponse { worker: snapshot }))
}

async fn get_worker_checkpoint(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    let checkpoint = state
        .workers
        .checkpoint_for_worker(&worker_id)
        .await
        .map_err(ApiError::from_worker)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(CheckpointResponse { checkpoint }))
}

fn api_error_from_liquidity(error: LiquidityError) -> ApiError {
    match error {
        LiquidityError::InvalidRequest(message) => ApiError::InvalidRequest(message),
        LiquidityError::NotFound => ApiError::NotFound,
        LiquidityError::Conflict(message) => ApiError::Conflict(message),
        LiquidityError::DependencyUnavailable(message) => ApiError::Internal(message),
        LiquidityError::Internal(message) => ApiError::Internal(message),
    }
}

fn api_error_from_liquidity_pool(error: LiquidityPoolError) -> ApiError {
    match error {
        LiquidityPoolError::InvalidRequest(message) => ApiError::InvalidRequest(message),
        LiquidityPoolError::NotFound => ApiError::NotFound,
        LiquidityPoolError::Conflict(message) => ApiError::Conflict(message),
        LiquidityPoolError::DependencyUnavailable(message) => ApiError::Internal(message),
        LiquidityPoolError::Internal(message) => ApiError::Internal(message),
    }
}

fn api_error_from_fx(error: FxServiceError) -> ApiError {
    match error {
        FxServiceError::InvalidRequest(message) => ApiError::InvalidRequest(message),
        FxServiceError::NotFound(_) => ApiError::NotFound,
        FxServiceError::Conflict(message) => ApiError::Conflict(message),
        FxServiceError::PolicyDenied(message) => ApiError::Forbidden(message),
        FxServiceError::Internal(message) => ApiError::Internal(message),
    }
}

fn api_error_from_credit(error: CreditError) -> ApiError {
    match error {
        CreditError::InvalidRequest(message) => {
            if message == "offer is not in offered status" {
                ApiError::Conflict(message)
            } else {
                ApiError::InvalidRequest(message)
            }
        }
        CreditError::NotFound => ApiError::NotFound,
        CreditError::Conflict(message) => ApiError::Conflict(message),
        CreditError::DependencyUnavailable(message) => ApiError::DependencyUnavailable(message),
        CreditError::Internal(message) => ApiError::Internal(message),
    }
}

#[derive(Debug)]
enum ApiError {
    NotFound,
    Forbidden(String),
    Conflict(String),
    KhalaUnauthorized(String),
    KhalaForbiddenTopic(String),
    KhalaOriginDenied(String),
    PublishRateLimited {
        retry_after_ms: u64,
        reason_code: String,
        topic: String,
        topic_class: String,
        max_publish_per_second: u32,
    },
    PayloadTooLarge {
        reason_code: String,
        topic: String,
        topic_class: String,
        payload_bytes: usize,
        max_payload_bytes: usize,
    },
    RateLimited {
        retry_after_ms: u64,
        reason_code: String,
    },
    SlowConsumerEvicted {
        topic: String,
        lag: u64,
        lag_threshold: u64,
        strikes: u32,
        max_strikes: u32,
        suggested_after_seq: Option<u64>,
    },
    StaleCursor {
        topic: String,
        requested_cursor: u64,
        oldest_available_cursor: u64,
        head_cursor: u64,
        reason_codes: Vec<String>,
        replay_lag: u64,
        replay_budget_events: u64,
        qos_tier: String,
    },
    WritePathFrozen(String),
    DependencyUnavailable(String),
    InvalidRequest(String),
    Internal(String),
}

impl ApiError {
    fn from_orchestration(error: OrchestrationError) -> Self {
        match error {
            OrchestrationError::RunNotFound(_) => Self::NotFound,
            OrchestrationError::EmptyEventType => {
                Self::InvalidRequest("event_type cannot be empty".to_string())
            }
            OrchestrationError::RunStateMachine(state_error) => {
                Self::InvalidRequest(state_error.to_string())
            }
            OrchestrationError::Authority(AuthorityError::SequenceConflict {
                expected_previous_seq,
                actual_previous_seq,
                ..
            }) => Self::Conflict(format!(
                "expected_previous_seq {expected_previous_seq} does not match actual previous seq {actual_previous_seq}"
            )),
            other => Self::Internal(other.to_string()),
        }
    }

    fn from_worker(error: WorkerError) -> Self {
        match error {
            WorkerError::InvalidOwner => Self::InvalidRequest(
                "owner_user_id or owner_guest_scope must be provided (but not both)".to_string(),
            ),
            WorkerError::NotFound(_) => Self::NotFound,
            WorkerError::Forbidden(worker_id) => {
                Self::Forbidden(format!("owner mismatch for worker {worker_id}"))
            }
            WorkerError::InvalidTransition { from, to } => {
                Self::InvalidRequest(format!("invalid worker transition from {from:?} to {to:?}"))
            }
            other => Self::Internal(other.to_string()),
        }
    }

    fn from_treasury(error: crate::treasury::TreasuryError) -> Self {
        match error {
            crate::treasury::TreasuryError::NotReserved => Self::NotFound,
            crate::treasury::TreasuryError::InsufficientBudget => {
                Self::Forbidden("insufficient budget".to_string())
            }
            crate::treasury::TreasuryError::OwnerMismatch
            | crate::treasury::TreasuryError::AmountMismatch
            | crate::treasury::TreasuryError::AlreadySettled
            | crate::treasury::TreasuryError::SettlementConflict => {
                Self::Conflict(error.to_string())
            }
        }
    }

    fn from_artifacts(error: ArtifactError) -> Self {
        Self::Internal(error.to_string())
    }

    fn from_fanout(error: FanoutError) -> Self {
        match error {
            FanoutError::InvalidTopic => {
                Self::InvalidRequest("topic is required for khala fanout operations".to_string())
            }
            FanoutError::StaleCursor {
                topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
            } => Self::StaleCursor {
                topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
            },
            FanoutError::PublishRateLimited {
                topic,
                topic_class,
                reason_code,
                max_publish_per_second,
                retry_after_ms,
            } => {
                tracing::warn!(
                    topic,
                    topic_class,
                    reason_code,
                    max_publish_per_second,
                    retry_after_ms,
                    "khala publish rate limit triggered"
                );
                Self::PublishRateLimited {
                    retry_after_ms,
                    reason_code,
                    topic,
                    topic_class,
                    max_publish_per_second,
                }
            }
            FanoutError::FramePayloadTooLarge {
                topic,
                topic_class,
                reason_code,
                payload_bytes,
                max_payload_bytes,
            } => {
                tracing::warn!(
                    topic,
                    topic_class,
                    reason_code,
                    payload_bytes,
                    max_payload_bytes,
                    "khala publish payload exceeds frame-size limit"
                );
                Self::PayloadTooLarge {
                    reason_code,
                    topic,
                    topic_class,
                    payload_bytes,
                    max_payload_bytes,
                }
            }
        }
    }

    fn from_sync_auth(error: SyncAuthError) -> Self {
        let code = error.code();
        if error.is_unauthorized() {
            tracing::warn!(reason_code = code, reason = %error, "khala auth denied");
            Self::KhalaUnauthorized(code.to_string())
        } else {
            tracing::warn!(reason_code = code, reason = %error, "khala topic denied");
            Self::KhalaForbiddenTopic(code.to_string())
        }
    }
}

fn validate_sandbox_request_phase0(request: &protocol::SandboxRunRequest) -> Result<(), ApiError> {
    if request.commands.is_empty() {
        return Err(ApiError::InvalidRequest(
            "sandbox request must include at least one command".to_string(),
        ));
    }
    if request.commands.len() > 20 {
        return Err(ApiError::InvalidRequest(
            "sandbox request exceeds 20 command cap".to_string(),
        ));
    }
    for command in &request.commands {
        if command.cmd.len() > 4096 {
            return Err(ApiError::InvalidRequest(
                "sandbox command exceeds 4096 byte cap".to_string(),
            ));
        }
    }
    if request.env.len() > 32 {
        return Err(ApiError::InvalidRequest(
            "sandbox env exceeds 32 entry cap".to_string(),
        ));
    }
    if request.sandbox.network_policy != protocol::jobs::sandbox::NetworkPolicy::None {
        return Err(ApiError::InvalidRequest(
            "sandbox network_policy must be none in Phase 0".to_string(),
        ));
    }
    if request.sandbox.resources.timeout_secs > 300 {
        return Err(ApiError::InvalidRequest(
            "sandbox timeout_secs exceeds 300 second cap".to_string(),
        ));
    }
    if request.sandbox.resources.memory_mb > 8192 {
        return Err(ApiError::InvalidRequest(
            "sandbox memory_mb exceeds 8192 cap".to_string(),
        ));
    }
    if request.sandbox.resources.cpus > 8.0 {
        return Err(ApiError::InvalidRequest(
            "sandbox cpus exceeds 8.0 cap".to_string(),
        ));
    }
    Ok(())
}

async fn dispatch_sandbox_request_to_provider(
    selection: &ProviderSelection,
    workers: &[WorkerSnapshot],
    job_hash: &str,
    request: &protocol::SandboxRunRequest,
) -> Result<(protocol::SandboxRunResponse, u64), ApiError> {
    let base_url = selection
        .provider
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::InvalidRequest("routed provider missing base_url".to_string()))?;
    let provider_snapshot = workers
        .iter()
        .find(|snapshot| snapshot.worker.worker_id == selection.provider.worker_id)
        .ok_or_else(|| ApiError::Internal("missing provider snapshot".to_string()))?;
    let max_timeout_secs = provider_snapshot
        .worker
        .metadata
        .pointer("/caps/max_timeout_secs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(60)
        .max(1)
        .min(3_600) as u32;
    if request.sandbox.resources.timeout_secs > max_timeout_secs {
        return Err(ApiError::InvalidRequest(format!(
            "sandbox timeout_secs {} exceeds provider max_timeout_secs {}",
            request.sandbox.resources.timeout_secs, max_timeout_secs
        )));
    }

    let url = format!("{}/v1/sandbox_run", base_url.trim_end_matches('/'));
    let timeout_secs = request.sandbox.resources.timeout_secs.saturating_add(5);
    let started = std::time::Instant::now();
    let resp = reqwest::Client::new()
        .post(url.as_str())
        .header("x-idempotency-key", job_hash)
        .timeout(Duration::from_secs(timeout_secs as u64))
        .json(request)
        .send()
        .await
        .map_err(|error| ApiError::InvalidRequest(format!("provider dispatch failed: {error}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::InvalidRequest(format!(
            "provider dispatch returned {}: {}",
            status, text
        )));
    }
    let parsed = resp
        .json::<protocol::SandboxRunResponse>()
        .await
        .map_err(|error| ApiError::Internal(format!("parse sandbox response: {error}")))?;
    let latency_ms = started.elapsed().as_millis() as u64;
    Ok((parsed, latency_ms))
}

const PROVIDER_VIOLATION_STRIKE_QUARANTINE_THRESHOLD: u64 = 3;
const PROVIDER_FAILURE_STRIKE_QUARANTINE_THRESHOLD: u64 = 5;
const PROVIDER_PRICE_INTEGRITY_QUARANTINE_THRESHOLD: u64 = 3;
const PRICE_INTEGRITY_TOLERANCE_MSATS: u64 = 0;

async fn apply_provider_violation_strike(
    state: &AppState,
    worker_id: &str,
    job_hash: &str,
    violations: &[String],
) -> Result<(), ApiError> {
    ensure_runtime_write_authority(state)?;

    let snapshot = state
        .workers
        .list_all_workers()
        .await
        .into_iter()
        .find(|snapshot| snapshot.worker.worker_id == worker_id)
        .ok_or_else(|| {
            ApiError::InvalidRequest(format!("unknown provider worker_id {worker_id}"))
        })?;

    let current_strikes = snapshot
        .worker
        .metadata
        .get("failure_strikes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    if snapshot
        .worker
        .metadata
        .get("last_violation_job_hash")
        .and_then(serde_json::Value::as_str)
        == Some(job_hash)
    {
        return Ok(());
    }

    let next_strikes = current_strikes.saturating_add(1);
    let mut patch = serde_json::json!({
        "failure_strikes": next_strikes,
        "last_violation_at": Utc::now().to_rfc3339(),
        "last_violation_reason": violations.first().cloned().unwrap_or_else(|| "violation".to_string()),
        "last_violation_job_hash": job_hash,
    });
    let should_quarantine = next_strikes >= PROVIDER_VIOLATION_STRIKE_QUARANTINE_THRESHOLD;
    if should_quarantine {
        if let Some(map) = patch.as_object_mut() {
            map.insert("quarantined".to_string(), serde_json::Value::Bool(true));
            map.insert(
                "quarantine_reason".to_string(),
                serde_json::Value::String("verification_violations".to_string()),
            );
            map.insert(
                "quarantined_at".to_string(),
                serde_json::Value::String(Utc::now().to_rfc3339()),
            );
        }
    }

    let updated = state
        .workers
        .heartbeat(
            worker_id,
            WorkerHeartbeatRequest {
                owner: snapshot.worker.owner.clone(),
                metadata_patch: patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(state, &updated).await?;

    if should_quarantine && updated.worker.status != WorkerStatus::Failed {
        let transitioned = state
            .workers
            .transition_status(
                worker_id,
                WorkerStatusTransitionRequest {
                    owner: updated.worker.owner.clone(),
                    status: WorkerStatus::Failed,
                    reason: Some("quarantined".to_string()),
                },
            )
            .await
            .map_err(ApiError::from_worker)?;
        publish_worker_snapshot(state, &transitioned).await?;
    }

    if should_quarantine {
        let provider_id = snapshot
            .worker
            .metadata
            .get("provider_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        if let Some(incident) = crate::fraud::FraudIncident::new(
            "provider_quarantined",
            "critical",
            provider_id,
            Some(worker_id.to_string()),
            Some(job_hash.to_string()),
            None,
            None,
            vec![
                "quarantined".to_string(),
                "verification_violations".to_string(),
            ],
            serde_json::json!({
                "reason_code": "verification_violations",
                "failure_strikes": next_strikes,
                "threshold": PROVIDER_VIOLATION_STRIKE_QUARANTINE_THRESHOLD,
                "violations": violations,
            }),
        ) {
            let _created = record_fraud_incident(state, incident).await;
        }
    }

    Ok(())
}

async fn apply_provider_failure_strike(
    state: &AppState,
    worker_id: &str,
    job_hash: &str,
    reason: &str,
) -> Result<(), ApiError> {
    ensure_runtime_write_authority(state)?;

    let snapshot = state
        .workers
        .list_all_workers()
        .await
        .into_iter()
        .find(|snapshot| snapshot.worker.worker_id == worker_id)
        .ok_or_else(|| {
            ApiError::InvalidRequest(format!("unknown provider worker_id {worker_id}"))
        })?;

    if snapshot
        .worker
        .metadata
        .get("last_failure_job_hash")
        .and_then(serde_json::Value::as_str)
        == Some(job_hash)
    {
        return Ok(());
    }

    let current_strikes = snapshot
        .worker
        .metadata
        .get("failure_strikes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let next_strikes = current_strikes.saturating_add(1);
    let mut patch = serde_json::json!({
        "failure_strikes": next_strikes,
        "last_failure_at": Utc::now().to_rfc3339(),
        "last_failure_reason": reason,
        "last_failure_job_hash": job_hash,
    });

    let should_quarantine = next_strikes >= PROVIDER_FAILURE_STRIKE_QUARANTINE_THRESHOLD;
    if should_quarantine {
        if let Some(map) = patch.as_object_mut() {
            map.insert("quarantined".to_string(), serde_json::Value::Bool(true));
            map.insert(
                "quarantine_reason".to_string(),
                serde_json::Value::String("provider_failures".to_string()),
            );
            map.insert(
                "quarantined_at".to_string(),
                serde_json::Value::String(Utc::now().to_rfc3339()),
            );
        }
    }

    let updated = state
        .workers
        .heartbeat(
            worker_id,
            WorkerHeartbeatRequest {
                owner: snapshot.worker.owner.clone(),
                metadata_patch: patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(state, &updated).await?;

    if should_quarantine && updated.worker.status != WorkerStatus::Failed {
        let transitioned = state
            .workers
            .transition_status(
                worker_id,
                WorkerStatusTransitionRequest {
                    owner: updated.worker.owner.clone(),
                    status: WorkerStatus::Failed,
                    reason: Some("quarantined".to_string()),
                },
            )
            .await
            .map_err(ApiError::from_worker)?;
        publish_worker_snapshot(state, &transitioned).await?;
    }

    if should_quarantine {
        let provider_id = snapshot
            .worker
            .metadata
            .get("provider_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        if let Some(incident) = crate::fraud::FraudIncident::new(
            "provider_quarantined",
            "critical",
            provider_id,
            Some(worker_id.to_string()),
            Some(job_hash.to_string()),
            None,
            None,
            vec!["quarantined".to_string(), "provider_failures".to_string()],
            serde_json::json!({
                "reason_code": "provider_failures",
                "failure_strikes": next_strikes,
                "threshold": PROVIDER_FAILURE_STRIKE_QUARANTINE_THRESHOLD,
                "reason": reason,
            }),
        ) {
            let _created = record_fraud_incident(state, incident).await;
        }
    }

    Ok(())
}

async fn apply_provider_success_signal(
    state: &AppState,
    worker_id: &str,
    job_hash: &str,
) -> Result<(), ApiError> {
    ensure_runtime_write_authority(state)?;

    let snapshot = state
        .workers
        .list_all_workers()
        .await
        .into_iter()
        .find(|snapshot| snapshot.worker.worker_id == worker_id)
        .ok_or_else(|| {
            ApiError::InvalidRequest(format!("unknown provider worker_id {worker_id}"))
        })?;

    if snapshot
        .worker
        .metadata
        .get("last_success_job_hash")
        .and_then(serde_json::Value::as_str)
        == Some(job_hash)
    {
        return Ok(());
    }

    let current_success = snapshot
        .worker
        .metadata
        .get("success_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let next_success = current_success.saturating_add(1);
    let patch = serde_json::json!({
        "success_count": next_success,
        "last_success_at": Utc::now().to_rfc3339(),
        "last_success_job_hash": job_hash,
    });

    let updated = state
        .workers
        .heartbeat(
            worker_id,
            WorkerHeartbeatRequest {
                owner: snapshot.worker.owner.clone(),
                metadata_patch: patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(state, &updated).await?;
    Ok(())
}

async fn apply_provider_price_integrity_signal(
    state: &AppState,
    worker_id: &str,
    job_hash: &str,
    quote_id: Option<&str>,
    quote_sha256: Option<&str>,
    quoted_provider_price_msats: u64,
    delivered_provider_amount_msats: u64,
    variance_msats: i64,
    variance_bps: u64,
    violation: bool,
) -> Result<(), ApiError> {
    ensure_runtime_write_authority(state)?;

    let snapshot = state
        .workers
        .list_all_workers()
        .await
        .into_iter()
        .find(|snapshot| snapshot.worker.worker_id == worker_id)
        .ok_or_else(|| {
            ApiError::InvalidRequest(format!("unknown provider worker_id {worker_id}"))
        })?;

    if snapshot
        .worker
        .metadata
        .get("last_price_integrity_job_hash")
        .and_then(serde_json::Value::as_str)
        == Some(job_hash)
    {
        return Ok(());
    }

    let current_samples = snapshot
        .worker
        .metadata
        .get("price_integrity_samples")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let current_violations = snapshot
        .worker
        .metadata
        .get("price_integrity_violations")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    let next_samples = current_samples.saturating_add(1);
    let next_violations = if violation {
        current_violations.saturating_add(1)
    } else {
        current_violations
    };

    let mut patch = serde_json::json!({
        "price_integrity_samples": next_samples,
        "price_integrity_violations": next_violations,
        "last_price_variance_bps": variance_bps,
        "last_price_integrity_at": Utc::now().to_rfc3339(),
        "last_price_integrity_job_hash": job_hash,
        "last_price_integrity_quote_id": quote_id,
        "last_price_integrity_quote_sha256": quote_sha256,
        "last_price_integrity_quoted_provider_price_msats": quoted_provider_price_msats,
        "last_price_integrity_delivered_provider_amount_msats": delivered_provider_amount_msats,
        "last_price_integrity_variance_msats": variance_msats,
    });

    let should_quarantine = next_violations >= PROVIDER_PRICE_INTEGRITY_QUARANTINE_THRESHOLD;
    if should_quarantine {
        if let Some(map) = patch.as_object_mut() {
            map.insert("quarantined".to_string(), serde_json::Value::Bool(true));
            map.insert(
                "quarantine_reason".to_string(),
                serde_json::Value::String("price_integrity".to_string()),
            );
            map.insert(
                "quarantined_at".to_string(),
                serde_json::Value::String(Utc::now().to_rfc3339()),
            );
        }
    }

    let updated = state
        .workers
        .heartbeat(
            worker_id,
            WorkerHeartbeatRequest {
                owner: snapshot.worker.owner.clone(),
                metadata_patch: patch,
            },
        )
        .await
        .map_err(ApiError::from_worker)?;
    publish_worker_snapshot(state, &updated).await?;
    maybe_spawn_nostr_provider_ad_mirror(state, &updated);

    if should_quarantine && updated.worker.status != WorkerStatus::Failed {
        let transitioned = state
            .workers
            .transition_status(
                worker_id,
                WorkerStatusTransitionRequest {
                    owner: updated.worker.owner.clone(),
                    status: WorkerStatus::Failed,
                    reason: Some("quarantined".to_string()),
                },
            )
            .await
            .map_err(ApiError::from_worker)?;
        publish_worker_snapshot(state, &transitioned).await?;
        maybe_spawn_nostr_provider_ad_mirror(state, &transitioned);
    }

    if should_quarantine {
        let provider_id = snapshot
            .worker
            .metadata
            .get("provider_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        if let Some(incident) = crate::fraud::FraudIncident::new(
            "provider_quarantined",
            "critical",
            provider_id,
            Some(worker_id.to_string()),
            Some(job_hash.to_string()),
            None,
            quote_sha256.map(str::to_string),
            vec!["quarantined".to_string(), "price_integrity".to_string()],
            serde_json::json!({
                "reason_code": "price_integrity",
                "price_integrity_samples": next_samples,
                "price_integrity_violations": next_violations,
                "threshold": PROVIDER_PRICE_INTEGRITY_QUARANTINE_THRESHOLD,
                "quoted_provider_price_msats": quoted_provider_price_msats,
                "delivered_provider_amount_msats": delivered_provider_amount_msats,
                "variance_msats": variance_msats,
                "variance_bps": variance_bps,
                "quote_id": quote_id,
                "violation": violation,
            }),
        ) {
            let _created = record_fraud_incident(state, incident).await;
        }
    }

    Ok(())
}

async fn record_fraud_incident(state: &AppState, incident: crate::fraud::FraudIncident) -> bool {
    let created = state.fraud.record(incident.clone()).await;
    if !created {
        return false;
    }

    let topic = "fraud:incidents";
    let seq = state
        .fraud_seq
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1);
    let payload = serde_json::to_value(&incident)
        .unwrap_or_else(|_| serde_json::json!({"error": "incident_serialization_failed"}));

    if let Err(error) = state
        .fanout
        .publish(
            topic,
            FanoutMessage {
                topic: topic.to_string(),
                sequence: seq,
                kind: incident.incident_type.clone(),
                payload,
                published_at: Utc::now(),
            },
        )
        .await
    {
        tracing::warn!(reason = %error, "fraud incident publish failed");
    }

    created
}

async fn publish_latest_run_event(state: &AppState, run: &RuntimeRun) -> Result<(), ApiError> {
    let Some(event) = run.events.last() else {
        return Ok(());
    };
    let topic = format!("run:{}:events", run.id);
    state
        .fanout
        .publish(
            &topic,
            FanoutMessage {
                topic: topic.clone(),
                sequence: event.seq,
                kind: event.event_type.clone(),
                payload: event.payload.clone(),
                published_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_fanout)
}

async fn publish_worker_snapshot(
    state: &AppState,
    snapshot: &WorkerSnapshot,
) -> Result<(), ApiError> {
    let meta = &snapshot.worker.metadata;
    let roles = meta
        .get("roles")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let capabilities = meta
        .get("capabilities")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let provider_id = meta.get("provider_id").and_then(serde_json::Value::as_str);
    let provider_base_url = meta
        .get("provider_base_url")
        .and_then(serde_json::Value::as_str);
    let min_price_msats = meta
        .get("min_price_msats")
        .and_then(serde_json::Value::as_u64);
    let reserve_pool = meta
        .get("reserve_pool")
        .and_then(serde_json::Value::as_bool);
    let qualified = meta.get("qualified").and_then(serde_json::Value::as_bool);
    let failure_strikes = meta
        .get("failure_strikes")
        .and_then(serde_json::Value::as_u64);
    let quarantined = meta.get("quarantined").and_then(serde_json::Value::as_bool);
    let quarantine_reason = meta
        .get("quarantine_reason")
        .and_then(serde_json::Value::as_str);

    let payload = serde_json::json!({
        "worker_id": snapshot.worker.worker_id,
        "status": snapshot.worker.status,
        "latest_seq": snapshot.worker.latest_seq,
        "heartbeat_state": snapshot.liveness.heartbeat_state,
        "heartbeat_age_ms": snapshot.liveness.heartbeat_age_ms,
        "roles": roles,
        "provider_id": provider_id,
        "provider_base_url": provider_base_url,
        "capabilities": capabilities,
        "min_price_msats": min_price_msats,
        "reserve_pool": reserve_pool,
        "qualified": qualified,
        "failure_strikes": failure_strikes,
        "quarantined": quarantined,
        "quarantine_reason": quarantine_reason,
        "owner_user_id": snapshot.worker.owner.user_id,
        "owner_guest_scope": snapshot.worker.owner.guest_scope,
    });

    let topic = format!("worker:{}:lifecycle", snapshot.worker.worker_id);
    state
        .fanout
        .publish(
            &topic,
            FanoutMessage {
                topic: topic.clone(),
                sequence: snapshot.worker.latest_seq,
                kind: snapshot.worker.status.as_event_label().to_string(),
                payload: payload.clone(),
                published_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_fanout)?;

    if let Some(user_id) = snapshot.worker.owner.user_id {
        let fleet_topic = format!("fleet:user:{user_id}:workers");
        let fleet_seq = state
            .fleet_seq
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        state
            .fanout
            .publish(
                &fleet_topic,
                FanoutMessage {
                    topic: fleet_topic.clone(),
                    sequence: fleet_seq,
                    kind: snapshot.worker.status.as_event_label().to_string(),
                    payload,
                    published_at: Utc::now(),
                },
            )
            .await
            .map_err(ApiError::from_fanout)?;
    }

    Ok(())
}

fn maybe_spawn_nostr_provider_ad_mirror(state: &AppState, snapshot: &WorkerSnapshot) {
    if state.config.bridge_nostr_relays.is_empty() {
        return;
    }
    let Some(secret_key) = state.config.bridge_nostr_secret_key else {
        return;
    };

    let Some(payload) = provider_ad_payload_from_snapshot(snapshot) else {
        return;
    };
    let provider_id = payload.provider_id.clone();
    let relays = state.config.bridge_nostr_relays.clone();

    tokio::spawn(async move {
        let event = match build_provider_ad_event(&secret_key, None, &payload) {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(
                    provider_id,
                    reason = %error,
                    "bridge nostr mirror failed to build provider ad"
                );
                return;
            }
        };
        let publisher = BridgeNostrPublisher::new(relays);
        if let Err(error) = publisher.connect().await {
            tracing::warn!(
                provider_id,
                reason = %error,
                "bridge nostr mirror failed to connect to relays"
            );
            return;
        }
        if let Err(error) = publisher.publish(&event).await {
            tracing::warn!(
                provider_id,
                reason = %error,
                "bridge nostr mirror failed to publish provider ad"
            );
        }
    });
}

const PROVIDER_MULTIHOMING_REFRESH_SECS: u64 = 60;

fn maybe_spawn_provider_multihoming_autopilot(state: &AppState) {
    if state.config.bridge_nostr_relays.is_empty() {
        return;
    }
    let Some(secret_key) = state.config.bridge_nostr_secret_key else {
        return;
    };

    let workers = state.workers.clone();
    let relays = state.config.bridge_nostr_relays.clone();

    tokio::spawn(async move {
        let publisher = BridgeNostrPublisher::new(relays);
        if let Err(error) = publisher.connect().await {
            tracing::warn!(
                reason = %error,
                "provider multihoming autopilot failed to connect to relays"
            );
            return;
        }

        loop {
            let snapshots = workers.list_all_workers().await;
            for snapshot in &snapshots {
                let Some(payload) = provider_ad_payload_from_snapshot(snapshot) else {
                    continue;
                };
                let provider_id = payload.provider_id.clone();
                let event = match build_provider_ad_event(&secret_key, None, &payload) {
                    Ok(event) => event,
                    Err(error) => {
                        tracing::warn!(
                            provider_id,
                            reason = %error,
                            "provider multihoming autopilot failed to build provider ad"
                        );
                        continue;
                    }
                };

                if let Err(error) = publisher.publish(&event).await {
                    tracing::warn!(
                        provider_id,
                        reason = %error,
                        "provider multihoming autopilot failed to publish provider ad"
                    );
                }
            }

            tokio::time::sleep(Duration::from_secs(PROVIDER_MULTIHOMING_REFRESH_SECS)).await;
        }
    });
}

fn maybe_spawn_liquidity_pool_snapshot_worker(state: &AppState) {
    if !state.config.liquidity_pool_snapshot_worker_enabled {
        return;
    }
    if !state.config.authority_write_mode.writes_enabled() {
        tracing::info!(
            authority_mode = %state.config.authority_write_mode.as_str(),
            "liquidity pool snapshot worker disabled because authority writes are not enabled"
        );
        return;
    }
    if state.config.liquidity_pool_snapshot_pool_ids.is_empty() {
        tracing::warn!("liquidity pool snapshot worker has no configured pool ids");
        return;
    }

    let pool = state.liquidity_pool.clone();
    let pool_ids = state.config.liquidity_pool_snapshot_pool_ids.clone();
    let interval_seconds = state.config.liquidity_pool_snapshot_interval_seconds.max(1);
    let jitter_seconds = state.config.liquidity_pool_snapshot_jitter_seconds;
    let retention_count = state.config.liquidity_pool_snapshot_retention_count.max(1);

    tokio::spawn(async move {
        loop {
            run_liquidity_pool_snapshot_tick(pool.as_ref(), pool_ids.as_slice(), retention_count)
                .await;

            let sleep_seconds =
                liquidity_pool_snapshot_sleep_seconds(interval_seconds, jitter_seconds);
            tokio::time::sleep(Duration::from_secs(sleep_seconds)).await;
        }
    });
}

async fn run_liquidity_pool_snapshot_tick(
    pool_service: &LiquidityPoolService,
    pool_ids: &[String],
    retention_count: i64,
) {
    for pool_id in pool_ids {
        match pool_service
            .generate_snapshot(pool_id.as_str(), PoolPartitionKindV1::Llp)
            .await
        {
            Ok(snapshot) => {
                if let Err(error) = pool_service
                    .prune_snapshots_keep_latest(
                        pool_id.as_str(),
                        PoolPartitionKindV1::Llp,
                        retention_count,
                    )
                    .await
                {
                    tracing::warn!(
                        pool_id,
                        reason = %error,
                        "liquidity pool snapshot prune failed"
                    );
                }

                tracing::debug!(
                    pool_id,
                    snapshot_id = %snapshot.snapshot.snapshot_id,
                    snapshot_as_of = %snapshot.snapshot.as_of,
                    "liquidity pool snapshot tick generated"
                );
            }
            Err(error) => {
                tracing::warn!(
                    pool_id,
                    reason = %error,
                    "liquidity pool snapshot tick failed"
                );
            }
        }
    }
}

fn liquidity_pool_snapshot_sleep_seconds(interval_seconds: u64, jitter_seconds: u64) -> u64 {
    if jitter_seconds == 0 {
        return interval_seconds.max(1);
    }
    let jitter = (Utc::now().timestamp_subsec_nanos() as u64) % (jitter_seconds + 1);
    interval_seconds.saturating_add(jitter).max(1)
}

fn maybe_spawn_treasury_reconciliation_worker(state: &AppState) {
    if !state.config.treasury_reconciliation_enabled {
        return;
    }
    let treasury = state.treasury.clone();
    let ttl_seconds =
        i64::try_from(state.config.treasury_reservation_ttl_seconds).unwrap_or(i64::MAX);
    let interval_seconds = state.config.treasury_reconciliation_interval_seconds.max(1);
    let max_jobs = state.config.treasury_reconciliation_max_jobs;

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(interval_seconds)).await;
            let summary = treasury
                .reconcile_reserved_compute_jobs(ttl_seconds, max_jobs)
                .await;
            if summary.expired_reservations > 0 {
                tracing::info!(
                    expired_reservations = summary.expired_reservations,
                    freed_msats_total = summary.freed_msats_total,
                    "treasury reconciliation released expired reservations"
                );
            }
        }
    });
}

fn provider_ad_payload_from_snapshot(snapshot: &WorkerSnapshot) -> Option<ProviderAdV1> {
    if !is_provider_worker(&snapshot.worker) {
        return None;
    }

    let meta = &snapshot.worker.metadata;
    let quarantined = meta
        .get("quarantined")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let provider_id = meta
        .get("provider_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| snapshot.worker.worker_id.clone());
    let name = meta
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("OpenAgents Compute Provider")
        .to_string();
    let description = meta
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("OpenAgents Compute provider enrolled in Nexus registry")
        .to_string();
    let website = meta
        .get("website")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let capabilities = meta
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let pricing_stage = match pricing_stage_from_metadata(meta) {
        PricingStage::Fixed => PricingStageV1::Fixed,
        PricingStage::Banded => PricingStageV1::Banded,
        PricingStage::Bidding => PricingStageV1::Bidding,
    };
    let pricing_bands = meta
        .get("pricing_bands")
        .and_then(serde_json::Value::as_array)
        .and_then(|bands| {
            serde_json::from_value::<Vec<PricingBand>>(serde_json::Value::Array(bands.clone())).ok()
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|band| {
            !band.capability.trim().is_empty()
                && band.min_price_msats > 0
                && band.max_price_msats >= band.min_price_msats
        })
        .map(|band| PricingBandV1 {
            capability: band.capability,
            min_price_msats: band.min_price_msats,
            max_price_msats: band.max_price_msats,
            step_msats: band.step_msats,
        })
        .collect::<Vec<_>>();
    let min_price_msats = meta
        .get("min_price_msats")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| pricing_bands.iter().map(|band| band.min_price_msats).min())
        .unwrap_or(1000);
    let caps = meta.get("caps").cloned().filter(|value| !value.is_null());
    let price_integrity_samples = meta
        .get("price_integrity_samples")
        .and_then(serde_json::Value::as_u64);
    let price_integrity_violations = meta
        .get("price_integrity_violations")
        .and_then(serde_json::Value::as_u64);
    let last_price_variance_bps = meta
        .get("last_price_variance_bps")
        .and_then(serde_json::Value::as_u64);
    let worker_status = match snapshot.worker.status {
        WorkerStatus::Starting => "starting",
        WorkerStatus::Running => "running",
        WorkerStatus::Stopping => "stopping",
        WorkerStatus::Stopped => "stopped",
        WorkerStatus::Failed => "failed",
    }
    .to_string();
    let heartbeat_state = snapshot.liveness.heartbeat_state.clone();
    let availability = if snapshot.worker.status == WorkerStatus::Running
        && snapshot.liveness.heartbeat_state == "fresh"
        && !quarantined
    {
        "available"
    } else {
        "unavailable"
    }
    .to_string();

    Some(ProviderAdV1 {
        provider_id,
        name,
        description,
        website,
        availability: Some(availability),
        worker_status: Some(worker_status),
        heartbeat_state: Some(heartbeat_state),
        caps,
        price_integrity_samples,
        price_integrity_violations,
        last_price_variance_bps,
        capabilities,
        min_price_msats,
        pricing_stage,
        pricing_bands,
    })
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "not_found",
                })),
            )
                .into_response(),
            Self::Forbidden(message) => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "forbidden",
                    "message": message,
                })),
            )
                .into_response(),
            Self::KhalaUnauthorized(reason_code) => (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "unauthorized",
                    "message": "khala sync authorization failed",
                    "reason_code": reason_code,
                })),
            )
                .into_response(),
            Self::KhalaForbiddenTopic(reason_code) => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "forbidden_topic",
                    "message": "topic subscription is not authorized",
                    "reason_code": reason_code,
                })),
            )
                .into_response(),
            Self::KhalaOriginDenied(reason_code) => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "forbidden_origin",
                    "message": "origin is not allowed for khala access",
                    "reason_code": reason_code,
                })),
            )
                .into_response(),
            Self::PublishRateLimited {
                retry_after_ms,
                reason_code,
                topic,
                topic_class,
                max_publish_per_second,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "message": "khala publish rate limit exceeded",
                    "reason_code": reason_code,
                    "retry_after_ms": retry_after_ms,
                    "topic": topic,
                    "topic_class": topic_class,
                    "max_publish_per_second": max_publish_per_second,
                })),
            )
                .into_response(),
            Self::PayloadTooLarge {
                reason_code,
                topic,
                topic_class,
                payload_bytes,
                max_payload_bytes,
            } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({
                    "error": "payload_too_large",
                    "message": "khala frame payload exceeds configured limit",
                    "reason_code": reason_code,
                    "topic": topic,
                    "topic_class": topic_class,
                    "payload_bytes": payload_bytes,
                    "max_payload_bytes": max_payload_bytes,
                })),
            )
                .into_response(),
            Self::RateLimited {
                retry_after_ms,
                reason_code,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "message": "poll interval guard triggered",
                    "reason_code": reason_code,
                    "retry_after_ms": retry_after_ms,
                })),
            )
                .into_response(),
            Self::SlowConsumerEvicted {
                topic,
                lag,
                lag_threshold,
                strikes,
                max_strikes,
                suggested_after_seq,
            } => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "slow_consumer_evicted",
                    "message": "consumer lag exceeded threshold repeatedly",
                    "reason_code": "slow_consumer_evicted",
                    "details": {
                        "topic": topic,
                        "lag": lag,
                        "lag_threshold": lag_threshold,
                        "strikes": strikes,
                        "max_strikes": max_strikes,
                        "suggested_after_seq": suggested_after_seq,
                        "recovery": "advance_cursor_or_rebootstrap"
                    }
                })),
            )
                .into_response(),
            Self::Conflict(message) => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "conflict",
                    "message": message,
                })),
            )
                .into_response(),
            Self::StaleCursor {
                topic,
                requested_cursor,
                oldest_available_cursor,
                head_cursor,
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
            } => (
                StatusCode::GONE,
                Json(serde_json::json!({
                    "error": "stale_cursor",
                    "message": "cursor cannot be resumed from retained stream window",
                    "details": {
                        "topic": topic,
                        "requested_cursor": requested_cursor,
                        "oldest_available_cursor": oldest_available_cursor,
                        "head_cursor": head_cursor,
                        "reason_codes": reason_codes,
                        "replay_lag": replay_lag,
                        "replay_budget_events": replay_budget_events,
                        "qos_tier": qos_tier,
                        "recovery": "reset_local_watermark_and_replay_bootstrap"
                    },
                })),
            )
                .into_response(),
            Self::WritePathFrozen(message) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "write_path_frozen",
                    "message": message,
                })),
            )
                .into_response(),
            Self::DependencyUnavailable(message) => (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": "dependency_unavailable",
                    "message": message,
                })),
            )
                .into_response(),
            Self::InvalidRequest(message) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "message": message,
                })),
            )
                .into_response(),
            Self::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "internal",
                    "message": message,
                })),
            )
                .into_response(),
        }
    }
}

#[cfg(test)]
mod tests;
