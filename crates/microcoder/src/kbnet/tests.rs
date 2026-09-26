//! Publishing and syncing against an in-process relay: it challenges with
//! NIP-42, checks signatures, keeps the latest addressable event, and
//! answers NIP-01 filters with `crates/nostr`'s own matcher. The signing
//! key is created in a scratch directory by the code under test.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use knowledge::search::Retriever;
use nostr::domain::{Event, Filter, matches_any};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::*;

pub(crate) type Store = Arc<Mutex<Vec<Event>>>;

pub(crate) async fn relay() -> (String, Store) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let store: Store = Arc::default();
    let shared = store.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(serve(stream, shared.clone()));
        }
    });
    (url, store)
}

fn reply(value: Value) -> Message {
    Message::Text(value.to_string().into())
}

async fn serve(stream: tokio::net::TcpStream, store: Store) {
    let mut ws = accept_async(stream).await.unwrap();
    ws.send(reply(json!(["AUTH", "challenge-1"])))
        .await
        .unwrap();
    while let Some(Ok(message)) = ws.next().await {
        let Message::Text(text) = message else {
            continue;
        };
        let value: Value = serde_json::from_str(text.as_str()).unwrap();
        let answers = match value[0].as_str().unwrap_or_default() {
            "AUTH" | "EVENT" => {
                let event: Event = serde_json::from_value(value[1].clone()).unwrap();
                let ok = |accepted: bool, why: &str| vec![json!(["OK", event.id, accepted, why])];
                if event.validate_crypto().is_err() {
                    ok(false, "invalid: bad signature")
                } else if value[0] == "AUTH" {
                    ok(true, "")
                } else {
                    let mut events = store.lock().unwrap();
                    if events.iter().any(|e| e.id == event.id) {
                        ok(true, "duplicate: already have it")
                    } else {
                        if (30_000..40_000).contains(&event.kind) {
                            let d = event.tag_values("d").next().map(str::to_string);
                            events.retain(|e| {
                                !(e.kind == event.kind
                                    && e.pubkey == event.pubkey
                                    && e.tag_values("d").next().map(str::to_string) == d)
                            });
                        }
                        events.push(event.clone());
                        ok(true, "")
                    }
                }
            }
            "REQ" => {
                let filters: Vec<Filter> = value.as_array().unwrap()[2..]
                    .iter()
                    .map(|f| serde_json::from_value(f.clone()).unwrap())
                    .collect();
                let events = store.lock().unwrap();
                let mut out: Vec<Value> = events
                    .iter()
                    .filter(|e| matches_any(&filters, e))
                    .map(|e| json!(["EVENT", value[1], e]))
                    .collect();
                out.push(json!(["EOSE", value[1]]));
                out
            }
            _ => Vec::new(),
        };
        for answer in answers {
            ws.send(reply(answer)).await.unwrap();
        }
    }
}

pub(crate) fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("microcoder-kbnet-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A writable copy of the seed entries.
fn seed_copy(name: &str) -> PathBuf {
    let dir = scratch(name);
    let seed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../knowledge");
    for file in std::fs::read_dir(seed).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_some_and(|x| x == "md") {
            std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
        }
    }
    dir
}

fn options(words: &[&str]) -> Options {
    let args: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
    knowledge::cli::parse(&args).unwrap()
}

fn count(store: &Store, kind: u16) -> usize {
    store
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.kind == kind)
        .count()
}

fn edit(path: &Path, from: &str, to: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains(from), "{from}");
    std::fs::write(path, text.replacen(from, to, 1)).unwrap();
}

#[tokio::test]
async fn entries_publish_sync_and_retrieve_through_a_relay() {
    let (url, store) = relay().await;
    let dir = seed_copy("entries");
    let home = scratch("entries-home");
    let key = home.join("nostr/knowledge-key");
    let d = dir.to_str().unwrap();
    let publish_all = options(&["--dir", d, "--relay", &url]);
    // Every entry file in the seed copy, and the admitted ones among them;
    // a reader leaves synced candidates out unless asked for them.
    let n = std::fs::read_dir(&dir).unwrap().count();
    let admitted = std::fs::read_dir(&dir)
        .unwrap()
        .filter(|f| {
            std::fs::read_to_string(f.as_ref().unwrap().path())
                .unwrap()
                .contains("\nstatus: admitted\n")
        })
        .count();

    // First publish: every entry and its head. The key is made on first use.
    assert_eq!(publish(&publish_all, &key).await.unwrap(), 0);
    assert_eq!(count(&store, kb::ENTRY_KIND), n);
    assert_eq!(count(&store, kb::HEAD_KIND), n);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&key).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let me = remote::own_pubkey(&key).unwrap();

    // Again: nothing new.
    assert_eq!(publish(&publish_all, &key).await.unwrap(), 0);
    assert_eq!(store.lock().unwrap().len(), 2 * n);

    // Changed without a new version: refused. With one: a new version, and
    // the head moves to it.
    let path = dir.join("shell.heredoc-quoting.md");
    edit(&path, "## ", "Revised.\n\n## ");
    let one = options(&["--dir", d, "--relay", &url, "shell.heredoc-quoting"]);
    assert_eq!(publish(&one, &key).await.unwrap(), 1);
    edit(&path, "version: 1", "version: 2");
    assert_eq!(publish(&one, &key).await.unwrap(), 0);
    assert_eq!(count(&store, kb::ENTRY_KIND), n + 1);
    assert_eq!(count(&store, kb::HEAD_KIND), n);

    // Sync into a fresh cache, then search it with no local entries at all.
    let cache = scratch("entries-cache");
    let sync_all = options(&[
        "--relay",
        &url,
        "--author",
        &remote::npub(&me),
        "--corpus",
        d,
    ]);
    assert_eq!(sync(&sync_all, &key, &cache).await.unwrap(), 0);
    let empty = scratch("entries-local");
    let trust = remote::TrustConfig::default();
    let (base, loaded) = remote::load(&empty, Some(&cache), &trust, Some(&me), false).unwrap();
    assert_eq!(loaded.remote.get(&me), Some(&admitted));
    assert_eq!(base.get("shell.heredoc-quoting").unwrap().version, 2);
    assert!(base.entries.iter().all(|e| e.author == remote::npub(&me)));
    let retriever = Retriever::<knowledge::search::OpenRouterEmbedder>::lexical(base, "test");
    let search = retriever
        .search("mmd kernel two-sample estimator rbf", 3)
        .await;
    assert_eq!(search.hits[0].id, "statistics.mmd-estimators");
    // Someone else's reader, trusting no one, shows none of them.
    let (base, _) = remote::load(&empty, Some(&cache), &trust, None, false).unwrap();
    assert!(base.entries.is_empty());

    // Withdrawn here, withdrawn on the relay, gone from the next sync.
    let gone = dir.join("numerics.float-comparison.md");
    let text = std::fs::read_to_string(&gone).unwrap();
    let text = knowledge::set_status(&text, Status::Withdrawn).unwrap();
    let text =
        knowledge::set_evidence(&text, &["withdrawn 2026-09-25: a test".to_string()]).unwrap();
    std::fs::write(&gone, text).unwrap();
    let one = options(&["--dir", d, "--relay", &url, "numerics.float-comparison"]);
    assert_eq!(publish(&one, &key).await.unwrap(), 0);
    assert_eq!(count(&store, kb::WITHDRAWAL_KIND), 1);
    assert_eq!(sync(&sync_all, &key, &cache).await.unwrap(), 0);
    let (base, _) = remote::load(&empty, Some(&cache), &trust, Some(&me), false).unwrap();
    assert_eq!(base.entries.len(), admitted - 1);
    assert!(base.get("numerics.float-comparison").is_none());
}

fn run(dir: &Path, name: &str, reward: f64, used: &[&str]) {
    let at = dir.join(name);
    std::fs::create_dir_all(&at).unwrap();
    let task = knowledge::evidence::task_of(name);
    let knowledge: Vec<Value> = used.iter().map(|id| json!({"id": id})).collect();
    let summary = json!({"task": task, "model": "m", "reward": reward,
        "outcome": {"model_usd": 0.1, "knowledge": knowledge}});
    std::fs::write(at.join("summary.json"), summary.to_string()).unwrap();
}

#[tokio::test]
async fn evidence_is_published_for_a_published_entry_and_synced_back() {
    let (url, store) = relay().await;
    let dir = seed_copy("evidence");
    let key = scratch("evidence-home").join("knowledge-key");
    let runs = scratch("evidence-runs");
    for (name, reward, used) in [
        ("task-a-1", 1.0, &["slip.tests-from-the-same-belief"][..]),
        ("task-a-2", 0.0, &[][..]),
        ("task-b-3", 1.0, &["slip.tests-from-the-same-belief"][..]),
        ("task-b-4", 0.0, &[][..]),
    ] {
        run(&runs, name, reward, used);
    }
    let evidence_dir = scratch("evidence-reports");
    let d = dir.to_str().unwrap();
    let one = |extra: &[&str]| {
        let mut words = vec!["--dir", d, "--relay", url.as_str()];
        words.extend_from_slice(extra);
        options(&words)
    };
    let id = "slip.tests-from-the-same-belief";
    let args = [
        "--runs",
        runs.to_str().unwrap(),
        "--evidence-dir",
        evidence_dir.to_str().unwrap(),
        id,
    ];
    // Not published yet: nothing to cite.
    assert_eq!(publish_evidence(&one(&args), &key).await.unwrap(), 0);
    assert_eq!(count(&store, kb::EVIDENCE_KIND), 0);
    assert_eq!(publish(&one(&[id]), &key).await.unwrap(), 0);
    assert_eq!(publish_evidence(&one(&args), &key).await.unwrap(), 0);
    let events = store.lock().unwrap().clone();
    let published = events.iter().find(|e| e.kind == kb::EVIDENCE_KIND).unwrap();
    let parsed = kb::parse_evidence(published).unwrap();
    let report: Value = serde_json::from_str(&parsed.report_bytes).unwrap();
    assert_eq!(report["verdict"], "pass");
    assert_eq!(report["evaluator"], published.pubkey);
    assert!(
        evidence_dir
            .join(format!("published/{}.json", published.id))
            .exists()
    );

    let cache = scratch("evidence-cache");
    assert_eq!(sync(&one(&[]), &key, &cache).await.unwrap(), 0);
    assert!(
        cache
            .join(format!("evidence/{}.json", published.id))
            .exists()
    );
}

#[tokio::test]
async fn publishing_needs_a_named_relay() {
    let dir = seed_copy("no-relay");
    let key = scratch("no-relay-home").join("knowledge-key");
    let error = publish(&options(&["--dir", dir.to_str().unwrap()]), &key)
        .await
        .unwrap_err();
    assert!(error.contains("--relay"));
    assert!(!key.exists());
}
