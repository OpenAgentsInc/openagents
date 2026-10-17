use coder_sync::server::{
    self, StateData, config::AuthMode, db::Database, rooms::Rooms, storage::Bucket,
};
use futures::{SinkExt, StreamExt};
use openagents_doc::{RegistryDoc, row_to_seed_op};
use openagents_sync::RegistryClient;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
struct Server {
    base: String,
    account: String,
    db: Database,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start() -> Self {
        let db = Database::new(
            &std::env::var("CODER_SYNC_TEST_DATABASE_URL").expect("Run scripts/coder-sync/test.sh"),
        )
        .unwrap();
        db.migrate().await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = server::router(StateData {
            database: db.clone(),
            bucket: Bucket::memory(),
            rooms: Rooms::default(),
            devices: Default::default(),
            auth: AuthMode::Dev,
            accounts: None,
            revision: "registry-test".into(),
            push: None,
        });
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base,
            account: format!("registry-{}", uuid::Uuid::new_v4()),
            db,
            task,
        }
    }
    fn room(&self) -> String {
        format!("{}/registry/{}", self.base, self.account)
    }
    fn url(&self) -> String {
        format!(
            "{}/ws?token={}",
            self.room().replacen("http:", "ws:", 1),
            self.account
        )
    }
    async fn get(&self, path: &str) -> Value {
        reqwest::Client::new()
            .get(format!("{}{path}", self.room()))
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
    async fn push(&self, body: Value) -> Value {
        reqwest::Client::new()
            .post(format!("{}/push?device=fixture", self.room()))
            .bearer_auth(&self.account)
            .json(&body)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}
async fn wait(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("registry converges");
}
fn create(id: &str, title: &str, ms: i64) -> Value {
    json!({"kind":"chats","id":id,"op":"upsert","set":{"title":title},"hlc":openagents_doc::encode_hlc(ms,0,"fixture")})
}
async fn send(ws: &mut Socket, frame: Value) {
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        frame.to_string(),
    ))
    .await
    .unwrap();
}
async fn next(ws: &mut Socket) -> Value {
    let message = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(&message.into_text().unwrap()).unwrap()
}
#[tokio::test]
async fn pinned_registry_clients_sync_sessions_replay_tombstones_and_reconnect() {
    let server = Server::start().await;
    let desktop = Arc::new(Mutex::new(RegistryDoc::new("desktop")));
    let phone = Arc::new(Mutex::new(RegistryDoc::new("phone")));
    let a = RegistryClient::connect(&server.url(), desktop.clone(), "desktop")
        .await
        .unwrap();
    let b = RegistryClient::connect(&server.url(), phone.clone(), "phone")
        .await
        .unwrap();
    {
        let mut doc = desktop.lock().unwrap();
        let space=serde_json::from_value(json!({"id":"space","deviceId":"desktop","path":"/fixture","name":"Fixture","gitDetected":false,"createdAt":"2026-10-16T00:00:00Z"})).unwrap();
        doc.upsert_space(&space).unwrap();
        let chat=serde_json::from_value(json!({"id":"chat","deviceId":"desktop","title":"First","archived":false,"spaceId":"space","createdAt":"2026-10-16T00:00:00Z"})).unwrap();
        doc.upsert_chat(&chat).unwrap();
        let session=serde_json::from_value(json!({"chatId":"chat","deviceId":"desktop","status":"working","updatedAt":"2026-10-16T00:00:00Z"})).unwrap();
        doc.upsert_session(&session).unwrap();
    }
    a.nudge();
    wait(|| {
        desktop.lock().unwrap().pending_len() == 0
            && phone.lock().unwrap().read_sessions().unwrap().len() == 1
    })
    .await;
    assert_eq!(
        phone.lock().unwrap().read_chats().unwrap()[0]
            .title
            .as_deref(),
        Some("First")
    );
    let state = server.get("/rows").await;
    let seq = state["seq"].as_u64().unwrap();
    let rows: Vec<openagents_doc::RegistryRow> =
        serde_json::from_value(state["rows"].clone()).unwrap();
    for batch in ["replay-one", "replay-two"] {
        let ack = server
            .push(json!({"batch":batch,"ops":rows.iter().map(row_to_seed_op).collect::<Vec<_>>()}))
            .await;
        assert_eq!(ack["applied"], 0);
        assert_eq!(ack["seq"], seq);
    }
    let rejoins = b.stats().rejoins;
    b.redial();
    wait(|| b.stats().rejoins > rejoins && b.stats().connected).await;
    assert!(
        phone
            .lock()
            .unwrap()
            .rename_chat("chat", "Renamed on phone")
            .unwrap()
    );
    b.nudge();
    wait(|| {
        phone.lock().unwrap().pending_len() == 0
            && desktop.lock().unwrap().read_chats().unwrap()[0]
                .title
                .as_deref()
                == Some("Renamed on phone")
    })
    .await;
    let before = server.get("/rows").await["seq"].as_u64().unwrap();
    let deleted = desktop.lock().unwrap().delete_space("space").unwrap();
    assert_eq!(deleted.chat_ids, vec!["chat"]);
    a.nudge();
    wait(|| {
        desktop.lock().unwrap().pending_len() == 0
            && phone.lock().unwrap().read_chats().unwrap().is_empty()
            && phone.lock().unwrap().read_spaces().unwrap().is_empty()
            && phone.lock().unwrap().read_sessions().unwrap().is_empty()
    })
    .await;
    let delta = server.get(&format!("/rows?since={before}")).await;
    assert_eq!(delta["full"], false);
    let tombstones = delta["rows"].as_array().unwrap();
    assert_eq!(tombstones.len(), 3);
    let after = delta["seq"].as_u64().unwrap();
    assert_eq!(after, before + 1);
    assert!(
        tombstones
            .iter()
            .all(|r| r["deleted"] == true && r["seq"] == after)
    );
    a.shutdown().await;
    b.shutdown().await;
}
#[tokio::test]
async fn exact_deltas_validation_and_transactional_batch_rollback() {
    let server = Server::start().await;
    assert_eq!(
        server
            .push(json!({"batch":"first","ops":[create("a","A",1),create("b","B",1)]}))
            .await["seq"],
        1
    );
    assert_eq!(
        server
            .push(json!({"batch":"same","ops":[create("a","A",1)]}))
            .await["applied"],
        0
    );
    let no_delta = server.get("/rows?since=1").await;
    assert_eq!(no_delta["rows"], json!([]));
    assert_eq!(no_delta["full"], false);
    assert_eq!(
        server
            .push(json!({"batch":"next","ops":[create("a","New",2)]}))
            .await["seq"],
        2
    );
    let delta = server.get("/rows?since=1").await;
    assert_eq!(delta["rows"].as_array().unwrap().len(), 1);
    assert_eq!(delta["rows"][0]["fields"]["title"], "New");
    assert_eq!(server.get("/rows?since=99").await["full"], true);
    let invalid = json!({"batch":"invalid","ops":[create("c","should not land",3),{"kind":"chats","id":"bad","op":"upsert","hlc":"junk","set":{}}]});
    let rejected = reqwest::Client::new()
        .post(format!("{}/push", server.room()))
        .bearer_auth(&server.account)
        .json(&invalid)
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 400);
    assert_eq!(server.get("/rows").await["seq"], 2);
    // A database failure after the first changed row must roll back the
    // whole batch, including the account head.
    let db = server.db.connect().await.unwrap();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    db.batch_execute(&format!("CREATE FUNCTION fail_registry_{suffix}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.account='{}' AND NEW.id='rollback-b' THEN RAISE EXCEPTION 'injected rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_registry_{suffix} BEFORE INSERT ON registry_rows FOR EACH ROW EXECUTE FUNCTION fail_registry_{suffix}();",server.account)).await.unwrap();
    let batch =
        json!({"batch":"rollback","ops":[create("rollback-a","A",3),create("rollback-b","B",3)]});
    let rejected = reqwest::Client::new()
        .post(format!("{}/push", server.room()))
        .bearer_auth(&server.account)
        .json(&batch)
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 503);
    db.batch_execute(&format!("DROP TRIGGER fail_registry_{suffix} ON registry_rows; DROP FUNCTION fail_registry_{suffix}();")).await.unwrap();
    let state = server.get("/rows").await;
    assert_eq!(state["seq"], 2);
    assert_eq!(state["rows"].as_array().unwrap().len(), 2);
    assert_eq!(server.push(batch).await["seq"], 3);
}
#[tokio::test]
async fn account_paths_and_workspace_aliases_never_share_rows_or_tokens() {
    let server = Server::start().await;
    server
        .push(json!({"batch":"owner","ops":[create("a","private",1)]}))
        .await;
    for (method, path) in [
        ("GET", "rows"),
        ("GET", "stats"),
        ("POST", "push"),
        ("POST", "reset"),
        ("POST", "push-target?device=phone"),
        ("DELETE", "push-target?device=phone"),
    ] {
        let response = reqwest::Client::new()
            .request(method.parse().unwrap(), format!("{}/{path}", server.room()))
            .bearer_auth("other-account")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 403, "{method} {path}");
    }
    let other = server
        .url()
        .replace(&format!("token={}", server.account), "token=other-account");
    assert!(tokio_tungstenite::connect_async(other).await.is_err());
    // Both accounts can belong to the same workspace. The URL is an alias;
    // the storage key always includes the authenticated account.
    let shared = format!("{}/registry/team", server.base);
    reqwest::Client::new()
        .post(format!("{shared}/push"))
        .bearer_auth(format!("{}@team", server.account))
        .json(&json!({"batch":"private","ops":[create("private","owner",2)]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let other = reqwest::Client::new()
        .get(format!("{shared}/rows"))
        .bearer_auth("other-account@team")
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(other["rows"], json!([]));
    let token = "ab".repeat(32);
    let result = reqwest::Client::new()
        .post(format!("{}/push-target?device=phone", server.room()))
        .bearer_auth(&server.account)
        .json(&json!({"token":token,"environment":"sandbox","prefs":{"done":false}}))
        .send()
        .await
        .unwrap();
    assert_eq!(result.status(), 200);
    let stats = server.get("/stats").await;
    assert_eq!(stats["pushConfigured"], false);
    assert_eq!(stats["pushLog"], json!([]));
    assert!(!stats.to_string().contains(&token));
    assert_eq!(
        stats["pushTargets"][0]["prefs"],
        json!({"done":false,"input":true,"failed":true})
    );
    assert!(!server.get("/rows").await.to_string().contains(&token));
    reqwest::Client::new()
        .delete(format!("{}/push-target?device=phone", server.room()))
        .bearer_auth(&server.account)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(server.get("/stats").await["pushTargets"], json!([]));
}
#[tokio::test]
async fn socket_rows_precede_acks_and_sequences_stay_ordered_with_other_writers() {
    let server = Server::start().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(server.url())
        .await
        .unwrap();
    send(
        &mut ws,
        json!({"t":"hello","device":"writer","cursor":null}),
    )
    .await;
    assert_eq!(next(&mut ws).await["t"], "state");
    let requests=(0..20).map(|i|{let base=server.room();let account=server.account.clone();tokio::spawn(async move{reqwest::Client::new().post(format!("{base}/push")).bearer_auth(account).json(&json!({"batch":format!("batch-{i}"),"ops":[create(&format!("row-{i}"),"other",1)]})).send().await.unwrap().error_for_status().unwrap();})}).collect::<Vec<_>>();
    send(
        &mut ws,
        json!({"t":"push","batch":"own","ops":[create("own","own",1)]}),
    )
    .await;
    let mut last = 0;
    let mut saw_own_rows = false;
    let mut saw_ack = false;
    let mut rows = 0;
    while rows < 21 || !saw_ack {
        let frame = next(&mut ws).await;
        let seq = frame["seq"].as_u64().unwrap();
        assert!(seq >= last, "a newer acknowledgment overtook older rows");
        last = seq;
        if frame["t"] == "rows" {
            rows += 1;
            if frame["rows"][0]["id"] == "own" {
                saw_own_rows = true;
            }
        }
        if frame["t"] == "ack" {
            assert!(saw_own_rows);
            assert_eq!(frame["batch"], "own");
            saw_ack = true;
        }
    }
    for r in requests {
        r.await.unwrap();
    }
    ws.close(None).await.unwrap();
}
#[tokio::test]
async fn reset_reconnect_reseeds_the_real_client() {
    let server = Server::start().await;
    let doc = Arc::new(Mutex::new(RegistryDoc::new("desktop")));
    let client = RegistryClient::connect(&server.url(), doc.clone(), "desktop")
        .await
        .unwrap();
    let space=serde_json::from_value(json!({"id":"space","deviceId":"desktop","path":"/fixture","gitDetected":false,"createdAt":"2026-10-16T00:00:00Z"})).unwrap();
    doc.lock().unwrap().upsert_space(&space).unwrap();
    client.nudge();
    wait(|| doc.lock().unwrap().pending_len() == 0).await;
    let before = client.stats().rejoins;
    reqwest::Client::new()
        .post(format!("{}/reset", server.room()))
        .bearer_auth(&server.account)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    wait(|| client.stats().rejoins > before && doc.lock().unwrap().pending_len() == 0).await;
    assert_eq!(doc.lock().unwrap().read_spaces().unwrap().len(), 1);
    let state = server.get("/rows").await;
    assert_eq!(state["rows"].as_array().unwrap().len(), 1);
    client.shutdown().await;
}

#[tokio::test]
async fn presence_relays_but_does_not_survive_a_server_restart() {
    let server = Server::start().await;
    let (mut first, _) = tokio_tungstenite::connect_async(server.url())
        .await
        .unwrap();
    let (mut second, _) = tokio_tungstenite::connect_async(server.url())
        .await
        .unwrap();
    send(
        &mut first,
        json!({"t":"hello","device":"first","cursor":null}),
    )
    .await;
    next(&mut first).await;
    send(
        &mut second,
        json!({"t":"hello","device":"second","cursor":null}),
    )
    .await;
    next(&mut second).await;
    send(&mut first, json!({"t":"presence","at":1792130000000i64})).await;
    let presence = next(&mut second).await;
    assert_eq!(presence["t"], "presence");
    assert_eq!(presence["device"], "first");
    assert_eq!(presence["at"], 1792130000000i64);
    assert!(
        server.get("/rows").await["presence"]["first"]
            .as_i64()
            .is_some()
    );
    assert_eq!(server.get("/rows").await["rows"], json!([]));
    first.close(None).await.unwrap();
    second.close(None).await.unwrap();
    let mut restarted = Server::start().await;
    restarted.account = server.account.clone();
    assert_eq!(restarted.get("/rows").await["presence"], json!({}));
}

#[tokio::test]
async fn applied_migrations_do_not_lock_live_registry_writers() {
    let server = Server::start().await;
    server.get("/rows").await;
    let mut db = server.db.connect().await.unwrap();
    let tx = db.transaction().await.unwrap();
    tx.query_one(
        "SELECT seq FROM registry_heads WHERE account=$1 FOR UPDATE",
        &[&server.account],
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), server.db.migrate())
        .await
        .expect("an already applied migration must not wait on live table locks")
        .unwrap();
    tx.rollback().await.unwrap();
}
