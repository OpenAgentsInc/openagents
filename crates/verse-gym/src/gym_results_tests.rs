//! The RESULTS panel over `gym_leaderboard::client`, against a local HTTP
//! fixture serving a copy of the committed publication at any ref.
//! Verification details are the client's own tests; these check what the
//! panel does with its events.

use super::*;
use std::io::{BufRead, BufReader, Write as _};
use std::net::TcpListener;
use std::path::Path;
use std::time::{Duration, Instant};

fn published() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/published")
}

/// Serves `dir`'s files at `/<ref>/<path>` for any ref, until the test
/// ends. Returns the base URL with `{ref}`.
fn serve(dir: &Path) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let dir = dir.to_owned();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/");
            let rel = path.trim_start_matches('/').split_once('/').map(|(_, r)| r);
            let body = rel
                .filter(|r| !r.contains(".."))
                .and_then(|r| std::fs::read(dir.join(r)).ok());
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
                None => b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    .to_vec(),
            };
            let _ = stream.write_all(&response);
        }
    });
    format!("http://127.0.0.1:{port}/{{ref}}/")
}

/// A base URL where nothing listens: offline.
fn nowhere() -> String {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    format!("http://127.0.0.1:{port}/{{ref}}/")
}

/// A copy of the publication.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy(&published(), dir.path());
    dir
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn results(base: String, cache: &Path) -> Results {
    Results::new(Config {
        base,
        cache_directory: Some(cache.into()),
    })
}

fn settle(results: &mut Results, done: impl Fn(&Results) -> bool) {
    let start = Instant::now();
    while !done(results) {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "{:?}",
            results.view().error
        );
        std::thread::sleep(Duration::from_millis(5));
        results.poll();
    }
}

fn freshness(results: &Results) -> Option<Freshness> {
    results.source.as_ref().map(|s| s.freshness)
}

fn board_count() -> usize {
    let leaderboard: Leaderboard = serde_json::from_slice(
        &std::fs::read(published().join(gym_leaderboard::LEADERBOARD_FILE)).unwrap(),
    )
    .unwrap();
    leaderboard.boards.len()
}

fn open(panel: &mut Results, board: &str, attempt: &str) {
    panel.act(Action::Board { id: board.into() }).unwrap();
    panel.act(Action::Attempt { id: attempt.into() }).unwrap();
    panel.act(Action::Trace).unwrap();
}

#[test]
fn entering_loads_the_current_publication_and_leaving_cancels() {
    let dir = fixture();
    let cache = tempfile::tempdir().unwrap();
    let mut panel = results(serve(dir.path()), cache.path());
    assert!(panel.view().page.is_none());
    assert!(panel.act(Action::Back).is_err(), "nothing to act on yet");
    panel.set_active(true);
    assert!(panel.view().loading);
    settle(&mut panel, |r| !r.view().loading);
    assert_eq!(freshness(&panel), Some(Freshness::Current));
    let view = panel.view();
    assert!(view.status.contains("current"), "{}", view.status);
    let Some(Page::Boards(list)) = view.page else {
        panic!("{view:?}")
    };
    assert_eq!(list.rows.len(), board_count());
    // Leaving drops the client; the loaded list stays on screen.
    panel.set_active(false);
    assert!(panel.client.is_none() && !panel.view().loading);
    assert!(panel.view().page.is_some());
}

#[test]
fn a_leaderboard_that_doesnt_verify_is_refused() {
    let dir = fixture();
    let cache = tempfile::tempdir().unwrap();
    let path = dir.path().join(gym_leaderboard::LEADERBOARD_FILE);
    // One tally changed, the digests left as they were.
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let passes = value["boards"][0]["totals"]["passes"].as_u64().unwrap();
    value["boards"][0]["totals"]["passes"] = serde_json::json!(passes + 1);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let mut panel = results(serve(dir.path()), cache.path());
    panel.set_active(true);
    settle(&mut panel, |r| !r.view().loading);
    assert!(panel.view().page.is_none());
    assert_eq!(
        panel.view().error.as_deref(),
        Some("Can't verify this publication")
    );
}

#[test]
fn offline_shows_the_cached_copy_labeled_and_without_a_cache_asks_for_a_connection() {
    let cache = tempfile::tempdir().unwrap();
    let mut panel = results(nowhere(), cache.path());
    panel.set_active(true);
    settle(&mut panel, |r| !r.view().loading);
    assert!(panel.view().page.is_none());
    assert_eq!(
        panel.view().error.as_deref(),
        Some("The published results need a connection once.")
    );
    // Online once fills the cache.
    let dir = fixture();
    let mut online = results(serve(dir.path()), cache.path());
    online.set_active(true);
    settle(&mut online, |r| !r.view().loading);
    // Offline again: the cached copy shows, labeled offline.
    let mut offline = results(nowhere(), cache.path());
    offline.set_active(true);
    settle(&mut offline, |r| !r.view().loading);
    assert_eq!(freshness(&offline), Some(Freshness::Offline));
    assert!(offline.view().status.contains("offline"));
    assert!(offline.view().page.is_some());
}

#[test]
fn a_trace_loads_on_demand_and_plays() {
    let dir = fixture();
    let cache = tempfile::tempdir().unwrap();
    let mut panel = results(serve(dir.path()), cache.path());
    panel.set_active(true);
    settle(&mut panel, |r| !r.view().loading);
    let revision = panel.revision();
    open(
        &mut panel,
        "tb4-fable-delegate-repro-9776",
        "coq-block-bound.p2",
    );
    assert!(panel.revision() > revision);
    assert!(panel.view().can_back);
    assert_eq!(panel.view().status, "Loading the trace…");
    assert!(
        panel.act(Action::Step { forward: true }).is_err(),
        "not loaded yet"
    );
    settle(&mut panel, |r| r.bundle.is_some());
    let Some(Page::Trace(trace)) = panel.view().page else {
        panic!()
    };
    assert_eq!(trace.header.task, "coq-block-bound");
    panel.act(Action::Play { playing: true }).unwrap();
    let before = panel.revision();
    while panel.playing() {
        panel.tick(1.0);
    }
    assert!(panel.revision() > before);
    assert!(panel.open_trace().is_some());
}

#[test]
fn a_bundle_that_changed_after_publication_is_refused() {
    let dir = fixture();
    let path = dir
        .path()
        .join("traces/tb4-fable-delegate-repro-9776/coq-block-bound.p1.json");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[10] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let mut panel = results(serve(dir.path()), cache.path());
    panel.set_active(true);
    settle(&mut panel, |r| !r.view().loading);
    open(
        &mut panel,
        "tb4-fable-delegate-repro-9776",
        "coq-block-bound.p1",
    );
    settle(&mut panel, |r| !r.view().loading);
    assert_eq!(
        panel.view().error.as_deref(),
        Some("Can't verify this trace")
    );
    assert!(panel.bundle.is_none());
    assert_eq!(panel.view().status, "The trace isn't available");
}

/// Every published bundle opens through the panel on demand, verified,
/// and renders every tab under the page bound; a frame never carries it.
#[test]
fn every_published_trace_opens_on_demand() {
    let cache = tempfile::tempdir().unwrap();
    let mut panel = results(serve(&published()), cache.path());
    panel.set_active(true);
    settle(&mut panel, |r| !r.view().loading);
    let leaderboard = panel.leaderboard.clone().unwrap();
    let mut opened = 0;
    for board in &leaderboard.boards {
        for attempt in board.attempts.iter().filter(|a| a.trace.is_some()) {
            open(&mut panel, &board.id, &attempt.id);
            settle(&mut panel, |r| {
                r.open_trace().is_some() || r.view().error.is_some()
            });
            assert!(
                panel.view().error.is_none(),
                "{}: {:?}",
                attempt.id,
                panel.view().error
            );
            for tab in [Tab::Jev, Tab::Briefing, Tab::Agent, Tab::Verifier] {
                panel.act(Action::Tab { tab }).unwrap();
                let bytes = serde_json::to_vec(&panel.view()).unwrap().len();
                assert!(bytes < view::MAX_PAGE_BYTES, "{}: {bytes}", attempt.id);
            }
            opened += 1;
            for _ in 0..3 {
                panel.act(Action::Back).unwrap();
            }
        }
    }
    assert!(opened >= 28, "{opened}");
}
