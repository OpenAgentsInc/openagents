use axum::{Json, Router, routing::get};
use openagents_sdk::{Client, Deployment, profile_earnings::EarningsSource};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
#[tokio::test]
async fn only_explicit_selected_sources_are_sent_for_publication() {
    let party = "11".repeat(32);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let app=Router::new()
        .route("/v1/account",get(||async{Json(json!({"account":{"id":"test","label":"Test"},"workspaces":[]}))}))
        .route("/v1/device/earnings/{party}",get(||async{Json(json!({"rows":[]}))}))
        .route("/v1/device/earnings/{party}/receipts",axum::routing::post(move |h:axum::http::HeaderMap,Json(body):Json<Value>| {let seen=seen.clone();async move{assert!(h["authorization"].to_str().unwrap().starts_with("Bearer oa_sk_"));seen.lock().unwrap().push(body);Json(json!({"event":{"id":"00".repeat(32),"pubkey":"11".repeat(32),"created_at":1,"kind":30078,"tags":[],"content":"{}","sig":"00".repeat(64)}}))}}))
        .route("/v1/earnings/public/{party}",get(|h:axum::http::HeaderMap|async move{assert!(!h.contains_key("authorization"));Json(json!({"events":[],"issuers":[]}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    assert!(
        client
            .profile_earnings(&session, &party)
            .await
            .unwrap()
            .rows
            .is_empty()
    );
    client
        .publish_earning(
            &session,
            &party,
            &EarningsSource::Ledger {
                sequence: 1,
                role: "author".into(),
            },
        )
        .await
        .unwrap();
    client.published_earnings(&party).await.unwrap();
    assert!(
        client
            .profile_earnings(&session, "../account")
            .await
            .is_err()
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec![json!({"source":{"kind":"ledger","sequence":1,"role":"author"}})]
    );
    server.abort();
}
