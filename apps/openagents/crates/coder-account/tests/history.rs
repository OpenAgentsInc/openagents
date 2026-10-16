use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use coder_account::{
    Controller,
    history::{History, LoadState},
    openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError},
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
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
    count: AtomicUsize,
    shared_count: AtomicUsize,
    projects: Mutex<Vec<serde_json::Value>>,
    reads: AtomicUsize,
    delayed: AtomicBool,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
fn id(n: usize) -> String {
    format!("{n:08x}-1234-4234-8234-123456789abc")
}
async fn list(State(s): State<Arc<Server>>, Query(q): Query<HashMap<String, String>>) -> Response {
    s.reads.fetch_add(1, Ordering::SeqCst);
    let status = s.status.load(Ordering::SeqCst);
    if s.delayed.load(Ordering::SeqCst) {
        s.started.notify_one();
        s.release.notified().await;
    }
    if status != 0 {
        return (
            StatusCode::from_u16(status as u16).unwrap(),
            Json(json!({"error":{"code":"unavailable"}})),
        )
            .into_response();
    }
    let limit: usize = q["limit"].parse().unwrap();
    assert!(limit <= 200);
    let start = q
        .get("after")
        .map(|s| usize::from_str_radix(&s[..8], 16).unwrap() + 1)
        .unwrap_or(0);
    let count = if q.contains_key("project") {
        s.shared_count.load(Ordering::SeqCst)
    } else {
        s.count.load(Ordering::SeqCst)
    };
    let end = (start + limit).min(count);
    let rows:Vec<_>=(start..end).map(|n|json!({"id":id(n),"title":"Same title","revision":1,"updated_unix":n+1,"pinned_unix":if n==204{Some(1)}else{None},"archived_unix":if n==5{Some(1)}else{None},"project":"prj_0000000000000001","source":match n%3{0=>"web",1=>"terminal",_=>"phone"},"messages":405})).collect();
    Json(json!({"data":rows,"next":(end<count).then(||id(end-1))})).into_response()
}
async fn setup() -> (
    Arc<Controller>,
    Arc<History>,
    Arc<Server>,
    tempfile::TempDir,
    Client,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = Arc::new(Server::default());
    server.count.store(205, Ordering::SeqCst);
    server.shared_count.store(205, Ordering::SeqCst);
    *server.projects.lock().unwrap() =
        vec![json!({"id":"prj_0000000000000001","name":"Shared project","shared":true})];
    let router = Router::new()
        .route(
            "/v1/account",
            get(|h: HeaderMap| async move {
                let account = if h["authorization"].to_str().unwrap().ends_with('1') {
                    "acct_a"
                } else {
                    "acct_b"
                };
                Json(json!({"account":{"id":account,"label":account},"workspaces":[]}))
            }),
        )
        .route("/v1/history", get(list))
        .route(
            "/v1/history/projects",
            get(|State(s): State<Arc<Server>>| async move {
                Json(json!({"data":s.projects.lock().unwrap().clone(),"next":null}))
            }),
        )
        .with_state(server.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    let c = account(&client, root.path(), 1).await;
    let history = History::new();
    history.reset(c.lease());
    (c, history, server, root, client)
}
async fn account(client: &Client, root: &std::path::Path, n: u8) -> Arc<Controller> {
    let session = client
        .authenticate(&format!("oa_sk_{}", n.to_string().repeat(64)))
        .await
        .unwrap();
    let store = Arc::new(Store::default());
    store.save(&session.record()).unwrap();
    let c = Controller::new(
        client.clone(),
        store,
        root.join(format!("account-{n}/signed-out")),
    );
    c.restore().await;
    c
}
#[tokio::test]
async fn bounded_pages_pin_archive_project_and_existing_navigation_never_write() {
    let (c, h, s, _, _) = setup().await;
    h.more_projects().await;
    h.more().await;
    assert_eq!(h.snapshot().rows.len(), 200);
    assert!(h.snapshot().has_more);
    assert_eq!(s.reads.load(Ordering::SeqCst), 1);
    h.more().await;
    let snap = h.snapshot();
    assert_eq!(snap.rows.len(), 205);
    assert!(!snap.has_more);
    assert_eq!(snap.rows[0].chat.id(), id(204));
    assert_eq!(snap.rows.last().unwrap().chat.id(), id(5));
    assert_eq!(snap.projects[0].name, "Shared project");
    let target = h.existing(&id(204)).unwrap();
    assert!(target.is_current());
    assert_eq!(target.chat.account(), "acct_a");
    assert_eq!(s.reads.load(Ordering::SeqCst), 2);
    h.more().await;
    assert_eq!(s.reads.load(Ordering::SeqCst), 2);
    // Opening a shared index is another read, with no host or task route mounted.
    h.project(Some(&snap.projects[0])).unwrap();
    h.more().await;
    assert_eq!(h.snapshot().rows.len(), 200);
    c.invalidate();
    h.reset(None);
    assert!(!target.is_current());
    assert!(h.snapshot().rows.is_empty());
    assert!(h.existing(&id(0)).is_none());
}
#[tokio::test]
async fn refresh_replaces_deleted_rows_and_outage_is_never_an_empty_account() {
    let (_account, h, s, _root, _) = setup().await;
    h.more().await;
    h.more().await;
    s.status.store(503, Ordering::SeqCst);
    h.refresh().unwrap();
    h.more().await;
    assert!(matches!(
        h.snapshot().state,
        LoadState::Failed { stale: true, .. }
    ));
    assert_eq!(h.snapshot().rows.len(), 205);
    s.status.store(0, Ordering::SeqCst);
    s.count.store(2, Ordering::SeqCst);
    h.more().await;
    assert_eq!(h.snapshot().rows.len(), 2);
    assert!(h.existing(&id(204)).is_none());
    assert_eq!(h.snapshot().state, LoadState::Ready);
    s.status.store(403, Ordering::SeqCst);
    h.refresh().unwrap();
    h.more().await;
    assert!(h.snapshot().rows.is_empty());
    assert!(matches!(
        h.snapshot().state,
        LoadState::Failed { stale: false, .. }
    ));
}
#[tokio::test]
async fn late_pages_cannot_refill_a_signed_out_or_different_account() {
    let (a, h, s, root, client) = setup().await;
    s.delayed.store(true, Ordering::SeqCst);
    let pending = tokio::spawn(h.more());
    s.started.notified().await;
    a.invalidate();
    h.reset(None);
    let b = account(&client, root.path(), 2).await;
    h.reset(b.lease());
    s.delayed.store(false, Ordering::SeqCst);
    s.release.notify_waiters();
    pending.await.unwrap();
    assert!(h.snapshot().rows.is_empty());
    h.more().await;
    assert!(
        h.snapshot()
            .rows
            .iter()
            .all(|r| r.chat.account() == "acct_b")
    );
    let target = h.existing(&id(0)).unwrap();
    h.reset(None);
    assert!(h.snapshot().rows.is_empty());
    // Disposing only the history view does not cancel the app's own login.
    assert!(target.lease.is_current());
    assert!(b.lease().is_some());
}

#[tokio::test]
async fn removed_shared_access_clears_its_rows_and_name() {
    let (_account, h, s, _root, _) = setup().await;
    h.more_projects().await;
    h.project(Some(&h.snapshot().projects[0])).unwrap();
    h.more().await;
    assert!(!h.snapshot().rows.is_empty());
    s.status.store(403, Ordering::SeqCst);
    h.refresh().unwrap();
    h.more().await;
    let snapshot = h.snapshot();
    assert!(snapshot.rows.is_empty());
    assert!(snapshot.project.is_none());
    assert!(snapshot.projects.is_empty());
    assert!(matches!(
        snapshot.state,
        LoadState::Failed { stale: false, .. }
    ));
}

#[tokio::test]
async fn a_new_account_lists_shared_projects_without_owned_chats_or_computers() {
    let (_controller, h, s, _root, _client) = setup().await;
    // This fixture mounts only the account and history APIs, with no host.
    s.count.store(0, Ordering::SeqCst);
    s.shared_count.store(2, Ordering::SeqCst);
    *s.projects.lock().unwrap() = vec![
        json!({"id":"prj_0000000000000001","name":"Studio","shared":true}),
        json!({"id":"prj_0000000000000002","name":"House","shared":true}),
    ];
    h.refresh_projects().unwrap();
    h.more_projects().await;
    h.more().await;
    let snapshot = h.snapshot();
    assert!(snapshot.rows.is_empty());
    assert_eq!(snapshot.projects.len(), 2);
    h.project(Some(&snapshot.projects[0])).unwrap();
    h.more().await;
    assert_eq!(h.snapshot().rows.len(), 2);
    s.shared_count.store(0, Ordering::SeqCst);
    h.project(Some(&snapshot.projects[1])).unwrap();
    h.more().await;
    assert_eq!(h.snapshot().project.as_ref().unwrap().name, "House");
    assert!(h.snapshot().rows.is_empty());
}
