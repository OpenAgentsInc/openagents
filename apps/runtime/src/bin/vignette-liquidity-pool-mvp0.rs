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

    /// Run only the automated snapshot smoke check and exit.
    #[arg(long, default_value_t = false)]
    snapshot_smoke_only: bool,
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

#[derive(Debug, Deserialize)]
struct WithdrawRequestResponse {
    withdrawal: Value,
    receipt: Value,
}

#[derive(Debug, Deserialize)]
struct SigningRequestListResponse {
    requests: Vec<Value>,
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
    onchain_quotes_by_plan: Arc<Mutex<HashMap<String, Value>>>,
    onchain_commits_by_plan: Arc<Mutex<HashMap<String, Value>>>,
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
    wait_for_http_ok(
        &client,
        &wallet.base_url,
        "/healthz",
        Duration::from_secs(3),
    )
    .await?;

    // Start runtime with wallet executor configured.
    let runtime = start_runtime(RuntimeDeps {
        wallet_base_url: wallet.base_url.clone(),
        wallet_auth_token: wallet_token,
        // Enable receipt signatures without enabling Nostr publish.
        receipt_signing_key: Some(deterministic_secret("vignette-liquidity-pool-mvp0")),
    })
    .await?;
    wait_for_http_ok(
        &client,
        &runtime.base_url,
        "/healthz",
        Duration::from_secs(3),
    )
    .await?;

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

    // 0.5) Assert automated snapshot worker writes a latest LLP snapshot without generate=true.
    let auto_snapshot =
        wait_for_pool_snapshot(&client, &runtime.base_url, pool_id, Duration::from_secs(6))
            .await
            .context("wait for automated LLP snapshot")?;
    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "pool_snapshot_auto",
        "pool_id": pool_id,
        "snapshot_id": auto_snapshot.snapshot.get("snapshot_id").and_then(Value::as_str),
        "snapshot_as_of": auto_snapshot.snapshot.get("as_of").and_then(Value::as_str),
        "receipt_id": auto_snapshot.receipt.get("receipt_id").and_then(Value::as_str),
    }));
    if args.snapshot_smoke_only {
        let summary = json!({
            "run_id": run_id,
            "mode": "snapshot_smoke_only",
            "pool_id": pool_id,
            "snapshot_id": auto_snapshot.snapshot.get("snapshot_id").and_then(Value::as_str),
            "snapshot_as_of": auto_snapshot.snapshot.get("as_of").and_then(Value::as_str),
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
        let _ = runtime.shutdown.send(());
        let _ = wallet.shutdown.send(());
        return Ok(());
    }

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

    let deposit_shares_minted = deposit_resp_1
        .deposit
        .get("shares_minted")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if deposit_shares_minted <= 0 {
        return Err(anyhow!("deposit_quote missing shares_minted"));
    }
    let deposit_shares_minted =
        u64::try_from(deposit_shares_minted).context("deposit_quote shares_minted invalid")?;

    let confirm_deposit_resp = post_json_expect_ok(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/deposits/{deposit_id_1}/confirm"),
        json!({}),
    )
    .await?;
    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "deposit_confirm",
        "pool_id": pool_id,
        "deposit_id": deposit_id_1,
        "response": confirm_deposit_resp,
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
        return Err(anyhow!(
            "liquidity pay missing receipt_id/canonical_json_sha256"
        ));
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
        return Err(anyhow!(
            "liquidity pay idempotency violated: receipt mismatch"
        ));
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
        return Err(anyhow!(
            "snapshot receipt missing signature (expected when configured)"
        ));
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

    // 4) Small lightning withdrawal + executor tick (Gate L: pay-after-verify like behavior).
    let share_price_sats = snapshot
        .snapshot
        .get("share_price_sats")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .max(1);
    let share_price_u64 =
        u64::try_from(share_price_sats).context("snapshot share_price_sats invalid")?;

    const WITHDRAW_AUTOPAY_MAX_SATS: u64 = 100_000;
    let max_shares_for_autopay = WITHDRAW_AUTOPAY_MAX_SATS / share_price_u64;
    if max_shares_for_autopay == 0 {
        return Err(anyhow!(
            "snapshot share_price_sats too high to test autopay"
        ));
    }
    let withdraw_ln_shares_burned = deposit_shares_minted
        .min(1_000)
        .min(max_shares_for_autopay)
        .max(1);
    let withdraw_ln_amount_sats = withdraw_ln_shares_burned
        .checked_mul(share_price_u64)
        .ok_or_else(|| anyhow!("withdraw-ln amount overflow"))?;
    let withdraw_ln_idem = "idem:withdraw-ln-1";
    let withdraw_ln_invoice =
        build_synthetic_bolt11_invoice_msats(withdraw_ln_amount_sats.saturating_mul(1_000));
    let withdraw_ln_body = json!({
        "schema": "openagents.liquidity.pool.withdraw_request.v1",
        "lp_id": lp_id,
        "idempotency_key": withdraw_ln_idem,
        "shares_burned": withdraw_ln_shares_burned,
        "rail_preference": "lightning",
        "payout_invoice_bolt11": withdraw_ln_invoice,
    });
    let withdraw_ln_resp_1: WithdrawRequestResponse = post_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/withdraw_request"),
        withdraw_ln_body.clone(),
    )
    .await?;

    let withdraw_ln_id = withdraw_ln_resp_1
        .withdrawal
        .get("withdrawal_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if withdraw_ln_id.is_empty() {
        return Err(anyhow!("withdraw-ln response missing withdrawal_id"));
    }

    // Withdrawal request receipt must be signed when receipt signing key is configured.
    if withdraw_ln_resp_1
        .receipt
        .get("signature")
        .map(|v| v.is_null())
        .unwrap_or(true)
    {
        return Err(anyhow!(
            "withdraw request receipt missing signature (expected when configured)"
        ));
    }

    let tick_resp_1 = post_json_expect_ok(
        &client,
        &runtime.base_url,
        "/internal/v1/pools/admin/withdrawals/execute_due",
        json!({}),
    )
    .await?;
    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "withdraw_ln_tick",
        "response": tick_resp_1,
    }));

    let mut withdraw_ln_paid: Option<WithdrawRequestResponse> = None;
    for _ in 0..25 {
        let current: WithdrawRequestResponse = post_json_expect_ok_typed(
            &client,
            &runtime.base_url,
            &format!("/internal/v1/pools/{pool_id}/withdraw_request"),
            withdraw_ln_body.clone(),
        )
        .await?;
        let status = current
            .withdrawal
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("");
        if status == "paid" {
            withdraw_ln_paid = Some(current);
            break;
        }

        let _ = post_json_expect_ok(
            &client,
            &runtime.base_url,
            "/internal/v1/pools/admin/withdrawals/execute_due",
            json!({}),
        )
        .await?;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let withdraw_ln_paid = withdraw_ln_paid.ok_or_else(|| anyhow!("withdraw-ln did not settle"))?;
    let withdraw_ln_wallet_receipt_sha256 = withdraw_ln_paid
        .withdrawal
        .get("wallet_receipt_sha256")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if withdraw_ln_wallet_receipt_sha256.is_empty() {
        return Err(anyhow!(
            "withdraw-ln missing wallet_receipt_sha256 after settlement"
        ));
    }

    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "withdraw_ln_paid",
        "withdrawal_id": withdraw_ln_id,
        "wallet_receipt_sha256": withdraw_ln_wallet_receipt_sha256,
    }));

    // 5) On-chain withdrawal executed via signer-set approval + execute.
    let signer_secret = deterministic_secret("vignette-liquidity-pool-signer-1");
    let dummy_sha = "00".repeat(32);
    let signer_pubkey = openagents_runtime_service::artifacts::sign_receipt_sha256(
        &signer_secret,
        dummy_sha.as_str(),
    )?
    .signer_pubkey;

    let signer_set_resp = post_json_expect_ok(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/admin/signer_set"),
        json!({
            "schema": "openagents.liquidity.pool_signer_set_upsert_request.v1",
            "threshold": 1,
            "signers": [{ "pubkey": signer_pubkey, "label": "vignette-signer-1" }],
        }),
    )
    .await?;
    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "pool_signer_set_upsert",
        "response": signer_set_resp,
    }));

    let withdraw_onchain_shares_burned = 500_u64;
    let withdraw_onchain_amount_sats = withdraw_onchain_shares_burned
        .checked_mul(share_price_u64)
        .ok_or_else(|| anyhow!("withdraw-onchain amount overflow"))?;
    let withdraw_onchain_idem = "idem:withdraw-onchain-1";
    let withdraw_onchain_body = json!({
        "schema": "openagents.liquidity.pool.withdraw_request.v1",
        "lp_id": lp_id,
        "idempotency_key": withdraw_onchain_idem,
        "shares_burned": withdraw_onchain_shares_burned,
        "rail_preference": "onchain",
        "payout_address": "bc1qvignette000000000000000000000000000000",
    });
    let withdraw_onchain_resp_1: WithdrawRequestResponse = post_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/withdraw_request"),
        withdraw_onchain_body.clone(),
    )
    .await?;
    let withdraw_onchain_id = withdraw_onchain_resp_1
        .withdrawal
        .get("withdrawal_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if withdraw_onchain_id.is_empty() {
        return Err(anyhow!("withdraw-onchain response missing withdrawal_id"));
    }

    let tick_resp_2 = post_json_expect_ok(
        &client,
        &runtime.base_url,
        "/internal/v1/pools/admin/withdrawals/execute_due",
        json!({}),
    )
    .await?;
    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "withdraw_onchain_tick",
        "response": tick_resp_2,
    }));

    let signing_requests: SigningRequestListResponse = get_json_expect_ok_typed(
        &client,
        &runtime.base_url,
        &format!("/internal/v1/pools/{pool_id}/admin/signing_requests?status=pending&limit=50"),
    )
    .await?;
    let signing_request = signing_requests
        .requests
        .iter()
        .find(|req| {
            req.get("action_class")
                .and_then(Value::as_str)
                .unwrap_or("")
                == "onchain_withdrawal_batch"
                && req
                    .pointer("/payload_json/withdrawal_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    == withdraw_onchain_id.as_str()
        })
        .cloned()
        .ok_or_else(|| anyhow!("missing onchain withdrawal signing request"))?;

    let signing_request_id = signing_request
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let payload_sha256 = signing_request
        .get("payload_sha256")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if signing_request_id.is_empty() || payload_sha256.is_empty() {
        return Err(anyhow!("signing request missing request_id/payload_sha256"));
    }

    let approval_sig =
        openagents_runtime_service::artifacts::sign_receipt_sha256(&signer_secret, &payload_sha256)
            .context("sign onchain withdrawal payload")?;

    let approve_resp = post_json_expect_ok(
        &client,
        &runtime.base_url,
        &format!(
            "/internal/v1/pools/{pool_id}/admin/signing_requests/{signing_request_id}/approve"
        ),
        json!({
            "schema": "openagents.liquidity.pool_signing_approval_submit_request.v1",
            "signature": approval_sig,
        }),
    )
    .await?;
    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "withdraw_onchain_approve",
        "signing_request_id": signing_request_id,
        "response": approve_resp,
    }));

    let execute_resp = post_json_expect_ok(
        &client,
        &runtime.base_url,
        &format!(
            "/internal/v1/pools/{pool_id}/admin/signing_requests/{signing_request_id}/execute"
        ),
        json!({}),
    )
    .await?;
    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "withdraw_onchain_execute",
        "signing_request_id": signing_request_id,
        "response": execute_resp,
    }));

    let mut withdraw_onchain_paid: Option<WithdrawRequestResponse> = None;
    for _ in 0..25 {
        let current: WithdrawRequestResponse = post_json_expect_ok_typed(
            &client,
            &runtime.base_url,
            &format!("/internal/v1/pools/{pool_id}/withdraw_request"),
            withdraw_onchain_body.clone(),
        )
        .await?;
        let status = current
            .withdrawal
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("");
        if status == "paid" {
            withdraw_onchain_paid = Some(current);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let withdraw_onchain_paid =
        withdraw_onchain_paid.ok_or_else(|| anyhow!("withdraw-onchain did not settle"))?;
    let withdraw_onchain_wallet_receipt_sha256 = withdraw_onchain_paid
        .withdrawal
        .get("wallet_receipt_sha256")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if withdraw_onchain_wallet_receipt_sha256.is_empty() {
        return Err(anyhow!(
            "withdraw-onchain missing wallet_receipt_sha256 after settlement"
        ));
    }

    events.push(json!({
        "at": Utc::now().to_rfc3339(),
        "step": "withdraw_onchain_paid",
        "withdrawal_id": withdraw_onchain_id,
        "wallet_receipt_sha256": withdraw_onchain_wallet_receipt_sha256,
        "amount_sats": withdraw_onchain_amount_sats,
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
        "withdraw_ln_id": withdraw_ln_id,
        "withdraw_ln_wallet_receipt_sha256": withdraw_ln_wallet_receipt_sha256,
        "withdraw_onchain_id": withdraw_onchain_id,
        "withdraw_onchain_wallet_receipt_sha256": withdraw_onchain_wallet_receipt_sha256,
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
    config.liquidity_pool_withdraw_delay_hours = 0;
    config.liquidity_pool_snapshot_worker_enabled = true;
    config.liquidity_pool_snapshot_pool_ids = vec!["pool_vignette_llp".to_string()];
    config.liquidity_pool_snapshot_interval_seconds = 1;
    config.liquidity_pool_snapshot_jitter_seconds = 0;
    config.liquidity_pool_snapshot_retention_count = 8;
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
        onchain_quotes_by_plan: Arc::new(Mutex::new(HashMap::new())),
        onchain_commits_by_plan: Arc::new(Mutex::new(HashMap::new())),
        balance_sats: Arc::new(Mutex::new(250_000)), // non-zero so snapshots are meaningful
    };

    let router = Router::new()
        .route("/healthz", get(wallet_healthz))
        .route("/status", get(wallet_status))
        .route("/create-invoice", post(wallet_create_invoice))
        .route("/receive-address", get(wallet_receive_address))
        .route("/pay-bolt11", post(wallet_pay_bolt11))
        .route("/send-onchain/quote", post(wallet_send_onchain_quote))
        .route("/send-onchain/commit", post(wallet_send_onchain_commit))
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
            Json(
                json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}}),
            ),
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
            Json(
                json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}}),
            ),
        );
    }
    if body.amount_sats == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"ok": false, "error": {"code": "invalid_request", "message": "amount_sats must be > 0"}}),
            ),
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
            let invoice =
                build_synthetic_bolt11_invoice_msats(body.amount_sats.saturating_mul(1000));
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
            Json(
                json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}}),
            ),
        );
    }
    (
        StatusCode::OK,
        Json(
            json!({"ok": true, "result": {"sparkAddress": "vignette@spark.mock", "bitcoinAddress": "bc1qvignette000000000000000000000000000000"}}),
        ),
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
            Json(
                json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}}),
            ),
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
    let wallet_id = "mock-wallet";
    let invoice_hash = sha256_hex(body.payment.invoice.as_bytes());
    let payment_id_seed = sha256_hex(format!("{request_id}|{}", body.payment.host).as_bytes());
    let payment_id = format!("mock-pay-{}", &payment_id_seed[..16]);
    let preimage_hex = sha256_hex(format!("preimage-hex|{request_id}").as_bytes());
    let preimage_sha256 = sha256_hex(preimage_hex.as_bytes());
    let receipt_sha256 = sha256_hex(
        format!(
            "{request_id}|{wallet_id}|{payment_id}|{invoice_hash}|{}|{paid_at_ms}",
            body.payment.max_amount_msats
        )
        .as_bytes(),
    );

    {
        let amount_sats = body.payment.max_amount_msats / 1_000;
        let mut balance = state.balance_sats.lock().await;
        *balance = balance.saturating_sub(amount_sats);
    }

    let response = json!({
        "ok": true,
        "result": {
            "receipt": {
                "receiptVersion": "openagents.lightning.wallet_receipt.v1",
                "receiptId": format!("wrec_{}", &receipt_sha256[..24]),
                "requestId": request_id,
                "walletId": wallet_id,
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
            "walletId": wallet_id,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendOnchainQuoteBody {
    #[serde(default)]
    request_id: Option<String>,
    payment: SendOnchainQuotePayment,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendOnchainQuotePayment {
    address: String,
    amount_sats: u64,
    confirmation_speed: String,
}

async fn wallet_send_onchain_quote(
    headers: HeaderMap,
    State(state): State<MockWalletState>,
    Json(body): Json<SendOnchainQuoteBody>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.auth_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}}),
            ),
        );
    }

    let plan_id = body
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("mock-plan")
        .to_string();

    let mut quotes = state.onchain_quotes_by_plan.lock().await;
    if let Some(existing) = quotes.get(&plan_id).cloned() {
        return (StatusCode::OK, Json(existing));
    }

    let fee_sats = (body.payment.amount_sats / 1000).max(1).min(500);
    let total_sats = body.payment.amount_sats.saturating_add(fee_sats);
    let quoted_at_ms = Utc::now().timestamp_millis();

    let response = json!({
        "ok": true,
        "result": {
            "planId": plan_id,
            "walletId": "mock-wallet",
            "address": body.payment.address,
            "amountSats": body.payment.amount_sats,
            "confirmationSpeed": body.payment.confirmation_speed,
            "feeSats": fee_sats,
            "totalSats": total_sats,
            "quotedAtMs": quoted_at_ms,
        }
    });

    quotes.insert(
        response
            .pointer("/result/planId")
            .and_then(Value::as_str)
            .unwrap_or("mock-plan")
            .to_string(),
        response.clone(),
    );
    (StatusCode::OK, Json(response))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendOnchainCommitBody {
    plan_id: String,
}

async fn wallet_send_onchain_commit(
    headers: HeaderMap,
    State(state): State<MockWalletState>,
    Json(body): Json<SendOnchainCommitBody>,
) -> impl IntoResponse {
    if !is_authorized(&headers, &state.auth_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"ok": false, "error": {"code": "unauthorized", "message": "missing/invalid token"}}),
            ),
        );
    }

    let plan_id = body.plan_id.trim().to_string();
    if plan_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"ok": false, "error": {"code": "invalid_request", "message": "plan_id is required"}}),
            ),
        );
    }

    let commits = state.onchain_commits_by_plan.lock().await;
    if let Some(existing) = commits.get(&plan_id).cloned() {
        return (StatusCode::OK, Json(existing));
    }
    drop(commits);

    let quotes = state.onchain_quotes_by_plan.lock().await;
    let Some(quote) = quotes.get(&plan_id).cloned() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"ok": false, "error": {"code": "invalid_request", "message": "unknown plan_id"}}),
            ),
        );
    };
    drop(quotes);

    let address = quote
        .pointer("/result/address")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let amount_sats = quote
        .pointer("/result/amountSats")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let confirmation_speed = quote
        .pointer("/result/confirmationSpeed")
        .and_then(Value::as_str)
        .unwrap_or("normal")
        .to_string();
    let fee_sats = quote
        .pointer("/result/feeSats")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let total_sats = amount_sats.saturating_add(fee_sats);

    let txid = sha256_hex(format!("tx|{plan_id}|{address}|{amount_sats}").as_bytes());
    let receipt_sha256 = sha256_hex(format!("onchain|{plan_id}|{txid}").as_bytes());
    let paid_at_ms = Utc::now().timestamp_millis();

    {
        let mut balance = state.balance_sats.lock().await;
        *balance = balance.saturating_sub(total_sats);
    }

    let response = json!({
        "ok": true,
        "result": {
            "planId": plan_id,
            "walletId": "mock-wallet",
            "address": address,
            "amountSats": amount_sats,
            "confirmationSpeed": confirmation_speed,
            "feeSats": fee_sats,
            "totalSats": total_sats,
            "txid": txid,
            "paidAtMs": paid_at_ms,
            "receipt": {
                "receiptVersion": "openagents.lightning.onchain_send_receipt.v1",
                "receiptId": format!("losr_{}", &receipt_sha256[..24]),
                "canonicalJsonSha256": receipt_sha256,
            }
        }
    });

    let mut commits = state.onchain_commits_by_plan.lock().await;
    commits.insert(plan_id, response.clone());

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

async fn wait_for_pool_snapshot(
    client: &reqwest::Client,
    base_url: &str,
    pool_id: &str,
    timeout: Duration,
) -> Result<PoolSnapshotResponse> {
    let deadline = tokio::time::Instant::now() + timeout;
    let path = format!("/internal/v1/pools/{pool_id}/snapshots/latest");
    let url = format!("{base_url}{path}");
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow!("timeout waiting for {}", url));
        }

        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let json = resp.json::<Value>().await.unwrap_or(Value::Null);
                if status.is_success() {
                    let parsed: PoolSnapshotResponse = serde_json::from_value(json)
                        .with_context(|| format!("parse response from {}", path))?;
                    return Ok(parsed);
                }
            }
            Err(_) => {}
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
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
        return Err(anyhow!(
            "POST {} failed: http_{}: {}",
            path,
            status.as_u16(),
            json
        ));
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
        return Err(anyhow!(
            "GET {} failed: http_{}: {}",
            path,
            status.as_u16(),
            json
        ));
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
