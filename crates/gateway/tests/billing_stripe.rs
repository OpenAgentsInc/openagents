//! Stripe subscriptions end to end (#11072), in Stripe test mode against a
//! loopback stand-in for Stripe's API: checkout opens a subscription
//! Checkout Session, signed events start, renew, fail, and end the
//! subscription, paid months reach the environment meter (hours reset on
//! renewal), and the billing portal opens. No real key, no real charge.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Once};

use axum::extract::State as ApiState;
use axum::http::{StatusCode, Uri};
use axum::{Form, Json, Router};
use serde_json::{Value, json};
use tenancy::billing::Plan;
use tenancy::{Registry, keys};

use gateway::billing::sign;
use gateway::config::{self, Config, Door, SCHEMA};
use gateway::money::{Money, Priced};
use gateway::serve::{self, ServeState};
use gateway::subscriptions;
use retail_cloud::environment::{self, Standing};
use retail_cloud::journal::Journal;

use common::*;

const KEY_ENV: &str = "OA_STRIPE_SUBSCRIPTIONS_TEST_KEY";
const SIGNING_ENV: &str = "OA_STRIPE_SUBSCRIPTIONS_TEST_SIGNING";
/// Fixtures, not credentials.
const SIGNING: &str = "whsec_fixture_not_a_real_signing_secret";
const DAY: i64 = 86_400;

fn secrets() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        std::env::set_var(KEY_ENV, "sk_test_fixture_not_a_real_key_0000");
        std::env::set_var(SIGNING_ENV, SIGNING);
    });
}

fn pro() -> Plan {
    serde_json::from_str(include_str!("../fixtures/plans/pro.json")).unwrap()
}

/// What the stand-in Stripe saw.
#[derive(Default)]
struct Fake {
    creates: Vec<(String, BTreeMap<String, String>)>,
    down: bool,
    /// Charges read back, by id.
    reads: Vec<String>,
}

/// Stripe's `GET /v1/charges/{id}`: a charge that names its customer and
/// amount but no invoice (the shape a dispute's charge has).
async fn fake_charge(
    ApiState(fake): ApiState<Arc<Mutex<Fake>>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let mut fake = fake.lock().unwrap();
    if fake.down {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    fake.reads.push(id.clone());
    Json(json!({"object": "charge", "id": id, "livemode": false,
        "customer": "cus_test_1", "amount": 2000, "invoice": null}))
    .into_response()
}

async fn fake_post(
    ApiState(fake): ApiState<Arc<Mutex<Fake>>>,
    uri: Uri,
    Form(fields): Form<BTreeMap<String, String>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let mut fake = fake.lock().unwrap();
    if fake.down {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    fake.creates.push((uri.path().to_string(), fields.clone()));
    let n = fake.creates.len();
    match uri.path() {
        "/v1/checkout/sessions" => Json(json!({
            "object": "checkout.session",
            "id": format!("cs_test_{n}"),
            "livemode": false,
            "mode": "subscription",
            "client_reference_id": fields["client_reference_id"],
            "url": format!("https://checkout.stripe.com/c/pay/cs_test_{n}"),
        }))
        .into_response(),
        "/v1/billing_portal/sessions" => Json(json!({
            "object": "billing_portal.session",
            "id": format!("bps_{n}"),
            "livemode": false,
            "customer": fields["customer"],
            "url": format!("https://billing.stripe.com/p/session/test_{n}"),
        }))
        .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

struct Deployment {
    address: String,
    meter: std::path::PathBuf,
    fake: Arc<Mutex<Fake>>,
    _dir: tempfile::TempDir,
    _state: Arc<ServeState>,
}

async fn deploy() -> Deployment {
    secrets();
    let fake = Arc::new(Mutex::new(Fake::default()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let stripe_origin = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route("/v1/charges/{id}", axum::routing::get(fake_charge))
                .fallback(axum::routing::post(fake_post))
                .with_state(fake.clone()),
        )
        .into_future(),
    );
    let (endpoint, _forwards) = backend(&artifact('b'), StatusCode::OK, answer(), 0).await;
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest(&artifact('b'), None);
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    for tenant in manifest.tenants.keys() {
        keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
    }
    let meter = dir.path().join("environment-meter.sqlite");
    let price = tenancy::money::Price {
        version: "synthetic-fixture-v1".to_string(),
        currency: "USD".to_string(),
        model: "kev-0.6b".to_string(),
        capacity: "shared".to_string(),
        policy: gateway::money::POLICY.to_string(),
        rates: [(
            tenancy::money::Resource::InputTokens,
            tenancy::money::Rate {
                millionths: 10,
                per_units: 1,
            },
        )]
        .into(),
    };
    let state = ServeState::open(Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: true,
        team_policy: None,
        team_reports: None,
        accounts: Some(config::Accounts {
            signup_tenant: Some("acme".to_string()),
            open_signup: true,
            operator_signup_token_env: None,
            session_ttl_secs: 28_800,
            recovery_ttl_secs: 3_600,
            github: None,
            github_app: None,
            anonymous: None,
        }),
        billing: Some(config::Billing {
            prepaid: None,
            plans: vec![pro()],
            provider: "stripe".to_string(),
            webhook_secret_env: "OPENAGENTS_BILLING_SECRET".to_string(),
            checkout_ttl_secs: 86_400,
            webhook_skew_secs: 300,
            stripe: Some(subscriptions::Config {
                live: false,
                secret_key_env: KEY_ENV.to_string(),
                webhook_secret_envs: vec![SIGNING_ENV.to_string()],
                prices: [("pro".to_string(), "price_test_pro".to_string())].into(),
                success_url: "https://openagents.test/settings?plan=started#settings-plan"
                    .to_string(),
                return_url: "https://openagents.test/settings#settings-plan".to_string(),
                environment_meter: Some(meter.clone()),
                api_origin: Some(stripe_origin),
            }),
        }),
        funding: None,
        inference: None,
        earnings: None,
        commercial: None,
        skills: None,
        money: Some(Money {
            hierarchical_budgets: false,
            ledger: dir.path().join("money-ledger.jsonl"),
            doors: [(
                "shared-kev".to_string(),
                Priced {
                    offer: None,
                    price,
                    maximum_usage: [(tenancy::money::Resource::InputTokens, 1_000)].into(),
                },
            )]
            .into(),
        }),
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
        doors: [(
            "shared-kev".to_string(),
            Door {
                endpoint,
                classify: None,
                classify_item_concurrency: 1,
                batching: None,
            },
        )]
        .into(),
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    })
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        address,
        meter,
        fake,
        _dir: dir,
        _state: state,
    }
}

async fn exchange(request: reqwest::RequestBuilder) -> (StatusCode, Value) {
    let response = request.send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or_default())
}

async fn post(
    d: &Deployment,
    path: &str,
    token: Option<&str>,
    body: &Value,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .post(format!("{}{path}", d.address))
        .json(body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    exchange(request).await
}

/// A Stripe event, signed the way Stripe signs it.
async fn deliver(d: &Deployment, event: &Value, secret: &str) -> (StatusCode, Value) {
    let body = serde_json::to_vec(event).unwrap();
    let t = unix_now();
    exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/billing/webhook", d.address))
            .header(
                "stripe-signature",
                format!("t={t},v1={}", sign(secret, t, &body)),
            )
            .header("content-type", "application/json")
            .body(body),
    )
    .await
}

fn event(id: &str, kind: &str, object: Value) -> Value {
    json!({"object": "event", "id": id, "type": kind, "livemode": false,
        "api_version": "2025-09-30.clover", "created": unix_now(), "data": {"object": object}})
}

fn metadata(account: &str, workspace: &str) -> Value {
    json!({"oa_account": account, "oa_workspace": workspace, "oa_plan": "pro", "oa_checkout": "x"})
}

/// An invoice in the newer API shape (subscription under `parent`).
fn invoice(id: &str, reason: &str, start: i64, meta: &Value) -> Value {
    json!({"object": "invoice", "id": id, "billing_reason": reason,
        "amount_paid": 2000, "amount_due": 2000, "currency": "usd", "customer": "cus_test_1",
        "parent": {"subscription_details": {"subscription": "sub_test_1", "metadata": meta}},
        "lines": {"data": [{"period": {"start": start, "end": start + 30 * DAY}}]}})
}

fn standing(d: &Deployment, account: &str, at: i64) -> Standing {
    let j = Journal::open(&d.meter).unwrap();
    environment::standing(&j, account, at).unwrap()
}

#[tokio::test]
async fn a_subscription_starts_renews_fails_and_ends_through_stripe() {
    let d = deploy().await;
    let (status, body) = post(&d, "/v1/accounts", None, &json!({"label": "Ada"})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let account = body["account"]["id"].as_str().unwrap().to_string();
    let workspace = body["workspace"]["id"].as_str().unwrap().to_string();
    let key = body["key_token"].as_str().unwrap().to_string();
    let billing = format!("/v1/workspaces/{workspace}/billing");

    // Checkout: Stripe is down, so nothing stays pending; then it opens.
    d.fake.lock().unwrap().down = true;
    let (status, body) = post(
        &d,
        &format!("{billing}/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{body}");
    assert_eq!(body["error"]["code"], "checkout_unavailable");
    d.fake.lock().unwrap().down = false;
    let (status, body) = post(
        &d,
        &format!("{billing}/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["url"], "https://checkout.stripe.com/c/pay/cs_test_1");
    let checkout = body["checkout"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["checkout"]["provider_ref"], "cs_test_1");
    {
        let fake = d.fake.lock().unwrap();
        let (path, fields) = &fake.creates[0];
        assert_eq!(path, "/v1/checkout/sessions");
        assert_eq!(fields["mode"], "subscription");
        assert_eq!(fields["line_items[0][price]"], "price_test_pro");
        assert_eq!(fields["client_reference_id"], checkout);
        assert_eq!(fields["subscription_data[metadata][oa_account]"], account);
        assert_eq!(
            fields["subscription_data[metadata][oa_workspace]"],
            workspace
        );
        assert_eq!(
            fields["success_url"],
            "https://openagents.test/settings?plan=started#settings-plan"
        );
    }
    // Top-ups don't go through plan checkout.
    let (status, _) = post(
        &d,
        &format!("{billing}/checkout"),
        Some(&key),
        &json!({"top_up": {"amount": 1_000_000}}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // A forged or other-mode event is refused.
    let now = unix_now() as i64;
    let meta = metadata(&account, &workspace);
    let first = event(
        "evt_inv1",
        "invoice.paid",
        invoice("in_1", "subscription_create", now, &meta),
    );
    let (status, _) = deliver(&d, &first, "whsec_wrong_secret_wrong_secret").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let mut live = first.clone();
    live["livemode"] = json!(true);
    let (status, _) = deliver(&d, &live, SIGNING).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // The first invoice can arrive before the checkout completes: the paid
    // month is recorded, and the book asks Stripe to send it again.
    assert_eq!(standing(&d, &account, now), Standing::Never);
    let (status, body) = deliver(&d, &first, SIGNING).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(matches!(
        standing(&d, &account, now),
        Standing::Active { .. }
    ));
    let j = Journal::open(&d.meter).unwrap();
    assert_eq!(
        environment::credits_account(&j, &account).unwrap(),
        Some(workspace.clone())
    );
    drop(j);

    let completed = event(
        "evt_cs1",
        "checkout.session.completed",
        json!({"object": "checkout.session", "id": "cs_test_1", "mode": "subscription",
            "payment_status": "paid", "client_reference_id": checkout,
            "metadata": {"oa_checkout": checkout}, "subscription": "sub_test_1", "customer": "cus_test_1"}),
    );
    let (status, body) = deliver(&d, &completed, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "applied");
    let (status, body) = deliver(&d, &completed, SIGNING).await;
    assert_eq!(
        (status, body["outcome"].clone()),
        (StatusCode::OK, json!("duplicate"))
    );
    // Stripe's retry of the first invoice now lands, changing nothing.
    let retry = event(
        "evt_inv1b",
        "invoice.paid",
        invoice("in_1", "subscription_create", now, &meta),
    );
    let (status, body) = deliver(&d, &retry, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, view) = exchange(
        reqwest::Client::new()
            .get(format!("{}{billing}", d.address))
            .bearer_auth(&key),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["subscription"]["plan"], "pro");
    assert_eq!(view["subscription"]["state"], "active");
    assert_eq!(view["subscription"]["period"], 1);
    assert_eq!(view["subscription"]["provider_ref"], "sub_test_1");
    assert_eq!(view["subscription"]["customer"], "cus_test_1");

    // Renewal: the next month is paid, so its hours start from 100.
    let next = now + 30 * DAY;
    let renewed = event(
        "evt_inv2",
        "invoice.paid",
        invoice("in_2", "subscription_cycle", next, &meta),
    );
    let (status, body) = deliver(&d, &renewed, SIGNING).await;
    assert_eq!(
        (status, body["outcome"].clone()),
        (StatusCode::OK, json!("applied")),
        "{body}"
    );
    match standing(&d, &account, next + 1) {
        Standing::Active { period } => {
            assert_eq!((period.start, period.end), (next, next + 30 * DAY))
        }
        other => panic!("{other:?}"),
    }
    let mut plan = environment::plan();
    plan.status = environment::PlanStatus::Published;
    let j = Journal::open(&d.meter).unwrap();
    let budget = environment::budget(&j, &plan, &account, next + 1).unwrap();
    assert_eq!(budget.included_left_seconds, 100 * 3600);
    drop(j);

    // A failed renewal: past due, and no new month is recorded.
    let later = next + 30 * DAY;
    let failed = event(
        "evt_inv3",
        "invoice.payment_failed",
        invoice("in_3", "subscription_cycle", later, &meta),
    );
    let (status, body) = deliver(&d, &failed, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, view) = exchange(
        reqwest::Client::new()
            .get(format!("{}{billing}", d.address))
            .bearer_auth(&key),
    )
    .await;
    assert_eq!(view["subscription"]["state"], "past-due");
    assert_eq!(
        standing(&d, &account, later + 1),
        Standing::Ended { at: later }
    );

    // The billing page opens for the customer; cancelling happens there.
    let (status, body) = post(&d, &format!("{billing}/portal"), Some(&key), &json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["url"]
            .as_str()
            .unwrap()
            .starts_with("https://billing.stripe.com/")
    );
    {
        let fake = d.fake.lock().unwrap();
        let (path, fields) = fake.creates.last().unwrap();
        assert_eq!(path, "/v1/billing_portal/sessions");
        assert_eq!(fields["customer"], "cus_test_1");
    }
    let (status, body) = post(&d, &format!("{billing}/cancel"), Some(&key), &json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "cancel_in_portal");

    // Stripe ends the subscription now: the month ends with it.
    let ended_at = next + 5 * DAY;
    let deleted = event(
        "evt_sub_del",
        "customer.subscription.deleted",
        json!({"object": "subscription", "id": "sub_test_1", "status": "canceled",
            "ended_at": ended_at, "metadata": meta}),
    );
    let (status, body) = deliver(&d, &deleted, SIGNING).await;
    assert_eq!(
        (status, body["outcome"].clone()),
        (StatusCode::OK, json!("applied")),
        "{body}"
    );
    assert_eq!(
        standing(&d, &account, ended_at + 1),
        Standing::Ended { at: ended_at }
    );
    let (_, view) = exchange(
        reqwest::Client::new()
            .get(format!("{}{billing}", d.address))
            .bearer_auth(&key),
    )
    .await;
    // Ended now: cancelled, and expired as soon as the book sweeps.
    assert!(
        matches!(
            view["subscription"]["state"].as_str(),
            Some("cancelled" | "expired")
        ),
        "{view}"
    );

    // Unused event types are acknowledged and ignored.
    let other = event(
        "evt_other",
        "customer.updated",
        json!({"object": "customer", "id": "cus_test_1"}),
    );
    let (status, body) = deliver(&d, &other, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[test]
fn stripe_settings_are_checked() {
    let plans = vec![pro()];
    let good = subscriptions::Config {
        live: false,
        secret_key_env: KEY_ENV.into(),
        webhook_secret_envs: vec![SIGNING_ENV.into()],
        prices: [("pro".to_string(), "price_1".to_string())].into(),
        success_url: "https://openagents.test/settings".into(),
        return_url: "https://openagents.test/settings".into(),
        environment_meter: None,
        api_origin: None,
    };
    good.check(&plans).unwrap();
    let mut bad = good.clone();
    bad.prices.clear();
    assert!(bad.check(&plans).is_err(), "a paid plan needs its price");
    let mut bad = good.clone();
    bad.secret_key_env = "sk_test_inline_secret".into();
    assert!(bad.check(&plans).is_err(), "names, never secrets");
    let mut bad = good.clone();
    bad.live = true;
    bad.api_origin = Some("http://127.0.0.1:9/".into());
    assert!(bad.check(&plans).is_err(), "no stand-in in live mode");
    let mut bad = good;
    bad.success_url = "http://openagents.test/settings".into();
    assert!(bad.check(&plans).is_err());
}

// ---------------------------------------------------------------------------
// Refunds and disputes (#11074).

struct Subscriber {
    account: String,
    key: String,
    billing: String,
    meta: Value,
    now: i64,
}

/// An invoice that carries the references of the payment that settled it.
fn paid_invoice(id: &str, reason: &str, start: i64, meta: &Value, charge: &str) -> Value {
    let mut value = invoice(id, reason, start, meta);
    value["charge"] = json!(charge);
    value["payment_intent"] = json!(format!("pi_{charge}"));
    value
}

/// Sign up, check out, and pay the first month through signed events.
async fn subscriber(d: &Deployment, charge: Option<&str>) -> Subscriber {
    let (status, body) = post(d, "/v1/accounts", None, &json!({"label": "Ada"})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let account = body["account"]["id"].as_str().unwrap().to_string();
    let workspace = body["workspace"]["id"].as_str().unwrap().to_string();
    let key = body["key_token"].as_str().unwrap().to_string();
    let billing = format!("/v1/workspaces/{workspace}/billing");
    let (status, body) = post(
        d,
        &format!("{billing}/checkout"),
        Some(&key),
        &json!({"plan": "pro"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let checkout = body["checkout"]["id"].as_str().unwrap().to_string();
    let completed = event(
        "evt_cs1",
        "checkout.session.completed",
        json!({"object": "checkout.session", "id": "cs_test_1", "mode": "subscription",
            "payment_status": "paid", "client_reference_id": checkout,
            "metadata": {"oa_checkout": checkout}, "subscription": "sub_test_1", "customer": "cus_test_1"}),
    );
    let (status, body) = deliver(d, &completed, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let now = unix_now() as i64;
    let meta = metadata(&account, &workspace);
    let object = match charge {
        Some(charge) => paid_invoice("in_1", "subscription_create", now, &meta, charge),
        None => invoice("in_1", "subscription_create", now, &meta),
    };
    let (status, body) = deliver(d, &event("evt_inv1", "invoice.paid", object), SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    Subscriber {
        account,
        key,
        billing,
        meta,
        now,
    }
}

async fn billing_view(d: &Deployment, s: &Subscriber) -> Value {
    let (status, view) = exchange(
        reqwest::Client::new()
            .get(format!("{}{}", d.address, s.billing))
            .bearer_auth(&s.key),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    view
}

fn notice(d: &Deployment, account: &str) -> Option<environment::Notice> {
    let j = Journal::open(&d.meter).unwrap();
    environment::notice(&j, account).unwrap()
}

fn dispute(id: &str, charge: &str, status: &str) -> Value {
    json!({"object": "dispute", "id": id, "charge": charge, "payment_intent": format!("pi_{charge}"),
        "amount": 2000, "currency": "usd", "status": status})
}

fn charge(id: &str, amount: u64, refunded: u64) -> Value {
    json!({"object": "charge", "id": id, "payment_intent": format!("pi_{id}"), "amount": amount,
        "amount_refunded": refunded, "refunded": refunded >= amount, "customer": "cus_test_1"})
}

#[tokio::test]
async fn a_dispute_pauses_new_months_until_it_closes() {
    let d = deploy().await;
    let s = subscriber(&d, Some("ch_1")).await;
    let next = s.now + 30 * DAY;

    // The bank opens a dispute: at risk, the paid month runs on, and
    // Settings says so.
    let opened = event(
        "evt_dp1",
        "charge.dispute.created",
        dispute("dp_1", "ch_1", "needs_response"),
    );
    let (status, body) = deliver(&d, &opened, SIGNING).await;
    assert_eq!(
        (status, body["outcome"].clone()),
        (StatusCode::OK, json!("applied")),
        "{body}"
    );
    assert!(
        d.fake.lock().unwrap().reads.is_empty(),
        "the invoice was already known"
    );
    let view = billing_view(&d, &s).await;
    assert_eq!(view["subscription"]["dispute"]["id"], "dp_1");
    assert_eq!(view["subscription"]["state"], "active");
    assert!(
        matches!(standing(&d, &s.account, s.now + 1), Standing::Active { .. }),
        "the paid month runs on"
    );
    assert_eq!(notice(&d, &s.account), Some(environment::Notice::Dispute));

    // Recorded once: the same event, and another event for the same
    // dispute, change nothing.
    let (_, body) = deliver(&d, &opened, SIGNING).await;
    assert_eq!(body["outcome"], "duplicate");
    let again = event(
        "evt_dp1b",
        "charge.dispute.created",
        dispute("dp_1", "ch_1", "needs_response"),
    );
    let (_, body) = deliver(&d, &again, SIGNING).await;
    assert_eq!(
        body["outcome"], "superseded:invoice is already disputed",
        "{body}"
    );

    // A renewal paid during the dispute waits: Stripe is told "not yet",
    // and no month is recorded.
    let renewal = event(
        "evt_inv2",
        "invoice.paid",
        paid_invoice("in_2", "subscription_cycle", next, &s.meta, "ch_2"),
    );
    let (status, body) = deliver(&d, &renewal, SIGNING).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "not_yet");
    assert_eq!(
        standing(&d, &s.account, next + 1),
        Standing::Ended { at: next }
    );

    // The dispute closes in our favour: the payment stands, and the same
    // renewal, sent again, starts its month.
    let won = event(
        "evt_dp1c",
        "charge.dispute.closed",
        dispute("dp_1", "ch_1", "won"),
    );
    let (status, body) = deliver(&d, &won, SIGNING).await;
    assert_eq!(
        (status, body["outcome"].clone()),
        (StatusCode::OK, json!("applied")),
        "{body}"
    );
    assert_eq!(notice(&d, &s.account), None);
    let view = billing_view(&d, &s).await;
    assert!(view["subscription"]["dispute"].is_null(), "{view}");
    let first = view["invoices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "in_1")
        .unwrap();
    assert_eq!(first["state"], "paid", "{view}");
    let (_, body) = deliver(&d, &won, SIGNING).await;
    assert_eq!(body["outcome"], "duplicate");
    let (status, body) = deliver(&d, &renewal, SIGNING).await;
    assert_eq!(
        (status, body["outcome"].clone()),
        (StatusCode::OK, json!("applied")),
        "{body}"
    );
    assert!(matches!(
        standing(&d, &s.account, next + 1),
        Standing::Active { .. }
    ));
}

#[tokio::test]
async fn a_full_refund_ends_the_month_and_a_partial_one_does_not() {
    let d = deploy().await;
    let s = subscriber(&d, Some("ch_1")).await;

    // A partial refund: the month stands.
    let mut partial = event("evt_rf0", "charge.refunded", charge("ch_1", 2000, 500));
    partial["created"] = json!(s.now + DAY);
    let (status, body) = deliver(&d, &partial, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["outcome"].as_str().unwrap().starts_with("ignored:"),
        "{body}"
    );
    assert!(matches!(
        standing(&d, &s.account, s.now + 2 * DAY),
        Standing::Active { .. }
    ));

    // A full refund five days in: the month ends then, the subscription
    // is cancelled, and Settings says why.
    let at = s.now + 5 * DAY;
    let mut full = event("evt_rf1", "charge.refunded", charge("ch_1", 2000, 2000));
    full["created"] = json!(at);
    let (status, body) = deliver(&d, &full, SIGNING).await;
    assert_eq!(
        (status, body["outcome"].clone()),
        (StatusCode::OK, json!("applied")),
        "{body}"
    );
    assert_eq!(standing(&d, &s.account, at + 1), Standing::Ended { at });
    assert!(matches!(
        standing(&d, &s.account, at - 1),
        Standing::Active { .. }
    ));
    assert_eq!(notice(&d, &s.account), Some(environment::Notice::Refunded));
    let view = billing_view(&d, &s).await;
    assert!(
        matches!(
            view["subscription"]["state"].as_str(),
            Some("cancelled" | "expired")
        ),
        "{view}"
    );
    assert_eq!(view["invoices"][0]["state"], "refunded", "{view}");

    // Once: the same event, and a second event for the same refund.
    let (_, body) = deliver(&d, &full, SIGNING).await;
    assert_eq!(body["outcome"], "duplicate");
    let mut other = event("evt_rf2", "charge.refunded", charge("ch_1", 2000, 2000));
    other["created"] = json!(at + DAY);
    let (_, body) = deliver(&d, &other, SIGNING).await;
    assert_eq!(
        body["outcome"], "superseded:invoice already closed",
        "{body}"
    );
    assert_eq!(standing(&d, &s.account, at + 2), Standing::Ended { at });

    // A charge that isn't a subscription payment is acknowledged.
    let stray = event("evt_rf3", "charge.refunded", charge("ch_stray", 700, 700));
    let (status, body) = deliver(&d, &stray, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["outcome"].as_str().unwrap().starts_with("ignored:"),
        "{body}"
    );

    // A forged event is refused before it can touch anything.
    let forged = event("evt_rf4", "charge.refunded", charge("ch_1", 2000, 2000));
    let (status, _) = deliver(&d, &forged, "whsec_wrong_secret_wrong_secret").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Paying again later is a fresh start: the notice clears.
    let later = at + 40 * DAY;
    let resubscribed = event(
        "evt_inv9",
        "invoice.paid",
        paid_invoice("in_9", "subscription_cycle", later, &s.meta, "ch_9"),
    );
    let (status, body) = deliver(&d, &resubscribed, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(notice(&d, &s.account), None);
}

#[tokio::test]
async fn a_lost_dispute_is_found_through_its_charge_and_closes_the_month_once() {
    let d = deploy().await;
    // The invoice carries no payment references (Stripe's newer shape), so
    // the dispute's charge is read from Stripe and matched by customer and
    // amount.
    let s = subscriber(&d, None).await;
    let opened = event(
        "evt_dp1",
        "charge.dispute.created",
        dispute("dp_1", "ch_77", "needs_response"),
    );
    let (status, body) = deliver(&d, &opened, SIGNING).await;
    assert_eq!(
        (status, body["outcome"].clone()),
        (StatusCode::OK, json!("applied")),
        "{body}"
    );
    assert_eq!(d.fake.lock().unwrap().reads, vec!["ch_77".to_string()]);

    let at = s.now + 3 * DAY;
    let mut lost = event(
        "evt_dp1z",
        "charge.dispute.closed",
        dispute("dp_1", "ch_77", "lost"),
    );
    lost["created"] = json!(at);
    let (status, body) = deliver(&d, &lost, SIGNING).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["outcome"], "applied", "{body}");
    assert_eq!(standing(&d, &s.account, at + 1), Standing::Ended { at });
    assert_eq!(
        notice(&d, &s.account),
        Some(environment::Notice::DisputeLost)
    );
    let view = billing_view(&d, &s).await;
    assert_eq!(view["invoices"][0]["state"], "dispute-lost", "{view}");
    assert!(view["subscription"]["dispute"].is_null(), "{view}");

    // Once.
    let (_, body) = deliver(&d, &lost, SIGNING).await;
    assert_eq!(body["outcome"], "duplicate");
    let mut again = event(
        "evt_dp1y",
        "charge.dispute.closed",
        dispute("dp_1", "ch_77", "lost"),
    );
    again["created"] = json!(at + DAY);
    let (_, body) = deliver(&d, &again, SIGNING).await;
    assert_eq!(
        body["outcome"], "superseded:invoice already closed",
        "{body}"
    );

    // Stripe down: a dispute that must be placed answers 503 so Stripe
    // sends it again.
    d.fake.lock().unwrap().down = true;
    let stray = event(
        "evt_dp2",
        "charge.dispute.created",
        dispute("dp_2", "ch_88", "needs_response"),
    );
    let (status, body) = deliver(&d, &stray, SIGNING).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
}
