use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use coder_account::{
    Controller,
    history::{ExistingChat, LoadState},
    openagents::{
        ChatSource, ChatSummary, Client, CredentialRecord, CredentialStore, Deployment,
        MessageRole, StorageError,
    },
    reader::{Limits, Reader},
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const FILE: &str = "0123456789abcdef0123456789abcdef";
fn id(n: usize) -> String {
    format!("{n:08x}-1234-4234-8234-123456789abc")
}
#[derive(Default)]
struct Store(Mutex<Option<CredentialRecord>>);
impl CredentialStore for Store {
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
    status: AtomicUsize,
    file_status: AtomicUsize,
    revision: AtomicUsize,
    reads: AtomicUsize,
    downloads: AtomicUsize,
    delayed: AtomicBool,
    file_delayed: AtomicBool,
    bad: AtomicBool,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
async fn messages(
    State(s): State<Arc<Server>>,
    Path(chat): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    s.reads.fetch_add(1, Ordering::SeqCst);
    if s.delayed.load(Ordering::SeqCst) && chat == id(1) {
        s.started.notify_one();
        s.release.notified().await;
    }
    let status = s.status.load(Ordering::SeqCst);
    if status != 0 {
        return StatusCode::from_u16(status as u16).unwrap().into_response();
    }
    let revision = s.revision.load(Ordering::SeqCst);
    if q.get("revision")
        .is_some_and(|r| r.parse::<usize>().unwrap() != revision)
    {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error":{"code":"changed"}})),
        )
            .into_response();
    }
    let start = q
        .get("after")
        .map(|s| s.strip_prefix("msg_").unwrap().parse::<usize>().unwrap() + 1)
        .unwrap_or(0);
    let limit = q["limit"].parse::<usize>().unwrap();
    assert!(limit <= 200);
    let end = (start + limit).min(405);
    let data:Vec<_>=(start..end).map(|n|json!({"id":format!("msg_{n}"),"role":match n{0=>"user",1=>"thinking",2=>"tool",3=>"future-role",_=>"assistant"},"text":if n==0{"# Saved question\n\n**Markdown** and `code`.\n\n```rust\nlet answer = 42;\n```".to_string()}else{format!("Saved message {n}")},"files":if s.bad.load(Ordering::SeqCst){json!("malformed")}else if n==0{json!([{"id":FILE,"name":"fixture.txt","kind":"text","size":8}])}else{json!([])},"tool":if n==2{Some(json!({"name":"read","summary":"Read a saved file","status":"done","input":"fixture.txt","output":"Full saved output"}))}else{None},"future_content":{"kind":"unknown","value":n},"generated_files":if n==3{json!("malformed additive content")}else{json!([])},"artifacts":[]})).collect();
    Json(json!({"thread":{"id":chat,"title":"Saved fixture chat","revision":revision,"updated_unix":1,"pinned_unix":null,"archived_unix":if chat==id(2){Some(1)}else{None},"project":"prj_0000000000000001","source":if chat==id(2){"phone"}else{"terminal"},"messages":405},"revision":revision,"data":data,"next":(end<405).then(||format!("msg_{}",end-1)),"captures":[{"id":id(555),"name":"Saved capture","state":"saved","size":3,"media":"text/plain"},{"id":id(556),"name":"Missing capture","state":"not_saved","size":null,"media":null}]})).into_response()
}
async fn file(State(s): State<Arc<Server>>) -> Response {
    s.downloads.fetch_add(1, Ordering::SeqCst);
    if s.file_delayed.load(Ordering::SeqCst) {
        s.started.notify_one();
        s.release.notified().await;
    }
    let status = s.file_status.load(Ordering::SeqCst);
    if status != 0 {
        return StatusCode::from_u16(status as u16).unwrap().into_response();
    }
    b"fixture!".to_vec().into_response()
}
async fn setup() -> (Arc<Controller>, Arc<Reader>, Arc<Server>, tempfile::TempDir) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = Arc::new(Server::default());
    server.revision.store(1, Ordering::SeqCst);
    let router = Router::new()
        .route(
            "/v1/account",
            get(|h: HeaderMap| async move {
                let acct = if h["authorization"].to_str().unwrap().ends_with('1') {
                    "acct_a"
                } else {
                    "acct_b"
                };
                Json(json!({"account":{"id":acct,"label":"Fixture"},"workspaces":[]}))
            }),
        )
        .route("/v1/history/{id}/messages", get(messages))
        .route("/v1/history/{id}/files/{file}", get(file))
        .route(
            "/v1/history/{id}/captures/{capture}",
            get(|| async { b"cap".to_vec() }),
        )
        .with_state(server.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "1".repeat(64)))
        .await
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::default());
    store.save(&session.record()).unwrap();
    let c = Controller::new(client, store, root.path().join("signed-out"));
    c.restore().await;
    (c, Reader::new(), server, root)
}
fn target(c: &Arc<Controller>, n: usize) -> ExistingChat {
    let lease = c.lease().unwrap();
    let chat = lease.client().chat(lease.session(), &id(n)).unwrap();
    ExistingChat {
        chat: chat.clone(),
        summary: ChatSummary {
            chat,
            title: "Existing target".into(),
            revision: 1,
            updated_unix: 1,
            pinned_unix: None,
            archived_unix: None,
            project: None,
            source: ChatSource::Web,
            messages: 0,
        },
        lease,
    }
}
#[tokio::test]
async fn direct_existing_target_reads_all_405_messages_and_downloads_only_on_demand() {
    let (c, r, s, _root) = setup().await;
    r.open(target(&c, 1)).unwrap();
    r.more().await;
    assert_eq!(r.snapshot().messages.len(), 200);
    assert_eq!(s.downloads.load(Ordering::SeqCst), 0);
    assert_eq!(r.snapshot().messages[1].role, MessageRole::Thinking);
    assert_eq!(
        r.snapshot().messages[2].tool.as_ref().unwrap().output,
        "Full saved output"
    );
    assert!(matches!(
        r.snapshot().messages[3].role,
        MessageRole::Other(_)
    ));
    assert!(r.snapshot().messages[3].extra["generated_files"].is_string());
    let reference = r.snapshot().messages[0].files[0]
        .download_ref()
        .unwrap()
        .clone();
    let epoch = r.snapshot().epoch;
    r.download(reference.clone(), "fixture.txt".into()).await;
    assert_eq!(&**r.bytes_for_save(epoch, &reference).unwrap(), b"fixture!");
    assert!(r.snapshot().captures[1].download_ref().is_none());
    let capture = r.snapshot().captures[0].download_ref().unwrap().clone();
    r.download(capture, "Saved capture".into()).await;
    assert_eq!(&**r.snapshot().download.unwrap().bytes.unwrap(), b"cap");
    r.more().await;
    r.more().await;
    let snap = r.snapshot();
    assert_eq!(snap.messages.len(), 405);
    assert_eq!(snap.messages.last().unwrap().id, "msg_404");
    assert!(!snap.has_more);
    assert_eq!(snap.summary.unwrap().source, ChatSource::Terminal);
    assert_eq!(s.reads.load(Ordering::SeqCst), 3);
    r.clear();
    assert!(r.snapshot().messages.is_empty());
    assert!(r.bytes_for_save(epoch, &reference).is_none());
}
#[tokio::test]
async fn changed_pages_never_mix_and_refresh_can_recover() {
    let (c, r, s, _root) = setup().await;
    r.open(target(&c, 1)).unwrap();
    r.more().await;
    s.revision.store(2, Ordering::SeqCst);
    r.more().await;
    assert!(r.snapshot().messages.is_empty());
    assert!(r.snapshot().reload_required);
    assert!(matches!(
        r.snapshot().state,
        LoadState::Failed { stale: false, .. }
    ));
    r.refresh().unwrap();
    r.more().await;
    assert_eq!(r.snapshot().messages.len(), 200);
    assert_eq!(r.snapshot().summary.unwrap().revision, 2);
    s.status.store(503, Ordering::SeqCst);
    r.more().await;
    assert!(matches!(
        r.snapshot().state,
        LoadState::Failed { stale: true, .. }
    ));
    assert_eq!(r.snapshot().messages.len(), 200);
    s.status.store(0, Ordering::SeqCst);
    r.more().await;
    assert_eq!(r.snapshot().messages.len(), 400);
    s.status.store(403, Ordering::SeqCst);
    r.more().await;
    assert!(r.snapshot().access_lost);
    assert!(r.snapshot().messages.is_empty());
    assert!(r.snapshot().summary.is_none());
}
#[tokio::test]
async fn late_message_and_file_responses_cannot_replace_a_new_selection_or_logout() {
    let (c, r, s, _root) = setup().await;
    r.open(target(&c, 1)).unwrap();
    s.delayed.store(true, Ordering::SeqCst);
    let pending = tokio::spawn(r.more());
    s.started.notified().await;
    r.open(target(&c, 2)).unwrap();
    r.more().await;
    s.release.notify_waiters();
    pending.await.unwrap();
    assert_eq!(r.snapshot().summary.unwrap().chat.id(), id(2));
    assert_eq!(r.snapshot().messages.len(), 200);
    let reference = r.snapshot().messages[0].files[0]
        .download_ref()
        .unwrap()
        .clone();
    let epoch = r.snapshot().epoch;
    s.file_delayed.store(true, Ordering::SeqCst);
    let pending = tokio::spawn(r.download(reference.clone(), "fixture.txt".into()));
    s.started.notified().await;
    c.invalidate();
    r.clear();
    s.release.notify_waiters();
    pending.await.unwrap();
    assert!(r.snapshot().messages.is_empty());
    assert!(r.bytes_for_save(epoch, &reference).is_none());
}
#[tokio::test]
async fn explicit_windows_reach_the_end_without_silently_truncating() {
    let (c, _reader, s, _root) = setup().await;
    let r = Reader::with_limits(Limits {
        bytes: 32 * 1024 * 1024,
        messages: 200,
    });
    r.open(target(&c, 1)).unwrap();
    r.more().await;
    r.more().await;
    assert!(r.snapshot().continue_required);
    assert_eq!(s.reads.load(Ordering::SeqCst), 1);
    r.continue_window();
    r.more().await;
    assert_eq!(r.snapshot().first, 200);
    assert_eq!(r.snapshot().messages.len(), 200);
    r.more().await;
    r.continue_window();
    r.more().await;
    assert_eq!(r.snapshot().first, 400);
    assert_eq!(r.snapshot().received, 405);
    assert_eq!(r.snapshot().messages.len(), 5);
    assert!(!r.snapshot().has_more);
}
#[tokio::test]
async fn large_pages_offer_smaller_reads_and_malformed_known_content_is_an_error() {
    let (c, _r, s, _root) = setup().await;
    let r = Reader::with_limits(Limits {
        bytes: 5000,
        messages: 2000,
    });
    r.open(target(&c, 1)).unwrap();
    r.more().await;
    assert!(r.snapshot().smaller_page);
    assert!(r.snapshot().messages.is_empty());
    r.smaller_page();
    r.more().await;
    assert_eq!(r.snapshot().messages.len(), 1);
    s.bad.store(true, Ordering::SeqCst);
    r.refresh().unwrap();
    r.more().await;
    assert!(matches!(r.snapshot().state, LoadState::Failed { .. }));
}
#[tokio::test]
async fn missing_files_preserve_the_chat_but_revoked_access_clears_it() {
    let (c, r, s, _root) = setup().await;
    r.open(target(&c, 1)).unwrap();
    r.more().await;
    let reference = r.snapshot().messages[0].files[0]
        .download_ref()
        .unwrap()
        .clone();
    s.file_status.store(404, Ordering::SeqCst);
    r.download(reference.clone(), "fixture.txt".into()).await;
    assert_eq!(r.snapshot().messages.len(), 200);
    assert!(r.snapshot().download.unwrap().bytes.is_none());
    s.file_status.store(403, Ordering::SeqCst);
    r.download(reference, "fixture.txt".into()).await;
    assert!(r.snapshot().messages.is_empty());
    assert!(r.snapshot().access_lost);
    assert!(r.snapshot().download.is_none());
}
