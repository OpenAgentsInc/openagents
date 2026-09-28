//! The client against a local HTTP fixture: happy path, a digest that
//! doesn't match, an oversize body, offline with and without a cache, the
//! index moving to a new digest, eviction, and requests that carry no
//! credential, cookie, or identity.

#![cfg(feature = "client")]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gym_leaderboard::client::{Client, ClientError, Config, Event, Fetcher, Freshness, Request};
use gym_leaderboard::contract::{Leaderboard, TraceRef};
use gym_leaderboard::verify::Refusal;
use serde_json::Value;

fn published() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(gym_leaderboard::PUBLISHED)
}

/// A request's path and header lines.
type Seen = (String, Vec<String>);

/// Files served by path, and every request's header lines.
#[derive(Clone, Default)]
struct Site {
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    requests: Arc<Mutex<Vec<Seen>>>,
}

impl Site {
    fn put(&self, path: &str, bytes: Vec<u8>) {
        self.files.lock().unwrap().insert(path.to_owned(), bytes);
    }

    fn remove(&self, path: &str) {
        self.files.lock().unwrap().remove(path);
    }

    /// Serves until the test ends; returns the base URL with `{ref}`.
    fn serve(&self) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let site = self.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let mut headers = Vec::new();
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
                        break;
                    }
                    headers.push(h.trim().to_owned());
                }
                site.requests.lock().unwrap().push((path.clone(), headers));
                let body = site.files.lock().unwrap().get(&path).cloned();
                let response = match body {
                    Some(body) => {
                        let mut r = format!(
                            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                            body.len()
                        )
                        .into_bytes();
                        r.extend(body);
                        r
                    }
                    None => {
                        b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                            .to_vec()
                    }
                };
                let _ = stream.write_all(&response);
            }
        });
        format!("http://127.0.0.1:{port}/{{ref}}/")
    }
}

fn leaderboard_bytes() -> Vec<u8> {
    std::fs::read(published().join(gym_leaderboard::LEADERBOARD_FILE)).unwrap()
}

fn digest_of(bytes: &[u8]) -> String {
    let v: Value = serde_json::from_slice(bytes).unwrap();
    v["digest"].as_str().unwrap().to_owned()
}

fn index(entries: &[(&str, Option<&str>)]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema": "openagents.gym.leaderboard-index.v1",
        "publications": entries.iter().map(|(d, c)| serde_json::json!({
            "digest": d, "commit": c, "boards": []
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

/// A second valid leaderboard: the committed one with a board renamed.
fn another_leaderboard() -> Vec<u8> {
    let mut v: Value = serde_json::from_slice(&leaderboard_bytes()).unwrap();
    v["boards"][0]["title"] = Value::from("renamed");
    v["digest"] = Value::from(atif::digest(&v["boards"]));
    serde_json::to_vec(&v).unwrap()
}

fn first_trace(bytes: &[u8]) -> TraceRef {
    let lb: Leaderboard = serde_json::from_slice(bytes).unwrap();
    lb.boards
        .iter()
        .flat_map(|b| &b.attempts)
        .find_map(|a| a.trace.clone())
        .unwrap()
}

/// A site at commit `abc123` and `main`, and a fetcher for it.
fn setup() -> (Site, tempfile::TempDir, Fetcher, String) {
    let site = Site::default();
    let bytes = leaderboard_bytes();
    let digest = digest_of(&bytes);
    site.put("/main/index.json", index(&[(&digest, Some("abc123"))]));
    site.put("/abc123/leaderboard.v1.json", bytes.clone());
    let trace = first_trace(&bytes);
    site.put(
        &format!("/abc123/{}", trace.path),
        std::fs::read(published().join(&trace.path)).unwrap(),
    );
    let base = site.serve();
    let cache = tempfile::tempdir().unwrap();
    let mut config = Config::new(cache.path());
    config.base_url = base.clone();
    config.timeout = Duration::from_secs(5);
    let fetcher = Fetcher::new(config).unwrap();
    (site, cache, fetcher, base)
}

fn go() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn happy_path_verifies_caches_and_reads_the_cache_back() {
    let (site, cache, fetcher, _) = setup();
    let loaded = fetcher.refresh(&go()).unwrap();
    assert_eq!(loaded.freshness, Freshness::Current);
    assert_eq!(loaded.commit.as_deref(), Some("abc123"));
    assert_eq!(loaded.digest, digest_of(&leaderboard_bytes()));
    assert!(
        cache
            .path()
            .join(format!("leaderboards/{}.json", loaded.digest))
            .is_file()
    );
    // The bundle is fetched at the same commit and checked by SHA-256.
    let trace = first_trace(&leaderboard_bytes());
    let bundle = fetcher.bundle(&trace, &go()).unwrap();
    assert_eq!(
        bundle.attempt,
        trace
            .path
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".json")
    );
    // A second refresh with the same digest reads nothing but the index.
    let before = site.requests.lock().unwrap().len();
    let again = fetcher.refresh(&go()).unwrap();
    assert_eq!(again.freshness, Freshness::Current);
    let paths: Vec<String> = site.requests.lock().unwrap()[before..]
        .iter()
        .map(|(p, _)| p.clone())
        .collect();
    assert_eq!(paths, vec!["/main/index.json".to_owned()]);
    // A cached bundle needs no request.
    let before = site.requests.lock().unwrap().len();
    fetcher.bundle(&trace, &go()).unwrap();
    assert_eq!(site.requests.lock().unwrap().len(), before);
}

#[test]
fn requests_carry_no_credential_cookie_or_identity() {
    let (site, _cache, fetcher, _) = setup();
    fetcher.refresh(&go()).unwrap();
    fetcher
        .bundle(&first_trace(&leaderboard_bytes()), &go())
        .unwrap();
    let requests = site.requests.lock().unwrap();
    assert!(requests.len() >= 3);
    for (path, headers) in requests.iter() {
        for h in headers {
            let name = h.split(':').next().unwrap().to_ascii_lowercase();
            assert!(
                matches!(name.as_str(), "host" | "accept"),
                "{path}: sent {h}"
            );
        }
    }
}

#[test]
fn a_leaderboard_that_doesnt_match_the_index_is_refused_and_the_cache_kept() {
    let (site, _cache, fetcher, _) = setup();
    // No cache yet: refused outright.
    let bytes = leaderboard_bytes();
    let tampered = String::from_utf8(bytes.clone())
        .unwrap()
        .replacen("\"passes\":13", "\"passes\":14", 1)
        .into_bytes();
    site.put("/abc123/leaderboard.v1.json", tampered.clone());
    site.put("/main/leaderboard.v1.json", tampered.clone());
    let err = fetcher.refresh(&go()).unwrap_err();
    assert!(
        matches!(err, ClientError::Refused(Refusal::DigestMismatch { .. })),
        "{err}"
    );
    // With a cache: the verified copy stays, with the problem.
    site.put("/abc123/leaderboard.v1.json", bytes.clone());
    fetcher.refresh(&go()).unwrap();
    let other = another_leaderboard();
    site.put(
        "/main/index.json",
        index(&[(&digest_of(&other), Some("def456"))]),
    );
    site.put("/def456/leaderboard.v1.json", tampered);
    let loaded = fetcher.refresh(&go()).unwrap();
    assert_eq!(loaded.digest, digest_of(&bytes));
    assert!(matches!(
        loaded.freshness,
        Freshness::Cached {
            problem: Some(ClientError::Refused(Refusal::DigestMismatch { .. }))
        }
    ));
    // A bundle whose bytes don't match its reference is refused.
    let mut trace = first_trace(&bytes);
    trace.sha256 = "0".repeat(64);
    let err = fetcher.bundle(&trace, &go()).unwrap_err();
    assert!(matches!(
        err,
        ClientError::Refused(Refusal::DigestMismatch { .. })
    ));
}

#[test]
fn an_oversize_body_is_refused_while_reading() {
    let (site, _cache, fetcher, _) = setup();
    site.put(
        "/abc123/leaderboard.v1.json",
        vec![b' '; gym_leaderboard::MAX_LEADERBOARD_BYTES + 1],
    );
    site.remove("/main/leaderboard.v1.json");
    let err = fetcher.refresh(&go()).unwrap_err();
    // The fallback ref had nothing; the oversize read is what failed first.
    assert!(
        matches!(
            err,
            ClientError::Refused(Refusal::TooLarge { .. }) | ClientError::NeedsConnection(_)
        ),
        "{err}"
    );
    site.put(
        "/main/leaderboard.v1.json",
        vec![b' '; gym_leaderboard::MAX_LEADERBOARD_BYTES + 1],
    );
    let err = fetcher.refresh(&go()).unwrap_err();
    assert!(
        matches!(err, ClientError::Refused(Refusal::TooLarge { .. })),
        "{err}"
    );
    // A bundle over its bound.
    site.put("/abc123/leaderboard.v1.json", leaderboard_bytes());
    fetcher.refresh(&go()).unwrap();
    let trace = first_trace(&leaderboard_bytes());
    site.put(
        &format!("/abc123/{}", trace.path),
        vec![b' '; gym_leaderboard::MAX_BUNDLE_BYTES + 1],
    );
    let err = fetcher.bundle(&trace, &go()).unwrap_err();
    assert!(
        matches!(err, ClientError::Refused(Refusal::TooLarge { .. })),
        "{err}"
    );
}

#[test]
fn offline_shows_the_cache_or_says_it_needs_a_connection() {
    // Nothing listens on the port the fetcher is pointed at.
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let cache = tempfile::tempdir().unwrap();
    let mut config = Config::new(cache.path());
    config.base_url = format!("http://127.0.0.1:{closed}/{{ref}}/");
    config.connect_timeout = Duration::from_secs(2);
    let offline = Fetcher::new(config.clone()).unwrap();
    let err = offline.refresh(&go()).unwrap_err();
    assert!(matches!(err, ClientError::NeedsConnection(_)), "{err}");

    // Fill the same cache from a live site, then go offline.
    let (_site, _other_cache, _, base) = setup();
    let mut live = config.clone();
    live.base_url = base;
    Fetcher::new(live).unwrap().refresh(&go()).unwrap();
    let loaded = offline.refresh(&go()).unwrap();
    assert_eq!(loaded.freshness, Freshness::Offline);
    assert_eq!(loaded.digest, digest_of(&leaderboard_bytes()));
    assert!(loaded.age_seconds(loaded.checked_at + 90) == 90);
}

#[test]
fn the_index_moving_to_a_new_digest_loads_the_new_publication() {
    let (site, cache, fetcher, _) = setup();
    let first = fetcher.refresh(&go()).unwrap();
    let other = another_leaderboard();
    let new_digest = digest_of(&other);
    // The new publication isn't at its evidence commit yet: it's on main.
    site.put(
        "/main/index.json",
        index(&[
            (&first.digest, Some("abc123")),
            (&new_digest, Some("def456")),
        ]),
    );
    site.put("/main/leaderboard.v1.json", other);
    let second = fetcher.refresh(&go()).unwrap();
    assert_eq!(second.digest, new_digest);
    assert_eq!(second.freshness, Freshness::Current);
    assert_eq!(second.leaderboard.boards[0].title, "renamed");
    assert!(
        cache
            .path()
            .join(format!("leaderboards/{new_digest}.json"))
            .is_file()
    );
    // And the cache now answers with the new one.
    assert_eq!(fetcher.cached().unwrap().digest, new_digest);
}

#[test]
fn the_cache_evicts_least_recently_used_bundles_above_its_cap() {
    let (site, cache, _, base) = setup();
    let bytes = leaderboard_bytes();
    let lb: Leaderboard = serde_json::from_slice(&bytes).unwrap();
    let traces: Vec<TraceRef> = lb
        .boards
        .iter()
        .flat_map(|b| &b.attempts)
        .filter_map(|a| a.trace.clone())
        .take(3)
        .collect();
    for t in &traces {
        site.put(
            &format!("/abc123/{}", t.path),
            std::fs::read(published().join(&t.path)).unwrap(),
        );
    }
    let mut config = Config::new(cache.path());
    config.base_url = base;
    // Room for the leaderboard and the last bundle only.
    config.cache_bytes = bytes.len() as u64 + traces[2].bytes;
    let fetcher = Fetcher::new(config).unwrap();
    fetcher.refresh(&go()).unwrap();
    for t in &traces {
        fetcher.bundle(t, &go()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    let bundles: Vec<String> = std::fs::read_dir(cache.path().join("bundles"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(bundles, vec![format!("{}.json", traces[2].sha256)]);
    assert!(fetcher.cached().is_some(), "the current leaderboard stays");
}

#[test]
fn a_cancelled_request_stops_and_the_worker_reports_the_latest() {
    let (_site, cache, fetcher, base) = setup();
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        fetcher.refresh(&cancelled).unwrap_err(),
        ClientError::Cancelled
    );

    let mut config = Config::new(cache.path());
    config.base_url = base;
    let client = Client::start(config).unwrap();
    let first = client.request(Request::Refresh);
    let second = client.request(Request::Refresh);
    assert!(second > first);
    let mut latest = None;
    while let Some(event) = client.wait(Duration::from_secs(10)) {
        if let Event::Leaderboard { seq, loaded } = &event
            && *seq == second
            && loaded.freshness == Freshness::Current
        {
            latest = Some(loaded.digest.clone());
            break;
        }
        if let Event::Failed { seq, error } = &event {
            assert!(*seq == first && *error == ClientError::Cancelled, "{error}");
        }
    }
    assert_eq!(latest, Some(digest_of(&leaderboard_bytes())));
    // Leaving cancels whatever is in flight; nothing blocks.
    client.cancel();
    drop(client);
}
