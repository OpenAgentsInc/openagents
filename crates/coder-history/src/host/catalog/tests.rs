use super::*;
use crate::host::Config;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "coder-history-index-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("codex")).unwrap();
        Self(root)
    }
    fn codex(&self, id: &str) -> PathBuf {
        let path = self
            .0
            .join(format!("codex/sessions/2026/01/01/rollout-{id}.jsonl"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\"}}}}\n"),
        )
        .unwrap();
        path
    }
    fn index(&self) -> PathBuf {
        self.0.join("catalog-index.json")
    }
    fn history(&self) -> History {
        History::open(Config {
            codex: Some(self.0.join("codex")),
            ..Config::default()
        })
        .unwrap()
        .with_catalog_index(self.index())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn request(cursor: Option<CatalogCursor>) -> CatalogRequest {
    CatalogRequest { cursor, limit: 1 }
}

#[test]
fn a_restarted_host_lists_from_its_index_and_ignores_a_bad_one() {
    let fixture = Fixture::new();
    let path = fixture.codex("kept");
    let first = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    assert_eq!(first.entries[0].native_id.as_deref(), Some("kept"));
    assert!(fixture.index().exists());
    assert_eq!(
        fs::metadata(fixture.index()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // A new process: nothing in memory. The file cannot be read now, and
    // the index still names it, because its identity and length are the
    // same ones the index kept.
    forget();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let restarted = fixture.history().catalog(CatalogRequest::default());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let restarted = restarted.unwrap();
    assert_eq!(restarted.entries, first.entries);
    assert_eq!(restarted.snapshot, first.snapshot);
    // An unrecognized index is ignored and the sources are read again.
    fs::write(fixture.index(), b"not an index").unwrap();
    forget();
    let again = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    assert_eq!(again.entries, first.entries);
    assert!(index::read(&fixture.index()).is_some());
}

#[test]
fn a_later_page_keeps_its_listing_until_a_directory_changes() {
    let fixture = Fixture::new();
    let one = fixture.codex("one");
    fixture.codex("two");
    fixture.codex("three");
    let history = fixture.history();
    let first = history.catalog(request(None)).unwrap();
    // A chat grows: its directory is the same, so the next page is a slice
    // of the same listing, in the same order, and repeats nothing.
    let mut bytes = fs::read(&one).unwrap();
    bytes.extend_from_slice(b"{\"type\":\"event_msg\"}\n");
    fs::write(&one, bytes).unwrap();
    let second = history.catalog(request(first.next.clone())).unwrap();
    let third = history.catalog(request(second.next.clone())).unwrap();
    let mut seen: Vec<_> = [&first, &second, &third]
        .iter()
        .map(|page| page.entries[0].native_id.clone().unwrap())
        .collect();
    seen.sort();
    assert_eq!(seen, ["one", "three", "two"]);
    assert!(third.next.is_none());
    // A new chat changes its directory: the members changed.
    fixture.codex("four");
    assert_eq!(
        history.catalog(request(first.next.clone())),
        Err(Error::CursorStale)
    );
    // A first page always reads again, and sees the grown chat as newest
    // once its last write is.
    let fresh = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(fresh.entries.len(), 4);
}

#[test]
fn an_unchanged_scan_reuses_its_listing_and_a_changed_title_does_not() {
    let fixture = Fixture::new();
    fixture.codex("one");
    let history = fixture.history();
    let before = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(history.catalog(CatalogRequest::default()).unwrap(), before);
    fs::write(
        fixture.0.join("codex/session_index.jsonl"),
        b"{\"id\":\"one\",\"thread_name\":\"Named\",\"updated_at\":\"x\"}\n",
    )
    .unwrap();
    let after = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(after.entries[0].title, "Named");
    assert_eq!(after.snapshot, before.snapshot);
}
