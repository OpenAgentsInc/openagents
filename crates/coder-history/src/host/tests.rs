use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "coder-history-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
    fn history(&self) -> History {
        History::open(Config {
            codex: Some(self.0.clone()),
            claude: None,
        })
        .unwrap()
    }
    fn codex(&self, id: &str, tail: &str) -> PathBuf {
        self.write(
            &format!("sessions/2026/01/01/rollout-{id}.jsonl"),
            format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\"}}}}\n{tail}"),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn first(history: &History) -> Chat {
    history
        .catalog(CatalogRequest::default())
        .unwrap()
        .entries
        .remove(0)
}
fn read(
    history: &History,
    source: &str,
    cursor: Option<TranscriptCursor>,
    max: u32,
) -> TranscriptPage {
    history
        .transcript(TranscriptRequest {
            source_id: source.into(),
            cursor,
            max_bytes: max,
        })
        .unwrap()
}
fn collect(
    history: &History,
    source: &str,
    max: u32,
) -> (Vec<u8>, Vec<RecordChunk>, TranscriptCursor) {
    let mut bytes = Vec::new();
    let mut chunks = Vec::new();
    let mut cursor = None;
    for _ in 0..2000 {
        let page = read(history, source, cursor, max);
        assert!(encoded_len(&page).unwrap() <= MAX_RESPONSE_BYTES);
        for chunk in &page.chunks {
            assert_eq!(chunk.offset, bytes.len() as u64);
            bytes.extend(STANDARD.decode(&chunk.raw_base64).unwrap());
        }
        chunks.extend(page.chunks);
        cursor = Some(page.next);
        if !page.has_more {
            return (bytes, chunks, cursor.unwrap());
        }
    }
    panic!("synthetic transcript did not finish within its bounded pages");
}

#[test]
fn catalog_discovers_archive_titles_missing_sources_and_new_chats() {
    let fixture = Fixture::new();
    fixture.codex("active", "");
    fixture.write(
        "archived_sessions/archive.jsonl",
        b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"archive\"}}\n",
    );
    fixture.write("session_index.jsonl",b"{\"id\":\"active\",\"thread_name\":\"First title\",\"updated_at\":\"2026-01-01\"}\n{\"id\":\"active\",\"thread_name\":\"Renamed\",\"updated_at\":\"2026-01-02\"}\n{\"id\":\"missing\",\"thread_name\":\"Unavailable chat\"}\n");
    let history = fixture.history();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 3);
    assert!(
        page.entries
            .iter()
            .any(|c| c.title == "Renamed" && c.native_id.as_deref() == Some("active"))
    );
    assert!(
        page.entries
            .iter()
            .any(|c| c.archived && c.native_id.as_deref() == Some("archive"))
    );
    assert!(
        page.entries
            .iter()
            .any(|c| c.status == SourceStatus::Missing && c.source_id.is_none())
    );
    fixture.codex("later", "");
    assert_eq!(
        history
            .catalog(CatalogRequest::default())
            .unwrap()
            .entries
            .len(),
        4
    );
}

#[test]
fn catalog_cursor_ignores_titles_but_refuses_changed_membership() {
    let fixture = Fixture::new();
    fixture.codex("one", "");
    fixture.codex("two", "");
    let history = fixture.history();
    let first = history
        .catalog(CatalogRequest {
            cursor: None,
            limit: 1,
        })
        .unwrap();
    fixture.write(
        "session_index.jsonl",
        b"{\"id\":\"one\",\"thread_name\":\"Updated title\",\"updated_at\":\"new\"}\n",
    );
    let next = history
        .catalog(CatalogRequest {
            cursor: first.next.clone(),
            limit: 1,
        })
        .unwrap();
    assert_eq!(next.snapshot, first.snapshot);
    assert_ne!(next.entries[0].source_id, first.entries[0].source_id);
    fixture.codex("three", "");
    assert_eq!(
        history.catalog(CatalogRequest {
            cursor: first.next,
            limit: 1
        }),
        Err(Error::CursorStale)
    );
}

#[test]
fn every_raw_byte_and_unknown_record_survives_small_pages() {
    let fixture = Fixture::new();
    let path=fixture.codex("literal","{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"café 日本語 👩🏽‍💻\"}]}}\n{\"type\":\"future_record\",\"private\":\"literal\"}\nnot-json\n");
    let before = fs::read(&path).unwrap();
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let (bytes, chunks, cursor) = collect(&history, &source, 7);
    assert_eq!(bytes, before);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(chunks.iter().any(|c| {
        c.readable
            .as_ref()
            .is_some_and(|r| r.text.contains("日本語"))
    }));
    assert!(chunks.iter().any(|c| {
        c.readable
            .as_ref()
            .is_some_and(|r| r.kind == "future_record" && r.unknown)
    }));
    assert!(chunks.iter().any(|c| {
        c.readable
            .as_ref()
            .is_some_and(|r| r.kind == "invalid_json" && r.unknown)
    }));
    let eof = read(&history, &source, Some(cursor.clone()), 8);
    assert!(eof.chunks.is_empty());
    assert_eq!(eof.next, cursor);
    assert!(!eof.pending_line);
}

#[test]
fn oversized_records_are_chunked_in_full_not_discarded() {
    let fixture = Fixture::new();
    let text = "x".repeat(MAX_READABLE_RECORD_BYTES + 123);
    let path=fixture.codex("large",&format!("{{\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call_output\",\"output\":\"{text}\"}}}}\n"));
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let (bytes, chunks, _) = collect(&history, &source, MAX_PAGE_BYTES);
    assert_eq!(bytes, fs::read(path).unwrap());
    let large: Vec<_> = chunks.iter().filter(|c| c.index == 1).collect();
    assert!(large.len() > 1);
    assert!(large.iter().all(|c| c.id == large[0].id));
    assert!(large.last().unwrap().complete);
    assert!(large.last().unwrap().oversized);
    assert!(large.last().unwrap().readable.is_none());
}

#[test]
fn incomplete_tail_is_retained_and_completed_once_after_append() {
    let fixture = Fixture::new();
    let path = fixture.codex(
        "append",
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"hel",
    );
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let (mut bytes, chunks, cursor) = collect(&history, &source, MAX_PAGE_BYTES);
    assert!(!chunks.last().unwrap().complete);
    let record_id = chunks.last().unwrap().id.clone();
    let waiting = read(&history, &source, Some(cursor.clone()), MAX_PAGE_BYTES);
    assert!(waiting.pending_line);
    assert!(waiting.chunks.is_empty());
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"lo\"}}\n")
        .unwrap();
    let reopened = fixture.history();
    let page = read(&reopened, &source, Some(cursor), MAX_PAGE_BYTES);
    assert_eq!(page.chunks.len(), 1);
    assert_eq!(page.chunks[0].id, record_id);
    assert!(page.chunks[0].complete);
    assert_eq!(page.chunks[0].readable.as_ref().unwrap().text, "hello");
    bytes.extend(STANDARD.decode(&page.chunks[0].raw_base64).unwrap());
    assert_eq!(bytes, fs::read(path).unwrap());
}

#[test]
fn rewritten_prefix_truncation_replacement_and_forged_cursor_refuse() {
    let fixture = Fixture::new();
    let path = fixture.codex("changed", "{\"type\":\"future\"}\n");
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let page = read(&history, &source, None, 10);
    let original = fs::read(&path).unwrap();
    let query = |cursor| TranscriptRequest {
        source_id: source.clone(),
        cursor: Some(cursor),
        max_bytes: 8,
    };
    let mut changed = original.clone();
    changed[1] = b' ';
    fs::write(&path, &changed).unwrap();
    assert_eq!(
        history.transcript(query(page.next.clone())),
        Err(Error::SourceChanged)
    );
    fs::write(&path, b"x").unwrap();
    assert_eq!(
        history.transcript(query(page.next.clone())),
        Err(Error::SourceChanged)
    );
    fs::write(&path, &original).unwrap();
    let mut forged = page.next.clone();
    forged.record_offset = 1;
    assert_eq!(history.transcript(query(forged)), Err(Error::SourceChanged));
    let replacement = fixture.write("replacement.jsonl", &original);
    fs::rename(replacement, &path).unwrap();
    assert_eq!(
        history.transcript(query(page.next)),
        Err(Error::SourceChanged)
    );
}

#[test]
fn symlink_sources_and_directories_cannot_escape_selected_roots() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    outside.codex("outside", "");
    fixture.codex("inside", "");
    symlink(&outside.0, fixture.0.join("sessions/linked-directory")).unwrap();
    symlink(
        outside.0.join("sessions/2026/01/01/rollout-outside.jsonl"),
        fixture.0.join("sessions/linked.jsonl"),
    )
    .unwrap();
    let history = fixture.history();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(
        page.notices
            .iter()
            .filter(|n| n.code == "symlink_refused")
            .count(),
        2
    );
    for notice in page.notices {
        assert_eq!(
            history.transcript(TranscriptRequest {
                source_id: notice.source_id.unwrap(),
                cursor: None,
                max_bytes: 8
            }),
            Err(Error::SourceMissing)
        );
    }
    let alias = fixture.0.join("alias");
    symlink(&outside.0, &alias).unwrap();
    assert!(matches!(
        History::open(Config {
            codex: Some(alias),
            claude: None
        }),
        Err(Error::InvalidRoot)
    ));
}

#[test]
fn malformed_index_and_missing_or_empty_sources_are_explicit() {
    let fixture = Fixture::new();
    fixture.write("sessions/empty.jsonl", b"");
    fixture.write("session_index.jsonl", b"not-json\n{\"id\":\"pending");
    let page = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    assert_eq!(page.entries[0].status, SourceStatus::Empty);
    assert!(
        page.notices
            .iter()
            .any(|n| n.code == "title_index_partial_line")
    );
    assert!(
        page.notices
            .iter()
            .any(|n| n.code == "title_index_unrecognized_records")
    );
}

#[test]
fn claude_projects_and_subagents_are_separate_read_only_sources() {
    let fixture = Fixture::new();
    fixture.write("projects/project/session.jsonl",b"{\"type\":\"assistant\",\"sessionId\":\"session\",\"uuid\":\"record\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"name\":\"Read\",\"input\":{\"file_path\":\"synthetic.rs\"}}]}}\n");
    fixture.write("projects/project/session/subagents/agent.jsonl",b"{\"type\":\"user\",\"sessionId\":\"session\",\"uuid\":\"child-record\",\"message\":{\"role\":\"user\",\"content\":\"Synthetic child\"}}\n");
    let history = History::open(Config {
        codex: None,
        claude: Some(fixture.0.clone()),
    })
    .unwrap();
    let page = history.catalog(CatalogRequest::default()).unwrap();
    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.entries.iter().filter(|c| c.subagent).count(), 1);
    assert!(
        page.entries
            .iter()
            .all(|c| c.harness == Harness::Claude && c.native_id.as_deref() == Some("session"))
    );
    let main = page.entries.iter().find(|c| !c.subagent).unwrap();
    let (_, chunks, _) = collect(&history, main.source_id.as_ref().unwrap(), MAX_PAGE_BYTES);
    let readable = chunks.last().unwrap().readable.as_ref().unwrap();
    assert_eq!(readable.role.as_deref(), Some("assistant"));
    assert!(readable.text.contains("Tool: Read"));
    assert_eq!(readable.tool_name.as_deref(), Some("Read"));
}

#[test]
fn many_short_and_invalid_utf8_records_keep_contiguous_bounded_pages() {
    let fixture = Fixture::new();
    let mut raw = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"dense\"}}\n".to_vec();
    raw.extend_from_slice(&b"\n".repeat(300));
    raw.extend_from_slice(b"\xff\xfe\n");
    let path = fixture.write("sessions/dense.jsonl", &raw);
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    let first = read(&history, &source, None, MAX_PAGE_BYTES);
    assert_eq!(first.chunks.len(), 128);
    assert!(first.has_more);
    let (bytes, chunks, _) = collect(&history, &source, MAX_PAGE_BYTES);
    assert_eq!(bytes, fs::read(path).unwrap());
    assert!(
        chunks
            .iter()
            .all(|c| STANDARD.decode(&c.raw_base64).unwrap().len() <= MAX_CHUNK_BYTES)
    );
    assert!(chunks.last().unwrap().readable.as_ref().unwrap().unknown);
}

#[test]
fn moving_to_archive_retains_chat_identity_but_requires_a_new_source_cursor() {
    let fixture = Fixture::new();
    let path = fixture.codex("moved", "");
    let history = fixture.history();
    let old = first(&history);
    let source = old.source_id.unwrap();
    let cursor = read(&history, &source, None, MAX_PAGE_BYTES).next;
    fs::create_dir(fixture.0.join("archived_sessions")).unwrap();
    fs::rename(path, fixture.0.join("archived_sessions/moved.jsonl")).unwrap();
    let new = first(&history);
    assert_eq!(old.id, new.id);
    assert!(new.archived);
    assert_ne!(Some(source.clone()), new.source_id);
    assert_eq!(
        history.transcript(TranscriptRequest {
            source_id: source,
            cursor: Some(cursor),
            max_bytes: MAX_PAGE_BYTES
        }),
        Err(Error::SourceMissing)
    );
}

#[test]
fn request_and_response_bounds_are_enforced() {
    let fixture = Fixture::new();
    let path = fixture.codex("bounded", "");
    let history = fixture.history();
    let source = first(&history).source_id.unwrap();
    assert_eq!(
        history.catalog(CatalogRequest {
            cursor: None,
            limit: 0
        }),
        Err(Error::InvalidRequest)
    );
    for max in [0, MAX_PAGE_BYTES + 1] {
        assert_eq!(
            history.transcript(TranscriptRequest {
                source_id: source.clone(),
                cursor: None,
                max_bytes: max
            }),
            Err(Error::InvalidRequest)
        );
    }
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(confined::MAX_SOURCE_BYTES + 1)
        .unwrap();
    assert_eq!(
        history.transcript(TranscriptRequest {
            source_id: source,
            cursor: None,
            max_bytes: 8
        }),
        Err(Error::ResourceLimit)
    );
    assert!(readable_record(&vec![b' '; MAX_READABLE_RECORD_BYTES + 1]).is_none());
}

#[test]
fn unconfigured_harness_trees_and_credentials_are_never_cataloged() {
    let fixture = Fixture::new();
    fixture.codex("visible", "");
    fixture.write("auth.json", b"synthetic non-credential marker");
    fixture.write("projects/hidden.jsonl", b"{\"sessionId\":\"hidden\"}\n");
    fixture.write("unrelated/history.jsonl", b"private synthetic marker\n");
    let page = fixture
        .history()
        .catalog(CatalogRequest::default())
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(matches!(
        History::open(Config::default()),
        Err(Error::InvalidRoot)
    ));
    assert!(matches!(
        History::open(Config {
            codex: Some(Path::new("relative").into()),
            claude: None
        }),
        Err(Error::InvalidRoot)
    ));
}
