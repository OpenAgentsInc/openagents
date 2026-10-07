use super::*;
use axum::{Router, body::Body, http::Response, routing::get};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn secret() -> Vec<u8> {
    secp256k1::SecretKey::new(&mut secp256k1::rand::rng())
        .secret_bytes()
        .to_vec()
}
fn body() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "id":"evt_fixture", "object":"event", "api_version":"fixture.v1",
        "livemode":false, "created":90, "type":"checkout.session.completed",
        "data":{"object":{"id":"cs_test_fixture", "object":"checkout.session"}}
    }))
    .unwrap()
}
fn sign(bytes: &[u8], key: &[u8], timestamp: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap();
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(bytes);
    let tag = mac.finalize().into_bytes();
    format!(
        "t={timestamp},v1={}",
        tag.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
#[test]
fn raw_signature_rotation_freshness_mode_version_and_duplicate_fields_are_checked() {
    let key = secret();
    let bytes = body();
    let header = sign(&bytes, &key, "100");
    let event = verify_webhook(&bytes, &header, &key, 100, 300, "fixture.v1", false).unwrap();
    assert_eq!(event.object, "cs_test_fixture");
    assert_eq!(event.body_sha256, format!("{:x}", Sha256::digest(&bytes)));
    let rotated = format!(
        "t=100,v1={},{}",
        "0".repeat(64),
        header.split_once(',').unwrap().1
    );
    assert!(verify_webhook(&bytes, &rotated, &key, 100, 300, "fixture.v1", false).is_ok());
    for altered in [
        format!("{header},t=100"),
        sign(&bytes, &key, "900"),
        sign(&bytes, &key, "1"),
    ] {
        assert!(verify_webhook(&bytes, &altered, &key, 400, 300, "fixture.v1", false).is_err());
    }
    assert!(verify_webhook(&bytes, &header, &secret(), 100, 300, "fixture.v1", false).is_err());
    assert!(verify_webhook(&bytes, &header, &key, 100, 300, "wrong", false).is_err());
    assert!(verify_webhook(&bytes, &header, &key, 100, 300, "fixture.v1", true).is_err());
    let mut changed = bytes.clone();
    changed.push(b' ');
    assert!(verify_webhook(&changed, &header, &key, 100, 300, "fixture.v1", false).is_err());
    let duplicate = br#"{"id":"evt_one","id":"evt_two"}"#;
    assert!(
        verify_webhook(
            duplicate,
            &sign(duplicate, &key, "100"),
            &key,
            100,
            300,
            "fixture.v1",
            false
        )
        .is_err()
    );
}
#[test]
fn native_scope_and_bounds_refuse_unrelated_connected_account_and_wrong_object() {
    let key = secret();
    for field in ["type", "account", "context", "data"] {
        let mut value: Value = serde_json::from_slice(&body()).unwrap();
        value[field] = match field {
            "type" => json!("invoice.paid"),
            "data" => json!({"object":{"object":"charge", "id":"ch_other"}}),
            _ => json!("acct_other"),
        };
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            verify_webhook(
                &bytes,
                &sign(&bytes, &key, "100"),
                &key,
                100,
                300,
                "fixture.v1",
                false
            )
            .is_err()
        );
    }
    let large = vec![b' '; MAX_BODY + 1];
    assert!(
        verify_webhook(
            &large,
            &sign(&large, &key, "100"),
            &key,
            100,
            300,
            "fixture.v1",
            false
        )
        .is_err()
    );
    assert!(identifier("cs_test_valid", "cs_").is_ok());
    for id in ["cs_", "cs_a/../other", "cs_a?token", "cs_a\n", "cs_é"] {
        assert!(identifier(id, "cs_").is_err());
    }
}

async fn server(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (origin, task)
}

#[test]
fn native_dispute_references_accept_current_family_and_refuse_other_objects_and_paths() {
    let key = secret();
    for id in ["du_fixture", "dp_fixture"] {
        let mut value: Value = serde_json::from_slice(&body()).unwrap();
        value["type"] = json!("charge.dispute.funds_reinstated");
        value["data"]["object"] = json!({"object":"dispute", "id":id});
        let bytes = serde_json::to_vec(&value).unwrap();
        let event = verify_webhook(
            &bytes,
            &sign(&bytes, &key, "100"),
            &key,
            100,
            300,
            "fixture.v1",
            false,
        )
        .unwrap();
        assert_eq!(event.object, id);
        value["data"]["object"]["object"] = json!("charge");
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            verify_webhook(
                &bytes,
                &sign(&bytes, &key, "100"),
                &key,
                100,
                300,
                "fixture.v1",
                false,
            )
            .is_err()
        );
    }
    for id in [
        "du_",
        "du_a/../other",
        "du_a?token",
        "du_a\n",
        "du_é",
        "ch_other",
    ] {
        assert!(identifier(id, "dp_").is_err());
    }
}
#[tokio::test]
async fn actual_native_api_checks_account_identity_bounds_and_never_follows_redirects() {
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let (other, other_task) = server(Router::new().fallback(get(move || {
        observed.fetch_add(1, Ordering::SeqCst);
        async { "credential must not reach this endpoint" }
    })))
    .await;
    let destination = other.clone();
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async { axum::Json(json!({"object":"account", "id":"acct_fixture"})) }),
        )
        .route(
            "/v1/charges/ch_wrong",
            get(|| async { axum::Json(json!({"object":"charge", "id":"ch_substituted"})) }),
        )
        .route(
            "/v1/charges/ch_redirect",
            get(move || {
                let destination = destination.clone();
                async move {
                    Response::builder()
                        .status(307)
                        .header("location", destination)
                        .body(Body::empty())
                        .unwrap()
                }
            }),
        )
        .route(
            "/v1/charges/ch_large",
            get(|| async { vec![b' '; MAX_BODY + 1] }),
        )
        .route(
            "/v1/disputes/du_fixture",
            get(|| async { axum::Json(json!({"object":"dispute", "id":"du_fixture"})) }),
        )
        .route(
            "/v1/disputes/du_wrong",
            get(|| async { axum::Json(json!({"object":"dispute", "id":"du_other"})) }),
        );
    let (origin, task) = server(router).await;
    let key = secret()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut client = Stripe::new(key, "fixture.v1".into()).unwrap();
    client.origin = origin;
    assert!(client.account("acct_fixture").await.is_ok());
    assert!(client.account("acct_other").await.is_err());
    assert!(client.get("charges", "ch_wrong").await.is_err());
    assert!(client.get("charges", "ch_redirect").await.is_err());
    assert!(client.get("charges", "ch_large").await.is_err());
    assert!(client.get("disputes", "du_fixture").await.is_ok());
    assert!(client.get("disputes", "du_wrong").await.is_err());
    assert!(client.get("../other", "ch_wrong").await.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    task.abort();
    other_task.abort();
}

#[tokio::test]
async fn actual_checkout_creation_is_native_account_bound_one_time_exact_and_retry_identical() {
    use axum::{
        extract::State,
        http::{HeaderMap, Uri},
        routing::post,
    };
    use std::{collections::BTreeMap, sync::Mutex};
    #[derive(Clone)]
    struct Fixture {
        requests: Arc<Mutex<Vec<(String, BTreeMap<String, String>, String)>>>,
        fault: Arc<AtomicUsize>,
    }
    async fn create(
        State(f): State<Fixture>,
        uri: Uri,
        headers: HeaderMap,
        bytes: axum::body::Bytes,
    ) -> axum::Json<Value> {
        assert_eq!(headers["stripe-version"], "fixture.v1");
        assert!(
            headers["authorization"]
                .to_str()
                .unwrap()
                .starts_with("Bearer rk_test_")
        );
        let encoded = std::str::from_utf8(&bytes).unwrap();
        let fields = reqwest::Url::parse(&format!("https://fixture.invalid/?{encoded}"))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect::<BTreeMap<_, _>>();
        let idempotency = headers["idempotency-key"].to_str().unwrap().to_string();
        f.requests
            .lock()
            .unwrap()
            .push((uri.path().into(), fields.clone(), idempotency));
        if uri.path() == "/v1/customers" {
            assert_eq!(fields.len(), 1);
            return axum::Json(
                json!({"object":"customer", "id":"cus_fixture", "livemode":false,
                "metadata":{"oa_customer":fields["metadata[oa_customer]"]}}),
            );
        }
        assert_eq!(fields.len(), 14);
        assert_eq!(fields["mode"], "payment");
        assert_eq!(fields["payment_method_types[0]"], "card");
        assert_eq!(fields["payment_intent_data[capture_method]"], "automatic");
        assert_eq!(
            fields["payment_intent_data[metadata][oa_quote]"],
            fields["metadata[oa_quote]"]
        );
        assert_eq!(fields["line_items[0][price_data][currency]"], "usd");
        assert_eq!(fields["line_items[0][quantity]"], "1");
        let fault = f.fault.load(Ordering::SeqCst);
        let amount = fields["line_items[0][price_data][unit_amount]"]
            .parse::<u64>()
            .unwrap();
        axum::Json(
            json!({"object":"checkout.session", "id":"cs_test_fixture", "livemode":fault==2,
            "mode":"payment", "customer":fields["customer"], "currency":"usd",
            "amount_total":if fault==1 {amount+1} else {amount},
            "client_reference_id":fields["client_reference_id"],
            "metadata":{"oa_quote":fields["metadata[oa_quote]"]},
            "expires_at":fields["expires_at"].parse::<u64>().unwrap(),
            "subscription":null, "setup_intent":null,
            "url":if fault==3 {"https://unrelated.invalid/checkout"} else if fault==4 {"https://checkout.stripe.com/c/pay/cs_test_other"} else {"https://checkout.stripe.com/c/pay/cs_test_fixture#native-fragment"}}),
        )
    }
    let fixture = Fixture {
        requests: Arc::new(Mutex::new(Vec::new())),
        fault: Arc::new(AtomicUsize::new(0)),
    };
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async { axum::Json(json!({"object":"account","id":"acct_fixture"})) }),
        )
        .route("/v1/customers", post(create))
        .route("/v1/checkout/sessions", post(create))
        .with_state(fixture.clone());
    let (origin, task) = server(router).await;
    let material = secret()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut client =
        Stripe::new_for_mode(format!("rk_test_{material}"), "fixture.v1".into(), false).unwrap();
    client.origin = origin;
    let customer = "customer_original_fixture";
    assert!(
        client
            .create_customer(customer, "customer_request_fixture")
            .await
            .is_err()
    );
    assert!(fixture.requests.lock().unwrap().is_empty());
    assert!(client.bind_account("acct_other").await.is_err());
    client.bind_account("acct_fixture").await.unwrap();
    assert!(client.bind_account("acct_other").await.is_err());
    let value = client
        .create_customer(customer, "customer_request_fixture")
        .await
        .unwrap();
    assert_eq!(value["id"], "cus_fixture");
    let mut request = CheckoutRequest {
        customer: "cus_fixture".into(),
        quote: "checkout_quote_fixture".into(),
        amount_cents: 100,
        expires_at: 4000,
        return_origin: "https://fixture.invalid".into(),
        idempotency: "checkout_request_fixture".into(),
    };
    let first = client.create_checkout(&request, 100).await.unwrap();
    let repeated = client.create_checkout(&request, 100).await.unwrap();
    assert_eq!(first, repeated);
    let captured = fixture.requests.lock().unwrap().clone();
    assert_eq!(captured[1], captured[2]);
    assert_eq!(captured[1].2, "checkout_request_fixture");
    assert_eq!(
        captured[1].1["success_url"],
        "https://fixture.invalid/dashboard?funding=checkout_quote_fixture"
    );
    request.amount_cents = 49;
    assert!(client.create_checkout(&request, 100).await.is_err());
    request.amount_cents = 100;
    request.expires_at = 101;
    assert!(client.create_checkout(&request, 100).await.is_err());
    request.expires_at = 4000;
    request.return_origin = "https://other.invalid/?target=private".into();
    assert!(client.create_checkout(&request, 100).await.is_err());
    assert_eq!(fixture.requests.lock().unwrap().len(), captured.len());
    request.return_origin = "https://fixture.invalid".into();
    for fault in [1, 2, 3, 4] {
        fixture.fault.store(fault, Ordering::SeqCst);
        assert!(client.create_checkout(&request, 100).await.is_err());
    }
    assert!(
        Stripe::new_for_mode(format!("rk_test_{material}"), "fixture.v1".into(), true).is_err()
    );
    assert!(
        Stripe::new_for_mode(format!("rk_live_{material}"), "fixture.v1".into(), false).is_err()
    );
    assert!(Stripe::new_for_mode(material, "fixture.v1".into(), false).is_err());
    task.abort();
}
