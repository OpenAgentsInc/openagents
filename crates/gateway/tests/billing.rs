//! The billing surface's end-to-end contract: published plans, free
//! and checked-out subscriptions, signed provider events with dedup
//! and ordering tolerance, renewal and failure and cancellation,
//! refunds and disputes, top-ups, reconciliation, and the plan's gate
//! on the decision path — over real HTTP against an in-process
//! deployment and the sandbox provider's journal.
//!
//! Every test stands up its own directory and listeners; nothing
//! shares state but the webhook secret's name and value — a fixture
//! secret the environment carries, never a real credential.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Once};

use axum::http::StatusCode;
use serde_json::{Value, json};
use tenancy::billing::{Event, ModelAccess, Plan, Price};
use tenancy::{Registry, keys};

use gateway::billing::{sandbox, sign};
use gateway::config::{self, Config, Door, SCHEMA};
use gateway::money::{Money, Priced};
use gateway::serve::{self, ServeState};

use common::*;

/// The webhook secret every test deploys under — a fixture, not a
/// credential, and it never enters a log line or a file.
const SECRET: &str = "test-fixture-webhook-secret-not-a-real-key";
const SECRET_ENV: &str = "OPENAGENTS_BILLING_SECRET";

/// Set the webhook secret once for the whole test process — parallel
/// tests all read the same value, so one write stands for all of them.
fn secret() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        std::env::set_var(SECRET_ENV, SECRET);
    });
}

/// The free plan: no charge, a small allowance, the shared door only.
fn free_plan() -> Plan {
    Plan {
        id: "free".to_string(),
        version: "2026-10".to_string(),
        name: "Free".to_string(),
        price: Price {
            amount: 0,
            currency: "USD".to_string(),
            period_secs: 2_592_000,
        },
        allowance: 5_000_000,
        signup_credit: 1_000_000,
        seats: Some(3),
        models: ModelAccess::Listed(vec!["shared-kev".to_string()]),
        spend_limit: u64::MAX,
        credit_expiry_secs: None,
        topups_allowed: true,
    }
}

/// The paid plan: a monthly price, a larger allowance, every door.
fn pro_plan() -> Plan {
    Plan {
        id: "pro".to_string(),
        version: "2026-10".to_string(),
        name: "Pro".to_string(),
        price: Price {
            amount: 9_000_000,
            currency: "USD".to_string(),
            period_secs: 2_592_000,
        },
        allowance: 100_000_000,
        signup_credit: 0,
        seats: Some(10),
        models: ModelAccess::All,
        spend_limit: u64::MAX,
        credit_expiry_secs: None,
        topups_allowed: true,
    }
}

/// The billing block both plans sit in.
fn billing_config() -> config::Billing {
    config::Billing {
        plans: vec![free_plan(), pro_plan()],
        provider: "sandbox".to_string(),
        webhook_secret_env: SECRET_ENV.to_string(),
        checkout_ttl_secs: 86_400,
        webhook_skew_secs: 300,
    }
}

/// The accounts block: sign-up onto `acme`, no anonymous lane.
fn account_config() -> config::Accounts {
    config::Accounts {
        signup_tenant: Some("acme".to_string()),
        session_ttl_secs: 28_800,
        recovery_ttl_secs: 3_600,
        anonymous: None,
    }
}

/// The monetary block charging the fixture price on `shared-kev` — the
/// ledger billing's grants and clawbacks write.
fn money_config(ledger: &std::path::Path) -> Money {
    let priced = |capacity: &str| Priced {
        price: tenancy::money::Price {
            version: "synthetic-fixture-v1".to_string(),
            currency: "USD".to_string(),
            model: "kev-0.6b".to_string(),
            capacity: capacity.to_string(),
            policy: gateway::money::POLICY.to_string(),
            rates: [(
                tenancy::money::Resource::InputTokens,
                tenancy::money::Rate {
                    millionths: 10,
                    per_units: 1,
                },
            )]
            .into(),
        },
        maximum_usage: [(tenancy::money::Resource::InputTokens, 1_000)].into(),
    };
    Money {
        ledger: ledger.to_path_buf(),
        doors: [
            ("shared-kev".to_string(), priced("shared")),
            ("acme-kev".to_string(), priced("dedicated")),
        ]
        .into(),
    }
}

/// A deployed gateway with accounts, money, and billing configured.
struct Deployment {
    dir: tempfile::TempDir,
    address: String,
    _state: Arc<ServeState>,
}

async fn deploy() -> Deployment {
    secret();
    let (endpoint, _forwards) = backend(&artifact('b'), StatusCode::OK, answer(), 0).await;
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest(&artifact('b'), None);
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    for tenant in manifest.tenants.keys() {
        keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
    }
    let ledger = dir.path().join("money-ledger.jsonl");
    let doors = ["shared-kev", "acme-kev"]
        .into_iter()
        .map(|door| {
            (
                door.to_string(),
                Door {
                    endpoint: endpoint.clone(),
                    classify: None,
                    classify_item_concurrency: 1,
                    batching: None,
                },
            )
        })
        .collect();
    let state = ServeState::open(Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: true,
        accounts: Some(account_config()),
        billing: Some(billing_config()),
        skills: None,
        money: Some(money_config(&ledger)),
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        classify_timeout_ms: None,
        max_tenant_classify_in_flight: None,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_classify_inputs: 1024,
        max_classify_inputs_per_tenant: 1024,
        max_questions: 256,
        cors_origins: vec![],
        max_options: 4096,
        doors,
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    })
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        dir,
        address,
        _state: state,
    }
}

/// A decision request body.
fn call(model: &str) -> Value {
    json!({
        "model": model,
        "state": "A member's private text.",
        "questions": {
            "q1": {"type": "noul", "instructions": "Is this about routing?", "criteria": "yes/no"},
        },
    })
}

async fn exchange(request: reqwest::RequestBuilder) -> (StatusCode, Value) {
    let response = request.send().await.unwrap();
    let status = response.status();
    let body = response.json().await.unwrap_or_default();
    (status, body)
}

async fn post(
    deployment: &Deployment,
    path: &str,
    token: Option<&str>,
    body: &Value,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .post(format!("{}{path}", deployment.address))
        .json(body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    exchange(request).await
}

async fn get(deployment: &Deployment, path: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new().get(format!("{}{path}", deployment.address));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    exchange(request).await
}

/// A decision call under `token` naming `workspace`.
async fn decide(deployment: &Deployment, token: &str, workspace: &str, door: &str) -> StatusCode {
    exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/systemone", deployment.address))
            .json(&call(door))
            .bearer_auth(token)
            .header("x-workspace-id", workspace),
    )
    .await
    .0
}

/// A signed webhook delivery — what the provider posts. `journal`
/// first writes the provider-side record when `true`; delivering the
/// event without journaling simulates the live path, and journaling
/// without delivering is what reconciliation recovers.
async fn deliver(deployment: &Deployment, event: &Event, journal: bool) -> (StatusCode, Value) {
    if journal {
        sandbox::emit(deployment.dir.path(), event).unwrap();
    }
    let body = serde_json::to_vec(event).unwrap();
    let timestamp = unix_now();
    let signature = sign(SECRET, timestamp, &body);
    exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/billing/webhook", deployment.address))
            .header(
                "x-openagents-billing-signature",
                format!("t={timestamp},v1={signature}"),
            )
            .header("content-type", "application/json")
            .body(body),
    )
    .await
}

/// An event with just the fields a test names.
fn event(kind: &str, fields: &[(&str, Value)]) -> Event {
    let mut event = Event {
        provider: "sandbox".to_string(),
        id: format!("evt_{}", uuid()),
        kind: kind.to_string(),
        checkout: None,
        subscription: None,
        invoice: None,
        period: 0,
        amount: 0,
        currency: None,
        at_period_end: true,
        provider_ref: None,
        received: 0,
        applied: false,
        outcome: String::new(),
    };
    for (name, value) in fields {
        match *name {
            "id" => event.id = value.as_str().unwrap().to_string(),
            "checkout" => event.checkout = Some(value.as_str().unwrap().to_string()),
            "subscription" => event.subscription = Some(value.as_str().unwrap().to_string()),
            "invoice" => event.invoice = Some(value.as_str().unwrap().to_string()),
            "period" => event.period = value.as_u64().unwrap() as u32,
            "amount" => event.amount = value.as_u64().unwrap(),
            "currency" => event.currency = Some(value.as_str().unwrap().to_string()),
            "provider_ref" => event.provider_ref = Some(value.as_str().unwrap().to_string()),
            other => panic!("unknown field {other}"),
        }
    }
    event
}

/// A unique-enough id for a test event.
fn uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!(
        "{:016x}{:016x}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

/// Sign an account up; returns `(account, workspace, key)`.
async fn join(deployment: &Deployment, label: &str) -> (String, String, String) {
    let (status, body) = post(deployment, "/v1/accounts", None, &json!({"label": label})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        body["account"]["id"].as_str().unwrap().to_string(),
        body["workspace"]["id"].as_str().unwrap().to_string(),
        body["key_token"].as_str().unwrap().to_string(),
    )
}

/// The workspace's ledger balance, in millionths of the account currency.
async fn balance(deployment: &Deployment, workspace: &str, token: &str) -> u64 {
    let (status, body) = exchange(
        reqwest::Client::new()
            .get(format!("{}/v1/balance", deployment.address))
            .bearer_auth(token)
            .header("x-workspace-id", workspace),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["balance"]["available"].as_u64().unwrap()
}

/// Subscribe `workspace` to `free` — the standard onboarding step.
async fn subscribe_free(deployment: &Deployment, workspace: &str, token: &str) -> Value {
    let (status, body) = post(
        deployment,
        &format!("/v1/workspaces/{workspace}/billing/subscribe"),
        Some(token),
        &json!({"plan": "free"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

#[tokio::test]
async fn the_plan_catalog_is_published() {
    let deployment = deploy().await;
    let (status, body) = get(&deployment, "/v1/plans", None).await;
    assert_eq!(status, StatusCode::OK);
    let plans = body["plans"].as_array().unwrap();
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0]["id"], "free");
    assert_eq!(plans[0]["price"]["amount"], 0);
    assert_eq!(plans[1]["id"], "pro");
    assert_eq!(plans[1]["price"]["amount"], 9_000_000);
    assert_eq!(plans[1]["price"]["currency"], "USD");
}

#[tokio::test]
async fn a_free_subscription_grants_and_entitles() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    // Before any subscription the door refuses as unpaid.
    let status = decide(&deployment, &key, &workspace, "shared-kev").await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED);
    let body = subscribe_free(&deployment, &workspace, &key).await;
    assert_eq!(body["subscription"]["plan"], "free");
    assert_eq!(body["subscription"]["state"], "active");
    // Sign-up credit plus the period allowance stand in the ledger.
    let credited = balance(&deployment, &workspace, &key).await;
    assert_eq!(credited, 6_000_000);
    // The subscription gates the door now.
    let status = decide(&deployment, &key, &workspace, "shared-kev").await;
    assert_eq!(status, StatusCode::OK, "subscribed call refused");
    // A second subscribe refuses — one subscription per workspace.
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/subscribe"),
        Some(&key),
        &json!({"plan": "free"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "already_subscribed");
}

#[tokio::test]
async fn the_plan_gates_the_door() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    subscribe_free(&deployment, &workspace, &key).await;
    // `free` lists only `shared-kev` — `acme-kev` refuses.
    let status = decide(&deployment, &key, &workspace, "acme-kev").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_paid_plan_travels_through_checkout() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    // `pro` is paid — direct subscribe points at checkout.
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/subscribe"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "checkout_required");
    // Open the session.
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let checkout = body["checkout"]["id"].as_str().unwrap();
    // The browser's return displays pending — and grants nothing.
    let (status, page) = get(
        &deployment,
        &format!("/v1/billing/sessions/{checkout}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["checkout"]["state"], "pending");
    // No money account exists yet — the browser's page granted nothing.
    let (status, _body) = exchange(
        reqwest::Client::new()
            .get(format!("{}/v1/balance", deployment.address))
            .bearer_auth(&key)
            .header("x-workspace-id", &workspace),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // The verified event completes it.
    let (status, body) = deliver(
        &deployment,
        &event(
            "checkout-completed",
            &[
                ("checkout", json!(checkout)),
                (
                    "provider_ref",
                    json!(body["checkout"]["provider_ref"].clone()),
                ),
            ],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "applied");
    let (status, page) = get(
        &deployment,
        &format!("/v1/billing/sessions/{checkout}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["checkout"]["state"], "complete");
    // The period allowance stands; the door admits every model now.
    let credited = balance(&deployment, &workspace, &key).await;
    assert_eq!(credited, 100_000_000);
    let (status, dbg) = decide_body(&deployment, &key, &workspace, "acme-kev").await;
    assert_eq!(status, StatusCode::OK, "{dbg}");
}

/// Same as `decide` but returns the refusal body for debugging.
async fn decide_body(
    deployment: &Deployment,
    token: &str,
    workspace: &str,
    door: &str,
) -> (StatusCode, Value) {
    exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/systemone", deployment.address))
            .json(&call(door))
            .bearer_auth(token)
            .header("x-workspace-id", workspace),
    )
    .await
}

#[tokio::test]
async fn a_tampered_signature_is_refused() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let checkout = body["checkout"]["id"].as_str().unwrap();
    let forged = event("checkout-completed", &[("checkout", json!(checkout))]);
    let body_bytes = serde_json::to_vec(&forged).unwrap();
    // A signature over a different body does not verify.
    let (status, _) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/billing/webhook", deployment.address))
            .header(
                "x-openagents-billing-signature",
                format!(
                    "t={},v1={}",
                    unix_now(),
                    sign(SECRET, unix_now(), b"forged")
                ),
            )
            .header("content-type", "application/json")
            .body(body_bytes),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // A stale timestamp refuses too.
    let stale = unix_now() - 10_000;
    let (status, _) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/billing/webhook", deployment.address))
            .header(
                "x-openagents-billing-signature",
                format!(
                    "t={stale},v1={}",
                    sign(SECRET, stale, &serde_json::to_vec(&forged).unwrap())
                ),
            )
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&forged).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Nothing was granted.
    let (status, _body) = exchange(
        reqwest::Client::new()
            .get(format!("{}/v1/balance", deployment.address))
            .bearer_auth(&key)
            .header("x-workspace-id", &workspace),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn duplicate_events_grant_once() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    let (_status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    let checkout = body["checkout"]["id"].as_str().unwrap().to_string();
    let reference = body["checkout"]["provider_ref"]
        .as_str()
        .unwrap()
        .to_string();
    let completed = event(
        "checkout-completed",
        &[
            ("id", json!("evt_once")),
            ("checkout", json!(checkout)),
            ("provider_ref", json!(reference)),
        ],
    );
    let (status, body) = deliver(&deployment, &completed, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "applied");
    // The same event id again acknowledges as a duplicate — no second grant.
    let (status, body) = deliver(&deployment, &completed, false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "duplicate");
    // A different event id claiming the same transition supersedes.
    let replay = event(
        "checkout-completed",
        &[
            ("checkout", json!(checkout)),
            ("provider_ref", json!(reference)),
        ],
    );
    let (status, body) = deliver(&deployment, &replay, true).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["outcome"].as_str().unwrap().starts_with("superseded"));
    assert_eq!(balance(&deployment, &workspace, &key).await, 100_000_000);
}

#[tokio::test]
async fn out_of_order_events_settle_through_reconcile() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    let (_status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    let checkout = body["checkout"]["id"].as_str().unwrap().to_string();
    let reference = body["checkout"]["provider_ref"]
        .as_str()
        .unwrap()
        .to_string();
    // The renewal arrives before the subscription exists — ignored,
    // journaled, and recoverable.
    let early = event(
        "invoice-paid",
        &[
            ("subscription", json!("sub_late")),
            ("invoice", json!("inv_p2")),
            ("period", json!(2)),
            ("amount", json!(9_000_000)),
        ],
    );
    let (status, body) = deliver(&deployment, &early, true).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["outcome"].as_str().unwrap().starts_with("ignored"));
    // The checkout completes — the subscription stands.
    let (status, _) = deliver(
        &deployment,
        &event(
            "checkout-completed",
            &[
                ("checkout", json!(checkout)),
                ("provider_ref", json!(reference)),
            ],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let subscription = {
        let (status, body) = get(
            &deployment,
            &format!("/v1/workspaces/{workspace}/billing"),
            Some(&key),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        body["subscription"]["id"].as_str().unwrap().to_string()
    };
    // The correctly-referenced renewal pays — period 2 grants once.
    let renewal = event(
        "invoice-paid",
        &[
            ("subscription", json!(subscription)),
            ("invoice", json!("inv_p2")),
            ("period", json!(2)),
            ("amount", json!(9_000_000)),
        ],
    );
    let (status, body) = deliver(&deployment, &renewal, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "applied");
    assert_eq!(balance(&deployment, &workspace, &key).await, 200_000_000);
    // Reconciliation is a no-op now — everything stands.
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/reconcile"),
        Some(&key),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(balance(&deployment, &workspace, &key).await, 200_000_000);
}

#[tokio::test]
async fn a_lost_delivery_is_reconciled() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    let (_status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    let checkout = body["checkout"]["id"].as_str().unwrap().to_string();
    let reference = body["checkout"]["provider_ref"]
        .as_str()
        .unwrap()
        .to_string();
    // The provider processed the payment — journaled — but the webhook
    // never arrived.
    sandbox::emit(
        deployment.dir.path(),
        &event(
            "checkout-completed",
            &[
                ("checkout", json!(checkout)),
                ("provider_ref", json!(reference)),
            ],
        ),
    )
    .unwrap();
    // The browser still sees pending — nothing moved.
    let (status, page) = get(
        &deployment,
        &format!("/v1/billing/sessions/{checkout}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["checkout"]["state"], "pending");
    // Reconciliation pulls the delivery out of the provider journal.
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/reconcile"),
        Some(&key),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body["delivered"].as_array().unwrap().is_empty());
    let (status, page) = get(
        &deployment,
        &format!("/v1/billing/sessions/{checkout}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["checkout"]["state"], "complete");
    assert_eq!(balance(&deployment, &workspace, &key).await, 100_000_000);
}

#[tokio::test]
async fn a_failed_payment_then_recovery() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    subscribe_free(&deployment, &workspace, &key).await;
    let subscription = {
        let (_, body) = get(
            &deployment,
            &format!("/v1/workspaces/{workspace}/billing"),
            Some(&key),
        )
        .await;
        body["subscription"]["id"].as_str().unwrap().to_string()
    };
    // The renewal charge fails — the subscription goes past-due but
    // keeps its entitlement through the grace window.
    let (status, body) = deliver(
        &deployment,
        &event(
            "invoice-failed",
            &[
                ("subscription", json!(subscription)),
                ("invoice", json!("inv_fail")),
                ("period", json!(2)),
            ],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "applied");
    let (_, body) = get(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing"),
        Some(&key),
    )
    .await;
    assert_eq!(body["subscription"]["state"], "past-due");
    let status = decide(&deployment, &key, &workspace, "shared-kev").await;
    assert_eq!(status, StatusCode::OK, "past-due should still serve");
    // The retry pays — the subscription recovers inside the period.
    let (status, body) = deliver(
        &deployment,
        &event(
            "invoice-paid",
            &[
                ("subscription", json!(subscription)),
                ("invoice", json!("inv_fail")),
                ("period", json!(2)),
                ("amount", json!(9_000_000)),
            ],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "applied");
    let (_, body) = get(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing"),
        Some(&key),
    )
    .await;
    assert_eq!(body["subscription"]["state"], "active");
}

#[tokio::test]
async fn a_scheduled_plan_change_lands_at_renewal() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    subscribe_free(&deployment, &workspace, &key).await;
    // Schedule the upgrade — it stands pending until the next paid renewal.
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/plan"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["subscription"]["pending_plan"], "pro");
    // Still on `free` — `acme-kev` refuses.
    let status = decide(&deployment, &key, &workspace, "acme-kev").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let subscription = {
        let (_, body) = get(
            &deployment,
            &format!("/v1/workspaces/{workspace}/billing"),
            Some(&key),
        )
        .await;
        body["subscription"]["id"].as_str().unwrap().to_string()
    };
    // The renewal pays — the pending change lands with the new period.
    let (status, _body) = deliver(
        &deployment,
        &event(
            "invoice-paid",
            &[
                ("subscription", json!(subscription)),
                ("invoice", json!("inv_p2")),
                ("period", json!(2)),
                ("amount", json!(9_000_000)),
            ],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = get(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing"),
        Some(&key),
    )
    .await;
    assert_eq!(body["subscription"]["plan"], "pro");
    assert!(body["subscription"]["pending_plan"].is_null());
    let status = decide(&deployment, &key, &workspace, "acme-kev").await;
    assert_eq!(status, StatusCode::OK, "pro opens every door");
}

#[tokio::test]
async fn a_cancel_keeps_the_period_then_expires() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    subscribe_free(&deployment, &workspace, &key).await;
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/cancel"),
        Some(&key),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["subscription"]["state"], "cancelled");
    // Entitled through the period's end.
    let status = decide(&deployment, &key, &workspace, "shared-kev").await;
    assert_eq!(status, StatusCode::OK, "cancelled still serves the period");
    // A provider cancel event lands the same way.
    let subscription = {
        let (_, body) = get(
            &deployment,
            &format!("/v1/workspaces/{workspace}/billing"),
            Some(&key),
        )
        .await;
        body["subscription"]["id"].as_str().unwrap().to_string()
    };
    let (status, body) = deliver(
        &deployment,
        &event(
            "subscription-cancelled",
            &[("subscription", json!(subscription))],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["outcome"].as_str().unwrap().starts_with("superseded"));
}

#[tokio::test]
async fn a_refund_and_a_dispute_claw_back() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    let (_status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    let checkout = body["checkout"]["id"].as_str().unwrap().to_string();
    let reference = body["checkout"]["provider_ref"]
        .as_str()
        .unwrap()
        .to_string();
    deliver(
        &deployment,
        &event(
            "checkout-completed",
            &[
                ("checkout", json!(checkout)),
                ("provider_ref", json!(reference)),
            ],
        ),
        true,
    )
    .await;
    let subscription = {
        let (_, body) = get(
            &deployment,
            &format!("/v1/workspaces/{workspace}/billing"),
            Some(&key),
        )
        .await;
        body["subscription"]["id"].as_str().unwrap().to_string()
    };
    let (status, _) = deliver(
        &deployment,
        &event(
            "invoice-paid",
            &[
                ("subscription", json!(subscription)),
                ("invoice", json!("inv_p2")),
                ("period", json!(2)),
                ("amount", json!(9_000_000)),
            ],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(balance(&deployment, &workspace, &key).await, 200_000_000);
    // The refund claws back its amount against available credit.
    let (status, body) = deliver(
        &deployment,
        &event(
            "charge-refunded",
            &[("invoice", json!("inv_p2")), ("amount", json!(9_000_000))],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "applied");
    assert_eq!(balance(&deployment, &workspace, &key).await, 191_000_000);
    // A second refund of the same invoice supersedes.
    let (status, body) = deliver(
        &deployment,
        &event(
            "charge-refunded",
            &[("invoice", json!("inv_p2")), ("amount", json!(9_000_000))],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["outcome"].as_str().unwrap().starts_with("superseded"));
    // A dispute on a paid invoice debits as well.
    let (status, _) = deliver(
        &deployment,
        &event(
            "invoice-paid",
            &[
                ("subscription", json!(subscription)),
                ("invoice", json!("inv_p3")),
                ("period", json!(3)),
                ("amount", json!(9_000_000)),
            ],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = deliver(
        &deployment,
        &event(
            "charge-disputed",
            &[("invoice", json!("inv_p3")), ("amount", json!(9_000_000))],
        ),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "applied");
    assert_eq!(balance(&deployment, &workspace, &key).await, 282_000_000);
}

#[tokio::test]
async fn a_top_up_checkout_credits_the_account() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    // Top-ups ride on a subscription — without one, the route refuses.
    let (status, _body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/checkout"),
        Some(&key),
        &json!({"top_up": {"amount": 5_000_000}}),
    )
    .await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED);
    subscribe_free(&deployment, &workspace, &key).await;
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/billing/checkout"),
        Some(&key),
        &json!({"top_up": {"amount": 5_000_000}}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let checkout = body["checkout"]["id"].as_str().unwrap().to_string();
    let (status, body) = deliver(
        &deployment,
        &event("checkout-completed", &[("checkout", json!(checkout))]),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // 6,000,000 subscription credit + 5,000,000 top-up.
    assert_eq!(balance(&deployment, &workspace, &key).await, 11_000_000);
}

#[tokio::test]
async fn billing_management_is_owner_only() {
    let deployment = deploy().await;
    let (_owner, _personal, owner_key) = join(&deployment, "Owner").await;
    let (_member, _ws2, member_key) = join(&deployment, "Member").await;
    // The owner opens an organization workspace and invites the member in.
    let (status, body) = post(
        &deployment,
        "/v1/workspaces",
        Some(&owner_key),
        &json!({"name": "team"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let workspace = body["workspace"]["id"].as_str().unwrap().to_string();
    let (status, invite) = post(
        &deployment,
        &format!("/v1/workspaces/{workspace}/invitations"),
        Some(&owner_key),
        &json!({"role": "member"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{invite}");
    let (status, _) = post(
        &deployment,
        "/v1/invitations/accept",
        Some(&member_key),
        &json!({"token": invite["token"].as_str().unwrap()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // The member cannot read or move the workspace's billing.
    for (method, path) in [
        ("GET", format!("/v1/workspaces/{workspace}/billing")),
        (
            "POST",
            format!("/v1/workspaces/{workspace}/billing/subscribe"),
        ),
        ("POST", format!("/v1/workspaces/{workspace}/billing/cancel")),
    ] {
        let request = reqwest::Client::new()
            .request(
                method.parse::<reqwest::Method>().unwrap(),
                format!("{}{path}", deployment.address),
            )
            .bearer_auth(&member_key)
            .json(&json!({"plan": "free"}));
        let (status, _) = exchange(request).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {path} admitted a member"
        );
    }
}

#[tokio::test]
async fn an_unknown_plan_is_not_sold() {
    let deployment = deploy().await;
    let (_account, workspace, key) = join(&deployment, "Ada").await;
    for path in ["subscribe", "checkout", "plan"] {
        let (status, body) = post(
            &deployment,
            &format!("/v1/workspaces/{workspace}/billing/{path}"),
            Some(&key),
            &json!({"plan": "platinum"}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        assert_eq!(body["error"]["code"], "unknown_plan");
    }
}

#[tokio::test]
async fn billing_requires_accounts_and_money() {
    // The config check refuses billing without its dependencies.
    let mut config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: std::path::PathBuf::from("/tmp/unused"),
        require_workspace_membership: false,
        accounts: None,
        billing: Some(billing_config()),
        skills: None,
        money: None,
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        classify_timeout_ms: None,
        max_tenant_classify_in_flight: None,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_classify_inputs: 1024,
        max_classify_inputs_per_tenant: 1024,
        max_questions: 256,
        cors_origins: vec![],
        max_options: 4096,
        doors: BTreeMap::new(),
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    };
    let path = std::path::Path::new("gateway.json");
    assert!(config.check(path).is_err());
    // A plan naming a door with no backend refuses too.
    config.accounts = Some(account_config());
    config.require_workspace_membership = true;
    let dir = tempfile::tempdir().unwrap();
    let mut money = money_config(&dir.path().join("money.jsonl"));
    config.money = Some(std::mem::replace(
        &mut money,
        money_config(&dir.path().join("m.jsonl")),
    ));
    let mut billing = billing_config();
    billing.plans[0].models = ModelAccess::Listed(vec!["unconfigured".to_string()]);
    config.billing = Some(billing);
    assert!(config.check(path).is_err());
}
