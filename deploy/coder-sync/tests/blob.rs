use coder_sync::server::{
    self, StateData, config::AuthMode, db::Database, rooms::Rooms, storage::Bucket,
};
use reqwest::{Method, Response};
use serde_json::json;
static CLEANUP_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Server {
    base: String,
    account: String,
    chat: String,
    state: StateData,
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
        let state = StateData {
            database: db,
            bucket: Bucket::memory(),
            rooms: Rooms::default(),
            devices: Default::default(),
            auth: AuthMode::Dev,
            accounts: None,
            revision: "blob-test".into(),
            push: None,
        };
        let app = server::router(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base,
            account: format!("blob-{}", uuid::Uuid::new_v4()),
            chat: uuid::Uuid::new_v4().to_string(),
            state,
            task,
        }
    }
    fn url(&self, part: &str) -> String {
        format!("{}/blob/{}/{part}", self.base, self.chat)
    }
    fn request(&self, method: Method, part: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .request(method, self.url(part))
            .bearer_auth(&self.account)
    }
    async fn put(&self, part: &str, bytes: Vec<u8>) -> Response {
        self.request(Method::PUT, part)
            .header("content-type", "image/png")
            .body(bytes)
            .send()
            .await
            .unwrap()
    }
    async fn get(&self, part: &str) -> Response {
        self.request(Method::GET, part).send().await.unwrap()
    }
}
#[tokio::test]
async fn mime_etags_head_ranges_and_replacement() {
    let s = Server::start().await;
    let original = (0..100u8).collect::<Vec<_>>();
    let response = s.put("m1%23c1.diff", original.clone()).await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        json!({"ok":true,"bytes":100})
    );
    let response = s.get("m1%23c1.diff").await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.headers()["cache-control"], "private, max-age=300");
    let etag = response.headers()["etag"].to_str().unwrap().to_owned();
    assert!(etag.starts_with('"') && etag.ends_with('"'));
    assert_eq!(&response.bytes().await.unwrap()[..], &original);
    let response = s
        .request(Method::HEAD, "m1%23c1.diff")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-length"], "100");
    assert_eq!(response.bytes().await.unwrap().len(), 0);
    for value in [
        etag.clone(),
        format!("W/{etag}"),
        format!("\"other\", {etag}"),
        "*".into(),
    ] {
        let r = s
            .request(Method::GET, "m1%23c1.diff")
            .header("if-none-match", value)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 304);
        assert!(r.bytes().await.unwrap().is_empty());
    }
    for (range, start, end) in [
        ("bytes=2-7", 2, 7),
        ("bytes=90-", 90, 99),
        ("bytes=-9", 91, 99),
        ("bytes=97-999", 97, 99),
        ("bytes=0-0", 0, 0),
        ("bytes=-500", 0, 99),
    ] {
        let r = s
            .request(Method::GET, "m1%23c1.diff")
            .header("range", range)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 206);
        assert_eq!(
            r.headers()["content-range"],
            format!("bytes {start}-{end}/100")
        );
        assert_eq!(&r.bytes().await.unwrap()[..], &original[start..=end]);
    }
    for value in [
        "bytes=100-",
        "bytes=20-2",
        "bytes=-0",
        "bytes=1-2,5-6",
        "bytes=abc-",
        "bytes=999999999999999999999999999-",
    ] {
        let r = s
            .request(Method::GET, "m1%23c1.diff")
            .header("range", value)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 416);
        assert_eq!(r.headers()["content-range"], "bytes */100");
    }
    let r = s
        .request(Method::GET, "m1%23c1.diff")
        .header("range", "bytes=1-2")
        .header("if-range", &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 206);
    let r = s
        .request(Method::GET, "m1%23c1.diff")
        .header("range", "bytes=1-2")
        .header("if-range", "\"old\"")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.bytes().await.unwrap().len(), 100);
    assert_eq!(
        s.put("m1%23c1.diff", b"replacement".to_vec())
            .await
            .status(),
        200
    );
    let r = s.get("m1%23c1.diff").await;
    assert_ne!(r.headers()["etag"], etag);
    assert_eq!(&r.bytes().await.unwrap()[..], b"replacement");
}
#[tokio::test]
async fn account_isolation_size_limits_and_legacy_ack() {
    let s = Server::start().await;
    assert_eq!(s.put("part", vec![42; 1024 * 1024]).await.status(), 200);
    let other = format!("other-{}", s.account);
    for method in [Method::GET, Method::HEAD, Method::PUT] {
        let r = reqwest::Client::new()
            .request(method, s.url("part"))
            .bearer_auth(&other)
            .body("wrong")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403);
    }
    assert_eq!(reqwest::get(s.url("part")).await.unwrap().status(), 401);
    assert_eq!(
        s.put("too-large", vec![0; 1024 * 1024 + 1]).await.status(),
        413
    );
    assert_eq!(s.get("too-large").await.status(), 404);
    for part in ["bad%2Fpart", "bad%20part", "bad%5Cpart", &"x".repeat(201)] {
        assert_eq!(s.put(part, vec![]).await.status(), 400);
    }
    let r = reqwest::Client::new()
        .put(format!("{}/attachments/{}/legacy.png", s.base, s.chat))
        .bearer_auth(&s.account)
        .body(vec![0; 2 * 1024 * 1024])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(
        r.json::<serde_json::Value>().await.unwrap(),
        json!({"ok":true})
    );
    let db = s.state.database.connect().await.unwrap();
    assert_eq!(
        db.query_one("SELECT count(*) FROM blobs WHERE account=$1", &[&s.account])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}
#[tokio::test]
async fn default_content_type_and_empty_objects() {
    let s = Server::start().await;
    let r = s
        .request(Method::PUT, "empty")
        .body(Vec::new())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let r = s.get("empty").await;
    assert_eq!(r.headers()["content-type"], "text/plain; charset=utf-8");
    assert!(r.bytes().await.unwrap().is_empty());
    let r = s
        .request(Method::GET, "empty")
        .header("range", "bytes=0-")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 416);
    assert_eq!(r.headers()["content-range"], "bytes */0");
}
#[tokio::test]
async fn orphan_cleanup_preserves_live_objects_and_inflight_versions() {
    let _guard = CLEANUP_TEST.lock().await;
    let s = Server::start().await;
    let db = s.state.database.connect().await.unwrap();
    assert_eq!(s.put("part", b"first".to_vec()).await.status(), 200);
    let old: String = db
        .query_one("SELECT object FROM blobs WHERE account=$1", &[&s.account])
        .await
        .unwrap()
        .get(0);
    assert_eq!(s.put("part", b"second".to_vec()).await.status(), 200);
    assert_eq!(server::blob::collect_orphans(&s.state).await.unwrap(), 0);
    assert!(s.state.bucket.get(&old).await.unwrap().is_some());
    db.execute(
        "UPDATE blob_gc SET queued_at=now()-interval '8 days' WHERE object=$1",
        &[&old],
    )
    .await
    .unwrap();
    assert_eq!(server::blob::collect_orphans(&s.state).await.unwrap(), 1);
    assert!(s.state.bucket.get(&old).await.unwrap().is_none());
    assert_eq!(&s.get("part").await.bytes().await.unwrap()[..], b"second");
    let live: String = db
        .query_one("SELECT object FROM blobs WHERE account=$1", &[&s.account])
        .await
        .unwrap()
        .get(0);
    db.execute(
        "INSERT INTO blob_gc(object,queued_at) VALUES($1,now()-interval '8 days')",
        &[&live],
    )
    .await
    .unwrap();
    assert_eq!(server::blob::collect_orphans(&s.state).await.unwrap(), 0);
    assert_eq!(&s.get("part").await.bytes().await.unwrap()[..], b"second");
    db.execute("DELETE FROM chats WHERE chat_id=$1", &[&s.chat])
        .await
        .unwrap();
    db.execute(
        "UPDATE blob_gc SET queued_at=now()-interval '8 days' WHERE object=$1",
        &[&live],
    )
    .await
    .unwrap();
    assert_eq!(server::blob::collect_orphans(&s.state).await.unwrap(), 1);
    assert!(s.state.bucket.get(&live).await.unwrap().is_none());
}
#[tokio::test]
async fn failed_publication_keeps_the_previous_version_and_tracks_the_orphan() {
    let _guard = CLEANUP_TEST.lock().await;
    let s = Server::start().await;
    assert_eq!(s.put("part", b"published".to_vec()).await.status(), 200);
    let db = s.state.database.connect().await.unwrap();
    let name = format!("blob_fail_{}", uuid::Uuid::new_v4().simple());
    db.batch_execute(&format!("CREATE FUNCTION {name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.account='{}' THEN RAISE EXCEPTION 'fixture failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER {name} BEFORE INSERT OR UPDATE ON blobs FOR EACH ROW EXECUTE FUNCTION {name}();",s.account)).await.unwrap();
    assert_eq!(s.put("part", b"unpublished".to_vec()).await.status(), 503);
    assert_eq!(
        &s.get("part").await.bytes().await.unwrap()[..],
        b"published"
    );
    let prefix = s
        .account
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let key: String = db
        .query_one(
            "SELECT object FROM blob_gc WHERE object LIKE $1",
            &[&format!("blob/{prefix}/%")],
        )
        .await
        .unwrap()
        .get(0);
    assert!(s.state.bucket.get(&key).await.unwrap().is_some());
    let staged = key.replacen("blob/", "orphan/", 1);
    assert!(s.state.bucket.get(&staged).await.unwrap().is_some());
    db.batch_execute(&format!(
        "DROP TRIGGER {name} ON blobs; DROP FUNCTION {name}();"
    ))
    .await
    .unwrap();
    db.execute(
        "UPDATE blob_gc SET queued_at=now()-interval '8 days' WHERE object=$1",
        &[&key],
    )
    .await
    .unwrap();
    assert_eq!(server::blob::collect_orphans(&s.state).await.unwrap(), 1);
    assert!(s.state.bucket.get(&key).await.unwrap().is_none());
    assert_eq!(
        &s.get("part").await.bytes().await.unwrap()[..],
        b"published"
    );
}
#[test]
fn lifecycle_only_expires_stages_and_health_probes() {
    let rules: serde_json::Value = serde_json::from_str(include_str!("../lifecycle.json")).unwrap();
    assert_eq!(
        rules["rule"],
        json!([{"action":{"type":"Delete"},"condition":{"age":7,"matchesPrefix":["orphan/"]}},{"action":{"type":"Delete"},"condition":{"age":1,"matchesPrefix":["_health/"]}}])
    );
}
