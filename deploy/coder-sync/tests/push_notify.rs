//! Session notifications end to end (#11853): registry writes through the
//! real server and PostgreSQL, delivered to a stand-in push gateway that
//! checks each request the way the real delivery listener does.
use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use coder_sync::server::{
    self, StateData,
    config::AuthMode,
    db::Database,
    push::{Push, Settings},
    rooms::Rooms,
    storage::Bucket,
};
use push_gateway::wire::{self, DeliveryRequest};
use serde_json::{Value, json};
use std::{
    os::unix::fs::PermissionsExt,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Default)]
struct Gateway {
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    answer: Arc<Mutex<Option<(StatusCode, Value)>>>,
}

async fn deliver(
    State(gateway): State<Gateway>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    body: Bytes,
) -> (StatusCode, Json<Value>) {
    let authorization = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let Ok(verified) =
        push_gateway::nip98::verify(authorization, uri.path(), &body, push_gateway::unix_now())
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"invalid_auth"})),
        );
    };
    // The body is the closed four-member request and nothing else.
    let Ok(request) = wire::parse_closed::<DeliveryRequest>(&body) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request"})),
        );
    };
    assert!(wire::valid_uuid(&request.request_id));
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value.as_object().unwrap().len(),
        4,
        "only the wake request travels: {value}"
    );
    gateway
        .seen
        .lock()
        .unwrap()
        .push((verified.pubkey, json!({"path":uri.path(),"body":value})));
    let answer = gateway.answer.lock().unwrap().clone();
    let (status, body) = answer.unwrap_or((StatusCode::OK, json!({"status":"accepted"})));
    (status, Json(body))
}

struct Fixture {
    base: String,
    account: String,
    gateway: Gateway,
    delegate: String,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
impl Fixture {
    async fn start() -> Self {
        let gateway = Gateway::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gateway_url = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route(wire::APNS_DELIVERY_PATH, post(deliver))
            .route(wire::FCM_DELIVERY_PATH, post(deliver))
            .with_state(gateway.clone());
        let gateway_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("push.key");
        std::fs::write(&key, "2a".repeat(32)).unwrap();
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
        let vars = std::collections::HashMap::from([
            ("CODER_SYNC_PUSH_GATEWAY", gateway_url.clone()),
            ("CODER_SYNC_PUSH_REGISTRATION", gateway_url),
            ("CODER_SYNC_PUSH_KEY_FILE", key.to_str().unwrap().to_owned()),
            ("CODER_SYNC_PUSH_APNS_PROFILE", "app.ios".to_owned()),
        ]);
        let settings = Settings::parse(&|k| vars.get(k).cloned()).unwrap().unwrap();
        let delegate = settings.pubkey();
        let db = Database::new(
            &std::env::var("CODER_SYNC_TEST_DATABASE_URL").expect("Run scripts/coder-sync/test.sh"),
        )
        .unwrap();
        db.migrate().await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = server::router(StateData {
            database: db,
            bucket: Bucket::memory(),
            rooms: Rooms::default(),
            devices: Default::default(),
            auth: AuthMode::Dev,
            accounts: None,
            revision: "push-test".into(),
            push: Some(Arc::new(Push::new(settings))),
        });
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base,
            account: format!("push-{}", uuid::Uuid::new_v4()),
            gateway,
            delegate,
            tasks: vec![gateway_task, server_task],
            _dir: dir,
        }
    }
    fn url(&self, path: &str) -> String {
        format!("{}/registry/{}{path}", self.base, self.account)
    }
    async fn get(&self, path: &str) -> Value {
        reqwest::Client::new()
            .get(self.url(path))
            .bearer_auth(&self.account)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    async fn grant(&self, device: &str, body: Value) -> StatusCode {
        reqwest::Client::new()
            .post(self.url(&format!("/push-grant?device={device}")))
            .bearer_auth(&self.account)
            .json(&body)
            .send()
            .await
            .unwrap()
            .status()
    }
    async fn write(&self, ops: Vec<Value>, test_run: bool) {
        let mut request = reqwest::Client::new()
            .post(self.url("/push?device=mac"))
            .bearer_auth(&self.account)
            .json(&json!({"batch":uuid::Uuid::new_v4().to_string(),"ops":ops}));
        if test_run {
            request = request.header("X-OpenAgents-Test", "1");
        }
        request.send().await.unwrap().error_for_status().unwrap();
    }
    fn seen(&self) -> Vec<(String, Value)> {
        self.gateway.seen.lock().unwrap().clone()
    }
    async fn settle(&self, count: usize) -> Vec<(String, Value)> {
        tokio::time::timeout(Duration::from_secs(10), async {
            while self.seen().len() < count {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("wakes arrive");
        // Give any extra, unwanted wake time to show up.
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.seen()
    }
}

static TICK: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
fn hlc() -> String {
    let tick = TICK.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    openagents_doc::encode_hlc(1_800_000_000_000 + tick, 0, "mac")
}
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
fn session(status: &str, turn: Option<&str>) -> Value {
    json!({"kind":"sessions","id":"c1","op":"upsert","hlc":hlc(),
        "set":{"chatId":"c1","deviceId":"mac","status":status,"lastCompletedTurn":turn,"updatedAt":now_ms()}})
}
fn chat(set: Value) -> Value {
    json!({"kind":"chats","id":"c1","op":"upsert","hlc":hlc(),"set":set})
}
fn grant_body(grant: &str, prefs: Value) -> Value {
    json!({"transport":"apns","grant":grant,"expiresAt":push_gateway::unix_now() + 3_600,"prefs":prefs})
}

#[tokio::test]
async fn registry_writes_wake_phones_through_the_gateway() {
    let fx = Fixture::start().await;
    let config = fx.get("/push-config").await;
    assert_eq!(config["configured"], true);
    assert_eq!(config["delegate"], fx.delegate);
    assert_eq!(config["profiles"]["apns"], "app.ios");

    assert_eq!(
        fx.grant("ios-1", json!({"transport":"apns","grant":"x"}))
            .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        fx.grant("ios-1", grant_body("pg1_one", json!({"failed":false})))
            .await,
        StatusCode::OK
    );
    let stats = fx.get("/stats").await;
    assert_eq!(stats["pushConfigured"], true);
    assert_eq!(stats["pushGrants"][0]["device"], "ios-1");
    assert_eq!(stats["pushGrants"][0]["prefs"]["failed"], false);

    // First sight sets the baseline; a heartbeat is not news.
    fx.write(
        vec![chat(json!({"title":"Fix login"})), session("working", None)],
        false,
    )
    .await;
    fx.write(vec![session("working", None)], false).await;
    // A question, then a finished turn: one wake each.
    fx.write(vec![session("awaitingInput", None)], false).await;
    fx.write(vec![session("working", None)], false).await;
    fx.write(vec![session("idle", Some("turn-1"))], false).await;
    // The same finished turn again is not news.
    fx.write(vec![session("idle", Some("turn-1"))], false).await;
    // The phone turned failures off.
    fx.write(vec![session("errored", Some("turn-1"))], false)
        .await;
    let seen = fx.settle(2).await;
    assert_eq!(seen.len(), 2, "{seen:?}");
    for (signer, request) in &seen {
        assert_eq!(signer, &fx.delegate);
        assert_eq!(request["path"], wire::APNS_DELIVERY_PATH);
        assert_eq!(request["body"]["endpoint_grant"], "pg1_one");
        assert!(!request.to_string().contains("Fix login"));
    }
    assert_ne!(
        seen[0].1["body"]["request_id"],
        seen[1].1["body"]["request_id"]
    );
    let log = fx.get("/stats").await["pushLog"].clone();
    let lines = log
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["category"].as_str().unwrap(),
                e["result"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(lines, [("input", "accepted"), ("done", "accepted")]);
}

#[tokio::test]
async fn test_runs_open_apps_and_side_chats_stay_quiet() {
    let fx = Fixture::start().await;
    fx.grant("ios-1", grant_body("pg1_one", json!({}))).await;
    fx.write(
        vec![chat(json!({"title":"Fix login"})), session("working", None)],
        false,
    )
    .await;
    // A test run never notifies.
    fx.write(vec![session("idle", Some("turn-1"))], true).await;
    fx.write(vec![session("working", None)], true).await;
    // While the app is open (a current presence beat), nothing is sent.
    fx.get("/rows?device=ios-1&beat=1").await;
    fx.write(vec![session("idle", Some("turn-2"))], false).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(fx.seen().is_empty(), "{:?}", fx.seen());
    let log = fx.get("/stats").await["pushLog"].clone();
    assert_eq!(log[0]["result"], "app open");

    // A side chat never notifies, even with the app closed.
    let other = Fixture::start().await;
    other.grant("ios-2", grant_body("pg1_two", json!({}))).await;
    other
        .write(
            vec![chat(json!({"parentChatId":"p"})), session("working", None)],
            false,
        )
        .await;
    other.write(vec![session("idle", Some("t"))], false).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(other.seen().is_empty());
}

#[tokio::test]
async fn a_dead_grant_is_forgotten() {
    let fx = Fixture::start().await;
    *fx.gateway.answer.lock().unwrap() = Some((
        StatusCode::GONE,
        json!({"status":"invalid_endpoint","generation":1,"invalid_at":1}),
    ));
    fx.grant("ios-1", grant_body("pg1_one", json!({}))).await;
    fx.write(
        vec![chat(json!({"title":"t"})), session("working", None)],
        false,
    )
    .await;
    fx.write(vec![session("errored", None)], false).await;
    fx.settle(1).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while fx.get("/stats").await["pushGrants"]
            .as_array()
            .is_some_and(|g| !g.is_empty())
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the dead grant is removed");
    let delete = reqwest::Client::new()
        .delete(fx.url("/push-grant?device=ios-1"))
        .bearer_auth(&fx.account)
        .send()
        .await
        .unwrap();
    assert!(delete.status().is_success());
}
