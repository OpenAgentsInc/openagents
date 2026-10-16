use axum::{
    Json, Router,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use coder_account::{
    Controller,
    history::ExistingChat,
    openagents::{
        ChatSource, ChatSummary, Client, CredentialRecord, CredentialStore, Deployment,
        StorageError,
    },
    reader::Reader,
    turns::Turns,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const CHAT: &str = "d75ef8a1-16ca-49b6-83be-000000000093";
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
    lost: AtomicBool,
    hold: AtomicBool,
    started: tokio::sync::Notify,
    attempts: AtomicUsize,
    ids: Mutex<HashSet<String>>,
}
async fn send(State(s): State<Arc<Server>>, Json(body): Json<Value>) -> Response {
    s.attempts.fetch_add(1, Ordering::SeqCst);
    s.ids
        .lock()
        .unwrap()
        .insert(body["request_id"].as_str().unwrap().into());
    s.started.notify_one();
    if s.hold.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    if s.lost.swap(false, Ordering::SeqCst) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":{"code":"unavailable","message":"Synthetic lost response"}})),
        )
            .into_response();
    }
    (StatusCode::ACCEPTED, Json(json!({"answering":true}))).into_response()
}
async fn events(Query(q): Query<HashMap<String, String>>) -> Response {
    let event =
        json!({"type":"done","request_id":q["request_id"],"outcome":"answered","revision":1});
    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        format!("data: {event}\n\n"),
    )
        .into_response()
}
struct Fixture {
    _root: tempfile::TempDir,
    _account: Arc<Controller>,
    reader: Arc<Reader>,
    turns: Arc<Turns>,
    server: Arc<Server>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.turns.clear();
        self.task.abort();
    }
}
async fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let server = Arc::new(Server::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router=Router::new().route("/v1/account",get(||async{Json(json!({"account":{"id":"synthetic","label":"Fixture"},"workspaces":[]}))}))
        .route("/v1/threads/{id}/messages",post(send)).route("/v1/threads/{id}/events",get(events))
        .route("/v1/history/{id}/messages",get(||async{Json(json!({"thread":{"id":CHAT,"title":"Cloud fixture","revision":1,"updated_unix":1,"pinned_unix":null,"archived_unix":null,"project":null,"source":"web","messages":0},"revision":1,"data":[],"next":null,"captures":[]}))})).with_state(server.clone());
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
    let lease = account.lease().unwrap();
    let chat = lease.client().chat(lease.session(), CHAT).unwrap();
    let reader = Reader::new();
    reader
        .open_latest(ExistingChat {
            chat: chat.clone(),
            summary: ChatSummary {
                chat,
                title: "Cloud fixture".into(),
                revision: 1,
                updated_unix: 1,
                pinned_unix: None,
                archived_unix: None,
                project: None,
                source: ChatSource::Web,
                messages: 0,
            },
            lease,
        })
        .unwrap();
    reader.more().await;
    reader.set_following(true);
    let turns = Turns::new(reader.clone());
    Fixture {
        _root: root,
        _account: account,
        reader,
        turns,
        server,
        task,
    }
}
#[tokio::test]
async fn composer_retry_keeps_one_send_after_an_uncertain_response() {
    let f = fixture().await;
    f.server.lost.store(true, Ordering::SeqCst);
    f.turns.submit("Hello".into()).await;
    assert!(f.turns.snapshot().retry);
    assert!(!f.turns.snapshot().busy);
    f.turns.retry().await;
    assert_eq!(f.server.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(f.server.ids.lock().unwrap().len(), 1);
    let snapshot = f.turns.snapshot();
    assert_eq!(snapshot.accepted, 1);
    assert!(!snapshot.busy && !snapshot.retry && snapshot.error.is_none());
}
#[tokio::test]
async fn pausing_keeps_retry_identity_and_selection_changes_reject_late_results() {
    let f = fixture().await;
    f.server.hold.store(true, Ordering::SeqCst);
    let job = tokio::spawn(f.turns.submit("Hello".into()));
    f.server.started.notified().await;
    f.turns.pause();
    job.await.unwrap();
    assert!(f.turns.snapshot().retry);
    f.server.hold.store(false, Ordering::SeqCst);
    f.turns.resume().await;
    assert_eq!(f.server.ids.lock().unwrap().len(), 1);
    assert_eq!(f.turns.snapshot().accepted, 1);
    f.server.hold.store(true, Ordering::SeqCst);
    let job = tokio::spawn(f.turns.submit("Another message".into()));
    f.server.started.notified().await;
    f.reader.clear();
    f.turns.clear();
    job.await.unwrap();
    assert!(f.turns.snapshot().chat.is_none());
    assert_eq!(f.turns.snapshot().accepted, 0);
}
