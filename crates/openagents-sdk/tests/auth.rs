#![cfg(unix)]

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

#[tokio::test]
async fn computer_job_import_checks_its_own_endpoint_and_keeps_general_restore_closed() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().route(
        "/v1/work/computer-session",
        get(|headers: HeaderMap| async move {
            assert_eq!(headers["authorization"], "Bearer scj_fixture");
            Json(view())
        }),
    );
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate_computer_job("scj_fixture")
        .await
        .unwrap();
    assert_eq!(session.account().id, "acct_fixture");
    assert!(!format!("{session:?}").contains("scj_fixture"));
    assert!(matches!(
        client.authenticate("scj_fixture").await,
        Err(Error::InvalidCredential)
    ));
    assert!(matches!(
        client.restore(&session.record()).await,
        Err(Error::InvalidCredential)
    ));
    assert!(matches!(
        client.authenticate_computer_job(&token()).await,
        Err(Error::InvalidCredential)
    ));
    task.abort();
}

#[tokio::test]
async fn environment_jobs_import_only_their_scope_and_cannot_restore_as_account_credentials() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let router = Router::new().route(
        "/v1/environment-computers/session",
        get(move |headers: HeaderMap| {
            let seen = seen.clone();
            async move {
                seen.fetch_add(1, Ordering::SeqCst);
                assert_eq!(headers["authorization"], "Bearer ecj_fixture");
                Json(view())
            }
        }),
    );
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate_environment_job("ecj_fixture")
        .await
        .unwrap();
    assert_eq!(session.account().id, "acct_fixture");
    assert!(!format!("{session:?}").contains("ecj_fixture"));
    assert!(matches!(
        client.authenticate("ecj_fixture").await,
        Err(Error::InvalidCredential)
    ));
    assert!(matches!(
        client.restore(&session.record()).await,
        Err(Error::InvalidCredential)
    ));
    assert!(matches!(
        client.authenticate_environment_job("scj_fixture").await,
        Err(Error::InvalidCredential)
    ));
    assert!(matches!(
        client.authenticate_computer_job("ecj_fixture").await,
        Err(Error::InvalidCredential)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    task.abort();
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
    assert_eq!(session.bearer_token(), key());
    assert!(!format!("{session:?}").contains(&key()));
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

#[test]
fn play_request_hash_uses_the_server_byte_contract() {
    use openagents_sdk::play_integrity_request_hash;
    for (challenge, pubkey) in [
        ("challenge", "key"),
        ("challenge\nkey", "other"),
        ("", "ab"),
    ] {
        assert_eq!(
            play_integrity_request_hash(challenge, pubkey),
            oa_auth::play_integrity::request_hash(challenge, pubkey)
        );
    }
    assert_ne!(
        play_integrity_request_hash("challenge", "key"),
        play_integrity_request_hash("other", "key")
    );
    assert_ne!(
        play_integrity_request_hash("challenge", "key"),
        play_integrity_request_hash("challenge", "other")
    );
}

#[tokio::test]
async fn workos_credentials_import_restore_and_rotate_without_losing_refresh_tokens() {
    let access = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.signature";
    let renewed = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ1c2VyMiJ9.signature";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let refreshed = Arc::new(AtomicUsize::new(0));
    let observed = refreshed.clone();
    let router = Router::new()
        .route("/v1/device/nostr/challenge", get(|| async { Json(json!({"challenge":"fixture"})) }))
        .route("/v1/device/nostr", post(move |headers: HeaderMap| async move {
            assert_eq!(headers["x-openagents-auth"], "workos");
            assert!(headers["authorization"].to_str().unwrap().starts_with("Nostr "));
            Json(json!({"access_token":access,"refresh_token":"private-refresh-1","token_type":"Bearer","expires_in":300,"account":{"id":"acct_fixture","label":"Existing account"},"pubkey":"a".repeat(64)}))
        }))
        .route("/v1/account", get(move |headers: HeaderMap| async move {
            let value = headers["authorization"].to_str().unwrap();
            assert!(value == format!("Bearer {access}") || value == format!("Bearer {renewed}"));
            Json(view())
        }))
        .route("/v1/device/refresh", post(move |headers: HeaderMap, Json(body): Json<Value>| {
            let observed = observed.clone();
            async move {
                assert!(headers.get("authorization").is_none());
                assert_eq!(body["refresh_token"], "private-refresh-1");
                if observed.fetch_add(1, Ordering::SeqCst) > 0 { return StatusCode::UNAUTHORIZED.into_response(); }
                Json(json!({"access_token":renewed,"refresh_token":"private-refresh-2","expires_in":300,"token_type":"Bearer"})).into_response()
            }
        }));
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let imported = client.authenticate(access).await.unwrap();
    assert!(!imported.can_refresh());
    struct Signer;
    impl openagents_sdk::NostrSigner for Signer {
        fn sign_event(
            &self,
            event: &openagents_sdk::EventTemplate,
        ) -> Result<String, openagents_sdk::SignerError> {
            Ok(serde_json::to_string(event).unwrap())
        }
    }
    let signed = client
        .sign_in_with_key(&Signer, "OpenAgents", "fixture")
        .await
        .unwrap();
    assert!(signed.session.can_refresh());
    let mut saved = serde_json::to_value(signed.session.record()).unwrap();
    saved["expires_at"] = json!(0);
    let expired: CredentialRecord = serde_json::from_value(saved).unwrap();
    let session = client.refresh_record(&expired).await.unwrap();
    assert!(session.can_refresh());
    assert_eq!(session.bearer_token(), renewed);
    let record = session.record();
    assert_eq!(
        serde_json::to_value(&record).unwrap()["refresh_token"],
        "private-refresh-2"
    );
    assert!(!format!("{record:?}").contains("private-refresh"));
    assert!(!format!("{record:?}").contains("signature"));
    let restored = client.restore(&record).await.unwrap();
    assert!(restored.can_refresh());
    assert!(client.refresh_record(&expired).await.is_err());
    task.abort();
}
