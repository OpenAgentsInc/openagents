use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use clap::Parser;
use neobank::{
    CepPaymentContext, InMemoryBudgetHooks, PaymentRouteKind, QuoteAndPayBolt11Request,
    RoutePolicy, RuntimeInternalApiClient, TreasuryRouter,
};
use openagents_runtime_service::config::{AuthorityWriteMode, Config as RuntimeConfig};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};

#[derive(Parser, Debug)]
struct Args {
    /// Output directory. Defaults to output/vignettes/neobank-pay-bolt11/<run_id>.
    #[arg(long)]
    output_dir: Option<PathBuf>,
}

struct RuntimeHandle {
    base_url: String,
    shutdown: oneshot::Sender<()>,
}

#[derive(Clone)]
struct MockWalletState {
    auth_token: String,
    payments_by_request: Arc<Mutex<HashMap<String, Value>>>,
}

struct MockWalletHandle {
    base_url: String,
    shutdown: oneshot::Sender<()>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let run_id = uuid::Uuid::now_v7().to_string();
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| PathBuf::from(format!("output/vignettes/neobank-pay-bolt11/{run_id}")));
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("create output dir {}", output_dir.display()))?;

    let wallet_token = "vignette-wallet-token".to_string();
    let wallet = start_mock_wallet_executor(wallet_token.clone()).await?;
    let runtime = start_runtime(RuntimeDeps {
        wallet_base_url: wallet.base_url.clone(),
        wallet_auth_token: wallet_token,
        receipt_signing_key: None,
    })
    .await?;

    let http = reqwest::Client::new();
    wait_for_http_ok(
        &http,
        wallet.base_url.as_str(),
        "/healthz",
        Duration::from_secs(3),
    )
    .await?;
    wait_for_http_ok(
        &http,
        runtime.base_url.as_str(),
        "/healthz",
        Duration::from_secs(3),
    )
    .await?;

    let runtime_client = RuntimeInternalApiClient::new(runtime.base_url.clone(), None);
    let budget_hooks = Arc::new(InMemoryBudgetHooks::new(500_000_000));
    let router = TreasuryRouter::new(runtime_client, budget_hooks);

    let direct_req = QuoteAndPayBolt11Request {
        invoice: build_synthetic_bolt11_invoice_msats(42_000_000),
        host: "provider.mock".to_string(),
        max_fee_msats: 7_500,
        urgency: Some("normal".to_string()),
        policy_context: json!({
            "schema": "openagents.policy_bundle.v1",
            "policyId": "policy_direct",
            "reason": "vignette-direct"
        }),
        run_id: Some("run_direct".to_string()),
        trajectory_hash: Some("traj_direct".to_string()),
        idempotency_key: "nb-direct-1".to_string(),
        route_policy: RoutePolicy::DirectOnly,
        cep: None,
    };

    let direct = router.quote_and_pay_bolt11(direct_req).await?;
    if direct.route_kind != PaymentRouteKind::DirectLiquidity {
        return Err(anyhow!("expected direct route kind"));
    }
    if direct.status != "succeeded" {
        return Err(anyhow!("direct payment not succeeded: {}", direct.status));
    }

    let cep_req = QuoteAndPayBolt11Request {
        invoice: build_synthetic_bolt11_invoice_msats(1_500_000),
        host: "provider.mock".to_string(),
        max_fee_msats: 2_000,
        urgency: Some("normal".to_string()),
        policy_context: json!({
            "schema": "openagents.policy_bundle.v1",
            "policyId": "policy_cep",
            "reason": "vignette-cep"
        }),
        run_id: Some("run_cep".to_string()),
        trajectory_hash: Some("traj_cep".to_string()),
        idempotency_key: "nb-cep-1".to_string(),
        route_policy: RoutePolicy::ForceCep,
        cep: Some(CepPaymentContext {
            agent_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            pool_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            provider_id: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string(),
            scope_id: "oa.sandbox_run.v1:fixture".to_string(),
            max_sats_cap: None,
            offer_ttl_seconds: Some(600),
            verification_passed: true,
            verification_receipt_sha256: Some(sha256_hex(b"vignette-verification")),
        }),
    };

    let cep = router.quote_and_pay_bolt11(cep_req).await?;
    if cep.route_kind != PaymentRouteKind::CepEnvelope {
        return Err(anyhow!("expected cep route kind"));
    }
    if cep.status != "success" {
        return Err(anyhow!("cep settle not successful: {}", cep.status));
    }

    let summary = json!({
        "schema": "openagents.vignette.neobank_pay_bolt11.summary.v1",
        "run_id": run_id,
        "runtime_base_url": runtime.base_url,
        "wallet_base_url": wallet.base_url,
        "direct": {
            "budget_reservation_id": direct.budget_reservation_id,
            "liquidity_quote_id": direct.liquidity_quote_id,
            "liquidity_receipt_sha256": direct.liquidity_receipt_sha256,
            "receipt_id": direct.receipt.receipt_id,
            "receipt_sha256": direct.receipt.canonical_json_sha256,
        },
        "cep": {
            "budget_reservation_id": cep.budget_reservation_id,
            "credit_offer_id": cep.credit_offer_id,
            "credit_envelope_id": cep.credit_envelope_id,
            "credit_settlement_receipt_sha256": cep.credit_settlement_receipt_sha256,
            "liquidity_receipt_sha256": cep.liquidity_receipt_sha256,
            "receipt_id": cep.receipt.receipt_id,
            "receipt_sha256": cep.receipt.canonical_json_sha256,
        },
        "generated_at": Utc::now().to_rfc3339(),
    });
    std::fs::write(
        output_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )
    .context("write summary")?;

    let _ = runtime.shutdown.send(());
    let _ = wallet.shutdown.send(());

    Ok(())
}

struct RuntimeDeps {
    wallet_base_url: String,
    wallet_auth_token: String,
    receipt_signing_key: Option<[u8; 32]>,
}

async fn start_runtime(deps: RuntimeDeps) -> Result<RuntimeHandle> {
    let mut config = RuntimeConfig::from_env().context("load runtime config")?;
    config.service_name = "runtime-vignette-neobank-pay-bolt11".to_string();
    config.build_sha = "vignette".to_string();
    config.authority_write_mode = AuthorityWriteMode::RustActive;
    config.bind_addr = "127.0.0.1:0".parse().context("parse bind addr")?;
    config.liquidity_wallet_executor_base_url = Some(deps.wallet_base_url);
    config.liquidity_wallet_executor_auth_token = Some(deps.wallet_auth_token);
    config.bridge_nostr_secret_key = deps.receipt_signing_key;
    config.bridge_nostr_relays = Vec::new();

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .context("bind runtime listener")?;
    let addr = listener.local_addr().context("runtime local_addr")?;
    let app = openagents_runtime_service::build_app(config)
        .await
        .context("build runtime app")?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(err) = server.await {
            tracing::error!(error = %err, "runtime server failed");
        }
    });

    Ok(RuntimeHandle {
        base_url: format!("http://{addr}"),
        shutdown: shutdown_tx,
    })
}

async fn start_mock_wallet_executor(auth_token: String) -> Result<MockWalletHandle> {
    let state = MockWalletState {
        auth_token,
        payments_by_request: Arc::new(Mutex::new(HashMap::new())),
    };

    let router = Router::new()
        .route("/healthz", get(wallet_healthz))
        .route("/pay-bolt11", post(wallet_pay_bolt11))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("bind wallet listener")?;
    let addr = listener.local_addr().context("wallet local_addr")?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(err) = server.await {
            tracing::error!(error = %err, "wallet server failed");
        }
    });

    Ok(MockWalletHandle {
        base_url: format!("http://{addr}"),
        shutdown: shutdown_tx,
    })
}

async fn wallet_healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

fn is_authorized(headers: &HeaderMap, expected_token: &str) -> bool {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value == format!("Bearer {expected_token}"))
        .unwrap_or(false)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PayBolt11Body {
    #[serde(default)]
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

async fn wallet_pay_bolt11(
    headers: HeaderMap,
    State(state): State<MockWalletState>,
    Json(body): Json<PayBolt11Body>,
) -> impl IntoResponse {
    if !is_authorized(&headers, state.auth_token.as_str()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}}),
            ),
        );
    }

    let request_id = body
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mock-pay");

    let mut payments = state.payments_by_request.lock().await;
    if let Some(existing) = payments.get(request_id).cloned() {
        return (StatusCode::OK, Json(existing));
    }

    let paid_at_ms = Utc::now().timestamp_millis();
    let invoice_hash = sha256_hex(body.payment.invoice.as_bytes());
    let payment_id_seed = sha256_hex(format!("{request_id}|{}", body.payment.host).as_bytes());
    let payment_id = format!("mock-pay-{}", &payment_id_seed[..16]);
    let preimage_hex = sha256_hex(format!("preimage|{request_id}").as_bytes());
    let preimage_sha256 = sha256_hex(preimage_hex.as_bytes());
    let receipt_sha256 = sha256_hex(
        format!(
            "{request_id}|{payment_id}|{invoice_hash}|{}|{paid_at_ms}",
            body.payment.max_amount_msats
        )
        .as_bytes(),
    );

    let response = json!({
        "ok": true,
        "result": {
            "receipt": {
                "receiptVersion": "openagents.lightning.wallet_receipt.v1",
                "receiptId": format!("wrec_{}", &receipt_sha256[..24]),
                "requestId": request_id,
                "walletId": "mock-wallet",
                "host": body.payment.host,
                "paymentId": payment_id,
                "invoiceHash": invoice_hash,
                "quotedAmountMsats": body.payment.max_amount_msats,
                "settledAmountMsats": body.payment.max_amount_msats,
                "preimageSha256": preimage_sha256,
                "paidAtMs": paid_at_ms,
                "rail": "lightning",
                "assetId": "BTC_LN",
                "canonicalJsonSha256": receipt_sha256,
            },
            "requestId": request_id,
            "walletId": "mock-wallet",
            "payment": {
                "paymentId": payment_id,
                "amountMsats": body.payment.max_amount_msats,
                "preimageHex": preimage_hex,
                "paidAtMs": paid_at_ms,
            },
            "quotedAmountMsats": body.payment.max_amount_msats,
            "windowSpendMsatsAfterPayment": body.payment.max_amount_msats,
        }
    });

    payments.insert(request_id.to_string(), response.clone());
    (StatusCode::OK, Json(response))
}

fn build_synthetic_bolt11_invoice_msats(amount_msats: u64) -> String {
    let amount_units = (amount_msats / 100).max(1);
    format!("lnbc{amount_units}n1vignette")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

async fn wait_for_http_ok(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    let url = format!("{base_url}{path}");
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow!("timeout waiting for {}", url));
        }
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}
