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
use openagents_runtime_service::config::{AuthorityWriteMode, Config as RuntimeConfig};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};

#[derive(Parser, Debug)]
struct Args {
    /// Output directory. Defaults to `output/vignettes/liquidity-pool/<run_id>`.
    #[arg(long)]
    output_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct DepositQuoteResponse {
    deposit: Value,
    receipt: Value,
}

#[derive(Debug, Deserialize)]
struct QuotePayResponse {
    quote_id: String,
    idempotency_key: String,
    invoice_hash: String,
}

#[derive(Debug, Deserialize)]
struct PayResponse {
    quote_id: String,
    status: String,
    wallet_receipt_sha256: Option<String>,
    preimage_sha256: Option<String>,
    paid_at_ms: Option<i64>,
    error_code: Option<String>,
    error_message: Option<String>,
    receipt: Value,
}

#[derive(Debug, Deserialize)]
struct PoolSnapshotResponse {
    snapshot: Value,
    receipt: Value,
}

struct RuntimeHandle {
    base_url: String,
    shutdown: oneshot::Sender<()>,
}

#[derive(Clone)]
struct MockWalletState {
    auth_token: String,
    invoices_by_request: Arc<Mutex<HashMap<String, (String, String, u64)>>>,
    payments_by_request: Arc<Mutex<HashMap<String, Value>>>,
    balance_sats: Arc<Mutex<u64>>,
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
        .unwrap_or_else(|| PathBuf::from(format!("output/vignettes/liquidity-pool/{run_id}")));
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("create output dir {}", output_dir.display()))?;

    let client = reqwest::Client::new();

    // Start mock wallet executor (no external network).
    let wallet_token = "vignette-wallet-token".to_string();
    let wallet = start_mock_wallet_executor(wallet_token.clone()).await?;
    wait_for_http_ok(&client, &wallet.base_url, "/healthz", Duration::from_secs(3)).await?;

    // Start runtime with wallet executor configured.
    let runtime = start_runtime(RuntimeDeps {
        wallet_base_url: wallet.base_url.clone(),
        wallet_auth_token: wallet_token,
        // Enable receipt signatures without enabling Nostr publish.
        receipt_signing_key: Some(deterministic_secret("vignette-liquidity-pool-mvp0")),
    })
    .await?;
    wait_for_http_ok(&client, &runtime.base_url, "/healthz", Duration::from_secs(3)).await?;

    let mut events = Vec::<Value>::new();

    // 0) Create pool.
    let pool_id = "pool_vignette_llp";
    let created_pool = post_json_expect_ok(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/admin/create"),
        json!({
            "schema": "openagents.liquidity.pool.create_request.v1",
            "operator_id": "operator:vignette",
            "pool_kind": "llp",
            "status": "active",
            "config": {
                "schema": "openagents.liquidity.pool_config.v1",
                "note": "vignette pool"
            }
        }),
    )
    .await?;
    events.push(json!({"at": Utc::now().to_rfc3339(), "step": "pool_create", "pool_id": pool_id, "response": created_pool}));

    // 1) Deposit quote (LN invoice) + stable idempotency.
    let lp_id = "lp:vignette";
    let deposit_idem = "idem:deposit-1";
    let deposit_body = json!({
        "schema": "openagents.liquidity.pool.deposit_quote_request.v1",
        "lp_id": lp_id,
        "idempotency_key": deposit_idem,
        "rail": "lightning_invoice",
        "amount_sats": 2500,
        "description": "vignette deposit",
        "expiry_secs": 900,
    });

    let deposit_resp_1: DepositQuoteResponse = post_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/deposit_quote"),
        deposit_body.clone(),
    )
    .await?;

    let invoice_1 = deposit_resp_1
        .deposit
        .pointer("/invoice_bolt11")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if invoice_1.trim().is_empty() {
        return Err(anyhow!("deposit_quote missing invoice_bolt11"));
    }

    let deposit_resp_2: DepositQuoteResponse = post_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/deposit_quote"),
        deposit_body.clone(),
    )
    .await?;

    let deposit_id_1 = deposit_resp_1
        .deposit
        .pointer("/deposit_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let deposit_id_2 = deposit_resp_2
        .deposit
        .pointer("/deposit_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if deposit_id_1.is_empty() || deposit_id_1 != deposit_id_2 {
        return Err(anyhow!(
            "deposit_quote idempotency violated: deposit_id mismatch: {deposit_id_1} vs {deposit_id_2}"
        ));
    }

    let invoice_2 = deposit_resp_2
        .deposit
        .pointer("/invoice_bolt11")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if invoice_1 != invoice_2 {
        return Err(anyhow!(
            "deposit_quote idempotency violated: invoice mismatch"
        ));
    }

    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "deposit_quote",
        "pool_id": pool_id,
        "lp_id": lp_id,
        "deposit_id": deposit_id_1,
        "invoice_bolt11": invoice_1,
        "receipt_id": deposit_resp_1.receipt.get("receipt_id").and_then(Value::as_str),
        "receipt_sha256": deposit_resp_1.receipt.get("canonical_json_sha256").and_then(Value::as_str),
    }));

    // 2) Liquidity quote + pay (uses liquidity service; independent from pool deposit).
    // We use a synthetic-but-parseable invoice format for `openagents-l402::Bolt11`.
    let invoice_to_pay = build_synthetic_bolt11_invoice_msats(1_500_000); // 1500 sats
    let quote_idem = "idem:quote-pay-1";
    let quote_resp_1: QuotePayResponse = post_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        "/internal/v1/liquidity/quote_pay",
        json!({
            "schema": "openagents.liquidity.quote_pay_request.v1",
            "idempotency_key": quote_idem,
            "invoice": invoice_to_pay,
            "host": "example.l402.local",
            "max_amount_msats": 2_000_000,
            "max_fee_msats": 50_000,
            "urgency": "normal",
            "policy_context": { "schema": "openagents.liquidity.policy_context.v1", "note": "vignette" }
        }),
    )
    .await?;
    events.push(json!({"at": Utc::now().to_rfc3339(), "step": "liquidity_quote_pay", "quote_id": quote_resp_1.quote_id, "invoice_hash": quote_resp_1.invoice_hash}));

    // Idempotency: same key+params returns same quote_id.
    let quote_resp_2: QuotePayResponse = post_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        "/internal/v1/liquidity/quote_pay",
        json!({
            "schema": "openagents.liquidity.quote_pay_request.v1",
            "idempotency_key": quote_idem,
            "invoice": invoice_to_pay,
            "host": "example.l402.local",
            "max_amount_msats": 2_000_000,
            "max_fee_msats": 50_000,
            "urgency": "normal",
            "policy_context": { "schema": "openagents.liquidity.policy_context.v1", "note": "vignette" }
        }),
    )
    .await?;
    if quote_resp_1.quote_id != quote_resp_2.quote_id {
        return Err(anyhow!(
            "quote_pay idempotency violated: quote_id mismatch {} vs {}",
            quote_resp_1.quote_id,
            quote_resp_2.quote_id
        ));
    }
    if quote_resp_2.idempotency_key != quote_idem {
        return Err(anyhow!(
            "quote_pay response idempotency_key mismatch: expected {}, got {}",
            quote_idem,
            quote_resp_2.idempotency_key
        ));
    }

    // Conflict: same idempotency key, different invoice -> 409.
    assert_post_json_conflict(
        &client,
        &runtime.base_url,
        "/internal/v1/liquidity/quote_pay",
        json!({
            "schema": "openagents.liquidity.quote_pay_request.v1",
            "idempotency_key": quote_idem,
            "invoice": build_synthetic_bolt11_invoice_msats(2_000_000),
            "host": "example.l402.local",
            "max_amount_msats": 2_500_000,
            "max_fee_msats": 50_000,
            "urgency": "normal",
            "policy_context": { "schema": "openagents.liquidity.policy_context.v1", "note": "vignette-different" }
        }),
    )
    .await?;
    events.push(json!({"at": Utc::now().to_rfc3339(), "step": "liquidity_quote_pay_conflict_ok"}));

    // Pay.
    let pay_resp_1: PayResponse = post_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        "/internal/v1/liquidity/pay",
        json!({
            "schema": "openagents.liquidity.pay_request.v1",
            "quote_id": quote_resp_1.quote_id,
        }),
    )
    .await?;

    if pay_resp_1.status != "succeeded" {
        return Err(anyhow!(
            "liquidity pay failed: status={} code={:?} msg={:?}",
            pay_resp_1.status,
            pay_resp_1.error_code,
            pay_resp_1.error_message
        ));
    }
    let receipt_id = pay_resp_1
        .receipt
        .get("receipt_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let receipt_sha256 = pay_resp_1
        .receipt
        .get("canonical_json_sha256")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if receipt_id.is_empty() || receipt_sha256.is_empty() {
        return Err(anyhow!("liquidity pay missing receipt_id/canonical_json_sha256"));
    }

    // Idempotency: second pay call returns stable response.
    let pay_resp_2: PayResponse = post_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        "/internal/v1/liquidity/pay",
        json!({
            "schema": "openagents.liquidity.pay_request.v1",
            "quote_id": pay_resp_1.quote_id,
        }),
    )
    .await?;
    if pay_resp_2.receipt != pay_resp_1.receipt {
        return Err(anyhow!("liquidity pay idempotency violated: receipt mismatch"));
    }

    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "liquidity_pay",
        "quote_id": pay_resp_1.quote_id,
        "receipt_id": receipt_id,
        "receipt_sha256": receipt_sha256,
        "wallet_receipt_sha256": pay_resp_1.wallet_receipt_sha256,
        "preimage_sha256": pay_resp_1.preimage_sha256,
        "paid_at_ms": pay_resp_1.paid_at_ms
    }));

    // 3) Snapshot generation via GET .../snapshots/latest?generate=true.
    let snapshot: PoolSnapshotResponse = get_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/snapshots/latest?generate=true"),
    )
    .await?;

    // Snapshot receipt must be signed when receipt signing key is configured.
    if snapshot
        .receipt
        .get("signature")
        .map(|v| v.is_null())
        .unwrap_or(true)
    {
        return Err(anyhow!("snapshot receipt missing signature (expected when configured)"));
    }

    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "pool_snapshot_generate",
        "pool_id": pool_id,
        "snapshot_id": snapshot.snapshot.get("snapshot_id").and_then(Value::as_str),
        "share_price_sats": snapshot.snapshot.get("share_price_sats"),
        "receipt_id": snapshot.receipt.get("receipt_id").and_then(Value::as_str),
        "receipt_sha256": snapshot.receipt.get("canonical_json_sha256").and_then(Value::as_str),
    }));

    // Write artifacts.
    let summary = json!({
        "schema": "openagents.vignette.liquidity_pool_mvp0.summary.v1",
        "run_id": run_id,
        "runtime_base_url": runtime.base_url,
        "wallet_base_url": wallet.base_url,
        "pool_id": pool_id,
        "lp_id": lp_id,
        "deposit_id": deposit_id_1,
        "liquidity_quote_id": quote_resp_1.quote_id,
        "liquidity_receipt_id": receipt_id,
        "liquidity_receipt_sha256": receipt_sha256,
        "generated_at": Utc::now().to_rfc3339(),
    });
    std::fs::write(
        output_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;

    let mut events_jsonl = String::new();
    for event in &events {
        events_jsonl.push_str(&serde_json::to_string(event)?);
        events_jsonl.push('\n');
    }
    std::fs::write(output_dir.join("events.jsonl"), events_jsonl)?;

    // Shutdown servers.
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
    config.service_name = "runtime-vignette-liquidity-pool-mvp0".to_string();
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
        invoices_by_request: Arc::new(Mutex::new(HashMap::new())),
        payments_by_request: Arc::new(Mutex::new(HashMap::new())),
        balance_sats: Arc::new(Mutex::new(250_000)), // non-zero so snapshots are meaningful
    };

    let router = Router::new()
        .route("/healthz", get(wallet_healthz))
        .route("/status", get(wallet_status))
        .route("/create-invoice", post(wallet_create_invoice))
        .route("/receive-address", get(wallet_receive_address))
        .route("/pay-bolt11", post(wallet_pay_bolt11))
        .with_state(state.clone());

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

async fn wallet_status(
    headers: HeaderMap,
    State(state): State<MockWalletState>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.auth_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}})),
        );
    }
    let balance_sats = *state.balance_sats.lock().await;
    (
        StatusCode::OK,
        Json(json!({"ok": true, "status": { "balanceSats": balance_sats }})),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInvoiceBody {
    amount_sats: u64,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    expiry_secs: Option<u64>,
}

async fn wallet_create_invoice(
    headers: HeaderMap,
    State(state): State<MockWalletState>,
    Json(body): Json<CreateInvoiceBody>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.auth_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}})),
        );
    }
    if body.amount_sats == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": {"code": "invalid_request", "message": "amount_sats must be > 0"}})),
        );
    }
    // Included for schema parity, but not used for mock invoice generation.
    let _ = body.description.as_deref();
    let _ = body.expiry_secs;

    let request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("mock-request");

    let mut invoices = state.invoices_by_request.lock().await;
    let (invoice, invoice_hash, amount_sats) = match invoices.get(request_id).cloned() {
        Some(existing) => existing,
        None => {
            let invoice = build_synthetic_bolt11_invoice_msats(body.amount_sats.saturating_mul(1000));
            let invoice_hash = sha256_hex(invoice.as_bytes());
            let record = (invoice, invoice_hash, body.amount_sats);
            invoices.insert(request_id.to_string(), record.clone());
            record
        }
    };

    // Simple "receive" model: mock wallets immediately reflect inbound deposits.
    // This makes pool snapshots non-trivial without having to run invoice settlement machinery.
    {
        let mut balance = state.balance_sats.lock().await;
        *balance = balance.saturating_add(amount_sats);
    }

    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "result": {
                "invoice": invoice,
                "invoiceHash": invoice_hash
            }
        })),
    )
}

async fn wallet_receive_address(
    headers: HeaderMap,
    State(state): State<MockWalletState>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.auth_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}})),
        );
    }
    (
        StatusCode::OK,
        Json(json!({"ok": true, "result": {"sparkAddress": "vignette@spark.mock", "bitcoinAddress": "bc1qvignette000000000000000000000000000000"}})),
    )
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

async fn wallet_pay_bolt11(
    headers: HeaderMap,
    State(state): State<MockWalletState>,
    Json(body): Json<PayBolt11Body>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.auth_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}})),
        );
    }

    let request_id = body
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("mock-pay");

    let mut payments = state.payments_by_request.lock().await;
    if let Some(existing) = payments.get(request_id).cloned() {
        return (StatusCode::OK, Json(existing));
    }

    let paid_at_ms = Utc::now().timestamp_millis();
    let receipt_sha256 = sha256_hex(format!("{request_id}|{}", body.payment.host).as_bytes());
    let preimage_sha256 = sha256_hex(format!("preimage|{request_id}").as_bytes());

    let response = json!({
        "ok": true,
        "result": {
            "paymentId": format!("mock-pay-{}", &receipt_sha256[..16]),
            "receipt": {
                "schema": "openagents.lightning.wallet_receipt.v1",
                "receiptId": format!("wrec_{}", &receipt_sha256[..24]),
                "canonicalJsonSha256": receipt_sha256,
                "preimageSha256": preimage_sha256,
                "paidAtMs": paid_at_ms
            },
            "payment": {
                "invoice": body.payment.invoice,
                "maxAmountMsats": body.payment.max_amount_msats,
                "host": body.payment.host
            }
        }
    });

    payments.insert(request_id.to_string(), response.clone());
    (StatusCode::OK, Json(response))
}

fn deterministic_secret(seed: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"openagents:vignette:");
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn build_synthetic_bolt11_invoice_msats(amount_msats: u64) -> String {
    // Minimal amount parser used in `openagents-l402::Bolt11`:
    // - starts with `ln` + 2 lowercase letters (network)
    // - digits for amount
    // - optional multiplier (m/u/n/p)
    // - separator `1`
    //
    // We use `n` multiplier (1 n = 100 msats) so we can express most values.
    let amount_units = (amount_msats / 100).max(1);
    format!("lnbc{amount_units}n1vignette")
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

async fn post_json_expect_ok(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    body: Value,
) -> Result<Value> {
    let url = format!("{base_url}{path}");
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("POST {}", path))?;
    let status = resp.status();
    let json = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(anyhow!("POST {} failed: http_{}: {}", path, status.as_u16(), json));
    }
    Ok(json)
}

async fn post_json_expect_ok_typed<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    body: Value,
) -> Result<T> {
    let json = post_json_expect_ok(client, base_url, path, body).await?;
    serde_json::from_value(json).with_context(|| format!("parse response from {}", path))
}

async fn get_json_expect_ok_typed<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
) -> Result<T> {
    let url = format!("{base_url}{path}");
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {}", path))?;
    let status = resp.status();
    let json = resp.json::<Value>().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(anyhow!("GET {} failed: http_{}: {}", path, status.as_u16(), json));
    }
    serde_json::from_value(json).with_context(|| format!("parse response from {}", path))
}

async fn assert_post_json_conflict(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    body: Value,
) -> Result<()> {
    let url = format!("{base_url}{path}");
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("POST {} (expect conflict)", path))?;
    let status = resp.status();
    if status != StatusCode::CONFLICT {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "expected 409 conflict from {}, got {}: {}",
            path,
            status,
            text
        ));
    }
    Ok(())
}
