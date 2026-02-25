use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, Method, Request},
    response::IntoResponse,
};
use chrono::Utc;
use futures::StreamExt;
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error as WsError, http::StatusCode as WsStatusCode};
use tower::ServiceExt;

use super::{AppState, build_router};
use crate::{
    authority::InMemoryRuntimeAuthority,
    config::{AuthorityWriteMode, Config},
    event_log::DurableEventLog,
    fanout::FanoutHub,
    orchestration::RuntimeOrchestrator,
    projectors::InMemoryProjectionPipeline,
    sync_auth::{SyncAuthConfig, SyncAuthorizer, SyncTokenClaims},
    workers::InMemoryWorkerRegistry,
};

const TEST_SYNC_SIGNING_KEY: &str = "runtime-sync-test-key";

fn loopback_bind_addr() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, 1], 0))
}

fn build_test_router_with_config(
    mode: AuthorityWriteMode,
    revoked_jtis: HashSet<String>,
    mutate_config: impl FnOnce(&mut Config),
) -> axum::Router {
    let projectors = InMemoryProjectionPipeline::shared();
    let projector_pipeline: Arc<dyn crate::projectors::ProjectionPipeline> = projectors.clone();
    let revoked_for_config = revoked_jtis.clone();
    let mut config = Config {
        service_name: "runtime-test".to_string(),
        bind_addr: loopback_bind_addr(),
        build_sha: "test".to_string(),
        db_url: None,
        authority_write_mode: mode,
        fanout_driver: "memory".to_string(),
        fanout_queue_capacity: 64,
        khala_poll_default_limit: 100,
        khala_poll_max_limit: 200,
        khala_outbound_queue_limit: 200,
        khala_fair_topic_slice_limit: 50,
        khala_poll_min_interval_ms: 250,
        khala_slow_consumer_lag_threshold: 300,
        khala_slow_consumer_max_strikes: 3,
        khala_consumer_registry_capacity: 4096,
        khala_reconnect_base_backoff_ms: 400,
        khala_reconnect_jitter_ms: 250,
        khala_enforce_origin: true,
        khala_allowed_origins: HashSet::from([
            "https://openagents.com".to_string(),
            "https://www.openagents.com".to_string(),
        ]),
        khala_run_events_publish_rate_per_second: 240,
        khala_worker_lifecycle_publish_rate_per_second: 180,
        khala_codex_worker_events_publish_rate_per_second: 240,
        khala_fallback_publish_rate_per_second: 90,
        khala_run_events_replay_budget_events: 20_000,
        khala_worker_lifecycle_replay_budget_events: 10_000,
        khala_codex_worker_events_replay_budget_events: 3_000,
        khala_fallback_replay_budget_events: 500,
        khala_run_events_max_payload_bytes: 256 * 1024,
        khala_worker_lifecycle_max_payload_bytes: 64 * 1024,
        khala_codex_worker_events_max_payload_bytes: 128 * 1024,
        khala_fallback_max_payload_bytes: 64 * 1024,
        sync_token_signing_key: TEST_SYNC_SIGNING_KEY.to_string(),
        sync_token_issuer: "https://openagents.com".to_string(),
        sync_token_audience: "openagents-sync".to_string(),
        sync_token_require_jti: true,
        sync_token_max_age_seconds: 300,
        sync_revoked_jtis: revoked_for_config,
        verifier_strict: false,
        verifier_allowed_signer_pubkeys: HashSet::new(),
        bridge_nostr_relays: Vec::new(),
        bridge_nostr_secret_key: None,
        liquidity_wallet_executor_base_url: None,
        liquidity_wallet_executor_auth_token: None,
        liquidity_wallet_executor_timeout_ms: 12_000,
        liquidity_quote_ttl_seconds: 60,
        liquidity_pool_withdraw_delay_hours: 24,
        liquidity_pool_withdraw_throttle: crate::config::LiquidityPoolWithdrawThrottleConfig {
            lp_mode_enabled: false,
            stress_liability_ratio_bps: 2_500,
            halt_liability_ratio_bps: 5_000,
            stress_connected_ratio_bps: 7_500,
            halt_connected_ratio_bps: 4_000,
            stress_outbound_coverage_bps: 10_000,
            halt_outbound_coverage_bps: 5_000,
            stress_extra_delay_hours: 24,
            halt_extra_delay_hours: 72,
            stress_execution_cap_per_tick: 5,
        },
        liquidity_pool_snapshot_worker_enabled: false,
        liquidity_pool_snapshot_pool_ids: vec!["llp-main".to_string()],
        liquidity_pool_snapshot_interval_seconds: 60,
        liquidity_pool_snapshot_jitter_seconds: 0,
        liquidity_pool_snapshot_retention_count: 120,
        hydra_fx_policy: crate::config::HydraFxPolicyConfig::default(),
        credit_policy: crate::credit::service::CreditPolicyConfig::default(),
        treasury_reconciliation_enabled: false,
        treasury_reservation_ttl_seconds: 3600,
        treasury_reconciliation_interval_seconds: 60,
        treasury_reconciliation_max_jobs: 200,
    };
    mutate_config(&mut config);
    let fanout_limits = config.khala_fanout_limits();
    let fanout_capacity = config.fanout_queue_capacity;
    let state = AppState::new(
        config,
        Arc::new(RuntimeOrchestrator::new(
            Arc::new(InMemoryRuntimeAuthority::with_event_log(
                DurableEventLog::new_memory(),
            )),
            projector_pipeline,
        )),
        Arc::new(InMemoryWorkerRegistry::new(projectors, 120_000)),
        Arc::new(FanoutHub::memory_with_limits(
            fanout_capacity,
            fanout_limits,
        )),
        Arc::new(SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: TEST_SYNC_SIGNING_KEY.to_string(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 300,
            revoked_jtis,
        })),
        None,
    );
    build_router(state)
}

fn test_router_with_mode_and_revoked(
    mode: AuthorityWriteMode,
    revoked_jtis: HashSet<String>,
) -> axum::Router {
    build_test_router_with_config(mode, revoked_jtis, |_| {})
}

fn test_router_with_mode(mode: AuthorityWriteMode) -> axum::Router {
    test_router_with_mode_and_revoked(mode, HashSet::new())
}

fn test_router() -> axum::Router {
    test_router_with_mode(AuthorityWriteMode::RustActive)
}

fn fx_expected_reservation_id(rfq_id: &str, quote_id: &str) -> Result<String> {
    #[derive(serde::Serialize)]
    struct SettlementJobHashInput<'a> {
        rfq_id: &'a str,
        quote_id: &'a str,
    }
    let hash = protocol::hash::canonical_hash(&SettlementJobHashInput { rfq_id, quote_id })?;
    let job_hash = format!("fxjob_{}", &hash[..24]);
    Ok(format!("rsv_{}", &job_hash[..16]))
}

async fn spawn_http_server(
    app: axum::Router,
) -> Result<(std::net::SocketAddr, tokio::sync::oneshot::Sender<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let _ = server.await;
    });
    Ok((addr, shutdown_tx))
}

async fn spawn_provider_stub() -> Result<(String, tokio::sync::oneshot::Sender<()>)> {
    let app = axum::Router::new().route(
        "/healthz",
        axum::routing::get(|| async { (axum::http::StatusCode::OK, "ok") }),
    );
    let (addr, shutdown) = spawn_http_server(app).await?;
    Ok((format!("http://{addr}"), shutdown))
}

async fn spawn_compute_provider_stub() -> Result<(String, tokio::sync::oneshot::Sender<()>)> {
    async fn sandbox_run(
        axum::Json(request): axum::Json<protocol::SandboxRunRequest>,
    ) -> axum::Json<protocol::SandboxRunResponse> {
        let runs = request
            .commands
            .iter()
            .map(|cmd| protocol::jobs::sandbox::CommandResult {
                cmd: cmd.cmd.clone(),
                exit_code: 0,
                duration_ms: 1,
                stdout_sha256: "stdout".to_string(),
                stderr_sha256: "stderr".to_string(),
                stdout_preview: None,
                stderr_preview: None,
            })
            .collect::<Vec<_>>();
        axum::Json(protocol::SandboxRunResponse {
            env_info: protocol::jobs::sandbox::EnvInfo {
                image_digest: request.sandbox.image_digest.clone(),
                hostname: None,
                system_info: None,
            },
            runs,
            artifacts: Vec::new(),
            status: protocol::jobs::sandbox::SandboxStatus::Success,
            error: None,
            provenance: protocol::Provenance::new("stub"),
        })
    }

    let app = axum::Router::new()
        .route(
            "/healthz",
            axum::routing::get(|| async { (axum::http::StatusCode::OK, "ok") }),
        )
        .route("/v1/sandbox_run", axum::routing::post(sandbox_run));
    let (addr, shutdown) = spawn_http_server(app).await?;
    Ok((format!("http://{addr}"), shutdown))
}

async fn spawn_cancelled_compute_provider_stub()
-> Result<(String, tokio::sync::oneshot::Sender<()>)> {
    async fn sandbox_run(
        axum::Json(request): axum::Json<protocol::SandboxRunRequest>,
    ) -> axum::Json<protocol::SandboxRunResponse> {
        let runs = request
            .commands
            .iter()
            .map(|cmd| protocol::jobs::sandbox::CommandResult {
                cmd: cmd.cmd.clone(),
                exit_code: 1,
                duration_ms: 1,
                stdout_sha256: "stdout".to_string(),
                stderr_sha256: "stderr".to_string(),
                stdout_preview: None,
                stderr_preview: None,
            })
            .collect::<Vec<_>>();
        axum::Json(protocol::SandboxRunResponse {
            env_info: protocol::jobs::sandbox::EnvInfo {
                image_digest: request.sandbox.image_digest.clone(),
                hostname: None,
                system_info: None,
            },
            runs,
            artifacts: Vec::new(),
            status: protocol::jobs::sandbox::SandboxStatus::Cancelled,
            error: None,
            provenance: protocol::Provenance::new("stub-cancelled"),
        })
    }

    let app = axum::Router::new()
        .route(
            "/healthz",
            axum::routing::get(|| async { (axum::http::StatusCode::OK, "ok") }),
        )
        .route("/v1/sandbox_run", axum::routing::post(sandbox_run));
    let (addr, shutdown) = spawn_http_server(app).await?;
    Ok((format!("http://{addr}"), shutdown))
}

#[derive(Clone)]
struct WalletExecutorStubState {
    token: String,
    calls: Arc<std::sync::atomic::AtomicUsize>,
    idempotency: Arc<Mutex<HashMap<String, String>>>,
    receipts: Arc<Mutex<HashMap<String, Value>>>,
}

struct WalletExecutorStubHandle {
    base_url: String,
    shutdown: tokio::sync::oneshot::Sender<()>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletExecutorStubPayBody {
    request_id: String,
    payment: WalletExecutorStubPayPaymentBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletExecutorStubPayPaymentBody {
    invoice: String,
    max_amount_msats: u64,
    host: String,
}

async fn spawn_wallet_executor_stub(token: &str) -> Result<WalletExecutorStubHandle> {
    async fn status(
        State(state): State<WalletExecutorStubState>,
        headers: HeaderMap,
    ) -> axum::response::Response {
        let provided = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        if provided.trim() != format!("Bearer {}", state.token).as_str() {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                axum::Json(json!({
                    "ok": false,
                    "error": {
                        "code": "unauthorized",
                        "message": "missing or invalid bearer token"
                    }
                })),
            )
                .into_response();
        }

        (
            axum::http::StatusCode::OK,
            axum::Json(json!({
                "ok": true,
                "status": {
                    "balanceSats": 250000,
                }
            })),
        )
            .into_response()
    }

    async fn pay_bolt11(
        State(state): State<WalletExecutorStubState>,
        headers: HeaderMap,
        body: Bytes,
    ) -> axum::response::Response {
        let provided = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        if provided.trim() != format!("Bearer {}", state.token).as_str() {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                axum::Json(json!({
                    "ok": false,
                    "error": {
                        "code": "unauthorized",
                        "message": "missing or invalid bearer token"
                    }
                })),
            )
                .into_response();
        }

        let parsed: WalletExecutorStubPayBody = match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(err) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    axum::Json(json!({
                        "ok": false,
                        "error": {"code": "invalid_request", "message": err.to_string()}
                    })),
                )
                    .into_response();
            }
        };

        let request_id = parsed.request_id.trim().to_string();
        if request_id.is_empty() {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(json!({
                    "ok": false,
                    "error": {"code": "invalid_request", "message": "requestId is required"}
                })),
            )
                .into_response();
        }
        if !request_id.starts_with("liqpay:liq_quote_") {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(json!({
                    "ok": false,
                    "error": {"code": "invalid_request", "message": "unexpected requestId prefix"}
                })),
            )
                .into_response();
        }

        let fingerprint = format!(
            "{}|{}|{}",
            parsed.payment.invoice.trim(),
            parsed.payment.max_amount_msats,
            parsed.payment.host.trim().to_ascii_lowercase()
        );

        {
            let mut idempotency = state.idempotency.lock().await;
            if let Some(existing) = idempotency.get(&request_id) {
                if existing != &fingerprint {
                    return (
                        axum::http::StatusCode::CONFLICT,
                        axum::Json(json!({
                            "ok": false,
                            "error": {"code": "conflict", "message": "requestId reused with different payment parameters"}
                        })),
                    )
                        .into_response();
                }
            } else {
                idempotency.insert(request_id.clone(), fingerprint);
            }
        }

        if let Some(existing) = state.receipts.lock().await.get(&request_id).cloned() {
            state
                .calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return (axum::http::StatusCode::OK, axum::Json(existing)).into_response();
        }

        // Deterministic receipt facts for tests.
        fn sha256_hex(data: &[u8]) -> String {
            use sha2::Digest;
            let mut hasher = sha2::Sha256::new();
            hasher.update(data);
            hex::encode(hasher.finalize())
        }

        let invoice_hash = sha256_hex(parsed.payment.invoice.trim().as_bytes());
        let canonical_json_sha256 = sha256_hex(format!("wallet:{request_id}").as_bytes());
        let receipt_id = format!("lwr_{}", &canonical_json_sha256[..24]);
        let preimage_sha256 = sha256_hex(format!("preimage:{request_id}").as_bytes());
        let now_ms = chrono::Utc::now().timestamp_millis();

        let response = json!({
            "ok": true,
            "requestId": request_id,
            "result": {
                "requestId": request_id,
                "walletId": "wallet-test",
                "payment": {
                    "paymentId": format!("pay_{request_id}"),
                    "amountMsats": 0,
                    "preimageHex": "00",
                    "paidAtMs": now_ms
                },
                "quotedAmountMsats": 0,
                "windowSpendMsatsAfterPayment": 0,
                "receipt": {
                    "receiptVersion": "openagents.lightning.wallet_receipt.v1",
                    "receiptId": receipt_id,
                    "requestId": request_id,
                    "walletId": "wallet-test",
                    "host": parsed.payment.host.trim().to_ascii_lowercase(),
                    "paymentId": format!("pay_{request_id}"),
                    "invoiceHash": invoice_hash,
                    "quotedAmountMsats": 0,
                    "settledAmountMsats": 0,
                    "preimageSha256": preimage_sha256,
                    "paidAtMs": now_ms,
                    "rail": "lightning",
                    "assetId": "BTC_LN",
                    "canonicalJsonSha256": canonical_json_sha256,
                }
            }
        });

        state
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        state
            .receipts
            .lock()
            .await
            .insert(request_id.clone(), response.clone());

        (axum::http::StatusCode::OK, axum::Json(response)).into_response()
    }

    let state = WalletExecutorStubState {
        token: token.to_string(),
        calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        idempotency: Arc::new(Mutex::new(HashMap::new())),
        receipts: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = axum::Router::new()
        .route("/status", axum::routing::get(status))
        .route("/pay-bolt11", axum::routing::post(pay_bolt11))
        .with_state(state.clone());
    let (addr, shutdown) = spawn_http_server(app).await?;

    Ok(WalletExecutorStubHandle {
        base_url: format!("http://{addr}"),
        shutdown,
        calls: state.calls,
    })
}

async fn response_json(response: axum::response::Response) -> Result<Value> {
    let collected = response.into_body().collect().await?;
    let bytes = collected.to_bytes();
    Ok(serde_json::from_slice(&bytes)?)
}

fn issue_sync_token(
    scopes: &[&str],
    user_id: Option<u64>,
    org_id: Option<&str>,
    jti: &str,
    exp_offset_seconds: i64,
) -> String {
    issue_sync_token_for_surface(scopes, user_id, org_id, jti, exp_offset_seconds, "ios")
}

fn issue_sync_token_for_surface(
    scopes: &[&str],
    user_id: Option<u64>,
    org_id: Option<&str>,
    jti: &str,
    exp_offset_seconds: i64,
    client_surface: &str,
) -> String {
    let now = Utc::now().timestamp();
    let claims = SyncTokenClaims {
        iss: "https://openagents.com".to_string(),
        aud: "openagents-sync".to_string(),
        sub: match user_id {
            Some(value) => format!("user:{value}"),
            None => "guest:unknown".to_string(),
        },
        exp: (now + exp_offset_seconds) as usize,
        nbf: now as usize,
        iat: now as usize,
        jti: jti.to_string(),
        oa_user_id: user_id,
        oa_org_id: org_id.map(ToString::to_string),
        oa_device_id: Some("ios-device".to_string()),
        oa_client_surface: Some(client_surface.to_string()),
        oa_sync_scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(TEST_SYNC_SIGNING_KEY.as_bytes()),
    )
    .expect("sync token should encode")
}

#[tokio::test]
async fn health_and_readiness_endpoints_are_available() -> Result<()> {
    let app = test_router();

    let health = app
        .clone()
        .oneshot(Request::builder().uri("/healthz").body(Body::empty())?)
        .await?;
    let readiness = app
        .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
        .await?;

    assert_eq!(health.status(), axum::http::StatusCode::OK);
    assert_eq!(readiness.status(), axum::http::StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn internal_openapi_route_includes_credit_and_hydra_endpoints_and_schemas() -> Result<()> {
    let app = test_router();
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/internal/v1/openapi.json")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let json = response_json(response).await?;
    assert_eq!(json.get("openapi").and_then(Value::as_str), Some("3.1.0"));
    assert!(json.pointer("/paths/~1credit~1intent/post").is_some());
    assert!(json.pointer("/paths/~1credit~1offer/post").is_some());
    assert!(json.pointer("/paths/~1credit~1envelope/post").is_some());
    assert!(json.pointer("/paths/~1credit~1settle/post").is_some());
    assert!(json.pointer("/paths/~1credit~1health/get").is_some());
    assert!(
        json.pointer("/paths/~1credit~1agents~1{agent_id}~1exposure/get")
            .is_some()
    );
    assert!(
        json.pointer("/components/schemas/CreditSettleResponseV1/properties/settlement_id")
            .is_some()
    );
    assert!(
        json.pointer("/paths/~1hydra~1routing~1score/post")
            .is_some()
    );
    assert!(json.pointer("/paths/~1hydra~1fx~1rfq/post").is_some());
    assert!(
        json.pointer("/paths/~1hydra~1fx~1rfq~1{rfq_id}/get")
            .is_some()
    );
    assert!(json.pointer("/paths/~1hydra~1fx~1quote/post").is_some());
    assert!(json.pointer("/paths/~1hydra~1fx~1select/post").is_some());
    assert!(json.pointer("/paths/~1hydra~1fx~1settle/post").is_some());
    assert!(json.pointer("/paths/~1hydra~1risk~1health/get").is_some());
    assert!(json.pointer("/paths/~1hydra~1observability/get").is_some());
    assert!(
        json.pointer("/components/schemas/HydraObservabilityResponseV1/properties/routing")
            .is_some()
    );
    assert!(
        json.pointer("/components/schemas/HydraFxRfqRequestV1/properties/schema")
            .is_some()
    );
    assert!(
        json.pointer("/components/schemas/HydraFxSelectResponseV1/properties/decision_sha256")
            .is_some()
    );
    assert!(
        json.pointer("/components/schemas/HydraFxSettleResponseV1/properties/settlement_id")
            .is_some()
    );
    Ok(())
}

#[tokio::test]
async fn comms_delivery_events_endpoint_accepts_and_deduplicates() -> Result<()> {
    let app = test_router();
    let payload = json!({
        "event_id": "resend_evt_123",
        "provider": "resend",
        "delivery_state": "delivered",
        "message_id": "email_abc",
        "integration_id": "resend.primary",
        "recipient": "user@example.com",
        "occurred_at": "2026-02-24T00:00:00Z",
        "payload": {
            "rawType": "email.delivered"
        }
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/comms/delivery-events")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::ACCEPTED);
    let first_json = response_json(first).await?;
    assert_eq!(first_json["status"], "accepted");
    assert_eq!(first_json["idempotentReplay"], false);

    let second = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/comms/delivery-events")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload)?))?,
        )
        .await?;
    assert_eq!(second.status(), axum::http::StatusCode::OK);
    let second_json = response_json(second).await?;
    assert_eq!(second_json["status"], "accepted");
    assert_eq!(second_json["idempotentReplay"], true);

    Ok(())
}

#[tokio::test]
async fn liquidity_quote_pay_is_idempotent_by_idempotency_key() -> Result<()> {
    let app = test_router();

    let payload = json!({
        "schema": "openagents.liquidity.quote_pay_request.v1",
        "idempotency_key": "quote-idempotency-test",
        "invoice": "lnbc420n1test",
        "host": "sats4ai.com",
        "max_amount_msats": 100_000,
        "max_fee_msats": 10_000,
        "urgency": "normal",
        "policy_context": {"purpose": "test"}
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/liquidity/quote_pay")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first).await?;
    let quote_id_first = first_json
        .get("quote_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing quote_id"))?
        .to_string();

    let second = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/liquidity/quote_pay")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload)?))?,
        )
        .await?;
    assert_eq!(second.status(), axum::http::StatusCode::OK);
    let second_json = response_json(second).await?;
    let quote_id_second = second_json
        .get("quote_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing quote_id (second)"))?
        .to_string();

    assert_eq!(quote_id_first, quote_id_second);
    Ok(())
}

#[tokio::test]
async fn liquidity_quote_pay_conflicts_on_idempotency_key_reuse_with_different_invoice()
-> Result<()> {
    let app = test_router();

    let first_payload = json!({
        "schema": "openagents.liquidity.quote_pay_request.v1",
        "idempotency_key": "quote-idempotency-conflict",
        "invoice": "lnbc420n1test",
        "host": "sats4ai.com",
        "max_amount_msats": 100_000,
        "max_fee_msats": 10_000,
        "policy_context": {}
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/liquidity/quote_pay")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&first_payload)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);

    let second_payload = json!({
        "schema": "openagents.liquidity.quote_pay_request.v1",
        "idempotency_key": "quote-idempotency-conflict",
        "invoice": "lnbc10p1test",
        "host": "sats4ai.com",
        "max_amount_msats": 1_000,
        "max_fee_msats": 10,
        "policy_context": {}
    });

    let second = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/liquidity/quote_pay")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&second_payload)?))?,
        )
        .await?;
    assert_eq!(second.status(), axum::http::StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn liquidity_pay_is_idempotent_by_quote_id() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token").await?;
    let wallet_base = wallet.base_url.clone();

    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.liquidity_wallet_executor_base_url = Some(wallet_base);
            config.liquidity_wallet_executor_auth_token = Some("test-token".to_string());
        });

    let quote_payload = json!({
        "schema": "openagents.liquidity.quote_pay_request.v1",
        "idempotency_key": "pay-idempotency-test",
        "invoice": "lnbc420n1test",
        "host": "sats4ai.com",
        "max_amount_msats": 100_000,
        "max_fee_msats": 10_000,
        "policy_context": {}
    });

    let quote_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/liquidity/quote_pay")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&quote_payload)?))?,
        )
        .await?;
    assert_eq!(quote_resp.status(), axum::http::StatusCode::OK);
    let quote_json = response_json(quote_resp).await?;
    let quote_id = quote_json
        .get("quote_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing quote_id"))?
        .to_string();

    let pay_payload = json!({
        "schema": "openagents.liquidity.pay_request.v1",
        "quote_id": quote_id,
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/liquidity/pay")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&pay_payload)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first).await?;
    let first_receipt_hash = first_json
        .pointer("/receipt/canonical_json_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing receipt hash"))?
        .to_string();

    let second = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/liquidity/pay")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&pay_payload)?))?,
        )
        .await?;
    assert_eq!(second.status(), axum::http::StatusCode::OK);
    let second_json = response_json(second).await?;
    let second_receipt_hash = second_json
        .pointer("/receipt/canonical_json_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing receipt hash (second)"))?
        .to_string();

    assert_eq!(first_receipt_hash, second_receipt_hash);
    assert_eq!(
        wallet.calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "wallet executor should be called once for idempotent pay"
    );

    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn liquidity_status_reports_wallet_executor_health_when_configured() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token").await?;
    let wallet_base = wallet.base_url.clone();

    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.liquidity_wallet_executor_base_url = Some(wallet_base);
            config.liquidity_wallet_executor_auth_token = Some("test-token".to_string());
            config.liquidity_quote_ttl_seconds = 90;
        });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/liquidity/status")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let json = response_json(response).await?;
    assert_eq!(json["schema"], "openagents.liquidity.status_response.v1");
    assert_eq!(json["wallet_executor_configured"], true);
    assert_eq!(json["wallet_executor_reachable"], true);
    assert_eq!(json["receipt_signing_enabled"], false);
    assert_eq!(json["quote_ttl_seconds"], 90);
    assert!(json.pointer("/wallet_status/status/balanceSats").is_some());
    assert!(json.get("error_code").is_none());

    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn liquidity_status_reports_clear_error_when_wallet_executor_unreachable() -> Result<()> {
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.liquidity_wallet_executor_base_url = Some("http://127.0.0.1:1".to_string());
            config.liquidity_wallet_executor_auth_token = Some("test-token".to_string());
        });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/liquidity/status")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let json = response_json(response).await?;
    assert_eq!(json["wallet_executor_configured"], true);
    assert_eq!(json["wallet_executor_reachable"], false);
    let code = json
        .get("error_code")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing error_code"))?;
    assert!(code.starts_with("wallet_executor_"));
    assert!(
        json.get("error_message")
            .and_then(Value::as_str)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        "expected non-empty error_message",
    );
    Ok(())
}

#[tokio::test]
async fn credit_routes_support_offer_envelope_settle_and_idempotent_replay() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token").await?;
    let wallet_base = wallet.base_url.clone();

    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.liquidity_wallet_executor_base_url = Some(wallet_base);
            config.liquidity_wallet_executor_auth_token = Some("test-token".to_string());
        });

    let agent_id = "a".repeat(64);
    let pool_id = "b".repeat(64);
    let provider_id = "c".repeat(64);

    let offer_payload = json!({
        "schema": "openagents.credit.offer_request.v1",
        "agent_id": agent_id,
        "pool_id": pool_id,
        "scope_type": "nip90",
        "scope_id": "oa.sandbox_run.v1:test",
        "max_sats": 10_000,
        "fee_bps": 100,
        "requires_verifier": true,
        "exp": (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
    });
    let offer_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/offer")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&offer_payload)?))?,
        )
        .await?;
    assert_eq!(offer_response.status(), axum::http::StatusCode::OK);
    let offer_json = response_json(offer_response).await?;
    assert_eq!(offer_json["schema"], "openagents.credit.offer_response.v1");
    let offer_id = offer_json
        .pointer("/offer/offer_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing offer_id"))?
        .to_string();

    let health_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/credit/health")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(health_response.status(), axum::http::StatusCode::OK);
    let health_json = response_json(health_response).await?;
    assert_eq!(
        health_json["schema"],
        "openagents.credit.health_response.v1"
    );
    assert!(health_json["open_envelope_count"].is_u64());
    assert!(health_json["open_reserved_commitments_sats"].is_u64());

    let exposure_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/credit/agents/{}/exposure",
                    "a".repeat(64)
                ))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(exposure_response.status(), axum::http::StatusCode::OK);
    let exposure_json = response_json(exposure_response).await?;
    assert_eq!(
        exposure_json["schema"],
        "openagents.credit.agent_exposure_response.v1"
    );

    let envelope_payload = json!({
        "schema": "openagents.credit.envelope_request.v1",
        "offer_id": offer_id,
        "provider_id": provider_id,
    });
    let envelope_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/envelope")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&envelope_payload)?))?,
        )
        .await?;
    assert_eq!(envelope_response.status(), axum::http::StatusCode::OK);
    let envelope_json = response_json(envelope_response).await?;
    assert_eq!(
        envelope_json["schema"],
        "openagents.credit.envelope_response.v1"
    );
    let envelope_id = envelope_json
        .pointer("/envelope/envelope_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing envelope_id"))?
        .to_string();

    let settle_payload = json!({
        "schema": "openagents.credit.settle_request.v1",
        "envelope_id": envelope_id,
        "verification_passed": true,
        "verification_receipt_sha256": "sha256:verified",
        "provider_invoice": "lnbc420n1test",
        "provider_host": "provider.example",
        "max_fee_msats": 10_000,
        "policy_context": {},
    });

    let settle_first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/settle")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle_payload)?))?,
        )
        .await?;
    assert_eq!(settle_first.status(), axum::http::StatusCode::OK);
    let settle_first_json = response_json(settle_first).await?;
    assert_eq!(
        settle_first_json["schema"],
        "openagents.credit.settle_response.v1"
    );
    assert_eq!(settle_first_json["outcome"], "success");
    let receipt_sha_first = settle_first_json
        .pointer("/receipt/canonical_json_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing settlement receipt hash"))?
        .to_string();

    let settle_second = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/settle")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle_payload)?))?,
        )
        .await?;
    assert_eq!(settle_second.status(), axum::http::StatusCode::OK);
    let settle_second_json = response_json(settle_second).await?;
    let receipt_sha_second = settle_second_json
        .pointer("/receipt/canonical_json_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing settlement receipt hash (replay)"))?
        .to_string();
    assert_eq!(receipt_sha_first, receipt_sha_second);
    assert_eq!(
        wallet.calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "credit settle idempotent replay must not double-pay",
    );

    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn credit_envelope_conflicts_when_offer_already_consumed() -> Result<()> {
    let app = test_router();
    let offer_payload = json!({
        "schema": "openagents.credit.offer_request.v1",
        "agent_id": "a".repeat(64),
        "pool_id": "b".repeat(64),
        "scope_type": "nip90",
        "scope_id": "oa.sandbox_run.v1:test-conflict",
        "max_sats": 10_000,
        "fee_bps": 100,
        "requires_verifier": true,
        "exp": (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
    });
    let offer_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/offer")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&offer_payload)?))?,
        )
        .await?;
    assert_eq!(offer_response.status(), axum::http::StatusCode::OK);
    let offer_json = response_json(offer_response).await?;
    let offer_id = offer_json
        .pointer("/offer/offer_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing offer_id"))?
        .to_string();

    let envelope_1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/envelope")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.envelope_request.v1",
                    "offer_id": offer_id,
                    "provider_id": "c".repeat(64),
                }))?))?,
        )
        .await?;
    assert_eq!(envelope_1.status(), axum::http::StatusCode::OK);

    let envelope_2 = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/envelope")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.envelope_request.v1",
                    "offer_id": offer_json["offer"]["offer_id"],
                    "provider_id": "d".repeat(64),
                }))?))?,
        )
        .await?;
    assert_eq!(envelope_2.status(), axum::http::StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn credit_health_exposes_effective_policy_from_config() -> Result<()> {
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.credit_policy.max_sats_per_envelope = 1_234;
            config.credit_policy.max_outstanding_envelopes_per_agent = 9;
            config.credit_policy.max_offer_ttl_seconds = 321;
            config.credit_policy.circuit_breaker_min_sample = 7;
            config.credit_policy.loss_rate_halt_threshold = 0.42;
            config.credit_policy.ln_failure_rate_halt_threshold = 0.33;
            config.credit_policy.ln_failure_large_settlement_cap_sats = 777;
        });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/credit/health")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let json = response_json(response).await?;
    assert_eq!(json["schema"], "openagents.credit.health_response.v1");
    assert_eq!(json["policy"]["max_sats_per_envelope"], 1_234);
    assert_eq!(json["policy"]["max_outstanding_envelopes_per_agent"], 9);
    assert_eq!(json["policy"]["max_offer_ttl_seconds"], 321);
    assert_eq!(json["policy"]["circuit_breaker_min_sample"], 7);
    assert_eq!(json["policy"]["loss_rate_halt_threshold"], 0.42);
    assert_eq!(json["policy"]["ln_failure_rate_halt_threshold"], 0.33);
    assert_eq!(json["policy"]["ln_failure_large_settlement_cap_sats"], 777);
    Ok(())
}

#[tokio::test]
async fn credit_offer_enforces_overridden_max_sats_policy() -> Result<()> {
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.credit_policy.max_sats_per_envelope = 900;
        });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/offer")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.offer_request.v1",
                    "agent_id": "a".repeat(64),
                    "pool_id": "b".repeat(64),
                    "scope_type": "nip90",
                    "scope_id": "oa.sandbox_run.v1:max-policy",
                    "max_sats": 901,
                    "fee_bps": 100,
                    "requires_verifier": true,
                    "exp": (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
                }))?))?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let json = response_json(response).await?;
    let message = json
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(message.contains("max_sats_per_envelope"));
    Ok(())
}

#[tokio::test]
async fn credit_settle_returns_bad_gateway_when_wallet_executor_unavailable() -> Result<()> {
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.liquidity_wallet_executor_base_url = Some("http://127.0.0.1:1".to_string());
            config.liquidity_wallet_executor_auth_token = Some("test-token".to_string());
        });

    let offer_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/offer")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.offer_request.v1",
                    "agent_id": "a".repeat(64),
                    "pool_id": "b".repeat(64),
                    "scope_type": "nip90",
                    "scope_id": "oa.sandbox_run.v1:test-dependency",
                    "max_sats": 10_000,
                    "fee_bps": 100,
                    "requires_verifier": true,
                    "exp": (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
                }))?))?,
        )
        .await?;
    assert_eq!(offer_response.status(), axum::http::StatusCode::OK);
    let offer_json = response_json(offer_response).await?;

    let envelope_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/envelope")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.envelope_request.v1",
                    "offer_id": offer_json["offer"]["offer_id"],
                    "provider_id": "c".repeat(64),
                }))?))?,
        )
        .await?;
    assert_eq!(envelope_response.status(), axum::http::StatusCode::OK);
    let envelope_json = response_json(envelope_response).await?;

    let settle_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/settle")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.settle_request.v1",
                    "envelope_id": envelope_json["envelope"]["envelope_id"],
                    "verification_passed": true,
                    "verification_receipt_sha256": "sha256:verified",
                    "provider_invoice": "lnbc420n1test",
                    "provider_host": "provider.example",
                    "max_fee_msats": 10_000,
                    "policy_context": {},
                }))?))?,
        )
        .await?;
    assert_eq!(
        settle_response.status(),
        axum::http::StatusCode::BAD_GATEWAY
    );
    let json = response_json(settle_response).await?;
    assert_eq!(json["error"], "dependency_unavailable");
    Ok(())
}

#[tokio::test]
async fn credit_intent_replays_by_idempotency_key_and_conflicts_on_drift() -> Result<()> {
    let app = test_router();

    let intent_payload = json!({
        "schema": "openagents.credit.intent_request.v1",
        "idempotency_key": "intent-idem-1",
        "agent_id": "a".repeat(64),
        "scope_type": "nip90",
        "scope_id": "oa.sandbox_run.v1:intent-test",
        "max_sats": 7_500,
        "exp": (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
        "policy_context": {"reason": "test"},
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/intent")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&intent_payload)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first).await?;
    assert_eq!(first_json["schema"], "openagents.credit.intent_response.v1");
    let first_intent_id = first_json
        .pointer("/intent/intent_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing intent_id"))?
        .to_string();

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/intent")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&intent_payload)?))?,
        )
        .await?;
    assert_eq!(second.status(), axum::http::StatusCode::OK);
    let second_json = response_json(second).await?;
    let second_intent_id = second_json
        .pointer("/intent/intent_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing intent_id replay"))?;
    assert_eq!(first_intent_id, second_intent_id);

    let drifted = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/intent")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.intent_request.v1",
                    "idempotency_key": "intent-idem-1",
                    "agent_id": "a".repeat(64),
                    "scope_type": "nip90",
                    "scope_id": "oa.sandbox_run.v1:intent-test",
                    "max_sats": 8_000,
                    "exp": (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
                    "policy_context": {"reason": "drift"},
                }))?))?,
        )
        .await?;
    assert_eq!(drifted.status(), axum::http::StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn credit_offer_with_intent_enforces_scope_cap_and_expiry_bounds() -> Result<()> {
    let app = test_router();
    let intent = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/intent")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.intent_request.v1",
                    "idempotency_key": "intent-offer-link-1",
                    "agent_id": "a".repeat(64),
                    "scope_type": "nip90",
                    "scope_id": "oa.sandbox_run.v1:intent-offer",
                    "max_sats": 5_000,
                    "exp": (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
                    "policy_context": {"reason": "offer-link"},
                }))?))?,
        )
        .await?;
    assert_eq!(intent.status(), axum::http::StatusCode::OK);
    let intent_json = response_json(intent).await?;
    let intent_id = intent_json["intent"]["intent_id"]
        .as_str()
        .ok_or_else(|| anyhow!("missing intent_id"))?
        .to_string();

    let too_large_offer = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/offer")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.offer_request.v1",
                    "agent_id": "a".repeat(64),
                    "pool_id": "b".repeat(64),
                    "intent_id": intent_id,
                    "scope_type": "nip90",
                    "scope_id": "oa.sandbox_run.v1:intent-offer",
                    "max_sats": 5_001,
                    "fee_bps": 100,
                    "requires_verifier": true,
                    "exp": (Utc::now() + chrono::Duration::minutes(9)).to_rfc3339(),
                }))?))?,
        )
        .await?;
    assert_eq!(too_large_offer.status(), axum::http::StatusCode::CONFLICT);

    let valid_offer = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/credit/offer")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.credit.offer_request.v1",
                    "agent_id": "a".repeat(64),
                    "pool_id": "b".repeat(64),
                    "intent_id": intent_json["intent"]["intent_id"],
                    "scope_type": "nip90",
                    "scope_id": "oa.sandbox_run.v1:intent-offer",
                    "max_sats": 4_500,
                    "fee_bps": 100,
                    "requires_verifier": true,
                    "exp": (Utc::now() + chrono::Duration::minutes(9)).to_rfc3339(),
                }))?))?,
        )
        .await?;
    assert_eq!(valid_offer.status(), axum::http::StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn run_lifecycle_updates_projector_checkpoint() -> Result<()> {
    let app = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:worker-1",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);

    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let append_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.step.completed",
                    "payload": {"step": 1}
                }))?))?,
        )
        .await?;
    assert_eq!(append_response.status(), axum::http::StatusCode::OK);

    let checkpoint_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/internal/v1/projectors/checkpoints/{run_id}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(checkpoint_response.status(), axum::http::StatusCode::OK);
    let checkpoint_json = response_json(checkpoint_response).await?;
    let last_seq = checkpoint_json
        .pointer("/checkpoint/last_seq")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| anyhow!("missing checkpoint last_seq"))?;
    assert_eq!(last_seq, 2);

    Ok(())
}

#[tokio::test]
async fn run_state_machine_rejects_invalid_terminal_transition() -> Result<()> {
    let app = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:worker-state",
                    "metadata": {"source": "state-machine-test"}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let finish_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.finished",
                    "payload": {"status": "succeeded", "reason_class": "completed"}
                }))?))?,
        )
        .await?;
    assert_eq!(finish_response.status(), axum::http::StatusCode::OK);

    let invalid_transition = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.cancel_requested",
                    "payload": {"reason": "late_cancel"}
                }))?))?,
        )
        .await?;
    assert_eq!(
        invalid_transition.status(),
        axum::http::StatusCode::BAD_REQUEST
    );

    Ok(())
}

#[tokio::test]
async fn append_run_event_supports_idempotency_and_sequence_conflicts() -> Result<()> {
    let app = test_router();
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:worker-idempotency",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let first_append = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.step.completed",
                    "payload": {"step": 1},
                    "idempotency_key": "step-1",
                    "expected_previous_seq": 1
                }))?))?,
        )
        .await?;
    assert_eq!(first_append.status(), axum::http::StatusCode::OK);

    let second_append = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.step.completed",
                    "payload": {"step": 1},
                    "idempotency_key": "step-1",
                    "expected_previous_seq": 1
                }))?))?,
        )
        .await?;
    assert_eq!(second_append.status(), axum::http::StatusCode::OK);
    let second_json = response_json(second_append).await?;
    let event_count = second_json
        .pointer("/run/events")
        .and_then(serde_json::Value::as_array)
        .map_or(0, std::vec::Vec::len);
    assert_eq!(event_count, 2);

    let conflict_append = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.step.completed",
                    "payload": {"step": 2},
                    "idempotency_key": "step-2",
                    "expected_previous_seq": 1
                }))?))?,
        )
        .await?;
    assert_eq!(conflict_append.status(), axum::http::StatusCode::CONFLICT);

    Ok(())
}

#[tokio::test]
async fn run_artifact_endpoints_return_receipt_and_replay() -> Result<()> {
    let secret = nostr::generate_secret_key();
    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.bridge_nostr_secret_key = Some(secret);
    });
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:artifact-worker",
                    "metadata": {"policy_bundle_id": "policy.runtime.v1"}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let receipt_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/internal/v1/runs/{run_id}/receipt"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(receipt_response.status(), axum::http::StatusCode::OK);
    let receipt_json = response_json(receipt_response).await?;
    assert_eq!(
        receipt_json
            .get("schema")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "openagents.receipt.v1"
    );
    let receipt_sha256 = receipt_json
        .get("canonical_json_sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing canonical_json_sha256"))?;
    assert_eq!(receipt_sha256.len(), 64);

    let signature_value = receipt_json
        .get("signature")
        .cloned()
        .ok_or_else(|| anyhow!("missing receipt signature"))?;
    let signer_pubkey = signature_value
        .get("signer_pubkey")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing signature signer_pubkey"))?
        .to_string();
    let signed_sha256 = signature_value
        .get("signed_sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing signature signed_sha256"))?
        .to_string();
    let signature_hex = signature_value
        .get("signature_hex")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing signature signature_hex"))?
        .to_string();
    let scheme = signature_value
        .get("scheme")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let signature_schema = signature_value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    assert_eq!(signed_sha256, receipt_sha256);
    assert_eq!(scheme, "secp256k1_schnorr_no_aux_rand");
    assert_eq!(signature_schema, "openagents.receipt_signature.v1");

    let signature = crate::artifacts::ReceiptSignatureV1 {
        schema: signature_schema,
        scheme,
        signer_pubkey,
        signed_sha256,
        signature_hex,
    };
    assert!(
        crate::artifacts::verify_receipt_signature(&signature)?,
        "receipt signature should verify"
    );

    // Receipt generation should be deterministic for identical run facts.
    let receipt_retry = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/internal/v1/runs/{run_id}/receipt"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(receipt_retry.status(), axum::http::StatusCode::OK);
    let retry_json = response_json(receipt_retry).await?;
    assert_eq!(
        retry_json
            .get("canonical_json_sha256")
            .and_then(serde_json::Value::as_str),
        Some(receipt_sha256)
    );
    assert_eq!(
        retry_json
            .get("signature")
            .and_then(serde_json::Value::as_object)
            .and_then(|obj| obj.get("signature_hex"))
            .and_then(serde_json::Value::as_str),
        Some(signature.signature_hex.as_str())
    );

    let replay_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/internal/v1/runs/{run_id}/replay"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(replay_response.status(), axum::http::StatusCode::OK);
    let replay_text = String::from_utf8(
        replay_response
            .into_body()
            .collect()
            .await?
            .to_bytes()
            .to_vec(),
    )?;
    crate::artifacts::validate_replay_jsonl(&replay_text)
        .map_err(|err| anyhow!("replay output missing required sections: {}", err))?;

    Ok(())
}

#[tokio::test]
async fn projector_summary_endpoint_returns_projected_run_state() -> Result<()> {
    let app = test_router();
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:summary-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.finished",
                    "payload": {"status": "succeeded"},
                    "idempotency_key": "summary-finish",
                    "expected_previous_seq": 1
                }))?))?,
        )
        .await?;

    let summary_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/internal/v1/projectors/run-summary/{run_id}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(summary_response.status(), axum::http::StatusCode::OK);
    let summary_json = response_json(summary_response).await?;
    assert_eq!(
        summary_json
            .pointer("/summary/status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "succeeded"
    );

    let drift_not_found = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/projectors/drift?topic=run:missing:events")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(drift_not_found.status(), axum::http::StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn khala_fanout_endpoints_surface_memory_driver_delivery() -> Result<()> {
    let app = test_router();
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "fanout-ok",
        300,
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:fanout-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let messages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(messages_response.status(), axum::http::StatusCode::OK);
    let messages_json = response_json(messages_response).await?;
    let first_kind = messages_json
        .pointer("/messages/0/kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    assert_eq!(first_kind, "run.started");
    assert_eq!(
        messages_json
            .pointer("/oldest_available_cursor")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        0
    );
    assert_eq!(
        messages_json
            .pointer("/head_cursor")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        1
    );
    assert_eq!(
        messages_json
            .pointer("/next_cursor")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        1
    );
    assert_eq!(
        messages_json
            .pointer("/replay_complete")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        true
    );
    assert_eq!(
        messages_json
            .pointer("/queue_depth")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        1
    );
    assert_eq!(
        messages_json
            .pointer("/limit_applied")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        10
    );
    assert_eq!(
        messages_json
            .pointer("/limit_capped")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        false
    );
    assert_eq!(
        messages_json
            .pointer("/fairness_applied")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        false
    );
    assert_eq!(
        messages_json
            .pointer("/outbound_queue_limit")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        200
    );

    let hooks_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/khala/fanout/hooks")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(hooks_response.status(), axum::http::StatusCode::OK);
    let hooks_json = response_json(hooks_response).await?;
    let hook_count = hooks_json
        .pointer("/hooks")
        .and_then(serde_json::Value::as_array)
        .map_or(0, std::vec::Vec::len);
    assert_eq!(hook_count, 3);
    assert_eq!(
        hooks_json
            .pointer("/delivery_metrics/total_polls")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        1
    );
    let topic_window_count = hooks_json
        .pointer("/topic_windows")
        .and_then(serde_json::Value::as_array)
        .map_or(0, std::vec::Vec::len);
    assert!(topic_window_count >= 1);

    let metrics_response = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/khala/fanout/metrics?topic_limit=5")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(metrics_response.status(), axum::http::StatusCode::OK);
    let metrics_json = response_json(metrics_response).await?;
    assert_eq!(
        metrics_json
            .pointer("/delivery_metrics/total_polls")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        1
    );

    Ok(())
}

#[tokio::test]
async fn khala_topic_messages_rate_limits_fast_pollers() -> Result<()> {
    let app = test_router();
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "poll-rate-limit",
        300,
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:poll-rate-limit-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let first_poll = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(first_poll.status(), axum::http::StatusCode::OK);

    let second_poll = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=1&limit=10"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        second_poll.status(),
        axum::http::StatusCode::TOO_MANY_REQUESTS
    );
    let error_json = response_json(second_poll).await?;
    assert_eq!(
        error_json
            .pointer("/error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "rate_limited"
    );
    assert_eq!(
        error_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "poll_interval_guard"
    );
    assert!(
        error_json
            .pointer("/retry_after_ms")
            .and_then(serde_json::Value::as_u64)
            .is_some()
    );

    Ok(())
}

#[tokio::test]
async fn khala_topic_messages_apply_fair_slice_when_principal_tracks_multiple_topics() -> Result<()>
{
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.khala_poll_min_interval_ms = 1;
            config.khala_poll_default_limit = 50;
            config.khala_poll_max_limit = 50;
            config.khala_outbound_queue_limit = 50;
            config.khala_fair_topic_slice_limit = 2;
        });
    let sync_token = issue_sync_token(
        &["runtime.run_events", "runtime.worker_lifecycle_events"],
        Some(1),
        Some("user:1"),
        "fair-slice",
        300,
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:fair-run-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    for index in 2..=12 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step",
                        "payload": {"step": index},
                        "expected_previous_seq": index - 1
                    }))?))?,
            )
            .await?;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    let worker_id = "desktop:fairness-worker";
    let register_worker = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": worker_id,
                    "owner_user_id": 1,
                    "metadata": {}
                }))?))?,
        )
        .await?;
    assert_eq!(register_worker.status(), axum::http::StatusCode::CREATED);

    let heartbeat_worker = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/workers/{worker_id}/heartbeat"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 1,
                    "metadata_patch": {"pulse": true}
                }))?))?,
        )
        .await?;
    assert_eq!(heartbeat_worker.status(), axum::http::StatusCode::OK);

    let warmup_run_poll = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=50"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(warmup_run_poll.status(), axum::http::StatusCode::OK);
    let warmup_run_json = response_json(warmup_run_poll).await?;
    assert_eq!(
        warmup_run_json
            .pointer("/fairness_applied")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        false
    );

    tokio::time::sleep(Duration::from_millis(5)).await;

    let warmup_worker_poll = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/worker:{worker_id}:lifecycle/messages?after_seq=0&limit=50"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(warmup_worker_poll.status(), axum::http::StatusCode::OK);

    tokio::time::sleep(Duration::from_millis(5)).await;

    let fair_poll = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=1&limit=50"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(fair_poll.status(), axum::http::StatusCode::OK);
    let fair_json = response_json(fair_poll).await?;
    assert_eq!(
        fair_json
            .pointer("/fairness_applied")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        true
    );
    assert_eq!(
        fair_json
            .pointer("/active_topic_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        2
    );
    assert_eq!(
        fair_json
            .pointer("/limit_applied")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        2
    );
    let delivered_count = fair_json
        .pointer("/messages")
        .and_then(serde_json::Value::as_array)
        .map_or(0, std::vec::Vec::len);
    assert_eq!(delivered_count, 2);

    Ok(())
}

#[tokio::test]
async fn khala_topic_messages_evict_slow_consumers_deterministically() -> Result<()> {
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.khala_poll_min_interval_ms = 1;
            config.khala_slow_consumer_lag_threshold = 2;
            config.khala_slow_consumer_max_strikes = 2;
            config.khala_poll_default_limit = 1;
            config.khala_poll_max_limit = 1;
        });
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "slow-consumer-evict",
        300,
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:slow-consumer-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    for index in 2..=7 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step",
                        "payload": {"step": index},
                        "expected_previous_seq": index - 1
                    }))?))?,
            )
            .await?;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    let first_poll = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=1"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(first_poll.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first_poll).await?;
    assert_eq!(
        first_json
            .pointer("/slow_consumer_strikes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        1
    );

    tokio::time::sleep(Duration::from_millis(5)).await;

    let second_poll = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=1"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(second_poll.status(), axum::http::StatusCode::CONFLICT);
    let second_json = response_json(second_poll).await?;
    assert_eq!(
        second_json
            .pointer("/error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "slow_consumer_evicted"
    );
    assert_eq!(
        second_json
            .pointer("/details/strikes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        2
    );
    assert_eq!(
        second_json
            .pointer("/details/recovery")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "advance_cursor_or_rebootstrap"
    );

    Ok(())
}

#[tokio::test]
async fn khala_publish_rate_limit_returns_deterministic_error() -> Result<()> {
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.khala_run_events_publish_rate_per_second = 1;
            config.khala_run_events_max_payload_bytes = 64 * 1024;
        });
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:publish-rate-limit-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let append_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.step",
                    "payload": {"step": 2},
                    "expected_previous_seq": 1
                }))?))?,
        )
        .await?;
    assert_eq!(
        append_response.status(),
        axum::http::StatusCode::TOO_MANY_REQUESTS
    );
    let append_json = response_json(append_response).await?;
    assert_eq!(
        append_json
            .pointer("/error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "rate_limited"
    );
    assert_eq!(
        append_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "khala_publish_rate_limited"
    );
    assert_eq!(
        append_json
            .pointer("/topic_class")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "run_events"
    );

    Ok(())
}

#[tokio::test]
async fn khala_payload_limit_returns_payload_too_large_error() -> Result<()> {
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.khala_run_events_publish_rate_per_second = 50;
            config.khala_run_events_max_payload_bytes = 80;
        });
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:payload-limit-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let append_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "run.step",
                    "payload": {"blob": "this payload is intentionally much larger than eighty bytes to trigger the khala frame-size guard"},
                    "expected_previous_seq": 1
                }))?))?,
        )
        .await?;
    assert_eq!(
        append_response.status(),
        axum::http::StatusCode::PAYLOAD_TOO_LARGE
    );
    let append_json = response_json(append_response).await?;
    assert_eq!(
        append_json
            .pointer("/error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "payload_too_large"
    );
    assert_eq!(
        append_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "khala_frame_payload_too_large"
    );
    assert_eq!(
        append_json
            .pointer("/topic_class")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "run_events"
    );

    Ok(())
}

#[tokio::test]
async fn khala_topic_messages_returns_stale_cursor_when_replay_floor_is_missed() -> Result<()> {
    let app = test_router();
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "stale-ok",
        300,
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:stale-cursor-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    for index in 2..=80 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step",
                        "payload": {"step": index},
                        "expected_previous_seq": index - 1
                    }))?))?,
            )
            .await?;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    let stale_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(stale_response.status(), axum::http::StatusCode::GONE);
    let stale_json = response_json(stale_response).await?;
    assert_eq!(
        stale_json
            .pointer("/error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "stale_cursor"
    );
    assert_eq!(
        stale_json
            .pointer("/details/recovery")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "reset_local_watermark_and_replay_bootstrap"
    );
    assert_eq!(
        stale_json
            .pointer("/details/qos_tier")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "warm"
    );
    assert_eq!(
        stale_json
            .pointer("/details/replay_budget_events")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        20_000
    );
    let reason_codes = stale_json
        .pointer("/details/reason_codes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("missing stale reason_codes"))?;
    let has_retention_reason = reason_codes.iter().any(|value| {
        value
            .as_str()
            .map(|reason| reason == "retention_floor_breach")
            .unwrap_or(false)
    });
    assert!(has_retention_reason, "expected retention_floor_breach");

    Ok(())
}

#[tokio::test]
async fn khala_topic_messages_returns_stale_cursor_when_replay_budget_is_exceeded() -> Result<()> {
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.fanout_queue_capacity = 256;
            config.khala_run_events_replay_budget_events = 3;
        });
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "stale-budget",
        300,
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:stale-budget-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    for index in 2..=12 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/internal/v1/runs/{run_id}/events"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "event_type": "run.step",
                        "payload": {"step": index},
                        "expected_previous_seq": index - 1
                    }))?))?,
            )
            .await?;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    let stale_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=1&limit=10"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(stale_response.status(), axum::http::StatusCode::GONE);
    let stale_json = response_json(stale_response).await?;
    let reason_codes = stale_json
        .pointer("/details/reason_codes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("missing stale reason_codes"))?;
    let has_budget_reason = reason_codes.iter().any(|value| {
        value
            .as_str()
            .map(|reason| reason == "replay_budget_exceeded")
            .unwrap_or(false)
    });
    assert!(has_budget_reason, "expected replay_budget_exceeded");
    assert_eq!(
        stale_json
            .pointer("/details/replay_budget_events")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        3
    );
    assert_eq!(
        stale_json
            .pointer("/details/qos_tier")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "warm"
    );

    Ok(())
}

#[tokio::test]
async fn khala_origin_policy_denies_untrusted_browser_origins() -> Result<()> {
    let app = test_router();
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "origin-deny",
        300,
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:origin-policy-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let denied_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .header("origin", "https://evil.example.com")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(denied_response.status(), axum::http::StatusCode::FORBIDDEN);
    let denied_json = response_json(denied_response).await?;
    assert_eq!(
        denied_json
            .pointer("/error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "forbidden_origin"
    );
    assert_eq!(
        denied_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "origin_not_allowed"
    );

    Ok(())
}

#[tokio::test]
async fn khala_ws_origin_policy_denies_untrusted_browser_origins() -> Result<()> {
    let app = test_router();
    let (addr, shutdown) = spawn_http_server(app).await?;
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "ws-origin-deny",
        300,
    );

    let url =
        format!("ws://{addr}/internal/v1/khala/topics/run:ws_test:events/ws?after_seq=0&limit=10");
    let mut request = url.into_client_request()?;
    request.headers_mut().insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {sync_token}"))?,
    );
    request
        .headers_mut()
        .insert("origin", HeaderValue::from_str("https://evil.example.com")?);

    let err = connect_async(request).await.expect_err("expected denial");
    match err {
        WsError::Http(response) => {
            assert_eq!(response.status(), WsStatusCode::FORBIDDEN);
        }
        other => return Err(anyhow!("unexpected ws error: {other:?}")),
    }

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn khala_ws_requires_valid_sync_token() -> Result<()> {
    let app = test_router();
    let (addr, shutdown) = spawn_http_server(app).await?;

    let url =
        format!("ws://{addr}/internal/v1/khala/topics/run:ws_test:events/ws?after_seq=0&limit=10");
    let request = url.into_client_request()?;
    let err = connect_async(request).await.expect_err("expected denial");
    match err {
        WsError::Http(response) => {
            assert_eq!(response.status(), WsStatusCode::UNAUTHORIZED);
            if let Some(body) = response.body() {
                if let Ok(value) = serde_json::from_slice::<Value>(body) {
                    assert_eq!(
                        value
                            .pointer("/reason_code")
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                        "missing_authorization"
                    );
                }
            }
        }
        other => return Err(anyhow!("unexpected ws error: {other:?}")),
    }

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn khala_ws_upgrade_succeeds_for_authorized_origin() -> Result<()> {
    let app = test_router();
    let (addr, shutdown) = spawn_http_server(app).await?;
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "ws-origin-allow",
        300,
    );

    let url =
        format!("ws://{addr}/internal/v1/khala/topics/run:ws_test:events/ws?after_seq=0&limit=10");
    let mut request = url.into_client_request()?;
    request.headers_mut().insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {sync_token}"))?,
    );
    request
        .headers_mut()
        .insert("origin", HeaderValue::from_str("https://openagents.com")?);

    let (mut stream, response) = connect_async(request).await?;
    assert_eq!(response.status(), WsStatusCode::SWITCHING_PROTOCOLS);

    let next = tokio::time::timeout(Duration::from_secs(1), stream.next()).await?;
    let Some(frame) = next else {
        return Err(anyhow!("expected hello frame"));
    };
    let frame = frame?;
    let text = match frame {
        WsMessage::Text(text) => text,
        other => return Err(anyhow!("unexpected ws frame: {other:?}")),
    };
    let value: Value = serde_json::from_str(&text)?;
    assert_eq!(value.get("type").and_then(Value::as_str), Some("hello"));

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn khala_origin_policy_allows_openagents_origin() -> Result<()> {
    let app = test_router();
    let sync_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "origin-allow",
        300,
    );
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:origin-policy-allow-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let allowed_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {sync_token}"))
                .header("origin", "https://openagents.com")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(allowed_response.status(), axum::http::StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn khala_topic_messages_requires_valid_sync_token() -> Result<()> {
    let app = test_router();
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:auth-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let missing_auth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(missing_auth.status(), axum::http::StatusCode::UNAUTHORIZED);
    let missing_auth_json = response_json(missing_auth).await?;
    assert_eq!(
        missing_auth_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "missing_authorization"
    );

    let expired_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "expired-token",
        -10,
    );
    let expired_auth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {expired_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(expired_auth.status(), axum::http::StatusCode::UNAUTHORIZED);
    let expired_json = response_json(expired_auth).await?;
    assert_eq!(
        expired_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "token_expired"
    );

    let revoked_router = test_router_with_mode_and_revoked(
        AuthorityWriteMode::RustActive,
        HashSet::from([String::from("revoked-jti")]),
    );
    let revoked_create = revoked_router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:revoked-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let revoked_create_json = response_json(revoked_create).await?;
    let revoked_run_id = revoked_create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();
    let revoked_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "revoked-jti",
        300,
    );
    let revoked_response = revoked_router
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{revoked_run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {revoked_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        revoked_response.status(),
        axum::http::StatusCode::UNAUTHORIZED
    );
    let revoked_json = response_json(revoked_response).await?;
    assert_eq!(
        revoked_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "token_revoked"
    );

    Ok(())
}

#[tokio::test]
async fn khala_topic_messages_enforce_scope_matrix() -> Result<()> {
    let app = test_router();
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:scope-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let missing_scope_token = issue_sync_token(
        &["runtime.codex_worker_events"],
        Some(1),
        Some("user:1"),
        "scope-missing",
        300,
    );
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {missing_scope_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(denied.status(), axum::http::StatusCode::FORBIDDEN);
    let denied_json = response_json(denied).await?;
    assert_eq!(
        denied_json
            .pointer("/error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "forbidden_topic"
    );
    assert_eq!(
        denied_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "missing_scope"
    );

    let allowed_token = issue_sync_token(
        &["runtime.run_events"],
        Some(1),
        Some("user:1"),
        "scope-allowed",
        300,
    );
    let allowed = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/internal/v1/khala/topics/run:{run_id}:events/messages?after_seq=0&limit=10"
                ))
                .header("authorization", format!("Bearer {allowed_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(allowed.status(), axum::http::StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn khala_onyx_surface_is_limited_to_run_event_topics() -> Result<()> {
    let app = test_router();

    let onyx_token = issue_sync_token_for_surface(
        &["runtime.run_events", "runtime.codex_worker_events"],
        Some(1),
        Some("user:1"),
        "onyx-surface-policy",
        300,
        "onyx",
    );

    let run_allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/khala/topics/run:019c7f93:events/messages?after_seq=0&limit=10")
                .header("authorization", format!("Bearer {onyx_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(run_allowed.status(), axum::http::StatusCode::OK);

    let worker_denied = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/khala/topics/worker:desktop:owner-worker:lifecycle/messages?after_seq=0&limit=10")
                .header("authorization", format!("Bearer {onyx_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(worker_denied.status(), axum::http::StatusCode::FORBIDDEN);
    let denied_json = response_json(worker_denied).await?;
    assert_eq!(
        denied_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "surface_policy_denied"
    );

    Ok(())
}

#[tokio::test]
async fn khala_worker_topic_enforces_owner_scope() -> Result<()> {
    let app = test_router();

    let create_worker = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:owner-worker",
                    "owner_user_id": 11,
                    "metadata": {}
                }))?))?,
        )
        .await?;
    assert_eq!(create_worker.status(), axum::http::StatusCode::CREATED);

    let denied_owner_token = issue_sync_token(
        &["runtime.codex_worker_events"],
        Some(22),
        Some("user:22"),
        "owner-mismatch",
        300,
    );
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/khala/topics/worker:desktop:owner-worker:lifecycle/messages?after_seq=0&limit=10")
                .header("authorization", format!("Bearer {denied_owner_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(denied.status(), axum::http::StatusCode::FORBIDDEN);
    let denied_json = response_json(denied).await?;
    assert_eq!(
        denied_json
            .pointer("/reason_code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "owner_mismatch"
    );

    let allowed_owner_token = issue_sync_token(
        &["runtime.codex_worker_events"],
        Some(11),
        Some("user:11"),
        "owner-allowed",
        300,
    );
    let allowed = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/khala/topics/worker:desktop:owner-worker:lifecycle/messages?after_seq=0&limit=10")
                .header("authorization", format!("Bearer {allowed_owner_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(allowed.status(), axum::http::StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn write_endpoints_are_frozen_when_authority_mode_is_read_only() -> Result<()> {
    let app = test_router_with_mode(AuthorityWriteMode::ReadOnly);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:blocked-worker",
                    "metadata": {}
                }))?))?,
        )
        .await?;
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );

    let response_json = response_json(response).await?;
    assert_eq!(
        response_json
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        "write_path_frozen"
    );
    Ok(())
}

#[tokio::test]
async fn worker_lifecycle_enforces_owner_and_updates_checkpoint() -> Result<()> {
    let app = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:worker-9",
                    "owner_user_id": 11,
                    "metadata": {"workspace": "openagents"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);

    let heartbeat_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers/desktop:worker-9/heartbeat")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "metadata_patch": {"ping": true}
                }))?))?,
        )
        .await?;
    assert_eq!(heartbeat_response.status(), axum::http::StatusCode::OK);

    let forbidden_get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/workers/desktop:worker-9?owner_user_id=12")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(forbidden_get.status(), axum::http::StatusCode::FORBIDDEN);

    let checkpoint_response = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/workers/desktop:worker-9/checkpoint")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(checkpoint_response.status(), axum::http::StatusCode::OK);
    let checkpoint_json = response_json(checkpoint_response).await?;
    let event_type = checkpoint_json
        .pointer("/checkpoint/last_event_type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing worker checkpoint event type"))?;
    assert_eq!(event_type, "worker.heartbeat");

    Ok(())
}

#[tokio::test]
async fn worker_list_and_provider_catalog_surface_expected_entries() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:catalog-worker-1",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-local-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let create_client_only = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:catalog-worker-2",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client"]
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_client_only.status(), axum::http::StatusCode::CREATED);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/workers?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(list_response.status(), axum::http::StatusCode::OK);
    let list_json = response_json(list_response).await?;
    let workers = list_json
        .get("workers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("missing workers array"))?;
    assert_eq!(workers.len(), 2);

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11&capability=oa.sandbox_run.v1")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
    let catalog_json = response_json(catalog_response).await?;
    let providers = catalog_json
        .get("providers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("missing providers array"))?;
    assert_eq!(providers.len(), 1);

    let provider_id = providers[0]
        .get("provider_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing provider_id"))?;
    assert_eq!(provider_id, "provider-local-1");

    let base_url = providers[0]
        .get("base_url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing base_url"))?;
    assert_eq!(base_url, provider_base_url);

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn provider_catalog_surfaces_local_cluster_provider_metadata() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:cluster-provider-1",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-cluster-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1500,
                        "supply_class": "local_cluster",
                        "cluster_id": "cluster-1",
                        "cluster_members": ["node-a", "node-b"]
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
    let catalog_json = response_json(catalog_response).await?;
    let providers = catalog_json
        .get("providers")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing providers array"))?;
    assert_eq!(providers.len(), 1);
    assert_eq!(
        providers[0]
            .pointer("/supply_class")
            .and_then(Value::as_str),
        Some("local_cluster")
    );
    assert_eq!(
        providers[0].pointer("/cluster_id").and_then(Value::as_str),
        Some("cluster-1")
    );
    let members = providers[0]
        .pointer("/cluster_members")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing cluster_members"))?;
    assert_eq!(members.len(), 2);

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn provider_catalog_infers_supply_class_from_adapter() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_provider_stub().await?;

    let bundle_rack = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:bundle-rack-provider-1",
                    "owner_user_id": 11,
                    "adapter": "bundle_rack_adapter",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-br-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1500
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(bundle_rack.status(), axum::http::StatusCode::CREATED);

    let instance_market = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:instance-market-provider-1",
                    "owner_user_id": 11,
                    "adapter": "instance_market_adapter",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-im-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1700
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(instance_market.status(), axum::http::StatusCode::CREATED);

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
    let catalog_json = response_json(catalog_response).await?;
    let providers = catalog_json
        .get("providers")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing providers array"))?;
    assert_eq!(providers.len(), 2);

    let by_id = |id: &str| -> Option<&Value> {
        providers
            .iter()
            .find(|provider| provider.get("provider_id").and_then(Value::as_str) == Some(id))
    };

    assert_eq!(
        by_id("provider-br-1")
            .and_then(|provider| provider.get("supply_class"))
            .and_then(Value::as_str),
        Some("bundle_rack")
    );
    assert_eq!(
        by_id("provider-im-1")
            .and_then(|provider| provider.get("supply_class"))
            .and_then(Value::as_str),
        Some("instance_market")
    );

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn provider_pricing_stage_controls_reject_bidding_until_enabled() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_provider_stub().await?;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:pricing-stage-provider-1",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-pricing-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000,
                        "pricing_stage": "bidding",
                        "pricing_bands": [{
                            "capability": "oa.sandbox_run.v1",
                            "min_price_msats": 1000,
                            "max_price_msats": 2000,
                            "step_msats": 100
                        }]
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let json = response_json(response).await?;
    assert_eq!(
        json.get("error").and_then(Value::as_str),
        Some("invalid_request")
    );
    let message = json.get("message").and_then(Value::as_str).unwrap_or("");
    assert!(message.contains("pricing_stage"));

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn provider_pricing_stage_banded_requires_valid_bands_and_surfaces_in_catalog() -> Result<()>
{
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_provider_stub().await?;

    let missing_bands = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:pricing-stage-provider-2",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-pricing-2",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000,
                        "pricing_stage": "banded"
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(missing_bands.status(), axum::http::StatusCode::BAD_REQUEST);

    let ok = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:pricing-stage-provider-3",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-pricing-3",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000,
                        "pricing_stage": "banded",
                        "pricing_bands": [{
                            "capability": "oa.sandbox_run.v1",
                            "min_price_msats": 1000,
                            "max_price_msats": 2000,
                            "step_msats": 100
                        }]
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(ok.status(), axum::http::StatusCode::CREATED);

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
    let catalog_json = response_json(catalog_response).await?;
    let providers = catalog_json
        .get("providers")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing providers array"))?;
    let provider = providers
        .iter()
        .find(|provider| {
            provider.get("provider_id").and_then(Value::as_str) == Some("provider-pricing-3")
        })
        .ok_or_else(|| anyhow!("missing provider-pricing-3 entry"))?;
    assert_eq!(
        provider.get("pricing_stage").and_then(Value::as_str),
        Some("banded")
    );
    let bands = provider
        .get("pricing_bands")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing pricing_bands"))?;
    assert_eq!(bands.len(), 1);
    assert_eq!(
        bands[0].get("capability").and_then(Value::as_str),
        Some("oa.sandbox_run.v1")
    );

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn provider_pricing_updates_are_validated_on_heartbeat() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:pricing-heartbeat-provider-1",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-pricing-heartbeat-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let heartbeat = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers/desktop:pricing-heartbeat-provider-1/heartbeat")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "metadata_patch": {
                        "pricing_stage": "bidding"
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(heartbeat.status(), axum::http::StatusCode::BAD_REQUEST);

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn dispatch_sandbox_run_calls_provider_and_enforces_phase0_hardening() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_compute_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:dispatch-provider-1",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-dispatch-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000,
                        "caps": { "max_timeout_secs": 120 }
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };

    let ok_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/dispatch/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "request": request
                }))?))?,
        )
        .await?;
    assert_eq!(ok_response.status(), axum::http::StatusCode::OK);
    let ok_json = response_json(ok_response).await?;
    assert_eq!(
        ok_json
            .pointer("/selection/provider/provider_id")
            .and_then(Value::as_str),
        Some("provider-dispatch-1")
    );
    assert_eq!(
        ok_json.pointer("/response/status").and_then(Value::as_str),
        Some("success")
    );

    let bad_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/dispatch/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "request": {
                        "sandbox": {
                            "image_digest": "sha256:test",
                            "network_policy": "full"
                        },
                        "commands": [{"cmd":"echo hi"}]
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(bad_response.status(), axum::http::StatusCode::BAD_REQUEST);

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn quote_sandbox_run_returns_all_in_quote_and_routes_by_total_cost() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_compute_provider_stub().await?;

    let create_single_node = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:quote-provider-single",
                    "owner_user_id": 11,
                    "adapter": "test",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-quote-single",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_single_node.status(), axum::http::StatusCode::CREATED);

    let create_instance_market = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:quote-provider-instance",
                    "owner_user_id": 11,
                    "adapter": "instance_market_adapter",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-quote-instance",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 900
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(
        create_instance_market.status(),
        axum::http::StatusCode::CREATED
    );

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };

    let quote_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/compute/quote/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "request": request
                }))?))?,
        )
        .await?;
    assert_eq!(quote_response.status(), axum::http::StatusCode::OK);
    let quote_json = response_json(quote_response).await?;

    assert_eq!(
        quote_json.pointer("/schema").and_then(Value::as_str),
        Some("openagents.marketplace.compute_quote.v1")
    );
    assert_eq!(
        quote_json
            .pointer("/selection/provider/provider_id")
            .and_then(Value::as_str),
        Some("provider-quote-single")
    );

    assert_eq!(
        quote_json
            .pointer("/quote/provider_price_msats")
            .and_then(Value::as_u64),
        Some(1000)
    );
    assert_eq!(
        quote_json
            .pointer("/quote/operator_fee_msats")
            .and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        quote_json
            .pointer("/quote/policy_adder_msats")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        quote_json
            .pointer("/quote/total_price_msats")
            .and_then(Value::as_u64),
        Some(1005)
    );
    assert!(
        quote_json
            .pointer("/quote/quote_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| value.len() == 64)
    );
    assert!(
        quote_json
            .pointer("/quote/quote_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("quote_"))
    );

    let issued_at = quote_json
        .pointer("/quote/issued_at_unix")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let valid_until = quote_json
        .pointer("/quote/valid_until_unix")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cancel_until = quote_json
        .pointer("/quote/cancel_until_unix")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cancel_fee = quote_json
        .pointer("/quote/cancel_fee_msats")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let refund_until = quote_json
        .pointer("/quote/refund_until_unix")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    assert!(issued_at > 0);
    assert!(valid_until >= issued_at);
    assert_eq!(cancel_until, valid_until);
    assert_eq!(cancel_fee, 0);
    assert!(refund_until >= issued_at);
    assert_eq!(
        quote_json
            .pointer("/quote/currency")
            .and_then(Value::as_str),
        Some("msats")
    );

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn settle_sandbox_run_releases_when_quote_matches() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_compute_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-provider-ok",
                    "owner_user_id": 11,
                    "adapter": "test",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-settle-ok",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let run_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(run_response.status(), axum::http::StatusCode::CREATED);
    let run_json = response_json(run_response).await?;
    let run_id = run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };

    let quote_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/compute/quote/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "request": request
                }))?))?,
        )
        .await?;
    assert_eq!(quote_response.status(), axum::http::StatusCode::OK);
    let quote_json = response_json(quote_response).await?;
    let quote = quote_json
        .get("quote")
        .cloned()
        .ok_or_else(|| anyhow!("missing quote"))?;
    let provider_price = quote
        .pointer("/provider_price_msats")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("missing provider_price_msats"))?;

    let response = protocol::SandboxRunResponse {
        env_info: protocol::jobs::sandbox::EnvInfo {
            image_digest: "sha256:test".to_string(),
            hostname: None,
            system_info: None,
        },
        runs: vec![protocol::jobs::sandbox::CommandResult {
            cmd: "echo hi".to_string(),
            exit_code: 0,
            duration_ms: 10,
            stdout_sha256: "stdout".to_string(),
            stderr_sha256: "stderr".to_string(),
            stdout_preview: None,
            stderr_preview: None,
        }],
        artifacts: Vec::new(),
        status: protocol::jobs::sandbox::SandboxStatus::Success,
        error: None,
        provenance: protocol::Provenance::new("stub-success"),
    };

    let settle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "run_id": run_id,
                    "provider_id": "provider-settle-ok",
                    "provider_worker_id": "desktop:settle-provider-ok",
                    "amount_msats": provider_price,
                    "quote": quote,
                    "request": request,
                    "response": response
                }))?))?,
        )
        .await?;
    assert_eq!(settle_response.status(), axum::http::StatusCode::OK);
    let settle_json = response_json(settle_response).await?;
    assert_eq!(
        settle_json
            .pointer("/settlement_status")
            .and_then(Value::as_str),
        Some("released")
    );

    let catalog_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
    let catalog_json = response_json(catalog_response).await?;
    let providers = catalog_json
        .pointer("/providers")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing providers list"))?;
    let provider_entry = providers
        .iter()
        .find(|entry| {
            entry
                .get("worker_id")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "desktop:settle-provider-ok")
        })
        .ok_or_else(|| anyhow!("missing provider entry"))?;
    assert_eq!(
        provider_entry
            .get("price_integrity_samples")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        provider_entry
            .get("price_integrity_violations")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        provider_entry.get("success_count").and_then(Value::as_u64),
        Some(1)
    );

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn settle_sandbox_run_withholds_and_labels_on_price_integrity_violation() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_compute_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-provider-bad",
                    "owner_user_id": 11,
                    "adapter": "test",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-settle-bad",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let run_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(run_response.status(), axum::http::StatusCode::CREATED);
    let run_json = response_json(run_response).await?;
    let run_id = run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };

    let quote_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/compute/quote/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "request": request
                }))?))?,
        )
        .await?;
    assert_eq!(quote_response.status(), axum::http::StatusCode::OK);
    let quote_json = response_json(quote_response).await?;
    let quote = quote_json
        .get("quote")
        .cloned()
        .ok_or_else(|| anyhow!("missing quote"))?;
    let provider_price = quote
        .pointer("/provider_price_msats")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("missing provider_price_msats"))?;

    let response = protocol::SandboxRunResponse {
        env_info: protocol::jobs::sandbox::EnvInfo {
            image_digest: "sha256:test".to_string(),
            hostname: None,
            system_info: None,
        },
        runs: vec![protocol::jobs::sandbox::CommandResult {
            cmd: "echo hi".to_string(),
            exit_code: 0,
            duration_ms: 10,
            stdout_sha256: "stdout".to_string(),
            stderr_sha256: "stderr".to_string(),
            stdout_preview: None,
            stderr_preview: None,
        }],
        artifacts: Vec::new(),
        status: protocol::jobs::sandbox::SandboxStatus::Success,
        error: None,
        provenance: protocol::Provenance::new("stub-success"),
    };

    let settle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "run_id": run_id,
                    "provider_id": "provider-settle-bad",
                    "provider_worker_id": "desktop:settle-provider-bad",
                    "amount_msats": provider_price + 500,
                    "quote": quote,
                    "request": request,
                    "response": response
                }))?))?,
        )
        .await?;
    assert_eq!(settle_response.status(), axum::http::StatusCode::OK);
    let settle_json = response_json(settle_response).await?;
    assert_eq!(
        settle_json
            .pointer("/settlement_status")
            .and_then(Value::as_str),
        Some("withheld")
    );
    assert_eq!(
        settle_json.pointer("/amount_msats").and_then(Value::as_u64),
        Some(provider_price)
    );

    let run_get = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/internal/v1/runs/{run_id}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(run_get.status(), axum::http::StatusCode::OK);
    let run_state = response_json(run_get).await?;
    let events = run_state
        .pointer("/run/events")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing run events"))?;
    assert!(events.iter().any(|event| {
        event
            .get("event_type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "receipt")
            && event
                .pointer("/payload/receipt_type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "PriceIntegrityFailed")
    }));
    let payment_event = events.iter().find(|event| {
        event
            .get("event_type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "payment")
    });
    let Some(payment_event) = payment_event else {
        return Err(anyhow!("missing payment event"));
    };
    assert_eq!(
        payment_event
            .pointer("/payload/amount_msats")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        payment_event
            .pointer("/payload/payment_proof/reason")
            .and_then(Value::as_str),
        Some("price_integrity_failed")
    );

    let catalog_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
    let catalog_json = response_json(catalog_response).await?;
    let providers = catalog_json
        .pointer("/providers")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing providers list"))?;
    let provider_entry = providers
        .iter()
        .find(|entry| {
            entry
                .get("worker_id")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "desktop:settle-provider-bad")
        })
        .ok_or_else(|| anyhow!("missing provider entry"))?;
    assert_eq!(
        provider_entry
            .get("price_integrity_samples")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        provider_entry
            .get("price_integrity_violations")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        provider_entry
            .get("last_price_variance_bps")
            .and_then(Value::as_u64),
        Some(5_000)
    );
    assert_eq!(
        provider_entry.get("success_count").and_then(Value::as_u64),
        Some(0)
    );

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn settle_sandbox_run_rejects_expired_quote_binding_window() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_compute_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-provider-expired",
                    "owner_user_id": 11,
                    "adapter": "test",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-settle-expired",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let run_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(run_response.status(), axum::http::StatusCode::CREATED);
    let run_json = response_json(run_response).await?;
    let run_id = run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };
    let job_hash = protocol::hash::canonical_hash(&request)?;

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
    let catalog_json = response_json(catalog_response).await?;
    let providers = catalog_json
        .pointer("/providers")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing providers list"))?;
    let provider_value = providers
        .iter()
        .find(|entry| {
            entry
                .get("worker_id")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "desktop:settle-provider-expired")
        })
        .cloned()
        .ok_or_else(|| anyhow!("missing provider entry"))?;
    let provider: crate::marketplace::ProviderCatalogEntry =
        serde_json::from_value(provider_value)?;

    let now_unix = Utc::now().timestamp().max(0) as u64;
    let issued_at_unix = now_unix.saturating_sub(120);
    let expired_quote = crate::marketplace::compute_all_in_quote_v1(
        &provider,
        "oa.sandbox_run.v1",
        job_hash.as_str(),
        issued_at_unix,
    )
    .ok_or_else(|| anyhow!("quote computation failed"))?;
    assert!(expired_quote.valid_until_unix < now_unix);

    let response = protocol::SandboxRunResponse {
        env_info: protocol::jobs::sandbox::EnvInfo {
            image_digest: "sha256:test".to_string(),
            hostname: None,
            system_info: None,
        },
        runs: vec![protocol::jobs::sandbox::CommandResult {
            cmd: "echo hi".to_string(),
            exit_code: 0,
            duration_ms: 10,
            stdout_sha256: "stdout".to_string(),
            stderr_sha256: "stderr".to_string(),
            stdout_preview: None,
            stderr_preview: None,
        }],
        artifacts: Vec::new(),
        status: protocol::jobs::sandbox::SandboxStatus::Success,
        error: None,
        provenance: protocol::Provenance::new("stub-success"),
    };

    let settle_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "run_id": run_id,
                    "provider_id": "provider-settle-expired",
                    "provider_worker_id": "desktop:settle-provider-expired",
                    "amount_msats": expired_quote.provider_price_msats,
                    "quote": expired_quote,
                    "request": request,
                    "response": response
                }))?))?,
        )
        .await?;
    assert_eq!(
        settle_response.status(),
        axum::http::StatusCode::BAD_REQUEST
    );
    let body = response_json(settle_response).await?;
    assert!(
        body.pointer("/message")
            .and_then(Value::as_str)
            .is_some_and(|msg| msg.contains("expired"))
    );

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn router_select_compute_rejects_unsupported_currency() -> Result<()> {
    let app = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "owner_user_id": 11,
        "run_id": run_id,
        "capability": "oa.sandbox_run.v1",
        "marketplace_id": "openagents",
        "policy": "lowest_total_cost_v1",
        "idempotency_key": "router:test",
        "candidates": [
            {"marketplace_id":"market-a","provider_id":"provider-a","total_price_msats":1200,"currency":"usd"}
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/router/compute/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
    let json = response_json(resp).await?;
    assert!(
        json.pointer("/message")
            .and_then(Value::as_str)
            .is_some_and(|msg| msg.contains("unsupported currency"))
    );
    Ok(())
}

#[tokio::test]
async fn router_select_compute_reliability_first_policy_prefers_high_reliability() -> Result<()> {
    let app = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "owner_user_id": 11,
        "run_id": run_id,
        "capability": "oa.sandbox_run.v1",
        "marketplace_id": "openagents",
        "policy": "reliability_first_v1",
        "idempotency_key": "router:reliability-first",
        "candidates": [
            {"marketplace_id":"market-a","provider_id":"provider-cheap","total_price_msats":1000,"latency_ms":50,"reliability_bps":6000},
            {"marketplace_id":"market-b","provider_id":"provider-reliable","total_price_msats":1200,"latency_ms":200,"reliability_bps":9900}
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/router/compute/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let json = response_json(resp).await?;
    assert_eq!(
        json.pointer("/selected/provider_id")
            .and_then(Value::as_str),
        Some("provider-reliable")
    );
    Ok(())
}

#[tokio::test]
async fn router_select_compute_balanced_policy_penalizes_latency_and_low_reliability() -> Result<()>
{
    let app = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "owner_user_id": 11,
        "run_id": run_id,
        "capability": "oa.sandbox_run.v1",
        "marketplace_id": "openagents",
        "policy": "balanced_v1",
        "idempotency_key": "router:balanced",
        "candidates": [
            {"marketplace_id":"market-a","provider_id":"provider-slow","total_price_msats":1000,"latency_ms":800,"reliability_bps":9000},
            {"marketplace_id":"market-b","provider_id":"provider-fast","total_price_msats":1300,"latency_ms":50,"reliability_bps":9900}
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/router/compute/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let json = response_json(resp).await?;
    assert_eq!(
        json.pointer("/selected/provider_id")
            .and_then(Value::as_str),
        Some("provider-fast")
    );
    Ok(())
}

#[tokio::test]
async fn router_select_compute_verifier_strict_rejects_missing_signer_key() -> Result<()> {
    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.verifier_strict = true;
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "owner_user_id": 11,
        "run_id": run_id,
        "capability": "oa.sandbox_run.v1",
        "marketplace_id": "openagents",
        "policy": "lowest_total_cost_v1",
        "idempotency_key": "router:strict-missing-signer",
        "candidates": [
            {"marketplace_id":"market-a","provider_id":"provider-a","total_price_msats":1200,"latency_ms":50,"reliability_bps":9000},
            {"marketplace_id":"market-b","provider_id":"provider-b","total_price_msats":1100,"latency_ms":200,"reliability_bps":8000}
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/router/compute/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(resp.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    Ok(())
}

#[tokio::test]
async fn router_select_compute_verifier_rejects_signer_not_in_key_graph() -> Result<()> {
    let secret = nostr::generate_secret_key();
    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.bridge_nostr_secret_key = Some(secret);
        c.verifier_strict = true;
        c.verifier_allowed_signer_pubkeys = HashSet::from(["0".repeat(64)]);
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "owner_user_id": 11,
        "run_id": run_id,
        "capability": "oa.sandbox_run.v1",
        "marketplace_id": "openagents",
        "policy": "lowest_total_cost_v1",
        "idempotency_key": "router:key-graph-deny",
        "candidates": [
            {"marketplace_id":"market-a","provider_id":"provider-a","total_price_msats":1200,"latency_ms":50,"reliability_bps":9000},
            {"marketplace_id":"market-b","provider_id":"provider-b","total_price_msats":1100,"latency_ms":200,"reliability_bps":8000}
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/router/compute/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(resp.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    Ok(())
}

#[tokio::test]
async fn router_select_compute_verifier_accepts_active_signer_key_graph() -> Result<()> {
    let secret = nostr::generate_secret_key();
    let pubkey = nostr::get_public_key_hex(&secret).map_err(|error| anyhow!(error.to_string()))?;
    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.bridge_nostr_secret_key = Some(secret);
        c.verifier_strict = true;
        c.verifier_allowed_signer_pubkeys = HashSet::from([pubkey]);
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "owner_user_id": 11,
        "run_id": run_id,
        "capability": "oa.sandbox_run.v1",
        "marketplace_id": "openagents",
        "policy": "lowest_total_cost_v1",
        "idempotency_key": "router:key-graph-allow",
        "candidates": [
            {"marketplace_id":"market-a","provider_id":"provider-a","total_price_msats":1200,"latency_ms":50,"reliability_bps":9000},
            {"marketplace_id":"market-b","provider_id":"provider-b","total_price_msats":1100,"latency_ms":200,"reliability_bps":8000}
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/router/compute/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn router_select_compute_normalizes_and_emits_signed_decision_when_configured() -> Result<()>
{
    let secret = nostr::generate_secret_key();
    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.bridge_nostr_secret_key = Some(secret);
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "owner_user_id": 11,
        "run_id": run_id,
        "capability": "oa.sandbox_run.v1",
        "objective_hash": "sha256:jobhash",
        "marketplace_id": "openagents",
        "policy": "lowest_total_cost_v1",
        "idempotency_key": "router:test",
        "decided_at_unix": 1_700_000_010,
        "candidates": [
            {"marketplace_id":"market-a","provider_id":"provider-a","total_price_msats":1200,"latency_ms":50,"reliability_bps":9000,"quote_id":"quote-a"},
            {"marketplace_id":"market-b","provider_id":"provider-b","total_price_msats":1100,"latency_ms":200,"reliability_bps":8000,"quote_id":"quote-b"}
        ]
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/router/compute/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first).await?;
    assert_eq!(
        first_json.pointer("/schema").and_then(Value::as_str),
        Some("openagents.marketplace.router_decision.v1")
    );
    assert_eq!(
        first_json
            .pointer("/selected/provider_id")
            .and_then(Value::as_str),
        Some("provider-b")
    );
    assert!(
        first_json
            .pointer("/decision_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| value.len() == 64)
    );

    let event_value = first_json
        .get("nostr_event")
        .cloned()
        .ok_or_else(|| anyhow!("missing nostr_event"))?;
    let event: nostr::Event = serde_json::from_value(event_value)?;
    let kind = crate::bridge::validate_bridge_event_v1(&event)
        .map_err(|error| anyhow!("bridge validation failed: {error}"))?;
    assert!(matches!(
        kind,
        crate::bridge::BridgeEventKind::CommerceMessage
    ));

    // Same request + idempotency key should be safe to retry.
    let retry = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/router/compute/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(retry.status(), axum::http::StatusCode::OK);
    let retry_json = response_json(retry).await?;
    assert_eq!(
        retry_json
            .pointer("/decision_sha256")
            .and_then(Value::as_str),
        first_json
            .pointer("/decision_sha256")
            .and_then(Value::as_str)
    );

    Ok(())
}

#[tokio::test]
async fn hydra_routing_score_emits_receipt_and_is_idempotent() -> Result<()> {
    let app = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:hydra-router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "schema": "openagents.hydra.routing_score_request.v1",
        "idempotency_key": "hydra-routing:test",
        "run_id": run_id,
        "marketplace_id": "openagents",
        "capability": "oa.sandbox_run.v1",
        "policy": "lowest_total_cost_v1",
        "objective_hash": "sha256:objective",
        "decided_at_unix": 1_700_000_111,
        "candidates": [
            {
                "marketplace_id": "market-a",
                "provider_id": "provider-a",
                "total_price_msats": 1200,
                "latency_ms": 50,
                "reliability_bps": 9100,
                "constraints": {"region":"us-central1"},
                "quote_id": "quote-a",
                "quote_sha256": "sha256:quote-a"
            },
            {
                "marketplace_id": "market-b",
                "provider_id": "provider-b",
                "total_price_msats": 900,
                "latency_ms": 120,
                "reliability_bps": 8700,
                "constraints": {"region":"us-east1"},
                "quote_id": "quote-b",
                "quote_sha256": "sha256:quote-b"
            }
        ]
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/routing/score")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first).await?;
    assert_eq!(
        first_json.pointer("/schema").and_then(Value::as_str),
        Some("openagents.hydra.routing_score_response.v1")
    );
    assert_eq!(
        first_json
            .pointer("/selected/provider_id")
            .and_then(Value::as_str),
        Some("provider-b")
    );
    assert!(
        first_json
            .pointer("/receipt/canonical_json_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| value.len() == 64)
    );

    let retry = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/routing/score")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(retry.status(), axum::http::StatusCode::OK);
    let retry_json = response_json(retry).await?;
    assert_eq!(
        retry_json
            .pointer("/decision_sha256")
            .and_then(Value::as_str),
        first_json
            .pointer("/decision_sha256")
            .and_then(Value::as_str)
    );
    assert_eq!(
        retry_json
            .pointer("/receipt/canonical_json_sha256")
            .and_then(Value::as_str),
        first_json
            .pointer("/receipt/canonical_json_sha256")
            .and_then(Value::as_str)
    );

    Ok(())
}

#[tokio::test]
async fn hydra_routing_score_rejects_non_object_constraints() -> Result<()> {
    let app = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:hydra-router-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "schema": "openagents.hydra.routing_score_request.v1",
        "idempotency_key": "hydra-routing:invalid",
        "run_id": run_id,
        "marketplace_id": "openagents",
        "capability": "oa.sandbox_run.v1",
        "policy": "balanced_v1",
        "decided_at_unix": 1_700_000_222,
        "candidates": [
            {
                "marketplace_id": "market-a",
                "provider_id": "provider-a",
                "total_price_msats": 1200,
                "latency_ms": 50,
                "reliability_bps": 9000,
                "constraints": "invalid"
            }
        ]
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/routing/score")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
    let json = response_json(resp).await?;
    assert!(
        json.pointer("/message")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("candidate.constraints"))
    );

    Ok(())
}

#[tokio::test]
async fn hydra_fx_rfq_endpoints_enforce_idempotency_and_readback() -> Result<()> {
    let app = test_router();

    let body = serde_json::json!({
        "schema": "openagents.hydra.fx_rfq_request.v1",
        "idempotency_key": "fx-rfq-idem-1",
        "requester_id": "autopilot:user-1",
        "budget_scope_id": "budget:scope-1",
        "sell": {
            "asset": "usd",
            "amount": 100000,
            "unit": "cents"
        },
        "buy_asset": "btc_ln",
        "min_buy_amount": 2500000,
        "max_spread_bps": 100,
        "max_fee_bps": 50,
        "max_latency_ms": 5000,
        "quote_ttl_seconds": 30,
        "policy_context": {"policy":"balanced_v1"}
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first).await?;
    assert_eq!(
        first_json.pointer("/schema").and_then(Value::as_str),
        Some("openagents.hydra.fx_rfq_response.v1")
    );
    let rfq_id = first_json
        .pointer("/rfq/rfq_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing rfq_id"))?
        .to_string();

    let get_rfq = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/internal/v1/hydra/fx/rfq/{rfq_id}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(get_rfq.status(), axum::http::StatusCode::OK);
    let get_json = response_json(get_rfq).await?;
    assert_eq!(
        get_json.pointer("/rfq/rfq_id").and_then(Value::as_str),
        Some(rfq_id.as_str())
    );
    assert_eq!(
        get_json.pointer("/rfq/sell/asset").and_then(Value::as_str),
        Some("USD")
    );

    let replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(replay.status(), axum::http::StatusCode::OK);
    let replay_json = response_json(replay).await?;
    assert_eq!(
        replay_json.pointer("/rfq/rfq_id").and_then(Value::as_str),
        Some(rfq_id.as_str())
    );

    let mut drifted = body.clone();
    drifted["max_fee_bps"] = json!(90);
    let conflict = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&drifted)?))?,
        )
        .await?;
    assert_eq!(conflict.status(), axum::http::StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn hydra_fx_rfq_rejects_disallowed_asset_pairs() -> Result<()> {
    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.hydra_fx_policy.allowed_pairs = HashSet::from(["EUR->BTC_LN".to_string()]);
    });

    let body = serde_json::json!({
        "schema": "openagents.hydra.fx_rfq_request.v1",
        "idempotency_key": "fx-rfq-policy-denied",
        "requester_id": "autopilot:user-1",
        "budget_scope_id": "budget:scope-1",
        "sell": {
            "asset": "USD",
            "amount": 100000,
            "unit": "cents"
        },
        "buy_asset": "BTC_LN",
        "min_buy_amount": 2500000,
        "max_spread_bps": 100,
        "max_fee_bps": 50,
        "max_latency_ms": 5000,
        "quote_ttl_seconds": 30,
        "policy_context": {"policy":"balanced_v1"}
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn hydra_fx_quote_select_is_deterministic_and_tie_break_stable() -> Result<()> {
    let app = test_router();
    let rfq = serde_json::json!({
        "schema": "openagents.hydra.fx_rfq_request.v1",
        "idempotency_key": "fx-rfq-select-idem",
        "requester_id": "autopilot:user-1",
        "budget_scope_id": "budget:scope-1",
        "sell": {
            "asset": "USD",
            "amount": 100000,
            "unit": "cents"
        },
        "buy_asset": "BTC_LN",
        "min_buy_amount": 2000000,
        "max_spread_bps": 100,
        "max_fee_bps": 60,
        "max_latency_ms": 5000,
        "quote_ttl_seconds": 30,
        "policy_context": {"policy":"reputation_first_v0"}
    });
    let rfq_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&rfq)?))?,
        )
        .await?;
    assert_eq!(rfq_response.status(), axum::http::StatusCode::OK);
    let rfq_json = response_json(rfq_response).await?;
    let rfq_id = rfq_json
        .pointer("/rfq/rfq_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing rfq_id"))?
        .to_string();
    let valid_until_unix = Utc::now().timestamp().max(0) as u64 + 30;

    for (idempotency_key, quote_id, provider_id) in [
        ("fx-quote-a-idem", "quote-a", "provider-a"),
        ("fx-quote-b-idem", "quote-b", "provider-b"),
    ] {
        let quote_upsert = serde_json::json!({
            "schema": "openagents.hydra.fx_quote_upsert_request.v1",
            "idempotency_key": idempotency_key,
            "quote": {
                "quote_id": quote_id,
                "rfq_id": rfq_id,
                "provider_id": provider_id,
                "sell": {
                    "asset": "USD",
                    "amount": 100000,
                    "unit": "cents"
                },
                "buy": {
                    "asset": "BTC_LN",
                    "amount": 2500000,
                    "unit": "msats"
                },
                "spread_bps": 70,
                "fee_bps": 20,
                "latency_ms": 1000,
                "reliability_bps": 9000,
                "valid_until_unix": valid_until_unix,
                "status": "active",
                "constraints": {},
                "quote_sha256": ""
            }
        });

        let upsert_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/hydra/fx/quote")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&quote_upsert)?))?,
            )
            .await?;
        assert_eq!(upsert_response.status(), axum::http::StatusCode::OK);
    }

    let select_request = serde_json::json!({
        "schema": "openagents.hydra.fx_select_request.v1",
        "idempotency_key": "fx-select-idem-1",
        "rfq_id": rfq_id,
        "policy": "reputation_first_v0"
    });
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&select_request)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first).await?;
    assert_eq!(
        first_json.pointer("/schema").and_then(Value::as_str),
        Some("openagents.hydra.fx_select_response.v1")
    );
    assert_eq!(
        first_json
            .pointer("/selected/provider_id")
            .and_then(Value::as_str),
        Some("provider-a")
    );
    let decision_sha = first_json
        .pointer("/decision_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing decision_sha256"))?
        .to_string();
    assert!(
        first_json
            .pointer("/selected/constraints/selection/weighted_score")
            .and_then(Value::as_u64)
            .is_some()
    );

    let replay = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&select_request)?))?,
        )
        .await?;
    assert_eq!(replay.status(), axum::http::StatusCode::OK);
    let replay_json = response_json(replay).await?;
    assert_eq!(
        replay_json
            .pointer("/decision_sha256")
            .and_then(Value::as_str),
        Some(decision_sha.as_str())
    );
    Ok(())
}

#[tokio::test]
async fn hydra_fx_select_returns_conflict_when_no_quotes_exist() -> Result<()> {
    let app = test_router();
    let rfq = serde_json::json!({
        "schema": "openagents.hydra.fx_rfq_request.v1",
        "idempotency_key": "fx-rfq-no-quote-idem",
        "requester_id": "autopilot:user-1",
        "budget_scope_id": "budget:scope-1",
        "sell": {
            "asset": "USD",
            "amount": 100000,
            "unit": "cents"
        },
        "buy_asset": "BTC_LN",
        "min_buy_amount": 2000000,
        "max_spread_bps": 100,
        "max_fee_bps": 60,
        "max_latency_ms": 5000,
        "quote_ttl_seconds": 30,
        "policy_context": {"policy":"reputation_first_v0"}
    });
    let rfq_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&rfq)?))?,
        )
        .await?;
    assert_eq!(rfq_response.status(), axum::http::StatusCode::OK);
    let rfq_json = response_json(rfq_response).await?;
    let rfq_id = rfq_json
        .pointer("/rfq/rfq_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing rfq_id"))?;

    let select_request = serde_json::json!({
        "schema": "openagents.hydra.fx_select_request.v1",
        "idempotency_key": "fx-select-empty-idem",
        "rfq_id": rfq_id,
        "policy": "reputation_first_v0"
    });
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&select_request)?))?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn hydra_fx_settle_releases_and_replays_without_double_spend() -> Result<()> {
    let app = test_router();
    let rfq = serde_json::json!({
        "schema": "openagents.hydra.fx_rfq_request.v1",
        "idempotency_key": "fx-rfq-settle-release-idem",
        "requester_id": "autopilot:user-1",
        "budget_scope_id": "budget:scope-1",
        "sell": {"asset": "USD", "amount": 100000, "unit": "cents"},
        "buy_asset": "BTC_LN",
        "min_buy_amount": 2000000,
        "max_spread_bps": 120,
        "max_fee_bps": 70,
        "max_latency_ms": 5000,
        "quote_ttl_seconds": 45,
        "policy_context": {"policy":"reputation_first_v0"}
    });
    let rfq_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&rfq)?))?,
        )
        .await?;
    assert_eq!(rfq_resp.status(), axum::http::StatusCode::OK);
    let rfq_json = response_json(rfq_resp).await?;
    let rfq_id = rfq_json
        .pointer("/rfq/rfq_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing rfq_id"))?
        .to_string();
    let valid_until_unix = Utc::now().timestamp().max(0) as u64 + 60;

    let quote = serde_json::json!({
        "schema": "openagents.hydra.fx_quote_upsert_request.v1",
        "idempotency_key": "fx-quote-release-idem",
        "quote": {
            "quote_id": "quote-release",
            "rfq_id": rfq_id,
            "provider_id": "provider-release",
            "sell": {"asset": "USD", "amount": 100000, "unit": "cents"},
            "buy": {"asset": "BTC_LN", "amount": 2500000, "unit": "msats"},
            "spread_bps": 80,
            "fee_bps": 25,
            "latency_ms": 900,
            "reliability_bps": 9200,
            "valid_until_unix": valid_until_unix,
            "status": "active",
            "constraints": {},
            "quote_sha256": ""
        }
    });
    let quote_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/quote")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&quote)?))?,
        )
        .await?;
    assert_eq!(quote_resp.status(), axum::http::StatusCode::OK);

    let select = serde_json::json!({
        "schema": "openagents.hydra.fx_select_request.v1",
        "idempotency_key": "fx-select-release-idem",
        "rfq_id": rfq_id,
        "policy": "reputation_first_v0"
    });
    let select_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&select)?))?,
        )
        .await?;
    assert_eq!(select_resp.status(), axum::http::StatusCode::OK);
    let select_json = response_json(select_resp).await?;
    let quote_id = select_json
        .pointer("/selected/quote_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing selected quote_id"))?;
    let reservation_id = fx_expected_reservation_id(rfq_id.as_str(), quote_id)?;

    let settle = serde_json::json!({
        "schema": "openagents.hydra.fx_settle_request.v1",
        "idempotency_key": "fx-settle-release-idem",
        "rfq_id": rfq_id,
        "quote_id": quote_id,
        "reservation_id": reservation_id,
        "policy_context": {"release_allowed": true}
    });
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/settle")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle)?))?,
        )
        .await?;
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_json = response_json(first).await?;
    assert_eq!(
        first_json.pointer("/status").and_then(Value::as_str),
        Some("released")
    );
    let settlement_id = first_json
        .pointer("/settlement_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing settlement_id"))?
        .to_string();
    assert_eq!(
        first_json
            .pointer("/receipt/schema")
            .and_then(Value::as_str),
        Some("openagents.hydra.fx_settlement_receipt.v1")
    );
    assert!(
        first_json
            .pointer("/receipt/canonical_json_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| value.len() == 64)
    );

    let replay = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/settle")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle)?))?,
        )
        .await?;
    assert_eq!(replay.status(), axum::http::StatusCode::OK);
    let replay_json = response_json(replay).await?;
    assert_eq!(
        replay_json
            .pointer("/settlement_id")
            .and_then(Value::as_str),
        Some(settlement_id.as_str())
    );
    Ok(())
}

#[tokio::test]
async fn hydra_fx_settle_returns_withheld_when_policy_disallows_release() -> Result<()> {
    let app = test_router();
    let rfq = serde_json::json!({
        "schema": "openagents.hydra.fx_rfq_request.v1",
        "idempotency_key": "fx-rfq-settle-withheld-idem",
        "requester_id": "autopilot:user-2",
        "budget_scope_id": "budget:scope-2",
        "sell": {"asset": "USD", "amount": 100000, "unit": "cents"},
        "buy_asset": "BTC_LN",
        "min_buy_amount": 2000000,
        "max_spread_bps": 120,
        "max_fee_bps": 70,
        "max_latency_ms": 5000,
        "quote_ttl_seconds": 45,
        "policy_context": {"policy":"reputation_first_v0"}
    });
    let rfq_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&rfq)?))?,
        )
        .await?;
    assert_eq!(rfq_resp.status(), axum::http::StatusCode::OK);
    let rfq_json = response_json(rfq_resp).await?;
    let rfq_id = rfq_json
        .pointer("/rfq/rfq_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing rfq_id"))?
        .to_string();
    let valid_until_unix = Utc::now().timestamp().max(0) as u64 + 60;

    let quote = serde_json::json!({
        "schema": "openagents.hydra.fx_quote_upsert_request.v1",
        "idempotency_key": "fx-quote-withheld-idem",
        "quote": {
            "quote_id": "quote-withheld",
            "rfq_id": rfq_id,
            "provider_id": "provider-withheld",
            "sell": {"asset": "USD", "amount": 100000, "unit": "cents"},
            "buy": {"asset": "BTC_LN", "amount": 2500000, "unit": "msats"},
            "spread_bps": 75,
            "fee_bps": 20,
            "latency_ms": 950,
            "reliability_bps": 9100,
            "valid_until_unix": valid_until_unix,
            "status": "active",
            "constraints": {},
            "quote_sha256": ""
        }
    });
    let quote_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/quote")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&quote)?))?,
        )
        .await?;
    assert_eq!(quote_resp.status(), axum::http::StatusCode::OK);

    let select = serde_json::json!({
        "schema": "openagents.hydra.fx_select_request.v1",
        "idempotency_key": "fx-select-withheld-idem",
        "rfq_id": rfq_id,
        "policy": "reputation_first_v0"
    });
    let select_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&select)?))?,
        )
        .await?;
    assert_eq!(select_resp.status(), axum::http::StatusCode::OK);
    let select_json = response_json(select_resp).await?;
    let quote_id = select_json
        .pointer("/selected/quote_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing selected quote_id"))?;
    let reservation_id = fx_expected_reservation_id(rfq_id.as_str(), quote_id)?;
    let settle = serde_json::json!({
        "schema": "openagents.hydra.fx_settle_request.v1",
        "idempotency_key": "fx-settle-withheld-idem",
        "rfq_id": rfq_id,
        "quote_id": quote_id,
        "reservation_id": reservation_id,
        "policy_context": {"release_allowed": false}
    });
    let settle_resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/settle")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle)?))?,
        )
        .await?;
    assert_eq!(settle_resp.status(), axum::http::StatusCode::OK);
    let settle_json = response_json(settle_resp).await?;
    assert_eq!(
        settle_json.pointer("/status").and_then(Value::as_str),
        Some("withheld")
    );
    Ok(())
}

#[tokio::test]
async fn hydra_fx_settle_rejects_reservation_conflict() -> Result<()> {
    let app = test_router();
    let rfq = serde_json::json!({
        "schema": "openagents.hydra.fx_rfq_request.v1",
        "idempotency_key": "fx-rfq-reservation-conflict-idem",
        "requester_id": "autopilot:user-3",
        "budget_scope_id": "budget:scope-3",
        "sell": {"asset": "USD", "amount": 100000, "unit": "cents"},
        "buy_asset": "BTC_LN",
        "min_buy_amount": 2000000,
        "max_spread_bps": 120,
        "max_fee_bps": 70,
        "max_latency_ms": 5000,
        "quote_ttl_seconds": 45,
        "policy_context": {"policy":"reputation_first_v0"}
    });
    let rfq_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&rfq)?))?,
        )
        .await?;
    assert_eq!(rfq_resp.status(), axum::http::StatusCode::OK);
    let rfq_json = response_json(rfq_resp).await?;
    let rfq_id = rfq_json
        .pointer("/rfq/rfq_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing rfq_id"))?
        .to_string();
    let valid_until_unix = Utc::now().timestamp().max(0) as u64 + 60;
    let quote = serde_json::json!({
        "schema": "openagents.hydra.fx_quote_upsert_request.v1",
        "idempotency_key": "fx-quote-reservation-conflict-idem",
        "quote": {
            "quote_id": "quote-reservation-conflict",
            "rfq_id": rfq_id,
            "provider_id": "provider-reservation-conflict",
            "sell": {"asset": "USD", "amount": 100000, "unit": "cents"},
            "buy": {"asset": "BTC_LN", "amount": 2500000, "unit": "msats"},
            "spread_bps": 75,
            "fee_bps": 20,
            "latency_ms": 950,
            "reliability_bps": 9100,
            "valid_until_unix": valid_until_unix,
            "status": "active",
            "constraints": {},
            "quote_sha256": ""
        }
    });
    let quote_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/quote")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&quote)?))?,
        )
        .await?;
    assert_eq!(quote_resp.status(), axum::http::StatusCode::OK);
    let select = serde_json::json!({
        "schema": "openagents.hydra.fx_select_request.v1",
        "idempotency_key": "fx-select-reservation-conflict-idem",
        "rfq_id": rfq_id,
        "policy": "reputation_first_v0"
    });
    let select_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&select)?))?,
        )
        .await?;
    assert_eq!(select_resp.status(), axum::http::StatusCode::OK);
    let select_json = response_json(select_resp).await?;
    let quote_id = select_json
        .pointer("/selected/quote_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing selected quote_id"))?;

    let settle = serde_json::json!({
        "schema": "openagents.hydra.fx_settle_request.v1",
        "idempotency_key": "fx-settle-reservation-conflict-idem",
        "rfq_id": rfq_id,
        "quote_id": quote_id,
        "reservation_id": "rsv_wrong",
        "policy_context": {}
    });
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/settle")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle)?))?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    Ok(())
}

#[tokio::test]
async fn hydra_fx_settle_withholds_when_quote_expired() -> Result<()> {
    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.hydra_fx_policy.min_quote_validity_seconds = 0;
    });
    let rfq = serde_json::json!({
        "schema": "openagents.hydra.fx_rfq_request.v1",
        "idempotency_key": "fx-rfq-expired-idem",
        "requester_id": "autopilot:user-4",
        "budget_scope_id": "budget:scope-4",
        "sell": {"asset": "USD", "amount": 100000, "unit": "cents"},
        "buy_asset": "BTC_LN",
        "min_buy_amount": 2000000,
        "max_spread_bps": 120,
        "max_fee_bps": 70,
        "max_latency_ms": 5000,
        "quote_ttl_seconds": 45,
        "policy_context": {"policy":"reputation_first_v0"}
    });
    let rfq_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/rfq")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&rfq)?))?,
        )
        .await?;
    assert_eq!(rfq_resp.status(), axum::http::StatusCode::OK);
    let rfq_json = response_json(rfq_resp).await?;
    let rfq_id = rfq_json
        .pointer("/rfq/rfq_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing rfq_id"))?
        .to_string();

    let valid_until_unix = Utc::now().timestamp().max(0) as u64 + 1;
    let quote = serde_json::json!({
        "schema": "openagents.hydra.fx_quote_upsert_request.v1",
        "idempotency_key": "fx-quote-expired-idem",
        "quote": {
            "quote_id": "quote-expired",
            "rfq_id": rfq_id,
            "provider_id": "provider-expired",
            "sell": {"asset": "USD", "amount": 100000, "unit": "cents"},
            "buy": {"asset": "BTC_LN", "amount": 2500000, "unit": "msats"},
            "spread_bps": 75,
            "fee_bps": 20,
            "latency_ms": 950,
            "reliability_bps": 9100,
            "valid_until_unix": valid_until_unix,
            "status": "active",
            "constraints": {},
            "quote_sha256": ""
        }
    });
    let quote_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/quote")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&quote)?))?,
        )
        .await?;
    assert_eq!(quote_resp.status(), axum::http::StatusCode::OK);
    let select = serde_json::json!({
        "schema": "openagents.hydra.fx_select_request.v1",
        "idempotency_key": "fx-select-expired-idem",
        "rfq_id": rfq_id,
        "policy": "reputation_first_v0"
    });
    let select_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/select")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&select)?))?,
        )
        .await?;
    assert_eq!(select_resp.status(), axum::http::StatusCode::OK);
    let select_json = response_json(select_resp).await?;
    let quote_id = select_json
        .pointer("/selected/quote_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing selected quote_id"))?;
    let reservation_id = fx_expected_reservation_id(rfq_id.as_str(), quote_id)?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    let settle = serde_json::json!({
        "schema": "openagents.hydra.fx_settle_request.v1",
        "idempotency_key": "fx-settle-expired-idem",
        "rfq_id": rfq_id,
        "quote_id": quote_id,
        "reservation_id": reservation_id,
        "policy_context": {}
    });
    let settle_resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/fx/settle")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle)?))?,
        )
        .await?;
    assert_eq!(settle_resp.status(), axum::http::StatusCode::OK);
    let settle_json = response_json(settle_resp).await?;
    assert_eq!(
        settle_json.pointer("/status").and_then(Value::as_str),
        Some("withheld")
    );
    Ok(())
}

#[tokio::test]
async fn hydra_risk_health_endpoint_returns_breaker_snapshot() -> Result<()> {
    let app = test_router();
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/internal/v1/hydra/risk/health")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let json = response_json(response).await?;
    assert_eq!(
        json.pointer("/schema").and_then(Value::as_str),
        Some("openagents.hydra.risk_health_response.v1")
    );
    assert!(json.pointer("/credit_breakers").is_some());
    assert!(json.pointer("/routing/degraded").is_some());
    Ok(())
}

#[tokio::test]
async fn hydra_routing_score_filters_cep_candidate_when_breaker_halts_envelopes() -> Result<()> {
    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.credit_policy.circuit_breaker_min_sample = 0;
        c.credit_policy.loss_rate_halt_threshold = -1.0;
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:hydra-router-breaker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);
    let create_json = response_json(create_response).await?;
    let run_id = create_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let body = serde_json::json!({
        "schema": "openagents.hydra.routing_score_request.v1",
        "idempotency_key": "hydra-routing:breaker-filter",
        "run_id": run_id,
        "marketplace_id": "openagents",
        "capability": "oa.sandbox_run.v1",
        "policy": "lowest_total_cost_v1",
        "decided_at_unix": 1_700_000_333,
        "candidates": [
            {
                "marketplace_id": "openagents",
                "provider_id": "provider-direct-safe",
                "total_price_msats": 3000,
                "latency_ms": 100,
                "reliability_bps": 9000,
                "constraints": {"routeKind":"direct_liquidity"}
            },
            {
                "marketplace_id": "openagents",
                "provider_id": "route-cep",
                "total_price_msats": 1000,
                "latency_ms": 80,
                "reliability_bps": 9900,
                "constraints": {"routeKind":"cep_envelope"}
            }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/routing/score")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let json = response_json(response).await?;
    assert_eq!(
        json.pointer("/selected/provider_id")
            .and_then(Value::as_str),
        Some("provider-direct-safe")
    );
    let notes = json
        .pointer("/factors/policy_notes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        notes
            .iter()
            .any(|value| value.as_str() == Some("cep_candidate_filtered_by_breaker"))
    );
    Ok(())
}

#[tokio::test]
async fn hydra_observability_endpoint_reports_mvp2_metrics() -> Result<()> {
    let pool_id = "d".repeat(64);

    let app = build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |c| {
        c.liquidity_pool_withdraw_throttle.lp_mode_enabled = true;
        c.liquidity_pool_withdraw_throttle
            .stress_liability_ratio_bps = 1;
        c.liquidity_pool_withdraw_throttle.halt_liability_ratio_bps = 1;
        c.liquidity_pool_snapshot_pool_ids = vec![pool_id.clone()];
    });

    let create_pool = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/pools/{pool_id}/admin/create"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.liquidity.pool.create_request.v1",
                    "operator_id": "operator:hydra-observability-test",
                    "pool_kind": "llp",
                    "status": "active",
                    "config": {}
                }))?))?,
        )
        .await?;
    assert_eq!(create_pool.status(), axum::http::StatusCode::OK);

    let withdraw_request = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/pools/{pool_id}/withdraw_request"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.liquidity.pool.withdraw_request.v1",
                    "lp_id": "lp:hydra-observability",
                    "idempotency_key": "hydra-observability:withdraw-1",
                    "shares_burned": 10,
                    "rail_preference": "onchain",
                    "payout_address": "bc1qhydraobservability0000000000000000000000000",
                }))?))?,
        )
        .await?;
    assert_eq!(withdraw_request.status(), axum::http::StatusCode::OK);

    let create_first_run = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "worker_id": "desktop:hydra-observability-a",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_first_run.status(), axum::http::StatusCode::CREATED);
    let first_run_json = response_json(create_first_run).await?;
    let first_run_id = first_run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing first run id"))?
        .to_string();

    let first_score = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/routing/score")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.hydra.routing_score_request.v1",
                    "idempotency_key": "hydra-observability:first",
                    "run_id": first_run_id,
                    "marketplace_id": "openagents",
                    "capability": "oa.sandbox_run.v1",
                    "policy": "lowest_total_cost_v1",
                    "decided_at_unix": 1_700_000_400,
                    "candidates": [
                        {
                            "marketplace_id": "openagents",
                            "provider_id": "route-direct",
                            "total_price_msats": 500,
                            "latency_ms": 20,
                            "reliability_bps": 9900,
                            "constraints": {"routeKind":"direct_liquidity"}
                        },
                        {
                            "marketplace_id": "openagents",
                            "provider_id": "route-cep",
                            "total_price_msats": 900,
                            "latency_ms": 40,
                            "reliability_bps": 9800,
                            "constraints": {"routeKind":"cep_envelope"}
                        }
                    ]
                }))?))?,
        )
        .await?;
    assert_eq!(first_score.status(), axum::http::StatusCode::OK);
    let first_score_json = response_json(first_score).await?;
    assert_eq!(
        first_score_json
            .pointer("/selected/provider_id")
            .and_then(Value::as_str),
        Some("route-cep"),
        "direct route should be filtered when throttle mode is halted"
    );

    let create_second_run = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "worker_id": "desktop:hydra-observability-b",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_second_run.status(), axum::http::StatusCode::CREATED);
    let second_run_json = response_json(create_second_run).await?;
    let second_run_id = second_run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing second run id"))?
        .to_string();

    let second_score = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/hydra/routing/score")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "schema": "openagents.hydra.routing_score_request.v1",
                    "idempotency_key": "hydra-observability:second",
                    "run_id": second_run_id,
                    "marketplace_id": "openagents",
                    "capability": "oa.sandbox_run.v1",
                    "policy": "lowest_total_cost_v1",
                    "decided_at_unix": 1_700_000_401,
                    "candidates": [
                        {
                            "marketplace_id": "openagents",
                            "provider_id": "provider-low-confidence",
                            "total_price_msats": 1000,
                            "latency_ms": 5000,
                            "reliability_bps": 0,
                            "constraints": {}
                        }
                    ]
                }))?))?,
        )
        .await?;
    assert_eq!(second_score.status(), axum::http::StatusCode::OK);

    let observability = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/internal/v1/hydra/observability")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(observability.status(), axum::http::StatusCode::OK);
    let json = response_json(observability).await?;
    assert_eq!(
        json.pointer("/schema").and_then(Value::as_str),
        Some("openagents.hydra.observability_response.v1")
    );
    assert_eq!(
        json.pointer("/routing/decision_total")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        json.pointer("/routing/selected_route_cep")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/routing/selected_route_other")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        json.pointer("/routing/confidence_lt_040")
            .and_then(Value::as_u64)
            .is_some_and(|value| value >= 1)
    );
    assert_eq!(
        json.pointer("/withdrawal_throttle/mode")
            .and_then(Value::as_str),
        Some("halted")
    );
    assert!(
        json.pointer("/withdrawal_throttle/affected_requests_total")
            .and_then(Value::as_u64)
            .is_some_and(|value| value >= 2)
    );
    Ok(())
}

#[tokio::test]
async fn dispatch_failure_applies_strike_and_reroutes_owned_provider() -> Result<()> {
    let app = test_router();
    let (failing_base_url, failing_shutdown) = spawn_cancelled_compute_provider_stub().await?;
    let (ok_base_url, ok_shutdown) = spawn_compute_provider_stub().await?;
    let (reserve_base_url, reserve_shutdown) = spawn_compute_provider_stub().await?;

    let create_failing = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:dispatch-fail-provider",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-fail",
                        "provider_base_url": failing_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 900,
                        "caps": { "max_timeout_secs": 120 }
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_failing.status(), axum::http::StatusCode::CREATED);

    let create_ok_owned = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:dispatch-ok-provider",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-ok-owned",
                        "provider_base_url": ok_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1200,
                        "caps": { "max_timeout_secs": 120 }
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_ok_owned.status(), axum::http::StatusCode::CREATED);

    let create_reserve = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:dispatch-reserve-provider",
                    "owner_user_id": 999,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-reserve",
                        "provider_base_url": reserve_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1500,
                        "reserve_pool": true,
                        "caps": { "max_timeout_secs": 120 }
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_reserve.status(), axum::http::StatusCode::CREATED);

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };

    let dispatch = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/dispatch/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "request": request.clone(),
                }))?))?,
        )
        .await?;
    assert_eq!(dispatch.status(), axum::http::StatusCode::OK);
    let dispatch_json = response_json(dispatch).await?;
    assert_eq!(
        dispatch_json
            .pointer("/selection/provider/provider_id")
            .and_then(Value::as_str),
        Some("provider-ok-owned")
    );
    assert_eq!(
        dispatch_json
            .pointer("/fallback_from_provider_id")
            .and_then(Value::as_str),
        Some("provider-fail")
    );

    let failing_worker = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/workers/desktop:dispatch-fail-provider?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(failing_worker.status(), axum::http::StatusCode::OK);
    let failing_worker_json = response_json(failing_worker).await?;
    assert_eq!(
        failing_worker_json
            .pointer("/worker/worker/metadata/failure_strikes")
            .and_then(Value::as_u64),
        Some(1)
    );

    let route = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/route/provider")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "capability": "oa.sandbox_run.v1",
                }))?))?,
        )
        .await?;
    assert_eq!(route.status(), axum::http::StatusCode::OK);
    let route_json = response_json(route).await?;
    assert_eq!(
        route_json
            .pointer("/provider/provider_id")
            .and_then(Value::as_str),
        Some("provider-ok-owned")
    );

    let _ = failing_shutdown.send(());
    let _ = ok_shutdown.send(());
    let _ = reserve_shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn treasury_settlement_is_pay_after_verify_and_idempotent() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_compute_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-provider-1",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-settle-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000,
                        "caps": { "max_timeout_secs": 120 }
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let create_run = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settlement-worker",
                    "metadata": {"policy_bundle_id": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_run.status(), axum::http::StatusCode::CREATED);
    let run_json = response_json(create_run).await?;
    let run_id = run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };

    let dispatch = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/dispatch/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "request": request.clone(),
                }))?))?,
        )
        .await?;
    assert_eq!(dispatch.status(), axum::http::StatusCode::OK);
    let dispatch_json = response_json(dispatch).await?;
    let response_value = dispatch_json
        .get("response")
        .cloned()
        .ok_or_else(|| anyhow!("missing dispatch response"))?;

    let settle = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "run_id": run_id.clone(),
                    "provider_id": "provider-settle-1",
                    "provider_worker_id": "desktop:settle-provider-1",
                    "amount_msats": 1000,
                    "request": request.clone(),
                    "response": response_value.clone(),
                }))?))?,
        )
        .await?;
    assert_eq!(settle.status(), axum::http::StatusCode::OK);
    let settle_json = response_json(settle).await?;
    assert_eq!(
        settle_json
            .pointer("/settlement_status")
            .and_then(Value::as_str),
        Some("released")
    );
    assert_eq!(
        settle_json
            .pointer("/verification_passed")
            .and_then(Value::as_bool),
        Some(true)
    );

    let worker_after_settle = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/workers/desktop:settle-provider-1?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(worker_after_settle.status(), axum::http::StatusCode::OK);
    let worker_after_settle_json = response_json(worker_after_settle).await?;
    assert_eq!(
        worker_after_settle_json
            .pointer("/worker/worker/metadata/success_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    let receipt = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/internal/v1/runs/{run_id}/receipt"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(receipt.status(), axum::http::StatusCode::OK);
    let receipt_json = response_json(receipt).await?;
    assert_eq!(
        receipt_json
            .pointer("/metrics/payments_msats_total")
            .and_then(Value::as_u64),
        Some(1000)
    );

    // Idempotent settle should not add additional payment events.
    let settle_retry = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "run_id": run_id.clone(),
                    "provider_id": "provider-settle-1",
                    "provider_worker_id": "desktop:settle-provider-1",
                    "amount_msats": 1000,
                    "request": request.clone(),
                    "response": response_value.clone(),
                }))?))?,
        )
        .await?;
    assert_eq!(settle_retry.status(), axum::http::StatusCode::OK);
    let receipt_retry = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/internal/v1/runs/{run_id}/receipt"))
                .body(Body::empty())?,
        )
        .await?;
    let receipt_retry_json = response_json(receipt_retry).await?;
    assert_eq!(
        receipt_retry_json
            .pointer("/metrics/payments_msats_total")
            .and_then(Value::as_u64),
        Some(1000)
    );

    let worker_after_retry = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/workers/desktop:settle-provider-1?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(worker_after_retry.status(), axum::http::StatusCode::OK);
    let worker_after_retry_json = response_json(worker_after_retry).await?;
    assert_eq!(
        worker_after_retry_json
            .pointer("/worker/worker/metadata/success_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    // Append-run-events endpoint must reject direct payment writes.
    let reject_payment = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/internal/v1/runs/{run_id}/events"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "event_type": "payment",
                    "payload": {"rail":"lightning","asset_id":"BTC_LN","amount_msats":1,"payment_proof":{}}
                }))?))?,
        )
        .await?;
    assert_eq!(reject_payment.status(), axum::http::StatusCode::BAD_REQUEST);

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn settle_sandbox_run_routes_through_cep_and_is_idempotent() -> Result<()> {
    let wallet_token = "wallet-token-cep-settle";
    let wallet = spawn_wallet_executor_stub(wallet_token).await?;
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.liquidity_wallet_executor_base_url = Some(wallet.base_url.clone());
            config.liquidity_wallet_executor_auth_token = Some(wallet_token.to_string());
        });
    let (provider_base_url, provider_shutdown) = spawn_compute_provider_stub().await?;
    let provider_id = "c".repeat(64);
    let agent_id = "a".repeat(64);
    let pool_id = "b".repeat(64);

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:cep-provider-ok",
                    "owner_user_id": 11,
                    "adapter": "test",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": provider_id,
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let run_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(run_response.status(), axum::http::StatusCode::CREATED);
    let run_json = response_json(run_response).await?;
    let run_id = run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };
    let response = protocol::SandboxRunResponse {
        env_info: protocol::jobs::sandbox::EnvInfo {
            image_digest: "sha256:test".to_string(),
            hostname: None,
            system_info: None,
        },
        runs: vec![protocol::jobs::sandbox::CommandResult {
            cmd: "echo hi".to_string(),
            exit_code: 0,
            duration_ms: 10,
            stdout_sha256: "stdout".to_string(),
            stderr_sha256: "stderr".to_string(),
            stdout_preview: None,
            stderr_preview: None,
        }],
        artifacts: Vec::new(),
        status: protocol::jobs::sandbox::SandboxStatus::Success,
        error: None,
        provenance: protocol::Provenance::new("stub-success"),
    };

    let settle_body = serde_json::json!({
        "owner_user_id": 11,
        "run_id": run_id.clone(),
        "provider_id": provider_id,
        "provider_worker_id": "desktop:cep-provider-ok",
        "amount_msats": 42000,
        "route_policy": {"kind": "force_cep"},
        "cep": {
            "agent_id": agent_id,
            "pool_id": pool_id,
            "provider_invoice": "lnbc420n1test",
            "provider_host": "provider.test",
            "max_fee_msats": 1000
        },
        "request": request,
        "response": response
    });

    let settle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle_body)?))?,
        )
        .await?;
    assert_eq!(settle_response.status(), axum::http::StatusCode::OK);
    let settle_json = response_json(settle_response).await?;
    assert_eq!(
        settle_json
            .pointer("/settlement_status")
            .and_then(Value::as_str),
        Some("released")
    );
    assert_eq!(
        settle_json
            .pointer("/settlement_route")
            .and_then(Value::as_str),
        Some("cep_envelope")
    );
    assert!(
        settle_json
            .pointer("/credit_offer_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("cepo_"))
    );
    assert!(
        settle_json
            .pointer("/credit_envelope_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("cepe_"))
    );
    assert!(
        settle_json
            .pointer("/credit_settlement_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("ceps_"))
    );
    assert!(
        settle_json
            .pointer("/credit_liquidity_receipt_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| value.len() == 64)
    );
    assert!(
        settle_json
            .pointer("/verification_receipt_sha256")
            .and_then(Value::as_str)
            .is_some_and(|value| value.len() == 64)
    );

    let run_get = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/internal/v1/runs/{run_id}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(run_get.status(), axum::http::StatusCode::OK);
    let run_json = response_json(run_get).await?;
    let events = run_json
        .pointer("/run/events")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing run events"))?;
    assert!(events.iter().any(|event| {
        event
            .pointer("/payload/receipt_type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "CepSettlementLinked")
    }));

    let retry = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&settle_body)?))?,
        )
        .await?;
    assert_eq!(retry.status(), axum::http::StatusCode::OK);
    assert_eq!(
        wallet.calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "cep settlement replay must not trigger a second pay-bolt11 call",
    );

    let _ = provider_shutdown.send(());
    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn settle_sandbox_run_cep_with_verification_failure_withholds_without_payment() -> Result<()>
{
    let wallet_token = "wallet-token-cep-withhold";
    let wallet = spawn_wallet_executor_stub(wallet_token).await?;
    let app =
        build_test_router_with_config(AuthorityWriteMode::RustActive, HashSet::new(), |config| {
            config.liquidity_wallet_executor_base_url = Some(wallet.base_url.clone());
            config.liquidity_wallet_executor_auth_token = Some(wallet_token.to_string());
        });
    let (provider_base_url, provider_shutdown) = spawn_compute_provider_stub().await?;
    let provider_id = "d".repeat(64);
    let agent_id = "e".repeat(64);
    let pool_id = "f".repeat(64);

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:cep-provider-fail",
                    "owner_user_id": 11,
                    "adapter": "test",
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": provider_id,
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let run_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:settle-worker",
                    "metadata": {"source": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(run_response.status(), axum::http::StatusCode::CREATED);
    let run_json = response_json(run_response).await?;
    let run_id = run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
            resources: protocol::jobs::sandbox::ResourceLimits {
                timeout_secs: 30,
                ..Default::default()
            },
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new(
            "echo expected",
        )],
        ..Default::default()
    };
    let response = protocol::SandboxRunResponse {
        env_info: protocol::jobs::sandbox::EnvInfo {
            image_digest: "sha256:test".to_string(),
            hostname: None,
            system_info: None,
        },
        runs: vec![protocol::jobs::sandbox::CommandResult {
            cmd: "echo actual".to_string(),
            exit_code: 0,
            duration_ms: 10,
            stdout_sha256: "stdout".to_string(),
            stderr_sha256: "stderr".to_string(),
            stdout_preview: None,
            stderr_preview: None,
        }],
        artifacts: Vec::new(),
        status: protocol::jobs::sandbox::SandboxStatus::Success,
        error: None,
        provenance: protocol::Provenance::new("stub-fail"),
    };

    let settle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "run_id": run_id,
                    "provider_id": provider_id,
                    "provider_worker_id": "desktop:cep-provider-fail",
                    "amount_msats": 42000,
                    "route_policy": {"kind": "force_cep"},
                    "cep": {
                        "agent_id": agent_id,
                        "pool_id": pool_id,
                        "provider_invoice": "lnbc420n1test",
                        "provider_host": "provider.test",
                        "max_fee_msats": 1000
                    },
                    "request": request,
                    "response": response
                }))?))?,
        )
        .await?;
    assert_eq!(settle_response.status(), axum::http::StatusCode::OK);
    let settle_json = response_json(settle_response).await?;
    assert_eq!(
        settle_json
            .pointer("/settlement_status")
            .and_then(Value::as_str),
        Some("withheld")
    );
    assert_eq!(
        settle_json
            .pointer("/settlement_route")
            .and_then(Value::as_str),
        Some("cep_envelope")
    );
    assert_eq!(
        settle_json
            .pointer("/credit_liquidity_receipt_sha256")
            .and_then(Value::as_str),
        None
    );
    assert_eq!(
        wallet.calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "verification failure must not release CEP payment",
    );

    let _ = provider_shutdown.send(());
    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn provider_is_auto_quarantined_after_repeated_verification_violations() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_provider_stub().await?;

    let create_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:quarantine-provider-1",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-quarantine-1",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_provider.status(), axum::http::StatusCode::CREATED);

    let create_run = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:quarantine-runner",
                    "metadata": {"policy_bundle_id": "test"}
                }))?))?,
        )
        .await?;
    assert_eq!(create_run.status(), axum::http::StatusCode::CREATED);
    let run_json = response_json(create_run).await?;
    let run_id = run_json
        .pointer("/run/id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing run id"))?
        .to_string();

    for idx in 0..3 {
        let request = protocol::SandboxRunRequest {
            sandbox: protocol::jobs::sandbox::SandboxConfig {
                image_digest: "sha256:test".to_string(),
                network_policy: protocol::jobs::sandbox::NetworkPolicy::None,
                ..Default::default()
            },
            commands: vec![protocol::jobs::sandbox::SandboxCommand::new(format!(
                "echo hi {idx}"
            ))],
            ..Default::default()
        };
        let bad_response = protocol::SandboxRunResponse {
            env_info: protocol::jobs::sandbox::EnvInfo {
                image_digest: "sha256:test".to_string(),
                hostname: None,
                system_info: None,
            },
            runs: vec![protocol::jobs::sandbox::CommandResult {
                cmd: "echo bye".to_string(),
                exit_code: 0,
                duration_ms: 1,
                stdout_sha256: "stdout".to_string(),
                stderr_sha256: "stderr".to_string(),
                stdout_preview: None,
                stderr_preview: None,
            }],
            artifacts: Vec::new(),
            status: protocol::jobs::sandbox::SandboxStatus::Success,
            error: None,
            provenance: protocol::Provenance::new("test"),
        };

        let settle_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/internal/v1/treasury/compute/settle/sandbox-run")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&serde_json::json!({
                        "owner_user_id": 11,
                        "run_id": run_id.clone(),
                        "provider_id": "provider-quarantine-1",
                        "provider_worker_id": "desktop:quarantine-provider-1",
                        "amount_msats": 1000,
                        "request": request,
                        "response": bad_response,
                    }))?))?,
            )
            .await?;
        assert_eq!(settle_response.status(), axum::http::StatusCode::OK);
    }

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/marketplace/catalog/providers?owner_user_id=11")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(catalog_response.status(), axum::http::StatusCode::OK);
    let catalog_json = response_json(catalog_response).await?;
    let providers = catalog_json
        .get("providers")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing providers array"))?;
    assert_eq!(providers.len(), 1);
    assert_eq!(
        providers[0].get("quarantined").and_then(Value::as_bool),
        Some(true)
    );

    let incidents_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/internal/v1/fraud/incidents?limit=10")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(incidents_response.status(), axum::http::StatusCode::OK);
    let incidents_json = response_json(incidents_response).await?;
    let incidents = incidents_json
        .get("incidents")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing incidents array"))?;
    assert!(
        incidents.iter().any(|incident| {
            incident
                .get("incident_type")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "compute_verification_violation")
        }),
        "expected compute_verification_violation incident"
    );
    assert!(
        incidents.iter().any(|incident| {
            incident
                .get("incident_type")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "provider_quarantined")
        }),
        "expected provider_quarantined incident"
    );

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn route_provider_prefers_owned_and_falls_back_to_reserve_pool() -> Result<()> {
    let app = test_router();
    let (provider_base_url, shutdown) = spawn_provider_stub().await?;

    let create_owned = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:routing-owned",
                    "owner_user_id": 11,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-owned",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 1000
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_owned.status(), axum::http::StatusCode::CREATED);

    let create_reserve = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:routing-reserve",
                    "owner_user_id": 99,
                    "metadata": {
                        "roles": ["client", "provider"],
                        "provider_id": "provider-reserve",
                        "provider_base_url": provider_base_url.clone(),
                        "capabilities": ["oa.sandbox_run.v1"],
                        "min_price_msats": 2000,
                        "reserve_pool": true
                    }
                }))?))?,
        )
        .await?;
    assert_eq!(create_reserve.status(), axum::http::StatusCode::CREATED);

    let route_owned = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/route/provider")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "capability": "oa.sandbox_run.v1"
                }))?))?,
        )
        .await?;
    assert_eq!(route_owned.status(), axum::http::StatusCode::OK);
    let route_owned_json = response_json(route_owned).await?;
    assert_eq!(
        route_owned_json
            .pointer("/provider/provider_id")
            .and_then(Value::as_str),
        Some("provider-owned")
    );
    assert_eq!(
        route_owned_json
            .pointer("/provider/supply_class")
            .and_then(Value::as_str),
        Some("single_node")
    );
    assert_eq!(
        route_owned_json.get("tier").and_then(Value::as_str),
        Some("owned")
    );

    let stop_owned = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers/desktop:routing-owned/status")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "status": "failed",
                    "reason": "offline"
                }))?))?,
        )
        .await?;
    assert_eq!(stop_owned.status(), axum::http::StatusCode::OK);

    let route_reserve = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/marketplace/route/provider")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "owner_user_id": 11,
                    "capability": "oa.sandbox_run.v1"
                }))?))?,
        )
        .await?;
    assert_eq!(route_reserve.status(), axum::http::StatusCode::OK);
    let route_reserve_json = response_json(route_reserve).await?;
    assert_eq!(
        route_reserve_json
            .pointer("/provider/provider_id")
            .and_then(Value::as_str),
        Some("provider-reserve")
    );
    assert_eq!(
        route_reserve_json
            .pointer("/provider/supply_class")
            .and_then(Value::as_str),
        Some("reserve_pool")
    );
    assert_eq!(
        route_reserve_json.get("tier").and_then(Value::as_str),
        Some("reserve_pool")
    );

    let _ = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn job_type_registry_surfaces_verification_metadata() -> Result<()> {
    let app = test_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/internal/v1/marketplace/catalog/job-types")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let json = response_json(response).await?;
    let job_types = json
        .get("job_types")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("missing job_types array"))?;

    let sandbox = job_types
        .iter()
        .find(|value| {
            value.get("job_type").and_then(serde_json::Value::as_str)
                == Some(<protocol::SandboxRunRequest as protocol::JobRequest>::JOB_TYPE)
        })
        .ok_or_else(|| anyhow!("missing sandbox job type info"))?;

    let default_verification_mode = sandbox
        .pointer("/default_verification/mode")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("missing sandbox default verification mode"))?;
    assert_eq!(default_verification_mode, "objective");

    Ok(())
}

#[tokio::test]
async fn sandbox_verification_endpoint_matches_objective_semantics() -> Result<()> {
    let app = test_router();

    let request = protocol::SandboxRunRequest {
        sandbox: protocol::jobs::sandbox::SandboxConfig {
            image_digest: "sha256:test".to_string(),
            ..Default::default()
        },
        commands: vec![protocol::jobs::sandbox::SandboxCommand::new("echo hi")],
        ..Default::default()
    };

    let response = protocol::SandboxRunResponse {
        env_info: protocol::jobs::sandbox::EnvInfo {
            image_digest: "sha256:test".to_string(),
            hostname: None,
            system_info: None,
        },
        runs: vec![protocol::jobs::sandbox::CommandResult {
            cmd: "echo hi".to_string(),
            exit_code: 0,
            duration_ms: 1,
            stdout_sha256: "stdout".to_string(),
            stderr_sha256: "stderr".to_string(),
            stdout_preview: None,
            stderr_preview: None,
        }],
        artifacts: Vec::new(),
        status: protocol::jobs::sandbox::SandboxStatus::Success,
        error: None,
        provenance: protocol::Provenance::new("test"),
    };

    let ok_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/verifications/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "request": request.clone(),
                    "response": response.clone()
                }))?))?,
        )
        .await?;
    assert_eq!(ok_response.status(), axum::http::StatusCode::OK);
    let ok_json = response_json(ok_response).await?;
    assert_eq!(ok_json.get("passed").and_then(Value::as_bool), Some(true));
    assert_eq!(ok_json.get("exit_code").and_then(Value::as_i64), Some(0));

    let mut failing_response = response;
    failing_response.runs[0].exit_code = 2;
    failing_response.status = protocol::jobs::sandbox::SandboxStatus::Failed;

    let fail_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/verifications/sandbox-run")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "request": request,
                    "response": failing_response
                }))?))?,
        )
        .await?;
    assert_eq!(fail_response.status(), axum::http::StatusCode::OK);
    let fail_json = response_json(fail_response).await?;
    assert_eq!(
        fail_json.get("passed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(fail_json.get("exit_code").and_then(Value::as_i64), Some(2));
    Ok(())
}

#[tokio::test]
async fn repo_index_verification_endpoint_matches_objective_semantics() -> Result<()> {
    let app = test_router();

    let digests = vec![
        protocol::jobs::repo_index::RepoFileDigest {
            path: "README.md".to_string(),
            sha256: "0".repeat(64),
            bytes: 1,
        },
        protocol::jobs::repo_index::RepoFileDigest {
            path: "src/lib.rs".to_string(),
            sha256: "1".repeat(64),
            bytes: 2,
        },
    ];
    let tree = protocol::jobs::repo_index::compute_tree_sha256(&digests)?;

    let request = protocol::RepoIndexRequest {
        expected_tree_sha256: tree.clone(),
        ..Default::default()
    };
    let response = protocol::RepoIndexResponse {
        tree_sha256: tree.clone(),
        digests: digests.clone(),
        artifacts: Vec::new(),
        provenance: protocol::Provenance::new("test"),
    };

    let ok_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/verifications/repo-index")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "request": request.clone(),
                    "response": response.clone()
                }))?))?,
        )
        .await?;
    assert_eq!(ok_response.status(), axum::http::StatusCode::OK);
    let ok_json = response_json(ok_response).await?;
    assert_eq!(ok_json.get("passed").and_then(Value::as_bool), Some(true));
    assert_eq!(
        ok_json.get("tree_sha256").and_then(Value::as_str),
        Some(tree.as_str())
    );
    assert_eq!(
        ok_json
            .get("violations")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let mut failing_request = request;
    failing_request.expected_tree_sha256 = "f".repeat(64);

    let fail_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/verifications/repo-index")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "request": failing_request,
                    "response": response
                }))?))?,
        )
        .await?;
    assert_eq!(fail_response.status(), axum::http::StatusCode::OK);
    let fail_json = response_json(fail_response).await?;
    assert_eq!(
        fail_json.get("passed").and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        fail_json
            .get("violations")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0)
            > 0
    );

    Ok(())
}

#[tokio::test]
async fn khala_fleet_topic_surfaces_worker_presence_stream() -> Result<()> {
    let app = test_router();
    let sync_token = issue_sync_token(
        &["runtime.worker_lifecycle_events"],
        Some(11),
        Some("user:11"),
        "fleet-presence",
        300,
    );

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/internal/v1/workers")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "worker_id": "desktop:fleet-worker-1",
                    "owner_user_id": 11,
                    "metadata": {"roles": ["client"]}
                }))?))?,
        )
        .await?;
    assert_eq!(create_response.status(), axum::http::StatusCode::CREATED);

    let poll = app
        .oneshot(
            Request::builder()
                .uri(
                    "/internal/v1/khala/topics/fleet:user:11:workers/messages?after_seq=0&limit=10",
                )
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(poll.status(), axum::http::StatusCode::OK);
    let poll_json = response_json(poll).await?;
    let messages = poll_json
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing messages array"))?;
    assert!(!messages.is_empty());
    Ok(())
}

#[tokio::test]
async fn khala_fleet_topic_enforces_user_binding() -> Result<()> {
    let app = test_router();
    let sync_token = issue_sync_token(
        &["runtime.worker_lifecycle_events"],
        Some(11),
        Some("user:11"),
        "fleet-binding",
        300,
    );

    let poll = app
        .oneshot(
            Request::builder()
                .uri(
                    "/internal/v1/khala/topics/fleet:user:12:workers/messages?after_seq=0&limit=10",
                )
                .header("authorization", format!("Bearer {sync_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(poll.status(), axum::http::StatusCode::FORBIDDEN);
    Ok(())
}
