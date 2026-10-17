//! Silent sign-in with the identity's key (#11995), against a small fake of
//! the key sign-in routes (#11993).
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use openagents_account::{
    Controller, Phase,
    identity::{Identities, Identity, IdentityVault, NoWallet},
    openagents::{Client, CredentialRecord, CredentialStore, Deployment, StorageError},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

const WORDS: &str =
    "leader monkey parrot ring guide accident before fence cannon height naive bean";

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
struct Vault(Mutex<Option<Vec<u8>>>);
impl IdentityVault for Vault {
    fn load(&self) -> Result<Option<(Vec<u8>, bool)>, StorageError> {
        Ok(self.0.lock().unwrap().clone().map(|e| (e, false)))
    }
    fn store(&self, entropy: &[u8], _: bool) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(entropy.to_vec());
        Ok(())
    }
}

#[derive(Default)]
struct Server {
    origin: Mutex<String>,
    /// pubkey -> account
    keys: Mutex<BTreeMap<String, String>>,
    /// bearer -> account
    sessions: Mutex<BTreeMap<String, String>>,
    issued: AtomicUsize,
    created: AtomicUsize,
    key_sign_ins: AtomicUsize,
    links: AtomicUsize,
    revokes: AtomicUsize,
    /// Seconds each issued session lasts.
    expires_in: AtomicU64,
}

impl Server {
    fn issue(&self, account: &str) -> String {
        let n = self.issued.fetch_add(1, Ordering::SeqCst) + 1;
        let token = format!("sess_{n:064x}");
        self.sessions
            .lock()
            .unwrap()
            .insert(token.clone(), account.into());
        token
    }
    fn session(&self, h: &HeaderMap) -> Option<String> {
        let token = h
            .get("authorization")?
            .to_str()
            .ok()?
            .strip_prefix("Bearer ")?;
        self.sessions.lock().unwrap().get(token).cloned()
    }
    /// The pubkey of a valid NIP-98 event over `path`, base64 in `proof`.
    fn signer(&self, proof: &str, path: &str) -> Option<String> {
        let url = format!("{}{path}", self.origin.lock().unwrap());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        nostr::domain::parse_http_authorization_claim(&format!("Nostr {proof}"), "POST", &url, now)
            .ok()
            .map(|claim| claim.auth.pubkey)
    }
}

fn label(account: &str) -> Value {
    json!({"id": account, "label": "Fixture"})
}

async fn challenge() -> Json<Value> {
    Json(json!({"challenge": "c0ffee"}))
}

async fn sign_in(State(s): State<Arc<Server>>, h: HeaderMap) -> Response {
    let Some(proof) = h
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Nostr "))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(pubkey) = s.signer(proof, "/v1/device/nostr") else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    s.key_sign_ins.fetch_add(1, Ordering::SeqCst);
    let (account, created) = {
        let mut keys = s.keys.lock().unwrap();
        match keys.get(&pubkey) {
            Some(account) => (account.clone(), false),
            None => {
                let n = s.created.fetch_add(1, Ordering::SeqCst) + 1;
                let account = format!("acct_key{n}");
                keys.insert(pubkey.clone(), account.clone());
                (account, true)
            }
        }
    };
    let token = s.issue(&account);
    Json(json!({
        "access_token": token, "token_type": "Bearer",
        "expires_in": s.expires_in.load(Ordering::SeqCst),
        "account": label(&account), "created": created, "pubkey": pubkey,
    }))
    .into_response()
}

async fn keys_challenge(State(s): State<Arc<Server>>, h: HeaderMap) -> Response {
    if s.session(&h).is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({"challenge": "beef"})).into_response()
}

async fn link(State(s): State<Arc<Server>>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    let Some(account) = s.session(&h) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(pubkey) = body["proof"]
        .as_str()
        .and_then(|p| s.signer(p, "/v1/device/keys"))
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": {"code": "key_proof_denied"}})),
        )
            .into_response();
    };
    s.links.fetch_add(1, Ordering::SeqCst);
    let mut keys = s.keys.lock().unwrap();
    if keys.get(&pubkey).is_some_and(|a| *a != account) {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": {"code": "identity_taken"}})),
        )
            .into_response();
    }
    keys.insert(pubkey.clone(), account);
    Json(json!({"keys": [{"pubkey": pubkey, "npub": "npub1fixture"}]})).into_response()
}

async fn account(State(s): State<Arc<Server>>, h: HeaderMap) -> Response {
    match s.session(&h) {
        Some(account) => {
            Json(json!({"account": label(&account), "workspaces": []})).into_response()
        }
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn revoke(State(s): State<Arc<Server>>, h: HeaderMap) -> Response {
    if let Some(token) = h
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        s.sessions.lock().unwrap().remove(token);
    }
    s.revokes.fetch_add(1, Ordering::SeqCst);
    Json(json!({"signed_out": true})).into_response()
}

async fn serve() -> (Arc<Server>, Client) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = Arc::new(Server::default());
    server.expires_in.store(30 * 86_400, Ordering::SeqCst);
    *server.origin.lock().unwrap() = origin.clone();
    let router = Router::new()
        .route("/v1/device/nostr/challenge", get(challenge))
        .route("/v1/device/nostr", post(sign_in))
        .route("/v1/device/keys/challenge", get(keys_challenge))
        .route("/v1/device/keys", post(link))
        .route("/v1/account", get(account))
        .route("/v1/device/sign-out", post(revoke))
        .with_state(server.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    (server, client)
}

fn controller(client: &Client, store: &Arc<Store>, dir: &Path) -> Arc<Controller> {
    Controller::new(
        client.clone(),
        store.clone(),
        dir.join("account/signed-out"),
    )
}

async fn signed_in(c: &Controller) -> String {
    let mut watch = c.watch();
    let wait = async {
        loop {
            match &watch.borrow_and_update().phase {
                Phase::SignedIn { account } => return account.id.clone(),
                Phase::Failed { message, .. } => panic!("failed: {message}"),
                _ => {}
            }
            watch.changed().await.unwrap();
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), wait)
        .await
        .expect("signed in")
}

#[tokio::test]
async fn first_launch_creates_the_account_and_relaunch_reuses_it() {
    let (server, client) = serve().await;
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(Vault::default());
    let ids = Identities::new(
        vault.clone(),
        Arc::new(NoWallet),
        &dir.path().join("identity"),
    );
    let identity = Arc::new(ids.load_or_create().unwrap());
    let store = Arc::new(Store::default());

    let first = controller(&client, &store, dir.path());
    first.start_with_key(identity.clone(), "Phone".into()).await;
    let account = signed_in(&first).await;
    assert_eq!(account, "acct_key1");
    assert_eq!(server.created.load(Ordering::SeqCst), 1);
    assert!(store.0.lock().unwrap().is_some());
    assert!(first.lease().is_some());
    drop(first);

    // Relaunch: the same words, the saved session, no new sign-in.
    let again = Arc::new(ids.load_or_create().unwrap());
    assert_eq!(again.npub(), identity.npub());
    let second = controller(&client, &store, dir.path());
    second.start_with_key(again, "Phone".into()).await;
    assert_eq!(signed_in(&second).await, account);
    assert_eq!(server.key_sign_ins.load(Ordering::SeqCst), 1);
    assert_eq!(server.created.load(Ordering::SeqCst), 1);
    assert_eq!(server.links.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn restoring_the_words_elsewhere_reaches_the_same_account() {
    let (server, client) = serve().await;
    let phone = tempfile::tempdir().unwrap();
    let identity = Arc::new(Identity::from_words(WORDS).unwrap());
    let a = controller(&client, &Arc::default(), phone.path());
    a.start_with_key(identity.clone(), "Old phone".into()).await;
    let account = signed_in(&a).await;

    let other = tempfile::tempdir().unwrap();
    let ids = Identities::new(
        Arc::new(Vault::default()),
        Arc::new(NoWallet),
        &other.path().join("identity"),
    );
    let restored = Arc::new(ids.restore(WORDS, false).unwrap());
    assert_eq!(restored.npub(), identity.npub());
    let b = controller(&client, &Arc::default(), other.path());
    b.sign_in_with_key(restored, "New phone".into()).await;
    assert_eq!(signed_in(&b).await, account);
    assert_eq!(server.created.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_existing_session_gets_the_key_instead_of_a_new_account() {
    let (server, client) = serve().await;
    let dir = tempfile::tempdir().unwrap();
    // Signed in by an earlier build, before the identity existed.
    let old = server.issue("acct_old");
    let store = Arc::new(Store::default());
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 20 * 86_400;
    *store.0.lock().unwrap() = Some(
        serde_json::from_value(json!({
            "deployment": client.deployment(), "account": label("acct_old"),
            "expires_at": expiry, "credential": old,
        }))
        .unwrap(),
    );
    let identity = Arc::new(Identity::from_words(WORDS).unwrap());
    let c = controller(&client, &store, dir.path());
    c.start_with_key(identity.clone(), "Phone".into()).await;
    assert_eq!(signed_in(&c).await, "acct_old");
    assert_eq!(server.links.load(Ordering::SeqCst), 1);
    assert_eq!(server.created.load(Ordering::SeqCst), 0);
    assert!(c.key_linked("acct_old", &identity.pubkey_hex()));

    // The next launch doesn't link again.
    let again = controller(&client, &store, dir.path());
    again.start_with_key(identity.clone(), "Phone".into()).await;
    signed_in(&again).await;
    assert_eq!(server.links.load(Ordering::SeqCst), 1);

    // The key's own sign-in now reaches the old account.
    let fresh = tempfile::tempdir().unwrap();
    let b = controller(&client, &Arc::default(), fresh.path());
    b.sign_in_with_key(identity, "Other".into()).await;
    assert_eq!(signed_in(&b).await, "acct_old");
    assert_eq!(server.created.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_session_near_its_end_is_replaced_by_signing_again() {
    let (server, client) = serve().await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::default());
    let identity = Arc::new(Identity::from_words(WORDS).unwrap());
    // Sessions that are always due for renewal.
    server.expires_in.store(86_400, Ordering::SeqCst);
    let c = controller(&client, &store, dir.path());
    c.start_with_key(identity.clone(), "Phone".into()).await;
    let account = signed_in(&c).await;
    let first = store.0.lock().unwrap().clone().unwrap();
    assert!(c.renewal_due());

    // Renewing in place keeps the account and saves the new session.
    c.renew_with_key(identity.clone(), "Phone".into()).await;
    let renewed = store.0.lock().unwrap().clone().unwrap();
    assert_ne!(
        serde_json::to_value(&first).unwrap()["credential"],
        serde_json::to_value(&renewed).unwrap()["credential"]
    );
    assert_eq!(renewed.account().id, account);
    assert!(matches!(c.snapshot().phase, Phase::SignedIn { .. }));

    // A relaunch with a due session signs in again and revokes the old one.
    let revokes = server.revokes.load(Ordering::SeqCst);
    let again = controller(&client, &store, dir.path());
    again.start_with_key(identity, "Phone".into()).await;
    assert_eq!(signed_in(&again).await, account);
    assert_eq!(server.key_sign_ins.load(Ordering::SeqCst), 3);
    assert_eq!(server.revokes.load(Ordering::SeqCst), revokes + 1);
    // Still one account for the key.
    assert_eq!(server.created.load(Ordering::SeqCst), 1);
}
