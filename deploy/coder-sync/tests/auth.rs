use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use coder_sync::server::{
    self, StateData, auth::Accounts, config::AuthMode, db::Database, rooms::Rooms, storage::Bucket,
};
use futures::StreamExt;
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Fixture {
    base: String,
    db: Database,
    revoke: Arc<AtomicBool>,
    unavailable: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

#[tokio::test]
async fn device_proofs_require_a_registered_host_exact_request_and_fresh_grant() {
    use coder_access::http_proof::{self, DEVICE_HEADER, HOST_HEADER, Intent};
    use secp256k1::SecretKey;
    let f = Fixture::start().await;
    let host = SecretKey::new(&mut secp256k1::rand::rng());
    let device = SecretKey::new(&mut secp256k1::rand::rng());
    let at = coder_access::unix_time().unwrap();
    let host_id = uuid::Uuid::new_v4().to_string();
    let chat = uuid::Uuid::new_v4().to_string();
    let public = nostr::domain::RelaySigner::from_secret_hex(&host.display_secret().to_string())
        .unwrap()
        .pubkey()
        .to_owned();
    let device_public =
        nostr::domain::RelaySigner::from_secret_hex(&device.display_secret().to_string())
            .unwrap()
            .pubkey()
            .to_owned();
    let url = format!("{}/device/{host_id}/key", f.base);
    let body = serde_json::to_vec(&json!({"publicKey":public})).unwrap();
    let intent = Intent::new(&url, "POST", &body).unwrap();
    let proof =
        http_proof::sign(&host, &intent, at, vec![("account", "acct_alpha".into())]).unwrap();
    let registration = || {
        reqwest::Client::new()
            .post(&url)
            .bearer_auth(token('a'))
            .header(DEVICE_HEADER, http_proof::header(&proof).unwrap())
            .header("content-type", "application/json")
            .body(body.clone())
    };
    assert_eq!(registration().send().await.unwrap().status(), 204);
    assert_eq!(
        registration().send().await.unwrap().status(),
        401,
        "replay is rejected across requests"
    );
    let db = f.db.connect().await.unwrap();
    db.execute(
        "INSERT INTO registry_heads(account) VALUES('acct_alpha') ON CONFLICT DO NOTHING",
        &[],
    )
    .await
    .unwrap();
    let row = json!({"fields":{"deviceId":host_id}}).to_string();
    db.execute("INSERT INTO registry_rows(account,kind,id,seq,clock,writer,value) VALUES('acct_alpha','chats',$1,1,1,'fixture',$2::text::jsonb)", &[&chat, &row]).await.unwrap();
    let path = format!("/chat2/{chat}/rows?device=phone&after=0");
    let url = format!("{}{path}", f.base);
    let intent = Intent::new(&url, "GET", b"").unwrap();
    let make = |intent: &Intent, key: &SecretKey, expiry: u64| {
        let host_proof = http_proof::sign(
            key,
            intent,
            at,
            vec![
                ("account", "acct_alpha".into()),
                ("hostId", host_id.clone()),
                ("p", device_public.clone()),
                ("expiration", expiry.to_string()),
            ],
        )
        .unwrap();
        let device_proof = http_proof::sign(
            &device,
            intent,
            at,
            vec![
                ("account", "acct_alpha".into()),
                ("host", host_proof.id.clone()),
            ],
        )
        .unwrap();
        (
            http_proof::header(&host_proof).unwrap(),
            http_proof::header(&device_proof).unwrap(),
        )
    };
    let send = |host: String, device: String, url: String, account: char| {
        reqwest::Client::new()
            .get(url)
            .bearer_auth(token(account))
            .header(HOST_HEADER, host)
            .header(DEVICE_HEADER, device)
    };
    let (h, d) = make(&intent, &host, at + 15);
    assert_eq!(
        send(h.clone(), d.clone(), url.clone(), 'a')
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        send(h, d, url.clone(), 'a').send().await.unwrap().status(),
        401
    );
    for (key, expiry, request_url, account) in [
        (&device, at + 15, url.clone(), 'a'),
        (&host, at, url.clone(), 'a'),
        (&host, at + 100, url.clone(), 'a'),
        (&host, at + 15, format!("{url}&changed=1"), 'a'),
        (&host, at + 15, url.clone(), 'b'),
    ] {
        let (h, d) = make(&intent, key, expiry);
        assert_eq!(
            send(h, d, request_url, account)
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let ws_url = format!(
        "{}/chat2/{chat}/ws?token={}&device=phone&hostDevice={host_id}",
        f.base.replacen("http:", "ws:", 1),
        token('a')
    );
    let ws_intent = Intent::new(&ws_url, "GET", b"").unwrap();
    let (h, d) = make(&ws_intent, &host, coder_access::unix_time().unwrap() + 2);
    let mut request = ws_url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert(HOST_HEADER, h.parse().unwrap());
    request
        .headers_mut()
        .insert(DEVICE_HEADER, d.parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let code = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(frame) = ws.next().await {
            if let tokio_tungstenite::tungstenite::Message::Close(Some(close)) = frame.unwrap() {
                return u16::from(close.code);
            }
        }
        0
    })
    .await
    .unwrap();
    assert_eq!(
        code, 4401,
        "a proved socket cannot outlive the host's admission"
    );
    db.execute(
        "UPDATE device_host_keys SET revoked=true WHERE device_id=$1",
        &[&host_id],
    )
    .await
    .unwrap();
    let (h, d) = make(&intent, &host, at + 15);
    assert_eq!(send(h, d, url, 'a').send().await.unwrap().status(), 401);
    // Legacy account clients keep the existing additive API behavior.
    assert_eq!(
        f.get(&path, Some(token('a')))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
fn token(c: char) -> String {
    format!("sess_{}", c.to_string().repeat(64))
}
impl Fixture {
    async fn start() -> Self {
        let revoke = Arc::new(AtomicBool::new(false));
        let unavailable = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let api_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = format!("http://{}", api_listener.local_addr().unwrap());
        let (r, u, c) = (revoke.clone(), unavailable.clone(), calls.clone());
        let app=Router::new().route("/v1/account",get(move|headers:HeaderMap|{let(r,u,c)=(r.clone(),u.clone(),c.clone());async move{c.fetch_add(1,Ordering::SeqCst);if u.load(Ordering::SeqCst){return StatusCode::SERVICE_UNAVAILABLE.into_response();}let bearer=headers.get("authorization").and_then(|v|v.to_str().ok()).unwrap_or("");let account=if bearer==format!("Bearer {}",token('a'))&&!r.load(Ordering::SeqCst){"acct_alpha"}else if bearer==format!("Bearer {}",token('b')){"acct_beta"}else{return StatusCode::UNAUTHORIZED.into_response();};Json(json!({"account":{"id":account,"label":"Dedicated fixture"},"workspaces":[]})).into_response()}}));
        let api_task = tokio::spawn(async move {
            axum::serve(api_listener, app).await.unwrap();
        });
        let db = Database::new(
            &std::env::var("CODER_SYNC_TEST_DATABASE_URL").expect("Run scripts/coder-sync/test.sh"),
        )
        .unwrap();
        db.migrate().await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = server::router(StateData {
            proof_origin: Some(base.clone()),
            database: db.clone(),
            bucket: Bucket::memory(),
            rooms: Rooms::default(),
            devices: Default::default(),
            auth: AuthMode::OpenAgents,
            accounts: Some(
                Accounts::new(openagents::Deployment::new(&format!("{api}/v1"), &api).unwrap())
                    .unwrap(),
            ),
            revision: "auth-fixture".into(),
            push: None,
        });
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base,
            db,
            revoke,
            unavailable,
            calls,
            tasks: vec![task, api_task],
        }
    }
    fn get(&self, path: &str, bearer: Option<String>) -> reqwest::RequestBuilder {
        let request = reqwest::Client::new().get(format!("{}{path}", self.base));
        if let Some(token) = bearer {
            request.bearer_auth(token)
        } else {
            request
        }
    }
    fn ws(&self, path: &str, bearer: &str) -> String {
        format!(
            "{}{path}?token={bearer}",
            self.base.replacen("http:", "ws:", 1)
        )
    }
}
#[tokio::test]
async fn every_request_verifies_authority_and_ignores_forged_account_headers() {
    let f = Fixture::start().await;
    let chat = uuid::Uuid::new_v4();
    let path = format!("/chat2/{chat}/rows");
    assert_eq!(f.get("/health", None).send().await.unwrap().status(), 200);
    for bearer in [
        None,
        Some(token('c')),
        Some("workos.jwt.token".into()),
        Some("user@workspace".into()),
    ] {
        assert_eq!(
            f.get(&path, bearer)
                .header("x-coder-verified-account", "acct_alpha")
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    let before = f.calls.load(Ordering::SeqCst);
    for _ in 0..2 {
        assert_eq!(
            f.get(&path, Some(token('a')))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    assert_eq!(f.calls.load(Ordering::SeqCst) - before, 2);
    assert_eq!(
        f.get(&path, Some(token('b')))
            .header("x-coder-verified-account", "acct_alpha")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let owner: String =
        f.db.connect()
            .await
            .unwrap()
            .query_one(
                "SELECT account FROM chats WHERE chat_id=$1",
                &[&chat.to_string()],
            )
            .await
            .unwrap()
            .get(0);
    assert_eq!(owner, "acct_alpha");
    f.revoke.store(true, Ordering::SeqCst);
    assert_eq!(
        f.get(&path, Some(token('a')))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}
#[tokio::test]
async fn outages_refuse_access_without_accepting_a_cached_identity() {
    let f = Fixture::start().await;
    let path = format!("/chat2/{}/rows", uuid::Uuid::new_v4());
    assert_eq!(
        f.get(&path, Some(token('a')))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    f.unavailable.store(true, Ordering::SeqCst);
    assert_eq!(
        f.get(&path, Some(token('a')))
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    f.unavailable.store(false, Ordering::SeqCst);
    assert_eq!(
        f.get(&path, Some(token('a')))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}
#[tokio::test]
async fn websocket_connects_are_account_scoped_and_revocation_closes_all_room_types() {
    let f = Fixture::start().await;
    let chat = uuid::Uuid::new_v4();
    let device = uuid::Uuid::new_v4();
    let chat_path = format!("/chat2/{chat}/ws");
    let registry_path = "/registry/acct_alpha/ws";
    let device_path = format!("/device/{device}/ws");
    let mut sockets = Vec::new();
    for path in [
        &chat_path,
        registry_path,
        &format!("{device_path}?role=host"),
    ] {
        let separator = if path.contains('?') { '&' } else { '?' };
        let url = format!(
            "{}{path}{separator}token={}",
            f.base.replacen("http:", "ws:", 1),
            token('a')
        );
        sockets.push(tokio_tungstenite::connect_async(url).await.unwrap().0);
    }
    for path in [&chat_path, registry_path, &device_path] {
        assert!(
            matches!(tokio_tungstenite::connect_async(f.ws(path,&token('c'))).await.unwrap_err(),tokio_tungstenite::tungstenite::Error::Http(r)if r.status()==401)
        );
        assert!(
            matches!(tokio_tungstenite::connect_async(f.ws(path,&token('b'))).await.unwrap_err(),tokio_tungstenite::tungstenite::Error::Http(r)if r.status()==403)
        );
    }
    f.revoke.store(true, Ordering::SeqCst);
    for socket in &mut sockets {
        tokio::time::timeout(Duration::from_secs(22), async {
            loop {
                match socket.next().await {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(Some(frame)))) => {
                        assert_eq!(u16::from(frame.code), 4401);
                        break;
                    }
                    Some(Ok(_)) => {}
                    other => panic!("Expected revoked connection close, got {other:?}"),
                }
            }
        })
        .await
        .unwrap();
    }
}
