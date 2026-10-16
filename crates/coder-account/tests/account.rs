use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use coder_account::{
    Controller, Phase,
    openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError},
};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[derive(Default)]
struct Store {
    record: Mutex<Option<CredentialRecord>>,
    deny_save: AtomicBool,
    deny_clear: AtomicBool,
    pause_save: AtomicBool,
    saving: AtomicBool,
}
impl CredentialStore for Store {
    fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        Ok(self.record.lock().unwrap().clone())
    }
    fn save(&self, r: &CredentialRecord) -> Result<(), StorageError> {
        if self.deny_save.load(Ordering::SeqCst) {
            return Err(StorageError::Denied);
        }
        self.saving.store(true, Ordering::SeqCst);
        while self.pause_save.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        *self.record.lock().unwrap() = Some(r.clone());
        Ok(())
    }
    fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
        if self.deny_clear.load(Ordering::SeqCst) {
            return Err(StorageError::Denied);
        }
        *self.record.lock().unwrap() = None;
        Ok(())
    }
}
#[derive(Default)]
struct Server {
    approved: AtomicBool,
    denied: AtomicBool,
    account_down: AtomicBool,
    signout_down: AtomicBool,
    revokes: AtomicUsize,
    issued: AtomicUsize,
    expires: AtomicUsize,
    active: Mutex<Vec<String>>,
    device_label: Mutex<Option<Value>>,
}
fn bearer(h: &HeaderMap) -> String {
    h.get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap()
        .to_owned()
}
async fn token(State(s): State<Arc<Server>>) -> Response {
    if s.denied.load(Ordering::SeqCst) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"access_denied"})),
        )
            .into_response();
    }
    if !s.approved.load(Ordering::SeqCst) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"authorization_pending"})),
        )
            .into_response();
    }
    let n = s.issued.fetch_add(1, Ordering::SeqCst) + 1;
    let token = format!("sess_{n:064x}");
    s.active.lock().unwrap().push(token.clone());
    let expires = match s.expires.load(Ordering::SeqCst) {
        0 => 30,
        n => n,
    };
    Json(json!({"access_token":token,"token_type":"Bearer","expires_in":expires,"account":{"id":"acct_fixture","label":"Actual account"}})).into_response()
}
async fn account(State(s): State<Arc<Server>>, h: HeaderMap) -> Response {
    if s.account_down.load(Ordering::SeqCst) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":{"code":"unavailable"}})),
        )
            .into_response();
    }
    if !s.active.lock().unwrap().contains(&bearer(&h)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({"account":{"id":"acct_fixture","label":"Actual account"},"workspaces":[]}))
        .into_response()
}
async fn revoke(State(s): State<Arc<Server>>, h: HeaderMap) -> Response {
    if s.signout_down.load(Ordering::SeqCst) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    s.revokes.fetch_add(1, Ordering::SeqCst);
    s.active.lock().unwrap().retain(|t| t != &bearer(&h));
    Json(json!({"signed_out":true})).into_response()
}
async fn setup() -> (
    Arc<Controller>,
    Arc<Server>,
    Arc<Store>,
    tempfile::TempDir,
    Client,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let code_origin = origin.clone();
    let server = Arc::new(Server::default());
    let router=Router::new().route("/v1/device/code",post(move|State(s): State<Arc<Server>>, Json(body): Json<Value>|{let origin=code_origin.clone();async move {*s.device_label.lock().unwrap()=Some(body);Json(json!({"device_code":"dvc_fixture","user_code":"BCDF-GHJK","verification_uri":format!("{origin}/device"),"verification_uri_complete":format!("{origin}/device?code=BCDF-GHJK"),"interval":1,"expires_in":30}))}}))
        .route("/v1/device/token",post(token)).route("/v1/account",get(account)).route("/v1/device/sign-out",post(revoke)).with_state(server.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::default());
    let controller = Controller::new(
        client.clone(),
        store.clone(),
        root.path().join("account/signed-out"),
    );
    (controller, server, store, root, client)
}
async fn phase(c: &Controller, p: impl Fn(&Phase) -> bool) {
    let mut watch = c.watch();
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            if p(&watch.borrow().phase) {
                return;
            }
            watch.changed().await.unwrap();
        }
    })
    .await
    .expect("account state did not change");
}
#[tokio::test]
async fn browser_approval_restores_the_actual_account_and_signs_out() {
    let (c, s, store, root, client) = setup().await;
    let task = tokio::spawn(c.sign_in("Fixture Mac".into()));
    phase(&c, |p| matches!(p, Phase::Waiting { .. })).await;
    match c.snapshot().phase {
        Phase::Waiting { code, url } => {
            assert_eq!(code, "BCDF-GHJK");
            assert!(url.contains("/device?code="));
        }
        _ => panic!(),
    }
    assert_eq!(
        s.device_label.lock().unwrap().as_ref().unwrap(),
        &json!({"app":"Coder Desktop", "computer":"Fixture Mac"})
    );
    s.approved.store(true, Ordering::SeqCst);
    task.await.unwrap();
    match c.snapshot().phase {
        Phase::SignedIn { account } => assert_eq!(account.label, "Actual account"),
        _ => panic!(),
    }
    let lease = c.lease().unwrap();
    assert!(c.accepts(&lease));
    let restarted = Controller::new(
        client,
        store.clone(),
        root.path().join("account/signed-out"),
    );
    restarted.restore().await;
    assert!(matches!(restarted.snapshot().phase, Phase::SignedIn { .. }));
    c.sign_out().await;
    assert!(!lease.is_current());
    assert!(store.record.lock().unwrap().is_none());
    assert_eq!(s.revokes.load(Ordering::SeqCst), 1);
    assert!(matches!(c.snapshot().phase, Phase::SignedOut { .. }));
}
#[tokio::test]
async fn cancellation_and_denial_do_not_leave_an_authenticated_view() {
    let (c, s, store, _, _) = setup().await;
    let task = tokio::spawn(c.sign_in("Fixture Mac".into()));
    phase(&c, |p| matches!(p, Phase::Waiting { .. })).await;
    let generation = c.snapshot().generation;
    c.clone().revalidate().await;
    assert_eq!(c.snapshot().generation, generation);
    assert!(matches!(c.snapshot().phase, Phase::Waiting { .. }));
    c.cancel().await;
    s.approved.store(true, Ordering::SeqCst);
    task.await.unwrap();
    assert!(c.lease().is_none());
    assert!(store.record.lock().unwrap().is_none());
    s.denied.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    assert!(matches!(
        c.snapshot().phase,
        Phase::Failed {
            can_restore: false,
            ..
        }
    ));
    assert!(c.lease().is_none());
}
#[tokio::test]
async fn storage_failure_revokes_the_new_session_and_does_not_save_it() {
    let (c, s, store, _, _) = setup().await;
    s.approved.store(true, Ordering::SeqCst);
    store.deny_save.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    assert!(matches!(c.snapshot().phase, Phase::Failed { .. }));
    assert!(c.lease().is_none());
    assert!(store.record.lock().unwrap().is_none());
    assert_eq!(s.revokes.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn account_outage_preserves_a_recoverable_credential_without_authorizing_reads() {
    let (c, s, store, _, _) = setup().await;
    s.approved.store(true, Ordering::SeqCst);
    s.account_down.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    assert!(matches!(
        c.snapshot().phase,
        Phase::Failed {
            can_restore: true,
            ..
        }
    ));
    assert!(store.record.lock().unwrap().is_some());
    assert!(c.lease().is_none());
    s.account_down.store(false, Ordering::SeqCst);
    c.restore().await;
    assert!(matches!(c.snapshot().phase, Phase::SignedIn { .. }));
    s.active.lock().unwrap().clear();
    c.restore().await;
    assert!(matches!(c.snapshot().phase, Phase::SignedOut { .. }));
    assert!(store.record.lock().unwrap().is_none());
}
#[tokio::test]
async fn offline_logout_and_refused_keychain_delete_never_restore_the_saved_login() {
    let (c, s, store, root, client) = setup().await;
    s.approved.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    let old = c.lease().unwrap();
    store.deny_clear.store(true, Ordering::SeqCst);
    s.signout_down.store(true, Ordering::SeqCst);
    c.sign_out().await;
    assert!(!old.is_current());
    assert!(matches!(
        c.snapshot().phase,
        Phase::SignedOut { notice: Some(_) }
    ));
    assert!(store.record.lock().unwrap().is_some());
    let restarted = Controller::new(client, store, root.path().join("account/signed-out"));
    restarted.restore().await;
    assert!(matches!(
        restarted.snapshot().phase,
        Phase::SignedOut { .. }
    ));
    assert!(restarted.lease().is_none());
}
#[tokio::test]
async fn an_old_restore_cannot_replace_a_new_logout() {
    let (c, s, store, _, _) = setup().await;
    s.approved.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    let restore = c.restore();
    let logout = c.sign_out();
    tokio::join!(restore, logout);
    assert!(matches!(c.snapshot().phase, Phase::SignedOut { .. }));
    assert!(c.lease().is_none());
    assert!(store.record.lock().unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn logout_during_a_blocked_save_clears_and_revokes_the_late_credential() {
    let (c, s, store, root, client) = setup().await;
    s.approved.store(true, Ordering::SeqCst);
    store.pause_save.store(true, Ordering::SeqCst);
    let login = tokio::spawn(c.sign_in("Fixture Mac".into()));
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        while !store.saving.load(Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let logout = tokio::spawn(c.sign_out());
    assert!(c.lease().is_none());
    store.pause_save.store(false, Ordering::SeqCst);
    login.await.unwrap();
    logout.await.unwrap();
    assert!(matches!(c.snapshot().phase, Phase::SignedOut { .. }));
    assert!(store.record.lock().unwrap().is_none());
    assert!(s.active.lock().unwrap().is_empty());
    assert_eq!(s.revokes.load(Ordering::SeqCst), 1);
    let restarted = Controller::new(client, store, root.path().join("account/signed-out"));
    restarted.restore().await;
    assert!(restarted.lease().is_none());
}
#[tokio::test]
async fn an_expired_saved_session_requires_login_again() {
    let (c, s, store, _, _) = setup().await;
    s.approved.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    let mut record = serde_json::to_value(store.record.lock().unwrap().as_ref().unwrap()).unwrap();
    record["expires_at"] = json!(1);
    *store.record.lock().unwrap() = Some(serde_json::from_value(record).unwrap());
    c.restore().await;
    assert!(matches!(c.snapshot().phase, Phase::SignedOut { .. }));
    assert!(c.lease().is_none());
    assert!(store.record.lock().unwrap().is_none());
}

#[tokio::test]
async fn logout_revokes_a_saved_login_when_account_discovery_is_down() {
    let (c, s, store, root, client) = setup().await;
    s.approved.store(true, Ordering::SeqCst);
    s.account_down.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    assert!(c.lease().is_none());
    assert!(store.record.lock().unwrap().is_some());
    // A restarted app also has only a saved credential, with no authorized lease.
    let restarted = Controller::new(
        client,
        store.clone(),
        root.path().join("account/signed-out"),
    );
    restarted.restore().await;
    assert!(restarted.lease().is_none());
    restarted.sign_out().await;
    assert!(store.record.lock().unwrap().is_none());
    assert!(s.active.lock().unwrap().is_empty());
    assert_eq!(s.revokes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn dropping_logout_before_io_finishes_still_prevents_restore() {
    let (c, s, store, root, client) = setup().await;
    s.approved.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    drop(c.sign_out());
    assert!(c.lease().is_none());
    let restarted = Controller::new(
        client,
        store.clone(),
        root.path().join("account/signed-out"),
    );
    restarted.restore().await;
    assert!(restarted.lease().is_none());
    assert!(matches!(
        restarted.snapshot().phase,
        Phase::SignedOut { .. }
    ));
    assert!(store.record.lock().unwrap().is_some());
}

#[tokio::test]
async fn live_leases_stop_authorizing_when_the_app_session_expires() {
    let (c, s, _, _, _) = setup().await;
    s.expires.store(2, Ordering::SeqCst);
    s.approved.store(true, Ordering::SeqCst);
    c.sign_in("Fixture Mac".into()).await;
    let lease = c.lease().unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    assert!(!lease.is_current());
    assert!(!c.accepts(&lease));
    assert!(c.lease().is_none());
    c.restore().await;
    assert!(matches!(c.snapshot().phase, Phase::SignedOut { .. }));
}

#[tokio::test]
async fn revalidation_preserves_outages_but_clears_revocation() {
    let (controller, server, store, _root, _client) = setup().await;
    server.approved.store(true, Ordering::SeqCst);
    controller.sign_in("Fixture device".into()).await;
    assert!(controller.lease().is_some());
    server.account_down.store(true, Ordering::SeqCst);
    controller.clone().revalidate().await;
    assert!(controller.lease().is_some());
    assert!(store.record.lock().unwrap().is_some());
    server.account_down.store(false, Ordering::SeqCst);
    server.active.lock().unwrap().clear();
    controller.clone().revalidate().await;
    assert!(matches!(
        controller.snapshot().phase,
        Phase::SignedOut { .. }
    ));
    assert!(store.record.lock().unwrap().is_none());
}

#[tokio::test]
async fn explicit_credentials_use_the_same_storage_and_sign_out_lifecycle() {
    let (controller, server, store, _root, _client) = setup().await;
    let credential = format!("sess_{:064x}", 9001);
    server.active.lock().unwrap().push(credential.clone());
    controller.import_credential(credential).await;
    assert!(controller.lease().is_some());
    assert!(store.record.lock().unwrap().is_some());
    controller.sign_out().await;
    assert!(controller.lease().is_none());
    assert!(store.record.lock().unwrap().is_none());
    controller
        .import_credential("not-a-credential".into())
        .await;
    assert!(controller.lease().is_none());
    assert!(store.record.lock().unwrap().is_none());
    assert!(matches!(
        controller.snapshot().phase,
        Phase::Failed {
            can_restore: false,
            ..
        }
    ));
}
