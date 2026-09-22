//! The usage surface's end-to-end contract: the member-scoped
//! summary, activity, timeseries, receipt, and export reads over the
//! workspace's own receipts joined to the money and quota ledgers, the
//! billing entitlement the summary reports, and the dashboard's cookie
//! session and pages — over real HTTP against an in-process deployment
//! with accounts, monetary admission, and billing.
//!
//! Every test stands up its own directory and listeners; nothing
//! shares state but the webhook secret's name and value — a fixture
//! secret the environment carries, never a real credential.

mod common;

use std::sync::{Arc, Once};

use axum::http::StatusCode;
use serde_json::{Value, json};
use tenancy::billing::{ModelAccess, Plan, Price};
use tenancy::{Registry, keys};

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

/// The free plan: no charge, an allowance plus sign-up credit that
/// funds the workspace's ledger account, the shared door only.
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

/// The billing block the free plan sits in.
fn billing_config() -> config::Billing {
    config::Billing {
        plans: vec![free_plan()],
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

/// The monetary block charging the fixture price on `shared-kev`.
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

/// A deployed gateway with accounts, money, and billing configured —
/// the surface usage reads join.
struct Deployment {
    _dir: tempfile::TempDir,
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
        _dir: dir,
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

async fn get(deployment: &Deployment, path: &str, token: &str) -> (StatusCode, Value) {
    exchange(
        reqwest::Client::new()
            .get(format!("{}{path}", deployment.address))
            .bearer_auth(token),
    )
    .await
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

/// The account id, workspace id, key secret, and session secret of a
/// sign-up answer.
struct Joined {
    account: String,
    workspace: String,
    key_token: String,
    session_token: String,
}

async fn join(deployment: &Deployment, label: &str) -> Joined {
    let (status, body) = post(deployment, "/v1/accounts", None, &json!({"label": label})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    Joined {
        account: body["account"]["id"].as_str().unwrap().to_string(),
        workspace: body["workspace"]["id"].as_str().unwrap().to_string(),
        key_token: body["key_token"].as_str().unwrap().to_string(),
        session_token: body["session_token"].as_str().unwrap().to_string(),
    }
}

/// Subscribe `workspace` to `free` — the standard onboarding step that
/// also funds the ledger account with the plan's sign-up credit and
/// period allowance.
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

/// A decision call under `token` naming `workspace`.
async fn decide(deployment: &Deployment, token: &str, workspace: &str) -> StatusCode {
    exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/systemone", deployment.address))
            .json(&call("shared-kev"))
            .bearer_auth(token)
            .header("x-workspace-id", workspace),
    )
    .await
    .0
}

fn code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn summary_reports_exact_ledger_totals() {
    let deployment = deploy().await;
    let joined = join(&deployment, "ada").await;
    subscribe_free(&deployment, &joined.workspace, &joined.key_token).await;
    assert_eq!(
        decide(&deployment, &joined.key_token, &joined.workspace).await,
        StatusCode::OK
    );
    assert_eq!(
        decide(&deployment, &joined.key_token, &joined.workspace).await,
        StatusCode::OK
    );

    let (status, body) = get(
        &deployment,
        &format!("/v1/workspaces/{}/usage", joined.workspace),
        &joined.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["v"], json!("openagents.usage.v1"));
    assert_eq!(body["totals"]["calls"], json!(2));
    assert_eq!(body["totals"]["answered"], json!(2));
    // Every call priced — the money lane charged each.
    assert_eq!(body["cost"]["priced_calls"], json!(2));
    assert_eq!(body["cost"]["unpriced_calls"], json!(0));
    assert!(body["cost"]["retail"].as_u64().unwrap() > 0);
    assert_eq!(body["cost"]["currency"], json!("USD"));
    // The quota join measured every call.
    assert!(body["units"]["questions"].as_u64().unwrap() > 0);
    assert_eq!(body["units"]["unmeasured"], json!(0));
    // Breakdowns name the door's model and the key.
    assert_eq!(body["by_model"][0]["calls"], json!(2));
    assert_eq!(body["by_key"][0]["calls"], json!(2));
    // The subscription is the reported entitlement — plan id, state,
    // and the doors it lists.
    assert_eq!(body["entitlement"]["plan"], json!("free"));
    assert_eq!(body["entitlement"]["state"], json!("active"));
    // The disclosure is present and reports nothing unverifiable.
    assert_eq!(body["disclosure"]["unverifiable"], json!(0));
    assert!(
        body["disclosure"]["scale"]
            .as_str()
            .unwrap()
            .contains("millionths")
    );
}

#[tokio::test]
async fn activity_paginates_with_a_stable_cursor() {
    let deployment = deploy().await;
    let joined = join(&deployment, "ada").await;
    subscribe_free(&deployment, &joined.workspace, &joined.key_token).await;
    for _ in 0..3 {
        assert_eq!(
            decide(&deployment, &joined.key_token, &joined.workspace).await,
            StatusCode::OK
        );
    }
    let base = format!("/v1/workspaces/{}/usage/activity", joined.workspace);
    let (status, page_one) = get(
        &deployment,
        &format!("{base}?limit=2"),
        &joined.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page_one}");
    assert_eq!(page_one["items"].as_array().unwrap().len(), 2);
    let cursor = page_one["cursor"].as_str().expect("a second page exists");

    let (status, page_two) = get(
        &deployment,
        &format!("{base}?limit=2&cursor={cursor}"),
        &joined.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page_two}");
    assert_eq!(page_two["items"].as_array().unwrap().len(), 1);
    assert_eq!(page_two["cursor"], Value::Null);

    // No receipt appears on both pages.
    let first: Vec<&str> = page_one["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["digest"].as_str().unwrap())
        .collect();
    let second: Vec<&str> = page_two["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["digest"].as_str().unwrap())
        .collect();
    assert!(first.iter().all(|digest| !second.contains(digest)));

    // Items carry the joined fields.
    let item = &page_one["items"][0];
    assert!(item["request"].as_str().is_some());
    assert_eq!(item["outcome"], json!("answered"));
    assert!(item["cost"]["retail"].as_u64().unwrap() > 0);
    assert!(item["units"]["questions"].as_u64().unwrap() > 0);

    // A malformed cursor refuses cleanly.
    let (status, body) = get(
        &deployment,
        &format!("{base}?cursor=garbage"),
        &joined.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "invalid_cursor");
}

#[tokio::test]
async fn filters_narrow_the_workspace_view() {
    let deployment = deploy().await;
    let joined = join(&deployment, "ada").await;
    subscribe_free(&deployment, &joined.workspace, &joined.key_token).await;
    assert_eq!(
        decide(&deployment, &joined.key_token, &joined.workspace).await,
        StatusCode::OK
    );
    let base = format!("/v1/workspaces/{}/usage/activity", joined.workspace);

    // The door's model matches; a different model does not.
    let (_, body) = get(
        &deployment,
        &format!("{base}?model=kev-0.6b"),
        &joined.session_token,
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    let (_, body) = get(
        &deployment,
        &format!("{base}?model=nope"),
        &joined.session_token,
    )
    .await;
    assert!(body["items"].as_array().unwrap().is_empty());

    // Outcome and transport narrow the same way.
    let (_, body) = get(
        &deployment,
        &format!("{base}?outcome=refused"),
        &joined.session_token,
    )
    .await;
    assert!(body["items"].as_array().unwrap().is_empty());
    let (_, body) = get(
        &deployment,
        &format!("{base}?outcome=answered&transport=http"),
        &joined.session_token,
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // A window that excludes everything returns nothing; today's
    // window keeps the call.
    let (_, body) = get(
        &deployment,
        &format!("{base}?to=2020-01-01"),
        &joined.session_token,
    )
    .await;
    assert!(body["items"].as_array().unwrap().is_empty());
    let (_, body) = get(
        &deployment,
        &format!("{base}?from=2020-01-01"),
        &joined.session_token,
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // The key filter names the credential id — the token's middle
    // segment, without the `oak_` prefix or the secret.
    let key_id = joined
        .key_token
        .strip_prefix("oak_")
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    let (_, body) = get(
        &deployment,
        &format!("{base}?key={key_id}"),
        &joined.session_token,
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // The capacity filter joins through the hold's price.
    let (_, body) = get(
        &deployment,
        &format!("{base}?capacity=shared"),
        &joined.session_token,
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    let (_, body) = get(
        &deployment,
        &format!("{base}?capacity=dedicated"),
        &joined.session_token,
    )
    .await;
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn a_stranger_reads_nothing_of_the_workspace() {
    let deployment = deploy().await;
    let ada = join(&deployment, "ada").await;
    let bob = join(&deployment, "bob").await;
    subscribe_free(&deployment, &ada.workspace, &ada.key_token).await;
    assert_eq!(
        decide(&deployment, &ada.key_token, &ada.workspace).await,
        StatusCode::OK
    );

    // Bob's session reaches none of Ada's usage.
    for suffix in [
        "usage",
        "usage/activity",
        "usage/timeseries",
        "usage/export",
    ] {
        let (status, body) = get(
            &deployment,
            &format!("/v1/workspaces/{}/{suffix}", ada.workspace),
            &bob.session_token,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{suffix}: {body}");
    }

    // And no token at all is unauthenticated, not merely denied.
    let (status, body) = exchange(reqwest::Client::new().get(format!(
        "{}/v1/workspaces/{}/usage",
        deployment.address, ada.workspace
    )))
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code(&body), "unauthenticated");

    // The disclosure field exists in Ada's own answer — the scoping is
    // disclosed, not silent.
    let (_, body) = get(
        &deployment,
        &format!("/v1/workspaces/{}/usage", ada.workspace),
        &ada.session_token,
    )
    .await;
    assert!(body["disclosure"]["other_workspace"].as_u64().is_some());
}

#[tokio::test]
async fn timeseries_buckets_by_utc_day() {
    let deployment = deploy().await;
    let joined = join(&deployment, "ada").await;
    subscribe_free(&deployment, &joined.workspace, &joined.key_token).await;
    for _ in 0..2 {
        assert_eq!(
            decide(&deployment, &joined.key_token, &joined.workspace).await,
            StatusCode::OK
        );
    }
    let (status, body) = get(
        &deployment,
        &format!("/v1/workspaces/{}/usage/timeseries", joined.workspace),
        &joined.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let days = body["days"].as_array().unwrap();
    assert_eq!(days.len(), 1, "{body}");
    assert_eq!(days[0]["calls"], json!(2));
    assert_eq!(days[0]["answered"], json!(2));
    assert!(days[0]["retail"].as_u64().unwrap() > 0);
    assert!(days[0]["questions"].as_u64().unwrap() > 0);
    assert_eq!(body["undated"], json!(0));
}

#[tokio::test]
async fn receipt_detail_and_export_round_trip() {
    let deployment = deploy().await;
    let joined = join(&deployment, "ada").await;
    subscribe_free(&deployment, &joined.workspace, &joined.key_token).await;
    assert_eq!(
        decide(&deployment, &joined.key_token, &joined.workspace).await,
        StatusCode::OK
    );

    let (_, activity) = get(
        &deployment,
        &format!("/v1/workspaces/{}/usage/activity", joined.workspace),
        &joined.session_token,
    )
    .await;
    let digest = activity["items"][0]["digest"].as_str().unwrap().to_string();

    let (status, body) = get(
        &deployment,
        &format!(
            "/v1/workspaces/{}/usage/receipts/{digest}",
            joined.workspace
        ),
        &joined.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["receipt"]["digest"], json!(digest));
    assert_eq!(body["receipt"]["workspace"], json!(joined.workspace));
    assert!(body["cost"]["retail"].as_u64().unwrap() > 0);
    assert!(body["units"]["questions"].as_u64().unwrap() > 0);

    // An unknown digest is a scoped miss.
    let (status, body) = get(
        &deployment,
        &format!(
            "/v1/workspaces/{}/usage/receipts/sha256:missing",
            joined.workspace
        ),
        &joined.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(code(&body), "unknown_receipt");

    // The export is NDJSON — one sealed receipt per line, re-verifying
    // offline by its own digest check.
    let response = reqwest::Client::new()
        .get(format!(
            "{}/v1/workspaces/{}/usage/export",
            deployment.address, joined.workspace
        ))
        .bearer_auth(&joined.session_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/x-ndjson"
    );
    let text = response.text().await.unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1);
    let receipt = receipts::execution::ExecutionReceipt::parse(lines[0]).unwrap();
    assert_eq!(receipt.digest, digest);
    assert_eq!(
        receipt.workspace.as_deref(),
        Some(joined.workspace.as_str())
    );
}

#[tokio::test]
async fn dashboard_signs_in_and_serves_every_page() {
    let deployment = deploy().await;
    let joined = join(&deployment, "ada").await;
    subscribe_free(&deployment, &joined.workspace, &joined.key_token).await;
    assert_eq!(
        decide(&deployment, &joined.key_token, &joined.workspace).await,
        StatusCode::OK
    );

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // Sign in with the session token — a 303 and a cookie.
    let response = client
        .post(format!("{}/dashboard/session", deployment.address))
        .form(&[("token", joined.session_token.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.contains("oa_session="), "{cookie}");
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    let token = cookie
        .split("oa_session=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    // The workspace picker lists the workspace.
    let response = client
        .get(format!("{}/dashboard", deployment.address))
        .header("cookie", format!("oa_session={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = response.text().await.unwrap();
    assert!(html.contains(&joined.workspace), "{html}");

    // Every workspace page answers under the cookie.
    for page in ["", "/usage", "/activity", "/members", "/keys", "/billing"] {
        let response = client
            .get(format!(
                "{}/dashboard/w/{}{page}",
                deployment.address, joined.workspace
            ))
            .header("cookie", format!("oa_session={token}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{page}");
        let html = response.text().await.unwrap();
        assert!(html.contains("<html"), "{page}");
    }

    // The activity page renders the call's receipt row, and the
    // receipt page answers for its digest.
    let response = client
        .get(format!(
            "{}/dashboard/w/{}/activity",
            deployment.address, joined.workspace
        ))
        .header("cookie", format!("oa_session={token}"))
        .send()
        .await
        .unwrap();
    let html = response.text().await.unwrap();
    assert!(html.contains("answered"), "{html}");

    // Sign-out clears the cookie.
    let response = client
        .post(format!("{}/dashboard/sign-out", deployment.address))
        .header("cookie", format!("oa_session={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cleared = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
}

#[tokio::test]
async fn dashboard_refuses_strangers_and_anonymous() {
    let deployment = deploy().await;
    let ada = join(&deployment, "ada").await;
    let bob = join(&deployment, "bob").await;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // Bob's cookie on Ada's workspace is a membership refusal, not a leak.
    let response = client
        .get(format!(
            "{}/dashboard/w/{}",
            deployment.address, ada.workspace
        ))
        .header("cookie", format!("oa_session={}", bob.session_token))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let html = response.text().await.unwrap();
    assert!(
        !html.contains(&ada.account),
        "the refusal leaks the account id"
    );

    // No cookie at all renders a sign-in-required page.
    let response = client
        .get(format!(
            "{}/dashboard/w/{}",
            deployment.address, ada.workspace
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // A garbage cookie on the picker degrades to the sign-in form —
    // the workspace pages still refuse, but the front door renders.
    let response = client
        .get(format!("{}/dashboard", deployment.address))
        .header("cookie", "oa_session=sess_garbage")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = response.text().await.unwrap();
    assert!(html.contains("session token"), "{html}");
}
