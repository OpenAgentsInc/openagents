use super::*;
use axum::{Form, Router, extract::State as ApiState, http::Uri};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Default)]
struct Native {
    records: BTreeMap<String, Value>,
    creates: Vec<(String, BTreeMap<String, String>)>,
    lose_checkout: bool,
}
async fn native_get(ApiState(native): ApiState<Arc<Mutex<Native>>>, uri: Uri) -> Response {
    let native = native.lock().unwrap();
    match native.records.get(uri.path()) {
        Some(v) => Json(v.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn native_create(
    ApiState(native): ApiState<Arc<Mutex<Native>>>,
    uri: Uri,
    headers: HeaderMap,
    Form(fields): Form<BTreeMap<String, String>>,
) -> Response {
    let mut native = native.lock().unwrap();
    native.creates.push((
        headers["idempotency-key"].to_str().unwrap().into(),
        fields.clone(),
    ));
    let (path, value) = if uri.path() == "/v1/customers" {
        (
            "/v1/customers/cus_controller",
            json!({"object":"customer","id":"cus_controller","livemode":false,"metadata":{"oa_customer":fields["metadata[oa_customer]"]}}),
        )
    } else {
        (
            "/v1/checkout/sessions/cs_test_controller",
            json!({"object":"checkout.session","id":"cs_test_controller","livemode":false,"mode":"payment",
            "customer":fields["customer"],"currency":"usd","amount_total":fields["line_items[0][price_data][unit_amount]"].parse::<u64>().unwrap(),
            "metadata":{"oa_quote":fields["metadata[oa_quote]"]},"client_reference_id":fields["client_reference_id"],
            "expires_at":fields["expires_at"].parse::<u64>().unwrap(),"status":"open","payment_status":"unpaid",
            "url":"https://checkout.stripe.com/c/pay/cs_test_controller"}),
        )
    };
    let value = native.records.entry(path.into()).or_insert(value).clone();
    if uri.path() == "/v1/checkout/sessions" && native.lose_checkout {
        native.lose_checkout = false;
        return StatusCode::BAD_GATEWAY.into_response();
    }
    Json(value).into_response()
}
async fn server(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (origin, task)
}
async fn body(response: Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), 512 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn browser_fields(html: &str) -> BTreeMap<String, String> {
    let form = html
        .split("<form")
        .find(|f| f.contains("name=\"csrf\""))
        .expect("An actual funding form renders.");
    form.split("<input")
        .filter_map(|input| {
            let input = input.split('>').next()?;
            let name = input.split("name=\"").nth(1)?.split('"').next()?;
            let value = input.split("value=\"").nth(1)?.split('"').next()?;
            Some((name.into(), value.into()))
        })
        .collect()
}
fn signature(secret: &str, bytes: &[u8], timestamp: u64) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(bytes);
    format!(
        "t={timestamp},v1={}",
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

#[tokio::test]
async fn native_checkout_lost_reply_webhook_and_recovery_keep_one_original_credit() {
    use tenancy::{Binding as DoorBinding, Expected, Lane, Manifest, Registry, Tenant, keys};
    let root = tempfile::tempdir().unwrap();
    let hash = format!("sha256:{}", "a".repeat(64));
    let manifest = Manifest {
        v: tenancy::SCHEMA.into(),
        sequence: 0,
        supersedes: None,
        digest: String::new(),
        shared: [(
            "fixture".into(),
            DoorBinding {
                lane: Lane::Shared,
                artifact: Expected {
                    model: "fixture-model".into(),
                    adapter: None,
                    artifact_signature: hash,
                    execution: BTreeMap::new(),
                },
                capacity: None,
                promotion: None,
                scope: vec![],
            },
        )]
        .into(),
        tenants: [(
            "fixture".into(),
            Tenant {
                credential: "key-ref:fixture".into(),
                principals: vec![],
                doors: BTreeMap::new(),
                quota: None,
            },
        )]
        .into(),
    };
    let registry = Registry::install(root.path(), manifest).unwrap();
    let mut config = crate::card_funding::config::tests::config();
    let suffix = tenancy::billing::fresh_ref().unwrap().to_ascii_uppercase();
    config.restricted_key_env = format!("OA_CARD_FIXTURE_{suffix}");
    config.webhook_secret_envs = vec![format!("OA_CARD_SIGN_{suffix}")];
    let material = secp256k1::SecretKey::new(&mut secp256k1::rand::rng())
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let signing = secp256k1::SecretKey::new(&mut secp256k1::rand::rng())
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    unsafe {
        std::env::set_var(&config.restricted_key_env, format!("rk_test_{material}"));
        std::env::set_var(&config.webhook_secret_envs[0], &signing);
    }
    let gateway:crate::config::Config=serde_json::from_value(json!({
        "v":crate::config::SCHEMA,"listen":"127.0.0.1:0","registry":root.path(),"require_workspace_membership":true,
        "accounts":{"signup_tenant":"fixture","open_signup":true},"billing":{"provider":"stripe","prepaid":config},
        "doors":{"fixture":{"endpoint":"http://127.0.0.1:1"}},
        "money":{"ledger":root.path().join("money.jsonl"),"doors":{"fixture":{
            "price":{"version":"fixture-price","currency":"USD","model":"fixture-model","capacity":"shared","policy":crate::money::POLICY,
                "rates":{"input-tokens":{"millionths":1,"per_units":1}}},"maximum_usage":{"input-tokens":1}}}}
    })).unwrap();
    let state = ServeState::open(gateway).unwrap();
    let accounts = tenancy::Accounts::open(root.path()).unwrap();
    let (account, _) = accounts
        .create_account_unattributed("private seeded customer label")
        .unwrap();
    let workspace = accounts
        .create_workspace(
            &account.id,
            "private seeded workspace",
            tenancy::WorkspaceKind::Personal,
            "fixture",
            None,
        )
        .unwrap();
    let issued = keys::issue_scoped(
        root.path(),
        registry.manifest(),
        "fixture",
        Some("fixture"),
        None,
    )
    .unwrap();
    accounts
        .update_principals(&account.id, &[format!("key:{}", issued.key.id)])
        .unwrap();
    tenancy::Sessions::open(root.path())
        .unwrap()
        .mutate(|b, _, now| {
            b.set_credential(account.id.as_str().into(), issued.key.digest.clone(), now)
        })
        .unwrap();
    let session = tenancy::Sessions::open(root.path())
        .unwrap()
        .mutate(|b, _, at| b.issue(account.id.as_str().into(), at))
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", session.once).parse().unwrap(),
    );
    headers.insert("x-workspace-id", workspace.id.parse().unwrap());
    let native = Arc::new(Mutex::new(Native {
        records: [(
            "/v1/account".into(),
            json!({"object":"account","id":"acct_fixture"}),
        )]
        .into(),
        creates: vec![],
        lose_checkout: true,
    }));
    let (origin, task) = server(
        Router::new()
            .fallback(axum::routing::get(native_get).post(native_create))
            .with_state(native.clone()),
    )
    .await;
    *state.card_test_origin.lock().unwrap() = Some(origin);
    let before_invalid = state.money_lock().await.unwrap().head().to_string();
    for invalid in [
        "short",
        "private.contact@example.invalid",
        "not/a/native/reference",
    ] {
        assert!(
            quoted(&state, &headers, "fixture", invalid, 100_000_000)
                .await
                .is_err()
        );
    }
    assert_eq!(state.money_lock().await.unwrap().head(), before_invalid);
    assert!(
        state
            .money_lock()
            .await
            .unwrap()
            .balance(&workspace.id)
            .is_err()
    );
    let (customer_origin, customer_task) = server(crate::serve::router(state.clone())).await;
    let client = reqwest::Client::new();
    let mut browser_headers = HeaderMap::new();
    browser_headers.insert(
        "cookie",
        format!("oa_session={}", session.once).parse().unwrap(),
    );
    let billing_url = format!("{customer_origin}/dashboard/w/{}/billing", workspace.id);
    let funding_url = format!("{customer_origin}/dashboard/w/{}/funding", workspace.id);
    let response = client
        .get(&billing_url)
        .headers(browser_headers.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut fields = browser_fields(&response.text().await.unwrap());
    let original_id = fields["id"].clone();
    let id = original_id.as_str();
    fields.insert("gross_cents".into(), "10000".into());
    let response = client
        .post(&funding_url)
        .headers(browser_headers.clone())
        .form(&fields)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let approve_fields = browser_fields(&response.text().await.unwrap());
    assert_eq!(approve_fields["action"], "checkout");
    for path in [
        "/v1/plans",
        "/v1/billing/sessions/native_controller_quote_001",
    ] {
        assert_eq!(
            client
                .get(format!("{customer_origin}{path}"))
                .headers(headers.clone())
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        client
            .post(format!("{customer_origin}/v1/billing/webhook"))
            .json(&json!({"provider":"sandbox","amount":99999999}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let original = retained(book(&state).unwrap(), id).unwrap();
    assert_eq!(
        state
            .money_lock()
            .await
            .unwrap()
            .balance(&workspace.id)
            .unwrap()
            .available,
        0
    );
    assert!(
        checkout(&state, &headers, "fixture", id, "wrong-approval")
            .await
            .is_err()
    );
    assert!(native.lock().unwrap().creates.is_empty());
    let approved = digest(&original.binding);
    assert_eq!(approve_fields["approved"], approved);
    let interrupted = client
        .post(&funding_url)
        .headers(browser_headers.clone())
        .form(&approve_fields)
        .send()
        .await
        .unwrap();
    assert_eq!(interrupted.status(), StatusCode::SERVICE_UNAVAILABLE);
    let unknown = retained(book(&state).unwrap(), id).unwrap();
    assert!(unknown.checkout.as_ref().unwrap().native.is_none());
    let original_key = unknown.checkout.as_ref().unwrap().idempotency.clone();
    let recovered = client
        .post(&funding_url)
        .headers(browser_headers.clone())
        .form(&approve_fields)
        .send()
        .await
        .unwrap();
    assert_eq!(recovered.status(), StatusCode::OK);
    assert!(
        recovered
            .text()
            .await
            .unwrap()
            .contains("Continue the original hosted checkout")
    );
    let ready = retained(book(&state).unwrap(), id).unwrap();
    assert_eq!(ready.checkout.as_ref().unwrap().idempotency, original_key);
    assert_eq!(
        ready.checkout.as_ref().unwrap().native.as_deref(),
        Some("cs_test_controller")
    );
    {
        let n = native.lock().unwrap();
        assert_eq!(n.creates.len(), 3);
        assert_eq!(n.creates[1], n.creates[2]);
        let sent = serde_json::to_string(&n.creates).unwrap();
        assert!(!sent.contains("private seeded"));
    }
    // The selected client reads private original terms without moving money.

    let resume_url = format!("{customer_origin}/dashboard/funding/{id}");
    let before = state.money_lock().await.unwrap().head().to_string();
    for _ in 0..2 {
        let r = client
            .get(&resume_url)
            .headers(browser_headers.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()["cache-control"], "no-store");
        let html = r.text().await.unwrap();
        assert!(html.contains("Unknown; no verified collection"));
        assert!(html.contains("Continue the original hosted checkout"));
        assert!(!html.contains(&session.once));
        assert!(!html.contains("private seeded"));
        assert!(html.contains("Processor and Lightning wallet liquidity: unknown"));
    }
    if let (Ok(binary), Ok(helper)) = (
        std::env::var("OPENAGENTS_REV23_BROWSER"),
        std::env::var("OPENAGENTS_REV23_BROWSER_CHECK"),
    ) {
        let (origin, token, ws, quote) = (
            customer_origin.clone(),
            session.once.clone(),
            workspace.id.clone(),
            id.to_string(),
        );
        let result = tokio::task::spawn_blocking(move || {
            std::process::Command::new(binary)
                .args(["browser", "run", "--json", "--", "python3", &helper])
                .env("REV23_FIXTURE_ORIGIN", origin)
                .env("REV23_FIXTURE_SESSION", token)
                .env("REV23_FIXTURE_WORKSPACE", ws)
                .env("REV23_FIXTURE_QUOTE", quote)
                .output()
                .expect("Isolated browser fixture launches.")
        })
        .await
        .unwrap();
        assert!(
            result.status.success(),
            "Isolated browser acceptance failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        println!("{}", String::from_utf8_lossy(&result.stdout));
    }
    assert_eq!(state.money_lock().await.unwrap().head(), before);
    let sdk = jev::Client::new(
        jev::Config::new()
            .api_key(session.once.clone())
            .base_url(&customer_origin),
    )
    .unwrap();
    let view = sdk
        .account()
        .card_funding(
            &workspace.id,
            "fixture",
            &jev::CardFundingAction::Read { id: id.into() },
        )
        .await
        .unwrap();
    assert_eq!(view.balance.available, 0);
    assert!(view.outstanding.as_ref().unwrap().is_empty());
    assert!(!format!("{view:?}").contains("checkout.stripe.com"));
    reconcile_original(&state, book(&state).unwrap(), id)
        .await
        .unwrap();
    assert_eq!(
        retained(book(&state).unwrap(), id)
            .unwrap()
            .unpaid_status
            .as_deref(),
        Some("pending")
    );
    native
        .lock()
        .unwrap()
        .records
        .get_mut("/v1/checkout/sessions/cs_test_controller")
        .unwrap()["status"] = json!("expired");
    reconcile_original(&state, book(&state).unwrap(), id)
        .await
        .unwrap();
    let r = client
        .get(&resume_url)
        .headers(browser_headers.clone())
        .send()
        .await
        .unwrap();
    assert!(
        r.text()
            .await
            .unwrap()
            .contains("Expired unpaid; no funding")
    );
    assert_eq!(
        state
            .money_lock()
            .await
            .unwrap()
            .balance(&workspace.id)
            .unwrap()
            .available,
        0
    );
    native
        .lock()
        .unwrap()
        .records
        .get_mut("/v1/checkout/sessions/cs_test_controller")
        .unwrap()["status"] = json!("open");
    assert_eq!(native.lock().unwrap().creates.len(), 3);
    assert_eq!(
        client.get(&resume_url).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let bytes=serde_json::to_vec(&json!({"object":"event","id":"evt_controller","api_version":"fixture.v1","livemode":false,"created":now(),"type":"charge.succeeded",
        "data":{"object":{"object":"charge","id":"ch_controller","paid":true,"amount":10000,"client_credit":999999999}}})).unwrap();
    let mut delivery = HeaderMap::new();
    delivery.insert(
        "stripe-signature",
        signature(&signing, &bytes, now()).parse().unwrap(),
    );
    assert_eq!(
        webhook(
            State(state.clone()),
            delivery.clone(),
            Bytes::from(bytes.clone())
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        state
            .money_lock()
            .await
            .unwrap()
            .balance(&workspace.id)
            .unwrap()
            .available,
        0
    );
    {
        let mut n = native.lock().unwrap();
        let checkout = n
            .records
            .get_mut("/v1/checkout/sessions/cs_test_controller")
            .unwrap();
        checkout["status"] = json!("complete");
        checkout["payment_status"] = json!("paid");
        checkout["payment_intent"] = json!("pi_controller");
        let at = ready.binding.quoted_at;
        n.records.extend([
            ("/v1/payment_intents/pi_controller".into(),json!({"object":"payment_intent","id":"pi_controller","livemode":false,"status":"succeeded","capture_method":"automatic","customer":"cus_controller","currency":"usd","amount":10000,"amount_received":10000,"amount_capturable":0,"metadata":{"oa_quote":id},"latest_charge":"ch_controller"})),
            ("/v1/charges/ch_controller".into(),json!({"object":"charge","id":"ch_controller","livemode":false,"payment_intent":"pi_controller","customer":"cus_controller","currency":"usd","status":"succeeded","paid":true,"captured":true,"payment_method_details":{"type":"card","card":{"last4":"seeded private processor payload"}},"amount":10000,"amount_captured":10000,"created":at,"balance_transaction":"txn_controller","amount_refunded":0,"refunded":false,"disputed":false})),
            ("/v1/balance_transactions/txn_controller".into(),json!({"object":"balance_transaction","id":"txn_controller","source":"ch_controller","type":"charge","currency":"usd","amount":10000,"fee":300,"net":9700,"status":"available","created":at,"available_on":at})),
            ("/v1/refunds".into(),json!({"object":"list","url":"/v1/refunds","has_more":false,"data":[]})),
            ("/v1/disputes".into(),json!({"object":"list","url":"/v1/disputes","has_more":false,"data":[]})),
        ]);
    }
    assert_eq!(
        webhook(
            State(state.clone()),
            delivery.clone(),
            Bytes::from(bytes.clone())
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        state
            .money_lock()
            .await
            .unwrap()
            .balance(&workspace.id)
            .unwrap()
            .credited,
        97_000_000
    );
    assert_eq!(
        webhook(State(state.clone()), delivery, Bytes::from(bytes.clone()))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        state
            .money_lock()
            .await
            .unwrap()
            .balance(&workspace.id)
            .unwrap()
            .credited,
        97_000_000
    );
    let r = client
        .get(&resume_url)
        .headers(browser_headers.clone())
        .send()
        .await
        .unwrap();
    let html = r.text().await.unwrap();
    assert!(html.contains("Funded"));
    assert!(html.contains("97.000000 USD"));
    assert!(!html.contains("Continue the original hosted checkout"));
    let old = retained(book(&state).unwrap(), id).unwrap();
    assert!(
        !serde_json::to_string(&old)
            .unwrap()
            .contains("seeded private processor")
    );
    {
        let mut ledger = state.money_lock().await.unwrap();
        use tenancy::money::{CreditKind, Mutation, Operation};
        for (source, operation) in [
            (
                "legacy-create",
                Operation::Create {
                    currency: "USD".into(),
                    spend_limit: 100_000_000,
                    topups_allowed: true,
                },
            ),
            (
                "legacy-credit",
                Operation::Credit {
                    amount: 100_000_000,
                    credit_kind: CreditKind::Grant,
                },
            ),
        ] {
            ledger
                .apply(Mutation {
                    workspace: "old-sandbox".into(),
                    source: source.into(),
                    audit: "isolated legacy balance fixture".into(),
                    operation,
                })
                .unwrap();
        }
        let head = ledger.head().to_string();
        assert!(check_money_profile(&state, &ledger, "old-sandbox").is_err());
        assert_eq!(ledger.head(), head);
        assert_eq!(
            ledger.balance("old-sandbox").unwrap().available,
            100_000_000
        );
        assert!(check_money_profile(&state, &ledger, &workspace.id).is_ok());
    }
    native.lock().unwrap().records.insert(
        "/v1/account".into(),
        json!({"object":"account","id":"acct_changed"}),
    );
    assert!(
        reconcile_original(&state, book(&state).unwrap(), id)
            .await
            .is_err()
    );
    assert_eq!(
        state
            .money_lock()
            .await
            .unwrap()
            .balance(&workspace.id)
            .unwrap()
            .available,
        0
    );
    native.lock().unwrap().records.insert(
        "/v1/account".into(),
        json!({"object":"account","id":"acct_fixture"}),
    );
    reconcile_original(&state, book(&state).unwrap(), id)
        .await
        .unwrap();
    let restored = state
        .money_lock()
        .await
        .unwrap()
        .balance(&workspace.id)
        .unwrap();
    assert_eq!(restored.available, 97_000_000);
    assert_eq!(restored.credited, 97_000_000);
    let provider = original_provider(&state, &config, &old.binding)
        .await
        .unwrap();
    native
        .lock()
        .unwrap()
        .records
        .remove("/v1/charges/ch_controller");
    assert!(
        reconcile_with(&state, book(&state).unwrap(), &provider, id)
            .await
            .is_err()
    );
    let balance = state
        .money_lock()
        .await
        .unwrap()
        .balance(&workspace.id)
        .unwrap();
    assert_eq!(balance.available, 0);
    assert_eq!(balance.credited, 97_000_000);
    let mut invalid = HeaderMap::new();
    invalid.insert(
        "stripe-signature",
        signature("wrong synthetic secret", &bytes, now())
            .parse()
            .unwrap(),
    );
    assert_eq!(
        webhook(State(state.clone()), invalid, Bytes::from(bytes))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let read = handle(
        State(state.clone()),
        Path((workspace.id.clone(), "fixture".into())),
        headers,
        Json(Request::Read { id: id.into() }),
    )
    .await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(body(read).await["balance"]["available"], 0);
    // Balance and inference access do not grant billing mutations.
    let readonly = keys::issue_scoped(
        root.path(),
        registry.manifest(),
        "fixture",
        Some("readonly"),
        Some(keys::Scopes {
            models: None,
            actions: Some(
                ["accounts", "balance", "inference"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            ),
        }),
    )
    .unwrap();
    accounts
        .update_principals(
            &account.id,
            &[
                format!("key:{}", issued.key.id),
                format!("key:{}", readonly.key.id),
            ],
        )
        .unwrap();
    let mut readonly_headers = HeaderMap::new();
    readonly_headers.insert(
        "authorization",
        format!("Bearer {}", readonly.token).parse().unwrap(),
    );
    readonly_headers.insert("x-workspace-id", workspace.id.parse().unwrap());
    let endpoint = format!(
        "{customer_origin}/v1/workspaces/{}/card-funding/fixture",
        workspace.id
    );
    let before = state.money_lock().await.unwrap().head().to_string();
    for action in [
        json!({"action":"quote","id":"readonly_native_quote","gross_units":10_000}),
        json!({"action":"checkout","id":id,"approved":approved}),
        json!({"action":"reconcile","id":id}),
    ] {
        assert_eq!(
            client
                .post(&endpoint)
                .headers(readonly_headers.clone())
                .json(&action)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(state.money_lock().await.unwrap().head(), before);
    assert_eq!(
        client
            .post(&endpoint)
            .headers(readonly_headers)
            .json(&json!({"action":"read","id":id}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    customer_task.abort();
    let _ = customer_task.await;
    task.abort();
    let _ = task.await;
}
