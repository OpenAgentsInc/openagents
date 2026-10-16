#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use axum::{
    Json, Router,
    body::Body,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use openagents_sdk::{
    CancellationToken, Client, CredentialRecord, CredentialStore, Deployment, Error, Poll,
    StorageError,
};
use serde_json::{Value, json};

fn token() -> String {
    format!("sess_{}", "a".repeat(64))
}
fn key() -> String {
    format!("oa_sk_{}", "b".repeat(64))
}
fn view() -> Value {
    json!({"account":{"id":"acct_fixture","label":"Existing account"},"workspaces":[{"id":"ws_fixture","name":"Personal","role":"owner","kind":"personal","tenant":"signup"}]})
}

#[derive(Clone, Copy)]
enum Case {
    Signed,
    PendingSlow,
    Denied,
    Expired,
    EarlyExpiry,
    Lost,
    UnsafeLink,
}

async fn mocked(case: Case, account_status: Arc<AtomicUsize>) -> (Client, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let link_origin = origin.clone();
    let polls = Arc::new(AtomicUsize::new(0));
    let counted = polls.clone();
    let site = Router::new()
        .route("/v1/device/code", post(move || {
            let origin = link_origin.clone();
            async move {
                Json(json!({"device_code":"dvc_private_fixture", "user_code":"BCDF-GHJK", "verification_uri":format!("{origin}/device"),
                    "verification_uri_complete": if matches!(case,Case::UnsafeLink) { "https://other.example/device?code=BCDF-GHJK".into() } else { format!("{origin}/device?code=BCDF-GHJK") },
                    "interval":1, "expires_in":if matches!(case,Case::EarlyExpiry) { 1 } else { 30 }}))
            }
        }))
        .route("/v1/device/token", post(move |Json(body): Json<Value>| {
            let count = counted.fetch_add(1, Ordering::SeqCst);
            async move {
                assert_eq!(body["device_code"], "dvc_private_fixture");
                if matches!(case,Case::Lost) { tokio::time::sleep(Duration::from_millis(200)).await; }
                let error = match (case,count) {
                    (Case::PendingSlow,0) => Some(json!({"error":"authorization_pending"})),
                    (Case::PendingSlow,1) => Some(json!({"error":"slow_down","interval":2})),
                    (Case::Denied,_) => Some(json!({"error":"access_denied"})),
                    (Case::Expired,_) => Some(json!({"error":"expired_token"})),
                    _ => None,
                };
                if let Some(error) = error { return (StatusCode::BAD_REQUEST,Json(error)).into_response(); }
                Json(json!({"access_token":token(), "token_type":"Bearer", "expires_in":2592000, "account":{"id":"acct_fixture","label":"Existing account"}})).into_response()
            }
        }))
        .route("/v1/account", get(move |headers: HeaderMap| {
            let status = account_status.load(Ordering::SeqCst);
            async move {
                assert!(headers.get("authorization").is_some());
                match status {
                    200 => Json(view()).into_response(),
                    429 => (StatusCode::TOO_MANY_REQUESTS,[("retry-after","2")],Json(json!({"error":{"message":token()}}))).into_response(),
                    500 => (StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":{"message":token()}}))).into_response(),
                    401 => (StatusCode::UNAUTHORIZED,Json(json!({"error":{"code":"signed_out"}}))).into_response(),
                    413 => Response::builder().status(200).header("content-length",1024*1024+1).body(Body::from(vec![b'x';1024*1024+1])).unwrap(),
                    _ => (StatusCode::OK,format!("bad JSON {}",token())).into_response(),
                }
            }
        }))
        .route("/v1/device/sign-out",post(|| async { Json(json!({"signed_out":true})) }));
    tokio::spawn(async move { axum::serve(listener, site).await.unwrap() });
    (
        Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap(),
        polls,
    )
}

#[tokio::test]
async fn device_polling_obeys_intervals_slowdown_and_one_shot_completion() {
    let (client, polls) = mocked(Case::PendingSlow, Arc::new(AtomicUsize::new(200))).await;
    let mut login = client
        .start_login("Coder Desktop", "test-mac")
        .await
        .unwrap();
    assert!(!format!("{login:?}").contains("dvc_private_fixture"));
    assert!(matches!(
        client.poll_login(&mut login).await.unwrap(),
        Poll::Pending { .. }
    ));
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    tokio::time::sleep(login.next_poll_in()).await;
    assert!(matches!(
        client.poll_login(&mut login).await.unwrap(),
        Poll::Pending { .. }
    ));
    tokio::time::sleep(login.next_poll_in()).await;
    assert!(
        matches!(client.poll_login(&mut login).await.unwrap(),Poll::SlowDown{retry_after} if retry_after==Duration::from_secs(2))
    );
    assert!(matches!(
        client.poll_login(&mut login).await.unwrap(),
        Poll::Pending { .. }
    ));
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    let session = client
        .wait_for_login(&mut login, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(session.account().id, "acct_fixture");
    assert!(!format!("{session:?}").contains(&token()));
    assert!(matches!(
        client.poll_login(&mut login).await,
        Err(Error::FinishedLogin)
    ));
    assert_eq!(polls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn denial_expiry_and_cancellation_do_not_restart_a_grant() {
    for case in [Case::Denied, Case::Expired, Case::EarlyExpiry] {
        let (client, polls) = mocked(case, Arc::new(AtomicUsize::new(200))).await;
        let mut login = client
            .start_login("Coder Desktop", "test-mac")
            .await
            .unwrap();
        let error = client
            .wait_for_login(&mut login, &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(
            (case, error),
            (Case::Denied, Error::Denied)
                | (Case::Expired, Error::ExpiredLogin)
                | (Case::EarlyExpiry, Error::ExpiredLogin)
        ));
        assert_eq!(
            polls.load(Ordering::SeqCst),
            if matches!(case, Case::EarlyExpiry) {
                0
            } else {
                1
            }
        );
    }
    let (client, polls) = mocked(Case::Signed, Arc::new(AtomicUsize::new(200))).await;
    let mut login = client
        .start_login("Coder Desktop", "test-mac")
        .await
        .unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        client.wait_for_login(&mut login, &cancellation).await,
        Err(Error::Cancelled)
    ));
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        client.poll_login(&mut login).await,
        Err(Error::FinishedLogin)
    ));
}

#[tokio::test]
async fn a_lost_redemption_is_not_retried_or_accepted_as_login() {
    let (original, polls) = mocked(Case::Lost, Arc::new(AtomicUsize::new(200))).await;
    let mut login = original
        .start_login("Coder Desktop", "test-mac")
        .await
        .unwrap();
    let client =
        Client::with_timeout(original.deployment().clone(), Duration::from_millis(50)).unwrap();
    assert!(matches!(
        client
            .wait_for_login(&mut login, &CancellationToken::new())
            .await,
        Err(Error::UncertainLogin(_))
    ));
    assert!(matches!(
        client.poll_login(&mut login).await,
        Err(Error::FinishedLogin)
    ));
    assert_eq!(polls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_during_a_poll_finishes_the_attempt() {
    let (client, polls) = mocked(Case::Lost, Arc::new(AtomicUsize::new(200))).await;
    let mut login = client
        .start_login("Coder Desktop", "test-mac")
        .await
        .unwrap();
    let cancellation = CancellationToken::new();
    let cancel = cancellation.clone();
    let observed = polls.clone();
    tokio::spawn(async move {
        while observed.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        cancel.cancel();
    });
    assert!(matches!(
        client.wait_for_login(&mut login, &cancellation).await,
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        client.poll_login(&mut login).await,
        Err(Error::FinishedLogin)
    ));
    assert_eq!(polls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn dropping_an_inflight_poll_cannot_replay_its_grant() {
    let (client, polls) = mocked(Case::Lost, Arc::new(AtomicUsize::new(200))).await;
    let mut login = client
        .start_login("Coder Desktop", "test-mac")
        .await
        .unwrap();
    tokio::time::sleep(login.next_poll_in()).await;
    {
        let request = client.poll_login(&mut login);
        tokio::pin!(request);
        tokio::select! {
            _ = &mut request => panic!("poll completed before the fixture interruption"),
            _ = async {
                while polls.load(Ordering::SeqCst) == 0 { tokio::time::sleep(Duration::from_millis(1)).await; }
            } => {}
        }
    }
    assert!(matches!(
        client.poll_login(&mut login).await,
        Err(Error::FinishedLogin)
    ));
    assert_eq!(polls.load(Ordering::SeqCst), 1);
}

#[derive(Default)]
struct Memory(Mutex<Option<CredentialRecord>>);
impl CredentialStore for Memory {
    fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, record: &CredentialRecord) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(record.clone());
        Ok(())
    }
    fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
        self.0.lock().unwrap().take();
        Ok(())
    }
}

#[tokio::test]
async fn caller_storage_restores_only_the_same_live_account_and_deployment() {
    let status = Arc::new(AtomicUsize::new(200));
    let (client, _) = mocked(Case::Signed, status.clone()).await;
    let session = client.authenticate(&key()).await.unwrap();
    let memory = Memory::default();
    assert!(memory.load(client.deployment()).unwrap().is_none());
    memory.save(&session.record()).unwrap();
    let record = memory.load(client.deployment()).unwrap().unwrap();
    assert_eq!(
        client.restore(&record).await.unwrap().account().id,
        "acct_fixture"
    );
    let (other, _) = mocked(Case::Signed, Arc::new(AtomicUsize::new(200))).await;
    assert!(matches!(
        other.restore(&record).await,
        Err(Error::WrongDeployment)
    ));
    assert!(matches!(
        other.sign_out_record(&record).await,
        Err(Error::WrongDeployment)
    ));
    // Imported sign-in keys remain caller-owned and are forgotten locally.
    client.sign_out_record(&record).await.unwrap();
    let mut changed = serde_json::to_value(&record).unwrap();
    changed["account"]["id"] = json!("acct_other");
    assert!(matches!(
        client
            .restore(&serde_json::from_value(changed).unwrap())
            .await,
        Err(Error::WrongAccount)
    ));
    let mut expired = serde_json::to_value(&record).unwrap();
    expired["expires_at"] = json!(0);
    assert!(matches!(
        client
            .restore(&serde_json::from_value(expired).unwrap())
            .await,
        Err(Error::ExpiredSession)
    ));
    status.store(500, Ordering::SeqCst);
    assert!(matches!(
        client.restore(&record).await,
        Err(Error::Unavailable { .. })
    ));
    assert!(memory.load(client.deployment()).unwrap().is_some());
    status.store(401, Ordering::SeqCst);
    assert!(matches!(
        client.restore(&record).await,
        Err(Error::Unauthorized)
    ));
    memory.clear(client.deployment()).unwrap();
    assert!(memory.load(client.deployment()).unwrap().is_none());
}

#[tokio::test]
async fn transport_bounds_and_errors_never_echo_secrets() {
    let status = Arc::new(AtomicUsize::new(429));
    let (client, _) = mocked(Case::Signed, status.clone()).await;
    let error = client.authenticate(&key()).await.unwrap_err();
    assert!(
        matches!(error,Error::RateLimited{retry_after:Some(duration)} if duration==Duration::from_secs(2))
    );
    assert!(!format!("{error:?} {error}").contains(&token()));
    status.store(413, Ordering::SeqCst);
    assert!(matches!(
        client.authenticate(&key()).await,
        Err(Error::TooLarge)
    ));
    status.store(400, Ordering::SeqCst);
    let error = client.authenticate(&key()).await.unwrap_err();
    assert!(matches!(error, Error::InvalidResponse));
    assert!(!format!("{error:?}").contains(&token()));
    let (unsafe_client, _) = mocked(Case::UnsafeLink, status).await;
    assert!(matches!(
        unsafe_client.start_login("Coder Desktop", "test-mac").await,
        Err(Error::InvalidResponse)
    ));
}

#[tokio::test]
async fn an_authenticated_redirect_does_not_forward_the_credential() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let reached = Arc::new(AtomicUsize::new(0));
    let count = reached.clone();
    let site = Router::new()
        .route(
            "/v1/account",
            get(|| async { (StatusCode::TEMPORARY_REDIRECT, [("location", "/stolen")]) }),
        )
        .route(
            "/stolen",
            get(move || {
                count.fetch_add(1, Ordering::SeqCst);
                async { Json(view()) }
            }),
        );
    tokio::spawn(async move { axum::serve(listener, site).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    assert!(matches!(
        client.authenticate(&key()).await,
        Err(Error::Redirect)
    ));
    assert_eq!(reached.load(Ordering::SeqCst), 0);
}

#[test]
fn deployment_addresses_cannot_hide_credentials_or_use_remote_plain_http() {
    for api in [
        "http://other.example/v1",
        "https://secret@api.example/v1",
        "https://api.example/v1?secret=value",
        "https://api.example/v1#secret",
        "https://api.example/other",
    ] {
        assert!(Deployment::new(api, "https://site.example").is_err());
    }
}

/// Real public website handlers over real tenancy stores, all under a temporary root.
#[tokio::test]
async fn sdk_device_login_restore_workspaces_and_sign_out_use_the_public_handlers() {
    use oa_auth::{
        fake::{self, Fake},
        local::LocalService,
    };
    // This dependency uses its production private-file checks. Use Cargo's
    // explicit test root, not a system temp path with writable ancestors.
    let fixture_root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(fixture_root).unwrap();
    let root = tempfile::Builder::new()
        .prefix("sdk-auth-")
        .tempdir_in(fixture_root)
        .unwrap();
    let state = root.path().canonicalize().unwrap();
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let redirect = format!("{origin}/auth/github/callback");
    let fake = Fake::new("client", "secret", &redirect, vec![fake::octo()]);
    let github = fake.spawn().await.unwrap();
    let credentials = fake::credentials(&github, "client", "secret", &redirect).unwrap();
    let accounts_dir = state.join("accounts");
    let service = LocalService::install(
        &accounts_dir,
        oa_auth::Github::new(credentials).unwrap(),
        "signup",
        3600,
    )
    .unwrap();
    let accounts = tenancy::Accounts::open(&accounts_dir).unwrap();
    let account = accounts
        .create_account("Existing account", &["key:0000000000000000".into()])
        .unwrap();
    let workspace = accounts
        .create_workspace(
            &account.id,
            "Personal",
            tenancy::WorkspaceKind::Personal,
            "signup",
            None,
        )
        .unwrap();
    let backend = service.spawn().await.unwrap();
    let key_path = state.join("csrf.key");
    std::fs::write(&key_path, [42; 32]).unwrap();
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config_path = state.join("cloud.json");
    std::fs::write(&config_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":origin,"account_service":backend,"csrf_secret":key_path})).unwrap()).unwrap();
    std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = openagents_web::Config::development(state.join("tasks"));
    config.port = address.port();
    config.public_hosts.push(address.to_string());
    config.cloud = Some(Arc::new(
        openagents_web::cloud::session::CloudSession::load(&config_path).unwrap(),
    ));
    config.inference = Some(Arc::new(
        openagents_web::upstream::Upstream::new(&backend).unwrap(),
    ));
    let router = openagents_web::router(config);
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let mut login = client
        .start_login("Coder Desktop", "fixture-mac")
        .await
        .unwrap();
    tenancy::sessions::Sessions::open(&accounts_dir)
        .unwrap()
        .mutate(|book, _, now| {
            book.decide_device(
                login.user_code(),
                &tenancy::workspaces::UserId::from(account.id.as_str()),
                true,
                now,
            )
        })
        .unwrap();
    let session = client
        .wait_for_login(&mut login, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(session.account().id, account.id);
    let view = client.account(&session).await.unwrap();
    assert_eq!(view.workspaces[0].id, workspace.id);
    let bytes = serde_json::to_vec(&session.record()).unwrap();
    let record: CredentialRecord = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        client.restore(&record).await.unwrap().account().id,
        account.id
    );
    // Fixture setup uses the existing sync API; the SDK exposes only reads.
    let fixture_credential = serde_json::to_value(&record).unwrap()["credential"]
        .as_str()
        .unwrap()
        .to_owned();
    let seed = reqwest::Client::new().put(format!("{origin}/v1/threads/synced/phone-sdk-fixture"))
        .bearer_auth(&fixture_credential).json(&json!({
            "computer":"Offline fixture", "title":"Long saved chat", "messages":(0..405).map(|n|json!({"role":if n==1 {"thinking"}else{"assistant"},"text":format!("Saved {n}")})).collect::<Vec<_>>()
        })).send().await.unwrap();
    assert!(seed.status().is_success());
    let cancel = CancellationToken::new();
    let mut chats = client.history(&session).unwrap();
    let page = chats
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].source, openagents_sdk::ChatSource::Phone);
    let mut transcript = client
        .transcript(&session, &page.data[0].chat)
        .unwrap()
        .with_page_size(200)
        .unwrap();
    let mut messages = Vec::new();
    while let Some(page) = transcript
        .next_page(&client, &session, &cancel)
        .await
        .unwrap()
    {
        messages.extend(page.data);
    }
    assert_eq!(messages.len(), 405);
    assert_eq!(messages[1].role, openagents_sdk::MessageRole::Thinking);
    assert_eq!(messages[404].text, "Saved 404");
    // A saved credential can be revoked even without a restorable account session.
    let mut saved = serde_json::to_value(session.record()).unwrap();
    saved["expires_at"] = json!(0);
    let saved = serde_json::from_value(saved).unwrap();
    client.sign_out_record(&saved).await.unwrap();
    assert!(matches!(
        client.restore(&record).await,
        Err(Error::Unauthorized)
    ));
    assert_eq!(accounts.store().unwrap().accounts.len(), 1);
}
