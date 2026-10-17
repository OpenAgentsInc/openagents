use axum::{
    Json, Router,
    routing::{get, post},
};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Error,
    continuations::{self, Claim, Home, PartRef},
};
use serde_json::{Value, json};
const SOURCE: &str = "d75ef8a1-16ca-49b6-83be-000000000001";
const TARGET: &str = "d75ef8a1-16ca-49b6-83be-000000000002";

#[tokio::test]
async fn claim_and_recovery_validate_the_account_and_destination() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app=Router::new()
        .route("/v1/account",get(||async{Json(json!({"account":{"id":"one","label":"Test account"},"workspaces":[]}))}))
        .route("/v1/threads/{id}/continue",post(|Json(request):Json<Value>|async move {
            Json(json!({"account":"one","source":{"id":SOURCE,"home":"cloud","future_field":true},"target":request["target"],"seed":{"title":"Earlier chat","summary":"A reading test","recent":[{"role":"user","text":"The fictional flower is coral orchid"}],"future_field":true},"future_field":true}))
        }))
        .route("/v1/threads/{id}",get(||async{Json(json!({"thread":{"id":SOURCE,"continued_by":{"id":TARGET,"home":"computer:test-mac","future_field":true}}}))}));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "1".repeat(64)))
        .await
        .unwrap();
    let source = client.chat(&session, SOURCE).unwrap();
    let cancel = CancellationToken::new();
    let claim = Claim {
        revision: 7,
        target: PartRef {
            id: TARGET.into(),
            home: Home::Computer("test-mac".into()),
        },
    };
    let response = continuations::claim(&client, &session, &source, &claim, &cancel)
        .await
        .unwrap();
    assert_eq!(response.account, "one");
    assert_eq!(response.target, claim.target);
    assert_eq!(
        continuations::destination(&client, &session, &source, &cancel)
            .await
            .unwrap(),
        Some(claim.target)
    );
    let wrong_chat = client.chat(&session, TARGET).unwrap();
    assert!(matches!(
        continuations::claim(
            &client,
            &session,
            &wrong_chat,
            &Claim {
                revision: 1,
                target: PartRef {
                    id: SOURCE.into(),
                    home: Home::Computer("test-mac".into())
                }
            },
            &cancel
        )
        .await,
        Err(Error::InvalidResponse)
    ));
    assert!(matches!(
        continuations::destination(&client, &session, &wrong_chat, &cancel).await,
        Err(Error::InvalidResponse)
    ));
    cancel.cancel();
    assert!(matches!(
        continuations::destination(&client, &session, &source, &cancel).await,
        Err(Error::Cancelled)
    ));
    task.abort();
}

#[tokio::test]
async fn computer_sources_recover_through_the_session_index_and_transfer_to_cloud() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/v1/account", get(|| async {Json(json!({"account":{"id":"one","label":"Test account"},"workspaces":[]}))}))
        .route("/v1/threads/{id}", get(|| async {(axum::http::StatusCode::NOT_FOUND, Json(json!({"error":{"code":"not_found","message":"No such chat."}})))}))
        .route("/v1/history/sessions/{id}", get(|| async {Json(json!({"session":{
            "id":SOURCE,"source":"session","home":"computer:test-mac","title":"Earlier work","computer":"Test Mac",
            "project":null,"folder":null,"status":"idle","last_activity_unix":7,"updated_unix":7,
            "recap":"","online":true,"transcript":false,"app_link":null,"continued_by":{"id":TARGET,"home":"cloud"}
        },"messages":null}))}))
        .route("/v1/threads/{id}/continue", post(|Json(request): Json<Value>| async move {
            assert_eq!(request["target"]["home"], "cloud");
            assert_eq!(request["seed"]["recent"][0]["text"], "coral orchid");
            Json(json!({"account":"one","source":{"id":SOURCE,"home":"computer:test-mac"},"target":request["target"],"seed":request["seed"]}))
        }));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "1".repeat(64)))
        .await
        .unwrap();
    let source = client.chat(&session, SOURCE).unwrap();
    let cancel = CancellationToken::new();
    let target = PartRef {
        id: TARGET.into(),
        home: Home::Cloud,
    };
    assert_eq!(
        continuations::destination(&client, &session, &source, &cancel)
            .await
            .unwrap(),
        Some(target.clone())
    );
    let moved = continuations::transfer(
        &client,
        &session,
        &source,
        &continuations::TransferClaim {
            revision: 7,
            target: target.clone(),
            seed: Some(continuations::Seed::new(
                "Earlier work",
                [(continuations::Role::User, "coral orchid")],
            )),
        },
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(moved.target, target);
    assert_eq!(moved.source.home, Home::Computer("test-mac".into()));
    task.abort();
}
