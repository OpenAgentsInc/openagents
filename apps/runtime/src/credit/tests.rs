use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};

use crate::credit::service::CreditPolicyConfig;
use crate::credit::service::CreditService;
use crate::credit::store;
use crate::credit::types::{
    CREDIT_AGENT_EXPOSURE_RESPONSE_SCHEMA_V1, CREDIT_ENVELOPE_REQUEST_SCHEMA_V1,
    CREDIT_HEALTH_RESPONSE_SCHEMA_V1, CREDIT_OFFER_REQUEST_SCHEMA_V1,
    CREDIT_SETTLE_REQUEST_SCHEMA_V1, CreditEnvelopeRequestV1, CreditOfferRequestV1,
    CreditScopeTypeV1, CreditSettleRequestV1, DEFAULT_NOTICE_SCHEMA_V1,
    ENVELOPE_ISSUE_RECEIPT_SCHEMA_V1, ENVELOPE_SETTLEMENT_RECEIPT_SCHEMA_V1,
};
use crate::liquidity::{LiquidityService, store as liquidity_store};

#[derive(Clone)]
struct WalletState {
    token: String,
    pay_calls: Arc<AtomicU64>,
    fail_bolt11: bool,
    cache: Arc<Mutex<HashMap<String, Value>>>,
}

struct WalletHandle {
    base_url: String,
    pay_calls: Arc<AtomicU64>,
    shutdown: oneshot::Sender<()>,
}

#[tokio::test]
async fn cep_issue_and_settle_success_is_idempotent() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token".to_string(), false).await?;

    let signing_key = [42u8; 32];
    let liquidity = Arc::new(LiquidityService::new(
        liquidity_store::memory(),
        Some(wallet.base_url.clone()),
        Some("test-token".to_string()),
        5_000,
        60,
        Some(signing_key),
    ));
    let credit = CreditService::new(store::memory(), liquidity, Some(signing_key));

    let agent_pk = "a".repeat(64);
    let pool_pk = "b".repeat(64);
    let provider_pk = "c".repeat(64);

    let offer = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_pk.clone(),
            pool_id: pool_pk.clone(),
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_hash_vignette".to_string(),
            max_sats: 10_000,
            fee_bps: 200,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await
        .context("offer")?;

    let envelope = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer.offer.offer_id.clone(),
            provider_id: provider_pk.clone(),
        })
        .await
        .context("envelope")?;

    assert_eq!(envelope.receipt.schema, ENVELOPE_ISSUE_RECEIPT_SCHEMA_V1);
    if envelope.receipt.signature.is_none() {
        return Err(anyhow!("expected envelope issue receipt to be signed"));
    }

    let invoice = build_synthetic_bolt11_invoice_msats(250_000); // 250 sats

    let settled_1 = credit
        .settle(CreditSettleRequestV1 {
            schema: CREDIT_SETTLE_REQUEST_SCHEMA_V1.to_string(),
            envelope_id: envelope.envelope.envelope_id.clone(),
            verification_passed: true,
            verification_receipt_sha256: "sha256:verification".to_string(),
            provider_invoice: invoice.clone(),
            provider_host: "provider.test".to_string(),
            max_fee_msats: 50_000,
            policy_context: json!({}),
        })
        .await
        .context("settle 1")?;

    assert_eq!(settled_1.outcome, "success");
    assert_eq!(wallet.pay_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        settled_1.receipt.get("schema").and_then(Value::as_str),
        Some(ENVELOPE_SETTLEMENT_RECEIPT_SCHEMA_V1)
    );
    if settled_1
        .receipt
        .get("signature")
        .map(|v| v.is_null())
        .unwrap_or(true)
    {
        return Err(anyhow!("expected settlement receipt signature"));
    }

    // Idempotency: second call returns the stored receipt and does not pay again.
    let settled_2 = credit
        .settle(CreditSettleRequestV1 {
            schema: CREDIT_SETTLE_REQUEST_SCHEMA_V1.to_string(),
            envelope_id: envelope.envelope.envelope_id.clone(),
            verification_passed: true,
            verification_receipt_sha256: "sha256:verification".to_string(),
            provider_invoice: invoice,
            provider_host: "provider.test".to_string(),
            max_fee_msats: 50_000,
            policy_context: json!({}),
        })
        .await
        .context("settle 2")?;
    assert_eq!(settled_2.receipt, settled_1.receipt);
    assert_eq!(wallet.pay_calls.load(Ordering::Relaxed), 1);

    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn cep_settle_verification_failed_emits_default_and_does_not_pay() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token".to_string(), false).await?;

    let signing_key = [7u8; 32];
    let liquidity = Arc::new(LiquidityService::new(
        liquidity_store::memory(),
        Some(wallet.base_url.clone()),
        Some("test-token".to_string()),
        5_000,
        60,
        Some(signing_key),
    ));
    let credit = CreditService::new(store::memory(), liquidity, Some(signing_key));

    let offer = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: "d".repeat(64),
            pool_id: "e".repeat(64),
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_hash_failed".to_string(),
            max_sats: 10_000,
            fee_bps: 200,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;

    let envelope = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer.offer.offer_id.clone(),
            provider_id: "f".repeat(64),
        })
        .await?;

    let settled = credit
        .settle(CreditSettleRequestV1 {
            schema: CREDIT_SETTLE_REQUEST_SCHEMA_V1.to_string(),
            envelope_id: envelope.envelope.envelope_id,
            verification_passed: false,
            verification_receipt_sha256: "sha256:failed".to_string(),
            provider_invoice: build_synthetic_bolt11_invoice_msats(250_000),
            provider_host: "provider.test".to_string(),
            max_fee_msats: 50_000,
            policy_context: json!({}),
        })
        .await?;

    assert_eq!(settled.outcome, "failed");
    assert_eq!(wallet.pay_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        settled.receipt.get("schema").and_then(Value::as_str),
        Some(DEFAULT_NOTICE_SCHEMA_V1)
    );

    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn cep_settle_expired_emits_default_and_does_not_pay() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token".to_string(), false).await?;

    let signing_key = [9u8; 32];
    let liquidity = Arc::new(LiquidityService::new(
        liquidity_store::memory(),
        Some(wallet.base_url.clone()),
        Some("test-token".to_string()),
        5_000,
        1,
        Some(signing_key),
    ));
    let credit = CreditService::new(store::memory(), liquidity, Some(signing_key));

    let offer = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: "1".repeat(64),
            pool_id: "2".repeat(64),
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_hash_expired".to_string(),
            max_sats: 10_000,
            fee_bps: 200,
            requires_verifier: false,
            exp: Utc::now() + Duration::seconds(1),
        })
        .await?;

    let envelope = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer.offer.offer_id.clone(),
            provider_id: "3".repeat(64),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let settled = credit
        .settle(CreditSettleRequestV1 {
            schema: CREDIT_SETTLE_REQUEST_SCHEMA_V1.to_string(),
            envelope_id: envelope.envelope.envelope_id,
            verification_passed: true,
            verification_receipt_sha256: "sha256:passed".to_string(),
            provider_invoice: build_synthetic_bolt11_invoice_msats(250_000),
            provider_host: "provider.test".to_string(),
            max_fee_msats: 50_000,
            policy_context: json!({}),
        })
        .await?;

    assert_eq!(settled.outcome, "expired");
    assert_eq!(wallet.pay_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        settled.receipt.get("schema").and_then(Value::as_str),
        Some(DEFAULT_NOTICE_SCHEMA_V1)
    );

    let _ = wallet.shutdown.send(());
    Ok(())
}

async fn spawn_wallet_executor_stub(token: String, fail_bolt11: bool) -> Result<WalletHandle> {
    let state = WalletState {
        token,
        pay_calls: Arc::new(AtomicU64::new(0)),
        fail_bolt11,
        cache: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/healthz", get(|| async { (StatusCode::OK, "ok") }))
        .route("/pay-bolt11", post(pay_bolt11))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let _ = server.await;
    });

    Ok(WalletHandle {
        base_url: format!("http://{addr}"),
        pay_calls: state.pay_calls.clone(),
        shutdown: shutdown_tx,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PayBolt11Body {
    request_id: Option<String>,
    payment: PayBolt11Payment,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PayBolt11Payment {
    invoice: String,
    max_amount_msats: u64,
    host: String,
}

async fn pay_bolt11(
    headers: HeaderMap,
    State(state): State<WalletState>,
    Json(body): Json<PayBolt11Body>,
) -> (StatusCode, Json<Value>) {
    let ok = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim() == format!("Bearer {}", state.token))
        .unwrap_or(false);
    if !ok {
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"ok": false, "error": {"code": "unauthorized", "message": "invalid token"}}),
            ),
        );
    }

    if state.fail_bolt11 {
        state.pay_calls.fetch_add(1, Ordering::Relaxed);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({"ok": false, "error": {"code": "wallet_fail", "message": "simulated failure"}}),
            ),
        );
    }

    let request_id = body
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("pay");

    let mut cache = state.cache.lock().await;
    if let Some(existing) = cache.get(request_id).cloned() {
        return (StatusCode::OK, Json(existing));
    }

    state.pay_calls.fetch_add(1, Ordering::Relaxed);
    let receipt_sha = sha256_hex(format!("{request_id}|{}", body.payment.host).as_bytes());

    let response = json!({
        "ok": true,
        "result": {
            "paymentId": format!("mock-pay-{}", &receipt_sha[..16]),
            "receipt": {
                "schema": "openagents.lightning.wallet_receipt.v1",
                "receiptId": format!("wrec_{}", &receipt_sha[..24]),
                "canonicalJsonSha256": receipt_sha,
                "preimageSha256": sha256_hex(format!("preimage|{request_id}").as_bytes()),
                "paidAtMs": Utc::now().timestamp_millis()
            },
            "payment": {
                "invoice": body.payment.invoice,
                "maxAmountMsats": body.payment.max_amount_msats,
                "host": body.payment.host
            }
        }
    });
    cache.insert(request_id.to_string(), response.clone());
    (StatusCode::OK, Json(response))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn build_synthetic_bolt11_invoice_msats(amount_msats: u64) -> String {
    let amount_units = (amount_msats / 100).max(1);
    format!("lnbc{amount_units}n1cep")
}

#[tokio::test]
async fn cep_envelope_refuses_when_max_outstanding_exceeded() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token".to_string(), false).await?;
    let signing_key = [11u8; 32];
    let liquidity = Arc::new(LiquidityService::new(
        liquidity_store::memory(),
        Some(wallet.base_url.clone()),
        Some("test-token".to_string()),
        5_000,
        60,
        Some(signing_key),
    ));

    let policy = CreditPolicyConfig {
        max_outstanding_envelopes_per_agent: 1,
        ..CreditPolicyConfig::default()
    };
    let credit =
        CreditService::new_with_policy(store::memory(), liquidity, Some(signing_key), policy);

    let agent_id = "a".repeat(64);
    let pool_id = "b".repeat(64);
    let provider_id = "c".repeat(64);

    let offer_1 = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_id.clone(),
            pool_id: pool_id.clone(),
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_1".to_string(),
            max_sats: 10_000,
            fee_bps: 0,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;
    let _envelope_1 = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer_1.offer.offer_id,
            provider_id: provider_id.clone(),
        })
        .await?;

    let offer_2 = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_id.clone(),
            pool_id,
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_2".to_string(),
            max_sats: 10_000,
            fee_bps: 0,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;

    let err = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer_2.offer.offer_id,
            provider_id,
        })
        .await
        .err()
        .context("expected envelope issuance to fail")?;
    let message = err.to_string();
    if !message.contains("max outstanding envelopes") && !message.contains("outstanding") {
        return Err(anyhow!("unexpected error: {message}"));
    }

    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn cep_loss_circuit_breaker_halts_new_envelopes_and_health_is_versioned() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token".to_string(), false).await?;
    let signing_key = [13u8; 32];
    let liquidity = Arc::new(LiquidityService::new(
        liquidity_store::memory(),
        Some(wallet.base_url.clone()),
        Some("test-token".to_string()),
        5_000,
        60,
        Some(signing_key),
    ));

    let policy = CreditPolicyConfig {
        circuit_breaker_min_sample: 2,
        loss_rate_halt_threshold: 0.10,
        health_window_seconds: 60 * 60,
        ..CreditPolicyConfig::default()
    };
    let credit =
        CreditService::new_with_policy(store::memory(), liquidity, Some(signing_key), policy);

    let agent_id = "d".repeat(64);
    let pool_id = "e".repeat(64);
    let provider_id = "f".repeat(64);

    for idx in 0..2 {
        let offer = credit
            .offer(CreditOfferRequestV1 {
                schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
                agent_id: agent_id.clone(),
                pool_id: pool_id.clone(),
                intent_id: None,
                scope_type: CreditScopeTypeV1::Nip90,
                scope_id: format!("job_fail_{idx}"),
                max_sats: 10_000,
                fee_bps: 0,
                requires_verifier: false,
                exp: Utc::now() + Duration::minutes(10),
            })
            .await?;
        let envelope = credit
            .envelope(CreditEnvelopeRequestV1 {
                schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
                offer_id: offer.offer.offer_id,
                provider_id: provider_id.clone(),
            })
            .await?;
        let _ = credit
            .settle(CreditSettleRequestV1 {
                schema: CREDIT_SETTLE_REQUEST_SCHEMA_V1.to_string(),
                envelope_id: envelope.envelope.envelope_id,
                verification_passed: false,
                verification_receipt_sha256: format!("sha256:fail:{idx}"),
                provider_invoice: "".to_string(),
                provider_host: "".to_string(),
                max_fee_msats: 0,
                policy_context: json!({}),
            })
            .await?;
    }

    let health = credit.health().await?;
    assert_eq!(health.schema, CREDIT_HEALTH_RESPONSE_SCHEMA_V1);
    assert!(health.breakers.halt_new_envelopes);

    let offer_3 = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_id.clone(),
            pool_id: pool_id.clone(),
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_blocked".to_string(),
            max_sats: 10_000,
            fee_bps: 0,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;

    let err = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer_3.offer.offer_id,
            provider_id,
        })
        .await
        .err()
        .context("expected circuit breaker to halt envelope issuance")?;
    assert!(err.to_string().contains("circuit breaker"));

    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn cep_default_reduces_future_underwriting_limit_and_agent_exposure_is_versioned()
-> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token".to_string(), false).await?;
    let signing_key = [17u8; 32];
    let liquidity = Arc::new(LiquidityService::new(
        liquidity_store::memory(),
        Some(wallet.base_url.clone()),
        Some("test-token".to_string()),
        5_000,
        60,
        Some(signing_key),
    ));

    let policy = CreditPolicyConfig {
        underwriting_base_sats: 20_000,
        underwriting_k: 0.0,
        underwriting_default_penalty_multiplier: 5.0,
        max_sats_per_envelope: 100_000,
        ..CreditPolicyConfig::default()
    };
    let credit =
        CreditService::new_with_policy(store::memory(), liquidity, Some(signing_key), policy);

    let agent_id = "1".repeat(64);
    let pool_id = "2".repeat(64);
    let provider_id = "3".repeat(64);

    let offer_before = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_id.clone(),
            pool_id: pool_id.clone(),
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_before".to_string(),
            max_sats: 20_000,
            fee_bps: 0,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;
    let before_max = offer_before.offer.max_sats;

    let offer_fail = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_id.clone(),
            pool_id: pool_id.clone(),
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_default".to_string(),
            max_sats: 20_000,
            fee_bps: 0,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;
    let envelope_fail = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer_fail.offer.offer_id,
            provider_id: provider_id.clone(),
        })
        .await?;
    let _ = credit
        .settle(CreditSettleRequestV1 {
            schema: CREDIT_SETTLE_REQUEST_SCHEMA_V1.to_string(),
            envelope_id: envelope_fail.envelope.envelope_id,
            verification_passed: false,
            verification_receipt_sha256: "sha256:default".to_string(),
            provider_invoice: "".to_string(),
            provider_host: "".to_string(),
            max_fee_msats: 0,
            policy_context: json!({}),
        })
        .await?;

    let offer_after = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_id.clone(),
            pool_id,
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_after".to_string(),
            max_sats: 20_000,
            fee_bps: 0,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;
    let after_max = offer_after.offer.max_sats;
    if after_max >= before_max {
        return Err(anyhow!(
            "expected underwriting limit to decay after default (before={}, after={})",
            before_max,
            after_max
        ));
    }

    let exposure = credit.agent_exposure(agent_id.as_str()).await?;
    assert_eq!(exposure.schema, CREDIT_AGENT_EXPOSURE_RESPONSE_SCHEMA_V1);

    let _ = wallet.shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn cep_ln_failure_circuit_breaker_halts_large_settlements() -> Result<()> {
    let wallet = spawn_wallet_executor_stub("test-token".to_string(), true).await?;
    let signing_key = [19u8; 32];
    let liquidity = Arc::new(LiquidityService::new(
        liquidity_store::memory(),
        Some(wallet.base_url.clone()),
        Some("test-token".to_string()),
        5_000,
        60,
        Some(signing_key),
    ));

    let policy = CreditPolicyConfig {
        circuit_breaker_min_sample: 1,
        ln_failure_rate_halt_threshold: 0.01,
        ln_failure_large_settlement_cap_sats: 100,
        health_window_seconds: 60 * 60,
        loss_rate_halt_threshold: 1.0,
        ..CreditPolicyConfig::default()
    };
    let credit =
        CreditService::new_with_policy(store::memory(), liquidity, Some(signing_key), policy);

    let agent_id = "9".repeat(64);
    let pool_id = "8".repeat(64);
    let provider_id = "7".repeat(64);

    let offer_small = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_id.clone(),
            pool_id: pool_id.clone(),
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_ln_fail_small".to_string(),
            max_sats: 10_000,
            fee_bps: 0,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;
    let envelope_small = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer_small.offer.offer_id,
            provider_id: provider_id.clone(),
        })
        .await?;

    // First attempt triggers a failed LN payment (wallet returns 500).
    let _ = credit
        .settle(CreditSettleRequestV1 {
            schema: CREDIT_SETTLE_REQUEST_SCHEMA_V1.to_string(),
            envelope_id: envelope_small.envelope.envelope_id,
            verification_passed: true,
            verification_receipt_sha256: "sha256:ok".to_string(),
            provider_invoice: build_synthetic_bolt11_invoice_msats(50_000), // 50 sats
            provider_host: "provider.test".to_string(),
            max_fee_msats: 50_000,
            policy_context: json!({}),
        })
        .await;
    assert_eq!(wallet.pay_calls.load(Ordering::Relaxed), 1);

    let offer_large = credit
        .offer(CreditOfferRequestV1 {
            schema: CREDIT_OFFER_REQUEST_SCHEMA_V1.to_string(),
            agent_id: agent_id.clone(),
            pool_id,
            intent_id: None,
            scope_type: CreditScopeTypeV1::Nip90,
            scope_id: "job_ln_fail_large".to_string(),
            max_sats: 10_000,
            fee_bps: 0,
            requires_verifier: false,
            exp: Utc::now() + Duration::minutes(10),
        })
        .await?;
    let envelope_large = credit
        .envelope(CreditEnvelopeRequestV1 {
            schema: CREDIT_ENVELOPE_REQUEST_SCHEMA_V1.to_string(),
            offer_id: offer_large.offer.offer_id,
            provider_id,
        })
        .await?;

    // Second attempt should be blocked by the circuit breaker before calling wallet again.
    let err = credit
        .settle(CreditSettleRequestV1 {
            schema: CREDIT_SETTLE_REQUEST_SCHEMA_V1.to_string(),
            envelope_id: envelope_large.envelope.envelope_id,
            verification_passed: true,
            verification_receipt_sha256: "sha256:ok2".to_string(),
            provider_invoice: build_synthetic_bolt11_invoice_msats(250_000), // 250 sats
            provider_host: "provider.test".to_string(),
            max_fee_msats: 50_000,
            policy_context: json!({}),
        })
        .await
        .err()
        .context("expected large settlement to be blocked")?;
    assert!(err.to_string().contains("halt_large_settlements"));
    assert_eq!(wallet.pay_calls.load(Ordering::Relaxed), 1);

    let _ = wallet.shutdown.send(());
    Ok(())
}
