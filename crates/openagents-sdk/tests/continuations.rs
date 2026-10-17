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
