//! The RESULTS panel's loader and state over a local copy of the committed
//! publication.

use super::*;
use std::time::Instant;

fn published() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/published")
}

/// A copy of the publication with its index, leaderboard, and one bundle.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for rel in [
        gym_leaderboard::INDEX_FILE,
        gym_leaderboard::LEADERBOARD_FILE,
        "traces/tb4-fable-delegate-repro-9776/coq-block-bound.p2.json",
    ] {
        let to = dir.path().join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(published().join(rel), to).unwrap();
    }
    dir
}

fn base(dir: &Path) -> String {
    dir.to_str().unwrap().to_owned()
}

fn settle(results: &mut Results, done: impl Fn(&Results) -> bool) {
    let start = Instant::now();
    while !done(results) {
        assert!(
            start.elapsed() < Duration::from_secs(10),
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

#[test]
fn entering_loads_and_verifies_the_latest_publication_and_leaving_cancels() {
    let dir = fixture();
    let mut results = Results::new(Config {
        base: base(dir.path()),
        cache_directory: None,
    });
    assert!(results.view().page.is_none());
    assert!(results.act(Action::Back).is_err(), "nothing to act on yet");
    results.set_active(true);
    assert!(results.view().loading);
    settle(&mut results, |r| !r.view().loading);
    assert_eq!(freshness(&results), Some(Freshness::Current));
    let view = results.view();
    assert!(view.status.contains("current"), "{}", view.status);
    let Some(Page::Boards(list)) = view.page else {
        panic!("{view:?}")
    };
    let committed: Leaderboard = serde_json::from_slice(
        &std::fs::read(published().join(gym_leaderboard::LEADERBOARD_FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(list.rows.len(), committed.boards.len());
    // Leaving drops the worker; the loaded board stays on screen.
    results.set_active(false);
    assert!(results.worker.is_none());
    assert!(results.view().page.is_some());
}

#[test]
fn a_leaderboard_that_doesnt_match_its_index_is_refused() {
    let dir = fixture();
    let path = dir.path().join(gym_leaderboard::LEADERBOARD_FILE);
    // One tally changed, the digests left as they were.
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let passes = value["boards"][0]["totals"]["passes"].as_u64().unwrap();
    value["boards"][0]["totals"]["passes"] = serde_json::json!(passes + 1);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let mut results = Results::new(Config {
        base: base(dir.path()),
        cache_directory: None,
    });
    results.set_active(true);
    settle(&mut results, |r| !r.view().loading);
    assert!(results.view().page.is_none());
    assert!(
        results
            .view()
            .error
            .unwrap()
            .starts_with("Can't verify this publication")
    );
}

#[test]
fn an_oversized_file_is_refused_while_reading() {
    let dir = fixture();
    let path = dir.path().join(gym_leaderboard::INDEX_FILE);
    std::fs::write(&path, vec![b' '; INDEX_BYTES + 1]).unwrap();
    let mut results = Results::new(Config {
        base: base(dir.path()),
        cache_directory: None,
    });
    results.set_active(true);
    settle(&mut results, |r| !r.view().loading);
    assert!(results.view().error.unwrap().contains("size bound"));
}

#[test]
fn offline_shows_the_cached_copy_labeled_and_without_a_cache_asks_for_a_connection() {
    let cache = tempfile::tempdir().unwrap();
    let missing = tempfile::tempdir().unwrap();
    let gone = missing.path().join("nowhere");
    // Without a cache, offline says the results need a connection once.
    let mut results = Results::new(Config {
        base: base(&gone),
        cache_directory: Some(cache.path().into()),
    });
    results.set_active(true);
    settle(&mut results, |r| !r.view().loading);
    assert!(results.view().page.is_none());
    assert_eq!(
        results.view().error.as_deref(),
        Some("The published results need a connection once.")
    );
    // Online once fills the cache.
    let dir = fixture();
    let mut online = Results::new(Config {
        base: base(dir.path()),
        cache_directory: Some(cache.path().into()),
    });
    online.set_active(true);
    settle(&mut online, |r| !r.view().loading);
    // Offline again: the cached copy shows at once, labeled offline.
    let mut offline = Results::new(Config {
        base: base(&gone),
        cache_directory: Some(cache.path().into()),
    });
    offline.set_active(true);
    settle(&mut offline, |r| !r.view().loading);
    assert_eq!(freshness(&offline), Some(Freshness::Offline));
    assert!(offline.view().status.contains("offline"));
    assert!(offline.view().page.is_some());
    // And online with the same cache, the cached copy is current.
    let mut again = Results::new(Config {
        base: base(dir.path()),
        cache_directory: Some(cache.path().into()),
    });
    again.set_active(true);
    settle(&mut again, |r| freshness(r) == Some(Freshness::Current));
    assert!(again.source.as_ref().unwrap().commit.is_some());
}

#[test]
fn a_new_digest_in_the_index_replaces_the_cached_publication() {
    let cache = tempfile::tempdir().unwrap();
    let dir = fixture();
    let config = Config {
        base: base(dir.path()),
        cache_directory: Some(cache.path().into()),
    };
    let mut first = Results::new(config.clone());
    first.set_active(true);
    settle(&mut first, |r| !r.view().loading);
    first
        .act(Action::Board {
            id: "tb21-oos-microcoder-9683".into(),
        })
        .unwrap();
    let old = first.source.clone().unwrap().digest;
    // A new publication: one board dropped, digest recomputed, index
    // appended.
    let path = dir.path().join(gym_leaderboard::LEADERBOARD_FILE);
    let mut leaderboard: Leaderboard =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    leaderboard.boards.truncate(1);
    leaderboard.digest = atif::digest(&serde_json::to_value(&leaderboard.boards).unwrap());
    std::fs::write(&path, serde_json::to_vec(&leaderboard).unwrap()).unwrap();
    let index_path = dir.path().join(gym_leaderboard::INDEX_FILE);
    let mut index: Index = serde_json::from_slice(&std::fs::read(&index_path).unwrap()).unwrap();
    let mut next = index.publications[0].clone();
    next.digest.clone_from(&leaderboard.digest);
    index.publications.push(next);
    std::fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    // Re-entering shows the cached copy, then moves to the new one and
    // back to the list, since the open board may be gone.
    first.set_active(false);
    first.set_active(true);
    settle(&mut first, |r| {
        r.source.as_ref().is_some_and(|s| s.digest != old) && !r.view().loading
    });
    assert_eq!(freshness(&first), Some(Freshness::Current));
    let Some(Page::Boards(list)) = first.view().page else {
        panic!()
    };
    assert_eq!(list.rows.len(), 1);
}

#[test]
fn a_trace_loads_on_demand_verifies_and_plays() {
    let cache = tempfile::tempdir().unwrap();
    let dir = fixture();
    let mut results = Results::new(Config {
        base: base(dir.path()),
        cache_directory: Some(cache.path().into()),
    });
    results.set_active(true);
    settle(&mut results, |r| !r.view().loading);
    results
        .act(Action::Board {
            id: "tb4-fable-delegate-repro-9776".into(),
        })
        .unwrap();
    results
        .act(Action::Attempt {
            id: "coq-block-bound.p2".into(),
        })
        .unwrap();
    let revision = results.revision();
    results.act(Action::Trace).unwrap();
    assert!(results.revision() > revision);
    assert!(
        results.act(Action::Step { forward: true }).is_err(),
        "not loaded yet"
    );
    settle(&mut results, |r| r.bundle.is_some());
    let Some(Page::Trace(trace)) = results.view().page else {
        panic!()
    };
    assert_eq!(trace.header.task, "coq-block-bound");
    results.act(Action::Play { playing: true }).unwrap();
    assert!(results.playing());
    let before = results.revision();
    while results.playing() {
        results.tick(1.0);
    }
    assert!(results.revision() > before);
    assert!(results.open_trace().is_some());
    // The bundle is cached by digest; a tampered bundle isn't accepted.
    let cached = cache.path().join("gym-results");
    assert!(std::fs::read_dir(&cached).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("bundle-")
    }));
    results.act(Action::Back).unwrap();
    results.bundle = None;
    std::fs::remove_dir_all(&cached).unwrap();
    let path = dir
        .path()
        .join("traces/tb4-fable-delegate-repro-9776/coq-block-bound.p2.json");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[10] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    results.act(Action::Trace).unwrap();
    settle(&mut results, |r| !r.view().loading);
    assert_eq!(
        results.view().error.as_deref(),
        Some("Can't verify this trace")
    );
    assert!(results.bundle.is_none());
}

#[test]
fn a_field_this_reader_doesnt_know_still_verifies() {
    let dir = fixture();
    let path = dir.path().join(gym_leaderboard::LEADERBOARD_FILE);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["boards"][0]["added_later"] = serde_json::json!("an optional field");
    let digest = atif::digest(&value["boards"]);
    value["digest"] = serde_json::json!(digest);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let index_path = dir.path().join(gym_leaderboard::INDEX_FILE);
    let mut index: Index = serde_json::from_slice(&std::fs::read(&index_path).unwrap()).unwrap();
    index.publications.last_mut().unwrap().digest = digest;
    std::fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let mut results = Results::new(Config {
        base: base(dir.path()),
        cache_directory: None,
    });
    results.set_active(true);
    settle(&mut results, |r| !r.view().loading);
    assert!(results.view().error.is_none(), "{:?}", results.view().error);
    assert!(results.view().page.is_some());
}

#[test]
fn only_https_or_a_local_directory_is_an_origin() {
    assert!(Origin::new("http://example.com/").is_err());
    assert!(Origin::new("https://example.com/no-slash").is_err());
    assert!(Origin::new("relative/path").is_err());
    assert!(Origin::new(DEFAULT_BASE_URL).is_ok());
}
