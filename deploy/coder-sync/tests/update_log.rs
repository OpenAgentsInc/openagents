//! Storage guarantees from edge/update-log.test.ts. Postgres has no SQLite
//! value cap; modern clients put large logical updates in checkpoints.
use coder_sync::server::{
    self, StateData, config::AuthMode, db::Database, rooms::Rooms, storage::Bucket,
};
use serde_json::Value;
use zeron_sync::chat_frames::{self as wire, frame_type as kind};
const CHUNK: usize = 1_500_000;
struct Server {
    base: String,
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
        Self::with_db(
            Database::new(
                &std::env::var("CODER_SYNC_TEST_DATABASE_URL")
                    .expect("Run scripts/coder-sync/test.sh"),
            )
            .unwrap(),
        )
        .await
    }
    async fn with_db(db: Database) -> Self {
        db.migrate().await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!(
            "http://{}/chat2/{}",
            listener.local_addr().unwrap(),
            uuid::Uuid::new_v4()
        );
        let app = server::router(StateData {
            database: db.clone(),
            bucket: Bucket::memory(),
            rooms: Rooms::default(),
            devices: Default::default(),
            auth: AuthMode::Dev,
            accounts: None,
            revision: "update-log-test".into(),
        });
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { base, db, task }
    }
    async fn push(&self, batch: &str, bytes: &[u8]) -> Value {
        reqwest::Client::new()
            .post(format!("{}/rows?batchId={batch}&device=fixture", self.base))
            .bearer_auth("update-log-fixture")
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
    async fn checkpoint(&self, seq: u64, bytes: &[u8]) {
        assert!(
            reqwest::Client::new()
                .post(format!("{}/checkpoint?seqCovered={seq}", self.base))
                .header("x-chat2-frontier", "ZnJvbnRpZXI=")
                .bearer_auth("update-log-fixture")
                .body(bytes.to_vec())
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
    }
    async fn snapshot(&self) -> Vec<u8> {
        reqwest::Client::new()
            .get(format!("{}/checkpoint", self.base))
            .bearer_auth("update-log-fixture")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .bytes()
            .await
            .unwrap()
            .to_vec()
    }
    async fn rows(&self) -> Vec<wire::WireFrame> {
        let bytes = reqwest::Client::new()
            .get(format!("{}/rows", self.base))
            .bearer_auth("update-log-fixture")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let mut bytes = &bytes[..];
        let mut frames = Vec::new();
        while !bytes.is_empty() {
            let n = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
            frames.push(wire::decode(&bytes[4..4 + n]).unwrap());
            bytes = &bytes[4 + n..];
        }
        frames.into_iter().filter(|f| f.kind == kind::ROW).collect()
    }
}
fn bytes(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| seed.wrapping_add((i * 31) as u8))
        .collect()
}
#[tokio::test]
async fn small_update_is_one_logical_row() {
    let s = Server::start().await;
    let data = bytes(1000, 1);
    assert_eq!(s.push("small", &data).await["seq"], 1);
    let rows = s.rows().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].payload, data);
}
#[tokio::test]
async fn whale_above_the_old_sqlite_cap_roundtrips_byte_identically() {
    let s = Server::start().await;
    s.push("seed", b"seed").await;
    let whale = bytes(3 * CHUNK + 123, 7);
    s.checkpoint(1, &whale).await;
    assert_eq!(s.snapshot().await, whale);
    assert!(s.rows().await.is_empty());
}
#[tokio::test]
async fn interleaved_small_and_large_updates_preserve_order_after_fold() {
    let s = Server::start().await;
    let a = bytes(10, 1);
    let b = bytes(CHUNK + 5, 2);
    let c = bytes(20, 3);
    s.push("a", &a).await;
    s.checkpoint(1, &[a.clone(), b.clone()].concat()).await;
    assert_eq!(s.push("c", &c).await["seq"], 2);
    let snapshot = s.snapshot().await;
    let rows = s.rows().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        [snapshot, rows[0].payload.clone()].concat(),
        [a, b, c].concat()
    );
}
#[tokio::test]
async fn a_sliced_update_keeps_its_offset_and_ragged_tail() {
    let s = Server::start().await;
    s.push("seed", b"seed").await;
    let backing = bytes(CHUNK + 200, 9);
    let view = &backing[100..CHUNK + 150];
    s.checkpoint(1, view).await;
    assert_eq!(s.snapshot().await, view);
}
#[tokio::test]
async fn pre_protocol_rows_migrate_and_remain_replayable() {
    let dsn = std::env::var("CODER_SYNC_TEST_DATABASE_URL").unwrap();
    let admin = Database::new(&dsn).unwrap();
    let schema = format!("legacy_{}", uuid::Uuid::new_v4().simple());
    admin
        .connect()
        .await
        .unwrap()
        .batch_execute(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let db = Database::new(&format!("{dsn} options='-c search_path={schema}'")).unwrap();
    let sql = db.connect().await.unwrap();
    sql.batch_execute(include_str!(
        "../../../crates/coder-sync/migrations/0001_rooms.sql"
    ))
    .await
    .unwrap();
    let chat = uuid::Uuid::new_v4().to_string();
    sql.execute(
        "INSERT INTO chats(chat_id,account,head) VALUES($1,'update-log-fixture',2)",
        &[&chat],
    )
    .await
    .unwrap();
    for (seq, data) in [(1i64, bytes(10, 1)), (2, bytes(11, 2))] {
        sql.execute(
            "INSERT INTO chat_rows(chat_id,seq,batch_id,bytes) VALUES($1,$2,$3,$4)",
            &[&chat, &seq, &format!("legacy-{seq}"), &data],
        )
        .await
        .unwrap();
    }
    let mut s = Server::with_db(db).await;
    s.base = format!("{}/{}", s.base.rsplit_once('/').unwrap().0, chat);
    let rows = s.rows().await;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].payload, bytes(10, 1));
    assert_eq!(rows[1].payload, bytes(11, 2));
    assert_eq!(s.push("legacy-1", b"retry").await["dup"], true);
    assert_eq!(s.push("new", b"third").await["seq"], 3);
    s.checkpoint(3, &bytes(CHUNK + 1, 3)).await;
    assert!(s.rows().await.is_empty());
}
#[tokio::test]
async fn migration_is_idempotent_with_updates_present() {
    let s = Server::start().await;
    let data = bytes(10, 1);
    s.push("before", &data).await;
    s.db.migrate().await.unwrap();
    s.db.migrate().await.unwrap();
    let rows = s.rows().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].payload, data);
    assert_eq!(s.push("after", b"next").await["seq"], 2);
}
#[tokio::test]
async fn real_loro_whale_fold_and_post_fold_replay_converge() {
    let s = Server::start().await;
    let writer = loro::LoroDoc::new();
    let mut rng = 0x12345678u32;
    let text = (0..3 * CHUNK)
        .map(|_| {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            (b'a' + (rng % 26) as u8) as char
        })
        .collect::<String>();
    writer.get_text("t").insert(0, &text).unwrap();
    writer.commit();
    let whale = writer.export(loro::ExportMode::all_updates()).unwrap();
    assert!(whale.len() > CHUNK);
    // The modern checkpoint carries the large logical update without a SQL value cap.
    s.push("seed", b"seed").await;
    s.checkpoint(1, &whale).await;
    let replayed = loro::LoroDoc::new();
    replayed.import(&s.snapshot().await).unwrap();
    assert_eq!(replayed.get_text("t").to_string(), text);
    let snapshot = replayed.export(loro::ExportMode::Snapshot).unwrap();
    s.checkpoint(1, &snapshot).await;
    let before = writer.oplog_vv();
    writer.get_text("t").insert(0, "post-fold ").unwrap();
    writer.commit();
    let delta = writer.export(loro::ExportMode::updates(&before)).unwrap();
    assert_eq!(s.push("post-fold", &delta).await["seq"], 2);
    let cold = loro::LoroDoc::new();
    cold.import(&s.snapshot().await).unwrap();
    for row in s.rows().await {
        cold.import(&row.payload).unwrap();
    }
    assert_eq!(
        cold.get_text("t").to_string(),
        writer.get_text("t").to_string()
    );
}
