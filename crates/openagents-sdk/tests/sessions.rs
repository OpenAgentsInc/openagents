use axum::{
    Json, Router,
    extract::Path,
    routing::{get, put},
};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Error,
    sessions::{self, CopiedMessage, CopiedRole, Home, PublishedSession, SessionStatus},
};
use serde_json::{Value, json};

const ONE: &str = "a1000000-0000-4000-8000-000000000001";
const CHAT: &str = "d75ef8a1-16ca-49b6-83be-000000000001";

fn row(id: &str) -> Value {
    json!({"id":id,"source":"session","home":"computer:test-mac","title":"Fix the badge","computer":"Test Mac",
        "project":null,"folder":"openagents","status":"working","last_activity_unix":5,"updated_unix":5,
        "recap":"Fixing it","online":true,"transcript":true,"app_link":null,"continued_by":null,"future_field":1})
}

#[tokio::test]
async fn sessions_publish_list_and_share_transcripts_through_the_account() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/v1/account", get(|| async { Json(json!({"account":{"id":"one","label":"Test account"},"workspaces":[]})) }))
        .route("/v1/history", get(|| async {
            Json(json!({"data":[],"next":null,"sessions":[row(ONE), {"id":"bad","home":"cloud"}]}))
        }))
        .route("/v1/history/sessions", get(|| async { Json(json!({"data":[row(ONE)]})) }))
        .route("/v1/history/sessions/{id}", get(|Path(id): Path<String>| async move {
            Json(json!({"session":row(&id),"messages":[{"role":"user","text":"Fix it"}]}))
        }))
        .route("/v1/history/sessions/computers/{device}", put(|Json(body): Json<Value>| async move {
            assert_eq!(body["name"], "Test Mac");
            assert_eq!(body["sessions"][0]["status"], "awaiting_input");
            assert_eq!(body["sessions"][0]["home_transfer"], true);
            assert_eq!(body["sessions"][0]["agent"], "Foreman");
            Json(json!({"transcripts":[body["sessions"][0]["id"]]}))
        }).delete(|| async { Json(json!({"removed":true})) }))
        .route("/v1/history/sessions/computers/{device}/moves", get(|| async {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            Json(json!({"moves":[{"source":ONE,"target":{"id":CHAT,"home":"cloud"},"expires_unix":now+600}]}))
        }))
        .route("/v1/history/sessions/{id}/transcript", put(|Json(body): Json<Value>| async move {
            Json(json!({"on":body["on"],"transcript":body["on"]}))
        }))
        .route("/v1/history/sessions/{id}/messages", put(|Json(body): Json<Value>| async move {
            Json(json!({"saved":body["messages"].as_array().unwrap().len()}))
        }));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "1".repeat(64)))
        .await
        .unwrap();
    let cancel = CancellationToken::new();

    let page = client
        .history(&session)
        .unwrap()
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert!(page.data.is_empty());
    assert_eq!(page.sessions.len(), 1, "an unreadable row is left out");
    assert_eq!(page.sessions[0].home, Home::Computer("test-mac".into()));
    assert_eq!(page.sessions[0].status, SessionStatus::Working);

    let rows = sessions::list(&client, &session, None, &cancel)
        .await
        .unwrap();
    assert_eq!(rows[0].computer, "Test Mac");
    assert!(!rows[0].home_transfer, "older summaries still decode");
    let detail = sessions::get(&client, &session, ONE, &cancel)
        .await
        .unwrap();
    assert_eq!(detail.messages.unwrap()[0].role, CopiedRole::User);

    let published = PublishedSession {
        id: ONE.into(),
        home_transfer: true,
        continues: None,
        title: "Fix the badge".into(),
        project: None,
        folder: None,
        status: SessionStatus::AwaitingInput,
        last_activity_unix: 5,
        recap: String::new(),
        workspace: None,
        agent: Some("Foreman".into()),
    };
    let on = sessions::publish(
        &client,
        &session,
        "test-mac",
        "Test Mac",
        std::slice::from_ref(&published),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(on, vec![ONE.to_owned()]);
    let moves = sessions::pending_moves(&client, &session, "test-mac", &cancel)
        .await
        .unwrap();
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].source, ONE);
    assert_eq!(moves[0].target.id, CHAT);
    assert!(matches!(
        sessions::publish(&client, &session, "not a device", "x", &[], &cancel).await,
        Err(Error::InvalidReference)
    ));
    assert!(
        sessions::set_transcript(&client, &session, ONE, true, &cancel)
            .await
            .unwrap()
    );
    let saved = sessions::upload_transcript(
        &client,
        &session,
        ONE,
        &[CopiedMessage {
            role: CopiedRole::Assistant,
            text: "Done".into(),
        }],
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(saved, 1);
    assert!(
        sessions::forget_computer(&client, &session, "test-mac", &cancel)
            .await
            .unwrap()
    );
    assert!(matches!(
        sessions::get(&client, &session, CHAT.strip_suffix('1').unwrap(), &cancel).await,
        Err(Error::InvalidReference)
    ));
    cancel.cancel();
    assert!(matches!(
        sessions::list(&client, &session, None, &cancel).await,
        Err(Error::Cancelled)
    ));
    task.abort();
}

#[tokio::test]
async fn indexed_read_ack_keeps_project_and_revision_and_rejects_false_confirmation() {
    use axum::extract::Query;
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route(
            "/v1/account",
            get(|| async {
                Json(json!({"account":{"id":"one","label":"Synthetic"},"workspaces":[]}))
            }),
        )
        .route(
            "/v1/history/sessions/{id}/seen",
            put(
                |Path(id): Path<String>,
                 Query(query): Query<HashMap<String, String>>,
                 Json(body): Json<Value>| async move {
                    assert_eq!(query["project"], "prj_shared");
                    assert_eq!(query["record"],"c".repeat(64));
                    assert_eq!(body["activity_revision"], "a".repeat(64));
                    Json(json!({"id":id,"record_key":"c".repeat(64),"activity_revision":if id==ONE {"b".repeat(64)} else {"a".repeat(64)},"unseen":false}))
                },
            ),
        );
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "1".repeat(64)))
        .await
        .unwrap();
    let mut value = row(ONE);
    value["activity_revision"] = json!("a".repeat(64));
    value["unseen"] = json!(true);
    value["record_key"] = json!("c".repeat(64));
    let summary = serde_json::from_value(value).unwrap();
    assert!(matches!(
        sessions::mark_seen(
            &client,
            &session,
            &summary,
            Some("prj_shared"),
            &CancellationToken::new()
        )
        .await,
        Err(Error::InvalidResponse)
    ));
    let mut confirmed = row(CHAT);
    confirmed["activity_revision"] = json!("a".repeat(64));
    confirmed["record_key"] = json!("c".repeat(64));
    let confirmed = serde_json::from_value(confirmed).unwrap();
    sessions::mark_seen(
        &client,
        &session,
        &confirmed,
        Some("prj_shared"),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    task.abort();
}
