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
    assert!(client.get("../other", "ch_wrong").await.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    task.abort();
    other_task.abort();
}
