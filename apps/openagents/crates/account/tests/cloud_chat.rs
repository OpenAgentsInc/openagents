//! #12026: the phone's cloud chat pins, copies, and archives its chat
//! through `PATCH /v1/threads/{id}`.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use openagents_account::{
    Controller,
    cloud_chat::CloudChat,
    openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
struct Credentials(Mutex<Option<CredentialRecord>>);
impl CredentialStore for Credentials {
    fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, r: &CredentialRecord) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(r.clone());
        Ok(())
    }
    fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[derive(Default)]
struct Server {
    broken: AtomicBool,
    patches: Mutex<Vec<(String, Value)>>,
}

async fn start(Json(body): Json<Value>) -> Response {
    (
        StatusCode::CREATED,
        Json(json!({"id": body["request_id"], "answering": true})),
    )
        .into_response()
}
async fn events(Query(q): Query<HashMap<String, String>>) -> Response {
    let text = json!({"type":"text","request_id":q["request_id"],"index":1,"role":"assistant","text":"Hi there.","replace":true});
    let done =
        json!({"type":"done","request_id":q["request_id"],"outcome":"answered","revision":1});
    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        format!("data: {text}\n\ndata: {done}\n\n"),
    )
        .into_response()
}
async fn file(
    State(s): State<Arc<Server>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    if s.broken.load(Ordering::SeqCst) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":{"code":"unavailable","message":"Synthetic outage"}})),
        )
            .into_response();
    }
    s.patches.lock().unwrap().push((id.clone(), body.clone()));
    Json(json!({"id": id, "object": "thread",
        "pinned": body["pinned"].as_bool().unwrap_or(false),
        "archived": body["archived"].as_bool().unwrap_or(false)}))
    .into_response()
}

#[tokio::test]
async fn pins_copies_and_archives_the_open_chat() {
    let root = tempfile::tempdir().unwrap();
    let server = Arc::new(Server::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async {
                Json(json!({"account":{"id":"synthetic","label":"Fixture"},"workspaces":[]}))
            }),
        )
        .route("/v1/threads", post(start))
        .route("/v1/threads/{id}", patch(file))
        .route("/v1/threads/{id}/events", get(events))
        .with_state(server.clone());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "1".repeat(64)))
        .await
        .unwrap();
    let store = Arc::new(Credentials::default());
    store.save(&session.record()).unwrap();
    let account = Controller::new(client, store, root.path().join("signed-out"));
    account.restore().await;
    let chat = CloudChat::new(account.clone());

    // Nothing sent yet: pinning waits, archiving only starts over.
    assert!(chat.set_pinned(true).await.is_err());
    assert!(chat.archive().await.is_ok());
    assert!(server.patches.lock().unwrap().is_empty());

    chat.send("Hello".into()).await;
    let id = chat.snapshot().chat.expect("the chat started");
    assert_eq!(
        chat.snapshot().transcript(),
        "You:\nHello\n\n---\n\nOpenAgents:\nHi there."
    );

    chat.set_pinned(true).await.unwrap();
    assert!(chat.snapshot().pinned);

    server.broken.store(true, Ordering::SeqCst);
    let error = chat.archive().await.unwrap_err();
    assert!(oa_copy::violations(&error, &[]).is_empty(), "{error}");
    assert_eq!(chat.snapshot().chat.as_deref(), Some(id.as_str()));
    server.broken.store(false, Ordering::SeqCst);

    chat.archive().await.unwrap();
    let after = chat.snapshot();
    assert!(after.chat.is_none() && after.lines.is_empty() && !after.pinned);
    assert_eq!(
        *server.patches.lock().unwrap(),
        [
            (id.clone(), json!({"pinned": true})),
            (id, json!({"archived": true})),
        ]
    );
    task.abort();
}
