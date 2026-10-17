use coder_sync::server::{
    self, StateData, config::AuthMode, db::Database, rooms::Rooms, storage::Bucket,
};
use futures::{SinkExt, StreamExt, future::BoxFuture};
use openagents_sync::{
    ChatClient, ChatDocSink, CheckpointFetcher, StaticUrl, SyncError,
    chat_client::{ChatTransport, RowImportOutcome},
    chat_frames::{self as wire, frame_type as kind},
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[test]
fn edge_frame_vectors_and_rejections() {
    let bytes = wire::encode(kind::PUSH, &json!({"batchId":"b1"}), &[9, 8, 7]);
    assert_eq!(
        bytes,
        [
            vec![6, 16, 0, 0, 0],
            br#"{"batchId":"b1"}"#.to_vec(),
            vec![9, 8, 7]
        ]
        .concat()
    );
    for k in 1..=11 {
        for payload in [
            vec![],
            (0..1000)
                .map(|i| ((k as usize + i * 31) % 256) as u8)
                .collect(),
        ] {
            let encoded = wire::encode(k, &json!({"seq":7}), &payload);
            let mut shifted = vec![0; 8];
            shifted.extend(&encoded);
            let decoded = server::chat::decode(&shifted[8..]).unwrap();
            assert_eq!(decoded.kind, k);
            assert_eq!(decoded.header, json!({"seq":7}));
            assert_eq!(decoded.payload, payload);
        }
    }
    for input in [
        vec![],
        vec![1],
        wire::encode(127, &json!({}), &[]),
        wire::encode(1, &json!([1]), &[]),
        wire::encode(1, &json!({"pad":"x".repeat(4096)}), &[]),
        vec![1, 2, 0, 0, 0, b'{', b'{'],
        vec![1, 15, 39, 0, 0, b'{', b'}'],
    ] {
        assert!(server::chat::decode(&input).is_none(), "{input:?}");
    }
}
struct TestServer {
    base: String,
    db: Database,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl TestServer {
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
            revision: "chat-test".into(),
        });
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { base, db, task }
    }
    fn room(&self) -> String {
        format!("{}/chat2/{}", self.base, uuid::Uuid::new_v4())
    }
}
#[derive(Default)]
struct Sink(Mutex<(u64, Vec<Vec<u8>>)>);
impl ChatDocSink for Sink {
    fn cursor_is_verified(&self) -> bool {
        true
    }
    fn reset_cursor(&self, c: u64) {
        self.0.lock().unwrap().0 = c;
    }
    fn apply_row(&self, b: &[u8], c: u64) -> RowImportOutcome {
        let mut s = self.0.lock().unwrap();
        if c > s.0 {
            s.1.push(b.to_vec());
            s.0 = c;
        }
        RowImportOutcome::Applied
    }
    fn apply_checkpoint(&self, b: &[u8], c: u64) -> Result<(), String> {
        let mut s = self.0.lock().unwrap();
        s.0 = c;
        s.1 = vec![b.to_vec()];
        Ok(())
    }
    fn contains_frontier(&self, _: &[u8]) -> bool {
        false
    }
    fn advance_cursor(&self, c: u64) {
        let mut s = self.0.lock().unwrap();
        s.0 = s.0.max(c);
    }
}
#[derive(Clone)]
struct Http {
    room: String,
    device: String,
}
impl Http {
    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .get(format!("{}{path}", self.room))
            .bearer_auth("chat-fixture")
    }
}
impl CheckpointFetcher for Http {
    fn fetch(&self) -> BoxFuture<'static, Result<Vec<u8>, SyncError>> {
        let this = self.clone();
        Box::pin(async move {
            Ok(this
                .get("/checkpoint")
                .send()
                .await
                .map_err(|e| SyncError::Protocol(e.to_string()))?
                .error_for_status()
                .map_err(|e| SyncError::Protocol(e.to_string()))?
                .bytes()
                .await
                .map_err(|e| SyncError::Protocol(e.to_string()))?
                .to_vec())
        })
    }
}
impl ChatTransport for Http {
    fn fetch_rows(&self, after: u64) -> BoxFuture<'static, Result<Vec<u8>, SyncError>> {
        let this = self.clone();
        Box::pin(async move {
            Ok(this
                .get("/rows")
                .query(&[("after", after.to_string())])
                .send()
                .await
                .map_err(|e| SyncError::Protocol(e.to_string()))?
                .error_for_status()
                .map_err(|e| SyncError::Protocol(e.to_string()))?
                .bytes()
                .await
                .map_err(|e| SyncError::Protocol(e.to_string()))?
                .to_vec())
        })
    }
    fn push(
        &self,
        batch_id: String,
        bytes: Vec<u8>,
    ) -> BoxFuture<'static, Result<String, SyncError>> {
        let this = self.clone();
        Box::pin(async move {
            reqwest::Client::new()
                .post(format!("{}/rows", this.room))
                .bearer_auth("chat-fixture")
                .query(&[("batchId", batch_id), ("device", this.device)])
                .body(bytes)
                .send()
                .await
                .map_err(|e| SyncError::Protocol(e.to_string()))?
                .error_for_status()
                .map_err(|e| SyncError::Protocol(e.to_string()))?
                .text()
                .await
                .map_err(|e| SyncError::Protocol(e.to_string()))
        })
    }
}
async fn wait(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("client converges");
}
async fn push(room: &str, batch: &str, bytes: &[u8]) -> Value {
    reqwest::Client::new()
        .post(format!("{room}/rows"))
        .bearer_auth("chat-fixture")
        .query(&[("batchId", batch), ("device", "writer")])
        .body(bytes.to_vec())
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn checkpoint(room: &str, seq: u64, bytes: &[u8]) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("{room}/checkpoint"))
        .bearer_auth("chat-fixture")
        .query(&[("seqCovered", seq)])
        .header("x-chat2-frontier", "ZnJvbnRpZXI=")
        .body(bytes.to_vec())
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn real_pinned_client_checkpoint_push_retry_reconnect_and_http_fallback() {
    let server = TestServer::start().await;
    let room = server.room();
    assert_eq!(push(&room, "seed", b"initial").await["seq"], 1);
    assert!(
        checkpoint(&room, 1, b"snapshot")
            .await
            .status()
            .is_success()
    );
    let http = Arc::new(Http {
        room: room.clone(),
        device: "reader".into(),
    });
    let sink = Arc::new(Sink::default());
    let ws = room.replacen("http:", "ws:", 1) + "/ws?token=chat-fixture";
    let client = ChatClient::connect(&ws, sink.clone(), http.clone(), "reader", 0)
        .await
        .unwrap();
    assert_eq!(*sink.0.lock().unwrap(), (1, vec![b"snapshot".to_vec()]));
    client.enqueue_batch("second".into(), b"update".to_vec());
    wait(|| client.stats().pending_pushes == 0 && client.stats().cursor == 2).await;
    // The client's per-connection sent set intentionally suppresses a
    // duplicate on the same socket. Reconnect to replay the retained batch.
    client.enqueue_batch("second".into(), b"update".to_vec());
    client.redial();
    wait(|| client.stats().pending_pushes == 0).await;
    assert_eq!(
        push(&room, "second", b"update").await,
        json!({"batchId":"second","seq":2,"dup":true})
    );
    let rejoins = client.stats().rejoins;
    client.redial();
    wait(|| client.stats().rejoins > rejoins && client.caught_up()).await;
    assert_eq!(push(&room, "third", b"live").await["seq"], 3);
    wait(|| sink.0.lock().unwrap().0 == 3).await;
    assert_eq!(sink.0.lock().unwrap().1.last().unwrap(), b"live");
    client.shutdown().await;
    // Force failed upgrades against a real HTTP-only endpoint. The actual
    // pinned client's transport fallback still downloads and publishes.
    let fallback_sink = Arc::new(Sink::default());
    let fallback = ChatClient::connect_via_transport(
        Arc::new(StaticUrl(
            room.replacen("http:", "ws:", 1) + "/rows?token=chat-fixture",
        )),
        fallback_sink.clone(),
        http.clone(),
        "fallback",
        0,
        http,
    )
    .await
    .unwrap();
    wait(|| fallback_sink.0.lock().unwrap().0 == 3).await;
    fallback.enqueue_batch("over-http".into(), b"fallback".to_vec());
    wait(|| fallback.stats().pending_pushes == 0 && fallback.stats().cursor == 4).await;
    assert_eq!(push(&room, "over-http", b"fallback").await["dup"], true);
    fallback.shutdown().await;
}

#[tokio::test]
async fn concurrent_writers_compaction_retry_and_account_isolation() {
    let server = TestServer::start().await;
    let room = server.room();
    let jobs = (0..20).map(|i| {
        let room = room.clone();
        tokio::spawn(async move { push(&room, &format!("batch-{i}"), b"row").await })
    });
    let mut seqs = Vec::new();
    for job in jobs.collect::<Vec<_>>() {
        seqs.push(job.await.unwrap()["seq"].as_u64().unwrap());
    }
    seqs.sort();
    assert_eq!(seqs, (1..=20).collect::<Vec<_>>());
    assert_eq!(checkpoint(&room, 21, b"ahead").await.status(), 409);
    assert_eq!(checkpoint(&room, 10, b"checkpoint").await.status(), 200);
    assert_eq!(checkpoint(&room, 9, b"regress").await.status(), 409);
    let first = push(&room, "batch-0", b"row").await;
    assert_eq!(first["dup"], true);
    assert!(first["seq"].as_u64().unwrap() <= 20);
    let key = room.rsplit('/').next().unwrap();
    let db = server.db.connect().await.unwrap();
    assert_eq!(
        db.query_one("SELECT head FROM chats WHERE chat_id=$1", &[&key])
            .await
            .unwrap()
            .get::<_, i64>(0),
        20
    );
    assert_eq!(
        db.query_one("SELECT min(seq) FROM chat_rows WHERE chat_id=$1", &[&key])
            .await
            .unwrap()
            .get::<_, i64>(0),
        11
    );
    for path in ["rows", "checkpoint", "ws"] {
        let response = reqwest::Client::new()
            .get(format!("{room}/{path}"))
            .bearer_auth("other-account")
            .send()
            .await
            .unwrap();
        // Upgrade headers are required before the WebSocket handler runs.
        assert_eq!(response.status(), if path == "ws" { 426 } else { 403 });
    }
    let http = Http {
        room: room.clone(),
        device: "".into(),
    };
    let range = http
        .get("/checkpoint")
        .header("range", "bytes=4-")
        .send()
        .await
        .unwrap();
    assert_eq!(range.status(), 206);
    assert_eq!(range.headers()["content-range"], "bytes 4-9/10");
    assert_eq!(range.bytes().await.unwrap(), b"kpoint"[..]);
    assert_eq!(
        http.get("/checkpoint")
            .header("range", "bytes=10-")
            .send()
            .await
            .unwrap()
            .status(),
        416
    );
    assert_eq!(
        http.get("/checkpoint")
            .header("range", "bytes=0-")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}

#[tokio::test]
async fn websocket_errors_presence_and_ping() {
    let server = TestServer::start().await;
    let room = server.room();
    let url = room.replacen("http:", "ws:", 1) + "/ws?token=chat-fixture";
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    ws.send(tokio_tungstenite::tungstenite::Message::Binary(vec![
        127, 2, 0, 0, 0, b'{', b'}',
    ]))
    .await
    .unwrap();
    let f = wire::decode(&ws.next().await.unwrap().unwrap().into_data()).unwrap();
    assert_eq!(f.header["code"], "bad_frame");
    ws.send(tokio_tungstenite::tungstenite::Message::Binary(
        wire::encode(kind::ROWS_REQ, &json!({"after":0}), &[]),
    ))
    .await
    .unwrap();
    let f = wire::decode(&ws.next().await.unwrap().unwrap().into_data()).unwrap();
    assert_eq!(f.header["code"], "hello_first");
    ws.send(tokio_tungstenite::tungstenite::Message::Text("ping".into()))
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_text().unwrap(),
        "pong"
    );
    ws.send(tokio_tungstenite::tungstenite::Message::Binary(
        wire::encode(kind::HELLO, &json!({"device":"first","cursor":0}), &[]),
    ))
    .await
    .unwrap();
    let state = wire::decode(&ws.next().await.unwrap().unwrap().into_data()).unwrap();
    assert_eq!(state.kind, kind::STATE);
    let (mut peer, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    peer.send(tokio_tungstenite::tungstenite::Message::Binary(
        wire::encode(kind::HELLO, &json!({"device":"second","cursor":0}), &[]),
    ))
    .await
    .unwrap();
    peer.next().await.unwrap().unwrap();
    peer.send(tokio_tungstenite::tungstenite::Message::Binary(
        wire::encode(kind::PRESENCE, &json!({"at":123}), b"presence"),
    ))
    .await
    .unwrap();
    let presence = wire::decode(&ws.next().await.unwrap().unwrap().into_data()).unwrap();
    assert_eq!(presence.kind, kind::PRESENCE);
    assert_eq!(presence.header["device"], "second");
    assert_eq!(presence.payload, b"presence");
    let other = url.replace("chat-fixture", "other-account");
    assert!(tokio_tungstenite::connect_async(other).await.is_err());
    ws.close(None).await.unwrap();
    peer.close(None).await.unwrap();
}

#[tokio::test]
async fn transaction_failure_has_no_sequence_gap_and_retry_after_compaction_is_once() {
    let server = TestServer::start().await;
    let room = server.room();
    let id = room.rsplit('/').next().unwrap();
    let db = server.db.connect().await.unwrap();
    // Simulate failure after INSERTs but before advancing the head. This is
    // the crash boundary the model treats as an uncommitted transaction.
    db.batch_execute(&format!("CREATE FUNCTION fail_head_{}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.chat_id='{}' THEN RAISE EXCEPTION 'injected rollback'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_head_{} BEFORE UPDATE ON chats FOR EACH ROW EXECUTE FUNCTION fail_head_{}();",id.replace('-',"_"),id,id.replace('-',"_"),id.replace('-',"_"))).await.unwrap();
    let response = reqwest::Client::new()
        .post(format!("{room}/rows?batchId=retry"))
        .bearer_auth("chat-fixture")
        .body("update")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    db.batch_execute(&format!(
        "DROP TRIGGER fail_head_{} ON chats; DROP FUNCTION fail_head_{}();",
        id.replace('-', "_"),
        id.replace('-', "_")
    ))
    .await
    .unwrap();
    assert_eq!(push(&room, "retry", b"update").await["seq"], 1);
    assert_eq!(checkpoint(&room, 1, b"checkpoint").await.status(), 200);
    assert_eq!(
        push(&room, "retry", b"update").await,
        json!({"batchId":"retry","seq":1,"dup":true})
    );
    assert_eq!(push(&room, "next", b"next").await["seq"], 2);
    let bad_frontier = reqwest::Client::new()
        .post(format!("{room}/checkpoint?seqCovered=2"))
        .bearer_auth("chat-fixture")
        .body("checkpoint")
        .send()
        .await
        .unwrap();
    assert_eq!(bad_frontier.status(), 400);
    let empty = reqwest::Client::new()
        .post(format!("{room}/rows?batchId=empty"))
        .bearer_auth("chat-fixture")
        .body("")
        .send()
        .await
        .unwrap();
    assert_eq!(empty.status(), 400);
}

#[tokio::test]
async fn paginated_http_and_websocket_replay_do_not_skip_rows() {
    let server = TestServer::start().await;
    let room = server.room();
    let id = room.rsplit('/').next().unwrap();
    let db = server.db.connect().await.unwrap();
    db.execute(
        "INSERT INTO chats(chat_id,account,head) VALUES($1,'chat-fixture',300)",
        &[&id],
    )
    .await
    .unwrap();
    db.execute("INSERT INTO chat_rows(chat_id,seq,batch_id,bytes,device) SELECT $1,n,'batch-'||n,decode('01','hex'),'writer' FROM generate_series(1,300) n",&[&id]).await.unwrap();
    let http = Arc::new(Http {
        room: room.clone(),
        device: "reader".into(),
    });
    fn unpack(mut bytes: &[u8]) -> Vec<wire::WireFrame> {
        let mut out = vec![];
        while !bytes.is_empty() {
            let n = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
            out.push(wire::decode(&bytes[4..4 + n]).unwrap());
            bytes = &bytes[4 + n..];
        }
        out
    }
    let page = unpack(&http.fetch_rows(0).await.unwrap());
    assert_eq!(page.len(), 257);
    assert_eq!(page.last().unwrap().header["seq"], 256);
    let page = unpack(&http.fetch_rows(256).await.unwrap());
    assert_eq!(page.len(), 46);
    assert_eq!(page.last().unwrap().kind, kind::ROWS_DONE);
    let sink = Arc::new(Sink::default());
    let client = ChatClient::connect(
        &(room.replacen("http:", "ws:", 1) + "/ws?token=chat-fixture"),
        sink.clone(),
        http,
        "reader",
        0,
    )
    .await
    .unwrap();
    assert_eq!(sink.0.lock().unwrap().0, 300);
    assert_eq!(sink.0.lock().unwrap().1.len(), 300);
    client.shutdown().await;
}

#[tokio::test]
async fn another_account_cannot_spend_the_owners_push_quota() {
    let server = TestServer::start().await;
    let room = server.room();
    assert_eq!(push(&room, "owner-first", b"first").await["seq"], 1);
    for _ in 0..9 {
        let response = reqwest::Client::new()
            .post(format!("{room}/rows?batchId=intruder&device=writer"))
            .bearer_auth("another-account")
            .body(vec![0u8; 1024 * 1024])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
    }
    assert_eq!(push(&room, "owner-next", b"next").await["seq"], 2);
}
