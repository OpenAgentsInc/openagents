use axum::{
    Json, Router,
    routing::{get, post},
};
use openagents_sdk::{Client, Deployment};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn address_management_uses_the_account_session_and_only_wallet_proofs() {
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let enrollment = requests.clone();
    let challenge = requests.clone();
    let token = format!("oa_sk_{}", "a".repeat(64));
    let expected = token.clone();
    let router = Router::new()
        .route("/v1/account", get(|| async { Json(json!({"account":{"id":"alice","label":"Alice"},"workspaces":[]})) }))
        .route("/v1/device/lightning-address", get(|| async { Json(json!({"configured":true,"addresses":[]})) }).put(move |headers: axum::http::HeaderMap, Json(body): Json<Value>| {
            let requests = enrollment.clone(); let expected = expected.clone();
            async move {
                assert_eq!(headers["authorization"], format!("Bearer {expected}"));
                requests.lock().unwrap().push(body);
                Json(json!({"name":"alice","account":"alice","nostr_pubkey":"11".repeat(32),"spark_pubkey":"02abcd","domain":"openagents.com","revision":1,"state":"pending","created":1,"updated":1,"last_challenge":"challenge"}))
            }
        }))
        .route("/v1/device/lightning-address/challenge", post(move |Json(body): Json<Value>| {
            let requests = challenge.clone();
            async move {
                requests.lock().unwrap().push(body);
                Json(json!({"id":"challenge","account":"alice","name":"alice","domain":"openagents.com","operation":"enable","expected_revision":0,"expires_at":300,"message":"account-scoped challenge"}))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client.authenticate(&token).await.unwrap();
    assert!(
        client
            .lightning_addresses(&session)
            .await
            .unwrap()
            .configured
    );
    let challenge = client.lightning_challenge(&session, 0).await.unwrap();
    let address = client
        .enroll_lightning_address(&session, &challenge.id, "02abcd", "public-signature")
        .await
        .unwrap();
    assert_eq!(address.name, "alice");
    let requests = requests.lock().unwrap();
    assert_eq!(requests[0], json!({"expected_revision":0}));
    assert_eq!(
        requests[1],
        json!({"challenge":"challenge","pubkey":"02abcd","signature":"public-signature"})
    );
    server.abort();
}
