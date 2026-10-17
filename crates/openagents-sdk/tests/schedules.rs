use axum::{Json, Router, routing::get, routing::post};
use openagents_sdk::{CancellationToken, Client, Deployment, Error};
use serde_json::{Value, json};

#[tokio::test]
async fn lists_and_syncs_one_computers_schedules() {
    let router = Router::new()
        .route("/v1/account", get(|| async { Json(json!({"account":{"id":"acct_a","label":"Fixture"},"workspaces":[]})) }))
        .route("/v1/schedules", get(|| async {
            Json(json!({"schedules":[{"id":"prompt-aa","computer":"dev","prompt":"Check in.","every_secs":1200,"chat":"c-1","updated":5}],"computers":["dev"],"chats":[]}))
        }))
        .route("/v1/schedules/sync", post(|Json(body): Json<Value>| async move {
            assert_eq!(body["computer"], "dev");
            Json(json!({"schedules": body["schedules"]}))
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    let schedules = client.schedules(&session).unwrap();
    let cancel = CancellationToken::new();
    let listed = schedules.list(&cancel).await.unwrap();
    assert_eq!(listed[0].every_secs, Some(1200));
    assert_eq!(listed[0].chat.as_deref(), Some("c-1"));
    let mut mine = listed[0].clone();
    mine.paused = true;
    mine.updated = 6;
    let back = schedules
        .sync("dev", &[mine.clone()], &cancel)
        .await
        .unwrap();
    assert_eq!(back, vec![mine.clone()]);
    assert!(matches!(
        schedules.sync("other", &[mine], &cancel).await,
        Err(Error::InvalidReference)
    ));
    task.abort();
}
