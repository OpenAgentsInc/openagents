//! Retrospective deep-import diagnostic through the public observer API.
//! This is not part of the registered five-case checker or its score.
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
async fn a_catalog_visible_deep_import_notifies_without_child_writes() {
    let mut endpoint = Endpoint::new().await;
    let staged = endpoint.outside.join("deep-import");
    // found_in starts at depth 0 and returns before reading depth 4. The
    // catalog supports this seven-component path below its source root.
    write_chat(
        &staged.join("one/two/three/four/chat.jsonl"),
        "deep-imported-chat",
    );
    let before = endpoint.catalog().await;
    assert!(has_chat(&before, "initial"));
    assert!(!has_chat(&before, "deep-imported-chat"));
    endpoint.expect_quiet().await;

    // Finish every child write outside the watched root, then import one
    // directory atomically. No later child write can mask a missed arrival.
    let imported_at = tokio::time::Instant::now();
    std::fs::rename(staged, endpoint.source.join("sessions/deep-import")).unwrap();

    // Save the notification outcome before another read can affect the
    // observer's dirty or coalescing state. The receiver stays attached.
    let notification =
        tokio::time::timeout_at(imported_at + NUDGE_LIMIT, endpoint.changes.recv()).await;

    // Even after a notification timeout, prove that the import is supported
    // public catalog data before asserting the saved notification result.
    let after = endpoint.catalog().await;
    assert!(
        has_chat(&after, "deep-imported-chat"),
        "The imported chat must be visible through a fresh public catalog read."
    );
    let change = notification
        .expect("A catalog-visible imported chat must notify within the existing four-second bound.")
        .expect("The direct notification channel must stay open.");
    assert_eq!(change, Change::Catalog);
}
