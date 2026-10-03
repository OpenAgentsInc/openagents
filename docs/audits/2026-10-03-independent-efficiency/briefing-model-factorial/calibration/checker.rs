//! Linux integration checks through the historical public observer API.
use coder_connect::coder_history::{CatalogPage, CatalogRequest, Config};
use coder_connect::direct::{self, Change, Connection, Hello, Welcome};
use coder_connect::host::Host;
use coder_connect::protocol::{Route, pubkey};
use coder_connect::{Client, Observation, Query, RelayPolicy, unix_time};
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::broadcast;

const NUDGE_LIMIT: Duration = Duration::from_secs(4);
const QUIET_WINDOW: Duration = Duration::from_millis(300);

struct Endpoint {
    _temp: tempfile::TempDir,
    source: PathBuf,
    outside: PathBuf,
    client: Client,
    connection: Connection,
    changes: broadcast::Receiver<Change>,
    server: tokio::task::JoinHandle<()>,
}

fn write_chat(path: &Path, id: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let record = serde_json::json!({"type": "session_meta", "payload": {"id": id}});
    std::fs::write(path, format!("{record}\n")).unwrap();
}

impl Endpoint {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("observed");
        let outside = temp.path().join("observed-extra");
        std::fs::create_dir_all(&outside).unwrap();
        write_chat(&source.join("sessions/2026/09/28/initial.jsonl"), "initial");
        let host = Arc::new(Host::new(
            temp.path().join("state"),
            RelayPolicy::LoopbackTest,
        ));
        let secret = SecretKey::new(&mut secp256k1::rand::rng());
        let now = unix_time().unwrap();
        let code = host
            .pair(
                &pubkey(&secret),
                // This URL is a grant field. The fixture only uses Connection::exchange.
                "ws://127.0.0.1:1",
                Config {
                    codex: Some(source.clone()),
                    ..Config::default()
                },
                now,
                now + 3600,
            )
            .unwrap();
        let client = Client::new_with_policy(code, secret, RelayPolicy::LoopbackTest).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (read, mut write) = stream.into_split();
            let mut reader = BufReader::new(read);
            let hello = direct::line(&mut reader, direct::MAX_HELLO_BYTES)
                .await
                .unwrap()
                .unwrap();
            assert!(Hello::parse(&hello).is_some());
            let mut welcome = serde_json::to_vec(&Welcome {
                v: direct::HELLO.into(),
                refused: None,
            })
            .unwrap();
            welcome.push(b'\n');
            write.write_all(&welcome).await.unwrap();
            direct::serve(reader, write, host).await;
        });
        let (send, changes) = broadcast::channel(64);
        let connection = Connection::open(address, send).await.unwrap();
        Self {
            _temp: temp,
            source,
            outside,
            client,
            connection,
            changes,
            server,
        }
    }

    async fn catalog(&self) -> CatalogPage {
        let now = unix_time().unwrap();
        let request = self
            .client
            .prepare_for(
                Query::Catalog(CatalogRequest::default()),
                now,
                Route::Direct,
            )
            .unwrap();
        let (event, payload) = self
            .connection
            .exchange(&request.event, Duration::from_secs(5))
            .await
            .unwrap();
        let observation = self
            .client
            .verify_detached(&request, &event, payload.as_deref().unwrap(), now)
            .unwrap();
        let Observation::Catalog(page) = observation else {
            panic!("A catalog request must return a catalog.");
        };
        page
    }

    async fn expect_catalog(&mut self) {
        let change = tokio::time::timeout(NUDGE_LIMIT, self.changes.recv())
            .await
            .expect("A newly available chat must promptly notify a catalog reader.")
            .expect("The direct notification channel must stay open.");
        assert_eq!(change, Change::Catalog);
    }

    async fn expect_quiet(&mut self) {
        assert!(
            tokio::time::timeout(QUIET_WINDOW, self.changes.recv())
                .await
                .is_err(),
            "An unrelated change or a read must not notify this connection."
        );
    }

    fn stage(&self, directory: &str, chat: &str) -> PathBuf {
        let path = self.outside.join(directory);
        write_chat(&path.join("chat.jsonl"), chat);
        path
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn has_chat(page: &CatalogPage, id: &str) -> bool {
    page.entries
        .iter()
        .any(|chat| chat.native_id.as_deref() == Some(id))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn importing_a_populated_day_directory_notifies_and_then_lists_the_chat() {
    let mut endpoint = Endpoint::new().await;
    let before = endpoint.catalog().await;
    assert!(has_chat(&before, "initial"));
    assert!(!has_chat(&before, "imported"));
    let staged = endpoint.stage("incoming-day", "imported");
    // All child writes finish outside the watched root. Rename supplies one
    // completed directory, so no child-file notification is required.
    std::fs::rename(staged, endpoint.source.join("sessions/2026/09/29")).unwrap();
    endpoint.expect_catalog().await;
    assert!(has_chat(&endpoint.catalog().await, "imported"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn importing_a_populated_directory_tree_notifies_without_child_writes() {
    let mut endpoint = Endpoint::new().await;
    assert!(has_chat(&endpoint.catalog().await, "initial"));
    let staged = endpoint.outside.join("incoming-year");
    write_chat(&staged.join("01/01/new.jsonl"), "nested-import");
    std::fs::rename(staged, endpoint.source.join("sessions/2027")).unwrap();
    endpoint.expect_catalog().await;
    assert!(has_chat(&endpoint.catalog().await, "nested-import"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn changes_outside_the_disclosed_root_and_ordinary_files_stay_quiet() {
    let mut endpoint = Endpoint::new().await;
    assert!(has_chat(&endpoint.catalog().await, "initial"));
    let staged = endpoint.stage("outside-stage", "outside-only");
    std::fs::rename(staged, endpoint.outside.join("outside-import")).unwrap();
    std::fs::write(endpoint.source.join("notes.txt"), "unrelated text").unwrap();
    let _ = std::fs::read(endpoint.source.join("sessions/2026/09/28/initial.jsonl")).unwrap();
    endpoint.expect_quiet().await;
    let page = endpoint.catalog().await;
    assert!(has_chat(&page, "initial"));
    assert!(!has_chat(&page, "outside-only"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_connection_is_not_notified_before_its_first_catalog_read() {
    let mut endpoint = Endpoint::new().await;
    let staged = endpoint.stage("before-read", "unobserved-import");
    std::fs::rename(staged, endpoint.source.join("sessions/2026/09/29")).unwrap();
    endpoint.expect_quiet().await;
    assert!(has_chat(&endpoint.catalog().await, "unobserved-import"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starting_a_chat_in_an_existing_directory_still_notifies() {
    let mut endpoint = Endpoint::new().await;
    assert!(has_chat(&endpoint.catalog().await, "initial"));
    write_chat(
        &endpoint.source.join("sessions/2026/09/28/another.jsonl"),
        "ordinary-new-chat",
    );
    endpoint.expect_catalog().await;
    assert!(has_chat(&endpoint.catalog().await, "ordinary-new-chat"));
}
